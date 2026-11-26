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
//! whole plate are balanced). The section can carry at most
//!
//! `C(â) = Σ_e F · s_e · ℓ_e · |â × r_e|` about the axis `â = T_A / |T_A|`
//!
//! (traction of at most `F · s` per metre of section, `F` the depth-integrated strength of old
//! lithosphere ([`BreakupParams::strength`]), `s` the relative strength of the lithosphere,
//! [`super::weak_zones::lithosphere_strength`], `ℓ_e` the length of the section across the mesh
//! edge `e` at `r_e`): a rigid-plastic limit (lower bound on the load needed to break it).
//! The ratio `S = |T_A| / C(â)` is the section's overload; `S > 1` means the lithosphere along
//! the section yields. Narrow sections carry little: a neck of width `w` resists a bending
//! moment only with `~F w² / 4`, so the arms of U- and L-shaped plates, long strips and plates
//! joined by thin necks break where the loads on their parts differ (slab pull along one part of
//! a trench, drag of a continent, …), without any rule about the plate's shape.
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
            _ => anyhow::bail!("unknown breakup parameter {name}"),
        }
        Ok(())
    }

    /// Whether the sections are analysed.
    pub(super) fn analyse(&self) -> bool {
        self.enabled || self.diagnose
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
    /// How long (Myr) the plate has been overloaded.
    pub timer: f64,
}

/// Totals of the breakup.
#[derive(Clone, Debug, Default)]
pub struct BreakupStatistics {
    /// Plates broken by their stress so far.
    pub splits: u64,
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
        let min_cells = ((params.min_area * cell_count as f64).ceil() as usize).max(1);

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

        let mut records = Vec::new();
        let mut statistics = std::mem::take(&mut self.breakup_statistics);
        statistics.overloaded = 0;
        statistics.max_overload = 0.0;
        // Position of a cell in the analysed piece (u32::MAX: not in it).
        let mut index = vec![u32::MAX; cell_count];
        for (p, cells) in plate_cells.iter().enumerate() {
            let piece = self.largest_piece(cells, p as u32);
            let analysed = piece.len() >= 2 * min_cells;
            let mut best: Option<Section> = None;
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
            let Some(section) = best else {
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
            let overloaded = section.overload > 1.0;
            if overloaded {
                statistics.overloaded += 1;
                plate.sheet.breakup_timer += dt;
            } else {
                plate.sheet.breakup_timer = 0.0;
            }
            statistics.max_overload = statistics.max_overload.max(section.overload);
            if params.enabled && overloaded && plate.sheet.breakup_timer >= params.time - 1e-9 {
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
                    timer: plate.sheet.breakup_timer,
                });
            }
        }
        statistics.seconds += start.elapsed().as_secs_f64();
        self.breakup_statistics = statistics;
        self.section_records = records;
    }

    /// The largest connected piece of the cells of `plate` (ties: the first one).
    fn largest_piece(&self, cells: &[u32], plate: u32) -> Vec<u32> {
        let mut visited = vec![false; self.mesh.len()];
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

    /// The overload of the section between the cells of the piece on `side` and the others:
    /// (overload, length on the unit sphere, mean relative strength).
    fn section_overload(
        &self,
        piece: &[u32],
        index: &[u32],
        side: &[bool],
        load: &[DVec3],
        strength: &[f64],
    ) -> (f64, f64, f64) {
        let geometry = &self.sheet_geometry;
        let mut torque = DVec3::ZERO;
        let mut edges: Vec<(DVec3, f64, f64)> = Vec::new();
        for (i, &cell) in piece.iter().enumerate() {
            if !side[i] {
                continue;
            }
            let r = self.mesh.vertex(cell);
            torque += r.cross(load[cell as usize]);
            for slot in geometry.edges(cell) {
                let n = geometry.neighbor[slot];
                let j = index[n as usize];
                if j != u32::MAX && !side[j as usize] {
                    let s = 0.5 * (strength[cell as usize] + strength[n as usize]);
                    let mid = (r + self.mesh.vertex(n)).normalize();
                    edges.push((mid, geometry.face[slot], s));
                }
            }
        }
        self.overload_of(torque, &edges)
    }

    /// The overload of a section with the edges (midpoint, length, relative strength) that
    /// carries `torque` (N per unit radius).
    fn overload_of(&self, torque: DVec3, edges: &[(DVec3, f64, f64)]) -> (f64, f64, f64) {
        let magnitude = torque.length();
        let length: f64 = edges.iter().map(|e| e.1).sum();
        let mean_strength = if length > 0.0 {
            edges.iter().map(|e| e.1 * e.2).sum::<f64>() / length
        } else {
            0.0
        };
        if magnitude == 0.0 || edges.is_empty() {
            return (0.0, length, mean_strength);
        }
        let axis = torque / magnitude;
        let force = self.params.breakup.strength * self.params.planet_radius;
        let capacity: f64 = edges
            .iter()
            .map(|&(mid, face, s)| force * s * face * axis.cross(mid).length())
            .sum();
        let overload = if capacity > 0.0 {
            magnitude / capacity
        } else {
            f64::INFINITY
        };
        (overload, length, mean_strength)
    }

    /// The most overloaded section of a piece of a plate (see the module documentation), or
    /// `None` if the piece has no section with both parts of at least `min_cells`.
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
    ) -> Option<Section> {
        let n = piece.len();
        // The loads of the piece with their net torque removed (a drag-weighted rotation): the
        // piece is in equilibrium as a whole, up to the parts of the plate not analysed and the
        // solve's residual.
        let (mut matrix, mut torque) = (DMat3::ZERO, DVec3::ZERO);
        for &cell in piece {
            let r = self.mesh.vertex(cell);
            matrix += (DMat3::IDENTITY - dynamics::outer(r, r)) * drag[cell as usize];
            torque += r.cross(load[cell as usize]);
        }
        let mut loads: Vec<DVec3> = piece.iter().map(|&c| load[c as usize]).collect();
        if matrix.determinant().abs() > 0.0 {
            let rotation = matrix.inverse() * torque;
            for (i, &cell) in piece.iter().enumerate() {
                loads[i] -= rotation.cross(self.mesh.vertex(cell)) * drag[cell as usize];
            }
        }
        let mut balanced = vec![DVec3::ZERO; self.mesh.len()];
        for (i, &cell) in piece.iter().enumerate() {
            balanced[cell as usize] = loads[i];
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
        // Best level section: (overload, field, level).
        let mut best: Option<(f64, usize, u32)> = None;
        let fields = [d1, d2, d3];
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
                        edge_count[d + 2] += 1;
                    }
                }
            }
            for l in 1..=levels {
                edge_count[l] += edge_count[l - 1];
            }
            let mut edges = vec![(DVec3::ZERO, 0.0, 0.0); edge_count[levels]];
            let mut fill = edge_count.clone();
            for (i, &cell) in piece.iter().enumerate() {
                let d = distance[i] as usize;
                let r = self.mesh.vertex(cell);
                for slot in geometry.edges(cell) {
                    let m = geometry.neighbor[slot];
                    let j = index[m as usize];
                    if j != u32::MAX && distance[j as usize] as usize == d + 1 {
                        let s = 0.5 * (strength[cell as usize] + strength[m as usize]);
                        let mid = (r + self.mesh.vertex(m)).normalize();
                        edges[fill[d + 1]] = (mid, geometry.face[slot], s);
                        fill[d + 1] += 1;
                    }
                }
            }
            // Section between the levels below `level` and the others.
            let (mut below, mut torque) = (0usize, DVec3::ZERO);
            for level in 1..levels {
                below += count[level - 1];
                torque += level_torque[level - 1];
                if below < min_cells || n - below < min_cells {
                    continue;
                }
                let crossing = &edges[edge_count[level]..edge_count[level + 1]];
                let (overload, _, _) = self.overload_of(torque, crossing);
                if best.is_none_or(|(o, _, _)| overload > o) {
                    best = Some((overload, f, level as u32));
                }
            }
        }
        let (level_overload, field, level) = best?;
        let distance = &fields[field];
        let level_side: Vec<bool> = distance.iter().map(|&d| d < level).collect();
        let (_, level_length, level_strength) =
            self.section_overload(piece, index, &level_side, &balanced, strength);
        let mut section = Section {
            side: level_side,
            overload: level_overload,
            level: level_overload,
            cut: 0.0,
            length: level_length,
            strength: level_strength,
        };
        // Refined along the weakest lithosphere, where it matters.
        if level_overload > 0.5 {
            let max_level = distance.iter().copied().max().unwrap_or(0);
            let low = level.saturating_sub(1 + params.band as u32);
            let high = (level + params.band as u32).min(max_level);
            let (source, sink) = (n, n + 1);
            let mut network = Network::new(n + 2);
            for (i, &cell) in piece.iter().enumerate() {
                if distance[i] <= low {
                    network.link(source, i, UNBREAKABLE);
                } else if distance[i] >= high {
                    network.link(i, sink, UNBREAKABLE);
                }
                for slot in geometry.edges(cell) {
                    let m = geometry.neighbor[slot];
                    let j = index[m as usize];
                    if j == u32::MAX || m < cell {
                        continue;
                    }
                    let s = 0.5 * (strength[cell as usize] + strength[m as usize]);
                    network.link(i, j as usize, geometry.face[slot] * s);
                }
            }
            network.max_flow(source, sink);
            let reachable = network.reachable(source);
            // The part connected to the source's cells.
            let mut side = vec![false; n];
            let mut stack: Vec<usize> = (0..n).filter(|&i| distance[i] <= low).collect();
            for &i in &stack {
                side[i] = true;
            }
            while let Some(i) = stack.pop() {
                for &m in self.mesh.neighbors(piece[i]) {
                    let j = index[m as usize];
                    if j != u32::MAX && reachable[j as usize] && !side[j as usize] {
                        side[j as usize] = true;
                        stack.push(j as usize);
                    }
                }
            }
            let cells = side.iter().filter(|&&a| a).count();
            if cells >= min_cells && n - cells >= min_cells {
                let (overload, length, mean_strength) =
                    self.section_overload(piece, index, &side, &balanced, strength);
                section.cut = overload;
                if overload > section.overload {
                    section = Section {
                        side,
                        overload,
                        length,
                        strength: mean_strength,
                        ..section
                    };
                }
            }
        }
        Some(section)
    }

    /// Splits the plates that have been overloaded for long enough (see
    /// [`PlateSheet::pending_breakup`]) along their most overloaded section, if both parts are
    /// still large enough.
    pub(super) fn break_overloaded_plates(&mut self) {
        let min_cells = self.params.breakup.min_area * self.mesh.len() as f64;
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
            if (piece.len() as f64) < min_cells || ((total - piece.len()) as f64) < min_cells {
                continue;
            }
            if let Some(new_plate) = self.split_plate_cells(plate, &piece) {
                self.plates[plate as usize].sheet.breakup_timer = 0.0;
                self.plates[new_plate as usize].sheet.breakup_timer = 0.0;
                self.breakup_statistics.splits += 1;
                tracing::debug!(
                    plate,
                    new_plate,
                    cells = piece.len(),
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
