//! Events that change the plate configuration: rifting above mantle plumes, initiation of
//! subduction at old passive margins, suturing of plates after a continental collision, and
//! capture of tiny plates by a neighbour.
//!
//! Subduction between two plates that already share a boundary needs no event: a polarity
//! (which plate would subduct, by buoyancy) exists from their first contact, and the boundary
//! becomes a trench as soon as they converge (see [`super::dynamics`]).

use glam::DVec3;
use rand::RngExt;
use tracing::debug;

use super::dynamics::{BoundaryCell, BoundaryKind};
use super::generation::random_unit_vector;
use super::*;

/// Parameters of plate birth and merging.
#[derive(Clone, Debug)]
pub struct PlateEventParams {
    /// A plume rifts the plate above it after this many Myr.
    pub rift_delay: f64,
    /// Plumes rift plates with continental crust within this angular distance (radians), the
    /// usual setting of plume-related break-up.
    pub rift_continent_radius: f64,
    /// Plumes also rift plates without nearby continental crust that cover at least this
    /// fraction of the surface.
    pub min_oceanic_rift_area: f64,
    /// Neither part of a split plate may cover less than this fraction of the surface.
    pub min_plate_area: f64,
    /// No plates are split while there are this many plates.
    pub max_plates: usize,
    /// Oceanic lithosphere at a passive margin may start to subduct from this age in Myr.
    pub initiation_age: f64,
    /// From this age (Myr) on, the hazard of subduction initiation at a passive margin has its
    /// full value; between [`Self::initiation_age`] and this age it rises quadratically.
    pub initiation_full_age: f64,
    /// Mean waiting time (Myr) until 1000 km of passive margin with lithosphere of at least
    /// [`Self::initiation_full_age`] starts to subduct. The 180 Myr old margins of the
    /// Atlantic have not subducted yet, so it is long.
    pub initiation_waiting_time: f64,
    /// Mean waiting time (Myr) until 1000 km of an existing plate boundary with old oceanic
    /// lithosphere on one side starts to subduct. Plate boundaries are weak zones, so
    /// subduction starts there much more easily than at a passive margin (Stern & Gerya 2018).
    pub boundary_initiation_waiting_time: f64,
    /// At an existing plate boundary, oceanic lithosphere only starts to subduct below oceanic
    /// lithosphere that is at least this much (Myr) younger.
    pub initiation_age_contrast: f64,
    /// Length of slab (m) that a new subduction zone at a passive margin starts with: the
    /// foundering lithosphere that triggers it.
    pub initiation_slab_length: f64,
    /// Plates whose continents have been colliding for this many Myr merge.
    pub suture_time: f64,
    /// Minimum number of colliding boundary cells for a pair of plates to count as colliding.
    pub suture_min_cells: usize,
    /// Plates smaller than this fraction of the surface are captured by a neighbour.
    pub min_plate_area_before_capture: f64,
}

impl Default for PlateEventParams {
    fn default() -> Self {
        Self {
            rift_delay: 20.0,
            rift_continent_radius: 0.17,
            min_oceanic_rift_area: 0.15,
            min_plate_area: 0.01,
            max_plates: 40,
            initiation_age: 80.0,
            initiation_full_age: 180.0,
            initiation_waiting_time: 1000.0,
            boundary_initiation_waiting_time: 150.0,
            initiation_age_contrast: 20.0,
            initiation_slab_length: 150e3,
            suture_time: 30.0,
            suture_min_cells: 10,
            min_plate_area_before_capture: 0.003,
        }
    }
}

/// A place where subduction may start.
#[derive(Clone, Copy, Debug)]
pub(super) enum Initiation {
    /// An oceanic cell at a passive margin.
    PassiveMargin(u32),
    /// The boundary between two plates at a cell.
    Boundary {
        subducting: u32,
        overriding: u32,
        cell: u32,
    },
}

impl PlateEventParams {
    /// Hazard of subduction initiation (per Myr and metre of passive margin) for oceanic
    /// lithosphere of the given age in Myr: zero when young, rising quadratically from
    /// [`Self::initiation_age`] to [`Self::initiation_full_age`].
    pub fn initiation_hazard(&self, age: f64) -> f64 {
        self.hazard(age, self.initiation_waiting_time)
    }

    /// Like [`Self::initiation_hazard`], at an existing plate boundary.
    pub fn boundary_initiation_hazard(&self, age: f64) -> f64 {
        self.hazard(age, self.boundary_initiation_waiting_time)
    }

    fn hazard(&self, age: f64, waiting_time: f64) -> f64 {
        if age < self.initiation_age {
            return 0.0;
        }
        let maturity = ((age - self.initiation_age)
            / (self.initiation_full_age - self.initiation_age).max(1e-9))
        .min(1.0);
        maturity * maturity / (waiting_time * 1e6)
    }
}

/// How many plate events of each kind happened so far.
#[derive(Clone, Debug, Default)]
pub struct EventCounts {
    /// Plates split by plumes.
    pub rifts: usize,
    /// Plates split at passive margins by subduction initiation.
    pub initiations: usize,
    /// Subduction induced at existing plate boundaries without subduction (no new plate).
    pub induced_subductions: usize,
    /// Subduction induced at the overriding side of a trench, which reverses the polarity.
    pub polarity_reversals: usize,
    /// Plates merged after continental collisions.
    pub sutures: usize,
    /// Tiny plates captured by a neighbour.
    pub captures: usize,
    /// Plates that disappeared by subduction.
    pub consumed: usize,
    /// Passive-margin initiations whose split did not happen (a part would be too small).
    pub failed_initiations: usize,
    /// Splits that did not happen because there were already
    /// [`PlateEventParams::max_plates`] plates.
    pub blocked_splits: usize,
    /// Continental crust volume (thickness × cells, m) lost in merges: hidden samples of the
    /// absorbed plate.
    pub merge_lost_continental_volume: f64,
}

impl EventCounts {
    pub fn births(&self) -> usize {
        self.rifts + self.initiations
    }

    pub fn deaths(&self) -> usize {
        self.sutures + self.captures + self.consumed
    }
}

impl Simulation {
    /// Runs the plate events for a time step of `dt` Myr. `boundaries` are the current
    /// [`Self::boundary_cells`] (only their collisions and convergence are used, which do not
    /// depend on the slab lengths).
    pub(super) fn plate_events(&mut self, dt: f64, boundaries: &[BoundaryCell]) {
        let configuration = self.configuration_changes;
        self.suture_colliding_plates(dt, boundaries);
        self.capture_tiny_plates();
        // Splits append plates, so plates from here on are born in this step.
        let first_new_plate = self.plates.len() as u32;
        self.rift_above_plumes();
        if self.configuration_changes == configuration {
            self.initiate_subduction(dt, first_new_plate, boundaries);
        } else {
            let boundaries = self.boundary_cells();
            self.initiate_subduction(dt, first_new_plate, &boundaries);
        }
        self.remove_dead_plates();
    }

    /// Area of each plate as a fraction of the surface.
    fn plate_area_fractions(&self) -> Vec<f64> {
        let mut area = vec![0.0; self.plates.len()];
        for owner in &self.owners {
            area[owner.plate as usize] += 1.0;
        }
        let cells = self.mesh.len() as f64;
        area.iter().map(|a| a / cells).collect()
    }

    /// Splits `plate` along the great circle with the given normal if both parts are large
    /// enough. Returns the new plate on the side the normal points to.
    fn split_if_large_enough(&mut self, plate: u32, normal: DVec3) -> Option<u32> {
        if self.plates.len() >= self.params.events.max_plates {
            self.event_counts.blocked_splits += 1;
            debug!(
                plate,
                time = self.time,
                "Split blocked by the maximum number of plates"
            );
            return None;
        }
        let (mut positive, mut negative) = (0usize, 0usize);
        for cell in 0..self.mesh.len() as u32 {
            if self.owners[cell as usize].plate == plate {
                if self.mesh.vertex(cell).dot(normal) > 0.0 {
                    positive += 1;
                } else {
                    negative += 1;
                }
            }
        }
        let min_cells = self.params.events.min_plate_area * self.mesh.len() as f64;
        if (positive as f64) < min_cells || (negative as f64) < min_cells {
            return None;
        }
        self.split_plate(plate, normal)
    }

    /// Merges `absorbed` into `into` and records the continental crust lost.
    fn merge_and_record(&mut self, into: u32, absorbed: u32) {
        let lost = self.merge_plates(into, absorbed);
        self.event_counts.merge_lost_continental_volume += lost;
    }

    /// A plume that has been rising below a plate with continental crust nearby (or below a
    /// very large plate) for long enough splits it along a great circle through the plume. The
    /// plume's outflow then drives the two parts apart.
    fn rift_above_plumes(&mut self) {
        let params = self.params.events.clone();
        let candidates: Vec<(usize, DVec3)> = self
            .mantle
            .plumes()
            .iter()
            .enumerate()
            .filter(|(_, plume)| !plume.has_rifted && plume.age >= params.rift_delay)
            .map(|(index, plume)| (index, plume.position))
            .collect();

        for (index, position) in candidates {
            let cell = self.mesh.nearest(position);
            let plate = self.owners[cell as usize].plate;
            let continent_nearby = (0..self.mesh.len() as u32).any(|other| {
                self.owners[other as usize].plate == plate
                    && self.mesh.vertex(other).angle_between(position)
                        < params.rift_continent_radius
                    && self.surface_crust(other).is_continental()
            });
            let large = self.plate_area_fractions()[plate as usize] >= params.min_oceanic_rift_area;
            if !continent_nearby && !large {
                continue;
            }
            // A random great circle through the plume.
            let direction = random_unit_vector(&mut self.rng);
            let normal = (direction - position * direction.dot(position)).normalize_or_zero();
            if normal == DVec3::ZERO {
                continue;
            }
            if let Some(new_plate) = self.split_if_large_enough(plate, normal) {
                self.event_counts.rifts += 1;
                debug!(plate, new_plate, time = self.time, "Plume rifted a plate");
            }
            // Each plume rifts at most once, also if the split was not possible.
            self.mantle.mark_rifted(index);
        }
    }

    /// Old oceanic lithosphere may start to sink, at two kinds of places:
    ///
    /// * at a passive margin (ocean and continent on the same plate): the plate splits along
    ///   the margin and the oceanic part subducts below the continental part;
    /// * at an existing plate boundary that is not diverging and where it does not subduct yet
    ///   (transform faults, fracture zones, locked margins, the overriding side of a trench),
    ///   where it meets younger oceanic or continental crust of the other plate: the old side
    ///   starts to subduct below the other plate (induced nucleation, like Izu–Bonin–Mariana at
    ///   a transform fault; at a trench, a polarity reversal: old, dense lithosphere does not
    ///   stay above much younger, buoyant lithosphere).
    ///
    /// Both start with a short slab. Initiation is a random process with a hazard per unit
    /// length that rises with the age of the lithosphere (see
    /// [`PlateEventParams::initiation_waiting_time`]); at most one initiation happens per step,
    /// and not on plates from `first_new_plate` on, which were born in this step. `boundaries`
    /// are the current [`Self::boundary_cells`].
    pub(super) fn initiate_subduction(
        &mut self,
        dt: f64,
        first_new_plate: u32,
        boundaries: &[BoundaryCell],
    ) {
        let candidates = self.initiation_candidates(first_new_plate, boundaries);
        let total: f64 = candidates.iter().map(|(_, hazard)| hazard).sum();
        if total * dt > 0.5 {
            debug!(
                expected = total * dt,
                time = self.time,
                "Subduction initiation saturated: at most one per step"
            );
        }
        if candidates.is_empty() || !self.rng.random_bool(1.0 - (-total * dt).exp()) {
            return;
        }

        let mut pick = self.rng.random_range(0.0..total);
        let initiation = candidates
            .iter()
            .find(|(_, hazard)| {
                pick -= hazard;
                pick <= 0.0
            })
            .map_or(candidates[candidates.len() - 1].0, |(initiation, _)| {
                *initiation
            });
        let cell = match initiation {
            Initiation::PassiveMargin(cell) => cell,
            Initiation::Boundary {
                subducting,
                overriding,
                cell,
            } => {
                self.induce_subduction(subducting, overriding, cell);
                return;
            }
        };

        // The margin runs perpendicular to the direction from the ocean towards the continent.
        let position = self.mesh.vertex(cell);
        let plate = self.owners[cell as usize].plate;
        let towards_continent: DVec3 = self
            .mesh
            .neighbors(cell)
            .iter()
            .filter(|&&neighbor| {
                self.owners[neighbor as usize].plate == plate
                    && self.surface_crust(neighbor).is_continental()
            })
            .map(|&neighbor| self.mesh.vertex(neighbor) - position)
            .sum();
        let normal =
            (towards_continent - position * towards_continent.dot(position)).normalize_or_zero();
        if normal == DVec3::ZERO {
            return;
        }
        // The new plate is the continental side; the oceanic side subducts below it.
        if let Some(continental_plate) = self.split_if_large_enough(plate, normal) {
            self.event_counts.initiations += 1;
            self.seed_margin_subduction(plate, continental_plate, cell);
            debug!(
                oceanic_plate = plate,
                continental_plate,
                time = self.time,
                "Subduction started at a passive margin"
            );
        } else {
            self.event_counts.failed_initiations += 1;
        }
    }

    /// Starts subduction of `oceanic_plate` below `continental_plate` along their new common
    /// boundary where old oceanic crust faces continental crust, and at `cell`.
    fn seed_margin_subduction(&mut self, oceanic_plate: u32, continental_plate: u32, cell: u32) {
        let params = &self.params.events;
        let (slab_length, min_age) = (params.initiation_slab_length, params.initiation_age);
        let rotations = self.rotations();
        let pair = (oceanic_plate, continental_plate);
        let mut seeds = vec![cell];
        for candidate in 0..self.mesh.len() as u32 {
            if self.owners[candidate as usize].plate != oceanic_plate {
                continue;
            }
            let crust = self.surface_crust(candidate);
            if !crust.is_oceanic() || crust.age(self.time) < min_age {
                continue;
            }
            if self.mesh.neighbors(candidate).iter().any(|&neighbor| {
                self.owners[neighbor as usize].plate == continental_plate
                    && self.surface_crust(neighbor).is_continental()
            }) {
                seeds.push(candidate);
            }
        }
        for seed in seeds {
            let r = self.mesh.vertex(seed);
            if self
                .polarities
                .segment_at(pair.0, pair.1, r, &rotations)
                .is_none()
            {
                let subduction = polarity::Subduction {
                    overriding: continental_plate,
                    slab_length,
                    initiation: true,
                };
                self.polarities
                    .set(pair, subduction, r, self.time, &rotations);
            }
        }
        self.configuration_changes += 1;
    }

    /// Places where subduction may start (see [`Self::initiate_subduction`]) with their hazard
    /// per Myr.
    pub(super) fn initiation_candidates(
        &self,
        first_new_plate: u32,
        boundaries: &[BoundaryCell],
    ) -> Vec<(Initiation, f64)> {
        let params = &self.params.events;
        let margin_length = self.boundary_length_per_cell();
        let min_convergence =
            self.params.min_trench_convergence / (self.params.planet_radius * 100.0 / 1e6);
        // Candidate places with their hazard (per Myr).
        let mut candidates: Vec<(Initiation, f64)> = Vec::new();
        // Old oceanic cells next to continental crust of the same plate.
        for cell in 0..self.mesh.len() as u32 {
            let owner = self.owners[cell as usize];
            if owner.plate >= first_new_plate {
                continue;
            }
            let crust = self.surface_crust(cell);
            let age = crust.age(self.time);
            if !crust.is_oceanic() || age < params.initiation_age {
                continue;
            }
            let at_margin = self.mesh.neighbors(cell).iter().any(|&neighbor| {
                self.owners[neighbor as usize].plate == owner.plate
                    && self.surface_crust(neighbor).is_continental()
            });
            if at_margin {
                candidates.push((
                    Initiation::PassiveMargin(cell),
                    params.initiation_hazard(age) * margin_length,
                ));
            }
        }
        // Old oceanic boundary cells facing younger or continental crust without subduction.
        for boundary in boundaries {
            if boundary.kind != BoundaryKind::Other
                || boundary.convergence <= -min_convergence
                || boundary.plate >= first_new_plate
                || boundary.other >= first_new_plate
                || boundary
                    .segment
                    .and_then(|id| self.polarities.segment(boundary.plate, boundary.other, id))
                    .is_some_and(|segment| {
                        segment.slab_length > 0.0 && segment.overriding == boundary.other
                    })
            {
                // Not a candidate, or the plate already subducts below the other one.
                continue;
            }
            let crust = self.surface_crust(boundary.cell);
            let age = crust.age(self.time);
            if !crust.is_oceanic() || age < params.initiation_age {
                continue;
            }
            let Some(&neighbor) = self
                .mesh
                .neighbors(boundary.cell)
                .iter()
                .find(|&&neighbor| self.owners[neighbor as usize].plate == boundary.other)
            else {
                continue;
            };
            let other_crust = self.surface_crust(neighbor);
            if other_crust.is_continental()
                || (other_crust.is_oceanic()
                    && other_crust.age(self.time) + params.initiation_age_contrast <= age)
            {
                candidates.push((
                    Initiation::Boundary {
                        subducting: boundary.plate,
                        overriding: boundary.other,
                        cell: boundary.cell,
                    },
                    params.boundary_initiation_hazard(age) * margin_length,
                ));
            }
        }
        candidates
    }

    /// Starts subduction of `subducting` below `overriding` in the boundary segment at `cell`
    /// with a short slab. If `overriding` subducted below `subducting` there so far, the
    /// polarity reverses.
    fn induce_subduction(&mut self, subducting: u32, overriding: u32, cell: u32) {
        let slab_length = self.params.events.initiation_slab_length;
        let time = self.time;
        let rotations = self.rotations();
        let subduction = polarity::Subduction {
            overriding,
            slab_length,
            initiation: true,
        };
        let reversal = self.polarities.set(
            (subducting, overriding),
            subduction,
            self.mesh.vertex(cell),
            time,
            &rotations,
        );
        self.configuration_changes += 1;
        if reversal {
            self.event_counts.polarity_reversals += 1;
        } else {
            self.event_counts.induced_subductions += 1;
        }
        debug!(
            subducting,
            overriding, time, reversal, "Subduction induced at a plate boundary"
        );
    }

    /// Tracks how long pairs of plates have been in continental collision, and merges them once
    /// the collision has lasted long enough. Contacts count unless the plates clearly move
    /// apart (collision resistance drives locked collisions to near-zero convergence).
    fn suture_colliding_plates(&mut self, dt: f64, boundaries: &[BoundaryCell]) {
        let min_convergence =
            self.params.min_trench_convergence / (self.params.planet_radius * 100.0 / 1e6);
        let mut colliding: HashMap<(u32, u32), usize> = HashMap::default();
        for boundary in boundaries {
            if boundary.kind == BoundaryKind::Collision && boundary.convergence > -min_convergence {
                *colliding
                    .entry(pair_key(boundary.plate, boundary.other))
                    .or_default() += 1;
            }
        }
        let min_cells = self.params.events.suture_min_cells;
        for (pair, state) in &mut self.polarities.pairs {
            if colliding.get(pair).copied().unwrap_or(0) >= min_cells {
                state.collision_time += dt;
            } else {
                state.collision_time = (state.collision_time - dt).max(0.0);
            }
        }

        let suture_time = self.params.events.suture_time;
        // Merge one pair at a time, since merging renumbers plates.
        while let Some(&(a, b)) = self
            .polarities
            .pairs
            .iter()
            .filter(|(_, state)| state.collision_time >= suture_time)
            .map(|(pair, _)| pair)
            .min()
        {
            let area = self.plate_area_fractions();
            let (into, absorbed) = if area[a as usize] >= area[b as usize] {
                (a, b)
            } else {
                (b, a)
            };
            debug!(into, absorbed, time = self.time, "Plates sutured");
            self.merge_and_record(into, absorbed);
            self.event_counts.sutures += 1;
        }
    }

    /// Tiny plates are captured by the neighbour they move most similarly to (like fragments of
    /// the Farallon plate joining the Pacific plate), the smallest first.
    fn capture_tiny_plates(&mut self) {
        let min_area = self.params.events.min_plate_area_before_capture;
        loop {
            let area = self.plate_area_fractions();
            let Some(tiny) = (0..self.plates.len())
                .filter(|&plate| area[plate] > 0.0 && area[plate] < min_area)
                .min_by(|&a, &b| area[a].total_cmp(&area[b]).then(a.cmp(&b)))
            else {
                return;
            };
            let tiny = tiny as u32;
            // Shared boundary length and summed relative speed per neighbour.
            let mut neighbours: HashMap<u32, (usize, f64)> = HashMap::default();
            for cell in 0..self.mesh.len() as u32 {
                if self.owners[cell as usize].plate != tiny {
                    continue;
                }
                let r = self.mesh.vertex(cell);
                for &neighbor in self.mesh.neighbors(cell) {
                    let other = self.owners[neighbor as usize].plate;
                    if other != tiny {
                        let relative = (self.plates[tiny as usize].angular_velocity
                            - self.plates[other as usize].angular_velocity)
                            .cross(r)
                            .length();
                        let entry = neighbours.entry(other).or_default();
                        entry.0 += 1;
                        entry.1 += relative;
                    }
                }
            }
            let Some((&into, _)) = neighbours.iter().min_by(|a, b| {
                let mean_a = a.1.1 / a.1.0 as f64;
                let mean_b = b.1.1 / b.1.0 as f64;
                mean_a
                    .total_cmp(&mean_b)
                    .then(b.1.0.cmp(&a.1.0))
                    .then(a.0.cmp(b.0))
            }) else {
                // A plate without neighbours covers the whole planet.
                return;
            };
            debug!(into, tiny, time = self.time, "Tiny plate captured");
            self.merge_and_record(into, tiny);
            self.event_counts.captures += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isostasy::CONTINENTAL_DENSITY;

    /// One plate: continental where x > 0.3, ocean of the given age elsewhere.
    fn passive_margin(params: TectonicParams, ocean_age: f64) -> Simulation {
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 1, params, 2);
        for (local, crust) in simulation.plates[0].crust.iter_mut().enumerate() {
            let x = simulation.mesh.vertex(local as u32).x;
            *crust = Some(if x > 0.3 {
                Crust {
                    thickness: 35_000.0,
                    density: CONTINENTAL_DENSITY,
                    birth: -1000.0,
                    sediment: 0.0,
                }
            } else {
                new_oceanic_crust(-ocean_age)
            });
        }
        simulation
    }

    #[test]
    fn initiation_hazard_rises_with_age() {
        let params = PlateEventParams::default();
        assert_eq!(params.initiation_hazard(params.initiation_age - 1.0), 0.0);
        let mut previous = 0.0;
        for age in [90.0, 120.0, 150.0, 179.0] {
            let hazard = params.initiation_hazard(age);
            assert!(hazard > previous, "{age}");
            previous = hazard;
        }
        assert_eq!(
            params.initiation_hazard(180.0),
            params.initiation_hazard(400.0)
        );
        // 1000 km of old margin waits on average the waiting time.
        let waiting = 1.0 / (params.initiation_hazard(400.0) * 1e6);
        assert!((waiting - params.initiation_waiting_time).abs() < 1e-9);
    }

    #[test]
    fn young_passive_margins_do_not_subduct() {
        let mut params = TectonicParams::default();
        params.events.initiation_waiting_time = 1e-6;
        let mut simulation = passive_margin(params, 70.0);
        for _ in 0..10 {
            simulation.initiate_subduction(1.0, u32::MAX, &[]);
        }
        assert_eq!(simulation.plates.len(), 1);
    }

    #[test]
    fn plates_born_in_the_same_step_do_not_subduct() {
        let mut params = TectonicParams::default();
        params.events.initiation_waiting_time = 1e-6;
        let mut simulation = passive_margin(params, 250.0);
        simulation.initiate_subduction(1.0, 0, &[]);
        assert_eq!(simulation.plates.len(), 1);
    }

    /// Two resting oceanic plates of the given ages without subduction polarity.
    fn resting_oceanic_plates(ages: [f64; 2]) -> Simulation {
        let mut params = TectonicParams::default();
        params.events.boundary_initiation_waiting_time = 1e-6;
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 2, params, 3);
        simulation.polarities.clear();
        for (plate, age) in simulation.plates.iter_mut().zip(ages) {
            plate.angular_velocity = DVec3::ZERO;
            for crust in plate.crust.iter_mut().flatten() {
                *crust = new_oceanic_crust(-age);
            }
        }
        simulation
    }

    #[test]
    fn old_ocean_starts_subducting_below_younger_ocean_at_a_boundary() {
        let mut simulation = resting_oceanic_plates([200.0, 30.0]);
        let boundaries = simulation.boundary_cells();
        assert!(boundaries.iter().all(|b| b.kind == BoundaryKind::Other));
        simulation.initiate_subduction(1.0, u32::MAX, &boundaries);
        assert_eq!(simulation.plates.len(), 2);
        assert_eq!(simulation.event_counts.induced_subductions, 1);
        // One segment: the old plate subducts there.
        let segments = simulation.polarities.pair_segments(0, 1);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].overriding, 1);
        // The old plate's side of the boundary is a trench there, and only there.
        let boundaries = simulation.boundary_cells();
        let trenches = boundaries
            .iter()
            .filter(|b| matches!(b.kind, BoundaryKind::Trench { .. }))
            .count();
        let old_side = boundaries.iter().filter(|b| b.plate == 0).count();
        assert!(
            trenches > 0 && trenches < old_side,
            "{trenches} of {old_side}"
        );
        assert!(
            boundaries
                .iter()
                .filter(|b| matches!(b.kind, BoundaryKind::Trench { .. }))
                .all(|b| b.plate == 0)
        );
    }

    #[test]
    fn old_overriding_ocean_reverses_the_subduction_polarity() {
        let mut simulation = resting_oceanic_plates([200.0, 30.0]);
        // The young plate subducts below the old one.
        simulation.seed_subduction((0, 1), 0, 600e3);
        let segments = simulation.polarities.pair_segments(0, 1).len();
        assert!(segments > 1);
        let boundaries = simulation.boundary_cells();
        simulation.initiate_subduction(1.0, u32::MAX, &boundaries);
        assert_eq!(simulation.event_counts.polarity_reversals, 1);
        // Exactly one segment reversed, with a new short slab; the others are unchanged.
        let segments = simulation.polarities.pair_segments(0, 1);
        let reversed: Vec<_> = segments.iter().filter(|s| s.overriding == 1).collect();
        assert_eq!(reversed.len(), 1);
        assert_eq!(
            reversed[0].slab_length,
            simulation.params.events.initiation_slab_length
        );
        assert!(
            segments
                .iter()
                .filter(|s| s.overriding == 0)
                .all(|s| s.slab_length == 600e3)
        );
    }

    #[test]
    fn ocean_of_similar_age_does_not_start_subducting_at_a_boundary() {
        let mut simulation = resting_oceanic_plates([200.0, 190.0]);
        let boundaries = simulation.boundary_cells();
        simulation.initiate_subduction(1.0, u32::MAX, &boundaries);
        assert_eq!(simulation.event_counts.induced_subductions, 0);
        assert!(simulation.polarities.is_empty());
    }

    #[test]
    fn old_passive_margins_start_subducting() {
        let mut params = TectonicParams::default();
        params.events.initiation_waiting_time = 1e-6;
        let mut simulation = passive_margin(params, 250.0);
        simulation.initiate_subduction(1.0, u32::MAX, &[]);
        assert_eq!(simulation.plates.len(), 2);
        // The oceanic plate subducts below the continental one along the margin.
        let segments = simulation.polarities.pair_segments(0, 1);
        assert!(segments.len() > 1, "{}", segments.len());
        assert!(
            segments
                .iter()
                .all(|s| s.overriding == 1 && s.slab_length > 0.0)
        );
        let continental_cells = (0..simulation.mesh.len() as u32)
            .filter(|&cell| simulation.owners[cell as usize].plate == 1)
            .filter(|&cell| simulation.surface_crust(cell).is_continental())
            .count();
        assert!(continental_cells > 0);
    }

    #[test]
    fn long_collisions_suture_plates() {
        let mut params = TectonicParams::default();
        params.events.suture_time = 3.0;
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 2, params, 4);
        for plate in &mut simulation.plates {
            for crust in plate.crust.iter_mut().flatten() {
                *crust = Crust {
                    thickness: 35_000.0,
                    density: CONTINENTAL_DENSITY,
                    birth: -1000.0,
                    sediment: 0.0,
                };
            }
        }
        simulation.seed_subduction((0, 1), 0, 0.0);
        for _ in 0..5 {
            let boundaries = simulation.boundary_cells();
            simulation.suture_colliding_plates(1.0, &boundaries);
        }
        assert_eq!(simulation.plates.len(), 1);
        assert_eq!(simulation.event_counts.sutures, 1);
    }

    #[test]
    fn plumes_rift_large_plates() {
        let mut simulation =
            Simulation::new(SphereMesh::icosphere(4), 1, TectonicParams::default(), 6);
        simulation.mantle.insert_plume(Plume {
            position: DVec3::Z,
            age: 50.0,
            lifetime: 200.0,
            peak_flux: 5e-3,
            has_rifted: false,
        });
        simulation.rift_above_plumes();
        assert_eq!(simulation.plates.len(), 2);
        assert_eq!(simulation.event_counts.rifts, 1);
        assert!(simulation.mantle.plumes()[0].has_rifted);
    }

    #[test]
    fn tiny_plates_are_captured() {
        let mut params = TectonicParams::default();
        params.events.min_plate_area_before_capture = 0.05;
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 30, params, 8);
        let before = simulation.plates.len();
        simulation.capture_tiny_plates();
        assert!(simulation.plates.len() < before);
        assert_eq!(
            before - simulation.plates.len(),
            simulation.event_counts.captures
        );
        let area = simulation.plate_area_fractions();
        assert!(area.iter().all(|&a| a >= 0.05));
        for cell in 0..simulation.mesh.len() as u32 {
            simulation.surface_crust(cell);
        }
    }

    #[test]
    fn plume_rifts_through_old_ocean_do_not_become_trenches() {
        let mut simulation =
            Simulation::new(SphereMesh::icosphere(4), 1, TectonicParams::default(), 6);
        for crust in simulation.plates[0].crust.iter_mut().flatten() {
            *crust = new_oceanic_crust(-200.0);
        }
        simulation.mantle.insert_plume(Plume {
            position: DVec3::Z,
            age: 50.0,
            lifetime: 200.0,
            peak_flux: 5e-3,
            has_rifted: false,
        });
        simulation.rift_above_plumes();
        assert_eq!(simulation.plates.len(), 2);
        for _ in 0..2 {
            simulation.step(1.0);
            assert!(
                simulation
                    .boundary_cells()
                    .iter()
                    .filter(|b| b.convergence < 0.0)
                    .all(|b| !matches!(b.kind, BoundaryKind::Trench { .. }))
            );
        }
    }
}
