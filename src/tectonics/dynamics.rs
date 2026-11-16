//! Plate dynamics: plate velocities from a balance of the torques acting on each plate.
//!
//! The mantle is so viscous that inertia is negligible: at every moment, the forces on each plate
//! balance. The forces considered are
//!
//! * **slab pull** along trenches, where the subducting slab pulls its plate towards the trench.
//!   It grows with the thickness of the slab, i.e. with the square root of its age;
//! * **ridge push** on the young flanks of spreading ridges, away from the ridge;
//! * **basal drag** between each plate and the mantle below it, `τ = c (v_mantle − v_plate)`,
//!   stronger under thick continental keels;
//! * **collision resistance** where continental crust of two plates converges, which opposes the
//!   relative motion of the plates across the boundary.
//!
//! Drag and collision resistance are linear in the plate velocities, so the torque balance of all
//! plates is one linear system for their angular velocities, solved directly. Collision
//! resistance only acts where the plates actually converge, which is resolved by re-solving with
//! the converging contacts only (active set).

use glam::{DMat3, DVec3};

use super::*;

/// Seconds per million years.
const SECONDS_PER_MYR: f64 = 1e6 * 365.25 * 24.0 * 3600.0;

/// Maximum number of solves for the set of converging collision contacts.
const MAX_ACTIVE_SET_ITERATIONS: usize = 20;

/// What happens at a boundary cell, seen from the plate the cell belongs to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum BoundaryKind {
    /// The plate subducts here under the other plate; the slab pulls the plate towards the
    /// boundary.
    Trench { slab_age: f64 },
    /// The plates move apart at young oceanic crust; ridge push acts away from the boundary.
    Ridge,
    /// Continental crust of both plates meets; converging motion is resisted.
    Collision,
    /// Any other boundary (overriding side of a trench, transform, old divergent margin).
    Other,
}

/// A cell at the boundary of its plate towards one other plate.
#[derive(Clone, Copy, Debug)]
pub(super) struct BoundaryCell {
    pub(super) cell: u32,
    pub(super) plate: u32,
    pub(super) other: u32,
    /// Unit vector tangent to the sphere, pointing from the plate towards the other plate.
    pub(super) normal: DVec3,
    /// Relative velocity of the plate towards the other plate along `normal` in rad/Myr
    /// (positive where they converge).
    pub(super) convergence: f64,
    pub(super) kind: BoundaryKind,
}

impl Simulation {
    /// Finds all boundary cells and classifies them with the current plate velocities.
    pub(super) fn boundary_cells(&self) -> Vec<BoundaryCell> {
        let params = &self.params;
        let min_convergence = params.min_trench_convergence / (params.planet_radius * 100.0 / 1e6);
        let mut result = Vec::new();
        // Per other plate: summed direction towards it and one representative neighbour.
        let mut towards: Vec<(u32, DVec3, u32)> = Vec::new();

        for cell in 0..self.mesh.len() as u32 {
            let owner = self.owners[cell as usize];
            let position = self.mesh.vertex(cell);
            towards.clear();
            for &neighbor in self.mesh.neighbors(cell) {
                let other = self.owners[neighbor as usize].plate;
                if other == owner.plate {
                    continue;
                }
                let direction = self.mesh.vertex(neighbor) - position;
                match towards.iter_mut().find(|(plate, _, _)| *plate == other) {
                    Some((_, sum, _)) => *sum += direction,
                    None => towards.push((other, direction, neighbor)),
                }
            }
            if towards.is_empty() {
                continue;
            }

            let crust = self.surface_crust(cell);
            let velocity = self.plates[owner.plate as usize]
                .angular_velocity
                .cross(position);
            for &(other, direction, neighbor) in &towards {
                // Project onto the tangent plane.
                let normal = (direction - position * direction.dot(position)).normalize_or_zero();
                if normal == DVec3::ZERO {
                    continue;
                }
                let other_crust = self.surface_crust(neighbor);
                let other_velocity = self.plates[other as usize].angular_velocity.cross(position);
                let convergence = (velocity - other_velocity).dot(normal);
                let polarity = self.polarities.get(&pair_key(owner.plate, other));
                // A polarity only says which plate would subduct if the plates converge. A trench
                // needs convergence, or an existing slab where the plates do not clearly diverge
                // (hysteresis: a slab keeps pulling while the plates are nearly at rest).
                let subducts_here = polarity.is_some_and(|polarity| {
                    polarity.overriding == other
                        && (convergence > min_convergence
                            || (polarity.slab_length > 0.0 && convergence > -min_convergence))
                });

                let kind = if crust.is_continental() && other_crust.is_continental() {
                    BoundaryKind::Collision
                } else if subducts_here && !crust.is_continental() {
                    BoundaryKind::Trench {
                        slab_age: crust.age(self.time),
                    }
                } else if convergence < 0.0
                    && !crust.is_continental()
                    && crust.age(self.time) <= params.ridge_age
                {
                    BoundaryKind::Ridge
                } else {
                    BoundaryKind::Other
                };
                result.push(BoundaryCell {
                    cell,
                    plate: owner.plate,
                    other,
                    normal,
                    convergence,
                    kind,
                });
            }
        }
        result
    }

    /// Computes the angular velocities of all plates from the balance of torques, given the
    /// mantle flow velocity (m/s, tangent to the sphere) at each cell.
    pub(super) fn update_plate_velocities(&mut self, mantle_velocity: &[DVec3]) {
        let params = &self.params;
        let plate_count = self.plates.len();
        let radius = params.planet_radius;
        let cell_area = 4.0 * std::f64::consts::PI * radius * radius / self.mesh.len() as f64;

        // Torque balance per plate: drag matrix and driving torques.
        let mut drag = vec![DMat3::ZERO; plate_count];
        let mut driving = vec![DVec3::ZERO; plate_count];
        for cell in 0..self.mesh.len() as u32 {
            let plate = self.owners[cell as usize].plate as usize;
            let r = self.mesh.vertex(cell);
            let coefficient = params.basal_drag
                * if self.surface_crust(cell).is_continental() {
                    params.continental_drag_factor
                } else {
                    1.0
                }
                * cell_area;
            // Drag torque: R r × c A (v_mantle − R ω × r) = c A R (r × v_mantle) − c A R² P ω,
            // with the projection P = I − r rᵀ.
            drag[plate] += (DMat3::IDENTITY - outer(r, r)) * (coefficient * radius * radius);
            driving[plate] += r.cross(mantle_velocity[cell as usize]) * (coefficient * radius);
        }

        let boundaries = self.boundary_cells();
        for boundary in &boundaries {
            let r = self.mesh.vertex(boundary.cell);
            let force = self.boundary_force(boundary);
            driving[boundary.plate as usize] += r.cross(force) * radius;
        }

        // Collision resistance: the force on the plate −k ((v_p − v_q) · n) n at R r gives the
        // torque −k R² m mᵀ (ω_p − ω_q) with m = r × n. Per boundary cell, it is as strong as the
        // basal drag under `collision_resistance` cells.
        let coupling = params.collision_resistance * params.basal_drag * cell_area;
        let collisions: Vec<&BoundaryCell> = boundaries
            .iter()
            .filter(|boundary| boundary.kind == BoundaryKind::Collision)
            .collect();
        let mut active = vec![true; collisions.len()];

        let mut omega = vec![DVec3::ZERO; plate_count];
        let mut converged = false;
        for _ in 0..MAX_ACTIVE_SET_ITERATIONS {
            let size = 3 * plate_count;
            let mut matrix = vec![0.0; size * size];
            let mut rhs = vec![0.0; size];
            for plate in 0..plate_count {
                // A small regularisation (like the drag under a millionth of a cell) keeps plates
                // without cells, or with a single cell (free to spin about its own axis),
                // solvable.
                let block = drag[plate]
                    + DMat3::IDENTITY * (1e-6 * params.basal_drag * cell_area * radius * radius);
                add_block(&mut matrix, size, plate, plate, block);
                for i in 0..3 {
                    rhs[3 * plate + i] = driving[plate][i];
                }
            }
            for (boundary, _) in collisions
                .iter()
                .zip(&active)
                .filter(|(_, active)| **active)
            {
                let r = self.mesh.vertex(boundary.cell);
                let m = r.cross(boundary.normal);
                let block = outer(m, m) * (coupling * radius * radius);
                add_block(
                    &mut matrix,
                    size,
                    boundary.plate as usize,
                    boundary.plate as usize,
                    block,
                );
                add_block(
                    &mut matrix,
                    size,
                    boundary.plate as usize,
                    boundary.other as usize,
                    -block,
                );
            }

            let solution = solve_linear_system(matrix, rhs, size);
            for (plate, omega) in omega.iter_mut().enumerate() {
                *omega = DVec3::new(
                    solution[3 * plate],
                    solution[3 * plate + 1],
                    solution[3 * plate + 2],
                );
            }

            // Collision resistance only acts where the continents converge.
            let mut changed = false;
            for (boundary, active) in collisions.iter().zip(active.iter_mut()) {
                let r = self.mesh.vertex(boundary.cell);
                let relative = (omega[boundary.plate as usize] - omega[boundary.other as usize])
                    .cross(r)
                    .dot(boundary.normal);
                let converging = relative > 0.0;
                if converging != *active {
                    *active = converging;
                    changed = true;
                }
            }
            if !changed {
                converged = true;
                break;
            }
        }
        if !converged {
            tracing::debug!(time = self.time, "Collision active set did not converge");
        }

        for (plate, omega) in self.plates.iter_mut().zip(omega) {
            plate.angular_velocity = omega * SECONDS_PER_MYR;
        }
    }
}

impl Simulation {
    /// The force (N, tangent to the sphere) acting on the plate at a boundary cell: slab pull at
    /// trenches, growing with the age of the slab (lithosphere thickness ∝ √age) and with the
    /// length of slab that has already sunk; ridge push at ridges.
    pub(super) fn boundary_force(&self, boundary: &BoundaryCell) -> DVec3 {
        let params = &self.params;
        let boundary_length = params.planet_radius * self.mesh.mean_edge_angle();
        match boundary.kind {
            BoundaryKind::Trench { slab_age } => {
                let thickness_factor = (slab_age.min(params.slab_pull_max_age)
                    / params.slab_pull_reference_age)
                    .max(0.0)
                    .sqrt();
                let slab_length = self
                    .polarities
                    .get(&pair_key(boundary.plate, boundary.other))
                    .map_or(0.0, |polarity| polarity.slab_length);
                let length_factor = (slab_length / params.max_slab_length).clamp(0.0, 1.0);
                boundary.normal
                    * (params.slab_pull * thickness_factor * length_factor * boundary_length)
            }
            BoundaryKind::Ridge => -boundary.normal * (params.ridge_push * boundary_length),
            BoundaryKind::Collision | BoundaryKind::Other => DVec3::ZERO,
        }
    }

    /// Updates the length of slab below each subduction zone: it grows with the convergence at
    /// the trench (up to the length of an upper-mantle slab) and decays when subduction stops
    /// (the slab sinks or breaks off). `boundaries` are the current [`Self::boundary_cells`].
    ///
    /// Returns whether a slab appeared or disappeared, which changes the classification of
    /// boundary cells (see [`Self::boundary_cells`]).
    pub(super) fn update_slab_lengths(&mut self, dt: f64, boundaries: &[BoundaryCell]) -> bool {
        let params = &self.params;
        // Mean convergence at the trench cells of each pair, in m/Myr.
        let mut convergence: HashMap<(u32, u32), (f64, usize)> = HashMap::default();
        for boundary in boundaries {
            if matches!(boundary.kind, BoundaryKind::Trench { .. }) && boundary.convergence > 0.0 {
                let entry = convergence
                    .entry(pair_key(boundary.plate, boundary.other))
                    .or_default();
                entry.0 += boundary.convergence * params.planet_radius;
                entry.1 += 1;
            }
        }
        let decay = (-dt / params.slab_decay_time).exp();
        let mut slabs_changed = false;
        for (pair, polarity) in &mut self.polarities {
            let had_slab = polarity.slab_length > 0.0;
            match convergence.get(pair) {
                Some(&(sum, count)) if count > 0 => {
                    polarity.slab_length = (polarity.slab_length + sum / count as f64 * dt)
                        .min(params.max_slab_length);
                }
                _ => polarity.slab_length *= decay,
            }
            slabs_changed |= had_slab != (polarity.slab_length > 0.0);
        }
        slabs_changed
    }

    /// Positions of trench cells with their subduction rate (convergence speed times boundary
    /// length, in m²/s), which feed the mantle downwellings. `boundaries` are the current
    /// [`Self::boundary_cells`].
    pub(super) fn subduction_rates(&self, boundaries: &[BoundaryCell]) -> Vec<(DVec3, f64)> {
        let boundary_length = self.params.planet_radius * self.mesh.mean_edge_angle();
        boundaries
            .iter()
            .filter(|boundary| matches!(boundary.kind, BoundaryKind::Trench { .. }))
            .filter_map(|boundary| {
                let r = self.mesh.vertex(boundary.cell);
                let relative = (self.plates[boundary.plate as usize].angular_velocity
                    - self.plates[boundary.other as usize].angular_velocity)
                    .cross(r)
                    .dot(boundary.normal)
                    * self.params.planet_radius
                    / SECONDS_PER_MYR;
                (relative > 0.0).then_some((r, relative * boundary_length))
            })
            .collect()
    }
}

fn outer(a: DVec3, b: DVec3) -> DMat3 {
    DMat3::from_cols(a * b.x, a * b.y, a * b.z)
}

/// Adds a 3×3 block at block position (`row`, `column`) of a dense row-major matrix.
fn add_block(matrix: &mut [f64], size: usize, row: usize, column: usize, block: DMat3) {
    for i in 0..3 {
        for j in 0..3 {
            matrix[(3 * row + i) * size + 3 * column + j] += block.col(j)[i];
        }
    }
}

/// Solves `A x = b` for a dense row-major matrix by Gaussian elimination with partial pivoting.
fn solve_linear_system(mut matrix: Vec<f64>, mut rhs: Vec<f64>, size: usize) -> Vec<f64> {
    for column in 0..size {
        let pivot = (column..size)
            .max_by(|&a, &b| {
                matrix[a * size + column]
                    .abs()
                    .total_cmp(&matrix[b * size + column].abs())
            })
            .expect("non-empty range");
        if pivot != column {
            for j in 0..size {
                matrix.swap(pivot * size + j, column * size + j);
            }
            rhs.swap(pivot, column);
        }
        let diagonal = matrix[column * size + column];
        if diagonal.abs() < f64::MIN_POSITIVE {
            continue;
        }
        for row in column + 1..size {
            let factor = matrix[row * size + column] / diagonal;
            if factor == 0.0 {
                continue;
            }
            for j in column..size {
                matrix[row * size + j] -= factor * matrix[column * size + j];
            }
            rhs[row] -= factor * rhs[column];
        }
    }
    let mut solution = vec![0.0; size];
    for row in (0..size).rev() {
        let mut sum = rhs[row];
        for j in row + 1..size {
            sum -= matrix[row * size + j] * solution[j];
        }
        let diagonal = matrix[row * size + row];
        solution[row] = if diagonal.abs() < f64::MIN_POSITIVE {
            0.0
        } else {
            sum / diagonal
        };
    }
    solution
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_linear_systems() {
        let matrix = vec![2.0, 1.0, 0.0, 1.0, 3.0, 1.0, 0.0, 1.0, 4.0];
        let expected = [1.0, -2.0, 3.0];
        let rhs = (0..3)
            .map(|i| (0..3).map(|j| matrix[i * 3 + j] * expected[j]).sum())
            .collect();
        let solution = solve_linear_system(matrix, rhs, 3);
        for (x, e) in solution.iter().zip(expected) {
            assert!((x - e).abs() < 1e-12);
        }
    }

    fn one_plate_simulation() -> Simulation {
        let mut simulation =
            Simulation::new(SphereMesh::icosphere(3), 2, TectonicParams::default(), 3);
        // Merge everything into plate 0.
        for cell in 0..simulation.mesh.len() as u32 {
            let owner = simulation.owners[cell as usize];
            if owner.plate == 1 {
                let crust = simulation.plates[1].crust[owner.local as usize].take();
                simulation.plates[0].crust[owner.local as usize] = crust;
                simulation.owners[cell as usize].plate = 0;
            }
        }
        simulation
    }

    #[test]
    fn a_free_plate_follows_the_mantle() {
        let mut simulation = one_plate_simulation();
        // The whole mantle rotates rigidly around the z axis.
        let omega = DVec3::Z * 1e-15;
        let mantle: Vec<DVec3> = (0..simulation.mesh.len() as u32)
            .map(|cell| omega.cross(simulation.mesh.vertex(cell)) * simulation.params.planet_radius)
            .collect();
        simulation.update_plate_velocities(&mantle);
        let result = simulation.plates[0].angular_velocity / SECONDS_PER_MYR;
        assert!(
            (result - omega).length() < 1e-3 * omega.length(),
            "{result:?}"
        );
    }

    #[test]
    fn slab_pull_moves_a_plate_towards_its_trench() {
        let mut simulation =
            Simulation::new(SphereMesh::icosphere(4), 2, TectonicParams::default(), 5);
        for plate in &mut simulation.plates {
            plate.angular_velocity = DVec3::ZERO;
        }
        // Plate 0 subducts under plate 1.
        simulation.polarities.insert(
            pair_key(0, 1),
            Polarity {
                overriding: 1,
                last_contact: 0.0,
                collision_time: 0.0,
                slab_length: 600e3,
            },
        );
        // Old oceanic crust everywhere, except that plate 0 is continental on the -x side, so
        // that it only subducts along the +x part of its boundary. Pulls from all sides of a
        // plate would cancel out.
        for (index, plate) in simulation.plates.iter_mut().enumerate() {
            for (local, crust) in plate.crust.iter_mut().enumerate() {
                if let Some(crust) = crust {
                    let x = simulation.mesh.vertex(local as u32).x;
                    *crust = if index == 0 && x < 0.0 {
                        Crust {
                            thickness: 35_000.0,
                            density: crate::isostasy::CONTINENTAL_DENSITY,
                            birth: -1000.0,
                            sediment: 0.0,
                        }
                    } else {
                        new_oceanic_crust(-100.0)
                    };
                }
            }
        }
        let mantle = vec![DVec3::ZERO; simulation.mesh.len()];
        simulation.update_plate_velocities(&mantle);

        // At the trench, plate 0 moves towards plate 1 relative to plate 1.
        let boundaries = simulation.boundary_cells();
        let trenches: Vec<_> = boundaries
            .iter()
            .filter(|b| matches!(b.kind, BoundaryKind::Trench { .. }))
            .collect();
        assert!(!trenches.is_empty());
        let mean_convergence: f64 = trenches
            .iter()
            .map(|b| {
                let r = simulation.mesh.vertex(b.cell);
                (simulation.plates[0].angular_velocity - simulation.plates[1].angular_velocity)
                    .cross(r)
                    .dot(b.normal)
            })
            .sum::<f64>()
            / trenches.len() as f64;
        assert!(mean_convergence > 0.0);
        // Earth-like speed: a few cm/yr (1 rad/Myr = 637 cm/yr).
        let speed_cm_per_year = simulation.plates[0].angular_velocity.length() * 637.1;
        assert!(
            (0.5..30.0).contains(&speed_cm_per_year),
            "{speed_cm_per_year}"
        );
    }

    #[test]
    fn collisions_stop_convergence() {
        let mut simulation =
            Simulation::new(SphereMesh::icosphere(4), 2, TectonicParams::default(), 5);
        // All crust continental: the boundary is a collision zone.
        for plate in &mut simulation.plates {
            for crust in plate.crust.iter_mut().flatten() {
                *crust = Crust {
                    thickness: 35_000.0,
                    density: crate::isostasy::CONTINENTAL_DENSITY,
                    birth: -1000.0,
                    sediment: 0.0,
                };
            }
        }
        // The mantle drives the plates into each other: it flows towards the boundary from both
        // sides, approximated by flowing towards plate 1's interior.
        let target = simulation.mesh.vertex(
            (0..simulation.mesh.len() as u32)
                .find(|&cell| simulation.owners[cell as usize].plate == 1)
                .unwrap(),
        );
        let mantle: Vec<DVec3> = (0..simulation.mesh.len() as u32)
            .map(|cell| {
                let r = simulation.mesh.vertex(cell);
                let towards = target - r * target.dot(r);
                towards.normalize_or_zero() * 3e-9
            })
            .collect();
        simulation.update_plate_velocities(&mantle);
        let boundaries = simulation.boundary_cells();
        let convergence: Vec<f64> = boundaries
            .iter()
            .filter(|b| b.kind == BoundaryKind::Collision)
            .map(|b| {
                let r = simulation.mesh.vertex(b.cell);
                (simulation.plates[b.plate as usize].angular_velocity
                    - simulation.plates[b.other as usize].angular_velocity)
                    .cross(r)
                    .dot(b.normal)
            })
            .collect();
        assert!(!convergence.is_empty());
        let mean = convergence.iter().sum::<f64>() / convergence.len() as f64;
        // Without resistance, the plates would converge at roughly the mantle speed
        // (3e-9 m/s ≈ 0.015 rad/Myr). Collision resistance reduces this to a small fraction.
        assert!(mean < 0.002, "{mean}");
    }

    /// Two plates, all oceanic crust, plate 0 would subduct under plate 1.
    fn two_oceanic_plates(slab_length: f64) -> Simulation {
        let mut simulation =
            Simulation::new(SphereMesh::icosphere(4), 2, TectonicParams::default(), 5);
        simulation.polarities.clear();
        simulation.polarities.insert(
            pair_key(0, 1),
            Polarity {
                overriding: 1,
                last_contact: 0.0,
                collision_time: 0.0,
                slab_length,
            },
        );
        for plate in &mut simulation.plates {
            for crust in plate.crust.iter_mut().flatten() {
                *crust = new_oceanic_crust(-100.0);
            }
        }
        simulation
    }

    /// Makes the plates move apart (`direction` = 1) or together (`direction` = -1) by rotating
    /// them in opposite senses about an axis through the boundary.
    fn set_relative_motion(simulation: &mut Simulation, direction: f64) {
        let boundary: Vec<BoundaryCell> = simulation.boundary_cells();
        let b = boundary.iter().find(|b| b.plate == 0).unwrap();
        let r = simulation.mesh.vertex(b.cell);
        // ω = r × n moves points at r along n.
        let axis = r.cross(b.normal).normalize();
        simulation.plates[0].angular_velocity = -axis * 0.01 * direction;
        simulation.plates[1].angular_velocity = axis * 0.01 * direction;
    }

    #[test]
    fn diverging_boundaries_are_not_trenches_despite_a_polarity() {
        let mut simulation = two_oceanic_plates(600e3);
        set_relative_motion(&mut simulation, 1.0);
        let boundaries = simulation.boundary_cells();
        // Where the plates diverge there must be no trench.
        assert!(
            boundaries
                .iter()
                .filter(|b| b.convergence < -0.001)
                .all(|b| !matches!(b.kind, BoundaryKind::Trench { .. }))
        );
        set_relative_motion(&mut simulation, -1.0);
        assert!(
            simulation
                .boundary_cells()
                .iter()
                .any(|b| matches!(b.kind, BoundaryKind::Trench { .. }))
        );
    }

    #[test]
    fn slabs_grow_with_convergence_and_decay_without() {
        let mut simulation = two_oceanic_plates(0.0);
        set_relative_motion(&mut simulation, -1.0);
        let mut previous = 0.0;
        let update = |simulation: &mut Simulation, dt| {
            let boundaries = simulation.boundary_cells();
            simulation.update_slab_lengths(dt, &boundaries);
        };
        for _ in 0..5 {
            update(&mut simulation, 1.0);
            let length = simulation.polarities[&(0, 1)].slab_length;
            assert!(length > previous);
            previous = length;
        }
        for _ in 0..200 {
            update(&mut simulation, 1.0);
        }
        assert_eq!(
            simulation.polarities[&(0, 1)].slab_length,
            simulation.params.max_slab_length
        );
        // Subduction stops: the plates come to rest.
        for plate in &mut simulation.plates {
            plate.angular_velocity = DVec3::ZERO;
        }
        update(&mut simulation, 10.0);
        let decayed = simulation.polarities[&(0, 1)].slab_length;
        assert!(decayed < 0.5 * simulation.params.max_slab_length);
    }
}
