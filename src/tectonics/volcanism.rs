//! Volcanism of the mantle plumes (#106): large igneous provinces (LIPs) when a plume head
//! arrives, and hotspot tracks fed by its tail.
//!
//! Ridges and arcs add their magmatic crust elsewhere (new sea floor, arc additions); this
//! module adds the third source, the plumes, as mafic crust on the visible samples around the
//! plume:
//!
//! - **Heads** ([`VolcanismParams::lip_age`]): each plume emplaces one LIP in the step in which
//!   its head has arrived. Its volume is log-normal around [`VolcanismParams::lip_volume`],
//!   scaled by the plume's strength and by the [`lid_factor`]: decompression melting stops at
//!   the base of the lithosphere, so thin (young oceanic) lithosphere gives oceanic plateaus of
//!   10⁷–10⁸ km³ and thick continental lithosphere flood basalts of ~10⁶ km³ (White & McKenzie
//!   1989; Coffin & Eldholm 1994). Plateaus become buoyant enough to resist subduction and
//!   accrete at trenches; flood basalts raise the continent only a little (isostasy).
//! - **Tails** ([`VolcanismParams::tail_volume_flux`]): after its head, every plume adds crust
//!   in proportion to its flux ([`Plume::flux`]) and the lid factor in a narrow footprint each
//!   step. The plate moves over the fixed plume, so age-progressive tracks of shallow sea floor
//!   form (Hawaii–Emperor), and a plume below a ridge thickens the new crust (Iceland).
//!
//! The crust is spread as a radial Gaussian over the visible samples (each once, at its
//! nearest cell), normalised so that the emplaced volume is exact; the thickness cap of the
//! crust applies as for other additions, and the ledger records the actual change.
//!
//! Individual volcanoes are sub-grid (#64): a Hawaii-size shield is about half a cell, so the
//! model shows its cell mean (a track of shallower sea floor), not the peak. See
//! `notes/issue-106/`.

use glam::DVec3;
use rand::rngs::ChaCha8Rng;
use rand::{Rng, RngExt};

use super::ledger::Term;
use super::*;

/// Density (kg/m³) of the crust plumes add: basalt with its gabbroic and ultramafic cumulates
/// (oceanic plateaus 2950–3050 kg/m³; normal oceanic crust is 2900 in the model).
pub const PLUME_CRUST_DENSITY: f64 = 2950.0;

/// Thermal diffusivity of the mantle lithosphere (m²/s), for the thickness of oceanic
/// lithosphere `2.32 √(κ t)`.
const THERMAL_DIFFUSIVITY: f64 = 1e-6;

const SECONDS_PER_MYR: f64 = 1e6 * 365.25 * 24.0 * 3600.0;

/// Parameters of the plume volcanism.
#[derive(Clone, Debug)]
pub struct VolcanismParams {
    /// Whether plumes add crust (`volcanism.enabled=0` for comparison runs: the run is then
    /// that without volcanism).
    pub enabled: bool,
    /// Age (Myr) of a plume at which its head has arrived below the lithosphere and erupts its
    /// LIP: when [`Plume::head_growth`] reaches 1. Plumes older than this at the start (a
    /// primed mantle) have had their LIP.
    pub lip_age: f64,
    /// Median volume (km³) of a LIP of a plume of mean peak flux under no lid
    /// ([`lid_factor`] 1). Under typical oceanic lithosphere (lid factor ~0.2–0.4) the median
    /// is a few 10⁶ km³, under continents (~0.08) ~1.5 × 10⁶ km³; Earth's LIPs span
    /// 10⁵–10⁸ km³ with a median near 10⁶ (Bryan & Ernst 2008; Ernst 2014).
    pub lip_volume: f64,
    /// Standard deviation of the natural logarithm of the LIP volume within one setting:
    /// ln(10)/3, so that ±1.5σ span a decade. Earth's LIPs span three decades in all, but most
    /// of that is the setting (oceanic plateaus vs. flood basalts), which the lid factor gives;
    /// a wider spread made continental provinces of > 10⁷ km³ (first evaluation,
    /// `notes/issue-106/results.md`), while Earth's largest are ~4 × 10⁶ km³ (Siberia).
    pub lip_volume_spread: f64,
    /// Smallest and largest LIP volume (km³).
    pub lip_volume_range: (f64, f64),
    /// Standard deviation (m) of the Gaussian footprint of a LIP; it is cut at 2.5 times this
    /// (~1000 km, the radius of a flattened plume head). Ontong Java covers ~2 × 10⁶ km², the
    /// Deccan ~1.5 × 10⁶ km² (originally).
    pub lip_width: f64,
    /// Peak extra crust (m) of a LIP: larger provinces get a wider footprint (σ² = V / (2π h)),
    /// so the largest plateaus stand ~25 km thicker at the centre (Ontong Java's crust is
    /// 30–35 km against 7 km of normal oceanic crust).
    pub lip_plateau_thickness: f64,
    /// Volume flux (km³/Myr) of the tail per m²/s of plume flux under no lid. A typical plume
    /// (4 × 10⁻³ m²/s) below old ocean (lid factor ~0.2) adds ~2.5 × 10⁴ km³/Myr, magma with its
    /// intrusions (Hawaii 0.01–0.04 km³/yr for most of its history, 0.2 recently; Iceland
    /// ~0.1–0.2), and all plumes together ~0.3–0.5 km³/yr, Earth's oceanic intraplate
    /// magmatism (0.3–0.4 km³/yr extrusive and intrusive; Crisp 1984). The first evaluation's
    /// 10⁸ gave ~1.2 km³/yr and piled up mafic crust (`notes/issue-106/results.md`).
    pub tail_volume_flux: f64,
    /// Standard deviation (m) of the footprint of a tail (cut at 2.5 times this): edifices and
    /// their underplate span ~200–400 km.
    pub tail_width: f64,
    /// Depth (m) at which a plume starts to melt by decompression: where a plume ~200–300 K
    /// hotter than the ambient mantle crosses the dry peridotite solidus.
    pub melting_depth: f64,
    /// Thickness (m) of lithosphere of continental origin (a non-cratonic mean; the model has no
    /// cratons) and the most of oceanic lithosphere (plate model).
    pub continental_lid: f64,
    pub max_oceanic_lid: f64,
}

impl Default for VolcanismParams {
    fn default() -> Self {
        Self {
            enabled: true,
            lip_age: 10.0,
            lip_volume: 2e7,
            lip_volume_spread: std::f64::consts::LN_10 / 3.0,
            lip_volume_range: (1e5, 1e8),
            lip_width: 400e3,
            lip_plateau_thickness: 25e3,
            tail_volume_flux: 3e7,
            tail_width: 150e3,
            melting_depth: 180e3,
            continental_lid: 130e3,
            max_oceanic_lid: 95e3,
        }
    }
}

impl VolcanismParams {
    /// Sets the parameter `name` (without the `volcanism.` prefix; `enabled` is off for 0).
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        match name {
            "enabled" => self.enabled = value != 0.0,
            "lip_age" => self.lip_age = value,
            "lip_volume" => self.lip_volume = value.max(0.0),
            "lip_volume_spread" => self.lip_volume_spread = value.max(0.0),
            "lip_volume_min" => self.lip_volume_range.0 = value.max(0.0),
            "lip_volume_max" => self.lip_volume_range.1 = value.max(0.0),
            "lip_width" => self.lip_width = value.max(1.0),
            "lip_plateau_thickness" => self.lip_plateau_thickness = value.max(1.0),
            "tail_volume_flux" => self.tail_volume_flux = value.max(0.0),
            "tail_width" => self.tail_width = value.max(1.0),
            "melting_depth" => self.melting_depth = value.max(1.0),
            "continental_lid" => self.continental_lid = value.max(0.0),
            "max_oceanic_lid" => self.max_oceanic_lid = value.max(0.0),
            _ => anyhow::bail!("unknown volcanism parameter {name}"),
        }
        Ok(())
    }
}

/// Thickness (m) of the lithosphere below `crust` at the simulated time `time` (Myr): fixed
/// for lithosphere of continental origin, `2.32 √(κ t)` of the age since it formed (not the
/// plume-reset strength age) up to [`VolcanismParams::max_oceanic_lid`] otherwise.
pub fn lid_thickness(params: &VolcanismParams, crust: &Crust, time: f64) -> f64 {
    if crust.is_continental_origin() {
        params.continental_lid
    } else {
        let age = crust.age(time).max(0.0) * SECONDS_PER_MYR;
        (2.32 * (THERMAL_DIFFUSIVITY * age).sqrt()).min(params.max_oceanic_lid)
    }
}

/// How much a plume melts below `crust` relative to below no lid (0 to 1): `((Z − z_L) / Z)²`
/// with the melting depth `Z` and the lid thickness `z_L`. The melt column and its mean melt
/// fraction both grow with `Z − z_L` (White & McKenzie 1989; Ellam 1992).
pub fn lid_factor(params: &VolcanismParams, crust: &Crust, time: f64) -> f64 {
    let column = (params.melting_depth - lid_thickness(params, crust, time)).max(0.0);
    let fraction = (column / params.melting_depth).min(1.0);
    fraction * fraction
}

/// A large igneous province, as emplaced.
#[derive(Clone, Copy, Debug)]
pub struct LipEvent {
    /// Simulated time (Myr) and position (unit vector, mantle frame).
    pub time: f64,
    pub position: DVec3,
    /// Volume emplaced (km³, before the thickness cap).
    pub volume: f64,
    /// Lid factor at the plume ([`lid_factor`]).
    pub lid_factor: f64,
    /// Whether the plume rose below lithosphere of continental origin.
    pub continental: bool,
    /// Samples that received crust, and the most crust (m) one received.
    pub samples: usize,
    pub peak_thickness: f64,
}

/// The plume volcanism so far (for the statistics, #110 and #204).
#[derive(Clone, Debug, Default)]
pub struct VolcanismRecord {
    /// LIPs so far, in order.
    pub lips: Vec<LipEvent>,
    /// Volume (km³) emplaced so far by LIPs and by plume tails (before the thickness cap).
    pub lip_volume: f64,
    pub tail_volume: f64,
}

/// The volcanism's state in the simulation: its record and its own random numbers, so that it
/// changes no other random draw of the simulation.
pub(super) struct Volcanism {
    pub(super) record: VolcanismRecord,
    rng: ChaCha8Rng,
}

impl Volcanism {
    pub(super) fn new(seed: u64) -> Self {
        Self {
            record: VolcanismRecord::default(),
            rng: ChaCha8Rng::seed_from_u64(seed ^ 0x766f_6c63_616e_6f73),
        }
    }
}

/// A standard normal deviate (Box–Muller).
fn standard_normal(rng: &mut impl Rng) -> f64 {
    let u: f64 = 1.0 - rng.random_range(0.0..1.0);
    let v: f64 = rng.random_range(0.0..1.0);
    (-2.0 * math::ln(u)).sqrt() * math::cos(std::f64::consts::TAU * v)
}

/// Result of [`Simulation::emplace`].
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Emplacement {
    /// Change of the areal mass (kg/m² summed over samples).
    pub(super) mass: f64,
    pub(super) samples: usize,
    pub(super) peak_thickness: f64,
}

impl Simulation {
    /// The plume volcanism so far.
    pub fn volcanism(&self) -> &VolcanismRecord {
        &self.volcanism.record
    }

    fn cell_area_m2(&self) -> f64 {
        let radius = self.params.planet_radius;
        4.0 * std::f64::consts::PI * radius * radius / self.mesh.len() as f64
    }

    /// Adds the plumes' crust of a step of `dt` Myr, after the mantle step (plumes aged and
    /// born): LIPs of plumes whose head arrived in the step and the tails of the older ones.
    pub(super) fn volcanism_step(&mut self, dt: f64) {
        let params = self.params.volcanism.clone();
        if !params.enabled || dt <= 0.0 {
            return;
        }
        let mean_peak = 0.5 * (self.params.mantle.plume_flux.0 + self.params.mantle.plume_flux.1);
        let plumes: Vec<Plume> = self.plumes().to_vec();
        for plume in &plumes {
            let centre = self.mesh.nearest(plume.position);
            let crust = self.surface_crust(centre);
            let lid = lid_factor(&params, &crust, self.time);
            let born_before = plume.age - dt;
            if born_before < params.lip_age && plume.age >= params.lip_age {
                let strength = if mean_peak > 0.0 {
                    plume.peak_flux / mean_peak
                } else {
                    1.0
                };
                let noise =
                    math::exp(params.lip_volume_spread * standard_normal(&mut self.volcanism.rng));
                let volume = (params.lip_volume * strength * lid * noise)
                    .clamp(params.lip_volume_range.0, params.lip_volume_range.1);
                // Large provinces spread wider rather than piling up: plateaus are flat-topped at
                // up to ~25 km of extra crust (Ontong Java), not Gaussian peaks.
                let width = params.lip_width.max(
                    (volume * 1e9 / (std::f64::consts::TAU * params.lip_plateau_thickness)).sqrt(),
                );
                let emplaced = self.emplace(plume.position, volume * 1e9, width);
                self.ledger.record(Term::Lips, emplaced.mass);
                let event = LipEvent {
                    time: self.time,
                    position: plume.position,
                    volume,
                    lid_factor: lid,
                    continental: crust.is_continental_origin(),
                    samples: emplaced.samples,
                    peak_thickness: emplaced.peak_thickness,
                };
                let latitude = math::asin(event.position.z.clamp(-1.0, 1.0)).to_degrees();
                let longitude = math::atan2(event.position.y, event.position.x).to_degrees();
                tracing::info!(
                    "LIP at {:.0} Myr ({latitude:.0}°, {longitude:.0}°): {:.2e} km³ under {} \
                     lithosphere (lid factor {:.2}), {} samples, up to {:.1} km of crust",
                    event.time,
                    event.volume,
                    if event.continental {
                        "continental"
                    } else {
                        "oceanic"
                    },
                    event.lid_factor,
                    event.samples,
                    event.peak_thickness / 1000.0,
                );
                self.volcanism.record.lip_volume += volume;
                self.volcanism.record.lips.push(event);
            } else if plume.age >= params.lip_age {
                // The tail over the part of the step after the head.
                let years = dt.min(plume.age - params.lip_age);
                let volume = params.tail_volume_flux * plume.flux() * lid * years;
                if volume > 0.0 {
                    let emplaced = self.emplace(plume.position, volume * 1e9, params.tail_width);
                    self.ledger.record(Term::PlumeTails, emplaced.mass);
                    self.volcanism.record.tail_volume += volume;
                }
            }
        }
    }

    /// Adds `volume` m³ of plume crust around `centre` (unit vector) as a radial Gaussian of
    /// standard deviation `width` m, cut at 2.5 times it, over the visible samples: each sample
    /// once, with the weight of its nearest cell, normalised over the samples reached so that
    /// the volume is exact (before the thickness cap).
    pub(super) fn emplace(&mut self, centre: DVec3, volume: f64, width: f64) -> Emplacement {
        let radius = self.params.planet_radius;
        // Chord distances (unit sphere): within a few hundred km they equal the arc.
        let sigma = width / radius;
        let cutoff = (2.5 * sigma).min(2.0);
        let mut samples: Vec<(Ownership, f64)> = Vec::new();
        let mut index: HashMap<Ownership, usize> = HashMap::default();
        fn add(
            samples: &mut Vec<(Ownership, f64)>,
            index: &mut HashMap<Ownership, usize>,
            owner: Ownership,
            weight: f64,
        ) {
            match index.get(&owner) {
                Some(&i) => samples[i].1 = samples[i].1.max(weight),
                None => {
                    index.insert(owner, samples.len());
                    samples.push((owner, weight));
                }
            }
        }
        for cell in 0..self.mesh.len() as u32 {
            let chord = self.mesh.vertex(cell).distance(centre);
            if chord > cutoff {
                continue;
            }
            let weight = math::exp(-0.5 * (chord / sigma) * (chord / sigma));
            add(&mut samples, &mut index, self.owners[cell as usize], weight);
        }
        if samples.is_empty() {
            // A footprint narrower than a cell: the nearest cell takes it all.
            let owner = self.owners[self.mesh.nearest(centre) as usize];
            add(&mut samples, &mut index, owner, 1.0);
        }
        let total_weight: f64 = samples.iter().map(|(_, weight)| weight).sum();
        if total_weight <= 0.0 || volume <= 0.0 {
            return Emplacement::default();
        }
        let per_weight = volume / (total_weight * self.cell_area_m2());
        let max_thickness = self.params.max_crust_thickness;
        let mut result = Emplacement {
            samples: samples.len(),
            ..Emplacement::default()
        };
        for (owner, weight) in samples {
            let crust = self.plates[owner.plate as usize].crust[owner.local as usize]
                .as_mut()
                .expect("owners always point to existing crust");
            let before = crust.mass();
            // Only what fits below the thickness cap, so that the cap does not thin the
            // existing (possibly felsic) crust from below.
            let thickness = (per_weight * weight).min((max_thickness - crust.thickness).max(0.0));
            crust.add_crust(thickness, PLUME_CRUST_DENSITY, (0.0, 0.0), max_thickness);
            result.mass += crust.mass() - before;
            result.peak_thickness = result.peak_thickness.max(thickness);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simulation(seed: u64) -> Simulation {
        Simulation::new(SphereMesh::icosphere(4), 8, TectonicParams::default(), seed)
    }

    fn oceanic(age: f64) -> Crust {
        Crust {
            thickness: OCEANIC_THICKNESS,
            density: crate::isostasy::OCEANIC_DENSITY,
            birth: -age,
            sediment: 0.0,
            felsic: 0.0,
            metasediment: 0.0,
            orogeny: NO_OROGENY,
            orogenic: 0.0,
            rifted: 0.0,
            reheated: NO_REHEATING,
            cohort: Cohort::NONE,
        }
    }

    fn continental() -> Crust {
        Crust {
            thickness: 35_000.0,
            density: CONTINENTAL_DENSITY,
            felsic: 1.0,
            ..oceanic(500.0)
        }
    }

    #[test]
    fn lid_factor_falls_with_age_and_is_smallest_under_continents() {
        let params = VolcanismParams::default();
        let factors: Vec<f64> = [0.5, 5.0, 25.0, 60.0, 150.0]
            .iter()
            .map(|&age| lid_factor(&params, &oceanic(age), 0.0))
            .collect();
        assert!(
            factors.windows(2).all(|pair| pair[0] >= pair[1]),
            "{factors:?}"
        );
        assert!(factors.iter().all(|&f| (0.0..=1.0).contains(&f)));
        let continent = lid_factor(&params, &continental(), 0.0);
        assert!(continent < factors[4] && continent > 0.0, "{continent}");
        // Old ocean ~0.22, continent ~0.08.
        assert!((factors[4] - 0.22).abs() < 0.02, "{}", factors[4]);
        assert!((continent - 0.077).abs() < 0.01, "{continent}");
    }

    /// The emplaced crust has exactly the requested volume (each sample counted once), and the
    /// ledger closes.
    #[test]
    fn emplacement_conserves_volume_and_the_ledger_closes() {
        let mut simulation = simulation(3);
        let volume = 2e6 * 1e9;
        let before: f64 = simulation
            .plates
            .iter()
            .flat_map(|plate| plate.crust.iter().flatten())
            .map(|crust| crust.thickness)
            .sum();
        let centre = simulation.mesh.vertex(100);
        let emplaced = simulation.emplace(centre, volume, 400e3);
        simulation.ledger.record(Term::Lips, emplaced.mass);
        let after: f64 = simulation
            .plates
            .iter()
            .flat_map(|plate| plate.crust.iter().flatten())
            .map(|crust| crust.thickness)
            .sum();
        let added = (after - before) * simulation.cell_area_m2();
        assert!((added / volume - 1.0).abs() < 1e-9, "{added} vs {volume}");
        assert!(emplaced.samples > 10);
        let mass = emplaced.mass * simulation.cell_area_m2();
        assert!((mass / (volume * PLUME_CRUST_DENSITY) - 1.0).abs() < 1e-9);
        let ledger = simulation.ledger.checked(simulation.total_mass());
        assert!(ledger.residual.abs() < 1e-12 * simulation.total_mass());
        assert!(ledger.lips > 0.0);
    }

    /// A plume whose head arrives emplaces one LIP, then only feeds its tail; an old plume has
    /// no LIP. The ledger closes over the steps.
    #[test]
    fn a_plume_head_erupts_once_and_its_tail_follows() {
        let mut simulation = simulation(5);
        let lip_age = simulation.params.volcanism.lip_age;
        let position = simulation.mesh.vertex(200);
        simulation.mantle.insert_plume(Plume {
            position,
            age: lip_age - 1.5,
            lifetime: 150.0,
            peak_flux: 4e-3,
        });
        simulation.mantle.insert_plume(Plume {
            position: -position,
            age: 50.0,
            lifetime: 150.0,
            peak_flux: 4e-3,
        });
        let mass = simulation.total_mass();
        for _ in 0..4 {
            simulation.step(1.0);
        }
        let record = simulation.volcanism().clone();
        let near = |lip: &LipEvent| lip.position.dot(position) > 0.999;
        assert_eq!(
            record.lips.iter().filter(|lip| near(lip)).count(),
            1,
            "{record:?}"
        );
        assert!(record.tail_volume > 0.0);
        let ledger = simulation.ledger.checked(simulation.total_mass());
        assert!(ledger.lips > 0.0 && ledger.plume_tails > 0.0);
        assert!(ledger.residual.abs() < 1e-9 * mass, "{}", ledger.residual);
    }

    /// A large LIP turns normal sea floor into a plateau that resists subduction and stands
    /// kilometres shallower; a continental flood basalt raises its continent only mildly.
    #[test]
    fn plateaus_resist_subduction_and_flood_basalts_rise_little() {
        let mut ocean = oceanic(80.0);
        let depth = ocean.surface_elevation(0.0, 0.0);
        ocean.add_crust(20_000.0, PLUME_CRUST_DENSITY, (0.0, 0.0), 75_000.0);
        assert!(ocean.resists_subduction());
        assert!(!ocean.is_continental_origin());
        let rise = ocean.surface_elevation(0.0, 0.0) - depth;
        assert!((2000.0..4500.0).contains(&rise), "{rise}");

        let mut land = continental();
        let height = land.surface_elevation(0.0, 0.0);
        land.add_crust(1500.0, PLUME_CRUST_DENSITY, (0.0, 0.0), 75_000.0);
        let rise = land.surface_elevation(0.0, 0.0) - height;
        assert!((100.0..300.0).contains(&rise), "{rise}");
        assert!(land.is_continental_origin());
    }

    /// Where the thickness cap binds, only what fits is added: the felsic crust there stays.
    #[test]
    fn the_cap_keeps_the_felsic_crust() {
        let mut simulation = simulation(7);
        let max = simulation.params.max_crust_thickness;
        let centre = simulation.mesh.vertex(50);
        let owner = simulation.owners[50];
        let thick = Crust {
            thickness: max - 1000.0,
            ..continental()
        };
        simulation.plates[owner.plate as usize].crust[owner.local as usize] = Some(thick);
        let felsic = |simulation: &Simulation| -> f64 {
            simulation
                .plates
                .iter()
                .flat_map(|plate| plate.crust.iter().flatten())
                .map(Crust::felsic_mass)
                .sum()
        };
        let before = felsic(&simulation);
        simulation.emplace(centre, 1e7 * 1e9, 400e3);
        assert!((felsic(&simulation) - before).abs() <= 1e-12 * before);
        let crust = simulation.plates[owner.plate as usize].crust[owner.local as usize].unwrap();
        assert!((crust.thickness - max).abs() < 1e-6, "{}", crust.thickness);
    }

    /// The LIP volumes are log-normal with the given spread.
    #[test]
    fn standard_normal_has_unit_spread() {
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let draws: Vec<f64> = (0..20_000).map(|_| standard_normal(&mut rng)).collect();
        let mean = draws.iter().sum::<f64>() / draws.len() as f64;
        let variance =
            draws.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / draws.len() as f64;
        assert!(
            mean.abs() < 0.03 && (variance - 1.0).abs() < 0.05,
            "{mean} {variance}"
        );
        assert!(draws.iter().all(|x| x.is_finite()));
    }

    /// Without volcanism the simulation adds no plume crust.
    #[test]
    fn disabled_volcanism_adds_nothing() {
        let mut params = TectonicParams::default();
        params.volcanism.set("enabled", 0.0).unwrap();
        let mut simulation = Simulation::new(SphereMesh::icosphere(4), 8, params, 5);
        simulation.mantle.insert_plume(Plume {
            position: simulation.mesh.vertex(10),
            age: 9.5,
            lifetime: 150.0,
            peak_flux: 4e-3,
        });
        simulation.step(1.0);
        let ledger = simulation.ledger.checked(simulation.total_mass());
        assert_eq!(ledger.lips, 0.0);
        assert_eq!(ledger.plume_tails, 0.0);
        assert!(simulation.volcanism().lips.is_empty());
    }
}
