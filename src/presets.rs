//! Presets for the biosphere and the age of the planet.
//!
//! The default parameters describe **present-day Earth**: a living planet with today's
//! vegetation, atmosphere, Sun and biogenic ocean (see issue #28 for the inventory). The presets
//! set groups of parameters for other planets, each with its physical reasoning; they are
//! applied before the `--set` overrides, so a single parameter can still be changed on top.
//!
//! - [`Biosphere`]: `modern` (the defaults) or `abiotic`, a planet without life: bare land, no
//!   transpiration or root-zone soil water, rivers without root-bound banks (braided), and no
//!   biogenic pelagic sediment.
//! - [`Epoch`]: the time before present, which sets the brightness of the young Sun (Gough, 1981)
//!   and the greenhouse forcing that kept the young Earth from freezing (the faint-young-Sun
//!   problem; Feulner, 2012).
//!
//! Not covered: dynamic vegetation, weathering and soils, a carbon cycle (the forcing is
//! prescribed), ocean chemistry, the shorter day and the hotter mantle of a young planet.

use crate::climate::ClimateParams;
use crate::erosion::ErosionParams;
use crate::math;
use crate::sun::{self, PRESENT_SOLAR_CONSTANT, SOLAR_AGE};
use crate::tectonics::TectonicParams;
use crate::temperature::TemperatureParams;

/// Earth's Bond albedo (CERES: 0.29–0.30; Stephens et al., 2015), to turn a change of the solar
/// constant into a change of absorbed sunlight.
pub const BOND_ALBEDO: f64 = 0.3;
/// Radiative forcing per e-fold of CO₂ in W/m² (Myhre et al., 1998: `F = 5.35 ln(C/C₀)`).
pub const CO2_FORCING_PER_E_FOLD: f64 = 5.35;

/// Which life the planet carries, as far as the physical parameters see it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum Biosphere {
    /// Today's Earth: the default parameters, unchanged.
    #[default]
    Modern,
    /// A planet without life, like Earth before land plants (and before plankton for the
    /// pelagic sediment). In 4-seed runs over 1000 Myr (means of 200–1000 Myr against modern):
    /// 2.1 K colder with 2.6% more permanent ice (bright land), 35% less rain on land and 11%
    /// more arid land (less recycling), ~80 m lower land and ~2% less land (K ×2), and the same
    /// sediment flux to the ocean within the scatter. The initial highlands of ~1.5 km lie
    /// close to the threshold of land ice in this model, so the first ~100 Myr are much colder
    /// (−3 to −9 K, ice sheets even on tropical plateaus).
    Abiotic,
}

impl Biosphere {
    /// Sets the parameters that depend on the biosphere. [`Biosphere::Modern`] keeps the
    /// defaults.
    pub fn apply(
        self,
        tectonics: &mut TectonicParams,
        erosion: &mut ErosionParams,
        climate: &mut ClimateParams,
        temperature: &mut TemperatureParams,
    ) {
        match self {
            Biosphere::Modern => {}
            Biosphere::Abiotic => {
                // Bare rock, regolith and desert instead of today's vegetation. Snow-free land
                // surfaces have an albedo of ~0.15–0.2 under vegetation and ~0.3–0.4 bare
                // (deserts 0.35–0.45, granite 0.3, basalt 0.1–0.15), so the land mean rises by
                // ~0.1. The top of the atmosphere sees about half of a change of the surface
                // albedo (Donohoe & Battisti, 2011): +0.05 for the planetary albedo of land.
                temperature.land_albedo = 0.37;
                // Without transpiration (~60% of today's land evapotranspiration; Jasechko et
                // al., 2013; Schlesinger & Jasechko, 2014) and without the water storage of a
                // root zone, less of the rain returns to the air. Fu's ω grows with plant
                // available water storage (Milly, 1994; Donohue et al., 2012); fits to catchments
                // range from ~1.4 (sparse cover) to ~3.5 (Zhang et al., 2004; Li et al., 2013).
                // The potential evaporation, a property of the air above a wet surface, stays.
                climate.budyko_omega = 1.6;
                // Hillslopes and channels without root cohesion or a protective cover: sediment
                // yields of humid regions were several times higher before land plants (Langbein &
                // Schumm, 1958; Schumm, 1968), and erosion rises steeply as vegetation cover
                // falls (Vanacker et al., 2007). Bedrock incision depends on vegetation only
                // indirectly, so K doubles (the low end of ×2–5). It applies to sediment as well
                // (its erodibility relative to bedrock is a lithological contrast and stays).
                // Since the discharge follows the precipitation rather than the runoff (#7), the
                // drier interiors of an abiotic planet erode less here, where on Earth the larger
                // runoff fraction would erode more: the model understates abiotic erosion.
                erosion.erodibility *= 2.0;
                // Pre-vegetation rivers were braided and sheet-braided (Davies & Gibling, 2010;
                // Gibling & Davies, 2012), without the bank strength of roots and mud that makes
                // channels single-threaded (Tal & Paola, 2007; McMahon & Davies, 2018: alluvial
                // mudrock is scarce before land plants): wide, laterally unstable channels spread the sediment
                // over braidplains, the upper end of the alluvial diffusivities (10³–10⁴ m²/yr;
                // Paola et al., 1992).
                erosion.sediment_diffusivity = 1e4;
                // No carbonate or siliceous ooze: only red clay of dust, ash and hydrogenous
                // precipitates on the sea floor. The clay rate stays at 1 m/Myr (today's red clay
                // with some siliceous ooze): bare continents shed more dust (the glacial, less
                // vegetated Earth delivered 2–4× today's dust to the ocean; Kohfeld & Harrison,
                // 2001; Maher et al., 2010), which makes up for the missing ooze.
                tectonics.initial_sediment.biogenic_pelagic = false;
            }
        }
    }
}

/// The time of the planet's history to simulate, as the time before present in Gyr (Ga).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Epoch {
    /// Time before present in Gyr: 0 is today, 3.5 the Palaeoarchean.
    pub before_present: f64,
    /// CO₂ relative to the pre-industrial 280 ppm, if given; otherwise the greenhouse forcing
    /// offsets the faint Sun (see [`Epoch::greenhouse_forcing`]).
    pub co2: Option<f64>,
}

impl Epoch {
    /// Today, with today's (pre-industrial) atmosphere.
    pub const PRESENT: Epoch = Epoch {
        before_present: 0.0,
        co2: None,
    };

    pub fn new(before_present: f64, co2: Option<f64>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            (0.0..SOLAR_AGE).contains(&before_present),
            "the epoch must be between 0 and {SOLAR_AGE} Ga"
        );
        if let Some(co2) = co2 {
            anyhow::ensure!(co2 > 0.0, "the CO₂ ratio must be positive");
        }
        Ok(Self {
            before_present,
            co2,
        })
    }

    /// The solar constant (W/m²) of the epoch, from the brightening of the Sun on the main
    /// sequence (Gough, 1981): `S(t) = S₀ / (1 + 0.4 (1 − t/t₀))` for the age `t` of the Sun and
    /// its present age `t₀`. 0.77 S₀ at 3.5 Ga, 0.92 S₀ at 1 Ga.
    /// The sun model of the view ([`crate::sun::Sun`]) uses the same formula.
    pub fn solar_constant(&self) -> f64 {
        sun::solar_constant(self.sun_age())
    }

    /// The age of the sun in Gyr at the start of a run of this epoch.
    pub fn sun_age(&self) -> f64 {
        sun::age_of_run(self.before_present, 0.0)
    }

    /// Greenhouse forcing (W/m²) of the epoch relative to today's atmosphere.
    ///
    /// With a CO₂ ratio, the forcing is `5.35 ln(C/C₀)` (Myhre et al., 1998); that fit holds up to
    /// a few tens of times the pre-industrial CO₂, beyond it line-by-line models give somewhat
    /// more (Byrne & Goldblatt, 2014).
    ///
    /// Otherwise the forcing offsets the sunlight that the faint Sun does not deliver,
    /// `(S₀ − S)(1 − α)/4` with Earth's Bond albedo α. The model has no carbon cycle; this
    /// assumes that the carbonate–silicate weathering feedback (Walker et al., 1981; Kasting,
    /// 1993) kept the climate close to today's, as the geological record suggests for most of
    /// the Archean and Proterozoic: liquid water and sediments since 3.8–4.4 Ga, and no lasting
    /// global glaciation except the Palaeoproterozoic and Neoproterozoic episodes (Feulner,
    /// 2012). At 3.5 Ga this is ~56 W/m², in radiative–convective models of the order of 0.1
    /// bar of CO₂, or 0.01–0.03 bar with ~0.1–0.5% of methane (Haqq-Misra et al., 2008; Wolf &
    /// Toon, 2013; Charnay et al., 2013). The default is a prescribed, climate-compensated
    /// forcing, not a prediction. It is uniform, while the missing sunlight is largest in the
    /// tropics, so the gradient flattens: at 3.5 Ga (4 seeds, 1000 Myr) the global mean was
    /// 0.8–1.9 K warmer than today's, the poles 8–10 K warmer, the equator ~2 K cooler, and the
    /// permanent ice covered 7–10% less of the surface. Without it the planet is a snowball
    /// (−36 °C, 95% frozen).
    ///
    /// The forcing offsets the sun of the run's start and stays fixed while the sun brightens
    /// during the run (#149), so long runs warm: there is no carbon cycle that would draw the
    /// greenhouse down.
    pub fn greenhouse_forcing(&self) -> f64 {
        match self.co2 {
            Some(co2) => CO2_FORCING_PER_E_FOLD * math::ln(co2),
            None => (PRESENT_SOLAR_CONSTANT - self.solar_constant()) * (1.0 - BOND_ALBEDO) / 4.0,
        }
    }

    /// Sets the solar constant of the run's start, the epoch (from which the sun brightens
    /// during the run) and the greenhouse forcing of the epoch. The forcing stays fixed during
    /// the run. The present keeps the defaults exactly (S₀, today's age and no forcing).
    pub fn apply(&self, temperature: &mut TemperatureParams) {
        if *self == Self::PRESENT {
            return;
        }
        temperature.epoch = self.before_present;
        temperature.orbit.solar_constant = self.solar_constant();
        temperature.greenhouse_forcing = self.greenhouse_forcing();
    }
}

/// The parameters of a run: the defaults with the planet radius, then the presets, then the
/// individual overrides `NAME=VALUE` (see [`Parameters::set`]).
pub struct Parameters {
    pub tectonics: TectonicParams,
    pub erosion: ErosionParams,
    pub climate: ClimateParams,
    pub temperature: TemperatureParams,
}

impl Parameters {
    /// The defaults with the presets applied.
    pub fn new(planet_radius: f64, biosphere: Biosphere, epoch: Epoch) -> Self {
        let mut parameters = Self {
            tectonics: TectonicParams::default(),
            erosion: ErosionParams {
                planet_radius,
                ..ErosionParams::default()
            },
            climate: ClimateParams::default(),
            temperature: TemperatureParams::default(),
        };
        biosphere.apply(
            &mut parameters.tectonics,
            &mut parameters.erosion,
            &mut parameters.climate,
            &mut parameters.temperature,
        );
        epoch.apply(&mut parameters.temperature);
        parameters
    }

    /// Overrides one parameter: `erosion.`, `climate.` and `temperature.` select those
    /// parameters, anything else is a tectonic parameter.
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        if let Some(name) = name.strip_prefix("erosion.") {
            self.erosion.set(name, value)
        } else if let Some(name) = name.strip_prefix("climate.") {
            self.climate.set(name, value)
        } else if let Some(name) = name.strip_prefix("temperature.") {
            self.temperature.set(name, value)
        } else {
            self.tectonics.set(name, value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RADIUS: f64 = 6.371e6;

    /// The parameters printed with `Debug`, which prints every f64 exactly (shortest
    /// round-trip representation), so equal strings mean bit-identical parameters.
    fn fingerprint(parameters: &Parameters) -> [String; 4] {
        [
            format!("{:?}", parameters.tectonics),
            format!("{:?}", parameters.erosion),
            format!("{:?}", parameters.climate),
            format!("{:?}", parameters.temperature),
        ]
    }

    fn defaults() -> Parameters {
        Parameters {
            tectonics: TectonicParams::default(),
            erosion: ErosionParams {
                planet_radius: RADIUS,
                ..ErosionParams::default()
            },
            climate: ClimateParams::default(),
            temperature: TemperatureParams::default(),
        }
    }

    #[test]
    fn modern_present_keeps_the_defaults_bit_for_bit() {
        let parameters = Parameters::new(RADIUS, Biosphere::Modern, Epoch::PRESENT);
        assert_eq!(fingerprint(&parameters), fingerprint(&defaults()));
        assert_eq!(
            parameters.temperature.orbit.solar_constant.to_bits(),
            1361.0f64.to_bits()
        );
        assert_eq!(parameters.temperature.greenhouse_forcing.to_bits(), 0);
        // Today's CO₂ is no forcing either.
        let today = Epoch::new(0.0, Some(1.0)).unwrap();
        assert_eq!(today.greenhouse_forcing().to_bits(), 0);
    }

    #[test]
    fn the_young_sun_is_fainter() {
        let at = |ga: f64| Epoch::new(ga, None).unwrap().solar_constant() / PRESENT_SOLAR_CONSTANT;
        assert_eq!(at(0.0), 1.0);
        assert!((at(0.5) - 0.958).abs() < 1e-3, "{}", at(0.5));
        assert!((at(3.5) - 0.766).abs() < 1e-3, "{}", at(3.5));
        assert!((at(4.5699) - 1.0 / 1.4).abs() < 1e-4);
        assert!(Epoch::new(4.6, None).is_err());
        assert!(Epoch::new(-0.1, None).is_err());
        assert!(Epoch::new(1.0, Some(0.0)).is_err());
    }

    #[test]
    fn the_climate_follows_the_brightening_sun() {
        for (epoch, time) in [(0.0, 500.0), (3.5, 1000.0), (2.0, 37.5)] {
            let parameters =
                Parameters::new(RADIUS, Biosphere::Modern, Epoch::new(epoch, None).unwrap());
            let temperature = &parameters.temperature;
            // The start keeps the epoch's solar constant bit for bit.
            assert_eq!(
                temperature.solar_constant(0.0).to_bits(),
                temperature.orbit.solar_constant.to_bits()
            );
            // Later the sun model's, as the view shows it.
            let sun = crate::sun::Sun::of_run(epoch, time);
            let climate = temperature.solar_constant(time);
            assert!(
                (climate - sun.luminosity * PRESENT_SOLAR_CONSTANT).abs() < 1e-9,
                "{epoch} Ga + {time} Myr: {climate} vs {}",
                sun.luminosity * PRESENT_SOLAR_CONSTANT
            );
            // About 1% brighter per 100 Myr (0.7% at 3.5 Ga, 0.9% today).
            let per_100_myr = temperature.solar_constant(100.0) / temperature.solar_constant(0.0);
            assert!((1.006..1.012).contains(&per_100_myr), "{per_100_myr}");
        }
        // An override of the solar constant is scaled by the brightening; `solar_brightening`
        // 0 keeps it.
        let mut parameters = Parameters::new(RADIUS, Biosphere::Modern, Epoch::PRESENT);
        parameters
            .set("temperature.solar_constant", 1300.0)
            .unwrap();
        let scaled = parameters.temperature.solar_constant(500.0);
        let expected = 1300.0 * crate::sun::Sun::of_run(0.0, 500.0).luminosity;
        assert!((scaled - expected).abs() < 1e-9, "{scaled} vs {expected}");
        parameters
            .set("temperature.solar_brightening", 0.0)
            .unwrap();
        assert_eq!(parameters.temperature.solar_constant(500.0), 1300.0);
    }

    #[test]
    fn greenhouse_forcing_offsets_the_faint_sun_or_follows_co2() {
        let archean = Epoch::new(3.5, None).unwrap();
        let forcing = archean.greenhouse_forcing();
        assert!((54.0..58.0).contains(&forcing), "{forcing}");
        // Absorbed sunlight plus forcing equals today's absorbed sunlight.
        let absorbed = |s: f64| s * (1.0 - BOND_ALBEDO) / 4.0;
        assert!(
            (absorbed(archean.solar_constant()) + forcing - absorbed(PRESENT_SOLAR_CONSTANT)).abs()
                < 1e-9
        );
        // A doubling of CO₂: 3.7 W/m².
        let doubled = Epoch::new(0.0, Some(2.0)).unwrap().greenhouse_forcing();
        assert!((doubled - 3.708).abs() < 1e-3, "{doubled}");
        let mut temperature = TemperatureParams::default();
        archean.apply(&mut temperature);
        assert_eq!(temperature.greenhouse_forcing, forcing);
        assert_eq!(temperature.orbit.solar_constant, archean.solar_constant());
    }

    #[test]
    fn abiotic_preset_changes_only_its_parameters() {
        let modern = Parameters::new(RADIUS, Biosphere::Modern, Epoch::PRESENT);
        let abiotic = Parameters::new(RADIUS, Biosphere::Abiotic, Epoch::PRESENT);
        assert_eq!(abiotic.temperature.land_albedo, 0.37);
        assert_eq!(abiotic.climate.budyko_omega, 1.6);
        assert_eq!(
            abiotic.erosion.erodibility,
            2.0 * modern.erosion.erodibility
        );
        assert_eq!(abiotic.erosion.sediment_diffusivity, 1e4);
        assert!(!abiotic.tectonics.initial_sediment.biogenic_pelagic);
        // Everything else stays: undo the preset's changes and compare.
        let mut undone = Parameters::new(RADIUS, Biosphere::Abiotic, Epoch::PRESENT);
        undone.temperature.land_albedo = modern.temperature.land_albedo;
        undone.climate.budyko_omega = modern.climate.budyko_omega;
        undone.erosion.erodibility = modern.erosion.erodibility;
        undone.erosion.sediment_diffusivity = modern.erosion.sediment_diffusivity;
        undone.tectonics.initial_sediment.biogenic_pelagic = true;
        assert_eq!(fingerprint(&undone), fingerprint(&modern));
    }

    #[test]
    fn overrides_win_over_presets() {
        let mut parameters = Parameters::new(
            RADIUS,
            Biosphere::Abiotic,
            Epoch::new(3.5, Some(10.0)).unwrap(),
        );
        assert!((parameters.temperature.greenhouse_forcing - 5.35 * math::ln(10f64)).abs() < 1e-12);
        parameters.set("climate.budyko_omega", 2.0).unwrap();
        parameters.set("temperature.land_albedo", 0.3).unwrap();
        parameters
            .set("temperature.greenhouse_forcing", 12.0)
            .unwrap();
        parameters
            .set("temperature.solar_constant", 1300.0)
            .unwrap();
        parameters.set("erosion.erodibility", 1e-7).unwrap();
        parameters
            .set("initial_sediment.biogenic_pelagic", 1.0)
            .unwrap();
        assert_eq!(parameters.climate.budyko_omega, 2.0);
        assert_eq!(parameters.temperature.land_albedo, 0.3);
        assert_eq!(parameters.temperature.greenhouse_forcing, 12.0);
        assert_eq!(parameters.temperature.orbit.solar_constant, 1300.0);
        assert_eq!(parameters.erosion.erodibility, 1e-7);
        assert!(parameters.tectonics.initial_sediment.biogenic_pelagic);
        assert!(parameters.set("no_such_parameter", 1.0).is_err());
    }

    /// Global mean temperature (°C), mean temperature poleward of 60° and permanently frozen
    /// fraction of a planet with one continent, under the temperature parameters of `epoch` and
    /// `biosphere`.
    fn climate(epoch: Epoch, biosphere: Biosphere) -> (f64, f64, f64) {
        use crate::sphere::SphereMesh;
        use crate::temperature::TemperatureModel;
        let mesh = SphereMesh::icosphere(4);
        // Land between 30°S and 50°N at longitudes -60°..60°, the rest is ocean.
        let elevation: Vec<f64> = (0..mesh.len() as u32)
            .map(|cell| {
                let p = mesh.vertex(cell);
                let latitude = math::asin(p.z).to_degrees();
                let longitude = math::atan2(p.y, p.x).to_degrees();
                if (-30.0..50.0).contains(&latitude) && longitude.abs() < 60.0 {
                    300.0
                } else {
                    -4000.0
                }
            })
            .collect();
        let mut parameters = Parameters::new(RADIUS, biosphere, epoch);
        parameters.temperature.spin_up_years = 60;
        let mut model = TemperatureModel::new(&mesh, parameters.temperature);
        model.update(&elevation, None, 0.0);
        let fields = model.fields();
        let count = mesh.len() as f64;
        let global = fields.annual_mean.iter().sum::<f64>() / count;
        let polar: Vec<f64> = (0..mesh.len())
            .filter(|&c| mesh.vertex(c as u32).z.abs() > math::sin(60f64.to_radians()))
            .map(|c| fields.annual_mean[c])
            .collect();
        let polar = polar.iter().sum::<f64>() / polar.len() as f64;
        let frozen = fields.ice.iter().filter(|&&i| i > 0.5).count() as f64 / count;
        (global, polar, frozen)
    }

    /// The faint Sun of 3.5 Ga freezes the planet without a greenhouse forcing, and the default
    /// forcing of the epoch keeps it temperate: the forcing is uniform while the missing
    /// sunlight is largest in the tropics, so the poles end up warmer than today's.
    #[test]
    fn the_greenhouse_forcing_resolves_the_faint_young_sun() {
        let archean = Epoch::new(3.5, None).unwrap();
        let today = climate(Epoch::PRESENT, Biosphere::Modern);
        let compensated = climate(archean, Biosphere::Modern);
        let unforced = {
            // The faint Sun alone, with today's atmosphere.
            let epoch = Epoch::new(3.5, Some(1.0)).unwrap();
            climate(epoch, Biosphere::Modern)
        };
        let abiotic = climate(archean, Biosphere::Abiotic);
        // Measured: today (14.5 °C, poles −9.9 °C, 13% frozen), 3.5 Ga compensated (15.3, −2.7,
        // 5%), unforced (−21.2, −38.5, 70%), abiotic 3.5 Ga (13.7, −3.7, 7%).
        let all = format!("{today:?} {compensated:?} {unforced:?} {abiotic:?}");
        assert!(unforced.2 > 0.5 && unforced.0 < -10.0, "{all}");
        assert!((compensated.0 - today.0).abs() < 3.0, "{all}");
        assert!(compensated.1 > today.1 + 3.0, "{all}");
        assert!(compensated.2 < today.2, "{all}");
        // Bright bare land cools the planet.
        assert!(abiotic.0 < compensated.0, "{all}");
    }
}
