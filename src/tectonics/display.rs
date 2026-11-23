//! What the plate boundaries are doing, per cell, for display (the plate render mode of the
//! viewer): the classification of the velocity solve ([`super::dynamics::BoundaryKind`]) and the
//! subduction polarity of the boundary segments, completed by the kinematics for the boundaries
//! the solve does not distinguish (transforms, rifts, the overriding side of a trench), and the
//! deforming bands of collisions and rifts (#69).
//!
//! Nothing here feeds back into the simulation.

use glam::DVec3;

use super::Simulation;
use super::dynamics::BoundaryKind;
use crate::math;

/// What happens at a boundary cell, seen from the plate the cell belongs to. A cell at the
/// boundary towards several plates shows the first in this order (most important first).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum BoundaryClass {
    /// The plate subducts here under the other plate (an active trench).
    Subducting = 1,
    /// Continental crust of both plates meets: a collision belt.
    Collision = 2,
    /// The other plate subducts here under this one: the upper plate of a trench.
    Overriding = 3,
    /// A diffuse convergent zone, either side: the plates converge but no thrust has broken
    /// through yet, so the convergence is taken up by distributed shortening (#42, #73).
    Diffuse = 4,
    /// The plates move apart at young oceanic crust: a mid-ocean ridge.
    Ridge = 5,
    /// The plates move apart at continental crust: a continental rift.
    Rift = 6,
    /// The plates slide past each other: the relative motion is within
    /// [`super::TectonicParams::min_trench_obliquity`] of the boundary's strike.
    Transform = 7,
    /// The plates converge, but none of the above (e.g. a decaying trench seen from its
    /// upper side, or a contact whose polarity is not decided yet).
    OtherConvergent = 8,
    /// The plates move apart at old oceanic or mixed crust (a boundary that just formed by a
    /// split, or an old divergent margin).
    OtherDivergent = 9,
    /// The plates move against each other slower than
    /// [`super::TectonicParams::min_trench_convergence`].
    Inactive = 10,
}

impl BoundaryClass {
    /// Every class, in order of their codes.
    pub const ALL: [BoundaryClass; 10] = [
        BoundaryClass::Subducting,
        BoundaryClass::Collision,
        BoundaryClass::Overriding,
        BoundaryClass::Diffuse,
        BoundaryClass::Ridge,
        BoundaryClass::Rift,
        BoundaryClass::Transform,
        BoundaryClass::OtherConvergent,
        BoundaryClass::OtherDivergent,
        BoundaryClass::Inactive,
    ];

    /// The class of a code (1 to 10), `None` for 0 (no boundary) and unknown codes.
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.get(usize::from(code).checked_sub(1)?).copied()
    }

    pub fn code(self) -> u8 {
        self as u8
    }

    /// A short description, e.g. for the cell widget.
    pub fn label(self) -> &'static str {
        match self {
            BoundaryClass::Subducting => "trench, subducting plate",
            BoundaryClass::Collision => "continental collision",
            BoundaryClass::Overriding => "trench, overriding plate",
            BoundaryClass::Diffuse => "diffuse convergent zone",
            BoundaryClass::Ridge => "mid-ocean ridge",
            BoundaryClass::Rift => "continental rift",
            BoundaryClass::Transform => "transform",
            BoundaryClass::OtherConvergent => "other convergent",
            BoundaryClass::OtherDivergent => "other divergent",
            BoundaryClass::Inactive => "inactive (slow)",
        }
    }
}

/// The boundary class and deforming band of every cell.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundaryMap {
    /// Per cell: its boundary class, `None` inside a plate.
    pub classes: Vec<Option<BoundaryClass>>,
    /// Per cell: whether it is in a deforming band: crust of continental origin of the upper
    /// plate within [`super::TectonicParams::collision_band_width`] of a converging collision
    /// (where the collision is taken up, see `deformation`), or continental crust within
    /// [`super::TectonicParams::rift_width`] of a rift on its plate (where the rift stretches).
    pub band: Vec<bool>,
}

impl Simulation {
    /// Classifies the boundary cells for display (see [`BoundaryMap`]) with the current plate
    /// velocities. Costs about one [`Simulation::boundary_cells`].
    pub fn boundary_map(&self) -> BoundaryMap {
        let params = &self.params;
        // rad/Myr per cm/yr.
        let per_cm_yr = 1.0 / (params.planet_radius * 100.0 / 1e6);
        let min_convergence = params.min_trench_convergence * per_cm_yr;
        let suture = params.events.suture_convergence * per_cm_yr;
        let strike_ratio = math::tan(params.min_trench_obliquity.to_radians());
        let rotations = self.rotations();
        let cell_count = self.mesh.len();
        let mut classes: Vec<Option<BoundaryClass>> = vec![None; cell_count];
        // Seeds of the bands: (cell, rings).
        let spacing = params.planet_radius * self.mesh.mean_edge_angle();
        let collision_rings = super::deformation::band_rings(params.collision_band_width, spacing);
        let rift_rings = super::deformation::band_rings(params.rift_width, spacing);
        let mut seeds: Vec<(u32, usize)> = Vec::new();

        for boundary in self.boundary_cells() {
            let cell = boundary.cell;
            let position = self.mesh.vertex(cell);
            let crust = self.surface_crust(cell);
            let other_crust = self
                .mesh
                .neighbors(cell)
                .iter()
                .find(|&&neighbor| self.owners[neighbor as usize].plate == boundary.other)
                .map(|&neighbor| self.surface_crust(neighbor));
            let relative = (self.plates[boundary.plate as usize].angular_velocity
                - self.plates[boundary.other as usize].angular_velocity)
                .cross(position);
            let convergence = boundary.convergence;
            let strike_slip = relative.dot(position.cross(boundary.normal)).abs();
            let segment =
                self.polarities
                    .segment_at(boundary.plate, boundary.other, position, &rotations);
            let upper = segment.is_some_and(|segment| segment.overriding == boundary.plate);
            let class = match boundary.kind {
                BoundaryKind::Trench { .. } => BoundaryClass::Subducting,
                BoundaryKind::Collision => {
                    if upper && convergence > suture && collision_rings > 0 {
                        seeds.push((cell, collision_rings));
                    }
                    BoundaryClass::Collision
                }
                BoundaryKind::Diffuse => BoundaryClass::Diffuse,
                BoundaryKind::Ridge | BoundaryKind::Other => {
                    let lower_subducts = other_crust.is_some_and(|crust| !crust.resists_subduction());
                    // The mirror of the trench and diffuse conditions of `boundary_cells`.
                    let overriding = segment.is_some_and(|segment| {
                        upper
                            && lower_subducts
                            && segment.localised
                            && ((segment.subducting && convergence > min_convergence)
                                || (segment.slab_length > 0.0 && convergence > -min_convergence))
                    });
                    let diffuse = segment.is_some_and(|segment| {
                        upper && lower_subducts && !segment.localised && segment.subducting
                    });
                    let speed = relative.length();
                    if overriding {
                        BoundaryClass::Overriding
                    } else if diffuse {
                        BoundaryClass::Diffuse
                    } else if speed < min_convergence {
                        BoundaryClass::Inactive
                    } else if convergence.abs() < strike_ratio * strike_slip {
                        BoundaryClass::Transform
                    } else if boundary.kind == BoundaryKind::Ridge {
                        BoundaryClass::Ridge
                    } else if convergence < 0.0 && crust.is_continental() {
                        if rift_rings > 0 {
                            seeds.push((cell, rift_rings));
                        }
                        BoundaryClass::Rift
                    } else if convergence < 0.0 {
                        BoundaryClass::OtherDivergent
                    } else {
                        BoundaryClass::OtherConvergent
                    }
                }
            };
            let slot = &mut classes[cell as usize];
            *slot = Some(slot.map_or(class, |previous| previous.min(class)));
        }

        // The bands: rings around their seeds over the crust of continental origin of the
        // seed's plate.
        let mut band = vec![false; cell_count];
        let mut stamp = vec![u32::MAX; cell_count];
        let (mut frontier, mut next) = (Vec::new(), Vec::new());
        for (index, &(seed, rings)) in seeds.iter().enumerate() {
            let plate = self.owners[seed as usize].plate;
            frontier.clear();
            frontier.push(seed);
            stamp[seed as usize] = index as u32;
            for _ in 0..=rings {
                next.clear();
                for &cell in &frontier {
                    if self.owners[cell as usize].plate != plate
                        || !self.surface_crust(cell).is_continental_origin_column()
                    {
                        continue;
                    }
                    band[cell as usize] = true;
                    for &neighbor in self.mesh.neighbors(cell) {
                        if stamp[neighbor as usize] != index as u32 {
                            stamp[neighbor as usize] = index as u32;
                            next.push(neighbor);
                        }
                    }
                }
                std::mem::swap(&mut frontier, &mut next);
            }
        }
        BoundaryMap { classes, band }
    }
}

/// The unit normal (tangent to the sphere at `position`) pointing from `position` towards
/// `towards`; zero if they coincide. For tests.
#[cfg(test)]
fn tangent_towards(position: DVec3, towards: DVec3) -> DVec3 {
    let direction = towards - position;
    (direction - position * direction.dot(position)).normalize_or_zero()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sphere::SphereMesh;
    use crate::tectonics::TectonicParams;

    #[test]
    fn codes_round_trip_in_order() {
        assert_eq!(BoundaryClass::from_code(0), None);
        assert_eq!(BoundaryClass::from_code(11), None);
        for (index, class) in BoundaryClass::ALL.into_iter().enumerate() {
            assert_eq!(usize::from(class.code()), index + 1);
            assert_eq!(BoundaryClass::from_code(class.code()), Some(class));
            assert!(!class.label().is_empty());
        }
        // Codes fit in 4 bits (the viewer keeps them in the cell flags).
        assert!(BoundaryClass::ALL.iter().all(|class| class.code() < 16));
        assert!(BoundaryClass::Subducting < BoundaryClass::Inactive);
    }

    /// Two plates, split at the equator (plate 1, the new one, north of it), with plate 1 moving with `omega` (rad/Myr) and plate 0 at rest.
    fn split(omega: DVec3) -> Simulation {
        let mut simulation = Simulation::new(
            SphereMesh::icosphere(4),
            1,
            TectonicParams {
                initial_sediment: Default::default(),
                ..TectonicParams::default()
            },
            1,
        );
        let new = simulation.split_plate(0, DVec3::Z).unwrap();
        for plate in &mut simulation.plates {
            plate.angular_velocity = DVec3::ZERO;
        }
        simulation.plates[new as usize].angular_velocity = omega;
        simulation
    }

    fn classes(simulation: &Simulation) -> Vec<BoundaryClass> {
        simulation
            .boundary_map()
            .classes
            .into_iter()
            .flatten()
            .collect()
    }

    fn share(classes: &[BoundaryClass], wanted: &[BoundaryClass]) -> f64 {
        classes.iter().filter(|class| wanted.contains(class)).count() as f64
            / classes.len().max(1) as f64
    }

    #[test]
    fn strike_slip_is_a_transform_and_rest_is_inactive() {
        // A rotation about the pole: the plates slide past each other along the equator.
        // Collisions (continental crust on both sides) are collisions whatever the motion.
        let without_collisions = |classes: Vec<BoundaryClass>| -> Vec<BoundaryClass> {
            classes
                .into_iter()
                .filter(|&class| class != BoundaryClass::Collision)
                .collect()
        };
        let classes = without_collisions(classes(&split(DVec3::Z * 0.01)));
        assert!(!classes.is_empty());
        assert!(
            share(&classes, &[BoundaryClass::Transform]) > 0.9,
            "{classes:?}"
        );
        let classes = without_collisions(self::classes(&split(DVec3::ZERO)));
        assert!(share(&classes, &[BoundaryClass::Inactive]) > 0.99);
    }

    #[test]
    fn opening_is_divergent_and_closing_convergent() {
        // A rotation about an equatorial axis: the plates open on one side of the axis and
        // close on the other.
        let simulation = split(DVec3::X * 0.01);
        let map = simulation.boundary_map();
        let mut opening = Vec::new();
        let mut closing = Vec::new();
        for (cell, class) in map.classes.iter().enumerate() {
            let Some(class) = *class else { continue };
            let position = simulation.mesh.vertex(cell as u32);
            // Velocity of the cell's plate relative to the other one, and the direction across
            // the equator towards the other plate.
            let plate = simulation.owners[cell].plate as usize;
            let omega = simulation.plates[plate].angular_velocity
                - simulation.plates[1 - plate].angular_velocity;
            let relative = omega.cross(position);
            // The new plate (1) is north of the equator.
            let across = tangent_towards(position, if plate == 1 { -DVec3::Z } else { DVec3::Z });
            let convergence = relative.dot(across);
            if convergence.abs() < 0.5 * relative.length() || class == BoundaryClass::Collision {
                continue;
            }
            if convergence > 0.0 {
                closing.push(class);
            } else {
                opening.push(class);
            }
        }
        use BoundaryClass::*;
        let divergent = [Ridge, Rift, OtherDivergent];
        let convergent = [Subducting, Overriding, Collision, Diffuse, OtherConvergent];
        assert!(share(&opening, &divergent) > 0.9, "{opening:?}");
        assert!(share(&closing, &convergent) > 0.9, "{closing:?}");
    }
}
