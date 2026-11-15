//! Changes of the plate configuration: splitting a plate in two, merging two plates, and
//! removing plates that have been consumed.

use std::collections::HashMap;

use glam::DVec3;

use super::*;

impl Simulation {
    /// Splits `plate` along the great circle with the given unit `normal`: the crust on the side
    /// the normal points to moves to a new plate with the same rotation and motion. Returns the
    /// new plate, or `None` if one side would be empty.
    pub(super) fn split_plate(&mut self, plate: u32, normal: DVec3) -> Option<u32> {
        let rotation = self.plates[plate as usize].rotation;
        let new_side: Vec<bool> = (0..self.mesh.len() as u32)
            .map(|local| (rotation * self.mesh.vertex(local)).dot(normal) > 0.0)
            .collect();
        let (moving, staying) = self.plates[plate as usize]
            .crust
            .iter()
            .zip(&new_side)
            .filter(|(crust, _)| crust.is_some())
            .fold((0usize, 0usize), |(moving, staying), (_, &new)| {
                if new {
                    (moving + 1, staying)
                } else {
                    (moving, staying + 1)
                }
            });
        if moving == 0 || staying == 0 {
            return None;
        }

        let crust: Vec<Option<Crust>> = self.plates[plate as usize]
            .crust
            .iter_mut()
            .zip(&new_side)
            .map(|(slot, &new)| if new { slot.take() } else { None })
            .collect();
        let new_plate = Plate {
            rotation: self.plates[plate as usize].rotation,
            angular_velocity: self.plates[plate as usize].angular_velocity,
            crust,
            lookup_cache: self.plates[plate as usize].lookup_cache.clone(),
            consumed: self.plates[plate as usize].consumed.clone(),
        };
        let new_index = self.plates.len() as u32;
        self.plates.push(new_plate);

        for owner in &mut self.owners {
            if owner.plate == plate
                && self.plates[plate as usize].crust[owner.local as usize].is_none()
            {
                owner.plate = new_index;
            }
        }

        // Both halves inherit the subduction polarities of the original plate.
        let inherited: Vec<((u32, u32), Polarity)> = self
            .polarities
            .iter()
            .filter(|((a, b), _)| *a == plate || *b == plate)
            .map(|(&(a, b), &polarity)| {
                let other = if a == plate { b } else { a };
                let overriding = if polarity.overriding == plate {
                    new_index
                } else {
                    polarity.overriding
                };
                (
                    pair_key(new_index, other),
                    Polarity {
                        overriding,
                        // A new plate has no history of collision with its neighbours.
                        collision_time: 0.0,
                        ..polarity
                    },
                )
            })
            .collect();
        self.polarities.extend(inherited);
        Some(new_index)
    }

    /// Merges plate `absorbed` into plate `into`. The crust of `absorbed` that is visible at the
    /// surface moves into the frame of `into`. Where `into` already has crust below it, continental
    /// crust is stacked; otherwise the visible crust replaces the hidden one. Returns the volume
    /// (thickness × cells, m) of continental crust of `absorbed` that was hidden and is lost.
    pub(super) fn merge_plates(&mut self, into: u32, absorbed: u32) -> f64 {
        assert_ne!(into, absorbed, "a plate cannot be merged into itself");
        let inverse_rotation = self.plates[into as usize].rotation.inverse();
        let max_thickness = self.params.max_crust_thickness;
        // Where each sample of `absorbed` went, for cells sharing a sample.
        let mut destination: HashMap<u32, u32> = HashMap::new();

        for cell in 0..self.mesh.len() as u32 {
            let owner = self.owners[cell as usize];
            if owner.plate != absorbed {
                continue;
            }
            let moved = self.plates[absorbed as usize].crust[owner.local as usize].take();
            let target = &mut self.plates[into as usize];
            let local = target.local_vertex(&self.mesh, inverse_rotation, cell);
            let local = match moved {
                Some(crust) => {
                    match &mut target.crust[local as usize] {
                        Some(existing) if existing.is_continental() && crust.is_continental() => {
                            existing.add_crust(
                                crust.thickness + crust.sediment,
                                crust.density,
                                max_thickness,
                            );
                        }
                        slot => *slot = Some(crust),
                    }
                    target.consumed[local as usize] = false;
                    destination.insert(owner.local, local);
                    local
                }
                // The sample was already moved for another cell that shared it.
                None if target.crust[local as usize].is_some() => local,
                None => destination[&owner.local],
            };
            self.owners[cell as usize] = Ownership { plate: into, local };
        }
        let lost = self.plates[absorbed as usize]
            .crust
            .iter()
            .flatten()
            .filter(|crust| crust.is_continental())
            .map(|crust| crust.thickness)
            .sum();
        self.remove_plate(absorbed);
        lost
    }

    /// Removes a plate that owns no cells. The last plate takes its index.
    pub(super) fn remove_plate(&mut self, plate: u32) {
        debug_assert!(
            self.owners.iter().all(|owner| owner.plate != plate),
            "removed plates must not own cells"
        );
        let last = self.plates.len() as u32 - 1;
        self.plates.swap_remove(plate as usize);
        self.polarities
            .retain(|&(a, b), _| a != plate && b != plate);
        if plate == last {
            return;
        }
        for owner in &mut self.owners {
            if owner.plate == last {
                owner.plate = plate;
            }
        }
        let renumber = |index: u32| if index == last { plate } else { index };
        self.polarities = self
            .polarities
            .drain()
            .map(|((a, b), polarity)| {
                (
                    pair_key(renumber(a), renumber(b)),
                    Polarity {
                        overriding: renumber(polarity.overriding),
                        ..polarity
                    },
                )
            })
            .collect();
    }

    /// Removes all plates that no longer own any cell.
    pub(super) fn remove_dead_plates(&mut self) {
        let mut area = vec![0usize; self.plates.len()];
        for owner in &self.owners {
            area[owner.plate as usize] += 1;
        }
        // Remove from the back so that indices of plates still to be checked stay valid.
        for plate in (0..self.plates.len()).rev() {
            if area[plate] == 0 {
                self.remove_plate(plate as u32);
                self.event_counts.consumed += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simulation() -> Simulation {
        Simulation::new(SphereMesh::icosphere(4), 6, TectonicParams::default(), 11)
    }

    fn total_thickness(simulation: &Simulation) -> f64 {
        simulation
            .plates
            .iter()
            .flat_map(|plate| plate.crust.iter().flatten())
            .map(|crust| crust.thickness)
            .sum()
    }

    fn assert_consistent(simulation: &Simulation) {
        for cell in 0..simulation.mesh.len() as u32 {
            // Panics if an owner points to a missing plate or missing crust.
            simulation.surface_crust(cell);
        }
        for (&(a, b), polarity) in &simulation.polarities {
            assert!(a < b && (b as usize) < simulation.plates.len());
            assert!(polarity.overriding == a || polarity.overriding == b);
        }
    }

    #[test]
    fn splitting_conserves_crust() {
        let mut simulation = simulation();
        let before = total_thickness(&simulation);
        let plates = simulation.plates.len();
        // Split the plate owning cell 0 through its own position.
        let plate = simulation.owners[0].plate;
        let normal = simulation.mesh.vertex(0).any_orthonormal_vector();
        let new_plate = simulation
            .split_plate(plate, normal)
            .expect("split succeeds");
        assert_eq!(simulation.plates.len(), plates + 1);
        assert!(
            simulation
                .owners
                .iter()
                .any(|owner| owner.plate == new_plate)
        );
        assert!(simulation.owners.iter().any(|owner| owner.plate == plate));
        assert!((total_thickness(&simulation) - before).abs() < 1e-6 * before);
        assert_consistent(&simulation);
    }

    #[test]
    fn merging_conserves_visible_crust() {
        let mut simulation = simulation();
        // Merge the plates of two neighbouring cells.
        let (a, b) = (0..simulation.mesh.len() as u32)
            .find_map(|cell| {
                let plate = simulation.owners[cell as usize].plate;
                simulation
                    .mesh
                    .neighbors(cell)
                    .iter()
                    .find_map(|&neighbor| {
                        let other = simulation.owners[neighbor as usize].plate;
                        (other != plate).then_some((plate, other))
                    })
            })
            .unwrap();
        let before = total_thickness(&simulation);
        let plates = simulation.plates.len();
        simulation.merge_plates(a.min(b), a.max(b));
        assert_eq!(simulation.plates.len(), plates - 1);
        // At t = 0 every sample is visible, so all crust is conserved.
        assert!((total_thickness(&simulation) - before).abs() < 1e-6 * before);
        assert_consistent(&simulation);
    }

    #[test]
    fn dead_plates_are_removed_and_indices_stay_valid() {
        let mut simulation = simulation();
        let plates = simulation.plates.len();
        // Merge plate 0 into plate 1 by hand: give all of plate 0's cells to plate 1.
        let (a, b) = (0, 1);
        simulation.merge_plates(b, a);
        simulation.remove_dead_plates();
        assert_eq!(simulation.plates.len(), plates - 1);
        assert_consistent(&simulation);
        // Keep simulating to check nothing refers to removed plates.
        for _ in 0..5 {
            simulation.step(1.0);
        }
        assert_consistent(&simulation);
    }

    #[test]
    fn split_plates_start_without_collision_history() {
        let mut simulation = simulation();
        let plate = simulation.owners[0].plate;
        for polarity in simulation.polarities.values_mut() {
            polarity.collision_time = 25.0;
        }
        let normal = simulation.mesh.vertex(0).any_orthonormal_vector();
        let new_plate = simulation.split_plate(plate, normal).unwrap();
        for (&(a, b), polarity) in &simulation.polarities {
            if a == new_plate || b == new_plate {
                assert_eq!(polarity.collision_time, 0.0);
            }
        }
    }
}
