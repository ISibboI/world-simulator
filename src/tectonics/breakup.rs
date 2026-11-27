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
//! The analysis needs the sheet's forces: with `sheet.enabled=0` plates do not break by stress.
//!
//! Grid: sections are resolved to a cell (~110 km at 6 subdivisions); strips narrower than ~3
//! cells are near the resolution limit, and the minimum part size keeps fragments well above it.

use glam::{DMat3, DVec3};

use super::dynamics::SECONDS_PER_MYR;
use super::weak_zones::{MIN_STRENGTH, Network, UNBREAKABLE, lithosphere_strength};
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
    /// section can carry per metre of its length in tension and shear. Estimates for old
    /// lithosphere span ~10¹³–10¹⁴ N/m depending on the rheology (Kohlstedt, Evans & Mackwell
    /// 1995; Burov 2011); 5 × 10¹³ is at the upper end, as the model's basal drag
    /// (`basal_drag`, ×4 under continents) is: with 2 × 10¹³ the plate count rose by two thirds
    /// and collision belts doubled (4 seeds); 4 × 10¹³ and 8 × 10¹³ gave the same plate shapes
    /// (see `notes/issue-123/results.md`).
    pub strength: f64,
    /// Time (Myr) for which a section must stay overloaded before the plate breaks (counted in
    /// steps of [`Self::interval`], so 5 Myr with analyses every 2 Myr take 6 Myr).
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
    /// Time (Myr) between analyses of the sections: the loads change over millions of years,
    /// and the analysis takes ~7% of a step at 6 subdivisions; every 2 Myr halves that.
    pub interval: f64,
    /// Speed (km/Myr) at which a magma-fed rift propagates from a plume across a plate (#51;
    /// see [`Simulation::plume_sections`]): published propagation rates of rifts and young
    /// spreading centres span ~15 km/Myr (3D models of propagating breakup, Jourdon et al.
    /// 2020) through a few tens (westward propagation of spreading in the Gulf of Aden,
    /// d'Acremont et al. 2010) and ~100 km/Myr (the South Atlantic breakup propagating
    /// northward) to ~140 km/Myr (Woodlark basin, Taylor et al. 1995); 50 km/Myr (5 cm/yr) is
    /// the middle of that range on a log scale. 0: plume rifts do not propagate.
    pub propagation: f64,
    /// How far (km) from a plume its magma reaches along a rift (0: along the whole rift, the
    /// default). Only for the sensitivity of the model to that assumption: dyke swarms of
    /// large igneous provinces reach 1000–2500 km from the plume (Ernst & Buchan 1997), and an
    /// opening rift melts by decompression itself.
    pub rift_reach: f64,
}

impl Default for BreakupParams {
    fn default() -> Self {
        Self {
            enabled: true,
            diagnose: false,
            strength: 5e13,
            time: 5.0,
            min_area: 0.005,
            band: 3,
            compression: 3.0,
            from: 0.0,
            interval: 2.0,
            propagation: 50.0,
            rift_reach: 0.0,
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
            "interval" => self.interval = value.max(0.0),
            "compression" => self.compression = value.max(0.0),
            "propagation" => self.propagation = value.max(0.0),
            "rift_reach" => self.rift_reach = value.max(0.0),
            _ => anyhow::bail!("unknown breakup parameter {name}"),
        }
        Ok(())
    }

    /// Whether the sections are analysed.
    pub(super) fn analyse(&self) -> bool {
        self.enabled || self.diagnose
    }
}

impl Simulation {
    /// Whether the sections are analysed in this step (see [`BreakupParams::interval`]).
    pub(super) fn section_analysis_due(&self) -> bool {
        let params = &self.params.breakup;
        params.analyse()
            && (self.last_section_analysis.is_nan()
                || self.time + 1e-9 >= self.last_section_analysis + params.interval)
    }
}

impl BreakupParams {
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
    /// Whether the section runs through a plume (see [`Simulation::plume_sections`]), and the
    /// overload of the most overloaded section through a plume (0 if the plate has none).
    pub through_plume: bool,
    pub plume_overload: f64,
    /// The overload of the best section through a plume as a magma-fed rift (see
    /// [`Simulation::plume_sections`]; 0 if none), how long (Myr) the plate's rift has
    /// propagated, and how far (km) it has to propagate from the plume's magma footprint (set at
    /// the rift's onset).
    pub rift_overload: f64,
    pub rift_timer: f64,
    pub rift_reach_km: f64,
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
    /// Splits along a section through a plume (see [`Simulation::plume_sections`]).
    pub plume_splits: u64,
    /// Analyses of plates with a section through a plume, by the overload of the most
    /// overloaded such section: below 0.25, 0.5, 1, and from 1 on.
    pub plume_overloads: [u64; 4],
    /// Plates split by a magma-fed rift that propagated across them from a plume (#51), and the
    /// analyses of plates with a plume by the overload of their best rift section (bins as
    /// [`Self::plume_overloads`]).
    pub rift_splits: u64,
    pub rift_overloads: [u64; 4],
    /// Plume rifts by the age (Myr) of their plume when the rift started to propagate: in the
    /// head stage (< 20), < 50, < 100, older.
    pub rift_onsets: [u64; 4],
    /// At the last analysis: plates with a propagating plume rift.
    pub propagating: usize,
    /// At the last step: plates with an overloaded section, and the largest overload.
    pub overloaded: usize,
    pub max_overload: f64,
    /// Wall time (s) of the analysis so far.
    pub seconds: f64,
}

/// The relative strength of the lithosphere per cell: in sliding (and, times
/// [`BreakupParams::compression`], in closing), and in opening, where a plume's magma helps
/// (see `weak_zones::magma_weakening`).
pub(super) struct Strength {
    pub(super) shear: Vec<f64>,
    pub(super) tension: Vec<f64>,
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
    /// Whether it was found among the sections through a plume, and the overload of the most
    /// overloaded section through a plume (0 if none).
    through_plume: bool,
    plume_overload: f64,
    /// The best section through a plume as a magma-fed rift, if the plate has a plume.
    rift: Option<Rift>,
}

/// A section through a plume as a magma-fed rift (#51): the opening strength along its whole
/// length is lowered by the plume's magma, so that its overload says whether the plate's loads
/// can open it as a rift propagating from the plume (see [`Simulation::plume_sections`]).
#[derive(Clone)]
struct Rift {
    /// The cells of the side next to the plume (indices into the piece).
    side: Vec<bool>,
    overload: f64,
    opening: f64,
    /// The plume's magma along the rift.
    magma: RiftMagma,
    /// The plume's position, the radius (rad) of its magma footprint and its age (Myr).
    plume: DVec3,
    radius: f64,
    plume_age: f64,
    /// How far (rad) the rift has to propagate from the footprint's edge to cross the plate.
    reach: f64,
}

/// The magma of a plume along a rift through it (#51): the factor `magma` on the strength in
/// opening of the cells `r` with `r · plume > min_dot` (all of them with `min_dot` = −2; see
/// [`BreakupParams::rift_reach`]).
#[derive(Clone, Copy, Debug)]
struct RiftMagma {
    magma: f64,
    plume: DVec3,
    min_dot: f64,
}

/// An active plume whose sections are searched (#51): its position, its magma factor on the
/// opening strength of a rift it feeds ([`super::weak_zones::plume_rift_magma`]) and the radius
/// (rad) of its magma footprint ([`super::weak_zones::magma_radius`]).
#[derive(Clone, Copy, Debug)]
pub(super) struct PlumeSource {
    pub(super) position: DVec3,
    pub(super) magma: f64,
    pub(super) radius: f64,
    pub(super) age: f64,
}

/// The sections through the plumes above a piece (see [`Simulation::plume_sections`]).
struct PlumeSearch {
    /// The most overloaded one: (overload, (level, level field whose cells below the level are
    /// the side next to the plume), torque of that side's loads).
    ordinary: Option<(f64, (u32, Vec<u32>), DVec3)>,
    /// The most overloaded one as a magma-fed rift.
    rift: Option<Rift>,
}

/// The opening strength of lithosphere of relative strength `s` with the magma factor `magma`:
/// never weaker than a ridge axis.
fn dyked(s: f64, magma: f64) -> f64 {
    if magma < 1.0 {
        (s * magma).max(MIN_STRENGTH.min(s))
    } else {
        s
    }
}

/// An edge of the mesh crossed by a section, from the side whose loads are summed to the other:
/// lever vectors of the sliding (`ω · p`) and of the opening (`ω · q`) of a relative rotation `ω`
/// of that side, and the section's relative strength × length there (unit sphere), as the
/// dissipation `weight · |ω · p| + a · |ω · q| + b · ω · q` of a relative rotation: `weight` in
/// sliding, `a + b` in opening, `a − b` in closing.
#[derive(Clone, Copy)]
struct SectionEdge {
    p: DVec3,
    q: DVec3,
    weight: f64,
    a: f64,
    b: f64,
    length: f64,
    /// Where it is (unit vector).
    r: DVec3,
}

impl SectionEdge {
    /// The section across the mesh edge from `r` towards `other` (unit vectors) with the dual
    /// face length `face`, relative strength `strength` in sliding, `tension` in opening and
    /// `compression · strength` in closing, as its two halves (so that a section of one edge
    /// also carries a moment about its middle).
    fn halves(
        r: DVec3,
        other: DVec3,
        face: f64,
        strength: f64,
        tension: f64,
        compression: f64,
    ) -> [Self; 2] {
        let mid = (r + other).normalize();
        let chord = other - r;
        let normal = (chord - mid * chord.dot(mid)).normalize();
        let along = mid.cross(normal) * (0.25 * face);
        // At a point x of the section with the unit normal n, the relative velocity ω × x
        // slides by (ω × x) · (x × n) = −ω · n and opens by −(ω × x) · n = ω · (n × x) (n points
        // away from the loaded side, which moves with ω relative to the other). The two
        // points lie ±ℓ/4 from the middle (off the sphere by ~(ℓ/4)², negligible).
        let (open, close) = (0.5 * face * tension, 0.5 * face * compression * strength);
        [mid - along, mid + along].map(|point| Self {
            p: -normal,
            q: normal.cross(point),
            weight: 0.5 * face * strength,
            a: 0.5 * (open + close),
            b: 0.5 * (open - close),
            length: 0.5 * face,
            r: point,
        })
    }
}

/// Most iterations of the limit analysis of a section (see [`limit_overload`]); the sweep over
/// the level sections estimates them quickly ([`quick_overload`]) and analyses the [`REFINED`]
/// most overloaded per distance field fully.
const ITERATIONS: usize = 30;
const REFINED: usize = 4;

/// Directions (evenly spaced over 360°) of the arms of the sections through a plume searched
/// (#51): each section runs from the plume along two arms [`PLUME_SECTION_ANGLES`] apart, one of
/// them a straight great circle (180°); a resolution of the search, not a parameter of the
/// physics. The best one is refined along the weakest lithosphere within
/// [`BreakupParams::band`] cells.
const PLUME_SECTION_DIRECTIONS: usize = 12;

/// Angles (in steps of 360° / [`PLUME_SECTION_DIRECTIONS`]) between the two arms of a section
/// through a plume: 90°, 120°, 150° and straight. Plume rifts open along two of the arms of a
/// triple junction (~120° apart; the third is the failed arm), or straight through.
const PLUME_SECTION_ANGLES: [usize; 4] = [3, 4, 5, 6];

/// The dissipation of the section `edges` at the yield traction (strength `force` of
/// lithosphere of relative strength 1) in the relative rotation `omega`.
fn dissipation(edges: &[SectionEdge], force: f64, omega: DVec3) -> f64 {
    edges
        .iter()
        .map(|e| {
            let open = omega.dot(e.q);
            e.weight * omega.dot(e.p).abs() + e.a * open.abs() + e.b * open
        })
        .sum::<f64>()
        * force
}

/// A quick lower bound of [`limit_overload`]: the best of three mechanisms, the rotation about
/// the torque's axis (the net force across the section), the rotation about the section's
/// middle (bending) and `warm` (the best one of the last section, updated).
fn quick_overload(torque: DVec3, edges: &[SectionEdge], force: f64, warm: &mut DVec3) -> f64 {
    let magnitude2 = torque.length_squared();
    if magnitude2 == 0.0 || edges.is_empty() || force <= 0.0 {
        return 0.0;
    }
    let dissipation = |omega: DVec3| dissipation(edges, force, omega);
    let middle: DVec3 = edges.iter().map(|e| e.r * e.weight).sum();
    let mut best = (f64::INFINITY, DVec3::ZERO);
    for candidate in [torque / magnitude2, middle, *warm] {
        let along = candidate.dot(torque);
        if along != 0.0 {
            let omega = candidate / along;
            let d = dissipation(omega);
            if d < best.0 {
                best = (d, omega);
            }
        }
    }
    *warm = best.1;
    1.0 / best.0
}

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
    warm: &mut DVec3,
    iterations: usize,
) -> (f64, f64) {
    let magnitude2 = torque.length_squared();
    if magnitude2 == 0.0 || edges.is_empty() || force <= 0.0 {
        return (0.0, 0.0);
    }
    let dissipation = |omega: DVec3| dissipation(edges, force, omega);
    let mut omega = torque / magnitude2;
    let mut best = dissipation(omega);
    let along = warm.dot(torque);
    if along != 0.0 {
        let candidate = *warm / along;
        let d = dissipation(candidate);
        if d < best {
            best = d;
            omega = candidate;
        }
    }
    let linear: DVec3 = edges.iter().map(|e| e.q * e.b).sum::<DVec3>();
    for _ in 0..iterations {
        let scale = edges
            .iter()
            .map(|e| omega.dot(e.p).abs().max(omega.dot(e.q).abs()))
            .fold(0.0, f64::max);
        let floor = 1e-6 * scale.max(f64::MIN_POSITIVE);
        let mut q = DMat3::IDENTITY * 1e-12 * edges.iter().map(|e| e.weight).sum::<f64>();
        for e in edges {
            q += dynamics::outer(e.p, e.p) * (e.weight / omega.dot(e.p).abs().max(floor))
                + dynamics::outer(e.q, e.q) * (e.a / omega.dot(e.q).abs().max(floor));
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
        if d.partial_cmp(&best) != Some(std::cmp::Ordering::Less) {
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
    /// documentation) and marks plates that have been overloaded for long enough (`dt` is the
    /// step; the timers count the time since the last analysis). `forces` are
    /// the forces on the lithosphere of every cell (N, world frame) at the plates' rotations
    /// `rotations` before the solve; the plates' rotations and deformation velocities are the
    /// solved ones.
    pub(super) fn analyse_sections(&mut self, forces: &[DVec3], rotations: &[DVec3], dt: f64) {
        let start = web_time::Instant::now();
        // The time since the last analysis.
        let dt = if self.last_section_analysis.is_nan() {
            dt
        } else {
            (self.time - self.last_section_analysis).max(dt)
        };
        self.last_section_analysis = self.time;
        let params = self.params.breakup.clone();
        let radius = self.params.planet_radius;
        let cell_count = self.mesh.len();
        let cell_area = 4.0 * std::f64::consts::PI * radius * radius / cell_count as f64;
        let to_metres_per_second = radius / SECONDS_PER_MYR;
        let min_cells = params.min_cells(cell_count);

        // Per cell: external load (N) at the material velocity, drag coefficient, strength.
        let mut load = vec![DVec3::ZERO; cell_count];
        let mut drag = vec![0.0; cell_count];
        let mut strength = Strength {
            shear: vec![0.0; cell_count],
            tension: vec![0.0; cell_count],
        };
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
            let s = lithosphere_strength(&crust, self.time);
            strength.shear[c] = s;
            // Magma-assisted opening, but never weaker than a ridge axis.
            strength.tension[c] = dyked(s, self.magma_weakening_at(r));
            continental[c] = crust.is_continental_origin();
            plate_cells[owner.plate as usize].push(cell);
        }

        let now = self.time;
        // Active plumes, whose sections are searched (#51).
        let plumes: Vec<PlumeSource> = self
            .mantle
            .plumes()
            .iter()
            .filter(|plume| plume.flux() > 0.0)
            .map(|plume| PlumeSource {
                position: plume.position,
                magma: weak_zones::plume_rift_magma(plume, self.params.magma_weakening),
                radius: weak_zones::magma_radius(plume),
                age: plume.age,
            })
            .collect();
        let mut records = Vec::new();
        let mut statistics = std::mem::take(&mut self.breakup_statistics);
        statistics.overloaded = 0;
        statistics.propagating = 0;
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
                    &piece, &index, &load, &drag, &strength, &plumes, min_cells, &params,
                );
                for &cell in &piece {
                    index[cell as usize] = u32::MAX;
                }
            }
            let plate = &mut self.plates[p];
            plate.sheet.pending_breakup = None;
            plate.sheet.breakup_rift = false;
            let Some((section, residual)) = best else {
                plate.sheet.breakup_timer = 0.0;
                plate.sheet.rift_timer = 0.0;
                plate.sheet.rift_plume = None;
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
            plate.sheet.breakup_through_plume = section.through_plume;
            if section.plume_overload > 0.0 {
                let bin = [0.25, 0.5, 1.0]
                    .iter()
                    .filter(|&&limit| section.plume_overload >= limit)
                    .count();
                statistics.plume_overloads[bin] += 1;
            }
            statistics.max_overload = statistics.max_overload.max(section.overload);
            // A magma-fed rift from a plume propagates while the loads can open it (#51; see
            // `plume_sections`) and stalls (its dykes freeze) as fast when they cannot; it
            // splits the plate once it has crossed it.
            let rift = section.rift.as_ref();
            if let Some(rift) = rift {
                let bin = [0.25, 0.5, 1.0]
                    .iter()
                    .filter(|&&limit| rift.overload >= limit)
                    .count();
                statistics.rift_overloads[bin] += 1;
            }
            let propagating = rift.filter(|rift| rift.overload >= 1.0 && params.propagation > 0.0);
            if let Some(rift) = propagating {
                // A rift from another plume starts anew. The rift's path is set where it
                // starts: the distance it has to run is the one at its onset, not that of
                // later sections as the plate drifts over the plume.
                let same = plate
                    .sheet
                    .rift_plume
                    .filter(|&(plume, _)| plume.dot(rift.plume) > math::cos(rift.radius));
                let reach = match same {
                    Some((_, reach)) if plate.sheet.rift_timer > 0.0 => reach,
                    _ => {
                        plate.sheet.rift_timer = 0.0;
                        rift.reach
                    }
                };
                statistics.propagating += 1;
                plate.sheet.rift_timer += dt;
                plate.sheet.rift_plume = Some((rift.plume, reach));
            } else {
                plate.sheet.rift_timer = (plate.sheet.rift_timer - dt).max(0.0);
                if plate.sheet.rift_timer == 0.0 {
                    plate.sheet.rift_plume = None;
                }
            }
            let crossed = propagating.filter(|_| {
                let reach = plate.sheet.rift_plume.map_or(0.0, |(_, reach)| reach);
                let crossing = (reach * radius / 1e3 / params.propagation).max(params.time);
                plate.sheet.rift_timer >= crossing - 1e-9
            });
            let due = if overloaded && plate.sheet.breakup_timer >= params.time - 1e-9 {
                Some((&section.side, section.opening, section.through_plume, false))
            } else {
                crossed.map(|rift| {
                    plate.sheet.rift_onset_age = rift.plume_age - plate.sheet.rift_timer;
                    (&rift.side, rift.opening, true, true)
                })
            };
            if params.enabled
                && now >= params.from
                && let Some((side, opening, through_plume, is_rift)) = due
            {
                let side_cells = side.iter().filter(|&&a| a).count();
                let small_side = side_cells * 2 <= piece.len();
                let locals: Vec<u32> = piece
                    .iter()
                    .zip(side)
                    .filter(|&(_, &a)| a == small_side)
                    .map(|(&cell, _)| self.owners[cell as usize].local)
                    .collect();
                plate.sheet.pending_breakup = Some(locals);
                plate.sheet.breakup_opening = opening;
                plate.sheet.breakup_through_plume = through_plume;
                plate.sheet.breakup_rift = is_rift;
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
                    through_plume: section.through_plume,
                    plume_overload: section.plume_overload,
                    rift_overload: rift.map_or(0.0, |rift| rift.overload),
                    rift_timer: plate.sheet.rift_timer,
                    rift_reach_km: plate
                        .sheet
                        .rift_plume
                        .map(|(_, reach)| reach)
                        .or(rift.map(|rift| rift.reach))
                        .unwrap_or(0.0)
                        * radius
                        / 1e3,
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

    /// The two halves of the section across the mesh edge `slot` (of `SheetGeometry`) from
    /// `cell` (see [`SectionEdge::halves`]); `magma`: the magma factor on the opening strength
    /// of a magma-fed rift along the section (see [`Rift`]).
    fn section_edge(
        &self,
        cell: u32,
        slot: usize,
        strength: &Strength,
        magma: Option<RiftMagma>,
    ) -> [SectionEdge; 2] {
        let geometry = &self.sheet_geometry;
        let n = geometry.neighbor[slot];
        let (c, m) = (cell as usize, n as usize);
        let tension = |i: usize| self.rift_tension(strength, i, magma);
        SectionEdge::halves(
            self.mesh.vertex(cell),
            self.mesh.vertex(n),
            geometry.face[slot],
            0.5 * (strength.shear[c] + strength.shear[m]),
            0.5 * (tension(c) + tension(m)),
            self.params.breakup.compression,
        )
    }

    /// The relative strength in opening of the cell `cell`, along a rift fed with `magma` if
    /// any.
    fn rift_tension(&self, strength: &Strength, cell: usize, magma: Option<RiftMagma>) -> f64 {
        match magma {
            Some(magma) if self.mesh.vertex(cell as u32).dot(magma.plume) > magma.min_dot => {
                strength.tension[cell].min(dyked(strength.shear[cell], magma.magma))
            }
            _ => strength.tension[cell],
        }
    }

    /// The strength (N per unit length on the unit sphere) of lithosphere of relative strength
    /// 1.
    fn section_force(&self) -> f64 {
        self.params.breakup.strength * self.params.planet_radius
    }

    /// The overload of the section between the cells of the piece on `side` and the others,
    /// with the loads `loads` (per cell of the piece): (overload, opening, length on the unit
    /// sphere, mean relative strength); `magma` as for [`Self::section_edge`].
    fn section_overload(
        &self,
        piece: &[u32],
        index: &[u32],
        side: &[bool],
        loads: &[DVec3],
        strength: &Strength,
        magma: Option<RiftMagma>,
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
                    edges.extend(self.section_edge(cell, slot, strength, magma));
                }
            }
        }
        let mut warm = DVec3::ZERO;
        let (overload, opening) =
            limit_overload(torque, &edges, self.section_force(), &mut warm, ITERATIONS);
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
    /// piece has no section with both parts of at least `min_cells`. `plumes` are the active
    /// plumes (sections through those above the piece are searched too).
    #[allow(clippy::too_many_arguments)]
    fn most_overloaded_section(
        &self,
        piece: &[u32],
        index: &[u32],
        load: &[DVec3],
        drag: &[f64],
        strength: &Strength,
        plumes: &[PlumeSource],
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
        // Best section: (overload, field, level, its torque, whether it runs through a plume).
        let mut best: Option<(f64, usize, u32, DVec3, bool)> = None;
        let mut fields = vec![d1, d2, d3];
        let empty = SectionEdge {
            p: DVec3::ZERO,
            q: DVec3::ZERO,
            weight: 0.0,
            a: 0.0,
            b: 0.0,
            length: 0.0,
            r: DVec3::ZERO,
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
                for slot in geometry.edges(cell) {
                    let m = geometry.neighbor[slot];
                    let j = index[m as usize];
                    if j != u32::MAX && distance[j as usize] as usize == d + 1 {
                        let halves = self.section_edge(cell, slot, strength, None);
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
                let overload = quick_overload(torque, crossing, force, &mut warm);
                candidates.push((overload, level, torque, warm));
            }
            candidates.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            for &(_, level, torque, mut warm) in candidates.iter().take(REFINED) {
                let crossing = &edges[edge_count[level]..edge_count[level + 1]];
                let (overload, _) = limit_overload(torque, crossing, force, &mut warm, ITERATIONS);
                if best.is_none_or(|(o, ..)| overload > o) {
                    best = Some((overload, f, level as u32, torque, false));
                }
            }
        }
        // Sections through the plumes above the piece (#51): a plume's swell pushes its
        // lithosphere apart and its magma weakens it in opening, so the most overloaded section
        // may run through it, which the level sets above rarely do.
        let mut plume_overload = 0.0;
        let search = self.plume_sections(piece, index, &loads, strength, plumes, min_cells);
        if let Some((overload, field, torque)) = search.ordinary {
            plume_overload = overload;
            if best.is_none_or(|(o, ..)| overload > o) {
                let level = field.0;
                fields.push(field.1);
                best = Some((overload, fields.len() - 1, level, torque, true));
            }
        }
        let (level_overload, field, level, level_torque, through_plume) = best?;
        let residual = net.length() / level_torque.length().max(f64::MIN_POSITIVE);
        let distance = &fields[field];
        let level_side: Vec<bool> = distance.iter().map(|&d| d < level).collect();
        let (_, level_opening, level_length, level_strength) =
            self.section_overload(piece, index, &level_side, &loads, strength, None);
        let mut section = Section {
            side: level_side,
            overload: level_overload,
            level: level_overload,
            cut: 0.0,
            length: level_length,
            strength: level_strength,
            opening: level_opening,
            through_plume,
            plume_overload,
            rift: None,
        };
        // Refined along the weakest lithosphere, where it matters. The break opens where it
        // can (most sections fail mainly by opening): the strength in opening, which a plume's
        // magma lowers.
        if level_overload > 0.5
            && let Some(side) = self.refined_side(piece, index, distance, level, params.band, |c| {
                strength.tension[c]
            })
            && side.iter().filter(|&&a| a).count() >= min_cells
            && side.iter().filter(|&&a| !a).count() >= min_cells
        {
            let (overload, opening, length, mean_strength) =
                self.section_overload(piece, index, &side, &loads, strength, None);
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
        // The best rift through a plume, refined the same way with the strength in opening of
        // the magma-fed rift.
        let mut rift = search.rift;
        if let Some(r) = rift.as_mut()
            && r.overload > 0.5
        {
            let (level, field) = self.side_field(piece, index, &r.side);
            let magma = Some(r.magma);
            if let Some(side) = self.refined_side(piece, index, &field, level, params.band, |c| {
                self.rift_tension(strength, c, magma)
            }) && side.iter().filter(|&&a| a).count() >= min_cells
                && side.iter().filter(|&&a| !a).count() >= min_cells
            {
                let (overload, opening, ..) =
                    self.section_overload(piece, index, &side, &loads, strength, magma);
                if overload > r.overload {
                    r.reach = self.rift_reach(piece, index, &side, r.plume, r.radius);
                    r.side = side;
                    r.overload = overload;
                    r.opening = opening;
                }
            }
        }
        section.rift = rift;
        Some((section, residual))
    }

    /// The side of the minimum cut of the strength `strength` (per cell, by its index in the
    /// mesh) within `band` cells of the section between the cells of the piece below `level` of
    /// the level field `distance` and the others: the source's side, connected to the cells at
    /// the band's low edge; `None` if the network is empty.
    fn refined_side(
        &self,
        piece: &[u32],
        index: &[u32],
        distance: &[u32],
        level: u32,
        band: usize,
        strength: impl Fn(usize) -> f64,
    ) -> Option<Vec<bool>> {
        let n = piece.len();
        let geometry = &self.sheet_geometry;
        let max_level = distance.iter().copied().max().unwrap_or(0);
        let low = level.saturating_sub(1 + band as u32);
        let high = (level + band as u32).min(max_level);
        // The network spans the band only: cells at its edges are tied to the source and
        // sink, and every link that can be cut lies between them.
        let cells: Vec<usize> = (0..n)
            .filter(|&i| (low..=high).contains(&distance[i]))
            .collect();
        if cells.is_empty() {
            return None;
        }
        let mut node = vec![u32::MAX; n];
        for (k, &i) in cells.iter().enumerate() {
            node[i] = k as u32;
        }
        let (source, sink) = (cells.len(), cells.len() + 1);
        let mut network = Network::new(cells.len() + 2);
        for (k, &i) in cells.iter().enumerate() {
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
                let s = 0.5 * (strength(cell as usize) + strength(m as usize));
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
        Some(side)
    }

    /// A level field for the section between the cells of the piece on `side` and the others:
    /// (level, field) with the cells of `side` below the level, the distance in cell steps from
    /// the section counted down from the level within the side and up from it outside (so that
    /// the section can be refined like the level sections).
    fn side_field(&self, piece: &[u32], index: &[u32], side: &[bool]) -> (u32, Vec<u32>) {
        let n = piece.len();
        let mut distance = vec![u32::MAX; n];
        let mut queue: Vec<usize> = Vec::new();
        for i in 0..n {
            let crosses = self.mesh.neighbors(piece[i]).iter().any(|&m| {
                let j = index[m as usize];
                j != u32::MAX && side[j as usize] != side[i]
            });
            if crosses {
                distance[i] = 0;
                queue.push(i);
            }
        }
        let mut next = 0;
        while next < queue.len() {
            let i = queue[next];
            next += 1;
            for &m in self.mesh.neighbors(piece[i]) {
                let j = index[m as usize];
                if j != u32::MAX && distance[j as usize] == u32::MAX && side[j as usize] == side[i]
                {
                    distance[j as usize] = distance[i] + 1;
                    queue.push(j as usize);
                }
            }
        }
        // Cells not reached (pieces of a side cut off by the other) count as far.
        let inside = (0..n)
            .filter(|&i| side[i] && distance[i] != u32::MAX)
            .map(|i| distance[i])
            .max()
            .unwrap_or(0);
        let level = inside + 1;
        let field = (0..n)
            .map(|i| {
                let d = distance[i].min(inside);
                if side[i] { level - 1 - d } else { level + d }
            })
            .collect();
        (level, field)
    }

    /// How far (rad) a rift from the plume at `plume` with a magma footprint of radius `radius`
    /// has to propagate to run along the whole section between the cells of the piece on
    /// `side` and the others: from the footprint's edge to the section's farthest cell.
    fn rift_reach(
        &self,
        piece: &[u32],
        index: &[u32],
        side: &[bool],
        plume: DVec3,
        radius: f64,
    ) -> f64 {
        let mut reach = 0.0f64;
        for (i, &cell) in piece.iter().enumerate() {
            if !side[i] {
                continue;
            }
            let crosses = self.mesh.neighbors(cell).iter().any(|&m| {
                let j = index[m as usize];
                j != u32::MAX && !side[j as usize]
            });
            if crosses {
                reach = reach.max(math::angle_between(plume, self.mesh.vertex(cell)));
            }
        }
        (reach - radius).max(0.0)
    }

    /// The most overloaded sections through the plumes `plumes` above the piece, with two arms
    /// from the plume ([`PLUME_SECTION_DIRECTIONS`], [`PLUME_SECTION_ANGLES`]).
    ///
    /// The side next to the plume is the connected part of the piece between the two arms that
    /// touches the plume's cell, the other side the rest; sections that leave a part smaller
    /// than `min_cells` are skipped. [`PlumeSearch::ordinary`] is the most overloaded of them
    /// with the lithosphere's strength (a candidate of #182's search); [`PlumeSearch::rift`]
    /// the most overloaded as a magma-fed rift: with the strength in opening along the whole
    /// section lowered by the plume's magma (#51). A rift tip concentrates the stress of the
    /// loads that the open rift no longer carries on the intact lithosphere ahead of it, which
    /// yields there long before the whole section would; the dykes advance into it, fed by melt
    /// channelled along the opening rift from the plume (Courtillot 1982; Ebinger & Sleep 1998;
    /// Buck 2006). What limits the rift is the work of the loads when the plate opens along it
    /// against the strength of the magma-fed rift: the limit analysis of the section with that
    /// strength (an overload of at least 1: the rift propagates). It propagates at
    /// [`BreakupParams::propagation`] (see [`Simulation::analyse_sections`]).
    fn plume_sections(
        &self,
        piece: &[u32],
        index: &[u32],
        loads: &[DVec3],
        strength: &Strength,
        plumes: &[PlumeSource],
        min_cells: usize,
    ) -> PlumeSearch {
        let n = piece.len();
        let force = self.section_force();
        let geometry = &self.sheet_geometry;
        let mut best: Option<(f64, Vec<bool>, DVec3)> = None;
        let mut best_rift: Option<Rift> = None;
        let mut side = vec![false; n];
        let (mut edges, mut rift_edges): (Vec<SectionEdge>, Vec<SectionEdge>) =
            (Vec::new(), Vec::new());
        for plume in plumes {
            let centre = self.mesh.nearest(plume.position);
            let start = index[centre as usize];
            if start == u32::MAX {
                continue;
            }
            let p = self.mesh.vertex(centre);
            let east = DVec3::Z.cross(p);
            let east = if east.length_squared() > 1e-12 {
                east.normalize()
            } else {
                DVec3::X
            };
            let north = p.cross(east);
            // Direction (in steps of the arms' directions) of every cell of the piece seen from
            // the plume.
            let step = std::f64::consts::TAU / PLUME_SECTION_DIRECTIONS as f64;
            let direction: Vec<f64> = piece
                .iter()
                .map(|&cell| {
                    let r = self.mesh.vertex(cell);
                    let a = math::atan2(r.dot(north), r.dot(east)) / step;
                    a.rem_euclid(PLUME_SECTION_DIRECTIONS as f64)
                })
                .collect();
            let wedges = PLUME_SECTION_ANGLES.iter().flat_map(|&width| {
                // A straight section and its complement are the same section.
                let starts = if 2 * width == PLUME_SECTION_DIRECTIONS {
                    PLUME_SECTION_DIRECTIONS / 2
                } else {
                    PLUME_SECTION_DIRECTIONS
                };
                (0..starts).map(move |start| (start, width))
            });
            // A rift only from a plume that supplies magma.
            let reach = self.params.breakup.rift_reach * 1e3 / self.params.planet_radius;
            let magma = (plume.magma < 1.0).then_some(RiftMagma {
                magma: plume.magma,
                plume: p,
                min_dot: if reach > 0.0 { math::cos(reach) } else { -2.0 },
            });
            // (overload, overload as a rift, side, torque of the side's loads)
            let mut candidates: Vec<(f64, f64, Vec<bool>, DVec3)> = Vec::new();
            for (first, width) in wedges {
                let inside = |i: usize| {
                    let d =
                        (direction[i] - first as f64).rem_euclid(PLUME_SECTION_DIRECTIONS as f64);
                    d < width as f64
                };
                // The plume's cell and the cells of the piece between the two arms, connected
                // to it.
                side.fill(false);
                side[start as usize] = true;
                let mut stack = vec![start as usize];
                while let Some(i) = stack.pop() {
                    for &m in self.mesh.neighbors(piece[i]) {
                        let j = index[m as usize];
                        if j != u32::MAX && !side[j as usize] && inside(j as usize) {
                            side[j as usize] = true;
                            stack.push(j as usize);
                        }
                    }
                }
                let cells = side.iter().filter(|&&a| a).count();
                if cells < min_cells || n - cells < min_cells {
                    continue;
                }
                let mut torque = DVec3::ZERO;
                edges.clear();
                rift_edges.clear();
                for (i, &cell) in piece.iter().enumerate() {
                    if !side[i] {
                        continue;
                    }
                    torque += self.mesh.vertex(cell).cross(loads[i]);
                    for slot in geometry.edges(cell) {
                        let j = index[geometry.neighbor[slot] as usize];
                        if j != u32::MAX && !side[j as usize] {
                            edges.extend(self.section_edge(cell, slot, strength, None));
                            if magma.is_some() {
                                rift_edges.extend(self.section_edge(cell, slot, strength, magma));
                            }
                        }
                    }
                }
                let mut warm = DVec3::ZERO;
                let overload = quick_overload(torque, &edges, force, &mut warm);
                let rift_overload = if magma.is_some() {
                    let mut warm = DVec3::ZERO;
                    quick_overload(torque, &rift_edges, force, &mut warm)
                } else {
                    0.0
                };
                candidates.push((overload, rift_overload, side.clone(), torque));
            }
            // The most overloaded orientations in full.
            if let Some((_, _, side, torque)) = candidates.iter().max_by(|a, b| a.0.total_cmp(&b.0))
            {
                let (overload, ..) =
                    self.section_overload(piece, index, side, loads, strength, None);
                if best.as_ref().is_none_or(|(o, ..)| overload > *o) {
                    best = Some((overload, side.clone(), *torque));
                }
            }
            if let Some(magma) = magma
                && let Some((_, _, side, _)) = candidates.iter().max_by(|a, b| a.1.total_cmp(&b.1))
            {
                let (overload, opening, ..) =
                    self.section_overload(piece, index, side, loads, strength, Some(magma));
                if best_rift.as_ref().is_none_or(|r| overload > r.overload) {
                    best_rift = Some(Rift {
                        side: side.clone(),
                        overload,
                        opening,
                        magma,
                        plume: p,
                        radius: plume.radius,
                        plume_age: plume.age,
                        reach: 0.0,
                    });
                }
            }
        }
        if let Some(rift) = best_rift.as_mut() {
            rift.reach = self.rift_reach(piece, index, &rift.side, rift.plume, rift.radius);
        }
        let ordinary = best.map(|(overload, side, torque)| {
            (overload, self.side_field(piece, index, &side), torque)
        });
        PlumeSearch {
            ordinary,
            rift: best_rift,
        }
    }

    /// Splits the plates that have been overloaded for long enough (see
    /// [`super::sheet::PlateSheet::pending_breakup`]) along their most overloaded section, if both parts are
    /// still large enough.
    pub(super) fn break_overloaded_plates(&mut self) {
        let min_cells = self.params.breakup.min_cells(self.mesh.len());
        for plate in 0..self.plates.len() as u32 {
            let Some(side) = self.plates[plate as usize].sheet.pending_breakup.take() else {
                continue;
            };
            if self.plates.len() >= self.params.events.max_plates {
                self.event_counts.blocked_splits += 1;
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
            let through_plume = self.plates[plate as usize].sheet.breakup_through_plume;
            let rift = self.plates[plate as usize].sheet.breakup_rift;
            if let Some(new_plate) = self.split_plate_cells(plate, &piece) {
                for p in [plate, new_plate] {
                    let sheet = &mut self.plates[p as usize].sheet;
                    sheet.breakup_timer = 0.0;
                    sheet.rift_timer = 0.0;
                    sheet.rift_plume = None;
                    sheet.breakup_rift = false;
                }
                if rift {
                    let onset = self.plates[plate as usize].sheet.rift_onset_age;
                    let bin = [20.0, 50.0, 100.0]
                        .iter()
                        .filter(|&&limit| onset >= limit)
                        .count();
                    self.breakup_statistics.rift_onsets[bin] += 1;
                    self.breakup_statistics.rift_splits += 1;
                    self.record_split_cause(plate, new_plate, events::SplitCause::PlumeRift);
                    tracing::debug!(
                        plate,
                        new_plate,
                        cells = piece.len(),
                        opening,
                        time = self.time,
                        "Plume rift crossed the plate"
                    );
                    continue;
                }
                let statistics = &mut self.breakup_statistics;
                statistics.splits += 1;
                statistics.split_cells += piece.len() as u64;
                statistics.plume_splits += u64::from(through_plume);
                if opening > 1.0 / 3.0 {
                    statistics.opening_splits += 1;
                } else if opening < -1.0 / 3.0 {
                    statistics.closing_splits += 1;
                }
                self.record_split_cause(
                    plate,
                    new_plate,
                    if opening > 1.0 / 3.0 {
                        events::SplitCause::BreakupOpening
                    } else {
                        events::SplitCause::BreakupOther
                    },
                );
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

    /// The most overloaded section of every plate, if this step analysed them (only with
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
        let strength = Strength {
            shear: vec![1.0; n],
            tension: vec![1.0; n],
        };
        let params = simulation.params.breakup.clone();
        let section = simulation
            .most_overloaded_section(cells, &index, &loads, &drag, &strength, &[], 5, &params)
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
    /// between the loaded side (west) and the other: opening along +y; `tension` the strength
    /// in opening, `compression` times as strong in closing.
    fn straight_section(tension: f64, compression: f64) -> Vec<SectionEdge> {
        (0..200)
            .flat_map(|i| {
                let z = 0.02 * ((i as f64 + 0.5) / 200.0 - 0.5);
                let r = DVec3::new(1.0, 0.0, z).normalize();
                let east = DVec3::new(0.0, 1e-3, 0.0);
                let a = (r - east).normalize();
                let b = (r + east).normalize();
                SectionEdge::halves(a, b, 0.02 / 200.0, 1.0, tension, compression)
            })
            .collect()
    }

    #[test]
    fn limit_loads_of_a_straight_section_match_plastic_beam_theory() {
        let edges = straight_section(1.0, 3.0);
        let (force, length) = (1.0, 0.02);
        let mut warm = DVec3::ZERO;
        // A pull west on the loaded side (tension across the section) at (1, 0, 0): torque
        // r × f.
        let pull = |n: f64| DVec3::X.cross(DVec3::new(0.0, -n, 0.0));
        let (s, opening) = limit_overload(pull(0.02), &edges, force, &mut warm, ITERATIONS);
        assert!((s - 1.0).abs() < 0.02, "tension: {s}");
        assert!(opening > 0.9, "{opening}");
        // Pushed east: compression, three times as strong.
        let (s, opening) = limit_overload(pull(-0.06), &edges, force, &mut warm, ITERATIONS);
        assert!((s - 1.0).abs() < 0.02, "compression: {s}");
        assert!(opening < -0.9, "{opening}");
        // Sheared north–south.
        let shear = DVec3::X.cross(DVec3::Z * 0.02);
        let (s, _) = limit_overload(shear, &edges, force, &mut warm, ITERATIONS);
        assert!((s - 1.0).abs() < 0.02, "shear: {s}");
        // A bending moment about the radial axis: the plastic moment of a section of length L
        // in tension on one half and compression (k = 3) on the other is F L² · k / (2 (1 + k)).
        let plastic = force * length * length * 3.0 / (2.0 * 4.0);
        let (s, _) = limit_overload(DVec3::X * plastic, &edges, force, &mut warm, ITERATIONS);
        assert!((s - 1.0).abs() < 0.03, "bending: {s}");
        // With equal strengths: F L² / 4.
        let symmetric = straight_section(1.0, 1.0);
        let moment = DVec3::X * (force * length * length / 4.0);
        let (s, _) = limit_overload(moment, &symmetric, force, &mut warm, ITERATIONS);
        assert!((s - 1.0).abs() < 0.03, "symmetric bending: {s}");
    }

    #[test]
    fn magma_weakens_sections_only_in_opening() {
        // A tenth of the strength in opening (magma-assisted rifting): ten times the overload
        // in tension, the same in shear and compression.
        let (plain, magmatic) = (straight_section(1.0, 3.0), straight_section(0.1, 3.0));
        let pull = |n: f64| DVec3::X.cross(DVec3::new(0.0, -n, 0.0));
        let shear = DVec3::X.cross(DVec3::Z * 0.02);
        for (torque, ratio) in [(pull(0.02), 10.0), (pull(-0.06), 1.0), (shear, 1.0)] {
            let mut warm = DVec3::ZERO;
            let (s0, _) = limit_overload(torque, &plain, 1.0, &mut warm, ITERATIONS);
            let mut warm = DVec3::ZERO;
            let (s1, _) = limit_overload(torque, &magmatic, 1.0, &mut warm, ITERATIONS);
            assert!(
                (s1 / s0 / ratio - 1.0).abs() < 0.03,
                "{torque}: {s0} → {s1}"
            );
        }
    }

    #[test]
    fn quick_estimates_see_bending_either_way_round() {
        let edges = straight_section(1.0, 3.0);
        let plastic = 0.02f64 * 0.02 * 3.0 / 8.0;
        for sign in [1.0, -1.0] {
            let mut warm = DVec3::ZERO;
            let s = quick_overload(DVec3::X * (sign * plastic), &edges, 1.0, &mut warm);
            // About the middle (the full analysis shifts the hinge towards the stronger,
            // compressed side): (3/8) / (1/2) of the limit.
            assert!((0.74..=1.0).contains(&s), "bending ({sign}): {s}");
        }
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

    #[test]
    fn plume_swells_open_sections_through_the_plume() {
        let simulation = Simulation::new(SphereMesh::icosphere(4), 4, TectonicParams::default(), 1);
        let mesh = &simulation.mesh;
        // A cap of 40° radius with a plume at its centre, pushing the lithosphere away from it
        // (a swell) and weakening it in opening within ~10° (magma).
        let plume = DVec3::new(0.2, 0.1, 1.0).normalize();
        let cap: Vec<u32> = (0..mesh.len() as u32)
            .filter(|&c| mesh.vertex(c).dot(plume) > math::cos(40f64.to_radians()))
            .collect();
        let n = mesh.len();
        let mut index = vec![u32::MAX; n];
        for (i, &cell) in cap.iter().enumerate() {
            index[cell as usize] = i as u32;
        }
        let loads: Vec<DVec3> = (0..n as u32)
            .map(|cell| {
                let r = mesh.vertex(cell);
                let angle = math::angle_between(r, plume);
                let away = r * r.dot(plume) - plume;
                if angle > 0.0 && angle < 0.3 {
                    away.normalize() * (1e19 * math::exp(-angle * angle / 0.01) * angle / 0.1)
                } else {
                    DVec3::ZERO
                }
            })
            .collect();
        let magma = |cell: u32| {
            let x = math::angle_between(mesh.vertex(cell), plume) / 0.17;
            1.0 - 0.9 * (1.0 - x * x).max(0.0)
        };
        let strength = Strength {
            shear: vec![1.0; n],
            tension: (0..n as u32).map(magma).collect(),
        };
        let drag = vec![1.0; n];
        let params = simulation.params.breakup.clone();
        let source = PlumeSource {
            position: plume,
            magma: 1.0,
            radius: 0.17,
            age: 30.0,
        };
        let section_of = |plumes: &[PlumeSource]| {
            simulation
                .most_overloaded_section(
                    &cap, &index, &loads, &drag, &strength, plumes, 20, &params,
                )
                .expect("a section")
                .0
        };
        let section = section_of(&[source]);
        // The most overloaded section runs through the plume and opens; so does the one found
        // among the sections through the plume
        // (a curved level section through the head may do slightly better than the great
        // circles).
        assert!(
            section.plume_overload >= 0.8 * section.overload,
            "{} vs {}",
            section.plume_overload,
            section.overload
        );
        assert!(section.opening > 0.5, "{}", section.opening);
        let nearest = cap
            .iter()
            .enumerate()
            .filter(|&(i, &cell)| {
                mesh.neighbors(cell).iter().any(|&m| {
                    let j = index[m as usize];
                    j != u32::MAX && section.side[j as usize] != section.side[i]
                })
            })
            .map(|(_, &cell)| math::angle_between(mesh.vertex(cell), plume))
            .fold(f64::INFINITY, f64::min);
        assert!(nearest < 0.05, "{nearest}");
        // The level sections alone find no more overloaded one.
        let plain = section_of(&[]);
        assert!(plain.overload <= section.overload * (1.0 + 1e-9));
    }

    #[test]
    fn magma_fed_rifts_open_plates_that_the_intact_lithosphere_holds() {
        let mut simulation =
            Simulation::new(SphereMesh::icosphere(4), 4, TectonicParams::default(), 1);
        let mesh = &simulation.mesh;
        // A cap of 40° radius with a plume at its centre, pulled apart along the x axis: two
        // halves pulled in opposite directions.
        let plume = DVec3::new(0.2, 0.1, 1.0).normalize();
        let cap: Vec<u32> = (0..mesh.len() as u32)
            .filter(|&c| mesh.vertex(c).dot(plume) > math::cos(40f64.to_radians()))
            .collect();
        let n = mesh.len();
        let mut index = vec![u32::MAX; n];
        for (i, &cell) in cap.iter().enumerate() {
            index[cell as usize] = i as u32;
        }
        let east = DVec3::Z.cross(plume).normalize();
        let loads: Vec<DVec3> = (0..n as u32)
            .map(|cell| {
                let r = mesh.vertex(cell);
                let f = east * (3e18 * r.dot(east).signum());
                f - r * f.dot(r)
            })
            .collect();
        let strength = Strength {
            shear: vec![1.0; n],
            tension: vec![1.0; n],
        };
        let drag = vec![1.0; n];
        let params = simulation.params.breakup.clone();
        let analyse = |simulation: &Simulation, magma: f64| {
            let source = PlumeSource {
                position: plume,
                magma,
                radius: 0.1,
                age: 30.0,
            };
            simulation
                .most_overloaded_section(
                    &cap,
                    &index,
                    &loads,
                    &drag,
                    &strength,
                    &[source],
                    20,
                    &params,
                )
                .expect("a section")
                .0
        };
        // The intact plate holds; as a magma-fed rift through the plume it opens.
        let section = analyse(&simulation, 0.1);
        let rift = section.rift.as_ref().expect("a rift section");
        assert!(section.overload < 1.0, "{}", section.overload);
        assert!(rift.overload > 1.0, "{}", rift.overload);
        assert!(rift.overload > 2.0 * section.overload);
        assert!(rift.opening > 0.5, "{}", rift.opening);
        // It has to propagate from the edge of the magma footprint to the plate's edge.
        let reach = rift.reach;
        assert!(
            reach > 0.3 && reach < 40f64.to_radians(),
            "{}",
            reach.to_degrees()
        );
        // No magma, no rift.
        assert!(analyse(&simulation, 1.0).rift.is_none());
        // Magma only near the plume: the rift is no more loaded than the intact section much.
        simulation.params.breakup.rift_reach = 200.0;
        let near = analyse(&simulation, 0.1);
        let near = near.rift.as_ref().expect("a rift section");
        assert!(near.overload < 0.5 * rift.overload, "{}", near.overload);
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
            let mut records = 0;
            for _ in 0..10 {
                simulation.step(1.0);
                records += simulation.section_records.len();
            }
            (fingerprint(&simulation), records)
        };
        let mut plain = TectonicParams::default();
        plain.breakup.enabled = false;
        let mut diagnosed = plain.clone();
        diagnosed.breakup.diagnose = true;
        let (plain, none) = run(plain);
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
