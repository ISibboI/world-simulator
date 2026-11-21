//! Plate dynamics: plate velocities from a balance of the torques acting on each plate.
//!
//! The mantle is so viscous that inertia is negligible: at every moment, the forces on each plate
//! balance. The forces considered are
//!
//! * **slab pull** along trenches, where the subducting slab pulls its plate towards the trench.
//!   It grows with the thickness of the slab, i.e. with the square root of its age;
//! * **ridge push** on the young flanks of spreading ridges, away from the ridge;
//! * **slab resistance** at trenches: the mantle resists the sinking of the slab and the bending
//!   of the plate into the trench, which opposes the motion of the subducting plate towards the
//!   trench relative to the mantle (Forsyth & Uyeda 1975, Conrad & Hager 1999). It limits the
//!   speed that slab pull alone can give a plate, also a small one;
//! * **basal drag** between each plate and the mantle below it, `τ = c (v_mantle − v_plate)`,
//!   stronger under thick continental keels;
//! * **collision resistance** where continental crust of two plates converges, which opposes the
//!   relative motion of the plates across the boundary;
//! * **boundary shear coupling**: plate boundaries are weak shear zones through the lithosphere,
//!   which resist strike-slip motion along them (transform faults, oblique ridges and trenches,
//!   continental strike-slip faults; equally at all boundaries, since continental transforms are
//!   as weak as oceanic ones, ~10¹¹–10¹² N/m). Opening and closing across a boundary are not
//!   resisted by it.
//!
//! Drag and the resistances are linear in the plate velocities, so the torque balance of all
//! plates is one linear system for their angular velocities, solved directly. Collision
//! resistance only acts where the plates actually converge, which is resolved by re-solving with
//! the converging contacts only (active set).

use crate::math;
use glam::{DMat3, DVec3};

use super::*;

/// Seconds per million years.
const SECONDS_PER_MYR: f64 = 1e6 * 365.25 * 24.0 * 3600.0;

/// Maximum number of solves for the set of converging collision contacts and the resisting
/// forces of subduction initiation. The damped iteration for a yielding contact alone needs
/// about ten; contacts that influence each other need more.
const MAX_ACTIVE_SET_ITERATIONS: usize = 100;

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
    /// The plate is the lower plate of a diffuse convergent zone (no through-going thrust yet,
    /// see [`super::polarity::Segment::localised`]) and converges: the convergence is taken up by
    /// distributed shortening, resisted by the strength of the plate.
    Diffuse,
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
    /// Id of the boundary segment of the pair covering the cell, if any (see
    /// [`super::polarity`]).
    pub(super) segment: Option<u32>,
}

impl Simulation {
    /// Finds all boundary cells and classifies them with the current plate velocities.
    ///
    /// The boundary normal of a cell is the smoothed one (see [`Self::smoothed_normals`]): the
    /// direction to the neighbouring cells of the other plate zig-zags by ±30° and more around
    /// the boundary, which made strike-slip contacts look partly converging (spurious trenches)
    /// and partly opening (spurious ridges).
    pub(super) fn boundary_cells(&self) -> Vec<BoundaryCell> {
        let params = &self.params;
        let min_convergence = params.min_trench_convergence / (params.planet_radius * 100.0 / 1e6);
        let mut result = Vec::new();
        // Per boundary cell: a neighbouring cell of the other plate.
        let mut neighbors = Vec::new();
        let rotations = self.rotations();
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
            for &(other, direction, neighbor) in &towards {
                // Project onto the tangent plane.
                let normal = (direction - position * direction.dot(position)).normalize_or_zero();
                if normal == DVec3::ZERO {
                    continue;
                }
                result.push(BoundaryCell {
                    cell,
                    plate: owner.plate,
                    other,
                    normal,
                    convergence: 0.0,
                    kind: BoundaryKind::Other,
                    segment: None,
                });
                neighbors.push(neighbor);
            }
        }

        let normals = self.smoothed_normals(&result);
        for ((boundary, smoothed), &neighbor) in result.iter_mut().zip(normals).zip(&neighbors) {
            // Slivers and one-cell necks, where the centroids of the two plates lie on the wrong
            // sides, keep the direction to the neighbours.
            if smoothed.dot(boundary.normal) > 0.0 {
                boundary.normal = smoothed;
            }
            let position = self.mesh.vertex(boundary.cell);
            let crust = self.surface_crust(boundary.cell);
            let other_crust = self.surface_crust(neighbor);
            let relative = (self.plates[boundary.plate as usize].angular_velocity
                - self.plates[boundary.other as usize].angular_velocity)
                .cross(position);
            let convergence = relative.dot(boundary.normal);
            let segment =
                self.polarities
                    .segment_at(boundary.plate, boundary.other, position, &rotations);
            // A polarity only says which plate would subduct if the plates converge. A trench
            // needs a segment that subducts (converging, not nearly along strike) and convergence
            // at the cell, or an existing slab where the plates do not clearly diverge
            // (hysteresis: a slab keeps pulling while the plates are nearly at rest, and while it
            // decays after subduction stopped).
            let subducts_here = segment.is_some_and(|segment| {
                segment.overriding == boundary.other
                    && segment.localised
                    && ((segment.subducting && convergence > min_convergence)
                        || (segment.slab_length > 0.0 && convergence > -min_convergence))
            });

            boundary.kind = if crust.resists_subduction() && other_crust.resists_subduction() {
                BoundaryKind::Collision
            } else if subducts_here && !crust.resists_subduction() {
                BoundaryKind::Trench {
                    slab_age: crust.age(self.time),
                }
            } else if !crust.resists_subduction()
                && segment.is_some_and(|segment| {
                    // A diffuse zone that converges across the segment (see
                    // `polarity::Segment::subducting`) but has no thrust yet.
                    segment.overriding == boundary.other && !segment.localised && segment.subducting
                })
            {
                BoundaryKind::Diffuse
            } else if convergence < 0.0
                && !crust.is_continental()
                && crust.age(self.time) <= params.ridge_age
            {
                BoundaryKind::Ridge
            } else {
                BoundaryKind::Other
            };
            boundary.convergence = convergence;
            boundary.segment = segment.map(|segment| segment.id);
        }
        result
    }

    /// Evaluates the motion across every boundary segment with the current plate velocities (see
    /// [`polarity::Segment::rate`]) and whether its lower plate subducts there (see
    /// [`polarity::Segment::subducting`]). `boundaries` are the current [`Self::boundary_cells`].
    /// Returns whether any segment started or stopped subducting. With `record`, segments whose
    /// slab was decaying and that subduct again are counted (once per step: the velocity solve
    /// records, the slab update does not).
    pub(super) fn update_segment_motion(
        &mut self,
        boundaries: &[BoundaryCell],
        record: bool,
    ) -> bool {
        let params = &self.params;
        let min_convergence = params.min_trench_convergence / (params.planet_radius * 100.0 / 1e6);
        let yield_rate = params.diffuse_convergence / (params.planet_radius * 100.0 / 1e6);
        // Per segment id: Σ normal convergence, Σ |strike-slip|, cells, converging cells, cells
        // converging faster than a diffuse zone creeps.
        let mut sums: HashMap<u32, (f64, f64, usize, usize, usize)> = HashMap::default();
        for boundary in boundaries {
            let Some(id) = boundary.segment else {
                continue;
            };
            let Some(segment) = self.polarities.segment(boundary.plate, boundary.other, id) else {
                continue;
            };
            if segment.overriding != boundary.other
                || self.surface_crust(boundary.cell).resists_subduction()
            {
                continue;
            }
            let r = self.mesh.vertex(boundary.cell);
            let relative = (self.plates[boundary.plate as usize].angular_velocity
                - self.plates[boundary.other as usize].angular_velocity)
                .cross(r);
            let entry = sums.entry(id).or_default();
            let convergence = relative.dot(boundary.normal);
            entry.0 += convergence;
            entry.1 += relative.dot(r.cross(boundary.normal)).abs();
            entry.2 += 1;
            entry.3 += usize::from(convergence > min_convergence);
            entry.4 += usize::from(convergence > yield_rate);
        }
        let obliquity = math::tan(params.min_trench_obliquity.to_radians());
        let polarity::Polarities {
            pairs, statistics, ..
        } = &mut self.polarities;
        let mut changed = false;
        for segment in pairs.values_mut().flat_map(|pair| pair.segments.iter_mut()) {
            let (rate, strike_rate, yielding) = match sums.get(&segment.id) {
                Some(&(normal, strike, count, converging, fast)) if count > 0 => (
                    normal / count as f64,
                    strike / count as f64,
                    // Where the converging part of a diffuse zone converges faster than it can
                    // creep, it has reached its strength (an oblique zone may converge along
                    // part of its length only).
                    2 * fast > converging,
                ),
                _ => (0.0, 0.0, false),
            };
            let subducting = rate > min_convergence && rate >= obliquity * strike_rate;
            if record && subducting && !segment.subducting && segment.slab_length > 0.0 {
                statistics.resumed_subductions += 1;
            }
            changed |= subducting != segment.subducting;
            segment.rate = rate;
            segment.strike_rate = strike_rate;
            segment.subducting = subducting;
            segment.yielding = subducting && yielding;
        }
        changed
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
            // Slab resistance −k ((v_p − v_mantle) · n) n at R r gives the torque
            // −k R² m mᵀ ω + k R (v_mantle · n) m with m = r × n.
            let resistance = self.slab_resistance(boundary);
            if resistance > 0.0 {
                let m = r.cross(boundary.normal);
                drag[boundary.plate as usize] += outer(m, m) * (resistance * radius * radius);
                driving[boundary.plate as usize] += m
                    * (resistance
                        * radius
                        * mantle_velocity[boundary.cell as usize].dot(boundary.normal));
            }
        }

        let size = 3 * plate_count;
        let mut base = vec![0.0; size * size];
        let mut rhs = vec![0.0; size];
        for plate in 0..plate_count {
            // A small regularisation (like the drag under a millionth of a cell) keeps plates
            // without cells, or with a single cell (free to spin about its own axis), solvable.
            let block = drag[plate]
                + DMat3::IDENTITY * (1e-6 * params.basal_drag * cell_area * radius * radius);
            add_block(&mut base, size, plate, plate, block);
            for i in 0..3 {
                rhs[3 * plate + i] = driving[plate][i];
            }
        }

        // Boundary shear coupling: the force −k ((v_p − v_q) · t) t on p (and the opposite on q)
        // with the strike t = r × n gives, since (ω × r) · t = −ω · n, the torque
        // −k R² n nᵀ (ω_p − ω_q). Each side of a boundary contributes half, with the (smoothed)
        // boundary normal n of its cell.
        let length_per_cell = self.boundary_length_per_cell();
        let normals: Vec<DVec3> = boundaries.iter().map(|boundary| boundary.normal).collect();
        for (boundary, &normal) in boundaries.iter().zip(&normals) {
            let k = 0.5 * params.basal_drag * params.boundary_coupling_width * length_per_cell;
            if k <= 0.0 {
                continue;
            }
            let block = outer(normal, normal) * (k * radius * radius);
            let (p, q) = (boundary.plate as usize, boundary.other as usize);
            add_block(&mut base, size, p, p, block);
            add_block(&mut base, size, q, q, block);
            add_block(&mut base, size, p, q, -block);
            add_block(&mut base, size, q, p, -block);
        }

        // Collision resistance: the force on the plate −k ((v_p − v_q) · n) n at R r gives the
        // torque −k R² m mᵀ (ω_p − ω_q) with m = r × n. Per boundary cell, it is as strong as the
        // basal drag under `collision_resistance` cells.
        let coupling = params.collision_resistance * params.basal_drag * cell_area;
        // With the smoothed normals (see `boundary_cells`): the raw ones scatter by ~20°, which
        // made strike-slip contacts look half converging and resisted along strike.
        let collisions: Vec<(&BoundaryCell, DVec3)> = boundaries
            .iter()
            .zip(&normals)
            .filter(|(boundary, _)| boundary.kind == BoundaryKind::Collision)
            .map(|(boundary, &normal)| (boundary, normal))
            .collect();
        let mut active = vec![true; collisions.len()];

        // Subduction initiation resistance: a plate that has not yet built a slab resists being
        // bent and thrust under its neighbour with a plastic strength (see
        // [`TectonicParams::initiation_resistance`]). The force opposes the convergence with a
        // fixed magnitude; in the linear system it is an effective viscous coupling
        // `strength / speed` from the last solve (Picard iteration), capped for slow contacts,
        // which are locked.
        // Contacts slower than their lock speed (m/s) are coupled viscously: localised thrusts
        // are locked below `locked_convergence`, diffuse zones creep up to
        // `diffuse_convergence` (see [`TectonicParams::diffuse_convergence`]).
        let to_m_per_s = |cm_per_yr: f64| cm_per_yr / 100.0 / (SECONDS_PER_MYR / 1e6);
        let (locked_speed, diffuse_speed) = (
            to_m_per_s(params.locked_convergence),
            to_m_per_s(params.diffuse_convergence),
        );
        let nascent: Vec<(&BoundaryCell, DVec3, f64, f64)> = boundaries
            .iter()
            .zip(&normals)
            .filter_map(|(boundary, &normal)| {
                // Thickened collision belts resist further convergence with their excess
                // potential energy, also a plastic force (see `Simulation::belt_resistance`).
                let belt = if boundary.kind == BoundaryKind::Collision {
                    self.belt_resistance(boundary.cell)
                } else {
                    0.0
                };
                let strength = self.initiation_strength(boundary) + belt;
                // A collision belt is a continental zone, not an oceanic diffuse one: below
                // `locked_convergence` its resistance falls linearly with the speed.
                let lock = if boundary.kind != BoundaryKind::Collision
                    && self.is_diffuse_contact(boundary)
                {
                    diffuse_speed
                } else {
                    locked_speed
                };
                (strength > 0.0).then_some((boundary, normal, strength, lock))
            })
            .collect();
        let mut nascent_coupling: Vec<f64> = nascent
            .iter()
            .map(|&(_, _, strength, lock)| strength / lock)
            .collect();

        let mut omega = vec![DVec3::ZERO; plate_count];
        let mut converged = false;
        for _ in 0..MAX_ACTIVE_SET_ITERATIONS {
            let mut matrix = base.clone();
            for (&(boundary, normal, _, _), &coupling) in nascent.iter().zip(&nascent_coupling) {
                if coupling <= 0.0 {
                    continue;
                }
                let r = self.mesh.vertex(boundary.cell);
                let m = r.cross(normal);
                let block = outer(m, m) * (coupling * radius * radius);
                let (p, q) = (boundary.plate as usize, boundary.other as usize);
                add_block(&mut matrix, size, p, p, block);
                add_block(&mut matrix, size, q, q, block);
                add_block(&mut matrix, size, p, q, -block);
                add_block(&mut matrix, size, q, p, -block);
            }
            for ((boundary, normal), _) in collisions
                .iter()
                .zip(&active)
                .filter(|(_, active)| **active)
            {
                let r = self.mesh.vertex(boundary.cell);
                let m = r.cross(*normal);
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

            let solution = solve_linear_system(matrix, rhs.clone(), size);
            for (plate, omega) in omega.iter_mut().enumerate() {
                *omega = DVec3::new(
                    solution[3 * plate],
                    solution[3 * plate + 1],
                    solution[3 * plate + 2],
                );
            }

            // Collision resistance only acts where the continents converge.
            let mut changed = false;
            for ((boundary, normal), active) in collisions.iter().zip(active.iter_mut()) {
                let r = self.mesh.vertex(boundary.cell);
                let relative = (omega[boundary.plate as usize] - omega[boundary.other as usize])
                    .cross(r)
                    .dot(*normal);
                let converging = relative > 0.0;
                if converging != *active {
                    *active = converging;
                    changed = true;
                }
            }
            // The effective coupling of the yielding contacts follows the new speeds: the
            // resisting force is the strength where the contact converges (less where it is
            // locked) and nothing where it opens.
            for (&(boundary, normal, strength, lock), coupling) in
                nascent.iter().zip(nascent_coupling.iter_mut())
            {
                let r = self.mesh.vertex(boundary.cell);
                let speed = (omega[boundary.plate as usize] - omega[boundary.other as usize])
                    .cross(r)
                    .dot(normal)
                    * radius;
                let target = if speed > 0.0 {
                    strength / speed.max(lock)
                } else {
                    0.0
                };
                // The force of the solve just done against the force wanted.
                let force = *coupling * speed.max(0.0);
                let wanted = target * speed.max(0.0);
                let opening_but_coupled = speed <= 0.0 && *coupling > 0.0;
                if (force - wanted).abs() > 0.05 * strength || opening_but_coupled {
                    changed = true;
                    // Damped, so that the iteration does not flip between locked and free;
                    // an opening contact is released at once.
                    *coupling = if speed > 0.0 {
                        0.5 * (*coupling + target)
                    } else {
                        0.0
                    };
                }
            }
            if !changed {
                converged = true;
                break;
            }
        }
        if !converged {
            self.unsettled_solves += 1;
            tracing::debug!(
                time = self.time,
                "Collision and subduction resistance did not converge"
            );
        }

        for (plate, omega) in self.plates.iter_mut().zip(omega) {
            plate.angular_velocity = omega * SECONDS_PER_MYR;
        }
        self.update_segment_motion(&boundaries, true);
    }

    /// Whether a boundary cell belongs to a diffuse contact: no segment yet, or a segment that
    /// has not localised (see [`super::polarity::Segment::localised`]).
    fn is_diffuse_contact(&self, boundary: &BoundaryCell) -> bool {
        boundary
            .segment
            .and_then(|id| self.polarities.segment(boundary.plate, boundary.other, id))
            .is_none_or(|segment| !segment.localised)
    }
}

impl Simulation {
    /// The force (N, tangent to the sphere) acting on the plate at a boundary cell: slab pull at
    /// trenches, growing with the age of the slab (lithosphere thickness ∝ √age) and with the
    /// length of slab that has already sunk; ridge push at ridges.
    pub(super) fn boundary_force(&self, boundary: &BoundaryCell) -> DVec3 {
        let params = &self.params;
        let boundary_length = self.boundary_length_per_cell();
        match boundary.kind {
            BoundaryKind::Trench { slab_age } => {
                let buoyancy = self.slab_buoyancy(slab_age, self.slab_length(boundary));
                boundary.normal * (params.slab_pull * buoyancy.max(0.0) * boundary_length)
            }
            BoundaryKind::Ridge => -boundary.normal * (params.ridge_push * boundary_length),
            BoundaryKind::Collision | BoundaryKind::Diffuse | BoundaryKind::Other => DVec3::ZERO,
        }
    }

    /// Boundary normals (unit, tangent, from the plate towards the other plate) smoothed over a
    /// few rings: the direction between the centroids of the two plates' cells near each
    /// boundary cell. Single mesh edges zig-zag at ±30° or more around the true normal.
    pub(super) fn smoothed_normals(&self, boundaries: &[BoundaryCell]) -> Vec<DVec3> {
        const RINGS: usize = 3;
        let mut stamp = vec![u32::MAX; self.mesh.len()];
        let (mut frontier, mut next) = (Vec::new(), Vec::new());
        boundaries
            .iter()
            .enumerate()
            .map(|(index, boundary)| {
                let index = index as u32;
                let (mut own, mut other) = (DVec3::ZERO, DVec3::ZERO);
                frontier.clear();
                frontier.push(boundary.cell);
                stamp[boundary.cell as usize] = index;
                for _ in 0..=RINGS {
                    next.clear();
                    for &cell in &frontier {
                        let plate = self.owners[cell as usize].plate;
                        if plate == boundary.plate {
                            own += self.mesh.vertex(cell);
                        } else if plate == boundary.other {
                            other += self.mesh.vertex(cell);
                        }
                        for &neighbor in self.mesh.neighbors(cell) {
                            if stamp[neighbor as usize] != index {
                                stamp[neighbor as usize] = index;
                                next.push(neighbor);
                            }
                        }
                    }
                    std::mem::swap(&mut frontier, &mut next);
                }
                let r = self.mesh.vertex(boundary.cell);
                let direction = other.normalize_or_zero() - own.normalize_or_zero();
                let tangent = direction - r * direction.dot(r);
                tangent.try_normalize().unwrap_or(boundary.normal)
            })
            .collect()
    }

    /// Slab resistance coefficient (N·s/m) at a boundary cell: at trenches, the force opposing
    /// the motion of the plate towards the trench relative to the mantle per unit of that speed.
    /// It grows with the length of the sunken slab like slab pull, so that full slab pull of
    /// reference-age lithosphere alone moves a plate at [`TectonicParams::slab_sinking_speed`].
    pub(super) fn slab_resistance(&self, boundary: &BoundaryCell) -> f64 {
        let params = &self.params;
        if !matches!(boundary.kind, BoundaryKind::Trench { .. }) {
            return 0.0;
        }
        let boundary_length = self.boundary_length_per_cell();
        let length_factor = self.slab_length_factor(boundary);
        // cm/yr → m/s.
        let speed = params.slab_sinking_speed / 100.0 / (SECONDS_PER_MYR / 1e6);
        params.slab_pull * length_factor * boundary_length / speed
    }

    /// Net negative buoyancy of a slab of the given age (Myr) and length (m), relative to a full
    /// slab of the reference age (which pulls with [`TectonicParams::slab_pull`]). The thermal
    /// contraction of the lithosphere grows with the square root of its age and acts along the
    /// whole slab. Against it stands the buoyancy of the basaltic crust and the depleted mantle
    /// below it, which makes lithosphere younger than
    /// [`TectonicParams::neutral_buoyancy_age`] lighter than the asthenosphere (Cloos 1993;
    /// Oxburgh & Parmentier 1977). The crust only floats down to ~80–100 km depth, where it
    /// turns into dense eclogite, so this term acts on the shallow part of the slab only (the
    /// first [`TectonicParams::self_sustaining_slab_length`]). Negative values mean a buoyant
    /// slab, which resists being pushed down.
    pub(super) fn slab_buoyancy(&self, slab_age: f64, slab_length: f64) -> f64 {
        let params = &self.params;
        let thermal = |age: f64| (age.max(0.0) / params.slab_pull_reference_age).sqrt();
        let fraction = |length: f64| (length / params.max_slab_length).clamp(0.0, 1.0);
        let crust = thermal(params.neutral_buoyancy_age);
        let shallow = params.self_sustaining_slab_length;
        (thermal(slab_age.min(params.slab_pull_max_age)) * fraction(slab_length)
            - crust * fraction(slab_length.min(shallow)))
            / (1.0 - crust * fraction(shallow))
    }

    /// Strength (N per boundary cell) with which the plate of a boundary cell resists being
    /// thrust under the other plate there, as a force of fixed magnitude that opposes
    /// convergence:
    ///
    /// * where the plate would subduct but its thrust is not yet self-sustaining,
    ///   [`TectonicParams::initiation_resistance`] per metre, fading as the displacement on the
    ///   thrust grows to [`TectonicParams::self_sustaining_slab_length`] (the plate must be
    ///   bent and a thrust must break through it; see
    ///   [`super::polarity::Segment::displacement`]); in full at diffuse zones. This holds
    ///   whether or not the contact converges at the moment, and also where the plates have
    ///   not overlapped yet (no segment; then each side's oceanic cells carry half);
    /// * at trenches, the buoyancy of a slab of young lithosphere (see
    ///   [`Self::slab_buoyancy`]).
    pub(super) fn initiation_strength(&self, boundary: &BoundaryCell) -> f64 {
        let params = &self.params;
        let boundary_length = self.boundary_length_per_cell();
        let segment = boundary
            .segment
            .and_then(|id| self.polarities.segment(boundary.plate, boundary.other, id));
        let slab_length = segment.map_or(0.0, |segment| segment.slab_length);
        // The strength fades with the convergence taken up on the thrust, not with the slab:
        // a slab seeded by an initiation event pulls from the start, but its thrust still has to
        // break through the plate.
        let displacement = segment.map_or(0.0, |segment| segment.displacement);
        let undeveloped = 1.0 - (displacement / params.self_sustaining_slab_length).min(1.0);
        let strength = match (boundary.kind, segment) {
            (BoundaryKind::Trench { slab_age }, _) => {
                params.initiation_resistance * undeveloped
                    + params.slab_pull * (-self.slab_buoyancy(slab_age, slab_length)).max(0.0)
            }
            (BoundaryKind::Diffuse | BoundaryKind::Other, Some(segment))
                if segment.overriding == boundary.other =>
            {
                if self.surface_crust(boundary.cell).resists_subduction() {
                    0.0
                } else {
                    params.initiation_resistance * undeveloped
                }
            }
            (BoundaryKind::Other, None) => {
                if self.surface_crust(boundary.cell).resists_subduction() {
                    0.0
                } else {
                    0.5 * params.initiation_resistance
                }
            }
            _ => 0.0,
        };
        strength * boundary_length
    }

    /// Length (m) of the slab below the trench at a boundary cell.
    pub(super) fn slab_length(&self, boundary: &BoundaryCell) -> f64 {
        boundary
            .segment
            .and_then(|id| self.polarities.segment(boundary.plate, boundary.other, id))
            .map_or(0.0, |segment| segment.slab_length)
    }

    /// Fraction of the full slab length that has sunk below the trench at a boundary cell.
    fn slab_length_factor(&self, boundary: &BoundaryCell) -> f64 {
        let slab_length = boundary
            .segment
            .and_then(|id| self.polarities.segment(boundary.plate, boundary.other, id))
            .map_or(0.0, |segment| segment.slab_length);
        (slab_length / self.params.max_slab_length).clamp(0.0, 1.0)
    }

    /// Updates the length of slab below each subduction zone: it grows with the convergence
    /// across the segment where its lower plate subducts (up to the length of an upper-mantle
    /// slab) and decays where it does not (the slab sinks or breaks off; below
    /// [`TectonicParams::min_slab_length`] it is gone). The convergence is the segment's net
    /// motion over its lower plate's boundary cells (see [`Self::update_segment_motion`]), so
    /// that a contact moving along strike does not keep a slab through cells whose normal happens
    /// to point into the motion. `boundaries` are the current [`Self::boundary_cells`].
    ///
    /// Diffuse convergent zones that yield localise (see
    /// [`TectonicParams::diffuse_convergence`]); the others accumulate shortening. Slabs grow
    /// only on localised segments, and so does the displacement on their thrust (see
    /// [`super::polarity::Segment::displacement`]); a localised segment stays localised until it
    /// expires.
    ///
    /// Returns whether a slab appeared or disappeared, a segment started or stopped
    /// subducting, or a zone localised, which changes the classification of boundary cells (see
    /// [`Self::boundary_cells`]).
    pub(super) fn update_slab_lengths(&mut self, dt: f64, boundaries: &[BoundaryCell]) -> bool {
        let mut slabs_changed = self.update_segment_motion(boundaries, false);
        // Segments with boundary cells of their lower plate (subducting or not) are in contact.
        let in_contact: HashSet<u32> = boundaries
            .iter()
            .filter_map(|boundary| {
                let id = boundary.segment?;
                let segment = self
                    .polarities
                    .segment(boundary.plate, boundary.other, id)?;
                (segment.overriding == boundary.other).then_some(id)
            })
            .collect();
        let params = &self.params;
        let decay = math::exp(-dt / params.slab_decay_time);
        let polarity::Polarities {
            pairs, statistics, ..
        } = &mut self.polarities;
        // Summed in a defined order below (hash-map order differs between platforms).
        let mut transient_stacked = Vec::new();
        for pair in pairs.values_mut() {
            for segment in &mut pair.segments {
                let had_slab = segment.slab_length > 0.0;
                // Trench fill: the mass-weighted mean sediment thickness subducted here,
                // averaged over time (samples are consumed here only now and then).
                let (sediment, squares) = std::mem::take(&mut segment.incoming);
                if sediment > 0.0 {
                    let elapsed = self.time - segment.fill_time;
                    let rate = 1.0 - math::exp(-elapsed.max(dt) / params.trench_fill_time);
                    segment.fill += (squares / sediment - segment.fill) * rate;
                    segment.fill_time = self.time;
                }
                if in_contact.contains(&segment.id) {
                    // Accretionary or erosive margin, with hysteresis between the two
                    // convergence thresholds (rad/Myr → cm/yr).
                    let rate = segment.rate * params.planet_radius * 1e-4;
                    if segment.fill <= params.subduction_channel_thickness
                        || rate > params.erosive_convergence
                    {
                        segment.accretionary = false;
                    } else if rate < params.accretionary_convergence {
                        segment.accretionary = true;
                    }
                    // A boundary keeps the segment in contact, also without overlap.
                    segment.last_contact = segment.last_contact.max(self.time);
                }
                if segment.subducting && segment.localised {
                    let growth = segment.rate * params.planet_radius * dt;
                    segment.slab_length =
                        (segment.slab_length + growth).min(params.max_slab_length);
                    // Slip on the thrust, by the smoothed normals of the force balance.
                    segment.displacement =
                        (segment.displacement + growth).min(params.max_slab_length);
                    polarity::Polarities::note_slab_growth(statistics, segment);
                } else {
                    segment.slab_length *= decay;
                    // A slab remnant this short has broken off: the trench is gone.
                    if segment.slab_length < params.min_slab_length {
                        segment.slab_length = 0.0;
                    }
                }
                if !segment.localised && segment.yielding {
                    // A diffuse zone that converges faster than it can creep has reached its
                    // strength and yields: it localises on a thrust (see
                    // `TectonicParams::diffuse_convergence`).
                    segment.localised = true;
                    slabs_changed = true;
                    statistics.localisations += 1;
                    if self.time - segment.created <= 2.0 {
                        transient_stacked.push(segment.stacked);
                    }
                } else if !segment.localised {
                    segment.shortening += segment.rate.max(0.0) * params.planet_radius * dt;
                }
                slabs_changed |= had_slab != (segment.slab_length > 0.0);
            }
        }
        transient_stacked.sort_by(f64::total_cmp);
        for stacked in transient_stacked {
            statistics.transient_stacked += stacked;
        }
        slabs_changed
    }

    /// Positions of trench cells with their subduction rate (convergence speed times boundary
    /// length, in m²/s), which feed the mantle downwellings. `boundaries` are the current
    /// [`Self::boundary_cells`].
    pub(super) fn subduction_rates(&self, boundaries: &[BoundaryCell]) -> Vec<(DVec3, f64)> {
        let boundary_length = self.boundary_length_per_cell();
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

impl Simulation {
    /// Where the slabs of the trench cells sink, with their mass flux in kg/s (see
    /// [`mantle::MantleFlow::dynamic_topography`]). The slab sinks under the overriding plate,
    /// [`MantleParams::slab_offset`] from the trench. Its excess mass per area is that of the
    /// cooled oceanic lithosphere, which the depth–age curve expresses isostatically:
    /// `(ρ_m − ρ_w) (d(age) − d(0))`, ~7 × 10⁶ kg/m² for old plates (cf. 2 ρ α ΔT √(κ t / π)).
    pub(super) fn sinking_slabs(&self, boundaries: &[BoundaryCell]) -> Vec<(DVec3, f64)> {
        let boundary_length = self.boundary_length_per_cell();
        let offset = self.params.mantle.slab_offset / self.params.planet_radius;
        boundaries
            .iter()
            .filter_map(|boundary| {
                let BoundaryKind::Trench { slab_age } = boundary.kind else {
                    return None;
                };
                let r = self.mesh.vertex(boundary.cell);
                let relative = (self.plates[boundary.plate as usize].angular_velocity
                    - self.plates[boundary.other as usize].angular_velocity)
                    .cross(r)
                    .dot(boundary.normal)
                    * self.params.planet_radius
                    / SECONDS_PER_MYR;
                let excess = (crate::isostasy::MANTLE_DENSITY - crate::isostasy::WATER_DENSITY)
                    * (crust::oceanic_depth(0.0) - crust::oceanic_depth(slab_age));
                let position =
                    (r * math::cos(offset) + boundary.normal * math::sin(offset)).normalize();
                (relative > 0.0).then_some((position, relative * boundary_length * excess))
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

    /// Two plates; plate 0 subducts under plate 1 along the +x part of its boundary.
    fn one_sided_trench(params: TectonicParams) -> Simulation {
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 2, params, 5);
        for plate in &mut simulation.plates {
            plate.angular_velocity = DVec3::ZERO;
        }
        // Plate 0 subducts under plate 1.
        simulation.polarities.clear();
        simulation.seed_subduction((0, 1), 1, 600e3);
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
                            felsic: 1.0,
                            metasediment: 0.0,
                            orogeny: NO_OROGENY,
                            orogenic: 0.0,
                            reheated: NO_REHEATING,
                            cohort: 0.0,
                        }
                    } else {
                        new_oceanic_crust(-100.0)
                    };
                }
            }
        }
        simulation
    }

    /// Mean speed (rad/Myr) of plate 0 towards plate 1 at the trench cells.
    fn mean_trench_convergence(simulation: &Simulation) -> f64 {
        let boundaries = simulation.boundary_cells();
        let trenches: Vec<_> = boundaries
            .iter()
            .filter(|b| matches!(b.kind, BoundaryKind::Trench { .. }))
            .collect();
        assert!(!trenches.is_empty());
        trenches
            .iter()
            .map(|b| {
                let r = simulation.mesh.vertex(b.cell);
                (simulation.plates[0].angular_velocity - simulation.plates[1].angular_velocity)
                    .cross(r)
                    .dot(b.normal)
            })
            .sum::<f64>()
            / trenches.len() as f64
    }

    #[test]
    fn slab_resistance_limits_the_speed_towards_the_trench() {
        let mut params = TectonicParams::default();
        // Hardly any basal drag: only slab resistance limits the motion.
        params.basal_drag *= 1e-4;
        let sinking_speed = params.slab_sinking_speed;
        let mut simulation = one_sided_trench(params.clone());
        let mantle = vec![DVec3::ZERO; simulation.mesh.len()];
        simulation.update_plate_velocities(&mantle);
        // rad/Myr → cm/yr
        let speed = mean_trench_convergence(&simulation) * 637.1;
        assert!(
            (0.3 * sinking_speed..1.2 * sinking_speed).contains(&speed),
            "{speed}"
        );

        params.slab_sinking_speed = 1e6;
        let mut free = one_sided_trench(params);
        free.update_plate_velocities(&mantle);
        assert!(mean_trench_convergence(&free) * 637.1 > 10.0 * speed);
    }

    #[test]
    fn young_lithosphere_is_buoyant_and_old_lithosphere_pulls() {
        let simulation = one_sided_trench(TectonicParams::default());
        let params = &simulation.params;
        let (shallow, full) = (params.self_sustaining_slab_length, params.max_slab_length);
        let neutral = params.neutral_buoyancy_age;
        // The shallow slab: buoyant below the neutral age, pulling above it.
        assert!(simulation.slab_buoyancy(0.0, shallow) < 0.0);
        assert!(simulation.slab_buoyancy(0.5 * neutral, shallow) < 0.0);
        assert!(simulation.slab_buoyancy(neutral, shallow).abs() < 1e-12);
        assert!(simulation.slab_buoyancy(2.0 * neutral, shallow) > 0.0);
        // A full slab of the reference age pulls with the calibrated slab pull, and full slabs
        // of mature lithosphere pull nearly as without the crustal buoyancy (√(age/reference)).
        let reference = params.slab_pull_reference_age;
        assert!((simulation.slab_buoyancy(reference, full) - 1.0).abs() < 1e-12);
        for age in [40.0, 60.0, 80.0] {
            let plain = (age / reference).sqrt();
            let ratio = simulation.slab_buoyancy(age, full) / plain;
            assert!((0.93..1.0).contains(&ratio), "{age}: {ratio}");
        }
        // Its deep part is eclogite: a full slab pulls even when young.
        assert!(simulation.slab_buoyancy(5.0, full) > 0.0);
        // No slab, no force.
        assert_eq!(simulation.slab_buoyancy(5.0, 0.0), 0.0);
        let mut previous = f64::NEG_INFINITY;
        for age in [0.0, 5.0, 10.0, 20.0, 50.0, 100.0] {
            let buoyancy = simulation.slab_buoyancy(age, shallow);
            assert!(buoyancy > previous, "{age}");
            previous = buoyancy;
        }
    }

    /// [`one_sided_trench`] with plate 0's ocean of the given age and slab length, moving as
    /// the mantle below plate 0 drags it at `mantle_speed` cm/yr in the +x direction (the
    /// mantle below plate 1 rests), with the given parameters.
    fn dragged(
        age_and_slab: (f64, f64),
        mantle_speed: f64,
        set: impl Fn(&mut TectonicParams),
    ) -> Simulation {
        dragged_with(age_and_slab, mantle_speed, |_| {}, set)
    }

    /// [`dragged`], with the segments of the trench changed by `segment` first.
    fn dragged_with(
        (age, slab_length): (f64, f64),
        mantle_speed: f64,
        segment: impl Fn(&mut polarity::Segment),
        set: impl Fn(&mut TectonicParams),
    ) -> Simulation {
        let params = TectonicParams {
            initiation_resistance: 0.0,
            slab_pull: 0.0,
            ..TectonicParams::default()
        };
        let mut simulation = one_sided_trench(params);
        simulation.polarities.clear();
        simulation.seed_subduction((0, 1), 1, slab_length);
        for pair in simulation.polarities.pairs.values_mut() {
            pair.segments.iter_mut().for_each(&segment);
        }
        for crust in simulation.plates[0].crust.iter_mut().flatten() {
            if !crust.is_continental() {
                *crust = new_oceanic_crust(-age);
            }
        }
        let mantle: Vec<DVec3> = (0..simulation.mesh.len() as u32)
            .map(|cell| {
                let r = simulation.mesh.vertex(cell);
                if simulation.owners[cell as usize].plate == 0 {
                    (DVec3::X - r * r.x) * mantle_speed / 100.0 / (SECONDS_PER_MYR / 1e6)
                } else {
                    DVec3::ZERO
                }
            })
            .collect();
        // First without pull and resistance, so that the boundary is classified with the
        // plates moving.
        simulation.update_plate_velocities(&mantle);
        set(&mut simulation.params);
        simulation.update_plate_velocities(&mantle);
        simulation
    }

    /// Mean speed (cm/yr) of plate 0 towards plate 1 over plate 0's boundary cells on the +x
    /// side (its oceanic side).
    fn mean_approach(simulation: &Simulation) -> f64 {
        let boundaries = simulation.boundary_cells();
        let normals = simulation.smoothed_normals(&boundaries);
        let cells: Vec<f64> = boundaries
            .iter()
            .zip(&normals)
            .filter(|(b, _)| b.plate == 0 && simulation.mesh.vertex(b.cell).x > 0.0)
            .map(|(b, &normal)| {
                let r = simulation.mesh.vertex(b.cell);
                (simulation.plates[0].angular_velocity - simulation.plates[1].angular_velocity)
                    .cross(r)
                    .dot(normal)
                    * 637.1
            })
            .collect();
        assert!(!cells.is_empty());
        cells.iter().sum::<f64>() / cells.len() as f64
    }

    #[test]
    fn plates_without_a_slab_resist_starting_to_subduct() {
        let strength = TectonicParams::default().initiation_resistance;
        let resist = |params: &mut TectonicParams| params.initiation_resistance = strength;
        let free = |_: &mut TectonicParams| {};
        // Old lithosphere without a slab, pushed by a slow mantle flow: with the resistance it
        // converges much more slowly, and the plate it is pushed against is pushed along.
        let unresisted = dragged((100.0, 0.0), 3.0, free);
        let resisted = dragged((100.0, 0.0), 3.0, resist);
        let (fast, slow) = (mean_approach(&unresisted), mean_approach(&resisted));
        assert!(fast > 0.5, "{fast}");
        assert!(slow < 0.5 * fast, "{slow} {fast}");
        assert!(
            resisted.plates[1].angular_velocity.length()
                > 2.0 * unresisted.plates[1].angular_velocity.length()
        );
        // A much stronger push overcomes the strength: the contact yields, and the resisting
        // force stays at the strength, so that it matters little.
        let (fast, yielding) = (
            mean_approach(&dragged((100.0, 0.0), 60.0, free)),
            mean_approach(&dragged((100.0, 0.0), 60.0, resist)),
        );
        assert!(yielding > 0.8 * fast, "{yielding} {fast}");
        // A self-sustaining slab of old lithosphere does not resist.
        let (fast, mature) = (
            mean_approach(&dragged((100.0, 600e3), 3.0, free)),
            mean_approach(&dragged((100.0, 600e3), 3.0, resist)),
        );
        assert!((mature - fast).abs() < 1e-6 * fast, "{mature} {fast}");
        // Opening is not resisted (the boundary is irregular, so a few of its cells converge
        // even when the plate is dragged away, and those are).
        let (opening, same) = (
            mean_approach(&dragged((100.0, 0.0), -3.0, free)),
            mean_approach(&dragged((100.0, 0.0), -3.0, resist)),
        );
        assert!(opening < -0.5, "{opening}");
        assert!(same < 0.85 * opening, "{same} {opening}");
    }

    #[test]
    fn diffuse_zones_creep_below_their_strength_and_localise_when_they_yield() {
        let strength = TectonicParams::default().initiation_resistance;
        let resist = |params: &mut TectonicParams| params.initiation_resistance = strength;
        let diffuse = |segment: &mut polarity::Segment| segment.localised = false;
        let free = |_: &mut TectonicParams| {};
        let v_d = TectonicParams::default().diffuse_convergence;
        // Pushed below the strength, a localised thrust without a slab is locked, while a diffuse
        // zone creeps by distributed shortening, but no faster than it can creep.
        let unresisted = mean_approach(&dragged((100.0, 0.0), 3.0, free));
        let locked = mean_approach(&dragged((100.0, 0.0), 3.0, resist));
        let mut creeping = dragged_with((100.0, 0.0), 3.0, diffuse, resist);
        let creep = mean_approach(&creeping);
        // (The mean over the boundary includes cells moving obliquely, so the locked thrust's
        // mean is not zero.)
        assert!(creep > 1.5 * locked.max(0.0), "{creep} {locked}");
        assert!(
            creep < v_d && creep < 0.75 * unresisted,
            "{creep} {unresisted}"
        );
        // It keeps shortening without localising or growing a slab.
        for _ in 0..10 {
            let boundaries = creeping.boundary_cells();
            creeping.update_slab_lengths(1.0, &boundaries);
        }
        let boundaries = creeping.boundary_cells();
        assert!(
            boundaries
                .iter()
                .any(|boundary| boundary.kind == BoundaryKind::Diffuse)
        );
        assert!(
            !boundaries
                .iter()
                .any(|boundary| matches!(boundary.kind, BoundaryKind::Trench { .. }))
        );
        assert!(
            creeping
                .polarities
                .segments()
                .all(|segment| !segment.localised && segment.slab_length == 0.0)
        );
        assert!(
            creeping
                .polarities
                .segments()
                .any(|segment| segment.shortening > 0.0)
        );
        // Pushed hard, it reaches the strength, yields and localises: a trench whose slab grows.
        let mut yielding = dragged_with((100.0, 0.0), 60.0, diffuse, resist);
        assert!(mean_approach(&yielding) > v_d);
        let boundaries = yielding.boundary_cells();
        assert!(yielding.update_slab_lengths(1.0, &boundaries));
        assert!(
            yielding
                .polarities
                .segments()
                .any(|segment| segment.localised)
        );
        for _ in 0..3 {
            let boundaries = yielding.boundary_cells();
            yielding.update_slab_lengths(1.0, &boundaries);
        }
        assert!(
            yielding
                .polarities
                .segments()
                .any(|segment| segment.slab_length > 0.0 && segment.displacement > 0.0)
        );
    }

    #[test]
    fn event_slabs_feel_the_initiation_strength() {
        let strength = TectonicParams::default().initiation_resistance;
        let resist = |params: &mut TectonicParams| params.initiation_resistance = strength;
        let free = |_: &mut TectonicParams| {};
        let fresh = |segment: &mut polarity::Segment| segment.displacement = 0.0;
        // A 150 km slab seeded by an initiation event, whose thrust has not moved yet, is
        // resisted like a plate without a slab; once 150 km have been taken up on the thrust it
        // is not.
        let unresisted = mean_approach(&dragged_with((100.0, 150e3), 3.0, fresh, free));
        let event = mean_approach(&dragged_with((100.0, 150e3), 3.0, fresh, resist));
        let established = mean_approach(&dragged((100.0, 150e3), 3.0, resist));
        assert!(event < 0.5 * unresisted, "{event} {unresisted}");
        assert!(
            (established - unresisted).abs() < 1e-6 * unresisted,
            "{established} {unresisted}"
        );
    }

    #[test]
    fn a_short_slab_of_young_lithosphere_resists_being_pushed_down() {
        let pull = TectonicParams::default().slab_pull;
        // A 150 km slab of 2 Myr old lithosphere, with its crustal buoyancy and with none (neutral
        // at its own age, so that it neither pulls nor resists; the slab's viscous resistance is
        // the same in both).
        let buoyant = dragged((2.0, 150e3), 3.0, |params| params.slab_pull = pull);
        let neutral = dragged((2.0, 150e3), 3.0, |params| {
            params.slab_pull = pull;
            params.neutral_buoyancy_age = 2.0;
        });
        let (slow, fast) = (mean_approach(&buoyant), mean_approach(&neutral));
        assert!(slow < 0.8 * fast, "{slow} {fast}");
    }

    #[test]
    fn creeping_contacts_build_no_slab() {
        let mut simulation = one_sided_trench(TectonicParams::default());
        simulation.polarities.clear();
        simulation.seed_subduction((0, 1), 1, 0.0);
        // Plate 0 creeps towards plate 1 at 0.05 cm/yr, below the trench threshold.
        simulation.plates[0].angular_velocity = DVec3::Y * (0.05 / 637.1);
        for _ in 0..50 {
            let boundaries = simulation.boundary_cells();
            simulation.update_slab_lengths(1.0, &boundaries);
        }
        assert!(
            simulation
                .polarities
                .segments()
                .all(|segment| segment.slab_length == 0.0)
        );
    }

    /// 1 cm/yr as an angular speed (rad/Myr) on the default planet.
    const CM_PER_YEAR: f64 = 1.0 / 637.1;

    #[test]
    fn boundary_normals_follow_the_boundary_not_the_mesh() {
        // Pure strike-slip along the zig-zag equator of the icosphere.
        let mut simulation = hemispheres(0.0, 4);
        simulation.plates[0].angular_velocity = DVec3::Z * (5.0 * CM_PER_YEAR);
        simulation.plates[1].angular_velocity = DVec3::ZERO;
        let boundaries = simulation.boundary_cells();
        assert!(!boundaries.is_empty());
        let max = boundaries
            .iter()
            .map(|b| b.convergence.abs())
            .fold(0.0, f64::max);
        let mean =
            boundaries.iter().map(|b| b.convergence.abs()).sum::<f64>() / boundaries.len() as f64;
        // The raw normals (towards the neighbours) are up to ~30° off and more.
        assert!(mean < 0.1 * 5.0 * CM_PER_YEAR, "{mean}");
        assert!(max < 0.3 * 5.0 * CM_PER_YEAR, "{max}");
        assert!(boundaries.iter().all(|b| b.kind == BoundaryKind::Other));
    }

    /// The two hemispheres of [`hemispheres`] with plate 0 subducting under plate 1 (slab of
    /// `slab_length`) and plate 0 moving at 5 cm/yr at `angle` degrees from the strike of the
    /// boundary near +y, towards plate 1.
    fn oblique_contact(angle: f64, slab_length: f64) -> Simulation {
        let mut simulation = hemispheres(0.0, 4);
        simulation.seed_subduction((0, 1), 1, slab_length);
        let speed = 5.0 * CM_PER_YEAR;
        let (along, across) = (math::cos(angle.to_radians()), math::sin(angle.to_radians()));
        // Near +y, the rotation about z moves along the equator, the one about x across it.
        simulation.plates[1].angular_velocity = DVec3::ZERO;
        simulation.plates[0].angular_velocity = (DVec3::Z * along + DVec3::X * across) * speed;
        if simulation.segment_near(DVec3::Y).rate < 0.0 {
            simulation.plates[0].angular_velocity = (DVec3::Z * along - DVec3::X * across) * speed;
        }
        simulation
    }

    impl Simulation {
        /// The segment of plates 0 and 1 closest to the world position `r`, with its motion
        /// evaluated for the current plate velocities.
        fn segment_near(&mut self, r: DVec3) -> polarity::Segment {
            let boundaries = self.boundary_cells();
            self.update_segment_motion(&boundaries, false);
            let rotations = self.rotations();
            self.polarities
                .segment_at(0, 1, r, &rotations)
                .expect("a segment covers the position")
                .clone()
        }
    }

    #[test]
    fn contacts_close_to_strike_do_not_subduct() {
        for (angle, subducts) in [(12.0, false), (45.0, true)] {
            let mut simulation = oblique_contact(angle, 0.0);
            let segment = simulation.segment_near(DVec3::Y);
            let measured = math::atan(segment.rate / segment.strike_rate).to_degrees();
            assert!((measured - angle).abs() < 5.0, "{angle}: {measured}");
            assert_eq!(segment.subducting, subducts, "{angle}");
            for _ in 0..5 {
                let boundaries = simulation.boundary_cells();
                simulation.update_slab_lengths(1.0, &boundaries);
            }
            let segment = simulation.segment_near(DVec3::Y);
            // The slab grows with the normal convergence, ~3.5 cm/yr = 35 km/Myr at 45°.
            if subducts {
                assert!(segment.slab_length > 100e3, "{}", segment.slab_length);
            } else {
                assert_eq!(segment.slab_length, 0.0);
            }
            // Converging cells near +y are trenches only where the segment subducts.
            let trenches_near = simulation
                .boundary_cells()
                .iter()
                .filter(|b| b.segment == Some(segment.id))
                .filter(|b| matches!(b.kind, BoundaryKind::Trench { .. }))
                .count();
            assert_eq!(trenches_near > 0, subducts, "{angle}: {trenches_near}");
        }
    }

    #[test]
    fn strike_slip_slabs_decay_and_stop_being_trenches() {
        let mut simulation = oblique_contact(0.0, 600e3);
        let trenches = |simulation: &Simulation| {
            simulation
                .boundary_cells()
                .iter()
                .filter(|b| matches!(b.kind, BoundaryKind::Trench { .. }))
                .count()
        };
        assert!(trenches(&simulation) > 0);
        for _ in 0..60 {
            let boundaries = simulation.boundary_cells();
            simulation.update_slab_lengths(1.0, &boundaries);
        }
        assert!(
            simulation
                .polarities
                .segments()
                .all(|segment| segment.slab_length == 0.0)
        );
        assert_eq!(trenches(&simulation), 0);
    }

    #[test]
    fn strike_slip_slabs_decay_with_solved_velocities() {
        // The mantle drags plate 0 along the boundary; the slabs' pull (symmetric around the
        // whole boundary) does not make the contact converge.
        let mut simulation = oblique_contact(0.0, 600e3);
        let omega = DVec3::Z * 1e-15;
        let mantle: Vec<DVec3> = (0..simulation.mesh.len() as u32)
            .map(|cell| {
                if simulation.owners[cell as usize].plate == 0 {
                    omega.cross(simulation.mesh.vertex(cell)) * simulation.params.planet_radius
                } else {
                    DVec3::ZERO
                }
            })
            .collect();
        for _ in 0..60 {
            simulation.update_plate_velocities(&mantle);
            let boundaries = simulation.boundary_cells();
            simulation.update_slab_lengths(1.0, &boundaries);
        }
        assert!(
            simulation
                .polarities
                .segments()
                .all(|segment| segment.slab_length == 0.0)
        );
    }

    #[test]
    fn slab_pull_per_boundary_length_matches_the_parameter() {
        // A trench along the whole equator: the summed slab pull on plate 0 per unit of
        // boundary length is `slab_pull` × the slab's buoyancy, on two meshes. (With the length
        // of a mesh edge per cell instead of the boundary length per cell, it was ~10% more.)
        let pull_per_length = |subdivisions: u32| {
            let mut simulation = hemispheres(0.0, subdivisions);
            simulation.seed_subduction((0, 1), 1, 600e3);
            simulation.plates[0].angular_velocity = DVec3::ZERO;
            simulation.plates[1].angular_velocity = DVec3::ZERO;
            let boundaries = simulation.boundary_cells();
            let force: f64 = boundaries
                .iter()
                .filter(|b| b.plate == 0)
                .map(|b| simulation.boundary_force(b).length())
                .sum();
            let expected = simulation.params.slab_pull * simulation.slab_buoyancy(50.0, 600e3);
            // The equator is 2πR long.
            force / (2.0 * std::f64::consts::PI * simulation.params.planet_radius) / expected
        };
        for subdivisions in [4, 5] {
            let ratio = pull_per_length(subdivisions);
            assert!((ratio - 1.0).abs() < 0.06, "{subdivisions}: {ratio}");
        }
    }

    #[test]
    fn segments_rotating_about_a_pole_inside_them_do_not_subduct() {
        // Plate 0 rotates about the axis through +y: near +y, one half of the segment converges,
        // the other opens.
        let mut simulation = oblique_contact(45.0, 0.0);
        simulation.plates[0].angular_velocity = DVec3::Y * (5.0 * CM_PER_YEAR / 0.1);
        let segment = simulation.segment_near(DVec3::Y);
        assert!(
            !segment.subducting,
            "{} {}",
            segment.rate, segment.strike_rate
        );
    }

    #[test]
    fn slab_pull_moves_a_plate_towards_its_trench() {
        let mut simulation = one_sided_trench(TectonicParams::default());
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
                    felsic: 1.0,
                    metasediment: 0.0,
                    orogeny: NO_OROGENY,
                    orogenic: 0.0,
                    reheated: NO_REHEATING,
                    cohort: 0.0,
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

    /// Two oceanic hemispheres split at the equator, without subduction, at rest, with the given
    /// boundary coupling width; returns the simulation and the boundary cells of plate 0.
    fn hemispheres(width: f64, subdivisions: u32) -> Simulation {
        let params = TectonicParams {
            boundary_coupling_width: width,
            ..TectonicParams::default()
        };
        let mut simulation = Simulation::new(SphereMesh::icosphere(subdivisions), 1, params, 1);
        for crust in simulation.plates[0].crust.iter_mut().flatten() {
            *crust = new_oceanic_crust(-50.0);
        }
        simulation.split_plate(0, DVec3::Z).unwrap();
        simulation.polarities.clear();
        simulation
    }

    /// Solves the plate velocities with the mantle under plate 0 moving like the rotation
    /// `omega` (rad/s), and returns the mean strike-slip and opening speed (rad/Myr) of plate 1
    /// relative to plate 0 at the boundary.
    fn relative_boundary_motion(simulation: &mut Simulation, omega: DVec3) -> (f64, f64) {
        let mantle: Vec<DVec3> = (0..simulation.mesh.len() as u32)
            .map(|cell| {
                if simulation.owners[cell as usize].plate == 0 {
                    omega.cross(simulation.mesh.vertex(cell)) * simulation.params.planet_radius
                } else {
                    DVec3::ZERO
                }
            })
            .collect();
        simulation.update_plate_velocities(&mantle);
        let boundaries: Vec<BoundaryCell> = simulation
            .boundary_cells()
            .into_iter()
            .filter(|b| b.plate == 0)
            .collect();
        let normals = simulation.smoothed_normals(&boundaries);
        let (mut strike, mut opening) = (0.0, 0.0);
        for (b, n) in boundaries.iter().zip(&normals) {
            let r = simulation.mesh.vertex(b.cell);
            let relative = (simulation.plates[1].angular_velocity
                - simulation.plates[0].angular_velocity)
                .cross(r);
            strike += relative.dot(r.cross(*n)).abs();
            opening += relative.dot(*n).abs();
        }
        let count = boundaries.len() as f64;
        (strike / count, opening / count)
    }

    #[test]
    fn thickened_belts_resist_their_own_collision() {
        // The same collision with a plateau of 60 km crust on both sides: its excess potential
        // energy resists the convergence; with the reference thickness nothing changes.
        let closing = DVec3::X * 2e-16;
        let motion = |thickness: f64, factor: f64| {
            let mut simulation = continental_hemispheres(50.0);
            simulation.params.belt_gpe_resistance = factor;
            for plate in &mut simulation.plates {
                for crust in plate.crust.iter_mut().flatten() {
                    crust.thickness = thickness;
                }
            }
            relative_boundary_motion(&mut simulation, closing).1
        };
        let (free, resisted) = (motion(60e3, 0.0), motion(60e3, 1.0));
        assert!(resisted < 0.9 * free, "{resisted} vs {free}");
        let (free, reference) = (motion(35e3, 0.0), motion(35e3, 1.0));
        assert!(
            (reference / free - 1.0).abs() < 1e-6,
            "{reference} vs {free}"
        );
    }

    /// Two continental hemispheres split at the equator, at rest, with the given collision
    /// resistance.
    fn continental_hemispheres(collision_resistance: f64) -> Simulation {
        let params = TectonicParams {
            collision_resistance,
            ..TectonicParams::default()
        };
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 1, params, 1);
        for crust in simulation.plates[0].crust.iter_mut().flatten() {
            *crust = Crust {
                thickness: 35_000.0,
                density: crate::isostasy::CONTINENTAL_DENSITY,
                birth: -1000.0,
                sediment: 0.0,
                felsic: 1.0,
                metasediment: 0.0,
                orogeny: NO_OROGENY,
                orogenic: 0.0,
                reheated: NO_REHEATING,
                cohort: 0.0,
            };
        }
        simulation.split_plate(0, DVec3::Z).unwrap();
        simulation.polarities.clear();
        simulation
    }

    #[test]
    fn collisions_resist_convergence_but_not_strike_slip() {
        // Strike-slip along a continental contact is resisted only by the boundary coupling.
        let strike_slip = DVec3::Z * 1e-15;
        let (free, _) = relative_boundary_motion(&mut continental_hemispheres(0.0), strike_slip);
        let (resisted, _) =
            relative_boundary_motion(&mut continental_hemispheres(1000.0), strike_slip);
        assert!(resisted > 0.9 * free, "{resisted} vs {free}");
        // Convergence across it is resisted.
        let closing = DVec3::X * 1e-15;
        let (_, free) = relative_boundary_motion(&mut continental_hemispheres(0.0), closing);
        let (_, resisted) = relative_boundary_motion(&mut continental_hemispheres(1000.0), closing);
        assert!(resisted < 0.8 * free, "{resisted} vs {free}");
    }

    #[test]
    fn boundary_coupling_resists_strike_slip_but_not_opening() {
        // Strike-slip: the mantle rotates plate 0 about the pole, along the boundary.
        let strike_slip = DVec3::Z * 1e-15;
        let (free, _) = relative_boundary_motion(&mut hemispheres(0.0, 4), strike_slip);
        let (coupled, _) = relative_boundary_motion(&mut hemispheres(2000e3, 4), strike_slip);
        assert!(coupled < 0.8 * free, "{coupled} vs {free}");
        // Opening: the mantle rotates plate 0 about an equatorial axis, which moves the boundary
        // apart on one side and together on the other.
        let opening = DVec3::X * 1e-15;
        let (_, free) = relative_boundary_motion(&mut hemispheres(0.0, 4), opening);
        let (_, coupled) = relative_boundary_motion(&mut hemispheres(2000e3, 4), opening);
        assert!((coupled / free - 1.0).abs() < 0.05, "{coupled} vs {free}");
    }

    #[test]
    fn boundary_coupling_is_resolution_independent() {
        let strike_slip = DVec3::Z * 1e-15;
        let ratio = |subdivisions| {
            let (free, _) =
                relative_boundary_motion(&mut hemispheres(0.0, subdivisions), strike_slip);
            let (coupled, _) =
                relative_boundary_motion(&mut hemispheres(1000e3, subdivisions), strike_slip);
            coupled / free
        };
        let (coarse, fine) = (ratio(4), ratio(5));
        assert!((coarse / fine - 1.0).abs() < 0.05, "{coarse} vs {fine}");
    }

    #[test]
    fn strong_boundary_coupling_locks_strike_slip() {
        let strike_slip = DVec3::Z * 1e-15;
        let mut free_simulation = hemispheres(0.0, 4);
        let (free, _) = relative_boundary_motion(&mut free_simulation, strike_slip);
        let mut simulation = hemispheres(1e9, 4);
        let (locked, _) = relative_boundary_motion(&mut simulation, strike_slip);
        assert!(locked < 0.02 * free, "{locked} vs {free}");
        // The driven plate drags the other one along (equal and opposite coupling torques).
        assert!(simulation.plates[1].angular_velocity.z > 0.0);
    }

    /// Two plates, all oceanic crust, plate 0 would subduct under plate 1.
    fn two_oceanic_plates(slab_length: f64) -> Simulation {
        let mut simulation =
            Simulation::new(SphereMesh::icosphere(4), 2, TectonicParams::default(), 5);
        simulation.polarities.clear();
        simulation.seed_subduction((0, 1), 1, slab_length);
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
            let length = simulation.polarities.max_slab(0, 1);
            assert!(length > previous);
            previous = length;
        }
        for _ in 0..200 {
            update(&mut simulation, 1.0);
        }
        assert_eq!(
            simulation.polarities.max_slab(0, 1),
            simulation.params.max_slab_length
        );
        // Subduction stops: the plates come to rest.
        for plate in &mut simulation.plates {
            plate.angular_velocity = DVec3::ZERO;
        }
        update(&mut simulation, 10.0);
        let decayed = simulation.polarities.max_slab(0, 1);
        assert!(decayed < 0.5 * simulation.params.max_slab_length);
        assert!(decayed > 0.0);
        // Eventually the slab breaks off completely.
        update(&mut simulation, 100.0);
        assert_eq!(simulation.polarities.max_slab(0, 1), 0.0);
    }
}
