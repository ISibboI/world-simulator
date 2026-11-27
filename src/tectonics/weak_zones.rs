//! The strength of the lithosphere, and where plates break along its weakest parts.
//!
//! [`lithosphere_strength`] combines the strength history of the lithosphere: its thermal age
//! (sea-floor age, stretching, plume heating), the thinning of stretched continental crust, the
//! orogenic weakening of collision belts and sutures, and the extension fabric of rifted
//! margins and failed rifts (#54). The breakup of plates by their stress
//! ([`super::breakup`]) and the deforming plates ([`super::sheet`]) use it; so does the split
//! of a plate where old lithosphere starts to subduct: the plate breaks into two parts that the
//! driving forces pull apart. Far from the break they move as rigid blocks; the break forms
//! where separating them takes the least force: the line of least integrated lithospheric
//! strength between them, from plate boundary to plate boundary (a minimum cut). It follows weak
//! zones: young sea floor, rifted margins, young arcs and accreted belts (sutures) rather than
//! old cratons and old ocean (rifts follow mobile belts around cratons: Vauchez et al. 1998;
//! Tommasi & Vauchez 2001; Wilson-cycle reactivation of sutures: Buiter & Torsvik 2014). The
//! rigid blocks are the parts of the plate farther than [`BAND_WIDTH`] from the split's
//! reference great circle (along the margin), which stands in for the stress field: the break
//! may wander from it within that band.
//!
//! Above a plume the lithosphere is heated and thinned (see [`plume_heating_age`]), carried with
//! the plate, and while the plume supplies magma its lithosphere opens more easily (see
//! [`magma_weakening`], in the mantle's frame). Plume rifts are breakups by the plates' own stress
//! (#51), which these weaknesses and the swell's push help.

use std::collections::VecDeque;

use glam::DVec3;

use super::mantle::Plume;
use super::*;

/// Thickness (m) of unthinned continental crust; stretched (thinner) continental lithosphere is
/// weaker.
const REFERENCE_CONTINENTAL_THICKNESS: f64 = 35e3;

/// Thermal age (Myr) at which lithosphere reaches its full thickness (plate model).
const FULL_STRENGTH_AGE: f64 = 100.0;

/// Strength of freshly deformed orogenic lithosphere (collision belts, sutures, accreted
/// terranes) relative to undeformed lithosphere of the same thermal age: young mobile belts and
/// former back-arcs, where collisions concentrate, have hot lithosphere 50–60 km thick, against
/// 150–300 km for cratons and regions with a thermotectonic age above ~300 Ma (Hyndman, Currie
/// & Mazzotti 2005). The weakening heals as the belt cools (see [`OROGENIC_HEALING_TIME`]);
/// while it lasts, rifts reactivate belts and sutures (the central Atlantic opened along the
/// Alleghanian sutures ~100 Myr after they closed; Wilson 1966; Tommasi & Vauchez 2001;
/// Buiter & Torsvik 2014) and avoid cratons. The permanent part of the inheritance (mantle
/// fabric) and the compositional strength of cratonic keels are not modelled.
const SUTURE_WEAKENING: f64 = 0.25;

/// Strength of lithosphere with a full extension fabric ([`Crust::rifted`], #54) relative to
/// unfaulted lithosphere of the same thermal state and thickness: the mechanical part of a
/// rift's inherited weakness (its thermal part, the heating by stretching, and the thinning are
/// separate terms). Faults and shear zones soften with strain to a fraction of their initial
/// friction and cohesion (Huismans & Beaumont 2003 use a reduction of the friction angle from
/// 15° to 2° in the brittle layer), and the stretched mantle lithosphere keeps a fabric that
/// weakens it for deformation along the rift (Tommasi & Vauchez 2001); the depth-integrated
/// strength, carried mostly by the ductile parts, falls less. Rifted margins and failed rifts
/// (aulacogens) are reactivated in later breakups and inverted in compression (Benue Trough,
/// Dniepr–Donets; Buiter & Torsvik 2014; Manatschal et al. 2015). Heals with
/// [`OROGENIC_HEALING_TIME`].
const RIFT_FABRIC_WEAKENING: f64 = 0.5;

/// Time constant (Myr) with which the orogenic weakening ([`Crust::orogenic`]) decays: the
/// first mode of conductive re-thickening of continental lithosphere towards ~250 km,
/// `L² / (π² κ)` with κ = 10⁻⁶ m²/s. Former back-arcs and mobile belts lose most of their
/// thermal anomaly and become strong within ~300 Myr (Currie & Hyndman 2006; the Appalachians,
/// last deformed ~300 Ma, are now nearly as cool as the adjacent craton). Slower than the
/// recovery of the thermal boundary layer after stretching or plume heating
/// ([`FULL_STRENGTH_AGE`]), which rebuilds only the plate-model thickness.
pub(super) const OROGENIC_HEALING_TIME: f64 = 200.0;

/// Thermal age (Myr) to which a plume reheats the lithosphere at its centre: the
/// thermal-rejuvenation model of hotspot swells (Detrick & Crough 1978; Crough 1983). Heat-flow
/// data suggest oceanic swells are reheated less (Von Herzen et al. 1989); continental plume
/// heads such as Afar clearly thin the lithosphere.
const PLUME_HEAD_THERMAL_AGE: f64 = 25.0;

/// Radius (rad, ~1000 km) of a plume head flattened below the lithosphere, ~2000 km across
/// (Campbell & Griffiths 1990; White & McKenzie 1989).
pub(super) const PLUME_HEAD_RADIUS: f64 = 0.157;

/// Radius (rad, ~500 km) heated by a plume's tail after its head stage: hotspot swells such as
/// Hawaii's are ~1000–1200 km wide (Crough 1983; Wessel 1993).
const PLUME_TAIL_RADIUS: f64 = 0.0785;

/// Age (Myr) of a plume up to which its head heats the lithosphere over [`PLUME_HEAD_RADIUS`]:
/// the head arrives over 10 Myr ([`Plume::head_growth`]) and spreads; its flood basalts erupt
/// within a few Myr (Courtillot & Renne 2003), and the ponded head is sheared off by the moving
/// plate. Afterwards only the tail heats ([`PLUME_TAIL_RADIUS`]); the lithosphere the head
/// heated keeps the reset where the plate carries it and cools from there (see
/// [`Crust::reheated`]).
const PLUME_HEAD_STAGE: f64 = 20.0;

/// Weakest lithosphere (the axis of a spreading ridge) relative to old lithosphere.
pub const MIN_STRENGTH: f64 = 0.05;

/// Half-width (rad, ~2200 km) of the band around the reference great circle in which a break may
/// form; the plate beyond it on either side moves as a rigid block.
const BAND_WIDTH: f64 = 0.35;

/// Capacity of links that cannot break (inside crust that is not cut, and to the blocks).
pub(super) const UNBREAKABLE: f64 = 1e12;

/// Strength of the lithosphere under `crust` relative to old (cold, unthinned) lithosphere,
/// ∝ its mechanical thickness: lithosphere thickens with the square root of its thermal age
/// (half-space cooling, saturating like the plate model), which is the sea-floor age for
/// oceanic crust, the time since formation for arcs and accreted belts, and the time since the
/// last stretching for continental crust (see `boundaries::stretch_rift`), or the time since a
/// plume last heated it, if younger ([`Crust::strength_age`]); stretched continental
/// lithosphere is also thinned with its crust (McKenzie 1978), and orogenic lithosphere is
/// weakened by its remaining orogenic weakening ([`Crust::orogenic`], [`SUTURE_WEAKENING`]).
/// Old cratons and old ocean are equally strong; rifted margins, young orogenic belts and
/// sutures, lithosphere above and behind plumes, young arcs and young sea floor are weak.
///
/// This is the one place where the strength history of the lithosphere is combined; code that
/// needs the strength of a sample should call it.
pub(super) fn lithosphere_strength(crust: &Crust, time: f64) -> f64 {
    let terms = StrengthTerms::of(crust, time);
    combine_strength(terms.thermal, terms.thinning, crust.orogenic, crust.rifted)
}

/// The terms of [`lithosphere_strength`] that depend on more than [`Crust::orogenic`], for the
/// view (see [`Simulation::lithosphere_strength_terms`]); [`combine_strength`] combines them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrengthTerms {
    /// Relative thickness of the thermal boundary layer, from the thermal age
    /// ([`Crust::strength_age`], see [`thermal_strength`]), 0 to 1.
    pub thermal: f64,
    /// Thinning of stretched continental lithosphere with its crust, 0 to 1 (1 for other
    /// lithosphere).
    pub thinning: f64,
}

impl StrengthTerms {
    fn of(crust: &Crust, time: f64) -> Self {
        let thinning = if crust.is_continental_origin() {
            (crust.basement().thickness / REFERENCE_CONTINENTAL_THICKNESS).min(1.0)
        } else {
            1.0
        };
        Self {
            thermal: thermal_strength(crust.strength_age(time)),
            thinning,
        }
    }
}

/// The thermal term of [`lithosphere_strength`] for lithosphere of thermal age `age` (Myr):
/// `√(age / FULL_STRENGTH_AGE)`, at most 1.
pub fn thermal_strength(age: f64) -> f64 {
    (age.max(0.0) / FULL_STRENGTH_AGE).sqrt().min(1.0)
}

/// The orogenic term of [`lithosphere_strength`] for an orogenic weakening `orogenic`
/// ([`Crust::orogenic`], 0 to 1): 1 for undeformed lithosphere, [`SUTURE_WEAKENING`] for a
/// fresh belt.
pub fn orogenic_strength(orogenic: f64) -> f64 {
    1.0 - (1.0 - SUTURE_WEAKENING) * orogenic
}

/// The extension-fabric term of [`lithosphere_strength`] for an extension fabric `rifted`
/// ([`Crust::rifted`], 0 to 1): 1 for unfaulted lithosphere, [`RIFT_FABRIC_WEAKENING`] for a
/// fresh rift.
pub fn fabric_strength(rifted: f64) -> f64 {
    1.0 - (1.0 - RIFT_FABRIC_WEAKENING) * rifted
}

/// [`lithosphere_strength`] from its terms (see [`StrengthTerms`]), the orogenic weakening
/// ([`Crust::orogenic`]) and the extension fabric ([`Crust::rifted`]): their product, at least
/// [`MIN_STRENGTH`].
pub fn combine_strength(thermal: f64, thinning: f64, orogenic: f64, rifted: f64) -> f64 {
    (thermal * thinning * orogenic_strength(orogenic) * fabric_strength(rifted)).max(MIN_STRENGTH)
}

/// The thermal age (Myr) to which `plume` reheats the lithosphere at `r` now, or `None` outside
/// its reach: a head heats and thins the lithosphere above it to the thickness of
/// [`PLUME_HEAD_THERMAL_AGE`] old lithosphere at its centre, tapering with the head's flattened
/// shape, `1 − (d/R)²`, back to the unaffected thickness at its edge ([`PLUME_HEAD_RADIUS`];
/// [`PLUME_TAIL_RADIUS`] after the head stage), in proportion to the plume's flux relative to
/// its peak (the head arriving, the plume fading at the end of its life). The age is the one
/// whose strength, `√(age / FULL_STRENGTH_AGE)`, is that relative thickness.
pub(super) fn plume_heating_age(plume: &Plume, r: DVec3) -> Option<f64> {
    let radius = if plume.age < PLUME_HEAD_STAGE {
        PLUME_HEAD_RADIUS
    } else {
        PLUME_TAIL_RADIUS
    };
    if r.dot(plume.position) <= math::cos(radius) || plume.peak_flux <= 0.0 {
        return None;
    }
    let x = math::angle_between(r, plume.position) / radius;
    let intensity = (plume.flux() / plume.peak_flux).clamp(0.0, 1.0);
    let centre = (PLUME_HEAD_THERMAL_AGE / FULL_STRENGTH_AGE).sqrt();
    let thickness = 1.0 - (1.0 - centre) * (1.0 - x * x).max(0.0) * intensity;
    (thickness < 1.0).then_some(FULL_STRENGTH_AGE * thickness * thickness)
}

/// The factor (`magma` to 1) on the depth-integrated strength for opening of the lithosphere
/// at `r` that the plumes `plumes` supply with magma now: dykes fed by a plume take up the
/// extension of the strong upper lithosphere, so that rifting needs only a fraction `magma`
/// ([`TectonicParams::magma_weakening`]) of the force of amagmatic rifting (Buck 2004, 2006;
/// Bialas, Buck & Qin 2010; dyking in the Afar and Main Ethiopian rifts: Kendall et al. 2005).
/// The melt comes from the plume head while it spreads below the lithosphere
/// ([`PLUME_HEAD_RADIUS`] during the head stage), later from the tail
/// ([`PLUME_TAIL_RADIUS`]); its supply tapers like the head's flattened shape and scales with
/// the plume's flux relative to its peak, as the heating does ([`plume_heating_age`]). Unlike
/// the heating it is not carried with the plate: it is where the melt is now (the mantle
/// frame). The weakest factor of all plumes counts.
pub(super) fn magma_weakening(plumes: &[Plume], r: DVec3, magma: f64) -> f64 {
    if magma >= 1.0 {
        return 1.0;
    }
    let mut factor = 1.0f64;
    for plume in plumes {
        let radius = magma_radius(plume);
        if r.dot(plume.position) <= math::cos(radius) || plume.peak_flux <= 0.0 {
            continue;
        }
        let x = math::angle_between(r, plume.position) / radius;
        let supply = (1.0 - x * x).max(0.0) * magma_intensity(plume);
        factor = factor.min(1.0 - (1.0 - magma.max(0.0)) * supply);
    }
    factor
}

/// The radius (rad) of the region that `plume` supplies with magma now (see
/// [`magma_weakening`]): its head during the head stage, its tail later.
pub(super) fn magma_radius(plume: &Plume) -> f64 {
    if plume.age < PLUME_HEAD_STAGE {
        PLUME_HEAD_RADIUS
    } else {
        PLUME_TAIL_RADIUS
    }
}

/// The plume's magma supply relative to its peak (the head arriving, the plume fading), 0 to 1.
fn magma_intensity(plume: &Plume) -> f64 {
    if plume.peak_flux <= 0.0 {
        return 0.0;
    }
    (plume.flux() / plume.peak_flux).clamp(0.0, 1.0)
}

/// The magma weakening ([`magma_weakening`]) of the strength in opening of a rift that `plume`
/// feeds with magma along its length (#51): the factor at the plume's centre, `magma` at full
/// supply.
pub(super) fn plume_rift_magma(plume: &Plume, magma: f64) -> f64 {
    if magma >= 1.0 {
        return 1.0;
    }
    1.0 - (1.0 - magma.max(0.0)) * magma_intensity(plume)
}

impl Simulation {
    /// The magma weakening ([`magma_weakening`]) at the world position `r` now.
    pub(super) fn magma_weakening_at(&self, r: DVec3) -> f64 {
        magma_weakening(self.mantle.plumes(), r, self.params.magma_weakening)
    }

    /// Plumes heat the lithosphere above them: the thermal age that sets its strength is reset
    /// on the crust samples at the surface (see [`plume_heating_age`], [`Crust::reheated`]),
    /// which carry it along with the plate and cool from there. Runs once per step,
    /// immediately before the plate events, so that splits see the heating of the plumes'
    /// current positions.
    pub(super) fn reheat_above_plumes(&mut self) {
        let time = self.time;
        for plume in self.mantle.plumes() {
            for cell in 0..self.mesh.len() as u32 {
                if let Some(age) = plume_heating_age(plume, self.mesh.vertex(cell)) {
                    let owner = self.owners[cell as usize];
                    if let Some(crust) =
                        self.plates[owner.plate as usize].crust[owner.local as usize].as_mut()
                    {
                        crust.reheat(time, age);
                    }
                }
            }
        }
    }

    /// The orogenic weakening of all crust samples (hidden ones too) decays over `dt` Myr (see
    /// [`OROGENIC_HEALING_TIME`]).
    pub(super) fn heal_orogenic_weakening(&mut self, dt: f64) {
        let factor = math::exp(-dt / OROGENIC_HEALING_TIME);
        for plate in &mut self.plates {
            for crust in plate.crust.iter_mut().flatten() {
                crust.heal_orogenic(factor);
            }
        }
    }

    /// Relative strength of the lithosphere at a world cell (see [`lithosphere_strength`]).
    pub fn lithosphere_strength(&self, cell: u32) -> f64 {
        lithosphere_strength(&self.surface_crust(cell), self.time)
    }

    /// The terms of the strength of the lithosphere at a world cell that the view cannot
    /// rebuild from what it shows (see [`StrengthTerms`]).
    pub fn lithosphere_strength_terms(&self, cell: u32) -> StrengthTerms {
        StrengthTerms::of(&self.surface_crust(cell), self.time)
    }
}

/// The reference geometry of a break, standing in for the stress field.
#[derive(Clone, Copy, Debug)]
pub(super) enum Break {
    /// A great circle with the given normal; the positive part is on the side the normal points
    /// to.
    Circle { normal: DVec3 },
}

impl Break {
    /// The side of the break a point is on (true: positive) and its angular distance from it.
    pub(super) fn place(&self, r: DVec3) -> (bool, f64) {
        match *self {
            Break::Circle { normal } => {
                let offset = math::asin(r.dot(normal).clamp(-1.0, 1.0));
                (offset > 0.0, offset.abs())
            }
        }
    }
}

/// A flow network for the minimum cut (Dinic's algorithm).
pub(super) struct Network {
    /// Per edge: target node, residual capacity; edge `e ^ 1` is the reverse of edge `e`.
    to: Vec<u32>,
    capacity: Vec<f64>,
    /// Outgoing edges per node.
    edges: Vec<Vec<u32>>,
}

impl Network {
    pub(super) fn new(nodes: usize) -> Self {
        Self {
            to: Vec::new(),
            capacity: Vec::new(),
            edges: vec![Vec::new(); nodes],
        }
    }

    /// A link that can carry `capacity` in both directions.
    pub(super) fn link(&mut self, a: usize, b: usize, capacity: f64) {
        self.edges[a].push(self.to.len() as u32);
        self.to.push(b as u32);
        self.capacity.push(capacity);
        self.edges[b].push(self.to.len() as u32);
        self.to.push(a as u32);
        self.capacity.push(capacity);
    }

    /// Breadth-first levels from `source` in the residual network.
    fn levels(&self, source: usize) -> Vec<u32> {
        let mut level = vec![u32::MAX; self.edges.len()];
        level[source] = 0;
        let mut queue = VecDeque::from([source]);
        while let Some(node) = queue.pop_front() {
            for &edge in &self.edges[node] {
                let next = self.to[edge as usize] as usize;
                if self.capacity[edge as usize] > 0.0 && level[next] == u32::MAX {
                    level[next] = level[node] + 1;
                    queue.push_back(next);
                }
            }
        }
        level
    }

    /// Maximum flow from `source` to `sink`; afterwards, the nodes reachable from `source` in
    /// the residual network are its side of a minimum cut.
    pub(super) fn max_flow(&mut self, source: usize, sink: usize) -> f64 {
        let mut total = 0.0;
        loop {
            let level = self.levels(source);
            if level[sink] == u32::MAX {
                return total;
            }
            let mut next_edge = vec![0usize; self.edges.len()];
            loop {
                // One augmenting path along increasing levels (iterative depth-first search).
                let mut path: Vec<u32> = Vec::new();
                let mut node = source;
                while node != sink {
                    let mut advanced = false;
                    while next_edge[node] < self.edges[node].len() {
                        let edge = self.edges[node][next_edge[node]] as usize;
                        let target = self.to[edge] as usize;
                        if self.capacity[edge] > 0.0 && level[target] == level[node] + 1 {
                            path.push(edge as u32);
                            node = target;
                            advanced = true;
                            break;
                        }
                        next_edge[node] += 1;
                    }
                    if !advanced {
                        // Dead end: retreat.
                        let Some(edge) = path.pop() else {
                            break;
                        };
                        node = self.to[edge as usize ^ 1] as usize;
                        next_edge[node] += 1;
                    }
                }
                if node != sink {
                    break;
                }
                let flow = path
                    .iter()
                    .map(|&edge| self.capacity[edge as usize])
                    .fold(f64::INFINITY, f64::min);
                for &edge in &path {
                    self.capacity[edge as usize] -= flow;
                    self.capacity[edge as usize ^ 1] += flow;
                }
                total += flow;
            }
        }
    }

    /// Nodes reachable from `source` in the residual network.
    pub(super) fn reachable(&self, source: usize) -> Vec<bool> {
        self.levels(source)
            .into_iter()
            .map(|level| level != u32::MAX)
            .collect()
    }
}

impl Simulation {
    /// Whether `cell` touches a cell of another plate.
    pub(super) fn at_plate_boundary(&self, cell: u32) -> bool {
        let plate = self.owners[cell as usize].plate;
        self.mesh
            .neighbors(cell)
            .iter()
            .any(|&neighbor| self.owners[neighbor as usize].plate != plate)
    }

    /// The two parts of `plate` along its weakest lithosphere (see the module documentation):
    /// `Some((side, cost))` with `side[cell]` true on the positive part of `geometry`, for every
    /// cell of the plate, and the integrated strength of the break. The rigid blocks reach to
    /// `scale` (positive, negative side) times the usual distance from the reference break.
    /// `forced` cells are on the given side; cells with the same `rigid` id (other than
    /// `u32::MAX`) stay together. `None` if there is no consistent division.
    pub(super) fn weak_zone_cut(
        &self,
        plate: u32,
        geometry: &Break,
        scale: [f64; 2],
        forced: &[(u32, bool)],
        rigid: &[u32],
    ) -> Option<(Vec<bool>, f64)> {
        let cell_count = self.mesh.len();
        let cells: Vec<u32> = (0..cell_count as u32)
            .filter(|&cell| self.owners[cell as usize].plate == plate)
            .collect();
        let mut node = vec![u32::MAX; cell_count];
        for (index, &cell) in cells.iter().enumerate() {
            node[cell as usize] = index as u32;
        }
        // The rigid blocks beyond the band on either side (closer for a plate that does not
        // reach that far).
        let placed: Vec<(bool, f64)> = cells
            .iter()
            .map(|&cell| geometry.place(self.mesh.vertex(cell)))
            .collect();
        let reach = |positive: bool| {
            let farthest = placed
                .iter()
                .filter(|(side, _)| *side == positive)
                .map(|&(_, distance)| distance)
                .fold(0.0, f64::max);
            BAND_WIDTH.min(0.5 * farthest) * scale[usize::from(!positive)]
        };
        let (reach_positive, reach_negative) = (reach(true), reach(false));
        let mut seed: Vec<Option<bool>> = placed
            .iter()
            .map(|&(side, distance)| {
                let reach = if side { reach_positive } else { reach_negative };
                (reach > 0.0 && distance >= reach).then_some(side)
            })
            .collect();
        // Rigid crust reaching into both blocks is not a block (it would join them).
        let mut signs: HashMap<u32, (bool, bool)> = HashMap::default();
        for (index, &cell) in cells.iter().enumerate() {
            let id = rigid[cell as usize];
            if id != u32::MAX
                && let Some(side) = seed[index]
            {
                let entry = signs.entry(id).or_default();
                if side {
                    entry.0 = true;
                } else {
                    entry.1 = true;
                }
            }
        }
        for (index, &cell) in cells.iter().enumerate() {
            let id = rigid[cell as usize];
            if id != u32::MAX && signs.get(&id).is_some_and(|&(a, b)| a && b) {
                seed[index] = None;
            }
        }
        let mut forced_rigid: HashMap<u32, bool> = HashMap::default();
        for &(cell, side) in forced {
            let index = node[cell as usize];
            if index == u32::MAX {
                continue;
            }
            seed[index as usize] = Some(side);
            let id = rigid[cell as usize];
            if id != u32::MAX && *forced_rigid.entry(id).or_insert(side) != side {
                return None;
            }
        }
        // Blocks of rigid crust forced to one side.
        for (index, &cell) in cells.iter().enumerate() {
            if let Some(&side) = forced_rigid.get(&rigid[cell as usize]) {
                seed[index] = Some(side);
            }
        }
        if !seed.contains(&Some(true)) || !seed.contains(&Some(false)) {
            return None;
        }
        let strength: Vec<f64> = cells
            .iter()
            .map(|&cell| lithosphere_strength(&self.surface_crust(cell), self.time))
            .collect();
        let (source, sink) = (cells.len(), cells.len() + 1);
        let mut network = Network::new(cells.len() + 2);
        for (index, &cell) in cells.iter().enumerate() {
            match seed[index] {
                Some(true) => network.link(source, index, UNBREAKABLE),
                Some(false) => network.link(index, sink, UNBREAKABLE),
                None => {}
            }
            let r = self.mesh.vertex(cell);
            for &neighbor in self.mesh.neighbors(cell) {
                let other = node[neighbor as usize];
                // Each link once.
                if other == u32::MAX || neighbor < cell {
                    continue;
                }
                let capacity = if rigid[cell as usize] != u32::MAX
                    && rigid[cell as usize] == rigid[neighbor as usize]
                {
                    UNBREAKABLE
                } else {
                    math::angle_between(r, self.mesh.vertex(neighbor))
                        * 0.5
                        * (strength[index] + strength[other as usize])
                };
                network.link(index, other as usize, capacity);
            }
        }
        let cost = network.max_flow(source, sink);
        if cost >= 0.5 * UNBREAKABLE {
            return None;
        }
        let reachable = network.reachable(source);
        let mut side = vec![false; cell_count];
        for (index, &cell) in cells.iter().enumerate() {
            side[cell as usize] = reachable[index];
        }
        Some((side, cost))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plate 0 of old ocean with a young band along the meridian at azimuth `band` (radians)
    /// and its antipode, plate 1 a cap around −z (so that plate 0 has a boundary).
    fn banded_ocean(band: f64) -> Simulation {
        let mut simulation =
            Simulation::new(SphereMesh::icosphere(4), 1, TectonicParams::default(), 0);
        let band_normal = DVec3::new(-math::sin(band), math::cos(band), 0.0);
        for (local, crust) in simulation.plates[0].crust.iter_mut().enumerate() {
            let r = simulation.mesh.vertex(local as u32);
            let age = if r.dot(band_normal).abs() < 0.05 {
                1.0
            } else {
                250.0
            };
            *crust = Some(new_oceanic_crust(-age));
        }
        let cap: Vec<u32> = (0..simulation.mesh.len() as u32)
            .filter(|&cell| simulation.mesh.vertex(cell).z < -0.7)
            .collect();
        simulation.split_plate_cells(0, &cap).unwrap();
        simulation
    }

    fn equator_cell(simulation: &Simulation, azimuth: f64) -> u32 {
        let azimuth = azimuth.to_radians();
        simulation
            .mesh
            .nearest(DVec3::new(math::cos(azimuth), math::sin(azimuth), 0.0))
    }

    #[test]
    fn cuts_follow_young_lithosphere_not_the_great_circle() {
        // The reference great circle runs through the poles at azimuth 90°/270°; the young
        // band 15° off it, at 75°/255°.
        let simulation = banded_ocean(75f64.to_radians());
        let free = vec![u32::MAX; simulation.mesh.len()];
        let circle = Break::Circle { normal: DVec3::X };
        let side = simulation
            .weak_zone_cut(0, &circle, [1.0, 1.0], &[], &free)
            .map(|(side, _)| side)
            .expect("the plate can be divided");
        // Between the band and the great circle, the cut follows the band.
        for (before, after) in [(68.0, 84.0), (248.0, 264.0)] {
            assert_ne!(
                side[equator_cell(&simulation, before) as usize],
                side[equator_cell(&simulation, after) as usize],
                "{before}° and {after}° lie on different sides of the band"
            );
        }
        // Far from both, the sides are those of the reference circle's normal.
        assert!(side[equator_cell(&simulation, 0.0) as usize]);
        assert!(!side[equator_cell(&simulation, 180.0) as usize]);
        // Without the band, the cut follows the great circle.
        let mut uniform = banded_ocean(75f64.to_radians());
        for crust in uniform.plates[0].crust.iter_mut().flatten() {
            crust.birth = -250.0;
        }
        let side = uniform
            .weak_zone_cut(0, &circle, [1.0, 1.0], &[], &free)
            .map(|(side, _)| side)
            .unwrap();
        for azimuth in [10.0, 60.0, 80.0, 100.0, 120.0, 170.0] {
            let cell = equator_cell(&uniform, azimuth);
            assert_eq!(
                side[cell as usize],
                uniform.mesh.vertex(cell).x > 0.0,
                "{azimuth}°"
            );
        }
    }

    #[test]
    fn breaks_through_a_belt_are_cheaper_than_through_its_diluted_remains() {
        // An old continent with an orogenic belt a few cells wide along the meridian at azimuth
        // 75°, freshly deformed: a break along the belt costs little more than a quarter of one
        // through the craton. The same orogenic lithosphere spread over a band three times as
        // wide (a third in each sample) saves only a fifth: it attracts breaks over much shorter
        // detours than a belt.
        let continent = |half_width: f64, orogenic: f64| {
            let mut simulation = banded_ocean(75f64.to_radians());
            let band_normal = DVec3::new(
                -math::sin(75f64.to_radians()),
                math::cos(75f64.to_radians()),
                0.0,
            );
            for plate in 0..simulation.plates.len() {
                for local in 0..simulation.mesh.len() {
                    let r = simulation.mesh.vertex(local as u32);
                    let Some(crust) = simulation.plates[plate].crust[local].as_mut() else {
                        continue;
                    };
                    let belt = r.dot(band_normal).abs() < half_width;
                    *crust = Crust {
                        thickness: 35_000.0,
                        density: crate::isostasy::CONTINENTAL_DENSITY,
                        birth: -1000.0,
                        sediment: 0.0,
                        felsic: 1.0,
                        metasediment: 0.0,
                        orogeny: if belt { -400.0 } else { NO_OROGENY },
                        orogenic: if belt { orogenic } else { 0.0 },
                        rifted: 0.0,
                        reheated: NO_REHEATING,
                        cohort: Cohort::NONE,
                    };
                }
            }
            simulation
        };
        let circle = Break::Circle { normal: DVec3::X };
        let cost = |simulation: &Simulation| {
            let free = vec![u32::MAX; simulation.mesh.len()];
            simulation
                .weak_zone_cut(0, &circle, [1.0, 1.0], &[], &free)
                .map(|(_, cost)| cost)
                .expect("the plate can be divided")
        };
        let craton = cost(&continent(0.1, 0.0));
        let belt = cost(&continent(0.1, 1.0));
        let diluted = cost(&continent(0.3, 1.0 / 3.0));
        assert!(belt < 0.35 * craton, "{belt} / {craton}");
        assert!(diluted > 0.75 * craton, "{diluted} / {craton}");
        assert!(diluted < 0.85 * craton, "{diluted} / {craton}");
    }

    #[test]
    fn rigid_crust_stays_whole_and_forced_cells_keep_their_side() {
        let simulation = banded_ocean(75f64.to_radians());
        // A block straddling the reference circle at the equator.
        let block = |r: DVec3| r.y > 0.9;
        let rigid: Vec<u32> = (0..simulation.mesh.len() as u32)
            .map(|cell| {
                if block(simulation.mesh.vertex(cell)) {
                    0
                } else {
                    u32::MAX
                }
            })
            .collect();
        let anchor = simulation.mesh.nearest(DVec3::Y);
        let circle = Break::Circle { normal: DVec3::X };
        let side = simulation
            .weak_zone_cut(0, &circle, [1.0, 1.0], &[(anchor, false)], &rigid)
            .map(|(side, _)| side)
            .unwrap();
        for cell in 0..simulation.mesh.len() as u32 {
            if block(simulation.mesh.vertex(cell)) {
                assert!(!side[cell as usize]);
            }
        }
        // Forced cells on both sides of one rigid block cannot be divided.
        let other = simulation
            .mesh
            .nearest(DVec3::new(0.1, 0.99, 0.0).normalize());
        assert!(
            simulation
                .weak_zone_cut(
                    0,
                    &circle,
                    [1.0, 1.0],
                    &[(anchor, false), (other, true)],
                    &rigid
                )
                .is_none()
        );
    }

    #[test]
    fn young_and_thinned_lithosphere_is_weak() {
        let old_ocean = new_oceanic_crust(-250.0);
        let young_ocean = new_oceanic_crust(-4.0);
        let craton = Crust {
            thickness: 40e3,
            density: crate::isostasy::CONTINENTAL_DENSITY,
            birth: -1000.0,
            sediment: 0.0,
            felsic: 1.0,
            metasediment: 0.0,
            orogeny: NO_OROGENY,
            orogenic: 0.0,
            rifted: 0.0,
            reheated: NO_REHEATING,
            cohort: Cohort::NONE,
        };
        let rifted = Crust {
            thickness: 20e3,
            birth: -4.0,
            ..craton
        };
        assert_eq!(lithosphere_strength(&old_ocean, 0.0), 1.0);
        assert_eq!(lithosphere_strength(&craton, 0.0), 1.0);
        assert!(lithosphere_strength(&young_ocean, 0.0) < 0.25);
        assert!(lithosphere_strength(&rifted, 0.0) < 0.25);
        // Sutures are weak as long as their orogenic weakening lasts.
        let suture = Crust {
            orogeny: -400.0,
            orogenic: 1.0,
            rifted: 0.0,
            ..craton
        };
        assert_eq!(lithosphere_strength(&suture, 0.0), SUTURE_WEAKENING);
    }

    #[test]
    fn orogenic_lithosphere_is_weak_in_proportion_to_its_share() {
        let craton = |orogenic: f64| Crust {
            thickness: 35_000.0,
            density: crate::isostasy::CONTINENTAL_DENSITY,
            birth: -1000.0,
            sediment: 0.0,
            felsic: 1.0,
            metasediment: 0.0,
            orogeny: if orogenic > 0.0 { -400.0 } else { NO_OROGENY },
            orogenic,
            rifted: 0.0,
            reheated: NO_REHEATING,
            cohort: Cohort::NONE,
        };
        let strength = |orogenic: f64| lithosphere_strength(&craton(orogenic), 0.0);
        assert_eq!(strength(0.0), 1.0);
        assert_eq!(strength(1.0), SUTURE_WEAKENING);
        // A sample that received a tenth of a belt is weakened by a tenth of the belt's
        // weakening, not like the belt.
        assert!((strength(0.1) - (1.0 - 0.1 * (1.0 - SUTURE_WEAKENING))).abs() < 1e-12);
    }

    #[test]
    fn plume_heads_thin_the_lithosphere_above_them() {
        let plume = |age: f64| Plume {
            position: DVec3::Z,
            age,
            lifetime: 200.0,
            peak_flux: 5e-3,
        };
        let at = |angle: f64| DVec3::new(math::sin(angle), 0.0, math::cos(angle));
        let strength = |plume: &Plume, r: DVec3| {
            plume_heating_age(plume, r).map_or(1.0, |age| (age / FULL_STRENGTH_AGE).sqrt())
        };
        let head = plume(15.0);
        // As thick as 25 Myr old lithosphere at the centre, unaffected from the head's edge on.
        assert!(
            (plume_heating_age(&head, DVec3::Z).unwrap() - PLUME_HEAD_THERMAL_AGE).abs() < 1e-9
        );
        let centre = strength(&head, DVec3::Z);
        assert!((centre - 0.5).abs() < 1e-9);
        assert_eq!(plume_heating_age(&head, at(PLUME_HEAD_RADIUS)), None);
        assert_eq!(plume_heating_age(&head, at(0.5)), None);
        let middle = strength(&head, at(0.5 * PLUME_HEAD_RADIUS));
        assert!(centre < middle && middle < 1.0);
        // A head that is still arriving heats less.
        assert!((strength(&plume(5.0), DVec3::Z) - 0.75).abs() < 1e-9);
        // After the head stage only the tail's narrower swell is heated, as strongly.
        let tail = plume(50.0);
        assert!((strength(&tail, DVec3::Z) - 0.5).abs() < 1e-9);
        assert_eq!(plume_heating_age(&tail, at(0.6 * PLUME_HEAD_RADIUS)), None);
        assert!(plume_heating_age(&tail, at(0.4 * PLUME_HEAD_RADIUS)).is_some());
        // A fading plume heats less.
        assert!(strength(&plume(190.0), DVec3::Z) > 0.5);
        // Reheating resets the strength age of old lithosphere; young lithosphere does not
        // become stronger, and a reset is never undone by a weaker heating.
        let mut old_ocean = new_oceanic_crust(-250.0);
        old_ocean.reheat(0.0, PLUME_HEAD_THERMAL_AGE);
        assert!((lithosphere_strength(&old_ocean, 0.0) - 0.5).abs() < 1e-9);
        old_ocean.reheat(0.0, 80.0);
        assert!((lithosphere_strength(&old_ocean, 0.0) - 0.5).abs() < 1e-9);
        let young_ocean = new_oceanic_crust(-4.0);
        let mut heated = young_ocean;
        heated.reheat(0.0, PLUME_HEAD_THERMAL_AGE);
        assert_eq!(
            lithosphere_strength(&heated, 0.0),
            lithosphere_strength(&young_ocean, 0.0)
        );
        // The heated lithosphere cools again: full strength 75 Myr later.
        assert!(lithosphere_strength(&old_ocean, 40.0) < 1.0);
        assert_eq!(lithosphere_strength(&old_ocean, 75.0), 1.0);
    }

    #[test]
    fn plume_magma_weakens_the_head_while_the_plume_is_strong() {
        let plume = |age: f64| Plume {
            position: DVec3::Z,
            age,
            lifetime: 200.0,
            peak_flux: 5e-3,
        };
        let at = |angle: f64| DVec3::new(math::sin(angle), 0.0, math::cos(angle));
        let head = [plume(15.0)];
        // A tenth of the strength at the centre of a full head, none beyond it.
        assert!((magma_weakening(&head, DVec3::Z, 0.1) - 0.1).abs() < 1e-9);
        let middle = magma_weakening(&head, at(0.5 * PLUME_HEAD_RADIUS), 0.1);
        assert!(middle > 0.1 && middle < 1.0, "{middle}");
        assert_eq!(
            magma_weakening(&head, at(1.01 * PLUME_HEAD_RADIUS), 0.1),
            1.0
        );
        // Switched off.
        assert_eq!(magma_weakening(&head, DVec3::Z, 1.0), 1.0);
        // The tail feeds a narrower zone; an arriving head and a fading plume less magma.
        let tail = [plume(50.0)];
        assert!((magma_weakening(&tail, DVec3::Z, 0.1) - 0.1).abs() < 1e-9);
        assert_eq!(
            magma_weakening(&tail, at(0.6 * PLUME_HEAD_RADIUS), 0.1),
            1.0
        );
        assert!(magma_weakening(&[plume(5.0)], DVec3::Z, 0.1) > 0.5);
        assert!(magma_weakening(&[plume(195.0)], DVec3::Z, 0.1) > 0.5);
        // The weakest of several plumes counts.
        let both = [plume(15.0), plume(5.0)];
        assert!((magma_weakening(&both, DVec3::Z, 0.1) - 0.1).abs() < 1e-9);
    }

    #[test]
    fn extension_fabric_weakens_heals_and_mixes_like_orogenic_weakening() {
        let craton = Crust {
            thickness: 35_000.0,
            density: crate::isostasy::CONTINENTAL_DENSITY,
            birth: -1000.0,
            sediment: 0.0,
            felsic: 1.0,
            metasediment: 0.0,
            orogeny: NO_OROGENY,
            orogenic: 0.0,
            rifted: 0.0,
            reheated: NO_REHEATING,
            cohort: Cohort::NONE,
        };
        let mut rift = craton;
        rift.mark_extension(0.4);
        assert!((rift.rifted - 0.4).abs() < 1e-12);
        rift.mark_extension(2.0);
        assert_eq!(rift.rifted, 1.0);
        assert_eq!(lithosphere_strength(&rift, 0.0), RIFT_FABRIC_WEAKENING);
        // Oceanic crust carries none.
        let mut ocean = new_oceanic_crust(-50.0);
        ocean.mark_extension(1.0);
        assert_eq!(ocean.rifted, 0.0);
        ocean.mix_orogeny(NO_OROGENY, 0.0, 0.7);
        assert_eq!(ocean.rifted, 0.0);
        // Mixed by mass on continental lithosphere.
        let mut mixed = craton;
        mixed.mix_orogeny(NO_OROGENY, 0.0, 0.3);
        assert_eq!(mixed.rifted, 0.3);
        // Heals with the orogenic weakening.
        rift.heal_orogenic(math::exp(-1.0f64));
        assert!((rift.rifted - math::exp(-1.0f64)).abs() < 1e-12);
        let healed = lithosphere_strength(&rift, 0.0);
        assert!(healed > RIFT_FABRIC_WEAKENING && healed < 1.0, "{healed}");
    }

    #[test]
    fn orogenic_weakening_heals_and_commutes_with_mixing() {
        let mut simulation =
            Simulation::new(SphereMesh::icosphere(3), 1, TectonicParams::default(), 0);
        for crust in simulation.plates[0].crust.iter_mut().flatten() {
            crust.orogenic = 1.0;
        }
        for _ in 0..200 {
            simulation.heal_orogenic_weakening(1.0);
        }
        for crust in simulation.plates[0].crust.iter().flatten() {
            assert!((crust.orogenic - math::exp(-1.0f64)).abs() < 1e-9);
        }
        // A belt 200 Myr after its orogeny has regained most of its strength.
        let belt = Crust {
            thickness: 40e3,
            density: crate::isostasy::CONTINENTAL_DENSITY,
            birth: -1000.0,
            sediment: 0.0,
            felsic: 1.0,
            metasediment: 0.0,
            orogeny: -200.0,
            orogenic: math::exp(-1.0f64),
            rifted: 0.0,
            reheated: NO_REHEATING,
            cohort: Cohort::NONE,
        };
        let healed = 1.0 - (1.0 - SUTURE_WEAKENING) * math::exp(-1.0f64);
        assert!((lithosphere_strength(&belt, 0.0) - healed).abs() < 1e-12);
        assert!(healed > 0.7 && healed < 0.75);
        // The decay is linear, so the orogenic mass decays exactly: the mixing by mass
        // (which conserves it) commutes with the healing.
        let mass = |simulation: &Simulation| -> f64 {
            simulation.plates[0]
                .crust
                .iter()
                .flatten()
                .map(|crust| crust.thickness * crust.density * crust.orogenic)
                .sum()
        };
        let before = mass(&simulation);
        simulation.heal_orogenic_weakening(37.0);
        let expected = before * math::exp(-37.0 / OROGENIC_HEALING_TIME);
        assert!((mass(&simulation) / expected - 1.0).abs() < 1e-12);
    }

    #[test]
    fn splits_see_only_the_heating_carried_by_the_crust() {
        // A plume above old ocean: before the reheating ran, the lithosphere above it is not
        // weakened (there is no field in the mantle frame any more); afterwards it is.
        let mut simulation = banded_ocean(75f64.to_radians());
        let position = DVec3::new(0.0, 1.0, 0.0);
        simulation.mantle.insert_plume(Plume {
            position,
            age: 15.0,
            lifetime: 200.0,
            peak_flux: 5e-3,
        });
        let cell = simulation.mesh.nearest(position);
        assert_eq!(simulation.lithosphere_strength(cell), 1.0);
        simulation.reheat_above_plumes();
        assert!((simulation.lithosphere_strength(cell) - 0.5).abs() < 0.05);
        // The plume dies: the lithosphere keeps the reset and cools.
        simulation.mantle = mantle::MantleFlow::new(simulation.params.mantle.clone());
        simulation.time += 30.0;
        simulation.reheat_above_plumes();
        let strength = simulation.lithosphere_strength(cell);
        assert!(strength > 0.5 && strength < 1.0, "{strength}");
    }

    #[test]
    fn steps_carry_plume_heating_with_the_plate() {
        let mut params = TectonicParams::default();
        // No new plumes, so that every reset after the plume died was carried by the crust.
        params.mantle.plume_interval = 1e12;
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 4, params, 1);
        let position = DVec3::new(0.3, 0.2, 0.9).normalize();
        simulation.mantle.insert_plume(Plume {
            position,
            age: 12.0,
            lifetime: 200.0,
            peak_flux: 5e-3,
        });
        for _ in 0..5 {
            simulation.step(1.0);
        }
        // The lithosphere above the plume was heated by this step's reheating.
        let above = simulation.surface_crust(simulation.mesh.nearest(position));
        assert!(above.reheated > NO_REHEATING);
        // The plume dies; the heated samples keep their reset (a field in the mantle frame
        // would leave nothing), move with their plates and cool: none is weakened by it 80 Myr
        // later.
        simulation.mantle = mantle::MantleFlow::new(simulation.params.mantle.clone());
        for _ in 0..20 {
            simulation.step(1.0);
        }
        let heated: Vec<Crust> = simulation
            .plates
            .iter()
            .flat_map(|plate| plate.crust.iter().flatten().copied())
            .filter(|crust| crust.reheated > NO_REHEATING)
            .collect();
        assert!(heated.len() >= 10, "{}", heated.len());
        assert!(
            heated
                .iter()
                .any(|crust| crust.strength_age(simulation.time) < FULL_STRENGTH_AGE)
        );
        let later = simulation.time + 80.0;
        assert!(heated.iter().all(|crust| {
            crust.strength_age(later) >= FULL_STRENGTH_AGE || crust.age(later) < FULL_STRENGTH_AGE
        }));
    }
}
