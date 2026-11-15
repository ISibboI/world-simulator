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
//! * If several plates cover a cell, they converge there. One plate overrides, the crust of the
//!   others is destroyed (subduction) and the overriding plate is uplifted around the cell
//!   (volcanic arcs, orogeny).
//!
//! Which plate overrides is decided per pair of plates when they first touch, from the buoyancy of
//! their crust (continental before oceanic, young oceanic before old oceanic), and then kept, so
//! that every boundary has a consistent subduction polarity. It flips when continental crust
//! arrives at the trench, since continents cannot subduct under oceanic crust. Colliding
//! continents drag their plates towards a common motion, and plates occasionally get a new
//! random motion (plate reorganisation), which keeps the system from settling down.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, VecDeque};

use glam::{DQuat, DVec3};
use rand::rngs::ChaCha8Rng;
use rand::{Rng, RngExt, SeedableRng};

use crate::sphere::SphereMesh;

/// Physical parameters of the simulation. Times are in Myr, elevations in metres.
#[derive(Clone, Debug)]
pub struct TectonicParams {
    /// Range of plate angular speeds in rad/Myr. 0.01 rad/Myr is about 6.4 cm/yr on Earth.
    pub angular_speed_range: (f64, f64),
    /// The fraction of the surface initially covered by continental crust.
    pub continental_fraction: f64,
    /// The number of initial continents.
    pub continent_count: usize,
    /// Elevation that continents erode towards.
    pub continental_base_elevation: f64,
    /// Time constant of continental erosion towards [`Self::continental_base_elevation`].
    pub erosion_time: f64,
    /// Time constant of oceanic crust relaxing towards its age-dependent depth.
    pub oceanic_relaxation_time: f64,
    /// Uplift of the overriding plate per subducted crust sample.
    pub subduction_uplift: f64,
    /// Uplift of the overriding plate per consumed continental crust sample.
    pub collision_uplift: f64,
    /// Radius (in cells) over which uplift is distributed.
    pub uplift_radius: u32,
    /// Oceanic crust that is uplifted above this elevation turns into continental crust
    /// (accretion of island arcs).
    pub accretion_elevation: f64,
    /// Uplift saturates when approaching this elevation.
    pub max_elevation: f64,
    /// Fraction of the velocity difference of two plates that is removed per continental
    /// collision event between them.
    pub collision_drag: f64,
    /// Mean time between random changes of the motion of a plate.
    pub reorganization_interval: f64,
    /// Subduction polarity between two plates is forgotten after they were not in contact for
    /// this long.
    pub polarity_memory: f64,
    /// Maximum fraction of a cell spacing that a plate may move in one substep.
    pub max_substep_movement: f64,
}

impl Default for TectonicParams {
    fn default() -> Self {
        Self {
            angular_speed_range: (0.004, 0.016),
            continental_fraction: 0.3,
            continent_count: 7,
            continental_base_elevation: 300.0,
            erosion_time: 40.0,
            oceanic_relaxation_time: 20.0,
            subduction_uplift: 150.0,
            collision_uplift: 300.0,
            uplift_radius: 3,
            accretion_elevation: -200.0,
            max_elevation: 9000.0,
            collision_drag: 0.002,
            reorganization_interval: 150.0,
            polarity_memory: 20.0,
            max_substep_movement: 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrustKind {
    Oceanic,
    Continental,
}

/// A sample of crust carried by a plate.
#[derive(Clone, Copy, Debug)]
pub struct Crust {
    pub kind: CrustKind,
    /// Age in Myr.
    pub age: f64,
    /// Elevation relative to sea level in metres.
    pub elevation: f64,
}

impl Crust {
    /// Crust with higher buoyancy overrides crust with lower buoyancy at convergent boundaries.
    fn buoyancy(&self) -> f64 {
        match self.kind {
            CrustKind::Continental => 1e9 + self.elevation,
            CrustKind::Oceanic => -self.age,
        }
    }
}

/// Depth of oceanic crust depending on its age (after Parsons & Sclater, 1977).
///
/// Young crust follows the half-space cooling model, old crust flattens out. Taking the
/// shallower of both curves keeps the depth continuous and monotonic.
pub fn oceanic_depth(age: f64) -> f64 {
    let age = age.max(0.0);
    let half_space_cooling = 2500.0 + 350.0 * age.sqrt();
    let plate_model = 6400.0 - 3200.0 * (-age / 62.8).exp();
    -half_space_cooling.min(plate_model)
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ownership {
    pub plate: u32,
    pub local: u32,
}

/// Which plate of a pair overrides the other at their common boundary.
#[derive(Clone, Copy, Debug)]
struct Polarity {
    overriding: u32,
    last_contact: f64,
}

pub struct Simulation {
    mesh: SphereMesh,
    params: TectonicParams,
    plates: Vec<Plate>,
    owners: Vec<Ownership>,
    /// Subduction polarity of plate pairs `(a, b)` with `a < b`.
    polarities: HashMap<(u32, u32), Polarity>,
    rng: ChaCha8Rng,
    /// Simulated time in Myr.
    time: f64,
}

/// Aggregate statistics of the current simulation state.
#[derive(Clone, Debug)]
pub struct Statistics {
    pub time: f64,
    pub continental_fraction: f64,
    pub land_fraction: f64,
    pub mean_oceanic_age: f64,
    pub max_elevation: f64,
    pub min_elevation: f64,
    /// Mean surface speed in rad/Myr (multiply by the planet radius for a linear speed).
    pub mean_speed: f64,
    /// Number of cells covered by each plate.
    pub plate_areas: Vec<usize>,
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
                angular_velocity: random_angular_velocity(&params, rng),
                crust: vec![None; cell_count],
                lookup_cache: (0..cell_count as u32).collect(),
            })
            .collect();

        for cell in 0..cell_count {
            let crust = if is_continental[cell] {
                let elevation = params.continental_base_elevation
                    + 150.0 * f64::from(coast_distance[cell]).min(6.0)
                    + rng.random_range(-100.0..100.0);
                Crust {
                    kind: CrustKind::Continental,
                    age: 1000.0,
                    elevation,
                }
            } else {
                let age = (10.0 + 8.0 * f64::from(boundary_distance[cell])).min(180.0)
                    + rng.random_range(0.0..5.0);
                Crust {
                    kind: CrustKind::Oceanic,
                    age,
                    elevation: oceanic_depth(age),
                }
            };
            plates[plate_of_cell[cell] as usize].crust[cell] = Some(crust);
        }

        let owners = (0..cell_count as u32)
            .map(|cell| Ownership {
                plate: plate_of_cell[cell as usize],
                local: cell,
            })
            .collect();

        Self {
            mesh,
            params,
            plates,
            owners,
            polarities: HashMap::new(),
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
        self.reorganize_plates(dt);
        let max_angle = self
            .plates
            .iter()
            .map(|plate| plate.angular_velocity.length() * dt)
            .fold(0.0, f64::max);
        let max_substep_angle = self.params.max_substep_movement * self.mesh.mean_edge_angle();
        let substeps = (max_angle / max_substep_angle).ceil().max(1.0) as usize;
        for _ in 0..substeps {
            self.substep(dt / substeps as f64);
        }
    }

    fn substep(&mut self, dt: f64) {
        self.time += dt;
        self.move_plates(dt);
        self.evolve_crust(dt);
        let ResolvedBoundaries {
            uplift_events,
            collisions,
        } = self.resolve_ownership();
        self.apply_uplift(&uplift_events);
        self.apply_collision_drag(&collisions);
    }

    /// Gives each plate a new random motion with probability `dt / reorganization_interval`.
    fn reorganize_plates(&mut self, dt: f64) {
        let probability = (dt / self.params.reorganization_interval).clamp(0.0, 1.0);
        for plate in &mut self.plates {
            if self.rng.random_bool(probability) {
                plate.angular_velocity = random_angular_velocity(&self.params, &mut self.rng);
            }
        }
    }

    fn move_plates(&mut self, dt: f64) {
        for plate in &mut self.plates {
            let delta = DQuat::from_scaled_axis(plate.angular_velocity * dt);
            plate.rotation = (delta * plate.rotation).normalize();
        }
    }

    /// Continental collisions slow down the convergence of the colliding plates by moving their
    /// angular velocities towards each other, conserving their area-weighted sum.
    fn apply_collision_drag(&mut self, collisions: &HashMap<(u32, u32), u32>) {
        if collisions.is_empty() {
            return;
        }
        let mut areas = vec![0usize; self.plates.len()];
        for owner in &self.owners {
            areas[owner.plate as usize] += 1;
        }
        // Iterate in a fixed order for determinism.
        let mut collisions: Vec<_> = collisions.iter().collect();
        collisions.sort_unstable();
        for (&(a, b), &count) in collisions {
            let (area_a, area_b) = (areas[a as usize] as f64, areas[b as usize] as f64);
            if area_a + area_b == 0.0 {
                continue;
            }
            let strength = (self.params.collision_drag * f64::from(count)).min(1.0);
            let difference =
                self.plates[b as usize].angular_velocity - self.plates[a as usize].angular_velocity;
            self.plates[a as usize].angular_velocity +=
                difference * strength * area_b / (area_a + area_b);
            self.plates[b as usize].angular_velocity -=
                difference * strength * area_a / (area_a + area_b);
        }
    }

    /// Ages all crust and lets its elevation relax (erosion, thermal subsidence of ocean floor).
    fn evolve_crust(&mut self, dt: f64) {
        let params = &self.params;
        let erosion = 1.0 - (-dt / params.erosion_time).exp();
        let relaxation = 1.0 - (-dt / params.oceanic_relaxation_time).exp();
        for plate in &mut self.plates {
            for crust in plate.crust.iter_mut().flatten() {
                crust.age += dt;
                match crust.kind {
                    CrustKind::Continental => {
                        crust.elevation +=
                            (params.continental_base_elevation - crust.elevation) * erosion;
                    }
                    CrustKind::Oceanic => {
                        crust.elevation +=
                            (oceanic_depth(crust.age) - crust.elevation) * relaxation;
                    }
                }
            }
        }
    }

    /// Assigns every world cell to a plate, subducting crust at convergent boundaries and creating
    /// crust at divergent boundaries.
    fn resolve_ownership(&mut self) -> ResolvedBoundaries {
        let Self {
            mesh,
            params,
            plates,
            owners,
            polarities,
            time,
            ..
        } = self;
        polarities.retain(|_, polarity| *time - polarity.last_contact <= params.polarity_memory);
        let cell_count = mesh.len();
        let inverse_rotations: Vec<DQuat> = plates.iter().map(|p| p.rotation.inverse()).collect();
        let alive: Vec<bool> = plates
            .iter()
            .map(|p| p.crust.iter().any(Option::is_some))
            .collect();

        let mut new_owners: Vec<Option<Ownership>> = vec![None; cell_count];
        let mut subducted = Vec::new();
        let mut uplift_events = Vec::new();
        let mut collisions: HashMap<(u32, u32), u32> = HashMap::new();
        let mut candidates: Vec<(Ownership, Crust)> = Vec::with_capacity(plates.len());

        for cell in 0..cell_count as u32 {
            candidates.clear();
            for (index, plate) in plates.iter_mut().enumerate() {
                if !alive[index] {
                    continue;
                }
                let local = plate.local_vertex(mesh, inverse_rotations[index], cell);
                if let Some(crust) = plate.crust[local as usize] {
                    let plate = index as u32;
                    candidates.push((Ownership { plate, local }, crust));
                }
            }

            let Some((winner, winner_crust)) = choose_overriding(&candidates, polarities, *time)
            else {
                continue;
            };
            new_owners[cell as usize] = Some(winner);

            for &(loser, loser_crust) in &candidates {
                if loser == winner {
                    continue;
                }
                subducted.push(loser);
                let amount = match (winner_crust.kind, loser_crust.kind) {
                    (CrustKind::Continental, CrustKind::Continental) => {
                        *collisions
                            .entry(pair_key(winner.plate, loser.plate))
                            .or_default() += 1;
                        params.collision_uplift
                    }
                    _ => params.subduction_uplift,
                };
                uplift_events.push(UpliftEvent {
                    cell,
                    plate: winner.plate,
                    amount,
                });
            }
        }

        for ownership in subducted {
            plates[ownership.plate as usize].crust[ownership.local as usize] = None;
        }

        // A sample may have been subducted at one cell while owning another; that cell is now free.
        for owner in &mut new_owners {
            if let Some(ownership) = owner
                && plates[ownership.plate as usize].crust[ownership.local as usize].is_none()
            {
                *owner = None;
            }
        }

        // Single-cell holes inside a plate are resampling artifacts rather than rifts, so they are
        // filled with a copy of the neighbouring crust.
        let holes: Vec<(u32, Ownership)> = (0..cell_count as u32)
            .filter(|&cell| new_owners[cell as usize].is_none())
            .filter_map(|cell| {
                let mut neighbors = mesh.neighbors(cell).iter().map(|&n| new_owners[n as usize]);
                let first = neighbors.next()??;
                neighbors
                    .all(|neighbor| neighbor.is_some_and(|n| n.plate == first.plate))
                    .then_some((cell, first))
            })
            .collect();
        for (cell, template) in holes {
            let plate_index = template.plate as usize;
            let plate = &mut plates[plate_index];
            let template_crust =
                plate.crust[template.local as usize].expect("owners point to existing crust");
            let local = plate.local_vertex(mesh, inverse_rotations[plate_index], cell);
            plate.crust[local as usize].get_or_insert(template_crust);
            new_owners[cell as usize] = Some(Ownership {
                plate: template.plate,
                local,
            });
        }

        // Fill the remaining gaps with new oceanic crust, growing the adjacent plates layer by
        // layer.
        let mut queue: VecDeque<u32> = (0..cell_count as u32)
            .filter(|&cell| new_owners[cell as usize].is_some())
            .collect();
        if queue.is_empty() {
            // All crust was destroyed, which can only happen in degenerate configurations.
            // Restart with the first plate covering the first cell.
            plates[0].crust[0] = Some(new_oceanic_crust());
            new_owners[0] = Some(Ownership { plate: 0, local: 0 });
            queue.push_back(0);
        }
        while let Some(cell) = queue.pop_front() {
            let plate_index = new_owners[cell as usize]
                .expect("queued cells are owned")
                .plate;
            for &neighbor in mesh.neighbors(cell) {
                if new_owners[neighbor as usize].is_some() {
                    continue;
                }
                let plate = &mut plates[plate_index as usize];
                let local =
                    plate.local_vertex(mesh, inverse_rotations[plate_index as usize], neighbor);
                plate.crust[local as usize].get_or_insert_with(new_oceanic_crust);
                new_owners[neighbor as usize] = Some(Ownership {
                    plate: plate_index,
                    local,
                });
                queue.push_back(neighbor);
            }
        }

        for (owner, new_owner) in owners.iter_mut().zip(new_owners) {
            *owner = new_owner.expect("all cells are owned after gap filling");
        }
        ResolvedBoundaries {
            uplift_events,
            collisions,
        }
    }

    /// Distributes uplift over the cells of the overriding plate around each event, and turns
    /// sufficiently uplifted oceanic crust into continental crust.
    fn apply_uplift(&mut self, events: &[UpliftEvent]) {
        let radius = self.params.uplift_radius;
        let mut visited_in_event = vec![usize::MAX; self.mesh.len()];
        let mut frontier = Vec::new();
        let mut next_frontier = Vec::new();

        for (event_index, event) in events.iter().enumerate() {
            frontier.clear();
            frontier.push(event.cell);
            visited_in_event[event.cell as usize] = event_index;

            for distance in 0..=radius {
                let falloff = 1.0 - f64::from(distance) / f64::from(radius + 1);
                next_frontier.clear();
                for &cell in &frontier {
                    let owner = self.owners[cell as usize];
                    if owner.plate != event.plate {
                        continue;
                    }
                    let crust = self.plates[owner.plate as usize].crust[owner.local as usize]
                        .as_mut()
                        .expect("owners always point to existing crust");
                    let headroom = (1.0 - crust.elevation / self.params.max_elevation).max(0.0);
                    crust.elevation += event.amount * falloff * headroom;
                    if crust.kind == CrustKind::Oceanic
                        && crust.elevation > self.params.accretion_elevation
                    {
                        crust.kind = CrustKind::Continental;
                    }

                    for &neighbor in self.mesh.neighbors(cell) {
                        if visited_in_event[neighbor as usize] != event_index {
                            visited_in_event[neighbor as usize] = event_index;
                            next_frontier.push(neighbor);
                        }
                    }
                }
                std::mem::swap(&mut frontier, &mut next_frontier);
            }
        }
    }

    pub fn statistics(&self) -> Statistics {
        let cell_count = self.mesh.len();
        let mut plate_areas = vec![0; self.plates.len()];
        let mut continental = 0;
        let mut land = 0;
        let mut oceanic_age_sum = 0.0;
        let mut max_elevation = f64::NEG_INFINITY;
        let mut min_elevation = f64::INFINITY;
        let mut speed_sum = 0.0;

        for cell in 0..cell_count as u32 {
            let plate = self.owner(cell).plate as usize;
            plate_areas[plate] += 1;
            speed_sum += self.plates[plate]
                .angular_velocity
                .cross(self.mesh.vertex(cell))
                .length();
            let crust = self.surface_crust(cell);
            match crust.kind {
                CrustKind::Continental => continental += 1,
                CrustKind::Oceanic => oceanic_age_sum += crust.age,
            }
            if crust.elevation > 0.0 {
                land += 1;
            }
            max_elevation = max_elevation.max(crust.elevation);
            min_elevation = min_elevation.min(crust.elevation);
        }

        let oceanic = cell_count - continental;
        Statistics {
            time: self.time,
            continental_fraction: continental as f64 / cell_count as f64,
            land_fraction: land as f64 / cell_count as f64,
            mean_oceanic_age: if oceanic > 0 {
                oceanic_age_sum / oceanic as f64
            } else {
                0.0
            },
            max_elevation,
            min_elevation,
            mean_speed: speed_sum / cell_count as f64,
            plate_areas,
        }
    }
}

struct ResolvedBoundaries {
    uplift_events: Vec<UpliftEvent>,
    /// Number of continental collision events per plate pair.
    collisions: HashMap<(u32, u32), u32>,
}

struct UpliftEvent {
    cell: u32,
    plate: u32,
    amount: f64,
}

fn new_oceanic_crust() -> Crust {
    Crust {
        kind: CrustKind::Oceanic,
        age: 0.0,
        elevation: oceanic_depth(0.0),
    }
}

fn pair_key(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

/// Chooses the overriding plate among the plates covering a cell, recording and updating the
/// subduction polarity of each pair of plates in contact.
fn choose_overriding(
    candidates: &[(Ownership, Crust)],
    polarities: &mut HashMap<(u32, u32), Polarity>,
    time: f64,
) -> Option<(Ownership, Crust)> {
    for (i, &(a, a_crust)) in candidates.iter().enumerate() {
        for &(b, b_crust) in &candidates[i + 1..] {
            let polarity = polarities
                .entry(pair_key(a.plate, b.plate))
                .or_insert_with(|| Polarity {
                    overriding: if a_crust.buoyancy() >= b_crust.buoyancy() {
                        a.plate
                    } else {
                        b.plate
                    },
                    last_contact: time,
                });
            polarity.last_contact = time;
            let (top, top_crust, bottom, bottom_crust) = if polarity.overriding == a.plate {
                (a, a_crust, b, b_crust)
            } else {
                (b, b_crust, a, a_crust)
            };
            if bottom_crust.kind == CrustKind::Continental && top_crust.kind == CrustKind::Oceanic {
                polarity.overriding = bottom.plate;
            } else {
                polarity.overriding = top.plate;
            }
        }
    }

    // Prefer the plate that overrides all others; with three or more plates the relation may be
    // cyclic, in which case buoyancy decides.
    candidates
        .iter()
        .find(|(a, _)| {
            candidates.iter().all(|(b, _)| {
                a.plate == b.plate || polarities[&pair_key(a.plate, b.plate)].overriding == a.plate
            })
        })
        .or_else(|| {
            candidates
                .iter()
                .max_by(|a, b| a.1.buoyancy().total_cmp(&b.1.buoyancy()))
        })
        .copied()
}

fn random_angular_velocity(params: &TectonicParams, rng: &mut impl Rng) -> DVec3 {
    let (min, max) = params.angular_speed_range;
    random_unit_vector(rng) * rng.random_range(min..=max)
}

fn random_unit_vector(rng: &mut impl Rng) -> DVec3 {
    loop {
        let v = DVec3::new(
            rng.random_range(-1.0..1.0),
            rng.random_range(-1.0..1.0),
            rng.random_range(-1.0..1.0),
        );
        let length_squared = v.length_squared();
        if length_squared > 1e-6 && length_squared <= 1.0 {
            return v / length_squared.sqrt();
        }
    }
}

fn sample_distinct_cells(mesh: &SphereMesh, count: usize, rng: &mut impl Rng) -> Vec<u32> {
    let count = count.min(mesh.len());
    let mut cells = Vec::with_capacity(count);
    while cells.len() < count {
        let cell = rng.random_range(0..mesh.len() as u32);
        if !cells.contains(&cell) {
            cells.push(cell);
        }
    }
    cells
}

/// Grows regions from `seeds` with randomised edge costs, until `limit` cells are assigned.
/// Regions with a larger scale grow faster. Unassigned cells are `u32::MAX`.
fn grow_regions(
    mesh: &SphereMesh,
    seeds: &[u32],
    scales: &[f64],
    limit: usize,
    rng: &mut impl Rng,
) -> Vec<u32> {
    let mut region = vec![u32::MAX; mesh.len()];
    let mut assigned = 0;
    let mut heap = BinaryHeap::new();
    for (index, &seed) in seeds.iter().enumerate() {
        heap.push(Reverse((0u64, seed, index as u32)));
    }

    while let Some(Reverse((cost, cell, index))) = heap.pop() {
        if assigned >= limit {
            break;
        }
        if region[cell as usize] != u32::MAX {
            continue;
        }
        region[cell as usize] = index;
        assigned += 1;
        for &neighbor in mesh.neighbors(cell) {
            if region[neighbor as usize] == u32::MAX {
                let step = rng.random_range(1.0..100.0) / scales[index as usize];
                heap.push(Reverse((cost + step as u64 + 1, neighbor, index)));
            }
        }
    }
    region
}

/// Graph distance (in edges) of every cell to the closest cell satisfying `is_source`.
/// Cells are at distance `u32::MAX` if there is no source.
fn distance_to(mesh: &SphereMesh, is_source: impl Fn(u32) -> bool) -> Vec<u32> {
    let mut distance = vec![u32::MAX; mesh.len()];
    let mut queue = VecDeque::new();
    for cell in 0..mesh.len() as u32 {
        if is_source(cell) {
            distance[cell as usize] = 0;
            queue.push_back(cell);
        }
    }
    while let Some(cell) = queue.pop_front() {
        for &neighbor in mesh.neighbors(cell) {
            if distance[neighbor as usize] == u32::MAX {
                distance[neighbor as usize] = distance[cell as usize] + 1;
                queue.push_back(neighbor);
            }
        }
    }
    distance
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simulation(seed: u64) -> Simulation {
        Simulation::new(SphereMesh::icosphere(4), 8, TectonicParams::default(), seed)
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
