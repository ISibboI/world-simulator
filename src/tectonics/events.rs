//! Events that change the plate configuration: rifting above mantle plumes, initiation of
//! subduction at old passive margins, and suturing of plates after a continental collision.

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
    /// Only plates covering at least this fraction of the surface are rifted by plumes.
    pub min_rift_area: f64,
    /// Neither part of a split plate may cover less than this fraction of the surface.
    pub min_plate_area: f64,
    /// No plates are split while there are this many plates.
    pub max_plates: usize,
    /// Oceanic lithosphere at a passive margin may start to subduct from this age in Myr.
    pub initiation_age: f64,
    /// Mean time in Myr between subduction initiations while old passive margins exist.
    pub initiation_interval: f64,
    /// Plates whose continents have been colliding for this many Myr merge.
    pub suture_time: f64,
    /// Minimum number of colliding boundary cells for a pair of plates to count as colliding.
    pub suture_min_cells: usize,
}

impl Default for PlateEventParams {
    fn default() -> Self {
        Self {
            rift_delay: 20.0,
            min_rift_area: 0.05,
            min_plate_area: 0.01,
            max_plates: 30,
            initiation_age: 180.0,
            initiation_interval: 25.0,
            suture_time: 20.0,
            suture_min_cells: 3,
        }
    }
}

impl Simulation {
    /// Runs the plate events for a time step of `dt` Myr.
    pub(super) fn plate_events(&mut self, dt: f64) {
        self.suture_colliding_plates(dt);
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
        let new_plate = self.split_plate(plate, normal)?;
        self.plate_births += 1;
        Some(new_plate)
    }

    /// A plume that has been rising below a large plate for long enough splits it along a great
    /// circle through the plume. The plume's outflow then drives the two parts apart.
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
            if self.plate_area_fractions()[plate as usize] < params.min_rift_area {
                continue;
            }
            // A random great circle through the plume.
            let direction = random_unit_vector(&mut self.rng);
            let normal = (direction - position * direction.dot(position)).normalize_or_zero();
            if normal == DVec3::ZERO {
                continue;
            }
            if let Some(new_plate) = self.split_if_large_enough(plate, normal) {
                debug!(plate, new_plate, time = self.time, "Plume rifted a plate");
            }
            // Each plume rifts at most once, also if the split was not possible.
            self.mantle.mark_rifted(index);
        }
    }

    /// Old oceanic lithosphere at a passive margin (ocean and continent on the same plate) may
    /// start to sink: the plate splits along the margin and the oceanic part subducts below the
    /// continental part.
    fn initiate_subduction(&mut self, dt: f64) {
        let params = self.params.events.clone();
        // Old oceanic cells next to continental crust of the same plate, weighted by age.
        let mut candidates: Vec<(u32, f64)> = Vec::new();
        for cell in 0..self.mesh.len() as u32 {
            let owner = self.owners[cell as usize];
            let crust = self.surface_crust(cell);
            if crust.is_continental() || crust.age < params.initiation_age {
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
            self.polarities.insert(
                pair_key(plate, continental_plate),
                Polarity {
                    overriding: continental_plate,
                    last_contact: self.time,
                    collision_time: 0.0,
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
    /// the collision has lasted long enough.
    fn suture_colliding_plates(&mut self, dt: f64) {
        let mut colliding: HashMap<(u32, u32), usize> = HashMap::new();
        for boundary in self.boundary_cells() {
            if boundary.kind == BoundaryKind::Collision {
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
                polarity.collision_time = 0.0;
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
            self.merge_plates(into, absorbed);
            self.plate_deaths += 1;
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
            },
        );
        for _ in 0..5 {
            simulation.suture_colliding_plates(1.0);
        }
        assert_eq!(simulation.plates.len(), 1);
        assert_eq!(simulation.plate_deaths, 1);
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
        assert_eq!(simulation.plate_births, 1);
        assert!(simulation.mantle.plumes()[0].has_rifted);
    }
}
