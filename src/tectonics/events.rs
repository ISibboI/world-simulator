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

use super::dynamics::BoundaryKind;
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
    /// Mean time in Myr between subduction initiations while old passive margins exist.
    pub initiation_interval: f64,
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
            max_plates: 30,
            initiation_age: 160.0,
            initiation_interval: 8.0,
            initiation_slab_length: 150e3,
            suture_time: 30.0,
            suture_min_cells: 10,
            min_plate_area_before_capture: 0.003,
        }
    }
}

/// How many plate events of each kind happened so far.
#[derive(Clone, Debug, Default)]
pub struct EventCounts {
    /// Plates split by plumes.
    pub rifts: usize,
    /// Plates split at passive margins by subduction initiation.
    pub initiations: usize,
    /// Plates merged after continental collisions.
    pub sutures: usize,
    /// Tiny plates captured by a neighbour.
    pub captures: usize,
    /// Plates that disappeared by subduction.
    pub consumed: usize,
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
    /// Runs the plate events for a time step of `dt` Myr.
    pub(super) fn plate_events(&mut self, dt: f64) {
        self.suture_colliding_plates(dt);
        self.capture_tiny_plates();
        self.rift_above_plumes();
        self.initiate_subduction(dt);
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

    /// Old oceanic lithosphere at a passive margin (ocean and continent on the same plate) may
    /// start to sink: the plate splits along the margin and the oceanic part subducts below the
    /// continental part, starting with a short slab.
    fn initiate_subduction(&mut self, dt: f64) {
        let params = self.params.events.clone();
        // Old oceanic cells next to continental crust of the same plate, weighted by age.
        let mut candidates: Vec<(u32, f64)> = Vec::new();
        for cell in 0..self.mesh.len() as u32 {
            let owner = self.owners[cell as usize];
            let crust = self.surface_crust(cell);
            if !crust.is_oceanic() || crust.age < params.initiation_age {
                continue;
            }
            let at_margin = self.mesh.neighbors(cell).iter().any(|&neighbor| {
                self.owners[neighbor as usize].plate == owner.plate
                    && self.surface_crust(neighbor).is_continental()
            });
            if at_margin {
                candidates.push((cell, crust.age - params.initiation_age + 1.0));
            }
        }
        if candidates.is_empty()
            || !self
                .rng
                .random_bool((dt / params.initiation_interval).clamp(0.0, 1.0))
        {
            return;
        }

        let total: f64 = candidates.iter().map(|(_, weight)| weight).sum();
        let mut pick = self.rng.random_range(0.0..total);
        let cell = candidates
            .iter()
            .find(|(_, weight)| {
                pick -= weight;
                pick <= 0.0
            })
            .map_or(candidates[candidates.len() - 1].0, |(cell, _)| *cell);

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
            self.polarities.insert(
                pair_key(plate, continental_plate),
                Polarity {
                    overriding: continental_plate,
                    last_contact: self.time,
                    collision_time: 0.0,
                    slab_length: params.initiation_slab_length,
                },
            );
            debug!(
                oceanic_plate = plate,
                continental_plate,
                time = self.time,
                "Subduction started at a passive margin"
            );
        }
    }

    /// Tracks how long pairs of plates have been in continental collision, and merges them once
    /// the collision has lasted long enough. Contacts count unless the plates clearly move
    /// apart (collision resistance drives locked collisions to near-zero convergence).
    fn suture_colliding_plates(&mut self, dt: f64) {
        let min_convergence =
            self.params.min_trench_convergence / (self.params.planet_radius * 100.0 / 1e6);
        let mut colliding: HashMap<(u32, u32), usize> = HashMap::new();
        for boundary in self.boundary_cells() {
            if boundary.kind == BoundaryKind::Collision && boundary.convergence > -min_convergence {
                *colliding
                    .entry(pair_key(boundary.plate, boundary.other))
                    .or_default() += 1;
            }
        }
        let min_cells = self.params.events.suture_min_cells;
        for (pair, polarity) in &mut self.polarities {
            if colliding.get(pair).copied().unwrap_or(0) >= min_cells {
                polarity.collision_time += dt;
            } else {
                polarity.collision_time = (polarity.collision_time - dt).max(0.0);
            }
        }

        let suture_time = self.params.events.suture_time;
        // Merge one pair at a time, since merging renumbers plates.
        while let Some(&(a, b)) = self
            .polarities
            .iter()
            .filter(|(_, polarity)| polarity.collision_time >= suture_time)
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
            let mut neighbours: HashMap<u32, (usize, f64)> = HashMap::new();
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

    #[test]
    fn old_passive_margins_start_subducting() {
        let mut params = TectonicParams::default();
        params.events.initiation_interval = 1.0;
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 1, params, 2);
        // One plate: continental where x > 0.3, very old ocean elsewhere.
        for (local, crust) in simulation.plates[0].crust.iter_mut().enumerate() {
            let x = simulation.mesh.vertex(local as u32).x;
            *crust = Some(if x > 0.3 {
                Crust {
                    thickness: 35_000.0,
                    density: CONTINENTAL_DENSITY,
                    age: 1000.0,
                    sediment: 0.0,
                }
            } else {
                Crust {
                    age: 250.0,
                    ..new_oceanic_crust()
                }
            });
        }
        simulation.initiate_subduction(1.0);
        assert_eq!(simulation.plates.len(), 2);
        // The oceanic plate subducts below the continental one.
        let polarity = simulation.polarities[&(0, 1)];
        assert_eq!(polarity.overriding, 1);
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
                    age: 1000.0,
                    sediment: 0.0,
                };
            }
        }
        simulation.polarities.insert(
            (0, 1),
            Polarity {
                overriding: 0,
                last_contact: 0.0,
                collision_time: 0.0,
                slab_length: 0.0,
            },
        );
        for _ in 0..5 {
            simulation.suture_colliding_plates(1.0);
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
            *crust = Crust {
                age: 200.0,
                ..new_oceanic_crust()
            };
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
