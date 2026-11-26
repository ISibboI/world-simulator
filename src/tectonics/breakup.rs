//! Plates break where the stress across a section exceeds the strength of the lithosphere
//! (#123).
//!
//! Every step the deforming-plate solve ([`super::sheet`]) gives the forces on the lithosphere
//! of every cell (drag against the mantle at the material velocity, slab pull and the
//! resistances at the plate's boundaries, ridge push, the push of thickened crust) and the
//! velocities of the material. Weak lithosphere carries its stress by viscous flow, which the
//! sheet resolves; strong lithosphere moves as rigid blocks, whose internal stress the sheet
//! does not compute. It follows from statics: a section that divides a plate into two parts
//! `A` and `B` must carry the net torque `T_A` of the external loads on `A` (the loads of the
//! whole plate are balanced). It can carry it if a traction within the yield limit along the
//! section balances it: at most `F · s` per metre in tension and shear and `k · F · s` in
//! compression (`F` the depth-integrated strength of old lithosphere,
//! [`BreakupParams::strength`]; `s` the relative strength of the lithosphere,
//! [`super::weak_zones::lithosphere_strength`], as for the other splits; `k`
//! [`BreakupParams::compression`]). By limit analysis (rigid-plastic, upper-bound theorem),
//! the section's **overload** is
//!
//! `S = max_ω (ω · T_A) / D(ω)`, `D(ω) = Σ_e F · s_e · ℓ_e · (|v_t| + max(v_n, 0) + k · max(−v_n, 0))`
//!
//! over relative rotations `ω` of `A` (`v = ω × r_e` at the section's edges `e` of length `ℓ_e`,
//! `v_n` its opening and `v_t` its sliding): the work of the loads over the dissipation at the
//! yield traction for the most critical mechanism. `S > 1` means the lithosphere along the
//! section yields. The mechanism tells how: opening (rifting), sliding or closing (thrusting).
//! Narrow sections carry little: a neck of width `w` carries a force `F w` but a bending moment
//! of only `F w² / 4`, so the arms of U- and L-shaped plates, long strips and plates joined by
//! thin necks break where the loads on their parts differ (slab pull along one part of a trench,
//! drag of a continent, …), without any rule about the plate's shape.
//!
//! **Sections searched.** For each plate (its largest connected piece) the sections are the
//! level sets of the distance (in cell steps, within the plate) from three far-apart cells of the
//! plate (farthest-point sampling): they cross a strip or arm along its whole length and every
//! part of a compact plate. The most overloaded section is then refined along the weakest
//! lithosphere within [`BreakupParams::band`] cells of it (a minimum cut of the integrated
//! strength, as for the other splits, [`super::weak_zones`]); the more overloaded of the two
//! sections is kept. Both parts must cover at least [`BreakupParams::min_area`].
//!
//! **Breakup.** A plate whose most overloaded section stays above its strength for
//! [`BreakupParams::time`] (the yielding zone localises into a new boundary, a few Myr) splits
//! there: the smaller part becomes a new plate (the larger one keeps the plate's identity and
//! colour), and the new boundary is classified by the plates' motions like any other.
//!
//! Grid: sections are resolved to a cell (~110 km at 6 subdivisions); strips narrower than ~3
//! cells are near the resolution limit, and the minimum part size keeps fragments well above it.

use glam::{DMat3, DVec3};

use super::dynamics::SECONDS_PER_MYR;
use super::weak_zones::{Network, UNBREAKABLE, lithosphere_strength};
use super::*;

/// Parameters of the breakup of plates by their internal stress (`--set breakup.NAME=VALUE`).
#[derive(Clone, Debug)]
pub struct BreakupParams {
    /// Whether overloaded plates break (0: they never do; the sections are still analysed with
    /// [`Self::diagnose`]).
    pub enabled: bool,
    /// Analyse the sections every step (for the plate log) even when breakup is off. Changes
    /// nothing in the simulation.
    pub diagnose: bool,
    /// Depth-integrated strength (N/m) of old lithosphere (relative strength 1): the traction a
    /// section can carry per metre of its length.
    pub strength: f64,
    /// Time (Myr) for which a section must stay overloaded before the plate breaks.
    pub time: f64,
    /// Smallest part of a plate that breaks off, as a fraction of the surface.
    pub min_area: f64,
    /// Half-width (cells) of the band around the most overloaded section in which the break
    /// follows the weakest lithosphere.
    pub band: usize,
    /// Strength of the lithosphere in compression relative to tension and shear: brittle
    /// faults slip at ~3× the differential stress in thrusting as in normal faulting (Byerlee
    /// friction; Sibson 1974), so plates rift and shear apart more readily than they break by
    /// thrusting, which the sheet resolves as thickening instead.
    pub compression: f64,
    /// Time (Myr) from which plates break (for comparisons with a run without breakup up to
    /// then).
    pub from: f64,
}

impl Default for BreakupParams {
    fn default() -> Self {
        Self {
            enabled: false,
            diagnose: false,
            strength: 1e13,
            time: 5.0,
            min_area: 0.005,
            band: 3,
            compression: 3.0,
            from: 0.0,
        }
    }
}

impl BreakupParams {
    /// Sets the parameter `name` (without the `breakup.` prefix).
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        match name {
            "enabled" => self.enabled = value != 0.0,
            "diagnose" => self.diagnose = value != 0.0,
            "strength" => self.strength = value,
            "time" => self.time = value,
            "min_area" => self.min_area = value,
            "band" => self.band = value.max(0.0) as usize,
            "from" => self.from = value,
            "compression" => self.compression = value.max(0.0),
            _ => anyhow::bail!("unknown breakup parameter {name}"),
        }
        Ok(())
    }

    /// Whether the sections are analysed.
    pub(super) fn analyse(&self) -> bool {
        self.enabled || self.diagnose
    }

    /// The smallest part (cells) of a mesh of `cell_count` cells that breaks off.
    fn min_cells(&self, cell_count: usize) -> usize {
        ((self.min_area * cell_count as f64).ceil() as usize).max(1)
    }
}

/// The most overloaded section of a plate at the last step (see the module documentation).
#[derive(Clone, Debug, Default)]
pub struct SectionRecord {
    /// The plate's id ([`Simulation::plate_id`]).
    pub plate_id: u32,
    /// Cells of the plate's analysed piece.
    pub cells: usize,
    /// Overload `S` of the most overloaded level section and of the refined (weakest) one.
    pub level_overload: f64,
    pub cut_overload: f64,
    /// Overload of the section kept, its length (km) and the cells of the smaller part.
    pub overload: f64,
    pub length_km: f64,
    pub piece_cells: usize,
    /// Mean relative strength along the section.
    pub strength: f64,
    /// Share of the smaller part that is of continental origin.
    pub piece_continental: f64,
    /// How the section fails at its limit load: the net opening of the mechanism relative to
    /// all its motion, from −1 (closing: thrusting) through 0 (sliding) to 1 (opening: rifting).
    pub opening: f64,
    /// The net torque of the piece's loads that was removed before the analysis, relative to
    /// the torque the section carries.
    pub residual: f64,
    /// How long (Myr) the plate has been overloaded.
    pub timer: f64,
}

/// Totals of the breakup.
#[derive(Clone, Debug, Default)]
pub struct BreakupStatistics {
    /// Plates broken by their stress so far, the cells of their smaller parts, and how many
    /// broke mainly by opening and by closing (|opening| > 1/3; the others by shear, see
    /// [`SectionRecord::opening`]).
    pub splits: u64,
    pub split_cells: u64,
    pub opening_splits: u64,
    pub closing_splits: u64,
    /// At the last step: plates with an overloaded section, and the largest overload.
    pub overloaded: usize,
    pub max_overload: f64,
    /// Wall time (s) of the analysis so far.
    pub seconds: f64,
}

/// A candidate section: the cells of one side (indices into the piece) and its overload.
struct Section {
    side: Vec<bool>,
    overload: f64,
    /// Overloads of the most overloaded level section and of the refined one (0 if none).
    level: f64,
    cut: f64,
    length: f64,
    strength: f64,
    /// How the section fails (see [`SectionRecord::opening`]).
    opening: f64,
}

/// An edge of the mesh crossed by a section, from the side whose loads are summed to the other:
/// lever vectors of the sliding (`ω · p`) and of the opening (`ω · q`) of a relative rotation `ω`
/// of that side, and the section's relative strength × length there (unit sphere).
#[derive(Clone, Copy)]
struct SectionEdge {
    p: DVec3,
    q: DVec3,
    weight: f64,
    length: f64,
}

impl SectionEdge {
    /// The section across the mesh edge from `r` towards `other` (unit vectors) with the dual
    /// face length `face` and relative strength `strength`, as its two halves (so that a section
    /// of one edge also carries a moment about its middle).
    fn halves(r: DVec3, other: DVec3, face: f64, strength: f64) -> [Self; 2] {
        let mid = (r + other).normalize();
        let chord = other - r;
        let normal = (chord - mid * chord.dot(mid)).normalize();
        let along = mid.cross(normal);
        [-0.25, 0.25].map(|offset| {
            let point = (mid + along * (offset * face)).normalize();
            let tangent = point.cross(normal);
            Self {
                p: point.cross(tangent),
                q: normal.cross(point),
                weight: 0.5 * face * strength,
                length: 0.5 * face,
            }
        })
    }
}

/// Most iterations of the limit analysis of a section (see [`limit_overload`]), and of the
/// quick estimate in the sweep over the level sections, of which the [`REFINED`] most overloaded
/// per distance field are then analysed fully.
const ITERATIONS: usize = 30;
const QUICK_ITERATIONS: usize = 1;
const REFINED: usize = 4;

/// The overload of a section that must carry the torque `torque` (N per unit radius), with
/// strength `force` (N per unit length on the unit sphere) of lithosphere of relative strength
/// 1, `compression` times as strong in compression as in tension and shear: `max_ω (ω · T) /
/// D(ω)` over relative rotations `ω` of the loaded side, `D` the dissipation of the section at
/// its yield traction (the support function of the torques the section can carry; a
/// rigid-plastic limit analysis). Minimises `D` on the plane `ω · T = 1` by iteratively
/// reweighted least squares, starting from `warm` (the last section's mechanism, if it fits)
/// or `T / |T|²`; `warm` is updated. Returns (overload, opening), the opening as in
/// [`SectionRecord::opening`].
fn limit_overload(
    torque: DVec3,
    edges: &[SectionEdge],
    force: f64,
    compression: f64,
    warm: &mut DVec3,
    iterations: usize,
) -> (f64, f64) {
    let magnitude2 = torque.length_squared();
    if magnitude2 == 0.0 || edges.is_empty() || force <= 0.0 {
        return (0.0, 0.0);
    }
    let a = 0.5 * (1.0 + compression);
    let b = 0.5 * (1.0 - compression);
    let dissipation = |omega: DVec3| -> f64 {
        edges
            .iter()
            .map(|e| {
                let open = omega.dot(e.q);
                e.weight * (omega.dot(e.p).abs() + a * open.abs() + b * open)
            })
            .sum::<f64>()
            * force
    };
    let mut omega = torque / magnitude2;
    let mut best = dissipation(omega);
    let along = warm.dot(torque);
    if along > 0.0 {
        let candidate = *warm / along;
        let d = dissipation(candidate);
        if d < best {
            best = d;
            omega = candidate;
        }
    }
    let linear: DVec3 = edges.iter().map(|e| e.q * (e.weight * b)).sum::<DVec3>();
    for _ in 0..iterations {
        let scale = edges
            .iter()
            .map(|e| omega.dot(e.p).abs().max(omega.dot(e.q).abs()))
            .fold(0.0, f64::max);
        let floor = 1e-6 * scale.max(f64::MIN_POSITIVE);
        let mut q = DMat3::IDENTITY * 1e-12 * edges.iter().map(|e| e.weight).sum::<f64>();
        for e in edges {
            q += dynamics::outer(e.p, e.p) * (e.weight / omega.dot(e.p).abs().max(floor))
                + dynamics::outer(e.q, e.q) * (a * e.weight / omega.dot(e.q).abs().max(floor));
        }
        if q.determinant().abs() <= 0.0 {
            break;
        }
        let inverse = q.inverse();
        // Minimise ½ ωᵀQω + g·ω on ω·T = 1.
        let qt = inverse * torque;
        let qg = inverse * linear;
        let denominator = torque.dot(qt);
        if denominator <= 0.0 {
            break;
        }
        let lambda = (1.0 + torque.dot(qg)) / denominator;
        let next = qt * lambda - qg;
        let d = dissipation(next);
        if !(d < best) {
            break;
        }
        let converged = d > best * (1.0 - 1e-4);
        best = d;
        omega = next;
        if converged {
            break;
        }
    }
    *warm = omega;
    let (mut opening, mut total) = (0.0, 0.0);
    for e in edges {
        let open = omega.dot(e.q);
        opening += e.weight * open;
        total += e.weight * (open.abs() + omega.dot(e.p).abs());
    }
    let opening = if total > 0.0 { opening / total } else { 0.0 };
    (1.0 / best, opening)
}

impl Simulation {
    /// Analyses the sections of every plate after the deforming-plate solve (see the module
    /// documentation) and marks plates that have been overloaded for long enough. `forces` are
    /// the forces on the lithosphere of every cell (N, world frame) at the plates' rotations
    /// `rotations` before the solve; the plates' rotations and deformation velocities are the
    /// solved ones.
    pub(super) fn analyse_sections(&mut self, forces: &[DVec3], rotations: &[DVec3], dt: f64) {
        let start = web_time::Instant::now();
        let params = self.params.breakup.clone();
        let radius = self.params.planet_radius;
        let cell_count = self.mesh.len();
        let cell_area = 4.0 * std::f64::consts::PI * radius * radius / cell_count as f64;
        let to_metres_per_second = radius / SECONDS_PER_MYR;
        let min_cells = params.min_cells(cell_count);

        // Per cell: external load (N) at the material velocity, drag coefficient, strength.
        let mut load = vec![DVec3::ZERO; cell_count];
        let mut drag = vec![0.0; cell_count];
        let mut strength = vec![0.0; cell_count];
        let mut continental = vec![false; cell_count];
        let mut plate_cells: Vec<Vec<u32>> = vec![Vec::new(); self.plates.len()];
        for cell in 0..cell_count as u32 {
            let owner = self.owners[cell as usize];
            let plate = &self.plates[owner.plate as usize];
            let r = self.mesh.vertex(cell);
            let crust = self.surface_crust(cell);
            let c = cell as usize;
            drag[c] = self.params.basal_drag
                * if crust.is_continental() {
                    self.params.continental_drag_factor
                } else {
                    1.0
                }
                * cell_area;
            let u = plate.rotation * plate.sheet.velocity[owner.local as usize];
            let v = ((plate.angular_velocity - rotations[owner.plate as usize]).cross(r) + u)
                * to_metres_per_second;
            let f = forces[c] - v * drag[c];
            load[c] = f - r * f.dot(r);
            strength[c] = lithosphere_strength(&crust, self.time);
            continental[c] = crust.is_continental_origin();
            plate_cells[owner.plate as usize].push(cell);
        }

        let now = self.time;
        let mut records = Vec::new();
        let mut statistics = std::mem::take(&mut self.breakup_statistics);
        statistics.overloaded = 0;
        statistics.max_overload = 0.0;
        // Position of a cell in the analysed piece (u32::MAX: not in it), and scratch.
        let mut index = vec![u32::MAX; cell_count];
        let mut visited = vec![false; cell_count];
        for (p, cells) in plate_cells.iter().enumerate() {
            let piece = self.largest_piece(cells, p as u32, &mut visited);
            for &cell in cells {
                visited[cell as usize] = false;
            }
            let analysed = piece.len() >= 2 * min_cells;
            let mut best: Option<(Section, f64)> = None;
            if analysed {
                for (i, &cell) in piece.iter().enumerate() {
                    index[cell as usize] = i as u32;
                }
                best = self.most_overloaded_section(
                    &piece, &index, &load, &drag, &strength, min_cells, &params,
                );
                for &cell in &piece {
                    index[cell as usize] = u32::MAX;
                }
            }
            let plate = &mut self.plates[p];
            plate.sheet.pending_breakup = None;
            let Some((section, residual)) = best else {
                plate.sheet.breakup_timer = 0.0;
                continue;
            };
            let side_cells = section.side.iter().filter(|&&a| a).count();
            let small_side = side_cells * 2 <= piece.len();
            let piece_cells = if small_side {
                side_cells
            } else {
                piece.len() - side_cells
            };
            // The yielding zone weakens while it is overloaded and heals as fast when not.
            let overloaded = section.overload > 1.0;
            if overloaded {
                statistics.overloaded += 1;
                plate.sheet.breakup_timer += dt;
            } else {
                plate.sheet.breakup_timer = (plate.sheet.breakup_timer - dt).max(0.0);
            }
            plate.sheet.breakup_opening = section.opening;
            statistics.max_overload = statistics.max_overload.max(section.overload);
            if params.enabled
                && now >= params.from
                && overloaded
                && plate.sheet.breakup_timer >= params.time - 1e-9
            {
                let locals: Vec<u32> = piece
                    .iter()
                    .zip(&section.side)
                    .filter(|&(_, &a)| a == small_side)
                    .map(|(&cell, _)| self.owners[cell as usize].local)
                    .collect();
                plate.sheet.pending_breakup = Some(locals);
            }
            if params.diagnose {
                let continental_cells = piece
                    .iter()
                    .zip(&section.side)
                    .filter(|&(&cell, &a)| a == small_side && continental[cell as usize])
                    .count();
                records.push(SectionRecord {
                    plate_id: plate.id,
                    cells: piece.len(),
                    level_overload: section.level,
                    cut_overload: section.cut,
                    overload: section.overload,
                    length_km: section.length * radius / 1e3,
                    piece_cells,
                    strength: section.strength,
                    piece_continental: continental_cells as f64 / piece_cells.max(1) as f64,
                    opening: section.opening,
                    residual,
                    timer: plate.sheet.breakup_timer,
                });
            }
        }
        statistics.seconds += start.elapsed().as_secs_f64();
        self.breakup_statistics = statistics;
        self.section_records = records;
    }

    /// The largest connected piece of the cells `cells` of `plate` (ties: the first one);
    /// marks the cells it visits in `visited`.
    fn largest_piece(&self, cells: &[u32], plate: u32, visited: &mut [bool]) -> Vec<u32> {
        let mut best: Vec<u32> = Vec::new();
        let mut seen = 0usize;
        for &start in cells {
            if visited[start as usize] {
                continue;
            }
            visited[start as usize] = true;
            let mut piece = vec![start];
            let mut next = 0;
            while next < piece.len() {
                let cell = piece[next];
                next += 1;
                for &n in self.mesh.neighbors(cell) {
                    if !visited[n as usize] && self.owners[n as usize].plate == plate {
                        visited[n as usize] = true;
                        piece.push(n);
                    }
                }
            }
            seen += piece.len();
            if piece.len() > best.len() {
                best = piece;
            }
            if 2 * best.len() >= cells.len() || seen == cells.len() {
                break;
            }
        }
        best
    }

    /// Distances (cell steps) within the piece from `source` (indices into the piece).
    fn piece_distances(&self, piece: &[u32], index: &[u32], source: usize) -> Vec<u32> {
        let mut distance = vec![u32::MAX; piece.len()];
        distance[source] = 0;
        let mut queue = vec![source as u32];
        let mut next = 0;
        while next < queue.len() {
            let i = queue[next] as usize;
            next += 1;
            for &n in self.mesh.neighbors(piece[i]) {
                let j = index[n as usize];
                if j != u32::MAX && distance[j as usize] == u32::MAX {
                    distance[j as usize] = distance[i] + 1;
                    queue.push(j);
                }
            }
        }
        distance
    }

    /// The strength (N per unit length on the unit sphere) of lithosphere of relative strength
    /// 1.
    fn section_force(&self) -> f64 {
        self.params.breakup.strength * self.params.planet_radius
    }

    /// The overload of the section between the cells of the piece on `side` and the others,
    /// with the loads `loads` (per cell of the piece): (overload, opening, length on the unit
    /// sphere, mean relative strength).
    fn section_overload(
        &self,
        piece: &[u32],
        index: &[u32],
        side: &[bool],
        loads: &[DVec3],
        strength: &[f64],
    ) -> (f64, f64, f64, f64) {
        let geometry = &self.sheet_geometry;
        let mut torque = DVec3::ZERO;
        let mut edges: Vec<SectionEdge> = Vec::new();
        for (i, &cell) in piece.iter().enumerate() {
            if !side[i] {
                continue;
            }
            let r = self.mesh.vertex(cell);
            torque += r.cross(loads[i]);
            for slot in geometry.edges(cell) {
                let n = geometry.neighbor[slot];
                let j = index[n as usize];
                if j != u32::MAX && !side[j as usize] {
                    let s = 0.5 * (strength[cell as usize] + strength[n as usize]);
                    edges.extend(SectionEdge::halves(
                        r,
                        self.mesh.vertex(n),
                        geometry.face[slot],
                        s,
                    ));
                }
            }
        }
        let mut warm = DVec3::ZERO;
        let (overload, opening) = limit_overload(
            torque,
            &edges,
            self.section_force(),
            self.params.breakup.compression,
            &mut warm,
            ITERATIONS,
        );
        let length: f64 = edges.iter().map(|e| e.length).sum();
        let mean_strength = if length > 0.0 {
            edges.iter().map(|e| e.weight).sum::<f64>() / length
        } else {
            0.0
        };
        (overload, opening, length, mean_strength)
    }

    /// The most overloaded section of a piece of a plate (see the module documentation) and the
    /// net torque of the loads that was removed, relative to the section's torque; `None` if the
    /// piece has no section with both parts of at least `min_cells`.
    #[allow(clippy::too_many_arguments)]
    fn most_overloaded_section(
        &self,
        piece: &[u32],
        index: &[u32],
        load: &[DVec3],
        drag: &[f64],
        strength: &[f64],
        min_cells: usize,
        params: &BreakupParams,
    ) -> Option<(Section, f64)> {
        let n = piece.len();
        // The loads of the piece with their net torque removed (a drag-weighted rotation): the
        // piece is in equilibrium as a whole, up to the parts of the plate not analysed, the
        // couplings across its boundaries and the solve's residual.
        let (mut matrix, mut net) = (DMat3::ZERO, DVec3::ZERO);
        for &cell in piece {
            let r = self.mesh.vertex(cell);
            matrix += (DMat3::IDENTITY - dynamics::outer(r, r)) * drag[cell as usize];
            net += r.cross(load[cell as usize]);
        }
        let mut loads: Vec<DVec3> = piece.iter().map(|&c| load[c as usize]).collect();
        if matrix.determinant().abs() > 0.0 {
            let rotation = matrix.inverse() * net;
            for (i, &cell) in piece.iter().enumerate() {
                loads[i] -= rotation.cross(self.mesh.vertex(cell)) * drag[cell as usize];
            }
        }

        // Three far-apart sources.
        let first = self.piece_distances(piece, index, 0);
        let farthest = |distance: &[u32]| {
            (0..n)
                .max_by(|&a, &b| distance[a].cmp(&distance[b]).then(b.cmp(&a)))
                .expect("pieces are not empty")
        };
        let d1 = self.piece_distances(piece, index, farthest(&first));
        let d2 = self.piece_distances(piece, index, farthest(&d1));
        let both: Vec<u32> = d1.iter().zip(&d2).map(|(&a, &b)| a.min(b)).collect();
        let d3 = self.piece_distances(piece, index, farthest(&both));

        let geometry = &self.sheet_geometry;
        let force = self.section_force();
        // Best level section: (overload, field, level, its torque).
        let mut best: Option<(f64, usize, u32, DVec3)> = None;
        let fields = [d1, d2, d3];
        let empty = SectionEdge {
            p: DVec3::ZERO,
            q: DVec3::ZERO,
            weight: 0.0,
            length: 0.0,
        };
        for (f, distance) in fields.iter().enumerate() {
            let levels = distance.iter().copied().max().unwrap_or(0) as usize + 1;
            let mut count = vec![0usize; levels];
            let mut level_torque = vec![DVec3::ZERO; levels];
            // Edges crossing into each level from the one below.
            let mut edge_count = vec![0usize; levels + 1];
            for (i, &cell) in piece.iter().enumerate() {
                let d = distance[i] as usize;
                count[d] += 1;
                level_torque[d] += self.mesh.vertex(cell).cross(loads[i]);
                for &m in self.mesh.neighbors(cell) {
                    let j = index[m as usize];
                    if j != u32::MAX && distance[j as usize] as usize == d + 1 {
                        edge_count[d + 2] += 2;
                    }
                }
            }
            for l in 1..=levels {
                edge_count[l] += edge_count[l - 1];
            }
            let mut edges = vec![empty; edge_count[levels]];
            let mut fill = edge_count.clone();
            for (i, &cell) in piece.iter().enumerate() {
                let d = distance[i] as usize;
                let r = self.mesh.vertex(cell);
                for slot in geometry.edges(cell) {
                    let m = geometry.neighbor[slot];
                    let j = index[m as usize];
                    if j != u32::MAX && distance[j as usize] as usize == d + 1 {
                        let s = 0.5 * (strength[cell as usize] + strength[m as usize]);
                        let halves =
                            SectionEdge::halves(r, self.mesh.vertex(m), geometry.face[slot], s);
                        edges[fill[d + 1]..fill[d + 1] + 2].copy_from_slice(&halves);
                        fill[d + 1] += 2;
                    }
                }
            }
            // Section between the levels below `level` and the others.
            // Quick estimates (lower bounds) for all levels, then the most overloaded ones in
            // full.
            let (mut below, mut torque) = (0usize, DVec3::ZERO);
            let mut warm = DVec3::ZERO;
            let mut candidates: Vec<(f64, usize, DVec3, DVec3)> = Vec::new();
            for level in 1..levels {
                below += count[level - 1];
                torque += level_torque[level - 1];
                if below < min_cells || n - below < min_cells {
                    continue;
                }
                let crossing = &edges[edge_count[level]..edge_count[level + 1]];
                let (overload, _) = limit_overload(
                    torque,
                    crossing,
                    force,
                    params.compression,
                    &mut warm,
                    QUICK_ITERATIONS,
                );
                candidates.push((overload, level, torque, warm));
            }
            candidates.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            for &(_, level, torque, mut warm) in candidates.iter().take(REFINED) {
                let crossing = &edges[edge_count[level]..edge_count[level + 1]];
                let (overload, _) = limit_overload(
                    torque,
                    crossing,
                    force,
                    params.compression,
                    &mut warm,
                    ITERATIONS,
                );
                if best.is_none_or(|(o, _, _, _)| overload > o) {
                    best = Some((overload, f, level as u32, torque));
                }
            }
        }
        let (level_overload, field, level, level_torque) = best?;
        let residual = net.length() / level_torque.length().max(f64::MIN_POSITIVE);
        let distance = &fields[field];
        let level_side: Vec<bool> = distance.iter().map(|&d| d < level).collect();
        let (_, level_opening, level_length, level_strength) =
            self.section_overload(piece, index, &level_side, &loads, strength);
        let mut section = Section {
            side: level_side,
            overload: level_overload,
            level: level_overload,
            cut: 0.0,
            length: level_length,
            strength: level_strength,
            opening: level_opening,
        };
        // Refined along the weakest lithosphere, where it matters.
        if level_overload > 0.5 {
            let max_level = distance.iter().copied().max().unwrap_or(0);
            let low = level.saturating_sub(1 + params.band as u32);
            let high = (level + params.band as u32).min(max_level);
            // The network spans the band only: cells at its edges are tied to the source and
            // sink, and every link that can be cut lies between them.
            let band: Vec<usize> = (0..n)
                .filter(|&i| (low..=high).contains(&distance[i]))
                .collect();
            let mut node = vec![u32::MAX; n];
            for (k, &i) in band.iter().enumerate() {
                node[i] = k as u32;
            }
            let (source, sink) = (band.len(), band.len() + 1);
            let mut network = Network::new(band.len() + 2);
            for (k, &i) in band.iter().enumerate() {
                let cell = piece[i];
                if distance[i] == low {
                    network.link(source, k, UNBREAKABLE);
                } else if distance[i] == high {
                    network.link(k, sink, UNBREAKABLE);
                }
                for slot in geometry.edges(cell) {
                    let m = geometry.neighbor[slot];
                    let j = index[m as usize];
                    if j == u32::MAX || m < cell || node[j as usize] == u32::MAX {
                        continue;
                    }
                    let s = 0.5 * (strength[cell as usize] + strength[m as usize]);
                    network.link(k, node[j as usize] as usize, geometry.face[slot] * s);
                }
            }
            network.max_flow(source, sink);
            let in_band = network.reachable(source);
            let reachable =
                |i: usize| distance[i] < low || (node[i] != u32::MAX && in_band[node[i] as usize]);
            // The part connected to the source's cells.
            let mut side = vec![false; n];
            let mut stack: Vec<usize> = (0..n).filter(|&i| distance[i] <= low).collect();
            for &i in &stack {
                side[i] = true;
            }
            while let Some(i) = stack.pop() {
                for &m in self.mesh.neighbors(piece[i]) {
                    let j = index[m as usize];
                    if j != u32::MAX && reachable(j as usize) && !side[j as usize] {
                        side[j as usize] = true;
                        stack.push(j as usize);
                    }
                }
            }
            let cells = side.iter().filter(|&&a| a).count();
            if cells >= min_cells && n - cells >= min_cells {
                let (overload, opening, length, mean_strength) =
                    self.section_overload(piece, index, &side, &loads, strength);
                section.cut = overload;
                if overload > section.overload {
                    section = Section {
                        side,
                        overload,
                        length,
                        strength: mean_strength,
                        opening,
                        ..section
                    };
                }
            }
        }
        Some((section, residual))
    }

    /// Splits the plates that have been overloaded for long enough (see
    /// [`PlateSheet::pending_breakup`]) along their most overloaded section, if both parts are
    /// still large enough.
    pub(super) fn break_overloaded_plates(&mut self) {
        let min_cells = self.params.breakup.min_cells(self.mesh.len());
        for plate in 0..self.plates.len() as u32 {
            let Some(side) = self.plates[plate as usize].sheet.pending_breakup.take() else {
                continue;
            };
            if self.plates.len() >= self.params.events.max_plates {
                continue;
            }
            let mut marked = vec![false; self.mesh.len()];
            for &local in &side {
                marked[local as usize] = true;
            }
            let piece: Vec<u32> = (0..self.mesh.len() as u32)
                .filter(|&cell| {
                    let owner = self.owners[cell as usize];
                    owner.plate == plate && marked[owner.local as usize]
                })
                .collect();
            let total = self
                .owners
                .iter()
                .filter(|owner| owner.plate == plate)
                .count();
            if piece.len() < min_cells || total - piece.len() < min_cells {
                continue;
            }
            let opening = self.plates[plate as usize].sheet.breakup_opening;
            if let Some(new_plate) = self.split_plate_cells(plate, &piece) {
                self.plates[plate as usize].sheet.breakup_timer = 0.0;
                self.plates[new_plate as usize].sheet.breakup_timer = 0.0;
                let statistics = &mut self.breakup_statistics;
                statistics.splits += 1;
                statistics.split_cells += piece.len() as u64;
                if opening > 1.0 / 3.0 {
                    statistics.opening_splits += 1;
                } else if opening < -1.0 / 3.0 {
                    statistics.closing_splits += 1;
                }
                tracing::debug!(
                    plate,
                    new_plate,
                    cells = piece.len(),
                    opening,
                    time = self.time,
                    "Overloaded plate broke"
                );
            }
        }
    }

    /// The most overloaded section of every plate at the last step (only with
    /// [`BreakupParams::diagnose`]).
    pub fn section_records(&self) -> &[SectionRecord] {
        &self.section_records
    }

    /// Totals of the breakup by stress.
    pub fn breakup_statistics(&self) -> &BreakupStatistics {
        &self.breakup_statistics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The most overloaded section of the cells `cells` of an icosphere with the loads `load`
    /// (N per cell) at uniform strength 1: (overload, the cells on one side of it).
    fn section_of(
        simulation: &Simulation,
        cells: &[u32],
        load: impl Fn(DVec3) -> DVec3,
    ) -> (f64, Vec<u32>) {
        let n = simulation.mesh.len();
        let mut index = vec![u32::MAX; n];
        for (i, &cell) in cells.iter().enumerate() {
            index[cell as usize] = i as u32;
        }
        let loads: Vec<DVec3> = (0..n as u32)
            .map(|cell| {
                let r = simulation.mesh.vertex(cell);
                let f = load(r);
                f - r * f.dot(r)
            })
            .collect();
        let drag = vec![1.0; n];
        let strength = vec![1.0; n];
        let params = simulation.params.breakup.clone();
        let section = simulation
            .most_overloaded_section(cells, &index, &loads, &drag, &strength, 5, &params)
            .expect("a section")
            .0;
        let side = cells
            .iter()
            .zip(&section.side)
            .filter(|&(_, &a)| a)
            .map(|(&c, _)| c)
            .collect();
        (section.overload, side)
    }

    /// A straight section along the meridian through (1, 0, 0), 0.02 rad long in 200 edges,
    /// between the loaded side (west) and the other: opening along +y.
    fn straight_section() -> Vec<SectionEdge> {
        (0..200)
            .flat_map(|i| {
                let z = 0.02 * ((i as f64 + 0.5) / 200.0 - 0.5);
                let r = DVec3::new(1.0, 0.0, z).normalize();
                let east = DVec3::new(0.0, 1e-3, 0.0);
                let a = (r - east).normalize();
                let b = (r + east).normalize();
                SectionEdge::halves(a, b, 0.02 / 200.0, 1.0)
            })
            .collect()
    }

    #[test]
    fn limit_loads_of_a_straight_section_match_plastic_beam_theory() {
        let edges = straight_section();
        let (force, length) = (1.0, 0.02);
        let mut warm = DVec3::ZERO;
        // A pull west on the loaded side (tension across the section) at (1, 0, 0): torque
        // r × f.
        let pull = |n: f64| DVec3::X.cross(DVec3::new(0.0, -n, 0.0));
        let (s, opening) = limit_overload(pull(0.02), &edges, force, 3.0, &mut warm, ITERATIONS);
        assert!((s - 1.0).abs() < 0.02, "tension: {s}");
        assert!(opening > 0.9, "{opening}");
        // Pushed east: compression, three times as strong.
        let (s, opening) = limit_overload(pull(-0.06), &edges, force, 3.0, &mut warm, ITERATIONS);
        assert!((s - 1.0).abs() < 0.02, "compression: {s}");
        assert!(opening < -0.9, "{opening}");
        // Sheared north–south.
        let (s, _) = limit_overload(
            DVec3::X.cross(DVec3::Z * 0.02),
            &edges,
            force,
            3.0,
            &mut warm,
            ITERATIONS,
        );
        assert!((s - 1.0).abs() < 0.02, "shear: {s}");
        // A bending moment about the radial axis: the plastic moment of a section of length L
        // in tension on one half and compression (k = 3) on the other is F L² · k / (2 (1 + k)).
        let plastic = force * length * length * 3.0 / (2.0 * 4.0);
        let (s, _) = limit_overload(
            DVec3::X * plastic,
            &edges,
            force,
            3.0,
            &mut warm,
            ITERATIONS,
        );
        assert!((s - 1.0).abs() < 0.03, "bending: {s}");
        // With equal strengths: F L² / 4.
        let (s, _) = limit_overload(
            DVec3::X * (force * length * length / 4.0),
            &edges,
            force,
            1.0,
            &mut warm,
            ITERATIONS,
        );
        assert!((s - 1.0).abs() < 0.03, "symmetric bending: {s}");
    }

    #[test]
    fn narrow_arms_break_where_compact_plates_do_not() {
        let simulation = Simulation::new(SphereMesh::icosphere(4), 4, TectonicParams::default(), 1);
        let mesh = &simulation.mesh;
        // A strip along the equator (~4 cells wide, 90° long) and a cap of the same area.
        let strip: Vec<u32> = (0..mesh.len() as u32)
            .filter(|&c| {
                let r = mesh.vertex(c);
                r.z.abs() < 0.12 && r.x > 0.0 && r.y > 0.0
            })
            .collect();
        let pole = DVec3::new(1.0, 1.0, 0.0).normalize();
        let mut cap: Vec<u32> = (0..mesh.len() as u32).collect();
        cap.sort_by(|&a, &b| {
            mesh.vertex(b)
                .dot(pole)
                .total_cmp(&mesh.vertex(a).dot(pole))
        });
        cap.truncate(strip.len());
        // Its ends are pushed north, its middle south (three-point bending).
        let load = |r: DVec3| {
            let sign = if r.dot(pole) < math::cos(0.25 * std::f64::consts::FRAC_PI_2) {
                1.0
            } else {
                -1.0
            };
            DVec3::Z * (sign * 1e18)
        };
        let (strip_overload, side) = section_of(&simulation, &strip, load);
        let (cap_overload, _) = section_of(&simulation, &cap, load);
        assert!(
            strip_overload > 5.0 * cap_overload,
            "strip {strip_overload}, cap {cap_overload}"
        );
        // The strip breaks near its middle, where the bending moment is largest.
        let fraction = side.len() as f64 / strip.len() as f64;
        assert!((0.3..0.7).contains(&fraction), "{fraction}");
    }

    /// Owners and rotations, bit for bit.
    fn fingerprint(simulation: &Simulation) -> Vec<u64> {
        let mut bits: Vec<u64> = simulation
            .owners
            .iter()
            .flat_map(|o| [o.plate as u64, o.local as u64])
            .collect();
        for plate in &simulation.plates {
            bits.extend(plate.angular_velocity.to_array().map(f64::to_bits));
        }
        bits
    }

    #[test]
    fn the_analysis_alone_changes_nothing() {
        let run = |params: TectonicParams| {
            let mut simulation = Simulation::new(SphereMesh::icosphere(4), 8, params, 5);
            for _ in 0..10 {
                simulation.step(1.0);
            }
            (fingerprint(&simulation), simulation.section_records.len())
        };
        let mut diagnosed = TectonicParams::default();
        diagnosed.breakup.diagnose = true;
        let (plain, none) = run(TectonicParams::default());
        let (analysed, records) = run(diagnosed);
        assert!(plain == analysed, "the analysis changed the simulation");
        assert_eq!(none, 0);
        assert!(records > 0);
    }

    #[test]
    fn weak_plates_break_into_parts_above_the_minimum() {
        let mut params = TectonicParams::default();
        params.breakup.enabled = true;
        params.breakup.strength = 1e10;
        params.breakup.time = 1.0;
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 6, params, 2);
        let mass = simulation.total_mass();
        let ids: Vec<u32> = simulation.plates.iter().map(|p| p.id).collect();
        for _ in 0..6 {
            simulation.step(1.0);
        }
        assert!(simulation.breakup_statistics.splits > 0);
        // The larger part keeps its plate's id: the original ids are all still there unless
        // their plate was merged away.
        let alive = simulation
            .plates
            .iter()
            .filter(|p| ids.contains(&p.id))
            .count();
        assert!(alive + simulation.event_counts.deaths() >= ids.len());
        // Splits move crust between plates without changing its mass.
        assert!(
            simulation.ledger.splits.abs() < 1e-9 * mass,
            "{}",
            simulation.ledger.splits
        );
    }
}
