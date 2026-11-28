//! Presets for the biosphere and the age of the planet.
//!
//! The default values of the parameter structs describe **present-day Earth**: a living planet
//! with today's vegetation, atmosphere, Sun and biogenic ocean (see issue #28 for the
//! inventory). The presets set groups of parameters for other planets, each with its physical
//! reasoning; they are applied before the `--set` overrides, so a single parameter can still be
//! changed on top.
//!
//! The **default run** (#190) is not present-day Earth but a barren planet at the earliest
//! start: [`Biosphere::Abiotic`] at epoch 0 (4.0 Ga). `--biosphere modern --epoch 4` (URL
//! `biosphere=modern&epoch=4`) gives today's Earth, the parameter defaults unchanged.
//!
//! - [`Biosphere`]: `abiotic` (the default of a run), a planet without life: bare land, no
//!   transpiration or root-zone soil water, rivers without root-bound banks (braided), and no
//!   biogenic pelagic sediment; or `modern`, the parameter defaults.
//! - [`Epoch`]: the time of the planet's history at the start of the run, counted forwards in
//!   Gyr from the earliest start (0; today is [`PRESENT_EPOCH`], 4.0). It sets the brightness of
//!   the Sun (Gough, 1981) and the greenhouse forcing that kept the young Earth from freezing
//!   (the faint-young-Sun problem; Feulner, 2012).
//!
//! The greenhouse forcing of the epoch is the start of the carbon cycle ([`crate::carbon`],
//! #154), which then changes it during the run (`--no-carbon-cycle` keeps it).
//!
//! Not covered: dynamic vegetation, soils, ocean chemistry beyond the carbonate buffer, the
//! shorter day and the hotter mantle of a young planet.

use crate::carbon::CarbonParams;
use crate::climate::ClimateParams;
use crate::erosion::ErosionParams;
use crate::escape::EscapeParams;
use crate::math;
use crate::ramp_up::RampUpParams;
use crate::sun::{self, MAX_EPOCH, MAX_LUMINOSITY, PRESENT_EPOCH, PRESENT_SOLAR_CONSTANT};
use crate::tectonics::TectonicParams;
use crate::temperature::TemperatureParams;

/// Earth's Bond albedo (CERES: 0.29–0.30; Stephens et al., 2015), to turn a change of the solar
/// constant into a change of absorbed sunlight.
pub const BOND_ALBEDO: f64 = 0.3;
/// Radiative forcing per e-fold of CO₂ in W/m² (Myhre et al., 1998: `F = 5.35 ln(C/C₀)`).
pub const CO2_FORCING_PER_E_FOLD: f64 = 5.35;
/// Curvature of the CO₂ forcing at high CO₂ in W/m² per e-fold squared (Byrne & Goldblatt,
/// 2014: `F = 5.32 ln(C/C₀) + 0.39 ln²(C/C₀)`, fitted over 200–10 000 ppmv; see
/// [`co2_forcing`]).
pub const CO2_FORCING_CURVATURE: f64 = 0.39;

/// Radiative forcing (W/m²) of CO₂ at `ratio` times the pre-industrial 280 ppm, relative to
/// it: Myhre et al. (1998), `5.35 ln r`, below today's CO₂, and with the quadratic term of Byrne
/// & Goldblatt (2014) above it, where the logarithmic fit underestimates the forcing (by a
/// quarter at 10 000 ppmv). Their fit reaches 10 000 ppmv (r ≈ 36); beyond, e.g. the ~0.5 bar
/// that the default forcing of epoch 0 (+61.8 W/m²) needs, it is an extrapolation: radiative–
/// convective models with pressure broadening reach that forcing with ~0.1–0.3 bar, so the CO₂
/// of thick atmospheres is indicative. Smooth at `r = 1`.
///
/// Bounded below by [`ZERO_CO2_FORCING`], which `5.35 ln r` reaches at ~1 ppm: below, the
/// forcing stays at that of an atmosphere without CO₂ (also for `r = 0`).
pub fn co2_forcing(ratio: f64) -> f64 {
    let x = math::ln(ratio);
    (CO2_FORCING_PER_E_FOLD * x + CO2_FORCING_CURVATURE * x.max(0.0) * x.max(0.0))
        .max(ZERO_CO2_FORCING)
}

/// The CO₂ ratio at which [`co2_forcing`] reaches [`ZERO_CO2_FORCING`] (~1 ppm).
pub fn zero_co2_forcing_ratio() -> f64 {
    math::exp(ZERO_CO2_FORCING / CO2_FORCING_PER_E_FOLD)
}

/// The CO₂ ratio whose forcing is `forcing` (W/m²): the inverse of [`co2_forcing`]. At or
/// below [`ZERO_CO2_FORCING`], where every CO₂ under ~1 ppm has the same forcing, the largest
/// of them, [`zero_co2_forcing_ratio`].
pub fn co2_ratio_for_forcing(forcing: f64) -> f64 {
    if forcing <= ZERO_CO2_FORCING {
        return zero_co2_forcing_ratio();
    }
    let (a, b) = (CO2_FORCING_PER_E_FOLD, CO2_FORCING_CURVATURE);
    let x = if forcing <= 0.0 {
        forcing / a
    } else {
        // b x² + a x − F = 0, in the form without cancellation.
        2.0 * forcing / (a + (a * a + 4.0 * b * forcing).sqrt())
    };
    math::exp(x)
}
/// The forcing (W/m²) of an atmosphere without any CO₂, relative to today's: the lowest
/// greenhouse forcing that removing CO₂ can give. Radiative transfer puts the effect of all of
/// today's CO₂ at ~30 W/m² (clear sky ~30–32 W/m²: Kiehl & Trenberth, 1997; van Wijngaarden &
/// Happer, 2020; ~20% of the 155 W/m² greenhouse effect: Lacis et al., 2010; Schmidt et al.,
/// 2010), about a fifth of the model's own greenhouse effect `σT⁴ − (A + B T)` = 155 W/m² at
/// 14.5 °C. Bounds [`Epoch::greenhouse_forcing`]: `5.35 ln(C/C₀)` diverges for `C → 0` and
/// reaches this value at ~1 ppm, below which the logarithmic fit does not hold (the absorption
/// of the 15 µm band is no longer saturated and the forcing tends to a finite limit).
pub const ZERO_CO2_FORCING: f64 = -30.0;

/// Which life the planet carries, as far as the physical parameters see it. A run is
/// [`Biosphere::Abiotic`] unless `--biosphere` says otherwise (#190).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum Biosphere {
    /// Today's Earth: the default parameters, unchanged.
    Modern,
    /// A planet without life, like Earth before land plants (and before plankton for the
    /// pelagic sediment). In 4-seed runs over 1000 Myr at epoch 4 (means of 200–1000 Myr
    /// against modern, #190): 1.3 K colder with 0.9% more permanent ice (bright land), 31% less
    /// rain on land and 6% more arid land (less recycling), ~340 m lower land and ~2% less land
    /// (more erosion: K ×2 and more runoff), and 50% more river sediment to the sea. The start
    /// is much colder (−4 K, 21% instead of 14% permanently frozen) until the initial highlands
    /// of ~1 km are worn down. The default of a run, together with epoch 0; see
    /// `notes/issue-190/results.md` for that climate.
    #[default]
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
        carbon: &mut CarbonParams,
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
                // Rivers carry the runoff (`erosion.runoff_discharge`, #7), so the lower ω above
                // (less evapotranspiration, a larger runoff fraction) raises the discharge as
                // well: both effects make an abiotic planet erode more.
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
                // Inorganic carbonate only (#200): no pelagic ooze, so all of it precipitates by
                // saturation on the sea floor; the initial cover holds the carbonate share of the
                // burial flux (~10%) instead of the Phanerozoic platforms' ~20%.
                carbon.precipitation.pelagic_share = 0.0;
                tectonics.initial_sediment.platform_carbonate = 0.1;
                // Silicate weathering without plants (#154): roots, mycorrhiza and soil
                // organic acids speed up weathering several times; GEOCARB takes 1/4 of today's
                // rate before vascular plants (f_E = 0.25; Berner, 1994; Moulton & Berner, 1998:
                // 2–10×). Without roots the soil's CO₂ is the atmosphere's, so the weathering
                // follows the CO₂ directly instead of through the saturating law of a rooted
                // soil: `r^0.3` (Walker et al., 1981; GEOCARB's pre-plant law has `r^0.5`, which
                // overstates the dependence at the ~10³ times today's CO₂ of a young planet,
                // where the pH of the soil water limits it).
                carbon.plant_factor = 0.25;
                carbon.co2_exponent = 0.3;
                carbon.soil_co2_saturation = 0.0;
            }
        }
    }
}

/// The time of the planet's history at the start of the run, counted forwards in Gyr from the
/// earliest start of a simulation (see [`crate::sun`]): 0 is 4.0 Ga, when the planet has cooled
/// enough for plates and oceans, 0.5 the Palaeoarchean (3.5 Ga), [`PRESENT_EPOCH`] (4.0)
/// today, and up to [`MAX_EPOCH`] (5.90) the future, until the Sun is 1.2 times as bright as
/// today (beyond, an ocean planet enters a runaway greenhouse the model does not simulate).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Epoch {
    /// Gyr after epoch 0, within `0..=MAX_EPOCH`.
    pub gyr: f64,
    /// CO₂ relative to the pre-industrial 280 ppm, if given; otherwise the greenhouse forcing
    /// offsets the faint Sun (see [`Epoch::greenhouse_forcing`]).
    pub co2: Option<f64>,
}

impl Epoch {
    /// Today, with today's (pre-industrial) atmosphere.
    pub const PRESENT: Epoch = Epoch {
        gyr: PRESENT_EPOCH,
        co2: None,
    };

    pub fn new(gyr: f64, co2: Option<f64>) -> anyhow::Result<Self> {
        anyhow::ensure!(
            (0.0..=MAX_EPOCH).contains(&gyr),
            "the epoch must be between 0 (the earliest start) and {MAX_EPOCH:.2} Gyr (today is \
             {PRESENT_EPOCH}): later, the Sun is more than {MAX_LUMINOSITY} times as bright as \
             today, and an ocean planet would enter a runaway greenhouse (its ocean boils into a \
             steam atmosphere), which the model does not simulate"
        );
        if let Some(co2) = co2 {
            anyhow::ensure!(co2 > 0.0, "the CO₂ ratio must be positive");
        }
        Ok(Self { gyr, co2 })
    }

    /// The solar constant (W/m²) of the epoch, from the brightening of the Sun on the main
    /// sequence (Gough, 1981): `S(t) = S₀ / (1 + 0.4 (1 − t/t₀))` for the age `t` of the Sun and
    /// its present age `t₀`. 0.77 S₀ at epoch 0.5 (3.5 Ga), 0.92 S₀ at epoch 3 (1 Ga), 1.10 S₀
    /// at epoch 5 (1 Gyr from today).
    /// The sun model of the view ([`crate::sun::Sun`]) uses the same formula.
    pub fn solar_constant(&self) -> f64 {
        sun::solar_constant(self.sun_age())
    }

    /// The age of the sun in Gyr at the start of a run of this epoch.
    pub fn sun_age(&self) -> f64 {
        sun::age_of_run(self.gyr, 0.0)
    }

    /// Greenhouse forcing (W/m²) of the epoch relative to today's atmosphere.
    ///
    /// With a CO₂ ratio, the forcing is [`co2_forcing`]: `5.35 ln(C/C₀)` (Myhre et al., 1998)
    /// with the quadratic term of Byrne & Goldblatt (2014) above today's CO₂.
    ///
    /// Otherwise the forcing offsets the sunlight that the faint Sun does not deliver,
    /// `(S₀ − S)(1 − α)/4` with Earth's Bond albedo α. This assumes that the
    /// carbonate–silicate weathering feedback (Walker et al., 1981; Kasting, 1993) kept the climate
    /// close to today's, as the geological record suggests for most of the Archean and Proterozoic:
    /// liquid water and sediments since 3.8–4.4 Ga, and no lasting global glaciation except the
    /// Palaeoproterozoic and Neoproterozoic episodes (Feulner, 2012). At 3.5 Ga this is ~56 W/m²,
    /// in radiative–convective models of the order of 0.1 bar of CO₂, or 0.01–0.03 bar with
    /// ~0.1–0.5% of methane (Haqq-Misra et al., 2008; Wolf & Toon, 2013; Charnay et al., 2013).
    /// Methane on that scale is biogenic; for a barren planet (the default run, #190) read the
    /// forcing as CO₂ alone, ~0.1–0.3 bar ([`co2_forcing`], extrapolated, gives ~0.5 bar at epoch
    /// 0). The default is a prescribed, climate-compensated forcing at the start, not a
    /// prediction. It is uniform, while the missing sunlight is largest in the
    /// tropics, so the gradient flattens: at 3.5 Ga (4 seeds, 1000 Myr) the global mean was
    /// 0.8–1.9 K warmer than today's, the poles 8–10 K warmer, the equator ~2 K cooler, and the permanent
    /// ice covered 7–10% less of the surface. Without it the planet is a snowball (−36 °C, 95%
    /// frozen).
    ///
    /// The forcing offsets the sun of the run's start. During the run the carbon cycle
    /// ([`crate::carbon`], #154) changes it while the sun brightens (#149); with
    /// `--no-carbon-cycle` it stays fixed and long runs warm.
    ///
    /// After today the Sun is brighter and the forcing negative: the weathering feedback draws
    /// CO₂ down, at most to none at all, so the forcing is bounded below by
    /// [`ZERO_CO2_FORCING`] (−30 W/m², reached at epoch ~5.3, 1.3 Gyr from today); later epochs
    /// are warmer than today. Life does not limit the drawdown (the default planet is barren,
    /// #190). Beyond ~1.1 S₀ an ocean planet enters a moist greenhouse and loses water to space
    /// (Kasting, 1988; Kopparapu et al., 2013; simulated by [`crate::escape`], #210); runs end at
    /// 1.2 S₀ ([`MAX_EPOCH`]), before the runaway greenhouse.
    ///
    /// Both forcings are bounded below by [`ZERO_CO2_FORCING`].
    pub fn greenhouse_forcing(&self) -> f64 {
        match self.co2 {
            Some(co2) => co2_forcing(co2),
            None => (PRESENT_SOLAR_CONSTANT - self.solar_constant()) * (1.0 - BOND_ALBEDO) / 4.0,
        }
        .max(ZERO_CO2_FORCING)
    }

    /// Sets the solar constant of the run's start, the epoch (from which the sun brightens
    /// during the run) and the greenhouse forcing of the epoch at the start, from which the carbon
    /// cycle goes on. The present keeps the defaults exactly (S₀, today's age and no forcing).
    pub fn apply(&self, temperature: &mut TemperatureParams) {
        if *self == Self::PRESENT {
            return;
        }
        temperature.epoch = self.gyr;
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
    pub carbon: CarbonParams,
    pub ramp_up: RampUpParams,
    pub escape: EscapeParams,
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
            carbon: CarbonParams::default(),
            ramp_up: RampUpParams::default(),
            escape: EscapeParams::default(),
        };
        biosphere.apply(
            &mut parameters.tectonics,
            &mut parameters.erosion,
            &mut parameters.climate,
            &mut parameters.temperature,
            &mut parameters.carbon,
        );
        epoch.apply(&mut parameters.temperature);
        parameters
    }

    /// Overrides one parameter: `erosion.`, `climate.`, `temperature.`, `carbon.`, `escape.`
    /// and `ramp_up.` select those parameters, anything else is a tectonic parameter.
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        if let Some(name) = name.strip_prefix("ramp_up.") {
            self.ramp_up.set(name, value)
        } else if let Some(name) = name.strip_prefix("escape.") {
            self.escape.set(name, value)
        } else if let Some(name) = name.strip_prefix("carbon.") {
            self.carbon.set(name, value)
        } else if let Some(name) = name.strip_prefix("erosion.") {
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
    fn fingerprint(parameters: &Parameters) -> [String; 6] {
        [
            format!("{:?}", parameters.tectonics),
            format!("{:?}", parameters.erosion),
            format!("{:?}", parameters.climate),
            format!("{:?}", parameters.temperature),
            format!("{:?}", parameters.carbon),
            format!("{:?}", parameters.escape),
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
            carbon: CarbonParams::default(),
            ramp_up: RampUpParams::default(),
            escape: EscapeParams::default(),
        }
    }

    /// The CO₂ forcing is Myhre's below today's CO₂, larger above, smooth at today's CO₂, and
    /// its inverse undoes it; epoch 0 needs ~0.5 bar.
    #[test]
    fn co2_forcing_and_its_inverse() {
        assert_eq!(co2_forcing(1.0), 0.0);
        assert!((co2_forcing(0.5) - 5.35 * math::ln(0.5)).abs() < 1e-12);
        assert!(co2_forcing(2.0) > 5.35 * math::ln(2.0));
        // Byrne & Goldblatt (2014): 38.1 W/m² at 50 000 ppmv (with their 5.32).
        assert!((co2_forcing(50_000.0 / 280.0) - 38.1).abs() < 0.3);
        for forcing in [-17.8, -1.0, 0.0, 0.5, 3.7, 20.0, 61.8, 120.0] {
            let ratio = co2_ratio_for_forcing(forcing);
            assert!((co2_forcing(ratio) - forcing).abs() < 1e-9, "{forcing}");
        }
        let epoch_zero = Epoch::new(0.0, None).unwrap().greenhouse_forcing();
        let bar = co2_ratio_for_forcing(epoch_zero) * 280e-6;
        assert!((0.4..0.6).contains(&bar), "{bar}");
    }

    /// Without CO₂ the forcing is that of an atmosphere without any, not −∞: below ~1 ppm the
    /// forcing stays at [`ZERO_CO2_FORCING`], and the inverse gives the CO₂ where it is reached.
    #[test]
    fn co2_forcing_is_bounded_by_an_atmosphere_without_co2() {
        let zero = zero_co2_forcing_ratio();
        assert!((co2_forcing(zero) - ZERO_CO2_FORCING).abs() < 1e-9);
        for ratio in [0.0, 1e-9, 1e-4, 0.5 * zero] {
            assert_eq!(co2_forcing(ratio), ZERO_CO2_FORCING, "{ratio}");
        }
        assert!(co2_forcing(1.01 * zero) > ZERO_CO2_FORCING);
        for forcing in [ZERO_CO2_FORCING, -31.0, -1000.0, f64::NEG_INFINITY] {
            assert_eq!(co2_ratio_for_forcing(forcing), zero, "{forcing}");
        }
        assert!((co2_ratio_for_forcing(-29.0) - math::exp(-29.0 / 5.35)).abs() < 1e-15);
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
        let today = Epoch::new(PRESENT_EPOCH, Some(1.0)).unwrap();
        assert_eq!(today.greenhouse_forcing().to_bits(), 0);
        assert_eq!(Epoch::new(PRESENT_EPOCH, None).unwrap(), Epoch::PRESENT);
    }

    #[test]
    fn the_young_sun_is_fainter() {
        let at =
            |gyr: f64| Epoch::new(gyr, None).unwrap().solar_constant() / PRESENT_SOLAR_CONSTANT;
        assert_eq!(at(PRESENT_EPOCH), 1.0);
        assert!((at(3.5) - 0.958).abs() < 1e-3, "{}", at(3.5));
        assert!((at(0.5) - 0.766).abs() < 1e-3, "{}", at(0.5));
        // Epoch 0: 4.0 Ga, 0.57 Gyr after the zero age of the Sun.
        assert!((at(0.0) - 0.741).abs() < 1e-3, "{}", at(0.0));
        assert!((at(5.0) - 1.096).abs() < 1e-3, "{}", at(5.0));
        assert!((at(MAX_EPOCH) - 1.2).abs() < 1e-9, "{}", at(MAX_EPOCH));
        assert!(Epoch::new(MAX_EPOCH + 0.01, None).is_err());
        assert!(Epoch::new(-0.1, None).is_err());
        assert!(Epoch::new(1.0, Some(0.0)).is_err());
    }

    #[test]
    fn the_climate_follows_the_brightening_sun() {
        for (epoch, time) in [
            (PRESENT_EPOCH, 500.0),
            (0.5, 1000.0),
            (2.0, 37.5),
            (5.8, 80.0),
        ] {
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
                "epoch {epoch} + {time} Myr: {climate} vs {}",
                sun.luminosity * PRESENT_SOLAR_CONSTANT
            );
            // About 1% brighter per 100 Myr (0.7% at 3.5 Ga, 0.9% today, 1.1% at epoch 5.8).
            let per_100_myr = temperature.solar_constant(100.0) / temperature.solar_constant(0.0);
            assert!((1.006..1.013).contains(&per_100_myr), "{per_100_myr}");
        }
        // An override of the solar constant is scaled by the brightening; `solar_brightening`
        // 0 keeps it.
        let mut parameters = Parameters::new(RADIUS, Biosphere::Modern, Epoch::PRESENT);
        parameters
            .set("temperature.solar_constant", 1300.0)
            .unwrap();
        let scaled = parameters.temperature.solar_constant(500.0);
        let expected = 1300.0 * crate::sun::Sun::of_run(PRESENT_EPOCH, 500.0).luminosity;
        assert!((scaled - expected).abs() < 1e-9, "{scaled} vs {expected}");
        parameters
            .set("temperature.solar_brightening", 0.0)
            .unwrap();
        assert_eq!(parameters.temperature.solar_constant(500.0), 1300.0);
    }

    #[test]
    fn greenhouse_forcing_offsets_the_faint_sun_or_follows_co2() {
        let archean = Epoch::new(0.5, None).unwrap();
        let forcing = archean.greenhouse_forcing();
        assert!((54.0..58.0).contains(&forcing), "{forcing}");
        // Absorbed sunlight plus forcing equals today's absorbed sunlight.
        let absorbed = |s: f64| s * (1.0 - BOND_ALBEDO) / 4.0;
        assert!(
            (absorbed(archean.solar_constant()) + forcing - absorbed(PRESENT_SOLAR_CONSTANT)).abs()
                < 1e-9
        );
        // A doubling of CO₂: 3.7 W/m² (Myhre) plus 0.19 W/m² of the quadratic term.
        let doubled = Epoch::new(PRESENT_EPOCH, Some(2.0))
            .unwrap()
            .greenhouse_forcing();
        assert!((doubled - 3.895).abs() < 1e-3, "{doubled}");
        let mut temperature = TemperatureParams::default();
        archean.apply(&mut temperature);
        assert_eq!(temperature.greenhouse_forcing, forcing);
        assert_eq!(temperature.orbit.solar_constant, archean.solar_constant());
        assert_eq!(temperature.epoch, 0.5);
        // A brighter future Sun: the forcing offsets it down to the atmosphere without CO₂
        // (epoch ~5.3), then stays there.
        for gyr in [4.5, 5.0, 5.25] {
            let soon = Epoch::new(gyr, None).unwrap();
            assert!(soon.greenhouse_forcing() > ZERO_CO2_FORCING, "{gyr}");
            assert!(
                (absorbed(soon.solar_constant()) + soon.greenhouse_forcing()
                    - absorbed(PRESENT_SOLAR_CONSTANT))
                .abs()
                    < 1e-9
            );
        }
        for gyr in [5.3, 5.6, MAX_EPOCH] {
            let late = Epoch::new(gyr, None).unwrap();
            assert_eq!(late.greenhouse_forcing(), ZERO_CO2_FORCING, "{gyr}");
        }
        // The logarithmic fit reaches it at ~1 ppm; less CO₂ changes nothing.
        let ppm = 280.0 * math::exp(ZERO_CO2_FORCING / CO2_FORCING_PER_E_FOLD);
        assert!((0.9..1.2).contains(&ppm), "{ppm}");
        let traces = Epoch::new(PRESENT_EPOCH, Some(1e-6)).unwrap();
        assert_eq!(traces.greenhouse_forcing(), ZERO_CO2_FORCING);
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
        undone.carbon.plant_factor = modern.carbon.plant_factor;
        undone.carbon.co2_exponent = modern.carbon.co2_exponent;
        undone.carbon.soil_co2_saturation = modern.carbon.soil_co2_saturation;
        undone.carbon.precipitation.pelagic_share = modern.carbon.precipitation.pelagic_share;
        undone.tectonics.initial_sediment.platform_carbonate =
            modern.tectonics.initial_sediment.platform_carbonate;
        assert_eq!(fingerprint(&undone), fingerprint(&modern));
    }

    #[test]
    fn overrides_win_over_presets() {
        let mut parameters = Parameters::new(
            RADIUS,
            Biosphere::Abiotic,
            Epoch::new(0.5, Some(10.0)).unwrap(),
        );
        assert!(
            (parameters.temperature.greenhouse_forcing
                - (5.35 * math::ln(10f64) + 0.39 * math::ln(10f64) * math::ln(10f64)))
            .abs()
                < 1e-12
        );
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
        model.update(&elevation, None, None, 0.0);
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
        let archean = Epoch::new(0.5, None).unwrap();
        let today = climate(Epoch::PRESENT, Biosphere::Modern);
        let compensated = climate(archean, Biosphere::Modern);
        let unforced = {
            // The faint Sun alone, with today's atmosphere.
            let epoch = Epoch::new(0.5, Some(1.0)).unwrap();
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
