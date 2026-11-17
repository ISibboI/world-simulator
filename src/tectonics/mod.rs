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
//! Every crust sample stores its thickness, density, time of formation and sediment cover; its
//! elevation follows from Airy isostasy plus the thermal subsidence of the lithosphere with age
//! (see [`crate::isostasy`]). Continental crust is not a separate type: crust counts as continental
//! when it is thick and light enough (see [`Crust::is_continental`]), so island arcs gradually
//! become continental crust as they thicken.
//!
//! Which plate would subduct is decided per segment of the boundary between two plates when they
//! first touch there, from the buoyancy of their crust or a neighbouring segment, and then kept
//! (subduction polarity, see [`polarity`]). Continental crust arriving at a trench stays on top,
//! and the segment flips once it keeps arriving. Where the plates converge, a segment's boundary
//! is a trench, with a slab that grows as it subducts.
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
mod ledger;
mod mantle;
mod matching;
mod polarity;
mod statistics;
mod topology;

pub use boundaries::GapStatistics;
pub use crust::*;
pub use events::{EventCounts, PlateEventParams};
pub use ledger::MassLedger;
pub use mantle::{MantleParams, Plume};
pub use matching::{MatchingParams, MatchingStatistics};
pub use statistics::*;

use boundaries::{ResolvedBoundaries, pair_key};
use generation::*;

/// Hash map with the hasher used throughout the tectonics (keys are small integers).
pub(crate) type HashMap<K, V> = std::collections::HashMap<K, V, Hasher>;
/// Hash set with the hasher used throughout the tectonics.
pub(crate) type HashSet<T> = std::collections::HashSet<T, Hasher>;
type Hasher = rustc_hash::FxBuildHasher;

use glam::{DQuat, DVec3};
use rand::rngs::ChaCha8Rng;
use rand::{RngExt, SeedableRng};

use crate::isostasy::{CONTINENTAL_DENSITY, SEDIMENT_ROCK_DENSITY};
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
    /// Crust thickness at the edges of the initial continents in metres (thinned, partly
    /// flooded margins).
    pub continental_coast_thickness: f64,
    /// Width (m) of the margins of the initial continents over which the crust thickens from the
    /// edge to the interior.
    pub continental_margin_width: f64,
    /// Crust thickness in the interior of the initial continents in metres.
    pub continental_interior_thickness: f64,
    /// Fraction of the sediment on a subducting plate that is scraped off at the trench into an
    /// accretionary wedge of the overriding plate (frontal accretion). Sediment recycled
    /// through arc magmas is part of the arc crust. Erosive margins accrete almost nothing, so
    /// the global fraction is low.
    pub frontal_accretion: f64,
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
    /// Speed (cm/yr) at which slab resistance balances full slab pull of reference-age
    /// lithosphere: the fastest that slab pull alone can move a plate towards its trench
    /// relative to the mantle. Earth's fastest subducting plates move at ~8–10 cm/yr.
    pub slab_sinking_speed: f64,
    /// A decaying slab shorter than this (m) has broken off: the trench stops acting.
    pub min_slab_length: f64,
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
    /// the basal drag under one cell. Collisions resist strike-slip motion as strongly.
    pub collision_resistance: f64,
    /// Resistance of oceanic and mixed plate boundaries to strike-slip motion along them: each
    /// metre of boundary resists like the basal drag under this many metres times a metre.
    /// Transform faults are weak (≈ 20 MPa over ≈ 15 km brittle depth at 5 cm/yr: ≈ 200 km;
    /// Forsyth & Uyeda 1975 found transform resistance small).
    pub boundary_coupling_width: f64,
    /// Volume of the ocean water in m³ (Earth: 1.335e18), which sets the sea level.
    pub ocean_volume: f64,
    /// Mantle flow below the plates.
    pub mantle: MantleParams,
    /// How plates are matched to cells.
    pub matching: MatchingParams,
    /// Rifting, subduction initiation and suturing.
    pub events: PlateEventParams,
    /// Boundary segments with their own subduction polarity cover this distance (m) around their
    /// position (at least three cell spacings).
    pub segment_radius: f64,
    /// A boundary segment flips its polarity once continental crust has been arriving along
    /// most of it for this long (Myr).
    pub polarity_flip_time: f64,
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
            continental_fraction: 0.4,
            continent_count: 7,
            continental_coast_thickness: 22_000.0,
            continental_interior_thickness: 35_000.0,
            continental_margin_width: 1000e3,
            frontal_accretion: 0.2,
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
            min_slab_length: 20e3,
            slab_sinking_speed: 10.0,
            ridge_push: 3e12,
            ridge_age: 20.0,
            basal_drag: 1e15,
            continental_drag_factor: 4.0,
            collision_resistance: 50.0,
            boundary_coupling_width: 200e3,
            ocean_volume: 1.335e18,
            mantle: MantleParams::default(),
            matching: MatchingParams::default(),
            events: PlateEventParams::default(),
            polarity_memory: 20.0,
            segment_radius: 500e3,
            polarity_flip_time: 3.0,
            max_substep_movement: 0.5,
        }
    }
}

impl TectonicParams {
    /// Sets the tunable parameter `name` (the field name, with `events.` for
    /// [`PlateEventParams`] fields) to `value`.
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        let field = match name {
            "basal_drag" => &mut self.basal_drag,
            "continental_drag_factor" => &mut self.continental_drag_factor,
            "collision_resistance" => &mut self.collision_resistance,
            "boundary_coupling_width" => &mut self.boundary_coupling_width,
            "slab_pull" => &mut self.slab_pull,
            "slab_sinking_speed" => &mut self.slab_sinking_speed,
            "ocean_volume" => &mut self.ocean_volume,
            "continental_coast_thickness" => &mut self.continental_coast_thickness,
            "continental_interior_thickness" => &mut self.continental_interior_thickness,
            "continental_margin_width" => &mut self.continental_margin_width,
            "ridge_push" => &mut self.ridge_push,
            "arc_crust_per_subduction" => &mut self.arc_crust_per_subduction,
            "subduction_erosion" => &mut self.subduction_erosion,
            "events.initiation_age" => &mut self.events.initiation_age,
            "events.initiation_full_age" => &mut self.events.initiation_full_age,
            "events.initiation_waiting_time" => &mut self.events.initiation_waiting_time,
            "events.boundary_initiation_waiting_time" => {
                &mut self.events.boundary_initiation_waiting_time
            }
            "events.initiation_age_contrast" => &mut self.events.initiation_age_contrast,
            "events.min_oceanic_rift_area" => &mut self.events.min_oceanic_rift_area,
            "events.rift_continent_radius" => &mut self.events.rift_continent_radius,
            "events.suture_time" => &mut self.events.suture_time,
            "events.suture_trench_ratio" => &mut self.events.suture_trench_ratio,
            "events.min_breakup_area" => &mut self.events.min_breakup_area,
            "mantle.plume_interval" => &mut self.mantle.plume_interval,
            _ => anyhow::bail!("unknown or untunable tectonic parameter {name}"),
        };
        *field = value;
        Ok(())
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
    /// there are never refilled, which would recreate consumed crust.
    consumed: Vec<bool>,
    /// Where the plate's crust was after the last matching, or `None` if unknown (new, split or
    /// merged plates). See [`matching`].
    tracked: Option<matching::TrackedCrust>,
    /// The rotation at the last matching.
    matched_rotation: DQuat,
}

/// Initial ocean ages with Earth's roughly triangular age–area distribution: the oceanic cells
/// ranked by `distance` (ties by `jitter`) get the age 180 Myr · (1 − √(1 − u)) of their
/// quantile u (mean 60 Myr).
fn triangular_ages(distance: &[u32], jitter: &[f64], is_continental: &[bool]) -> Vec<f64> {
    let mut oceanic: Vec<usize> = (0..distance.len())
        .filter(|&cell| !is_continental[cell])
        .collect();
    oceanic.sort_unstable_by(|&a, &b| {
        distance[a]
            .cmp(&distance[b])
            .then(jitter[a].total_cmp(&jitter[b]))
            .then(a.cmp(&b))
    });
    let mut ages = vec![0.0; distance.len()];
    let count = oceanic.len().max(1) as f64;
    for (rank, &cell) in oceanic.iter().enumerate() {
        let quantile = (rank as f64 + 0.5) / count;
        ages[cell] = 180.0 * (1.0 - (1.0 - quantile).sqrt());
    }
    ages
}

/// Ratio of the length of a straight boundary measured as boundary cells × mean edge length to
/// its true length: boundaries on the mesh zig-zag (see the test in [`statistics`]).
const BOUNDARY_ZIGZAG: f64 = 1.1;

/// Position of crust sample `owner` in a dense per-sample array over all plates.
fn sample_index(owner: Ownership, cell_count: usize) -> usize {
    owner.plate as usize * cell_count + owner.local as usize
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

pub struct Simulation {
    mesh: SphereMesh,
    params: TectonicParams,
    plates: Vec<Plate>,
    owners: Vec<Ownership>,
    /// Subduction polarity of the boundary segments of plate pairs.
    polarities: polarity::Polarities,
    rng: ChaCha8Rng,
    mantle: mantle::MantleFlow,
    /// How uncovered cells were filled so far.
    gap_statistics: GapStatistics,
    /// Plate births and deaths so far.
    event_counts: EventCounts,
    /// Where crust mass came from and went to.
    ledger: MassLedger,
    /// How plates were matched to cells.
    matching_statistics: MatchingStatistics,
    matching_scratch: matching::MatchingScratch,
    /// Number of changes of the plate configuration (splits, merges, removals, induced
    /// subduction) so far; boundaries must be reclassified after each.
    configuration_changes: u64,
    /// Area of sea floor opened by diverging plates so far (one flank), in km².
    opened_area: f64,
    /// Sea level as an offset of dry elevations (m) relative to the datum of the isostasy, set
    /// so that the ocean holds [`TectonicParams::ocean_volume`].
    sea_level: f64,
    /// Changes of the sea level so far (m) by tectonics and by surface processes.
    sea_level_change: (f64, f64),
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
        let margin_cells = (params.continental_margin_width
            / (params.planet_radius * mesh.mean_edge_angle()))
        .max(1.0);
        let coast_distance = distance_to(&mesh, |cell| !is_continental[cell as usize]);
        let boundary_distance = distance_to(&mesh, |cell| {
            mesh.neighbors(cell)
                .iter()
                .any(|&n| plate_of_cell[n as usize] != plate_of_cell[cell as usize])
        });

        // Ocean ages grow away from plate boundaries with Earth's roughly triangular age–area
        // distribution (see `triangular_ages`); first from all boundaries, which only matters
        // for choosing the initial subduction zones below, then away from the boundaries that
        // do not become trenches (ridges), so that old crust subducts at the trenches.
        let jitter: Vec<f64> = (0..cell_count)
            .map(|cell| {
                if is_continental[cell] {
                    0.0
                } else {
                    rng.random_range(0.0..1.0)
                }
            })
            .collect();
        let ocean_age = triangular_ages(&boundary_distance, &jitter, &is_continental);

        let mut plates: Vec<Plate> = (0..plate_count)
            .map(|_| Plate {
                rotation: DQuat::IDENTITY,
                angular_velocity: DVec3::ZERO,
                crust: vec![None; cell_count],
                lookup_cache: (0..cell_count as u32).collect(),
                consumed: vec![false; cell_count],
                tracked: None,
                matched_rotation: DQuat::IDENTITY,
            })
            .collect();

        for cell in 0..cell_count {
            let crust = if is_continental[cell] {
                // Continents thicken from their coasts towards their interior.
                let inland = (f64::from(coast_distance[cell]) / margin_cells).min(1.0);
                let thickness = params.continental_coast_thickness
                    + (params.continental_interior_thickness - params.continental_coast_thickness)
                        * inland
                    + rng.random_range(-500.0..500.0);
                Crust {
                    thickness,
                    density: CONTINENTAL_DENSITY,
                    birth: -1000.0,
                    sediment: 0.0,
                }
            } else {
                new_oceanic_crust(-ocean_age[cell])
            };
            plates[plate_of_cell[cell] as usize].crust[cell] = Some(crust);
        }

        // Start some plate boundaries as subduction zones; there the denser side subducts.
        let mut boundary_buoyancy: HashMap<(u32, u32), (f64, f64)> = HashMap::default();
        for cell in 0..cell_count as u32 {
            let plate = plate_of_cell[cell as usize];
            for &neighbor in mesh.neighbors(cell) {
                let other = plate_of_cell[neighbor as usize];
                if other != plate {
                    let crust = plates[plate as usize].crust[cell as usize]
                        .expect("every cell has initial crust");
                    let sums = boundary_buoyancy.entry(pair_key(plate, other)).or_default();
                    if plate < other {
                        sums.0 += crust.buoyancy(0.0);
                    } else {
                        sums.1 += crust.buoyancy(0.0);
                    }
                }
            }
        }
        let mut pairs: Vec<_> = boundary_buoyancy.into_iter().collect();
        pairs.sort_unstable_by_key(|((a, b), _)| (*a, *b));
        let mut initial_subduction = Vec::new();
        for ((a, b), (buoyancy_a, buoyancy_b)) in pairs {
            if rng.random_bool(params.initial_subduction_fraction.clamp(0.0, 1.0)) {
                let overriding = if buoyancy_a >= buoyancy_b { a } else { b };
                initial_subduction.push(((a, b), overriding));
            }
        }
        // Ages grow away from the boundaries that are no subduction zones.
        let trench_pairs: HashSet<(u32, u32)> =
            initial_subduction.iter().map(|&(pair, _)| pair).collect();
        let ridge_distance = distance_to(&mesh, |cell| {
            let plate = plate_of_cell[cell as usize];
            mesh.neighbors(cell).iter().any(|&n| {
                let other = plate_of_cell[n as usize];
                other != plate && !trench_pairs.contains(&pair_key(plate, other))
            })
        });
        let ocean_age = triangular_ages(&ridge_distance, &jitter, &is_continental);
        for (cell, &age) in ocean_age.iter().enumerate() {
            if !is_continental[cell] {
                plates[plate_of_cell[cell] as usize].crust[cell] = Some(new_oceanic_crust(-age));
            }
        }

        let segment_radius =
            (params.segment_radius / params.planet_radius).max(3.0 * mesh.mean_edge_angle());
        let polarities = polarity::Polarities::new(segment_radius);

        let owners = (0..cell_count as u32)
            .map(|cell| Ownership {
                plate: plate_of_cell[cell as usize],
                local: cell,
            })
            .collect();

        let mantle = mantle::MantleFlow::new(params.mantle.clone());
        let mut simulation = Self {
            mesh,
            params,
            plates,
            owners,
            polarities,
            mantle,
            gap_statistics: GapStatistics::default(),
            event_counts: EventCounts::default(),
            ledger: MassLedger::default(),
            matching_statistics: MatchingStatistics::default(),
            matching_scratch: matching::MatchingScratch::default(),
            rng: owned_rng,
            configuration_changes: 0,
            opened_area: 0.0,
            sea_level: 0.0,
            sea_level_change: (0.0, 0.0),
            time: 0.0,
        };
        simulation.ledger = MassLedger::new(simulation.total_mass());
        simulation.sea_level = simulation.solve_sea_level();
        // Initial subduction zones have fully developed slabs.
        let max_slab_length = simulation.params.max_slab_length;
        for (pair, overriding) in initial_subduction {
            simulation.seed_subduction(pair, overriding, max_slab_length);
        }
        simulation
    }

    /// True length (m) of a boundary per boundary cell: the mean edge length, corrected for the
    /// zig-zag of boundaries on the mesh.
    pub(super) fn boundary_length_per_cell(&self) -> f64 {
        self.params.planet_radius * self.mesh.mean_edge_angle() / BOUNDARY_ZIGZAG
    }

    /// Current rotations of all plates.
    fn rotations(&self) -> Vec<DQuat> {
        self.plates.iter().map(|plate| plate.rotation).collect()
    }

    /// Makes `overriding` override the other plate of `pair` with a slab of `slab_length` along
    /// their whole common boundary, in segments that do not exist yet.
    pub(super) fn seed_subduction(&mut self, pair: (u32, u32), overriding: u32, slab_length: f64) {
        let rotations = self.rotations();
        for cell in 0..self.mesh.len() as u32 {
            let plate = self.owners[cell as usize].plate;
            if plate != pair.0 && plate != pair.1 {
                continue;
            }
            let other = if plate == pair.0 { pair.1 } else { pair.0 };
            if self
                .mesh
                .neighbors(cell)
                .iter()
                .any(|&neighbor| self.owners[neighbor as usize].plate == other)
            {
                let r = self.mesh.vertex(cell);
                self.polarities
                    .seed(pair, overriding, r, slab_length, self.time, &rotations);
            }
        }
        self.configuration_changes += 1;
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
        let rotations = self.rotations();
        self.polarities
            .end_step(self.time, self.params.polarity_memory, &rotations);
        self.crustal_flow(dt);
        self.remove_dead_plates();
        let boundaries = self.boundary_cells();
        let slabs_changed = self.update_slab_lengths(dt, &boundaries);
        // Trenches whose slab broke off are no trenches any more.
        let boundaries = if slabs_changed {
            self.boundary_cells()
        } else {
            boundaries
        };
        let topology = self.configuration_changes;
        self.plate_events(dt, &boundaries);
        // The boundaries only change if plates were split, merged or removed, or subduction was
        // induced.
        let boundaries = if self.configuration_changes == topology {
            boundaries
        } else {
            self.boundary_cells()
        };

        // Opening rate × boundary length over the diverging cells of both sides, halved.
        let opening: f64 = boundaries
            .iter()
            .filter(|boundary| boundary.convergence < 0.0)
            .map(|boundary| -boundary.convergence)
            .sum();
        self.opened_area += 0.5 * opening * self.boundary_length_per_cell() / 1000.0
            * self.params.planet_radius
            / 1000.0
            * dt;

        let subduction = self.subduction_rates(&boundaries);
        self.mantle.step(dt, &subduction, &mut self.rng);

        let sea_level = self.solve_sea_level();
        self.sea_level_change.0 += sea_level - self.sea_level;
        self.sea_level = sea_level;
    }

    /// Areal mass (kg/m², summed over cells) of all crust, including hidden samples.
    pub fn total_mass(&self) -> f64 {
        ledger::plates_mass(&self.plates)
    }

    /// The active mantle plumes.
    pub fn plumes(&self) -> &[Plume] {
        self.mantle.plumes()
    }

    fn substep(&mut self, dt: f64) {
        self.time += dt;
        self.move_plates(dt);
        let ResolvedBoundaries {
            additions,
            collisions,
        } = self.resolve_ownership();
        let rotations = self.rotations();
        self.polarities
            .end_pass(dt, self.params.polarity_flip_time, &rotations);
        self.apply_additions(&additions);
        tracing::trace!(colliding_pairs = collisions.len(), "Substep");
    }

    fn move_plates(&mut self, dt: f64) {
        for plate in &mut self.plates {
            let delta = DQuat::from_scaled_axis(plate.angular_velocity * dt);
            plate.rotation = (delta * plate.rotation).normalize();
        }
    }

    /// Surface elevation (bedrock plus sediment) of a world cell relative to sea level in metres.
    pub fn surface_elevation(&self, cell: u32) -> f64 {
        self.surface_crust(cell)
            .surface_elevation(self.time, self.sea_level)
    }

    /// The sea level at which the ocean holds [`TectonicParams::ocean_volume`].
    fn solve_sea_level(&self) -> f64 {
        let dry: Vec<f64> = (0..self.mesh.len() as u32)
            .map(|cell| self.surface_crust(cell).dry_elevation(self.time))
            .collect();
        let cell_area =
            4.0 * std::f64::consts::PI * self.params.planet_radius.powi(2) / self.mesh.len() as f64;
        crate::isostasy::sea_level_for_volume(&dry, cell_area, self.params.ocean_volume)
    }

    /// Surface elevation (bedrock plus sediment) of every world cell.
    pub fn surface_elevations(&self) -> Vec<f64> {
        (0..self.mesh.len() as u32)
            .map(|cell| self.surface_elevation(cell))
            .collect()
    }

    /// Density of the crystalline crust of every world cell in kg/m³.
    pub fn bedrock_densities(&self) -> Vec<f64> {
        (0..self.mesh.len() as u32)
            .map(|cell| self.surface_crust(cell).density)
            .collect()
    }

    /// Density giving the isostatic response of removing crust from the top, per world cell.
    pub fn bedrock_isostatic_densities(&self) -> Vec<f64> {
        (0..self.mesh.len() as u32)
            .map(|cell| self.surface_crust(cell).removal_isostatic_density())
            .collect()
    }

    /// Sediment thickness of every world cell.
    pub fn sediment_thicknesses(&self) -> Vec<f64> {
        (0..self.mesh.len() as u32)
            .map(|cell| self.surface_crust(cell).sediment)
            .collect()
    }

    /// Applies per-cell changes of crust thickness and sediment thickness to the crust at the
    /// surface. A crust sample visible at several cells gets the sum of their changes: erosion
    /// works per cell, and about as many samples are hidden by resampling as are shown twice.
    /// Sediment removal beyond the available sediment comes from the bedrock at the same mass.
    pub fn apply_surface_changes(&mut self, thickness_delta: &[f64], sediment_delta: &[f64]) {
        let mut changes: Vec<(Ownership, f64, f64)> = Vec::new();
        let cell_count = self.mesh.len();
        // Per crust sample: its position in `changes`, or `u32::MAX`.
        let mut index = vec![u32::MAX; self.plates.len() * cell_count];
        for cell in 0..cell_count {
            let owner = self.owners[cell];
            let slot = &mut index[sample_index(owner, cell_count)];
            if *slot == u32::MAX {
                *slot = changes.len() as u32;
                changes.push((owner, 0.0, 0.0));
            }
            let slot = *slot as usize;
            changes[slot].1 += thickness_delta[cell];
            changes[slot].2 += sediment_delta[cell];
        }
        let mut change = 0.0;
        for (owner, thickness, sediment) in changes {
            let crust = self.plates[owner.plate as usize].crust[owner.local as usize]
                .as_mut()
                .expect("owners always point to existing crust");
            let before = crust.mass();
            let new_sediment = crust.sediment + sediment;
            let mut new_thickness = crust.thickness + thickness;
            if new_sediment < 0.0 {
                new_thickness += new_sediment * SEDIMENT_ROCK_DENSITY / crust.density;
            }
            crust.sediment = new_sediment.max(0.0);
            crust.thickness = new_thickness.max(0.0);
            change += crust.mass() - before;
        }
        self.ledger.record(ledger::Term::SurfaceProcesses, change);

        // Sediment displaces water and eroded land no longer holds it back.
        let sea_level = self.solve_sea_level();
        self.sea_level_change.1 += sea_level - self.sea_level;
        self.sea_level = sea_level;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isostasy::{ARC_DENSITY, CONTINENTAL_DENSITY, OCEANIC_DENSITY};

    fn simulation(seed: u64) -> Simulation {
        Simulation::new(SphereMesh::icosphere(4), 8, TectonicParams::default(), seed)
    }

    #[test]
    fn the_ocean_holds_its_volume() {
        let mut simulation = simulation(1);
        let cell_area = 4.0 * std::f64::consts::PI * simulation.params.planet_radius.powi(2)
            / simulation.mesh.len() as f64;
        let check = |simulation: &Simulation| {
            let volume = crate::isostasy::ocean_volume(&simulation.surface_elevations(), cell_area);
            let target = simulation.params.ocean_volume;
            assert!((volume / target - 1.0).abs() < 1e-9, "{volume} vs {target}");
        };
        check(&simulation);
        for _ in 0..5 {
            simulation.step(1.0);
            check(&simulation);
            // Sediment on the sea floor displaces water.
            let cells = simulation.mesh.len();
            let sediment: Vec<f64> = (0..cells as u32)
                .map(|c| {
                    if simulation.surface_elevation(c) < 0.0 {
                        50.0
                    } else {
                        0.0
                    }
                })
                .collect();
            let before = simulation.sea_level;
            simulation.apply_surface_changes(&vec![0.0; cells], &sediment);
            check(&simulation);
            assert!(simulation.sea_level > before);
        }
    }

    #[test]
    fn isostasy_gives_earth_like_elevations() {
        let ocean = |age: f64| new_oceanic_crust(-age);
        assert!((ocean(0.0).surface_elevation(0.0, 0.0) + 2600.0).abs() < 1.0);
        assert!((ocean(1000.0).surface_elevation(0.0, 0.0) + 5651.0).abs() < 1.0);
        assert!(!ocean(0.0).is_continental());

        let continent = |thickness| Crust {
            thickness,
            density: CONTINENTAL_DENSITY,
            birth: -1000.0,
            sediment: 0.0,
        };
        // Normal continental crust stands a few hundred metres above sea level, doubly
        // thickened crust forms high plateaus like Tibet, thinned crust forms shelves.
        assert!((200.0..900.0).contains(&continent(35_000.0).surface_elevation(0.0, 0.0)));
        assert!((4500.0..5500.0).contains(&continent(70_000.0).surface_elevation(0.0, 0.0)));
        assert!((-500.0..0.0).contains(&continent(30_000.0).surface_elevation(0.0, 0.0)));
        assert!(continent(30_000.0).is_continental());
    }

    #[test]
    fn elevation_rises_with_thickness_at_any_age() {
        for age in [0.0, 1.0, 5.0, 20.0, 1000.0] {
            for density in [OCEANIC_DENSITY, ARC_DENSITY, CONTINENTAL_DENSITY] {
                let mut previous = f64::NEG_INFINITY;
                let mut thickness = 5000.0;
                while thickness <= 75_000.0 {
                    let crust = Crust {
                        thickness,
                        density,
                        birth: -age,
                        sediment: 0.0,
                    };
                    let elevation = crust.dry_elevation(0.0);
                    // At least 0.5 m per 100 m of crust (the thermal uplift fading out over the
                    // continental transition leaves a margin of ~1 m per 100 m on young
                    // lithosphere).
                    assert!(
                        elevation > previous + 0.5,
                        "age {age}, density {density}, {thickness} m: {elevation} after {previous}"
                    );
                    previous = elevation;
                    thickness += 100.0;
                }
            }
        }
    }

    #[test]
    fn young_arcs_override_young_ocean() {
        for age in [0.0, 1.0, 5.0] {
            for thickness in [15_000.0, 20_000.0] {
                let arc = Crust {
                    thickness,
                    density: ARC_DENSITY,
                    birth: -age,
                    sediment: 0.0,
                };
                assert!(arc.buoyancy(0.0) > new_oceanic_crust(0.0).buoyancy(0.0));
            }
        }
    }

    #[test]
    fn continental_crust_gets_no_oceanic_thermal_uplift() {
        let continent = |age: f64| Crust {
            thickness: 40_000.0,
            density: CONTINENTAL_DENSITY,
            birth: -age,
            sediment: 0.0,
        };
        assert!(
            (continent(0.0).surface_elevation(0.0, 0.0)
                - continent(1000.0).surface_elevation(0.0, 0.0))
            .abs()
                < 1.0
        );
    }

    #[test]
    fn erosion_response_matches_the_elevation_formula() {
        for thickness in [30_000.0, 49_000.0, 60_000.0, 74_000.0] {
            let crust = Crust {
                thickness,
                density: CONTINENTAL_DENSITY,
                birth: -1000.0,
                sediment: 0.0,
            };
            let removed = 10.0;
            let thinner = Crust {
                thickness: thickness - removed,
                ..crust
            };
            let expected = thinner.surface_elevation(0.0, 0.0);
            let predicted = crate::isostasy::add_layer(
                crust.surface_elevation(0.0, 0.0),
                -removed,
                crust.removal_isostatic_density(),
            );
            assert!(
                (predicted - expected).abs() < 0.01,
                "{thickness}: {predicted} vs {expected}"
            );
        }
    }

    #[test]
    fn thinned_continental_crust_is_not_ocean() {
        let eroded = Crust {
            thickness: OCEANIC_THICKNESS,
            density: CONTINENTAL_DENSITY,
            birth: -1000.0,
            sediment: 0.0,
        };
        assert!(!eroded.is_oceanic());
        assert!(!eroded.is_continental());
        assert!(new_oceanic_crust(0.0).is_oceanic());
    }

    #[test]
    fn arcs_mature_into_continental_crust() {
        let mut crust = new_oceanic_crust(-80.0);
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
        assert!(crust.surface_elevation(0.0, 0.0) > 0.0);
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
        assert!((statistics.continental_fraction - 0.4).abs() < 0.01);
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
                .any(|cell| simulation.surface_crust(cell).age(simulation.time()) < 10.0)
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

    #[test]
    fn crust_mass_ledger_closes() {
        let mut simulation = simulation(3);
        let total = simulation.total_mass();
        let assert_closes = |simulation: &Simulation, context: &str| {
            let residual = simulation.ledger.checked(simulation.total_mass()).residual;
            assert!(
                residual.abs() < 1e-9 * total,
                "{context}: residual {residual} of total {total}"
            );
        };
        for step in 0..40 {
            simulation.step(1.0);
            assert_closes(&simulation, &format!("step {step}"));
            // Erosion-like changes, including a sample shown at several cells.
            let cells = simulation.mesh.len();
            let thickness: Vec<f64> = (0..cells).map(|c| -((c % 7) as f64)).collect();
            let sediment: Vec<f64> = (0..cells).map(|c| (c % 5) as f64 - 2.0).collect();
            simulation.apply_surface_changes(&thickness, &sediment);
            assert_closes(&simulation, &format!("surface changes {step}"));
        }
        // Plates moved and things happened.
        let ledger = simulation.ledger.checked(simulation.total_mass());
        assert!(ledger.new_sea_floor > 0.0);
        assert!(ledger.subducted_oceanic < 0.0);
    }

    #[test]
    fn shared_samples_get_the_sum_of_their_changes() {
        let mut simulation = simulation(4);
        // Show the sample of cell 0 also at its first neighbour.
        let neighbor = simulation.mesh.neighbors(0)[0];
        simulation.owners[neighbor as usize] = simulation.owners[0];
        let before = simulation.surface_crust(0);
        let mut thickness = vec![0.0; simulation.mesh.len()];
        thickness[0] = -10.0;
        thickness[neighbor as usize] = -20.0;
        simulation.apply_surface_changes(&thickness, &vec![0.0; simulation.mesh.len()]);
        assert!((simulation.surface_crust(0).thickness - (before.thickness - 30.0)).abs() < 1e-9);
    }
}
