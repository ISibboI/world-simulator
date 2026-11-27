//! Seismic and volcanic activity (#110): long-term rates of earthquakes and magmatism estimated
//! from the simulated processes, for display only.
//!
//! Earthquakes and eruptions happen far more often than the 1 Myr step resolves, so they cannot
//! emerge from the simulation. What it has are the processes that drive them, and the view
//! estimates their long-term rates from these with published relations (see
//! `notes/issue-110/plan.md`):
//!
//! - **Seismic moment rate** per area (N·m yr⁻¹ km⁻²): at plate boundaries from the relative
//!   motion with the coupled seismogenic thickness of the boundary class (Kostrov 1974; Bird &
//!   Kagan 2004), inside plates from the strain rate of the deforming lithosphere (#182's sheet;
//!   Kostrov 1974, Savage & Simpson 1997).
//! - **Magma supply** per area (mm/yr, as a layer over the area, intrusions included): ridges
//!   by their spreading rate, arcs as the model grows them behind its trenches, hotspots above
//!   the mantle plumes by their flux and the lithosphere's lid, continental rifts by their
//!   opening rate.
//!
//! Everything is read from the state; nothing changes it.

use super::crust::OCEANIC_THICKNESS;
use super::{BoundaryClass, BoundaryMap, Crust, Simulation};
use crate::SECONDS_PER_YEAR;

/// Shear modulus (Pa) of subduction megathrusts and of other faults (Bird & Kagan 2004).
const SUBDUCTION_MODULUS: f64 = 49e9;
const CRUST_MODULUS: f64 = 27.7e9;

/// Coupled seismogenic thickness (m) and dip of a kind of fault (Bird & Kagan 2004).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Coupling {
    thickness: f64,
    dip: f64,
    modulus: f64,
}

impl Coupling {
    const fn new(thickness_km: f64, dip: f64, modulus: f64) -> Self {
        Self {
            thickness: thickness_km * 1e3,
            dip,
            modulus,
        }
    }

    /// Moment rate per boundary length (N·m yr⁻¹ m⁻¹) for a horizontal relative motion
    /// (heave) of `rate` m/yr: μ (c z) v / (sin δ cos δ) for a dipping fault (the slip is
    /// v / cos δ, the down-dip width (c z) / sin δ), μ (c z) v for a vertical one.
    fn moment_rate(self, rate: f64) -> f64 {
        let geometry = if self.dip >= 90.0 {
            1.0
        } else {
            let (sin, cos) = crate::math::sin_cos(self.dip.to_radians());
            1.0 / (sin * cos)
        };
        self.modulus * self.thickness * rate.abs() * geometry
    }
}

/// Subduction zones (SUB).
const SUBDUCTION: Coupling = Coupling::new(18.0, 14.0, SUBDUCTION_MODULUS);
/// Continental convergent boundaries (CCB): collision belts.
const CONTINENTAL_CONVERGENT: Coupling = Coupling::new(18.0, 20.0, CRUST_MODULUS);
/// Oceanic convergent boundaries (OCB).
const OCEANIC_CONVERGENT: Coupling = Coupling::new(3.8, 20.0, CRUST_MODULUS);
/// Continental transforms (CTF).
const CONTINENTAL_TRANSFORM: Coupling = Coupling::new(8.6, 90.0, CRUST_MODULUS);
/// Oceanic transforms (OTF), slow (below [`SLOW_TRANSFORM`]), medium (below
/// [`MEDIUM_TRANSFORM`]) and fast.
const OCEANIC_TRANSFORM: [Coupling; 3] = [
    Coupling::new(13.0, 90.0, CRUST_MODULUS),
    Coupling::new(1.8, 90.0, CRUST_MODULUS),
    Coupling::new(1.6, 90.0, CRUST_MODULUS),
];
const SLOW_TRANSFORM: f64 = 4.0;
const MEDIUM_TRANSFORM: f64 = 6.8;
/// Spreading ridges (OSR): normal faulting and strike-slip.
const RIDGE_NORMAL: Coupling = Coupling::new(0.13, 55.0, CRUST_MODULUS);
const RIDGE_STRIKE_SLIP: Coupling = Coupling::new(0.40, 90.0, CRUST_MODULUS);
/// Continental rifts (CRB).
const CONTINENTAL_RIFT: Coupling = Coupling::new(3.0, 55.0, CRUST_MODULUS);

/// Strain rates (1/s) below this are taken as no deformation: stable plate interiors strain at
/// 10⁻¹⁹–10⁻¹⁸ s⁻¹ and are left out (intraplate seismicity, ~1 % of Earth's moment rate, is not
/// estimated); the floor keeps numerical noise of the sheet from tinting whole plates.
const MIN_STRAIN_RATE: f64 = 1e-17;

/// The coupling of distributed deformation at strain rate `strain_rate` with area change
/// `dilatation` (1/s, positive when stretching): shortening where the area shrinks by more than
/// half the strain rate, stretching where it grows by as much, shear otherwise; with the
/// coupled thickness of the boundaries of that style.
fn deformation_coupling(continental: bool, strain_rate: f64, dilatation: f64) -> Coupling {
    if dilatation < -0.5 * strain_rate {
        if continental {
            CONTINENTAL_CONVERGENT
        } else {
            OCEANIC_CONVERGENT
        }
    } else if dilatation > 0.5 * strain_rate {
        if continental {
            CONTINENTAL_RIFT
        } else {
            RIDGE_NORMAL
        }
    } else if continental {
        CONTINENTAL_TRANSFORM
    } else {
        OCEANIC_TRANSFORM[1]
    }
}

/// Magma supply of continental rifts relative to a ridge opening at the same rate: thick
/// lithosphere melts far less until it has thinned by a factor of 2–3 (McKenzie & Bickle 1988).
const RIFT_MELT_SHARE: f64 = 0.1;

/// Magma supply of a plume (m³/yr per m²/s of plume flux) under no lid; the constants of the
/// plume tails of #106's plan, so that the estimate matches what #106 emplaces.
const PLUME_MAGMA_PER_FLUX: f64 = 1e11;
/// Depth (m) at which the hot plume head starts to melt, and the thermal lid of oceanic
/// lithosphere: z = min(2.32 √(κ t), 95 km), κ = 10⁻⁶ m²/s; lithosphere of continental
/// origin: 130 km.
const PLUME_MELT_DEPTH: f64 = 180e3;
const MAX_OCEANIC_LID: f64 = 95e3;
const CONTINENTAL_LID: f64 = 130e3;
const DIFFUSIVITY: f64 = 1e-6;
/// Standard deviation and cut-off (m) of the footprint of a hotspot's magma.
const PLUME_FOOTPRINT: f64 = 150e3;
const PLUME_CUTOFF: f64 = 400e3;

/// What causes most of the earthquakes of a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum SeismicSource {
    /// A subduction megathrust.
    Subduction = 1,
    /// A continental collision belt.
    Collision = 2,
    /// Other convergence: diffuse convergent zones, convergent boundaries without a trench.
    Convergent = 3,
    /// Strike-slip at a transform or the strike-slip part of an oblique boundary.
    Transform = 4,
    /// Normal faulting at a spreading ridge.
    Ridge = 5,
    /// Normal faulting at a continental rift.
    Rift = 6,
    /// Distributed deformation inside a plate.
    Deformation = 7,
}

impl SeismicSource {
    pub const ALL: [SeismicSource; 7] = [
        SeismicSource::Subduction,
        SeismicSource::Collision,
        SeismicSource::Convergent,
        SeismicSource::Transform,
        SeismicSource::Ridge,
        SeismicSource::Rift,
        SeismicSource::Deformation,
    ];

    /// The source of a code (1 to 7), `None` for 0 and unknown codes.
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.get(usize::from(code).checked_sub(1)?).copied()
    }

    pub fn code(self) -> u8 {
        self as u8
    }

    /// A short name, e.g. for the cell widget.
    pub fn label(self) -> &'static str {
        match self {
            SeismicSource::Subduction => "subduction",
            SeismicSource::Collision => "collision",
            SeismicSource::Convergent => "convergence",
            SeismicSource::Transform => "strike-slip",
            SeismicSource::Ridge => "ridge",
            SeismicSource::Rift => "rift",
            SeismicSource::Deformation => "deformation",
        }
    }
}

/// What supplies most of the magma of a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum VolcanicSource {
    /// A volcanic arc above a subducting slab.
    Arc = 1,
    /// A spreading ridge.
    Ridge = 2,
    /// A hotspot above a mantle plume.
    Hotspot = 3,
    /// A continental rift.
    Rift = 4,
}

impl VolcanicSource {
    pub const ALL: [VolcanicSource; 4] = [
        VolcanicSource::Arc,
        VolcanicSource::Ridge,
        VolcanicSource::Hotspot,
        VolcanicSource::Rift,
    ];

    /// The source of a code (1 to 4), `None` for 0 and unknown codes.
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.get(usize::from(code).checked_sub(1)?).copied()
    }

    pub fn code(self) -> u8 {
        self as u8
    }

    /// A short name, e.g. for the cell widget.
    pub fn label(self) -> &'static str {
        match self {
            VolcanicSource::Arc => "arc",
            VolcanicSource::Ridge => "ridge",
            VolcanicSource::Hotspot => "hotspot",
            VolcanicSource::Rift => "rift",
        }
    }
}

/// The activity of every cell (see the module documentation).
#[derive(Debug, Clone, Default)]
pub struct Activity {
    /// Seismic moment rate per area in N·m yr⁻¹ km⁻².
    pub seismicity: Vec<f64>,
    /// Its largest contribution, if there is any.
    pub seismic_source: Vec<Option<SeismicSource>>,
    /// Magma supply per area in mm/yr (10³ m³ yr⁻¹ km⁻²).
    pub magma: Vec<f64>,
    /// Its largest contribution, if there is any.
    pub volcanic_source: Vec<Option<VolcanicSource>>,
}

/// Contributions of the sources of one cell.
#[derive(Debug, Clone, Copy)]
struct Contributions<const N: usize>([f64; N]);

impl<const N: usize> Contributions<N> {
    const ZERO: Self = Self([0.0; N]);

    fn total(&self) -> f64 {
        self.0.iter().sum()
    }

    /// The index of the largest positive contribution.
    fn largest(&self) -> Option<usize> {
        let mut best: Option<usize> = None;
        for (index, &value) in self.0.iter().enumerate() {
            if value > 0.0 && best.is_none_or(|best| value > self.0[best]) {
                best = Some(index);
            }
        }
        best
    }
}

/// The coupling of the normal (convergent or opening) motion at a boundary of class `class`,
/// on crust that is of continental origin or not, and the source it counts for.
fn normal_coupling(
    class: BoundaryClass,
    continental: bool,
    convergence: f64,
) -> Option<(Coupling, SeismicSource)> {
    let convergent_class = matches!(
        class,
        BoundaryClass::Subducting
            | BoundaryClass::Overriding
            | BoundaryClass::Collision
            | BoundaryClass::Diffuse
            | BoundaryClass::OtherConvergent
    );
    Some(if convergence > 0.0 {
        match class {
            BoundaryClass::Subducting | BoundaryClass::Overriding => {
                (SUBDUCTION, SeismicSource::Subduction)
            }
            BoundaryClass::Collision => (CONTINENTAL_CONVERGENT, SeismicSource::Collision),
            _ if continental => (CONTINENTAL_CONVERGENT, SeismicSource::Convergent),
            _ => (OCEANIC_CONVERGENT, SeismicSource::Convergent),
        }
    } else if convergent_class {
        // A trench or collision that the slab's hysteresis keeps while the plates move
        // slightly apart: no opening faults.
        return None;
    } else if continental {
        (CONTINENTAL_RIFT, SeismicSource::Rift)
    } else {
        (RIDGE_NORMAL, SeismicSource::Ridge)
    })
}

/// The coupling of the strike-slip motion at a boundary of class `class` whose plates move at
/// `speed` cm/yr relative to each other.
fn strike_slip_coupling(class: BoundaryClass, continental: bool, speed: f64) -> Coupling {
    if continental {
        CONTINENTAL_TRANSFORM
    } else if class == BoundaryClass::Ridge {
        RIDGE_STRIKE_SLIP
    } else if speed < SLOW_TRANSFORM {
        OCEANIC_TRANSFORM[0]
    } else if speed < MEDIUM_TRANSFORM {
        OCEANIC_TRANSFORM[1]
    } else {
        OCEANIC_TRANSFORM[2]
    }
}

/// The lid factor of plume melting under lithosphere `crust` at the simulated time `time`:
/// ((Z − z_L) / Z)², 0 to 1 (White & McKenzie 1989; Ellam 1992).
fn lid_factor(crust: &Crust, time: f64) -> f64 {
    let lid = if crust.is_continental_origin() {
        CONTINENTAL_LID
    } else {
        let age = crust.age(time).max(0.0) * 1e6 * SECONDS_PER_YEAR;
        (2.32 * (DIFFUSIVITY * age).sqrt()).min(MAX_OCEANIC_LID)
    };
    ((PLUME_MELT_DEPTH - lid) / PLUME_MELT_DEPTH)
        .max(0.0)
        .powi(2)
}

impl Simulation {
    /// The seismic and volcanic activity of every cell (see the module documentation), with
    /// the boundaries as [`Simulation::boundary_map`] classifies them.
    pub fn activity(&self, boundaries: &BoundaryMap) -> Activity {
        let mesh = &self.mesh;
        let count = mesh.len();
        let radius = self.params.planet_radius;
        let time = self.time;
        // Spacing (m) and area (m², km²) of a cell.
        let spacing = radius * mesh.mean_edge_angle();
        let area = 4.0 * std::f64::consts::PI * radius * radius / count.max(1) as f64;
        let area_km2 = area / 1e6;
        let mut seismic = vec![Contributions::<7>::ZERO; count];
        let mut magma = vec![Contributions::<4>::ZERO; count];
        let crusts: Vec<Crust> = (0..count as u32)
            .map(|cell| self.surface_crust(cell))
            .collect();
        let continental = |cell: usize| crusts[cell].is_continental_origin_column();

        for cell in 0..count {
            let Some(class) = boundaries.classes[cell] else {
                continue;
            };
            if class == BoundaryClass::Inactive {
                continue;
            }
            let [convergence, strike_slip] = boundaries.motion[cell];
            let continental = continental(cell);
            // Each boundary cell takes half of its stretch of the boundary (one cell spacing);
            // the cells across it the other half.
            let share = 0.5 * spacing;
            if let Some((coupling, source)) = normal_coupling(class, continental, convergence) {
                seismic[cell].0[source as usize - 1] +=
                    coupling.moment_rate(convergence / 100.0) * share / area_km2;
            }
            let speed = (convergence * convergence + strike_slip * strike_slip).sqrt();
            let coupling = strike_slip_coupling(class, continental, speed);
            let source = if class == BoundaryClass::Ridge {
                SeismicSource::Ridge
            } else {
                SeismicSource::Transform
            };
            seismic[cell].0[source as usize - 1] +=
                coupling.moment_rate(strike_slip / 100.0) * share / area_km2;

            // Magma of opening boundaries: new crust at ridges, a share of it at rifts.
            let opening = (-convergence).max(0.0) / 100.0;
            // (Trenches and collisions that open slightly, see `normal_coupling`, get none.)
            if opening > 0.0 {
                let volume = opening * OCEANIC_THICKNESS * share;
                let (volume, source) = match class {
                    BoundaryClass::Ridge => (volume, VolcanicSource::Ridge),
                    BoundaryClass::Rift => (volume * RIFT_MELT_SHARE, VolcanicSource::Rift),
                    BoundaryClass::OtherDivergent if continental => {
                        (volume * RIFT_MELT_SHARE, VolcanicSource::Rift)
                    }
                    BoundaryClass::OtherDivergent => (volume, VolcanicSource::Ridge),
                    _ => (0.0, VolcanicSource::Ridge),
                };
                magma[cell].0[source as usize - 1] += volume / area * 1e3;
            }
        }

        // Arcs: as the model grows arc crust (see `Simulation::apply_additions`), the crust of
        // each trench stretch spread over the rings of the overriding plate behind it with the
        // arc profile.
        let profile = &self.params.arc_profile;
        let mut reached: Vec<(u32, usize)> = Vec::new();
        let mut visited = vec![usize::MAX; count];
        for seed in 0..count {
            if boundaries.classes[seed] != Some(BoundaryClass::Overriding) {
                continue;
            }
            let convergence = boundaries.motion[seed][0] / 100.0;
            if convergence <= 0.0 {
                continue;
            }
            let plate = self.owners[seed].plate;
            reached.clear();
            reached.push((seed as u32, 0));
            visited[seed] = seed;
            let mut next = 0;
            while next < reached.len() {
                let (cell, ring) = reached[next];
                next += 1;
                if ring + 1 >= profile.len() {
                    continue;
                }
                for &neighbor in mesh.neighbors(cell) {
                    if visited[neighbor as usize] != seed
                        && self.owners[neighbor as usize].plate == plate
                    {
                        visited[neighbor as usize] = seed;
                        reached.push((neighbor, ring + 1));
                    }
                }
            }
            let total: f64 = reached.iter().map(|&(_, ring)| profile[ring]).sum();
            if total <= 0.0 {
                continue;
            }
            let volume = convergence * self.params.arc_crust_per_subduction * spacing;
            for &(cell, ring) in &reached {
                magma[cell as usize].0[VolcanicSource::Arc as usize - 1] +=
                    volume * profile[ring] / total / area * 1e3;
            }
        }

        // Hotspots: each plume's magma as a Gaussian around it, scaled by the lid of the
        // lithosphere above it.
        let cutoff = PLUME_CUTOFF / radius;
        let mut weights: Vec<(usize, f64)> = Vec::new();
        for plume in self.plumes() {
            let flux = plume.flux();
            if flux <= 0.0 {
                continue;
            }
            let above = (0..count)
                .max_by(|&a, &b| {
                    let position = |cell: usize| mesh.vertex(cell as u32).dot(plume.position);
                    position(a).total_cmp(&position(b))
                })
                .unwrap_or(0);
            let volume = PLUME_MAGMA_PER_FLUX * flux * lid_factor(&crusts[above], time);
            if volume <= 0.0 {
                continue;
            }
            weights.clear();
            for cell in 0..count {
                let chord = mesh.vertex(cell as u32).distance(plume.position);
                if chord <= cutoff {
                    let distance = chord * radius;
                    let weight = crate::math::exp(
                        -0.5 * (distance / PLUME_FOOTPRINT) * (distance / PLUME_FOOTPRINT),
                    );
                    weights.push((cell, weight));
                }
            }
            let total: f64 = weights.iter().map(|&(_, weight)| weight).sum();
            if total <= 0.0 {
                continue;
            }
            for &(cell, weight) in &weights {
                magma[cell].0[VolcanicSource::Hotspot as usize - 1] +=
                    volume * weight / total / area * 1e3;
            }
        }

        // Distributed deformation inside the plates (Kostrov 1974): faults of the style of the
        // deformation (shortening, stretching or shear, from the sign of the area change) take
        // up the strain rate with the coupling of the boundaries of that style, so that a band
        // taking up a convergence v over its width releases what the boundary would.
        for (cell, contributions) in seismic.iter_mut().enumerate() {
            let (strain_rate, dilatation) = self.strain_rate(cell as u32);
            if strain_rate < MIN_STRAIN_RATE {
                continue;
            }
            let coupling = deformation_coupling(continental(cell), strain_rate, dilatation);
            // 1/s → 1/yr; N·m yr⁻¹ m⁻² → per km².
            contributions.0[SeismicSource::Deformation as usize - 1] +=
                coupling.moment_rate(strain_rate * SECONDS_PER_YEAR) * 1e6;
        }

        Activity {
            seismicity: seismic.iter().map(Contributions::total).collect(),
            seismic_source: seismic
                .iter()
                .map(|cell| cell.largest().map(|index| SeismicSource::ALL[index]))
                .collect(),
            magma: magma.iter().map(Contributions::total).collect(),
            volcanic_source: magma
                .iter()
                .map(|cell| cell.largest().map(|index| VolcanicSource::ALL[index]))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use glam::DVec3;

    use super::*;
    use crate::sphere::SphereMesh;
    use crate::tectonics::TectonicParams;

    #[test]
    fn subduction_moment_rate_per_length() {
        // 6 cm/yr: μ (c z) v / (sin 14° cos 14°) = 49 GPa · 18 km · 0.06 m/yr · 4.26.
        let rate = SUBDUCTION.moment_rate(0.06);
        assert!((rate / 2.26e14 - 1.0).abs() < 0.01, "{rate:e}");
        // Strike-slip faults are vertical: no geometry factor.
        let rate = CONTINENTAL_TRANSFORM.moment_rate(-0.03);
        assert!((rate / (27.7e9 * 8.6e3 * 0.03) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn plume_melting_shrinks_under_thick_lids() {
        let ocean = |age: f64| Crust {
            birth: -age,
            ..crate::tectonics::crust::new_oceanic_crust(0.0)
        };
        let young = lid_factor(&ocean(2.0), 0.0);
        let old = lid_factor(&ocean(100.0), 0.0);
        assert!(young > old && old > 0.0 && young <= 1.0, "{young} {old}");
        assert!((old - (85.0f64 / 180.0).powi(2)).abs() < 1e-9);
    }

    /// Two plates, split at the equator, plate 1 (north) moving with `omega` (rad/Myr).
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

    #[test]
    fn active_boundaries_are_seismic_and_quiet_ones_not() {
        let still = split(DVec3::ZERO);
        let activity = still.activity(&still.boundary_map());
        let sum = |values: &[f64]| values.iter().sum::<f64>();
        assert_eq!(sum(&activity.seismicity), 0.0);
        // Sliding past each other: strike-slip on the boundary cells only.
        let sliding = split(DVec3::Z * 0.01);
        let map = sliding.boundary_map();
        let activity = sliding.activity(&map);
        assert!(sum(&activity.seismicity) > 0.0);
        for cell in 0..map.classes.len() {
            if activity.seismicity[cell] > 0.0 {
                assert!(map.classes[cell].is_some());
                assert_eq!(
                    activity.seismic_source[cell],
                    Some(SeismicSource::Transform)
                );
            }
        }
        // Rotating about an equatorial axis: convergent on one side, opening on the other.
        let rotating = split(DVec3::X * 0.01);
        let activity = rotating.activity(&rotating.boundary_map());
        let sources: Vec<_> = activity.seismic_source.iter().flatten().collect();
        assert!(
            sources
                .iter()
                .any(|&&source| source != SeismicSource::Transform)
        );
        assert!(
            activity
                .volcanic_source
                .iter()
                .flatten()
                .any(|&source| source == VolcanicSource::Ridge || source == VolcanicSource::Rift),
            "opening boundaries supply magma"
        );
    }
}
