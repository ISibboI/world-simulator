//! Changes of the plate configuration: splitting a plate in two, merging two plates, and
//! removing plates that have been consumed.

use glam::DVec3;

use super::ledger::{self, Term};
use super::*;

/// Rings around the target vertex searched for an empty vertex when merging plates.
const MERGE_RINGS: usize = 2;

/// The empty vertex of `plate` within [`MERGE_RINGS`] rings of `target` nearest to `position`
/// (in the plate's frame), if any.
fn free_vertex(mesh: &SphereMesh, plate: &Plate, target: u32, position: DVec3) -> Option<u32> {
    let mut ring = vec![target];
    let mut seen = vec![target];
    for _ in 0..=MERGE_RINGS {
        let best = ring
            .iter()
            .copied()
            .filter(|&vertex| plate.crust[vertex as usize].is_none())
            .max_by(|&a, &b| {
                mesh.vertex(a)
                    .dot(position)
                    .total_cmp(&mesh.vertex(b).dot(position))
                    .then(b.cmp(&a))
            });
        if best.is_some() {
            return best;
        }
        let mut next = Vec::new();
        for &vertex in &ring {
            for &neighbor in mesh.neighbors(vertex) {
                if !seen.contains(&neighbor) {
                    seen.push(neighbor);
                    next.push(neighbor);
                }
            }
        }
        ring = next;
    }
    None
}

/// Adds `crust` to the sample of `plate` at `target` and its neighbours with crust in equal
/// shares (thickness and sediment; densities mixed by mass, no thickness cap: mass is
/// conserved, crustal flow and erosion act on it later).
fn spread_crust(mesh: &SphereMesh, plate: &mut Plate, target: u32, crust: Crust) {
    let receivers: Vec<u32> = std::iter::once(target)
        .chain(mesh.neighbors(target).iter().copied())
        .filter(|&vertex| plate.crust[vertex as usize].is_some())
        .collect();
    let share = 1.0 / receivers.len() as f64;
    for vertex in receivers {
        let receiver = plate.crust[vertex as usize]
            .as_mut()
            .expect("receivers have crust");
        let added = crust.thickness * share;
        let mass = receiver.thickness * receiver.density + added * crust.density;
        receiver.thickness += added;
        receiver.density = mass / receiver.thickness.max(1e-9);
        receiver.sediment += crust.sediment * share;
    }
}

impl Simulation {
    /// Splits `plate` along the great circle with the given unit `normal`: the crust on the side
    /// the normal points to moves to a new plate with the same rotation and motion. Returns the
    /// new plate, or `None` if one side would be empty.
    pub(super) fn split_plate(&mut self, plate: u32, normal: DVec3) -> Option<u32> {
        let rotation = self.plates[plate as usize].rotation;
        let new_side: Vec<bool> = (0..self.mesh.len() as u32)
            .map(|local| (rotation * self.mesh.vertex(local)).dot(normal) > 0.0)
            .collect();
        let near = self.polarities.radius().sin();
        self.split_plate_by(plate, new_side, move |_, r| {
            let side = r.dot(normal);
            polarity::Side {
                new: side > -near,
                old: side < near,
            }
        })
    }

    /// Splits the piece of `plate` made of the world cells `piece` off into a new plate with the
    /// same rotation and motion: the crust samples shown at those cells, and hidden samples whose
    /// nearest cell is in or next to the piece. Returns the new plate, or `None` if one side would
    /// be empty.
    pub(super) fn split_plate_cells(&mut self, plate: u32, piece: &[u32]) -> Option<u32> {
        let cell_count = self.mesh.len();
        let mut in_piece = vec![false; cell_count];
        for &cell in piece {
            in_piece[cell as usize] = true;
        }
        // Cells in or next to the piece.
        let near_piece: Vec<bool> = (0..cell_count as u32)
            .map(|cell| {
                in_piece[cell as usize]
                    || self
                        .mesh
                        .neighbors(cell)
                        .iter()
                        .any(|&neighbor| in_piece[neighbor as usize])
            })
            .collect();
        let mut new_side = vec![false; cell_count];
        let mut visible = vec![false; cell_count];
        for (cell, owner) in self.owners.iter().enumerate() {
            if owner.plate == plate {
                visible[owner.local as usize] = true;
                if in_piece[cell] {
                    new_side[owner.local as usize] = true;
                }
            }
        }
        let rotation = self.plates[plate as usize].rotation;
        for (local, crust) in self.plates[plate as usize].crust.iter().enumerate() {
            if crust.is_some() && !visible[local] {
                let cell = self.mesh.nearest(rotation * self.mesh.vertex(local as u32));
                new_side[local] = near_piece[cell as usize];
            }
        }
        // Boundary segments go to the part(s) with cells within a segment radius.
        let rings = (self.polarities.radius() / self.mesh.mean_edge_angle()).ceil() as usize;
        let rest: Vec<u32> = (0..cell_count as u32)
            .filter(|&cell| self.owners[cell as usize].plate == plate && !in_piece[cell as usize])
            .collect();
        let near_new = self.within_rings(piece, rings);
        let near_old = self.within_rings(&rest, rings);
        let side_of_segment = move |mesh: &SphereMesh, r: DVec3| {
            let cell = mesh.nearest(r) as usize;
            let (new, old) = (near_new[cell], near_old[cell]);
            polarity::Side {
                new,
                old: old || !new,
            }
        };
        self.split_plate_by(plate, new_side, side_of_segment)
    }

    /// Cells within `rings` neighbour steps of any of `cells`.
    fn within_rings(&self, cells: &[u32], rings: usize) -> Vec<bool> {
        let mut reached = vec![false; self.mesh.len()];
        let mut frontier: Vec<u32> = cells.to_vec();
        for &cell in cells {
            reached[cell as usize] = true;
        }
        for _ in 0..rings {
            let mut next = Vec::new();
            for &cell in &frontier {
                for &neighbor in self.mesh.neighbors(cell) {
                    if !reached[neighbor as usize] {
                        reached[neighbor as usize] = true;
                        next.push(neighbor);
                    }
                }
            }
            frontier = next;
        }
        reached
    }

    /// Splits off the crust samples of `plate` whose local vertex is marked in `new_side` into a
    /// new plate; `segment_side` tells which plate(s) a boundary segment at a world position goes
    /// to (given the mesh).
    fn split_plate_by(
        &mut self,
        plate: u32,
        new_side: Vec<bool>,
        segment_side: impl Fn(&SphereMesh, DVec3) -> polarity::Side,
    ) -> Option<u32> {
        let before = ledger::plates_mass(&self.plates);
        let result = self.split_plate_inner(plate, &new_side, segment_side);
        let after = ledger::plates_mass(&self.plates);
        self.ledger.record(Term::Splits, after - before);
        if result.is_some() {
            self.configuration_changes += 1;
        }
        result
    }

    fn split_plate_inner(
        &mut self,
        plate: u32,
        new_side: &[bool],
        segment_side: impl Fn(&SphereMesh, DVec3) -> polarity::Side,
    ) -> Option<u32> {
        let (moving, staying) = self.plates[plate as usize]
            .crust
            .iter()
            .zip(new_side)
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
            .zip(new_side)
            .map(|(slot, &new)| if new { slot.take() } else { None })
            .collect();
        let new_plate = Plate {
            rotation: self.plates[plate as usize].rotation,
            angular_velocity: self.plates[plate as usize].angular_velocity,
            crust,
            lookup_cache: self.plates[plate as usize].lookup_cache.clone(),
            consumed: self.plates[plate as usize].consumed.clone(),
            tracked: None,
            matched_rotation: self.plates[plate as usize].matched_rotation,
        };
        // The tracked crust of the remaining part is no longer accurate.
        self.plates[plate as usize].tracked = None;
        let new_index = self.plates.len() as u32;
        self.plates.push(new_plate);

        for owner in &mut self.owners {
            if owner.plate == plate
                && self.plates[plate as usize].crust[owner.local as usize].is_none()
            {
                owner.plate = new_index;
            }
        }

        // Segments of the original plate's boundaries go to the half they lie on.
        let rotations = self.rotations();
        let mesh = &self.mesh;
        self.polarities
            .split(plate, new_index, |r| segment_side(mesh, r), &rotations);
        Some(new_index)
    }

    /// Merges plate `absorbed` into plate `into`: every crust sample of `absorbed`, visible or
    /// hidden, moves by its position into the frame of `into`, to the nearest empty vertex
    /// within [`MERGE_RINGS`] rings, or else is spread over the sample there and its
    /// neighbours. No crust is lost or created. Returns the volume (thickness × cells, m) of
    /// continental crust lost, which is zero.
    pub(super) fn merge_plates(&mut self, into: u32, absorbed: u32) -> f64 {
        let before = ledger::plates_mass(&self.plates);
        self.merge_plates_inner(into, absorbed);
        let after = ledger::plates_mass(&self.plates);
        self.ledger.record(Term::Merges, after - before);
        0.0
    }

    fn merge_plates_inner(&mut self, into: u32, absorbed: u32) {
        assert_ne!(into, absorbed, "a plate cannot be merged into itself");
        let rotations = self.rotations();
        self.polarities.merge(into, absorbed, &rotations);
        // The receiving plate gains crust at new vertices.
        self.plates[into as usize].tracked = None;
        let to_into =
            self.plates[into as usize].rotation.inverse() * self.plates[absorbed as usize].rotation;
        let cell_count = self.mesh.len();
        // Where each sample of `absorbed` went in `into`.
        let mut destination = vec![u32::MAX; cell_count];
        let mut target = 0u32;
        for local in 0..cell_count as u32 {
            let Some(crust) = self.plates[absorbed as usize].crust[local as usize].take() else {
                continue;
            };
            let position = to_into * self.mesh.vertex(local);
            target = self.mesh.nearest_from(position, target);
            let receiving = &mut self.plates[into as usize];
            destination[local as usize] = match free_vertex(&self.mesh, receiving, target, position)
            {
                Some(vertex) => {
                    receiving.crust[vertex as usize] = Some(crust);
                    receiving.consumed[vertex as usize] = false;
                    vertex
                }
                None => {
                    spread_crust(&self.mesh, receiving, target, crust);
                    target
                }
            };
        }
        for owner in &mut self.owners {
            if owner.plate == absorbed {
                *owner = Ownership {
                    plate: into,
                    local: destination[owner.local as usize],
                };
            }
        }
        self.remove_plate(absorbed);
    }

    /// Removes a plate that owns no cells. The last plate takes its index.
    pub(super) fn remove_plate(&mut self, plate: u32) {
        debug_assert!(
            self.owners.iter().all(|owner| owner.plate != plate),
            "removed plates must not own cells"
        );
        self.configuration_changes += 1;
        let last = self.plates.len() as u32 - 1;
        self.plates.swap_remove(plate as usize);
        self.polarities.remove_plate(plate, last);
        if plate == last {
            return;
        }
        for owner in &mut self.owners {
            if owner.plate == last {
                owner.plate = plate;
            }
        }
    }

    /// Removes all plates that no longer own any cell; their hidden crust is lost.
    pub(super) fn remove_dead_plates(&mut self) {
        let mut area = vec![0usize; self.plates.len()];
        for owner in &self.owners {
            area[owner.plate as usize] += 1;
        }
        // Remove from the back so that indices of plates still to be checked stay valid.
        for plate in (0..self.plates.len()).rev() {
            if area[plate] == 0 {
                let lost = ledger::plates_mass(&self.plates[plate..=plate]);
                self.ledger.record(Term::RemovedPlates, -lost);
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
        for (&(a, b), pair) in &simulation.polarities.pairs {
            assert!(a < b && (b as usize) < simulation.plates.len());
            for segment in &pair.segments {
                assert!(segment.overriding == a || segment.overriding == b);
            }
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
    fn merging_after_motion_conserves_all_crust() {
        let mut simulation = simulation();
        for _ in 0..20 {
            simulation.step(1.0);
        }
        // Two neighbouring plates, which have moved relative to each other.
        let (a, b) = (0..simulation.mesh.len() as u32)
            .find_map(|cell| {
                let plate = simulation.owners[cell as usize].plate;
                simulation
                    .mesh
                    .neighbors(cell)
                    .iter()
                    .find_map(|&neighbor| {
                        let other = simulation.owners[neighbor as usize].plate;
                        (other != plate).then_some((plate.min(other), plate.max(other)))
                    })
            })
            .unwrap();
        let mass = simulation.total_mass();
        let cells = simulation
            .owners
            .iter()
            .filter(|owner| owner.plate == a || owner.plate == b)
            .count();
        let max_thickness = |simulation: &Simulation| {
            simulation
                .plates
                .iter()
                .flat_map(|plate| plate.crust.iter().flatten())
                .map(|crust| crust.thickness)
                .fold(0.0, f64::max)
        };
        let thickest = max_thickness(&simulation);
        simulation.merge_plates(a, b);
        // Hidden samples included, nothing is lost or created.
        assert!((simulation.total_mass() / mass - 1.0).abs() < 1e-12);
        // The merged plate shows crust at all cells of both plates.
        let merged = simulation
            .owners
            .iter()
            .filter(|owner| owner.plate == a)
            .count();
        assert_eq!(merged, cells);
        // Spreading samples that found no free vertex thickens crust only a little.
        assert!(max_thickness(&simulation) < 1.25 * thickest);
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
        for pair in simulation.polarities.pairs.values_mut() {
            pair.collision_time = 25.0;
        }
        let normal = simulation.mesh.vertex(0).any_orthonormal_vector();
        let new_plate = simulation.split_plate(plate, normal).unwrap();
        for (&(a, b), pair) in &simulation.polarities.pairs {
            if a == new_plate || b == new_plate {
                assert_eq!(pair.collision_time, 0.0);
            }
        }
    }
}
