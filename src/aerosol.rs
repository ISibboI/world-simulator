//! Cloud condensation nuclei and the brightness of clouds (#204).
//!
//! The temperature model has no clouds of its own: they are part of its planetary albedos,
//! calibrated on today's Earth ([`crate::temperature::TemperatureParams::ocean_albedo`],
//! `land_albedo`). Those include the cloud condensation nuclei (CCN) of a living planet:
//! sulfate from the dimethyl sulfide (DMS) of plankton and marine organics over the ocean,
//! secondary organic aerosol of plants and smoke over land. A barren planet keeps sea salt,
//! mineral dust and volcanic sulfate. With fewer nuclei, liquid clouds have fewer and larger
//! droplets and reflect less sunlight (the Twomey effect; Rosing et al., 2010, for the Archean;
//! Goldblatt & Zahnle, 2011, and later work find a few to ~10 W/m²).
//!
//! This module corrects the calibrated albedos for the difference of the CCN supply to
//! pre-industrial Earth, with no free tuning: Earth's sources give no change.
//!
//! - **Sources** relative to pre-industrial Earth (1 = Earth): sea salt from the wind speed over
//!   the ocean (number flux ∝ U^3.41; Monahan et al., 1986), zonal, from the model's prescribed
//!   zonal-mean winds; marine and land biogenic sources as parameters (1 with life, 0 without);
//!   volcanic sulfate from the carbon cycle's subaerial mantle degassing (arcs and plumes, a fixed
//!   S/C ratio), smoothed over [`AerosolParams::volcanic_smoothing`]; mineral dust from dry,
//!   bare, ice-free land ([`dust_source`]).
//! - **CCN** of two air masses, marine and continental: Earth's pre-industrial number of each
//!   source times its relative strength ([`AerosolParams::marine_ccn`],
//!   [`AerosolParams::continental_ccn`]).
//! - **Droplets**: `N_d = 375 (1 − exp(−2.5e−3 N_a))` cm⁻³ (Jones et al., 1994), which
//!   saturates in polluted air.
//! - **Cloud albedo** at a fixed liquid water path: optical depth `τ ∝ N_d^{1/3}` (Twomey, 1977),
//!   two-stream albedo `α_c = τ / (τ + 7.7)` (conservative scattering, asymmetry 0.85, Lacis &
//!   Hansen, 1974), over a surface of albedo `a_s`: `A = α_c + (1 − α_c)² a_s / (1 − α_c a_s)`.
//!   The planetary albedo changes by `f T² ΔA` with the fraction `f` of low liquid clouds seen
//!   from space and the two-way transmission `T²` of the air above them. Only the cloud albedo
//!   effect: the adjustments of liquid water and cloud cover (Albrecht, 1989) are uncertain in
//!   sign and size and left out.
//! - **Direct effects** of the changed sulfate and dust burdens as a uniform albedo change.
//!
//! The changes apply to open water (weighted by the open share under thin sea ice) and
//! snow-free land; not over thick sea ice or ice sheets, whose bright surface leaves little to
//! the clouds. See `notes/issue-204/plan.md` for the values and their sources.

use crate::math;

/// Pre-industrial Earth's subaerial mantle degassing in the carbon cycle's coefficients (mol C/yr):
/// arcs, 0.015 mol/kg of the ~65 Gt/yr of subducted crust, and plumes, 1 Tmol/yr
/// ([`crate::carbon::CarbonParams`]).
pub const EARTH_VOLCANIC_DEGASSING: f64 = 0.015 * 6.5e13 + 1e12;

/// Pre-industrial Earth's dust source, as the share of the planet's surface that
/// [`dust_source`] counts: Köppen group B covers 30.2% of land (Peel et al., 2007), about 19% of
/// it desert (BW, weight 1) and 11% steppe (BS, mean weight ~0.5), on 29% land.
pub const EARTH_DUST_SOURCE: f64 = (0.19 + 0.11 * 0.5) * 0.29;

/// Earth's mean insolation (W/m²), to turn a radiative effect into an albedo.
const EARTH_MEAN_INSOLATION: f64 = 1361.0 / 4.0;

/// Exponent of the wind speed in the sea-spray number flux (Monahan et al., 1986).
const SALT_WIND_EXPONENT: f64 = 3.41;
/// Speed (m/s) of the fluctuations of weather systems added to the zonal-mean wind, as for the
/// wind stress ([`crate::climate::surface_wind_stress`]).
const WIND_VARIABILITY: f64 = 4.5;
/// Jones et al. (1994): `N_d = a (1 − exp(−b N_a))`, cm⁻³.
const JONES_MAX: f64 = 375.0;
const JONES_RATE: f64 = 2.5e-3;
/// `2 / (√3 (1 − g))` with the asymmetry parameter g = 0.85 of cloud droplets: the optical depth
/// at which a non-absorbing cloud reflects half (Lacis & Hansen, 1974).
const HALF_REFLECTION_DEPTH: f64 = 7.7;
/// Number of latitude bins for the global mean of the sea-salt profile.
const SALT_BINS: usize = 180;

/// The sources of cloud nuclei, in the order of [`AerosolParams::marine_ccn`].
pub const SOURCES: [&str; 5] = [
    "salt",
    "marine_biogenic",
    "volcanic",
    "land_biogenic",
    "dust",
];

/// Parameters of the cloud nuclei; the defaults are pre-industrial Earth with life (see
/// [`crate::presets::Biosphere`] for a barren planet).
#[derive(Clone, Debug)]
pub struct AerosolParams {
    /// Whether the cloud nuclei change the albedos; otherwise the calibrated albedos apply.
    pub enabled: bool,
    /// Whether the volcanic and dust sources follow the planet; otherwise they stay Earth's
    /// (only the biosphere changes the nuclei).
    pub dynamic_sources: bool,
    /// Marine biogenic sources (DMS, marine organics) relative to Earth's: 1 with plankton.
    pub marine_biogenic: f64,
    /// Land biogenic sources (secondary organic aerosol of plants, biogenic nucleation) and
    /// wildfire smoke relative to Earth's: 1 with land plants.
    pub land_biogenic: f64,
    /// Pre-industrial Earth's CCN (cm⁻³ at 0.2–0.3% supersaturation) of the marine boundary
    /// layer by source ([`SOURCES`]): remote marine CCN 50–150 cm⁻³, sea spray typically below
    /// 30% of it (Quinn & Bates, 2011; Quinn et al., 2017), volcanic degassing +40% droplets
    /// before industry (Schmidt et al., 2012), DMS the largest natural source over the ocean
    /// (Rap et al., 2013); the sea salt is the global mean of its zonal profile.
    pub marine_ccn: [f64; 5],
    /// The same for continental air, dominated by biogenic secondary organic aerosol, biogenic
    /// nucleation and fires before industry (Gordon et al., 2016; Rap et al., 2013); dust
    /// contributes few CCN by number.
    pub continental_ccn: [f64; 5],
    /// Optical depth of liquid clouds with Earth's droplets (MODIS: ~10 for marine stratocumulus).
    pub optical_depth: f64,
    /// Fraction of the ocean and of the land covered by low liquid clouds seen from space (not
    /// below higher clouds).
    pub ocean_cloud_fraction: f64,
    pub land_cloud_fraction: f64,
    /// Two-way transmission of sunlight through the air above the low clouds.
    pub transmission: f64,
    /// Surface albedo below the clouds of open water and of snow-free land (vegetated ~0.17,
    /// bare ~0.3).
    pub water_surface_albedo: f64,
    pub land_surface_albedo: f64,
    /// Time (Myr) over which the volcanic source follows the degassing.
    pub volcanic_smoothing: f64,
    /// The subaerial mantle degassing (mol C/yr) that gives Earth's volcanic sulfate. 0 (the
    /// default): the planet's own when its ramp-up phase ends ([`crate::ramp_up`]), as the
    /// carbon cycle calibrates φ there. The model's tectonics run faster than Earth's (new crust
    /// ~1.7×), and a fixed Earth value ([`EARTH_VOLCANIC_DEGASSING`]) would brighten the clouds
    /// of every run, today's Earth too; with the planet's own, the volcanic source follows the
    /// changes of its degassing after the ramp-up.
    pub volcanic_reference: f64,
    /// Direct radiative effect (W/m², global mean) of Earth's DMS sulfate, volcanic sulfate
    /// (Rap et al., 2013) and dust (Kok et al., 2017: net −0.2, range −0.5 to +0.2).
    pub marine_biogenic_direct: f64,
    pub volcanic_direct: f64,
    pub dust_direct: f64,
}

impl Default for AerosolParams {
    fn default() -> Self {
        Self {
            enabled: true,
            dynamic_sources: true,
            marine_biogenic: 1.0,
            land_biogenic: 1.0,
            marine_ccn: [20.0, 35.0, 30.0, 12.0, 3.0],
            continental_ccn: [15.0, 25.0, 60.0, 170.0, 30.0],
            optical_depth: 8.0,
            ocean_cloud_fraction: 0.3,
            land_cloud_fraction: 0.15,
            transmission: 0.8,
            water_surface_albedo: 0.07,
            land_surface_albedo: 0.17,
            volcanic_smoothing: 20.0,
            volcanic_reference: 0.0,
            marine_biogenic_direct: -0.23,
            volcanic_direct: -0.21,
            dust_direct: -0.2,
        }
    }
}

impl AerosolParams {
    /// Sets a parameter by its field name (`enabled`, `dynamic_sources` as 0 or 1; the CCN as
    /// `marine_ccn.<source>` and `continental_ccn.<source>`, see [`SOURCES`]).
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        if let Some((group, source)) = name.split_once('.') {
            let index = SOURCES
                .iter()
                .position(|&s| s == source)
                .ok_or_else(|| anyhow::anyhow!("unknown aerosol source {source}"))?;
            match group {
                "marine_ccn" => self.marine_ccn[index] = value,
                "continental_ccn" => self.continental_ccn[index] = value,
                _ => anyhow::bail!("unknown aerosol parameter {name}"),
            }
            return Ok(());
        }
        let field = match name {
            "enabled" => {
                self.enabled = value != 0.0;
                return Ok(());
            }
            "dynamic_sources" => {
                self.dynamic_sources = value != 0.0;
                return Ok(());
            }
            "marine_biogenic" => &mut self.marine_biogenic,
            "land_biogenic" => &mut self.land_biogenic,
            "optical_depth" => &mut self.optical_depth,
            "ocean_cloud_fraction" => &mut self.ocean_cloud_fraction,
            "land_cloud_fraction" => &mut self.land_cloud_fraction,
            "transmission" => &mut self.transmission,
            "water_surface_albedo" => &mut self.water_surface_albedo,
            "land_surface_albedo" => &mut self.land_surface_albedo,
            "volcanic_smoothing" => &mut self.volcanic_smoothing,
            "volcanic_reference" => &mut self.volcanic_reference,
            "marine_biogenic_direct" => &mut self.marine_biogenic_direct,
            "volcanic_direct" => &mut self.volcanic_direct,
            "dust_direct" => &mut self.dust_direct,
            _ => anyhow::bail!("unknown aerosol parameter {name}"),
        };
        *field = value;
        Ok(())
    }
}

/// The volcanic and dust sources of the planet relative to pre-industrial Earth's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sources {
    /// Volcanic sulfate: [`Self::degassing`] over [`Self::reference`]; 1 until the reference is
    /// set.
    pub volcanic: f64,
    pub dust: f64,
    /// Subaerial mantle degassing (mol C/yr), smoothed over
    /// [`AerosolParams::volcanic_smoothing`]; NaN before the carbon cycle's first step.
    pub degassing: f64,
    /// The degassing of Earth's volcanic sulfate (mol C/yr); NaN until it is set.
    pub reference: f64,
}

impl Default for Sources {
    /// Earth's, also for the first climate update before the planet has a carbon cycle step or
    /// a water cycle.
    fn default() -> Self {
        Self {
            volcanic: 1.0,
            dust: 1.0,
            degassing: f64::NAN,
            reference: f64::NAN,
        }
    }
}

impl Sources {
    /// The sources after a step of `dt` Myr with the subaerial mantle `degassing` (mol C/yr; none
    /// before the carbon cycle's first step) and the dust source `dust` ([`dust_source`]; none
    /// without a water cycle yet). Earth's when the sources are not dynamic.
    pub fn advance(
        self,
        params: &AerosolParams,
        degassing: Option<f64>,
        dust: Option<f64>,
        dt: f64,
    ) -> Self {
        if !params.dynamic_sources {
            return Self::default();
        }
        let degassing = match degassing {
            Some(degassing) if self.degassing.is_finite() => {
                let weight = if params.volcanic_smoothing > 0.0 {
                    1.0 - math::exp(-dt / params.volcanic_smoothing)
                } else {
                    1.0
                };
                self.degassing + weight * (degassing - self.degassing)
            }
            Some(degassing) => degassing,
            None => self.degassing,
        };
        let reference = if params.volcanic_reference > 0.0 {
            params.volcanic_reference
        } else {
            self.reference
        };
        Self {
            volcanic: if reference > 0.0 && degassing.is_finite() {
                degassing / reference
            } else {
                1.0
            },
            dust: dust.map_or(self.dust, |dust| dust / EARTH_DUST_SOURCE),
            degassing,
            reference,
        }
    }

    /// Takes `degassing` as Earth's (when the ramp-up phase ends, as the carbon
    /// cycle's φ; see [`AerosolParams::volcanic_reference`]), unless the reference is fixed.
    /// The smoothed degassing restarts from `degassing` (mol C/yr), the carbon cycle's of the
    /// settled tectonics, rather than lagging behind the start's.
    pub fn calibrate(self, params: &AerosolParams, degassing: f64) -> Self {
        if params.volcanic_reference > 0.0 || !params.dynamic_sources || degassing <= 0.0 {
            return self;
        }
        Self {
            volcanic: 1.0,
            degassing,
            reference: degassing,
            ..self
        }
    }
}

/// The dust source of a planet as a share of its surface: over the land cells (not lakes), given
/// as (annual mean temperature °C, annual precipitation m/yr, ice cover 0–1), the emission
/// weight `s(d) b(d) (1 − ice)` of the Köppen dryness `d = P / P_B` with the arid threshold
/// `P_B = 10 (2 T + 14)` mm (the threshold of rain spread over the year; `P_B ≤ 0` is frozen
/// ground and does not emit). Soil dryness `s = clamp((2 − d) / 1.5)`: full in deserts
/// (`d < 0.5`), none where it is twice as wet as the arid limit (moist soils do not emit; Fécan
/// et al., 1999). Bare share `b`: 1 without plants; with them `clamp((1 − d) / 0.5)` (bare in
/// deserts, closing across the steppe), mixed by [`AerosolParams::land_biogenic`].
/// `cells` is the number of cells of the planet.
pub fn dust_source(
    params: &AerosolParams,
    land: impl Iterator<Item = (f64, f64, f64)>,
    cells: usize,
) -> f64 {
    let vegetation = params.land_biogenic.clamp(0.0, 1.0);
    let total: f64 = land
        .map(|(temperature, precipitation, ice)| {
            let threshold = 10.0 * (2.0 * temperature + 14.0);
            if threshold <= 0.0 || !precipitation.is_finite() {
                return 0.0;
            }
            let dryness = 1000.0 * precipitation.max(0.0) / threshold;
            let soil = ((2.0 - dryness) / 1.5).clamp(0.0, 1.0);
            let covered = 1.0 - ((1.0 - dryness) / 0.5).clamp(0.0, 1.0);
            let bare = 1.0 - vegetation * covered;
            soil * bare * (1.0 - ice.clamp(0.0, 1.0))
        })
        .sum();
    total / cells.max(1) as f64
}

/// Relative sea-salt emission at `latitude` (degrees): the wind speed to the power 3.41.
fn salt_emission(latitude: f64) -> f64 {
    let (eastward, poleward) = crate::climate::zonal_mean_wind(latitude.abs());
    let speed =
        (eastward * eastward + poleward * poleward + WIND_VARIABILITY * WIND_VARIABILITY).sqrt();
    math::powf(speed, SALT_WIND_EXPONENT)
}

/// The cloud nuclei, droplets and cloud albedo of one air mass.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AirMass {
    /// CCN (cm⁻³).
    pub ccn: f64,
    /// Cloud droplets (cm⁻³).
    pub droplets: f64,
    /// Albedo of the low liquid clouds alone.
    pub cloud_albedo: f64,
}

/// The change of the planetary albedos by the cloud nuclei of a planet.
#[derive(Clone, Debug)]
pub struct CloudBrightening {
    params: AerosolParams,
    /// Relative strength of every source ([`SOURCES`]; sea salt 1, its profile is zonal).
    strength: [f64; 5],
    /// Global mean of [`salt_emission`].
    salt_mean: f64,
    /// Uniform albedo change of the direct effects.
    direct: f64,
    continental: AirMass,
    continental_reference: AirMass,
}

impl CloudBrightening {
    pub fn new(params: &AerosolParams, sources: Sources) -> Self {
        let strength = [
            1.0,
            params.marine_biogenic,
            sources.volcanic,
            params.land_biogenic,
            sources.dust,
        ];
        // Equal-area bins in sin(latitude).
        let salt_mean = (0..SALT_BINS)
            .map(|bin| {
                let x = -1.0 + (bin as f64 + 0.5) * 2.0 / SALT_BINS as f64;
                salt_emission(math::asin(x).to_degrees())
            })
            .sum::<f64>()
            / SALT_BINS as f64;
        let direct = -(params.marine_biogenic_direct * (params.marine_biogenic - 1.0)
            + params.volcanic_direct * (sources.volcanic - 1.0)
            + params.dust_direct * (sources.dust - 1.0))
            / EARTH_MEAN_INSOLATION;
        let mut brightening = Self {
            params: params.clone(),
            strength,
            salt_mean,
            direct,
            continental: AirMass::default(),
            continental_reference: AirMass::default(),
        };
        // The continental air takes the salt of the global mean.
        brightening.continental = brightening.air_mass(&params.continental_ccn, 1.0, false);
        brightening.continental_reference =
            brightening.air_mass(&params.continental_ccn, 1.0, true);
        brightening
    }

    /// The air mass with Earth's CCN `ccn` by source, sea salt `salt` times its global mean, of
    /// Earth (`reference`) or of the planet.
    fn air_mass(&self, ccn: &[f64; 5], salt: f64, reference: bool) -> AirMass {
        let ccn: f64 = ccn
            .iter()
            .zip(&self.strength)
            .enumerate()
            .map(|(source, (n, strength))| {
                let relative = if source == 0 {
                    salt
                } else if reference {
                    1.0
                } else {
                    strength.max(0.0)
                };
                n * relative
            })
            .sum();
        let droplets = JONES_MAX * -math::exp_m1(-JONES_RATE * ccn);
        AirMass {
            ccn,
            droplets,
            cloud_albedo: 0.0,
        }
    }

    /// Cloud albedo of the droplets of `air` relative to those of `reference`.
    fn cloud_albedo(&self, air: AirMass, reference: AirMass) -> f64 {
        if reference.droplets <= 0.0 {
            return 0.0;
        }
        let tau = self.params.optical_depth * math::cbrt(air.droplets / reference.droplets);
        tau / (tau + HALF_REFLECTION_DEPTH)
    }

    /// The change of the planetary albedo by clouds over a surface of albedo `surface` covered
    /// by a fraction `fraction` of low clouds, with the air `air` instead of `reference`.
    fn change(&self, air: AirMass, reference: AirMass, surface: f64, fraction: f64) -> f64 {
        let over = |cloud: f64| cloud + (1.0 - cloud).powi(2) * surface / (1.0 - cloud * surface);
        let cloud = self.cloud_albedo(air, reference);
        let earth = self.cloud_albedo(reference, reference);
        fraction * self.params.transmission * (over(cloud) - over(earth)) + self.direct
    }

    /// The marine air mass at `latitude` (radians) and the change of the planetary albedo of
    /// open ocean there.
    pub fn ocean(&self, latitude: f64) -> (AirMass, f64) {
        if !self.params.enabled {
            return (AirMass::default(), 0.0);
        }
        let salt = salt_emission(latitude.to_degrees()) / self.salt_mean;
        let mut air = self.air_mass(&self.params.marine_ccn, salt, false);
        let reference = self.air_mass(&self.params.marine_ccn, salt, true);
        air.cloud_albedo = self.cloud_albedo(air, reference);
        let change = self.change(
            air,
            reference,
            self.params.water_surface_albedo,
            self.params.ocean_cloud_fraction,
        );
        (air, change)
    }

    /// The continental air mass and the change of the planetary albedo of snow-free land.
    pub fn land(&self) -> (AirMass, f64) {
        if !self.params.enabled {
            return (AirMass::default(), 0.0);
        }
        let mut air = self.continental;
        air.cloud_albedo = self.cloud_albedo(air, self.continental_reference);
        let change = self.change(
            air,
            self.continental_reference,
            self.params.land_surface_albedo,
            self.params.land_cloud_fraction,
        );
        (air, change)
    }

    /// The change of the planetary albedo of open lake water: continental air over water.
    pub fn lake(&self) -> f64 {
        if !self.params.enabled {
            return 0.0;
        }
        self.change(
            self.continental,
            self.continental_reference,
            self.params.water_surface_albedo,
            self.params.land_cloud_fraction,
        )
    }
}

/// What the climate saw of the cloud nuclei in its last update, for the log.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AerosolDiagnostics {
    pub sources: Sources,
    /// The marine air, mean over the ocean, and the continental air.
    pub marine: AirMass,
    pub continental: AirMass,
    /// Mean change of the planetary albedo of open ocean and of snow-free land.
    pub ocean_change: f64,
    pub land_change: f64,
}

impl AerosolDiagnostics {
    /// One line for the log.
    pub fn summary(&self) -> String {
        format!(
            "aerosol: sources volcanic {:.2} (degassing {:.2}× Earth's), dust {:.2} × Earth | marine \
             CCN {:.0}, droplets {:.0} cm⁻³, cloud albedo {:.3}, ocean albedo {:+.4} | continental \
             CCN {:.0}, droplets {:.0} cm⁻³, cloud albedo {:.3}, land albedo {:+.4}",
            self.sources.volcanic,
            self.sources.degassing / EARTH_VOLCANIC_DEGASSING,
            self.sources.dust,
            self.marine.ccn,
            self.marine.droplets,
            self.marine.cloud_albedo,
            self.ocean_change,
            self.continental.ccn,
            self.continental.droplets,
            self.continental.cloud_albedo,
            self.land_change,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn barren() -> AerosolParams {
        AerosolParams {
            marine_biogenic: 0.0,
            land_biogenic: 0.0,
            land_surface_albedo: 0.3,
            ..AerosolParams::default()
        }
    }

    /// Earth's sources change nothing, at every latitude.
    #[test]
    fn earth_keeps_the_calibrated_albedos() {
        let clouds = CloudBrightening::new(&AerosolParams::default(), Sources::default());
        for degrees in [-80.0, -50.0, -20.0, 0.0, 15.0, 45.0, 70.0, 90.0] {
            let (air, change) = clouds.ocean(f64::to_radians(degrees));
            assert!(change.abs() < 1e-15, "{degrees}: {change}");
            assert!(air.ccn > 0.0 && air.droplets < air.ccn);
        }
        assert!(clouds.land().1.abs() < 1e-15);
        assert!(clouds.lake().abs() < 1e-15);
        // Earth's droplets: ~83 marine, ~198 continental cm⁻³ in the global mean.
        assert!((clouds.continental.droplets - 198.0).abs() < 1.0);
    }

    /// Without life the clouds are darker, more over the ocean than over bright land, and by a
    /// few W/m² (the literature's few to ~10 W/m² for the Archean).
    #[test]
    fn a_barren_planet_has_darker_clouds() {
        let params = barren();
        let clouds = CloudBrightening::new(&params, Sources::default());
        let (marine, ocean) = clouds.ocean(0.3);
        let (_, land) = clouds.land();
        assert!(ocean < -0.005 && ocean > -0.02, "{ocean}");
        assert!(land < 0.0 && land > ocean, "{land} vs {ocean}");
        assert!(marine.cloud_albedo < 0.51);
        // More volcanic sulfate brightens them again; the response saturates.
        let volcanic = |volcanic: f64| {
            CloudBrightening::new(
                &params,
                Sources {
                    volcanic,
                    ..Sources::default()
                },
            )
            .ocean(0.3)
            .1
        };
        assert!(volcanic(2.0) > volcanic(1.0) && volcanic(1.0) > volcanic(0.0));
        assert!(volcanic(4.0) - volcanic(3.0) < volcanic(1.0) - volcanic(0.0));
        // Turned off, nothing changes.
        let off = AerosolParams {
            enabled: false,
            ..params
        };
        let clouds = CloudBrightening::new(&off, Sources::default());
        assert_eq!(clouds.ocean(0.3).1, 0.0);
        assert_eq!(clouds.land().1, 0.0);
    }

    /// The sea salt follows the wind: more in the westerlies than in the subtropical calms,
    /// and its profile has a global mean of 1.
    #[test]
    fn sea_salt_follows_the_wind() {
        assert!(salt_emission(50.0) > 3.0 * salt_emission(30.0));
        let clouds = CloudBrightening::new(&AerosolParams::default(), Sources::default());
        let mean = (0..1000)
            .map(|bin| {
                let x: f64 = -1.0 + (bin as f64 + 0.5) / 500.0;
                salt_emission(x.asin().to_degrees()) / clouds.salt_mean
            })
            .sum::<f64>()
            / 1000.0;
        assert!((mean - 1.0).abs() < 1e-3, "{mean}");
    }

    /// Dust comes from dry, bare, ice-free land; without plants also from the steppe and drier
    /// grassland.
    #[test]
    fn dust_comes_from_dry_bare_land() {
        let modern = AerosolParams::default();
        let barren = barren();
        // 20 °C: the arid threshold is 540 mm.
        let source = |params: &AerosolParams, precipitation: f64, ice: f64| {
            dust_source(params, std::iter::once((20.0, precipitation, ice)), 1)
        };
        assert_eq!(source(&modern, 0.1, 0.0), 1.0);
        assert_eq!(source(&modern, 0.1, 1.0), 0.0);
        assert_eq!(source(&modern, 0.6, 0.0), 0.0);
        assert!(source(&barren, 0.6, 0.0) > 0.5);
        assert_eq!(source(&barren, 1.2, 0.0), 0.0);
        // Frozen ground does not emit.
        assert_eq!(
            dust_source(&barren, std::iter::once((-10.0, 0.0, 0.0)), 1),
            0.0
        );
        // The sources follow the planet only if they are dynamic, the volcanic one smoothly.
        // Earth's volcanic sulfate until the reference is set; the first degassing is taken
        // as it is, later ones smoothly.
        let earth = EARTH_VOLCANIC_DEGASSING;
        let sources =
            Sources::default().advance(&modern, Some(earth), Some(EARTH_DUST_SOURCE * 3.0), 1.0);
        assert_eq!(sources.volcanic, 1.0);
        assert!((sources.dust - 3.0).abs() < 1e-12);
        let calibrated = sources.calibrate(&modern, earth);
        assert_eq!(calibrated.reference, earth);
        let later = calibrated.advance(&modern, Some(2.0 * earth), None, 20.0);
        assert!((later.volcanic - (2.0 - math::exp(-1.0))).abs() < 1e-12);
        assert_eq!(later.dust, sources.dust);
        // A fixed reference applies from the start.
        let fixed = AerosolParams {
            volcanic_reference: 0.5 * earth,
            ..modern.clone()
        };
        let sources = Sources::default().advance(&fixed, Some(earth), None, 1.0);
        assert_eq!(sources.volcanic, 2.0);
        assert_eq!(sources.calibrate(&fixed, earth), sources);
        let earth_only = AerosolParams {
            dynamic_sources: false,
            ..modern
        };
        let sources = Sources::default().advance(&earth_only, Some(0.0), Some(0.0), 1.0);
        assert_eq!((sources.volcanic, sources.dust), (1.0, 1.0));
    }
}
