//! Rigid-plate tectonics on the unit sphere.
//!
//! Each plate is a rigid body rotating around its own Euler pole. A plate stores its crust in a
//! plate-local frame, indexed by the vertices of the shared [`SphereMesh`]. The world position of
//! local sample `j` of plate `p` is `p.rotation * mesh.vertex(j)`.
//!
//! In every substep, the plates are rotated, and then each world cell is assigned to the plate
//! whose crust covers it:
//!
//! * If no plate covers a cell, the plates diverge there, and new oceanic crust is created on one
//!   of the adjacent plates (sea-floor spreading).
//! * If several plates cover a cell, they converge there. One plate overrides:
//!   - Oceanic (thin, dense) crust of the other plates subducts and is destroyed. Melting above
//!     the slab adds new arc crust in a band behind the trench on the overriding plate (volcanic
//!     arcs, Andean margins).
//!   - Continental (thick, buoyant) crust cannot subduct. Where two continental crusts collide,
//!     the crust of the lower plate is stacked onto the overriding plate around the collision
//!     (crustal thickening, orogeny, terrane accretion), so continental crust is conserved.
//!
//! Every crust sample stores its thickness, density, age and sediment cover; its elevation follows
//! from Airy isostasy plus the thermal subsidence of the lithosphere with age (see
//! [`crate::isostasy`]). Continental crust is not a separate type: crust counts as continental
//! when it is thick and light enough (see [`Crust::is_continental`]), so island arcs gradually
//! become continental crust as they thicken.
//!
//! Which plate would subduct is decided per pair of plates when they first touch, from the
//! buoyancy of their crust, and then kept (subduction polarity). It flips when continental crust
//! arrives at the trench, since continents cannot subduct under oceanic crust. Where the plates
//! of a pair converge, their boundary is a trench, with a slab that grows as it subducts.
//!
//! Plate velocities follow from a balance of forces on each plate (slab pull, ridge push, drag
//! against the mantle flow, collision resistance; see [`dynamics`]), with mantle flow from sinking
//! slabs and plumes ([`mantle`]). Plates are born by rifting above plumes and by subduction
//! initiation at old passive margins, and disappear by subduction, suturing after continental
//! collisions, or capture of tiny plates by a neighbour ([`events`]).

mod boundaries;
mod crust;
mod dynamics;
mod events;
mod generation;
mod mantle;
mod statistics;
mod topology;

pub use boundaries::GapStatistics;
pub use crust::*;
pub use events::{EventCounts, PlateEventParams};
pub use mantle::{MantleParams, Plume};
pub use statistics::*;

use boundaries::{ResolvedBoundaries, pair_key};
use generation::*;

use std::collections::{HashMap, HashSet};

use glam::{DQuat, DVec3};
use rand::rngs::ChaCha8Rng;
use rand::{RngExt, SeedableRng};

use crate::isostasy::CONTINENTAL_DENSITY;
use crate::sphere::SphereMesh;

/// Physical parameters of the simulation. Times are in Myr, elevations in metres.
#[derive(Clone, Debug)]
pub struct TectonicParams {
    /// Fraction of the initial plate boundaries that start as subduction zones.
    pub initial_subduction_fraction: f64,
    /// The fraction of the surface initially covered by continental crust.
    pub continental_fraction: f64,
    /// The number of initial continents.
    pub continent_count: usize,
    /// Crust thickness at the coasts of the initial continents in metres.
    pub continental_coast_thickness: f64,
    /// Crust thickness in the interior of the initial continents in metres.
    pub continental_interior_thickness: f64,
    /// Volume of new arc crust per subducted crust sample, as a thickness over one cell.
    ///
    /// Arcs grow by roughly 30–90 km³ per km of arc and Myr on Earth; at 50 km/Myr convergence
    /// and ~110 km cells, one subducted cell corresponds to roughly 1e13 m³ ≈ 800 m over a cell.
    pub arc_crust_per_subduction: f64,
    /// Thickness of crust (and sediment) removed from the overriding plate at the trench per
    /// subducted crust sample (subduction erosion), as a thickness over one cell.
    pub subduction_erosion: f64,
    /// Relative amounts of new arc crust at distances of 0, 1, 2, … cells from the trench.
    pub arc_profile: Vec<f64>,
    /// Relative amounts of stacked crust at distances of 0, 1, 2, … cells from a continental
    /// collision.
    pub collision_profile: Vec<f64>,
    /// Crust thicker than this is removed from below (delamination).
    pub max_crust_thickness: f64,
    /// Crust thicker than this flows sideways under its own weight (lower crustal flow,
    /// orogenic collapse), spreading thickened crust over a larger area.
    pub crustal_flow_threshold: f64,
    /// Thickness range above the threshold over which crustal flow reaches full strength.
    pub crustal_flow_range: f64,
    /// Fraction of the thickness difference between neighbouring cells that flows per Myr at full
    /// strength.
    pub crustal_flow_rate: f64,
    /// Planet radius in metres.
    pub planet_radius: f64,
    /// Effective slab pull per metre of trench at the reference slab age, in N/m. The gross
    /// negative buoyancy of a slab is several times larger, but most of it is balanced by the
    /// viscous resistance of the mantle to the sinking slab.
    pub slab_pull: f64,
    /// Slab age in Myr at which slab pull reaches [`Self::slab_pull`]; it scales with the square
    /// root of the age, like the thickness of the lithosphere.
    pub slab_pull_reference_age: f64,
    /// Lithosphere stops thickening after this age in Myr.
    pub slab_pull_max_age: f64,
    /// Plates must separate at least this fast (cm/yr) for a gap between them to become new sea
    /// floor; slower gaps are resampling artifacts. The slowest ridges spread at ~0.6–1.5 cm/yr.
    pub min_spreading_rate: f64,
    /// Plates must converge at least this fast (cm/yr) for a new trench to act (an existing slab
    /// keeps acting until the plates diverge faster than this).
    pub min_trench_convergence: f64,
    /// Slab pull reaches full strength once this much slab (m) has subducted (about the depth
    /// of the upper mantle; Conrad & Lithgow-Bertelloni 2002).
    pub max_slab_length: f64,
    /// Time constant (Myr) for a slab to sink away or break off once subduction stops.
    pub slab_decay_time: f64,
    /// Ridge push per metre of ridge in N/m.
    pub ridge_push: f64,
    /// Oceanic crust up to this age in Myr at a divergent boundary counts as a ridge flank.
    pub ridge_age: f64,
    /// Basal drag coefficient `c` in Pa·s/m: shear stress per relative velocity between plate
    /// and mantle (asthenosphere viscosity divided by its thickness).
    pub basal_drag: f64,
    /// Basal drag is this many times stronger under continental crust (deep lithospheric keels).
    pub continental_drag_factor: f64,
    /// Resistance to converging continental crust per collision boundary cell, as a multiple of
    /// the basal drag under one cell.
    pub collision_resistance: f64,
    /// Mantle flow below the plates.
    pub mantle: MantleParams,
    /// Rifting, subduction initiation and suturing.
    pub events: PlateEventParams,
    /// Subduction polarity between two plates is forgotten after they were not in contact for
    /// this long.
    pub polarity_memory: f64,
    /// Maximum fraction of a cell spacing that a plate may move in one substep.
    pub max_substep_movement: f64,
}

impl Default for TectonicParams {
    fn default() -> Self {
        Self {
            initial_subduction_fraction: 0.5,
            continental_fraction: 0.3,
            continent_count: 7,
            continental_coast_thickness: 30_000.0,
            continental_interior_thickness: 38_000.0,
            arc_crust_per_subduction: 500.0,
            subduction_erosion: 300.0,
            arc_profile: vec![0.25, 1.0, 1.0, 0.25],
            collision_profile: vec![1.0, 0.75, 0.5, 0.25],
            max_crust_thickness: 75_000.0,
            crustal_flow_threshold: 40_000.0,
            crustal_flow_range: 20_000.0,
            crustal_flow_rate: 0.05,
            planet_radius: 6.371e6,
            slab_pull: 2e13,
            slab_pull_reference_age: 100.0,
            slab_pull_max_age: 120.0,
            min_spreading_rate: 0.5,
            min_trench_convergence: 0.1,
            max_slab_length: 600e3,
            slab_decay_time: 10.0,
            ridge_push: 3e12,
            ridge_age: 20.0,
            basal_drag: 1e15,
            continental_drag_factor: 4.0,
            collision_resistance: 50.0,
            mantle: MantleParams::default(),
            events: PlateEventParams::default(),
            polarity_memory: 20.0,
            max_substep_movement: 0.5,
        }
    }
}

pub struct Plate {
    /// Rotation from the plate-local frame into the world frame.
    pub rotation: DQuat,
    /// Rotation axis scaled by the rotation speed in rad/Myr.
    pub angular_velocity: DVec3,
    /// Crust samples in the plate-local frame, indexed by mesh vertex.
    pub crust: Vec<Option<Crust>>,
    /// For each world cell, the local vertex that was closest to it in the previous substep.
    lookup_cache: Vec<u32>,
    /// Local vertices whose crust was consumed (subducted or stacked onto another plate). Gaps
    /// there are never refilled with copies, which would recreate consumed crust.
    consumed: Vec<bool>,
}

impl Plate {
    /// Maps a world cell to the nearest local vertex of this plate.
    fn local_vertex(&mut self, mesh: &SphereMesh, inverse_rotation: DQuat, cell: u32) -> u32 {
        let local_position = inverse_rotation * mesh.vertex(cell);
        let local = mesh.nearest_from(local_position, self.lookup_cache[cell as usize]);
        self.lookup_cache[cell as usize] = local;
        local
    }
}

/// The plate covering a world cell, and the local vertex of the plate's crust there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ownership {
    pub plate: u32,
    pub local: u32,
}

/// Which plate of a pair overrides the other at their common boundary.
#[derive(Clone, Copy, Debug)]
struct Polarity {
    overriding: u32,
    last_contact: f64,
    /// How long, in Myr, the continents of the two plates have been colliding without pause.
    collision_time: f64,
    /// Length of slab (m) that has subducted below the overriding plate.
    slab_length: f64,
}

pub struct Simulation {
    mesh: SphereMesh,
    params: TectonicParams,
    plates: Vec<Plate>,
    owners: Vec<Ownership>,
    /// Subduction polarity of plate pairs `(a, b)` with `a < b`.
    polarities: HashMap<(u32, u32), Polarity>,
    rng: ChaCha8Rng,
    mantle: mantle::MantleFlow,
    /// How uncovered cells were filled so far.
    gap_statistics: GapStatistics,
    /// Plate births and deaths so far.
    event_counts: EventCounts,
    /// Simulated time in Myr.
    time: f64,
}

impl Simulation {
    pub fn new(mesh: SphereMesh, plate_count: usize, params: TectonicParams, seed: u64) -> Self {
        let mut owned_rng = ChaCha8Rng::seed_from_u64(seed);
        let rng = &mut owned_rng;
        assert!(plate_count >= 1, "there must be at least one plate");
        assert!(plate_count <= mesh.len(), "more plates than cells");
        let cell_count = mesh.len();

        // Partition the sphere into plates of varying size.
        let plate_seeds = sample_distinct_cells(&mesh, plate_count, rng);
        let plate_scales: Vec<f64> = (0..plate_count)
            .map(|_| rng.random_range(0.5..2.0))
            .collect();
        let plate_of_cell = grow_regions(&mesh, &plate_seeds, &plate_scales, cell_count, rng);

        // Place continents independently of the plates.
        let continental_cells =
            (params.continental_fraction.clamp(0.0, 1.0) * cell_count as f64).round() as usize;
        let continent_seeds = sample_distinct_cells(&mesh, params.continent_count.max(1), rng);
        let continent_scales = vec![1.0; continent_seeds.len()];
        let continent_of_cell = grow_regions(
            &mesh,
            &continent_seeds,
            &continent_scales,
            continental_cells,
            rng,
        );
        let is_continental: Vec<bool> = continent_of_cell.iter().map(|&c| c != u32::MAX).collect();

        // Continents rise towards their interior, oceans get older away from plate boundaries.
        let coast_distance = distance_to(&mesh, |cell| !is_continental[cell as usize]);
        let boundary_distance = distance_to(&mesh, |cell| {
            mesh.neighbors(cell)
                .iter()
                .any(|&n| plate_of_cell[n as usize] != plate_of_cell[cell as usize])
        });

        let mut plates: Vec<Plate> = (0..plate_count)
            .map(|_| Plate {
                rotation: DQuat::IDENTITY,
                angular_velocity: DVec3::ZERO,
                crust: vec![None; cell_count],
                lookup_cache: (0..cell_count as u32).collect(),
                consumed: vec![false; cell_count],
            })
            .collect();

        for cell in 0..cell_count {
            let crust = if is_continental[cell] {
                // Continents thicken from their coasts towards their interior.
                let inland = (f64::from(coast_distance[cell]) / 5.0).min(1.0);
                let thickness = params.continental_coast_thickness
                    + (params.continental_interior_thickness - params.continental_coast_thickness)
                        * inland
                    + rng.random_range(-500.0..500.0);
                Crust {
                    thickness,
                    density: CONTINENTAL_DENSITY,
                    age: 1000.0,
                    sediment: 0.0,
                }
            } else {
                let age = (10.0 + 8.0 * f64::from(boundary_distance[cell])).min(180.0)
                    + rng.random_range(0.0..5.0);
                Crust {
                    age,
                    ..new_oceanic_crust()
                }
            };
            plates[plate_of_cell[cell] as usize].crust[cell] = Some(crust);
        }

        // Start some plate boundaries as subduction zones; there the denser side subducts.
        let mut boundary_buoyancy: HashMap<(u32, u32), (f64, f64)> = HashMap::new();
        for cell in 0..cell_count as u32 {
            let plate = plate_of_cell[cell as usize];
            for &neighbor in mesh.neighbors(cell) {
                let other = plate_of_cell[neighbor as usize];
                if other != plate {
                    let crust = plates[plate as usize].crust[cell as usize]
                        .expect("every cell has initial crust");
                    let sums = boundary_buoyancy.entry(pair_key(plate, other)).or_default();
                    if plate < other {
                        sums.0 += crust.buoyancy();
                    } else {
                        sums.1 += crust.buoyancy();
                    }
                }
            }
        }
        let mut pairs: Vec<_> = boundary_buoyancy.into_iter().collect();
        pairs.sort_unstable_by_key(|((a, b), _)| (*a, *b));
        let mut polarities = HashMap::new();
        for ((a, b), (buoyancy_a, buoyancy_b)) in pairs {
            if rng.random_bool(params.initial_subduction_fraction.clamp(0.0, 1.0)) {
                let overriding = if buoyancy_a >= buoyancy_b { a } else { b };
                polarities.insert(
                    (a, b),
                    Polarity {
                        overriding,
                        last_contact: 0.0,
                        collision_time: 0.0,
                        // Initial subduction zones have fully developed slabs.
                        slab_length: params.max_slab_length,
                    },
                );
            }
        }

        let owners = (0..cell_count as u32)
            .map(|cell| Ownership {
                plate: plate_of_cell[cell as usize],
                local: cell,
            })
            .collect();

        let mantle = mantle::MantleFlow::new(params.mantle.clone());
        Self {
            mesh,
            params,
            plates,
            owners,
            polarities,
            mantle,
            gap_statistics: GapStatistics::default(),
            event_counts: EventCounts::default(),
            rng: owned_rng,
            time: 0.0,
        }
    }

    pub fn mesh(&self) -> &SphereMesh {
        &self.mesh
    }

    pub fn time(&self) -> f64 {
        self.time
    }

    /// The plate covering a world cell.
    pub fn owner(&self, cell: u32) -> Ownership {
        self.owners[cell as usize]
    }

    /// The crust at the surface of a world cell.
    pub fn surface_crust(&self, cell: u32) -> Crust {
        let owner = self.owner(cell);
        self.plates[owner.plate as usize].crust[owner.local as usize]
            .expect("owners always point to existing crust")
    }

    /// Advances the simulation by `dt` Myr, splitting it into substeps such that no plate moves
    /// more than [`TectonicParams::max_substep_movement`] cells per substep.
    pub fn step(&mut self, dt: f64) {
        let mantle_velocity = self
            .mantle
            .velocities(&self.mesh, self.params.planet_radius);
        self.update_plate_velocities(&mantle_velocity);
        let max_angle = self
            .plates
            .iter()
            .map(|plate| plate.angular_velocity.length() * dt)
            .fold(0.0, f64::max);
        let max_substep_angle = self.params.max_substep_movement * self.mesh.mean_edge_angle();
        let substeps = (max_angle / max_substep_angle).ceil().max(1.0) as usize;
        tracing::trace!(substeps, "Tectonic step");
        for _ in 0..substeps {
            self.substep(dt / substeps as f64);
        }
        self.crustal_flow(dt);
        self.remove_dead_plates();
        self.update_slab_lengths(dt);
        self.plate_events(dt);

        let subduction = self.subduction_rates();
        self.mantle.step(dt, &subduction, &mut self.rng);
    }

    /// The active mantle plumes.
    pub fn plumes(&self) -> &[Plume] {
        self.mantle.plumes()
    }

    fn substep(&mut self, dt: f64) {
        self.time += dt;
        self.move_plates(dt);
        self.evolve_crust(dt);
        let ResolvedBoundaries {
            additions,
            collisions,
        } = self.resolve_ownership();
        self.apply_additions(&additions);
        tracing::trace!(colliding_pairs = collisions.len(), "Substep");
    }

    fn move_plates(&mut self, dt: f64) {
        for plate in &mut self.plates {
            let delta = DQuat::from_scaled_axis(plate.angular_velocity * dt);
            plate.rotation = (delta * plate.rotation).normalize();
        }
    }

    /// Ages all crust; its elevation follows from its age (thermal subsidence). Erosion is
    /// handled separately, see [`crate::erosion`].
    fn evolve_crust(&mut self, dt: f64) {
        for plate in &mut self.plates {
            for crust in plate.crust.iter_mut().flatten() {
                crust.age += dt;
            }
        }
    }

    /// Surface elevation (bedrock plus sediment) of every world cell.
    pub fn surface_elevations(&self) -> Vec<f64> {
        (0..self.mesh.len() as u32)
            .map(|cell| self.surface_crust(cell).surface_elevation())
            .collect()
    }

    /// Density of the crystalline crust of every world cell in kg/m³.
    pub fn bedrock_densities(&self) -> Vec<f64> {
        (0..self.mesh.len() as u32)
            .map(|cell| self.surface_crust(cell).density)
            .collect()
    }

    /// Sediment thickness of every world cell.
    pub fn sediment_thicknesses(&self) -> Vec<f64> {
        (0..self.mesh.len() as u32)
            .map(|cell| self.surface_crust(cell).sediment)
            .collect()
    }

    /// Applies per-cell changes of crust thickness and sediment thickness to the crust at the
    /// surface. A crust sample that is visible at several cells is only changed once.
    pub fn apply_surface_changes(&mut self, thickness_delta: &[f64], sediment_delta: &[f64]) {
        let mut changed = HashSet::new();
        for cell in 0..self.mesh.len() {
            let owner = self.owners[cell];
            if !changed.insert(owner) {
                continue;
            }
            let crust = self.plates[owner.plate as usize].crust[owner.local as usize]
                .as_mut()
                .expect("owners always point to existing crust");
            crust.thickness = (crust.thickness + thickness_delta[cell]).max(0.0);
            crust.sediment = (crust.sediment + sediment_delta[cell]).max(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isostasy::{ARC_DENSITY, CONTINENTAL_DENSITY};

    fn simulation(seed: u64) -> Simulation {
        Simulation::new(SphereMesh::icosphere(4), 8, TectonicParams::default(), seed)
    }

    #[test]
    fn isostasy_gives_earth_like_elevations() {
        let ocean = |age| Crust {
            age,
            ..new_oceanic_crust()
        };
        assert!((ocean(0.0).surface_elevation() + 2600.0).abs() < 1.0);
        assert!((ocean(1000.0).surface_elevation() + 5651.0).abs() < 1.0);
        assert!(!ocean(0.0).is_continental());

        let continent = |thickness| Crust {
            thickness,
            density: CONTINENTAL_DENSITY,
            age: 1000.0,
            sediment: 0.0,
        };
        // Normal continental crust stands a few hundred metres above sea level, doubly
        // thickened crust forms high plateaus like Tibet, thinned crust forms shelves.
        assert!((200.0..900.0).contains(&continent(35_000.0).surface_elevation()));
        assert!((4500.0..6500.0).contains(&continent(70_000.0).surface_elevation()));
        assert!((-500.0..0.0).contains(&continent(30_000.0).surface_elevation()));
        assert!(continent(30_000.0).is_continental());
    }

    #[test]
    fn thinned_continental_crust_is_not_ocean() {
        let eroded = Crust {
            thickness: OCEANIC_THICKNESS,
            density: CONTINENTAL_DENSITY,
            age: 1000.0,
            sediment: 0.0,
        };
        assert!(!eroded.is_oceanic());
        assert!(!eroded.is_continental());
        assert!(new_oceanic_crust().is_oceanic());
    }

    #[test]
    fn arcs_mature_into_continental_crust() {
        let mut crust = Crust {
            age: 80.0,
            ..new_oceanic_crust()
        };
        let mut became_continental_at = None;
        for _ in 0..100 {
            crust.add_crust(500.0, ARC_DENSITY, 75_000.0);
            if crust.is_continental() && became_continental_at.is_none() {
                became_continental_at = Some(crust.thickness);
            }
        }
        let thickness = became_continental_at.expect("arc became continental");
        assert!((18_000.0..28_000.0).contains(&thickness), "{thickness}");
        // A mature arc emerges above sea level.
        assert!(crust.surface_elevation() > 0.0);
    }

    #[test]
    fn oceanic_depth_deepens_with_age() {
        let mut previous = oceanic_depth(0.0);
        for age in 1..300 {
            let depth = oceanic_depth(f64::from(age));
            assert!(depth < previous);
            previous = depth;
        }
    }

    #[test]
    fn initial_state_covers_sphere() {
        let simulation = simulation(0);
        let statistics = simulation.statistics();
        assert_eq!(
            statistics.plate_areas.iter().sum::<usize>(),
            simulation.mesh().len()
        );
        assert!(statistics.plate_areas.iter().all(|&area| area > 0));
        assert!((statistics.continental_fraction - 0.3).abs() < 0.01);
    }

    #[test]
    fn simulation_keeps_sphere_covered_and_plates_move() {
        let mut simulation = simulation(1);
        let initial_owners: Vec<Ownership> = simulation.owners.clone();
        for _ in 0..50 {
            simulation.step(2.0);
        }
        assert!((simulation.time() - 100.0).abs() < 1e-9);

        for cell in 0..simulation.mesh().len() as u32 {
            // Panics if an owner points to destroyed crust.
            simulation.surface_crust(cell);
        }
        let statistics = simulation.statistics();
        assert!(statistics.continental_fraction > 0.1);
        assert!(statistics.continental_fraction < 0.9);
        assert_ne!(initial_owners, simulation.owners);
        // New crust was created at ridges.
        assert!(
            (0..simulation.mesh().len() as u32)
                .any(|cell| simulation.surface_crust(cell).age < 10.0)
        );
    }

    #[test]
    fn simulation_is_deterministic() {
        let mut a = simulation(7);
        let mut b = simulation(7);
        for _ in 0..10 {
            a.step(1.0);
            b.step(1.0);
        }
        assert_eq!(a.owners, b.owners);
    }
}
