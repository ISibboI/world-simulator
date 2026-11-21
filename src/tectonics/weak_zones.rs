//! Where plates break: along the weakest lithosphere between the parts that move apart.
//!
//! A plate splits (above a plume, or where old lithosphere starts to subduct) into two parts that
//! the driving forces pull apart. Far from the break they move as rigid blocks; the break forms
//! where separating them takes the least force: the line of least integrated lithospheric strength
//! between them, from plate boundary to plate boundary (a minimum cut). It follows weak zones:
//! young sea floor, rifted margins, young arcs and accreted belts (sutures) rather than old
//! cratons and old ocean (rifts follow mobile belts around cratons: Vauchez et al. 1998; Tommasi
//! & Vauchez 2001; Wilson-cycle reactivation of sutures: Buiter & Torsvik 2014). The rigid blocks
//! are the parts of the plate farther than [`BAND_WIDTH`] from the split's reference great circle
//! (through the plume, or along the margin), which stands in for the stress field: the break
//! may wander from it within that band.
//!
//! Above a plume head the lithosphere is heated and thinned (see [`plume_head_strength`]), so
//! plume rifts break through the plume where that is the weakest way, and their arms leave it
//! along whatever weak zones lie around it: the angle at the plume is an outcome of the cut.

use std::collections::VecDeque;

use glam::DVec3;

use super::mantle::Plume;
use super::*;

/// Thickness (m) of unthinned continental crust; stretched (thinner) continental lithosphere is
/// weaker.
const REFERENCE_CONTINENTAL_THICKNESS: f64 = 35e3;

/// Thermal age (Myr) at which lithosphere reaches its full thickness (plate model).
const FULL_STRENGTH_AGE: f64 = 100.0;

/// Strength of lithosphere deformed in an orogeny (collision belts, sutures, accreted
/// terranes) relative to undeformed lithosphere of the same thermal age, for good: orogenic
/// belts keep their mantle fabric and faults and are reactivated in later rifting (the
/// Atlantic opened along the Caledonian–Appalachian sutures ~200 Myr after they closed;
/// Wilson 1966; Tommasi & Vauchez 2001; Buiter & Torsvik 2014), while cratons are avoided. The
/// factor is the ratio of the lithosphere thickness of Phanerozoic orogenic belts to that of
/// cratons, ~100–150 vs ~200–300 km (Artemieva & Mooney 2001).
const SUTURE_WEAKENING: f64 = 0.5;

/// Thermal age (Myr) to which a plume head reheats the lithosphere at its centre: the
/// thermal-rejuvenation model of hotspot swells (Detrick & Crough 1978; Crough 1983). Heat-flow
/// data suggest oceanic swells are reheated less (Von Herzen et al. 1989); continental plume
/// heads such as Afar clearly thin the lithosphere.
const PLUME_HEAD_THERMAL_AGE: f64 = 25.0;

/// Radius (rad, ~1000 km) of a plume head flattened below the lithosphere, ~2000 km across
/// (Campbell & Griffiths 1990; White & McKenzie 1989).
pub(super) const PLUME_HEAD_RADIUS: f64 = 0.157;

/// Weakest lithosphere (the axis of a spreading ridge) relative to old lithosphere.
const MIN_STRENGTH: f64 = 0.05;

/// Half-width (rad, ~2200 km) of the band around the reference great circle in which a break may
/// form; the plate beyond it on either side moves as a rigid block.
const BAND_WIDTH: f64 = 0.35;

/// Capacity of links that cannot break (inside crust that is not cut, and to the blocks).
const UNBREAKABLE: f64 = 1e12;

/// Strength of the lithosphere under `crust` relative to old (cold, unthinned) lithosphere,
/// ∝ its mechanical thickness: lithosphere thickens with the square root of its thermal age
/// (half-space cooling, saturating like the plate model), which is the sea-floor age for
/// oceanic crust, the time since formation for arcs and accreted belts, and the time since the
/// last stretching for continental crust (see `boundaries::stretch_rift`); stretched continental
/// lithosphere is also thinned with its crust (McKenzie 1978). Old cratons and old ocean are
/// equally strong; rifted margins, orogenic belts and sutures of any age (see
/// [`SUTURE_WEAKENING`]), young arcs and young sea floor are weak. `plume_head` caps the
/// thermal thickness above plume heads (see [`plume_head_strength`]; 1 elsewhere).
pub(super) fn lithosphere_strength(crust: &Crust, time: f64, plume_head: f64) -> f64 {
    let thermal = (crust.age(time).max(0.0) / FULL_STRENGTH_AGE)
        .sqrt()
        .min(1.0)
        .min(plume_head);
    let thinning = if crust.is_continental_origin() {
        (crust.basement().thickness / REFERENCE_CONTINENTAL_THICKNESS).min(1.0)
    } else {
        1.0
    };
    let suture = 1.0 - (1.0 - SUTURE_WEAKENING) * crust.orogenic;
    (thermal * thinning * suture).max(MIN_STRENGTH)
}

/// The largest relative thickness (strength) of the lithosphere at `r` above the given plume
/// heads: a head heats and thins the lithosphere above it to the thickness of
/// [`PLUME_HEAD_THERMAL_AGE`] old lithosphere at its centre, tapering with the head's flattened
/// shape, `1 − (d/R)²`, back to the unaffected thickness at its edge ([`PLUME_HEAD_RADIUS`]),
/// in proportion to how far the head has arrived ([`Plume::head_growth`]). The weakening is
/// tied to the plume's position in the mantle, not carried along with the plate; the narrower
/// swell of the later plume tail is not distinguished from the head. 1 away from plumes.
pub(super) fn plume_head_strength(plumes: &[Plume], r: DVec3) -> f64 {
    let centre = (PLUME_HEAD_THERMAL_AGE / FULL_STRENGTH_AGE).sqrt();
    plumes
        .iter()
        .map(|plume| {
            let x = r.angle_between(plume.position) / PLUME_HEAD_RADIUS;
            if x >= 1.0 {
                1.0
            } else {
                1.0 - (1.0 - centre) * (1.0 - x * x) * plume.head_growth()
            }
        })
        .fold(1.0, f64::min)
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
                let offset = r.dot(normal).clamp(-1.0, 1.0).asin();
                (offset > 0.0, offset.abs())
            }
        }
    }
}

/// A flow network for the minimum cut (Dinic's algorithm).
struct Network {
    /// Per edge: target node, residual capacity; edge `e ^ 1` is the reverse of edge `e`.
    to: Vec<u32>,
    capacity: Vec<f64>,
    /// Outgoing edges per node.
    edges: Vec<Vec<u32>>,
}

impl Network {
    fn new(nodes: usize) -> Self {
        Self {
            to: Vec::new(),
            capacity: Vec::new(),
            edges: vec![Vec::new(); nodes],
        }
    }

    /// A link that can carry `capacity` in both directions.
    fn link(&mut self, a: usize, b: usize, capacity: f64) {
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
    fn max_flow(&mut self, source: usize, sink: usize) -> f64 {
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
    fn reachable(&self, source: usize) -> Vec<bool> {
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
            .map(|&cell| {
                let plume_head = plume_head_strength(self.mantle.plumes(), self.mesh.vertex(cell));
                lithosphere_strength(&self.surface_crust(cell), self.time, plume_head)
            })
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
                    r.angle_between(self.mesh.vertex(neighbor))
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
        let band_normal = DVec3::new(-band.sin(), band.cos(), 0.0);
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
            .nearest(DVec3::new(azimuth.cos(), azimuth.sin(), 0.0))
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
        // 75°: a break along the belt costs little more than half of one through the craton.
        // The same orogenic lithosphere spread over a band three times as wide (a third in each
        // sample) saves only a sixth: it attracts breaks over much shorter detours than a belt.
        let continent = |half_width: f64, orogenic: f64| {
            let mut simulation = banded_ocean(75f64.to_radians());
            let band_normal = DVec3::new(-75f64.to_radians().sin(), 75f64.to_radians().cos(), 0.0);
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
                        cohort: 0.0,
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
        assert!(belt < 0.65 * craton, "{belt} / {craton}");
        assert!(diluted > 0.8 * craton, "{diluted} / {craton}");
        assert!(diluted < 0.9 * craton, "{diluted} / {craton}");
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
            cohort: 0.0,
        };
        let rifted = Crust {
            thickness: 20e3,
            birth: -4.0,
            ..craton
        };
        assert_eq!(lithosphere_strength(&old_ocean, 0.0, 1.0), 1.0);
        assert_eq!(lithosphere_strength(&craton, 0.0, 1.0), 1.0);
        assert!(lithosphere_strength(&young_ocean, 0.0, 1.0) < 0.25);
        assert!(lithosphere_strength(&rifted, 0.0, 1.0) < 0.25);
        // Old sutures stay weak.
        let suture = Crust {
            orogeny: -400.0,
            orogenic: 1.0,
            ..craton
        };
        assert_eq!(lithosphere_strength(&suture, 0.0, 1.0), SUTURE_WEAKENING);
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
            cohort: 0.0,
        };
        let strength = |orogenic: f64| lithosphere_strength(&craton(orogenic), 0.0, 1.0);
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
            has_rifted: false,
        };
        let at = |angle: f64| DVec3::new(angle.sin(), 0.0, angle.cos());
        let head = [plume(50.0)];
        // As thick as 25 Myr old lithosphere at the centre, unaffected from the head's edge on.
        let centre = plume_head_strength(&head, DVec3::Z);
        assert!((centre - 0.5).abs() < 1e-9);
        assert_eq!(plume_head_strength(&head, at(PLUME_HEAD_RADIUS)), 1.0);
        assert_eq!(plume_head_strength(&head, at(0.5)), 1.0);
        let middle = plume_head_strength(&head, at(0.5 * PLUME_HEAD_RADIUS));
        assert!(centre < middle && middle < 1.0);
        // A head that is still arriving weakens less.
        assert!((plume_head_strength(&[plume(5.0)], DVec3::Z) - 0.75).abs() < 1e-9);
        // Old lithosphere is weakened, young lithosphere does not become stronger.
        let old_ocean = new_oceanic_crust(-250.0);
        let young_ocean = new_oceanic_crust(-4.0);
        assert_eq!(lithosphere_strength(&old_ocean, 0.0, centre), centre);
        assert_eq!(
            lithosphere_strength(&young_ocean, 0.0, centre),
            lithosphere_strength(&young_ocean, 0.0, 1.0)
        );
    }
}
