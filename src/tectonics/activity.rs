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

/// Duration (Myr) of the main pulse of a large igneous province, over which its volume is shown
/// as a rate: LIPs emplace most (≥ 75 %) of their volume within 1–5 Myr (Bryan & Ernst 2008);
/// the model emplaces each in one step.
const LIP_PULSE: f64 = 3.0;

/// Erupted (extrusive) share of the magma supply per setting, 1 / (1 + I/E) with the
/// intrusive-to-extrusive ratio I/E (Crisp 1984; White, Crisp & Spera 2006):
/// - ridges: I/E ≈ 6 (oceanic crust is ~1 km of lavas over ~6 km of dikes and gabbro);
/// - arcs: I/E ≈ 5;
/// - oceanic hotspots (islands, seamounts): I/E ≈ 2; hotspots under continents: I/E ≈ 10, as
///   continental magmatism generally (most magma stalls in the thick crust);
/// - continental rifts: I/E ≈ 10;
/// - large igneous provinces: mostly extrusive (flood basalts, plateau lavas), I/E ≈ 1 assumed
///   for the province with its sills and underplate.
const RIDGE_ERUPTED: f64 = 1.0 / 7.0;
const ARC_ERUPTED: f64 = 1.0 / 6.0;
const OCEANIC_HOTSPOT_ERUPTED: f64 = 1.0 / 3.0;
const CONTINENTAL_ERUPTED: f64 = 1.0 / 11.0;
const LIP_ERUPTED: f64 = 0.5;

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
    /// A large igneous province (a plume head), during its main pulse.
    Lip = 5,
}

impl VolcanicSource {
    pub const ALL: [VolcanicSource; 5] = [
        VolcanicSource::Arc,
        VolcanicSource::Ridge,
        VolcanicSource::Hotspot,
        VolcanicSource::Rift,
        VolcanicSource::Lip,
    ];

    /// The source of a code (1 to 5), `None` for 0 and unknown codes.
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
            VolcanicSource::Lip => "large igneous province",
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
    /// Eruption rate per area in mm/yr: the erupted (extrusive) share of the magma supply.
    pub eruptions: Vec<f64>,
    /// Its largest contribution, if there is any.
    pub eruption_source: Vec<Option<VolcanicSource>>,
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
        let mut magma = vec![Contributions::<5>::ZERO; count];
        let mut eruptions = vec![Contributions::<5>::ZERO; count];
        // Adds `supply` mm/yr of magma of `source` to a cell, `erupted` of it erupting.
        let mut add_magma = |cell: usize, source: VolcanicSource, supply: f64, erupted: f64| {
            magma[cell].0[source as usize - 1] += supply;
            eruptions[cell].0[source as usize - 1] += supply * erupted;
        };
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
                let ridge = (volume, VolcanicSource::Ridge, RIDGE_ERUPTED);
                let rift = (
                    volume * RIFT_MELT_SHARE,
                    VolcanicSource::Rift,
                    CONTINENTAL_ERUPTED,
                );
                let (volume, source, erupted) = match class {
                    BoundaryClass::Ridge => ridge,
                    BoundaryClass::Rift => rift,
                    BoundaryClass::OtherDivergent if continental => rift,
                    BoundaryClass::OtherDivergent => ridge,
                    _ => (0.0, VolcanicSource::Ridge, 0.0),
                };
                add_magma(cell, source, volume / area * 1e3, erupted);
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
                add_magma(
                    cell as usize,
                    VolcanicSource::Arc,
                    volume * profile[ring] / total / area * 1e3,
                    ARC_ERUPTED,
                );
            }
        }

        // Plumes, as the volcanism emplaces them (see `Simulation::volcanism_step`): the tail
        // of every plume whose head has arrived, scaled by the lid of the lithosphere above it,
        // and the large igneous provinces of the last `LIP_PULSE` Myr, each as a Gaussian
        // around the plume.
        let volcanism = &self.params.volcanism;
        let mut footprint =
            |centre: glam::DVec3, width: f64, volume: f64, source: VolcanicSource, erupted: f64| {
                // Chord distances on the unit sphere, cut at 2.5 σ, normalised over the cells.
                let sigma = width / radius;
                let cutoff = (2.5 * sigma).min(2.0);
                let weights: Vec<(usize, f64)> = (0..count)
                    .filter_map(|cell| {
                        let chord = mesh.vertex(cell as u32).distance(centre);
                        (chord <= cutoff)
                            .then(|| (cell, crate::math::exp(-0.5 * (chord / sigma).powi(2))))
                    })
                    .collect();
                let total: f64 = weights.iter().map(|&(_, weight)| weight).sum();
                if total <= 0.0 {
                    let cell = mesh.nearest(centre) as usize;
                    add_magma(cell, source, volume / area * 1e3, erupted);
                    return;
                }
                for &(cell, weight) in &weights {
                    add_magma(cell, source, volume * weight / total / area * 1e3, erupted);
                }
            };
        if volcanism.enabled {
            for plume in self.plumes() {
                if plume.age < volcanism.lip_age {
                    continue;
                }
                let crust = &crusts[mesh.nearest(plume.position) as usize];
                // km³/Myr → m³/yr.
                let volume = volcanism.tail_volume_flux
                    * plume.flux()
                    * super::volcanism::lid_factor(volcanism, crust, time)
                    * 1e3;
                if volume > 0.0 {
                    let erupted = if crust.is_continental_origin() {
                        CONTINENTAL_ERUPTED
                    } else {
                        OCEANIC_HOTSPOT_ERUPTED
                    };
                    footprint(
                        plume.position,
                        volcanism.tail_width,
                        volume,
                        VolcanicSource::Hotspot,
                        erupted,
                    );
                }
            }
            for lip in self.volcanism().lips.iter().rev() {
                let since = time - lip.time;
                if since >= LIP_PULSE {
                    break;
                }
                if since < 0.0 || lip.volume <= 0.0 {
                    continue;
                }
                // As emplaced: large provinces spread wider (see `Simulation::volcanism_step`).
                let width = volcanism.lip_width.max(
                    (lip.volume * 1e9 / (std::f64::consts::TAU * volcanism.lip_plateau_thickness))
                        .sqrt(),
                );
                // km³ over the pulse → m³/yr.
                let volume = lip.volume * 1e9 / (LIP_PULSE * 1e6);
                footprint(
                    lip.position,
                    width,
                    volume,
                    VolcanicSource::Lip,
                    LIP_ERUPTED,
                );
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
            eruptions: eruptions.iter().map(Contributions::total).collect(),
            eruption_source: eruptions
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

    #[test]
    fn a_large_igneous_province_erupts_over_its_pulse() {
        let mut simulation = split(DVec3::ZERO);
        let time = simulation.time;
        let centre = simulation.mesh.vertex(500);
        simulation
            .volcanism
            .record
            .lips
            .push(crate::tectonics::volcanism::LipEvent {
                time,
                position: centre,
                volume: 1e6,
                lid_factor: 0.2,
                continental: false,
                samples: 0,
                peak_thickness: 0.0,
            });
        let activity = simulation.activity(&simulation.boundary_map());
        let radius = simulation.params.planet_radius;
        let area_km2 =
            4.0 * std::f64::consts::PI * radius * radius / 1e6 / simulation.mesh.len() as f64;
        // mm/yr × km² = 10⁻⁶ km³/yr: the province's volume over its pulse, half of it erupted.
        let lip = |values: &[f64]| {
            values
                .iter()
                .zip(&activity.volcanic_source)
                .filter(|(_, source)| **source == Some(VolcanicSource::Lip))
                .map(|(value, _)| value * area_km2 * 1e-6)
                .sum::<f64>()
        };
        let rate = 1e6 / (LIP_PULSE * 1e6);
        assert!(
            (lip(&activity.magma) / rate - 1.0).abs() < 1e-6,
            "{}",
            lip(&activity.magma)
        );
        assert!((lip(&activity.eruptions) / (rate * LIP_ERUPTED) - 1.0).abs() < 1e-6);
        assert_eq!(activity.eruption_source[500], Some(VolcanicSource::Lip));
        // Nowhere erupts more than it is supplied.
        for (erupted, supplied) in activity.eruptions.iter().zip(&activity.magma) {
            assert!(erupted <= supplied);
        }
        // After its pulse, the province is quiet.
        simulation.volcanism.record.lips[0].time = time - LIP_PULSE;
        let activity = simulation.activity(&simulation.boundary_map());
        assert!(
            activity
                .volcanic_source
                .iter()
                .all(|source| *source != Some(VolcanicSource::Lip))
        );
    }

    #[test]
    fn eruptions_are_a_share_of_the_supply_by_setting() {
        // Opening oceanic boundaries: ridges, a seventh erupting.
        let rotating = split(DVec3::X * 0.01);
        let activity = rotating.activity(&rotating.boundary_map());
        for cell in 0..activity.magma.len() {
            if activity.volcanic_source[cell] == Some(VolcanicSource::Ridge)
                && activity.magma[cell] > 0.0
            {
                let share = activity.eruptions[cell] / activity.magma[cell];
                assert!(share <= RIDGE_ERUPTED + 1e-9 && share > 0.0, "{share}");
            }
        }
        // Earth's settings: ~20 km³/yr at ridges and ~5 at arcs erupt ~3.7 km³/yr, within
        // Earth's 3–4 km³/yr (Crisp 1984; White et al. 2006).
        let earth = 20.0 * RIDGE_ERUPTED + 5.0 * ARC_ERUPTED + 0.4 * OCEANIC_HOTSPOT_ERUPTED;
        assert!((3.0..4.5).contains(&earth), "{earth}");
    }
}
