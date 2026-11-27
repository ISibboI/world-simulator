//! Atmospheric escape (#210): the loss of hydrogen, and with it of water, to space, as a global
//! budget like the carbon cycle ([`crate::carbon`]).
//!
//! Sunlight splits the water vapour of the upper atmosphere; its hydrogen escapes and the oxygen
//! stays behind. How much water gets up there is set by the **cold trap** of the tropical
//! tropopause, and how fast its hydrogen can leave by two limits:
//!
//! - **Cold trap** ([`stratospheric_water`]): the saturated (moist) adiabat of the tropical
//!   surface is followed up to the cold point ([`EscapeParams::cold_point_temperature`]); the
//!   vapour's share there is the water mixing ratio of the stratosphere (Kasting, 1988). A
//!   temperate planet keeps a few ppm; above ~70 °C at the surface the adiabat reaches the cold
//!   point only at very low pressure, the stratosphere becomes wet and the escape fast: the
//!   **moist greenhouse**.
//! - **Diffusion limit** ([`diffusion_limited_flux`]): hydrogen crosses the homopause at most at
//!   `(b/H_a) f/(1 + f)` H₂ per m² and second (Hunten, 1973; Walker, 1977), with `b/H_a` ≈
//!   2.5e13 cm⁻² s⁻¹.
//! - **Energy limit** ([`energy_limited_flux`]): lifting the gas out of the planet's gravity takes
//!   the absorbed XUV energy, `ε F_XUV / (4 g R m_H)` H atoms per m² and second (Watson et al.,
//!   1981), with the sun's XUV flux of its age ([`xuv_flux`]; Ribas et al., 2005), 13 times
//!   today's at epoch 0.
//!
//! The escape is the smaller of the two. With a cold trap it is diffusion-limited by orders of
//! magnitude even under the young sun's XUV, and removes ~1 m of sea level per Gyr; in a moist
//! greenhouse the XUV limits it, to about an ocean in a few Gyr around a Sun-like star.
//!
//! - **Water**: the water of the escaped hydrogen is taken from the ocean (the planet's water,
//!   [`crate::tectonics::Simulation::remove_water`]), so the sea level falls, and the carbon
//!   cycle sees the smaller ocean in its next step.
//! - **Oxygen**: one O₂ per four H stays (no oxygen is dragged along). It is a source of abiotic
//!   O₂ (#201); a placeholder sink of constant capacity ([`EscapeParams::oxygen_sink`]: reduced
//!   volcanic gases, oxidation of the crust) removes it. What is left adds to the air.
//! - **Not modelled**: ion pickup and sputtering by the solar wind, impact erosion and Jeans
//!   escape of heavy gases (all minor for an Earth-sized planet with a magnetic field after the
//!   heavy bombardment), hydrogen from other gases (H₂, CH₄; #202), and the runaway greenhouse.
//!
//! See `notes/issue-210/plan.md` for the values and their sources.

use tracing::warn;

use crate::carbon::SEA_WATER_DENSITY;
use crate::math;
use crate::sun::SOLAR_AGE;

/// Surface gravity in m/s² (as [`crate::carbon`]).
const GRAVITY: f64 = 9.81;
/// Avogadro's number per mol.
const AVOGADRO: f64 = 6.022_140_76e23;
/// Mass of a hydrogen atom in kg.
const HYDROGEN_MASS: f64 = 1.6735e-27;
/// Molar masses in kg/mol.
const WATER_MOLAR_MASS: f64 = 0.018_015;
const OXYGEN_MOLAR_MASS: f64 = 0.032;
/// 0 °C in K.
const ZERO_CELSIUS: f64 = 273.15;
/// Triple point of water: temperature (K) and pressure (Pa).
const TRIPLE_POINT_TEMPERATURE: f64 = 273.16;
const TRIPLE_POINT_PRESSURE: f64 = 611.657;
/// Specific gas constants (J/(kg K)) of water vapour and of the dry air, the specific heats
/// (J/(kg K)) at constant pressure of both, and the latent heats (J/kg) of evaporation and of
/// sublimation.
const VAPOUR_GAS_CONSTANT: f64 = 461.5;
const VAPOUR_HEAT_CAPACITY: f64 = 1850.0;
/// Molar gas constant in J/(mol K).
const GAS_CONSTANT: f64 = 8.314;
/// Molar mass (kg/mol) and specific heat (J/(kg K)) of the background air (N₂ and argon, or
/// today's air), of CO₂ and of O₂.
const AIR_MOLAR_MASS: f64 = 0.029;
const AIR_HEAT_CAPACITY: f64 = 1004.0;
const CO2_MOLAR_MASS: f64 = 0.044;
const CO2_HEAT_CAPACITY: f64 = 844.0;
const OXYGEN_HEAT_CAPACITY: f64 = 918.0;
const EVAPORATION_HEAT: f64 = 2.501e6;
/// Decrease of the latent heat of evaporation per kelvin (J/(kg K)): `c_pv − c_w`.
const EVAPORATION_HEAT_SLOPE: f64 = 2370.0;
const SUBLIMATION_HEAT: f64 = 2.834e6;
/// The hottest surface (K) the cold trap is computed for: the adiabat with a constant latent
/// heat stops being meaningful towards the critical point (647 K); the model's oceans stay far
/// below.
const MAX_SURFACE_TEMPERATURE: f64 = 450.0;
/// Steps of the integration of the moist adiabat (RK2 in ln T).
const ADIABAT_STEPS: usize = 200;
/// Spacing (K) of the table of the cold trap over the surface temperature.
const TABLE_SPACING: f64 = 1.0;

/// The non-condensing air: its pressure (Pa), specific gas constant and specific heat
/// (J/(kg K)).
#[derive(Clone, Copy, Debug)]
pub struct Air {
    pub pressure: f64,
    pub gas_constant: f64,
    pub heat_capacity: f64,
}

impl Air {
    /// Air of the background gas, its partial pressure `background`, with `co2` and `oxygen` Pa
    /// of CO₂ and O₂: molar mass and specific heat weighted by the mix.
    pub fn mix(background: f64, co2: f64, oxygen: f64) -> Self {
        let parts = [
            (background, AIR_MOLAR_MASS, AIR_HEAT_CAPACITY),
            (co2, CO2_MOLAR_MASS, CO2_HEAT_CAPACITY),
            (oxygen, OXYGEN_MOLAR_MASS, OXYGEN_HEAT_CAPACITY),
        ];
        let pressure: f64 = parts.iter().map(|part| part.0).sum();
        // Mole fractions are pressure fractions; mass fractions weight them by molar mass.
        let molar_mass = parts.iter().map(|p| p.0 * p.1).sum::<f64>() / pressure;
        let heat_capacity =
            parts.iter().map(|p| p.0 * p.1 * p.2).sum::<f64>() / (pressure * molar_mass);
        Self {
            pressure,
            gas_constant: GAS_CONSTANT / molar_mass,
            heat_capacity,
        }
    }

    /// Air of `pressure` Pa of the background gas alone.
    #[cfg(test)]
    pub fn background(pressure: f64) -> Self {
        Self::mix(pressure, 0.0, 0.0)
    }
}

/// Parameters of the escape.
#[derive(Clone, Debug)]
pub struct EscapeParams {
    /// Whether the planet loses water (`--no-escape` turns it off; then nothing is computed and
    /// the run is that of the model without escape).
    pub enabled: bool,
    /// Temperature (K) of the cold point of the tropical tropopause, where the moist adiabat of
    /// the surface meets the stratosphere: today's ~190 K, kept for all climates (between
    /// Kasting's 200 K and the colder stratospheres of non-grey models).
    pub cold_point_temperature: f64,
    /// Partial pressure (Pa) of the non-condensing air besides the CO₂ of the carbon cycle and
    /// the O₂ of the escape: N₂ and argon (and today's O₂ for a living planet).
    pub background_pressure: f64,
    /// The diffusion limit `b/H_a` in H₂ per m² and second at a mixing ratio of 1 (2.5e13
    /// cm⁻² s⁻¹; Walker, 1977; Catling & Kasting, 2017).
    pub diffusion_limit: f64,
    /// XUV flux (1–1180 Å) of today's sun at the planet in W/m² (Ribas et al., 2005: 4.6 erg
    /// cm⁻² s⁻¹), its power law in the sun's age and the age (Gyr) before which it is saturated.
    pub xuv_flux: f64,
    pub xuv_exponent: f64,
    pub xuv_saturation_age: f64,
    /// Share of the absorbed XUV that lifts gas out of the gravity well (0.1–0.3).
    pub heating_efficiency: f64,
    /// Latitude (degrees) within which the surface sets the cold trap.
    pub tropics_latitude: f64,
    /// Capacity of the sinks of O₂ (mol/yr): reduced volcanic gases, oxidation of new sea floor
    /// and of the Fe²⁺ of weathered rock, each of the order of 1 Tmol/yr on a tectonically
    /// active planet (Catling & Kasting, 2017); a placeholder until the oxygen has a budget of
    /// its own (#201).
    pub oxygen_sink: f64,
    /// The escape stops when the ocean is down to this share of the planet's water at the start
    /// (the climate model needs an ocean).
    pub min_ocean_fraction: f64,
}

impl Default for EscapeParams {
    fn default() -> Self {
        Self {
            enabled: true,
            cold_point_temperature: 190.0,
            background_pressure: 1e5,
            diffusion_limit: 2.5e17,
            xuv_flux: 4.6e-3,
            xuv_exponent: -1.23,
            xuv_saturation_age: 0.1,
            heating_efficiency: 0.15,
            tropics_latitude: 20.0,
            oxygen_sink: 3e12,
            min_ocean_fraction: 0.01,
        }
    }
}

impl EscapeParams {
    /// Sets a parameter by its field name (`enabled` as 0 or 1).
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        if name == "enabled" {
            self.enabled = value != 0.0;
            return Ok(());
        }
        let field = match name {
            "cold_point_temperature" => &mut self.cold_point_temperature,
            "background_pressure" => &mut self.background_pressure,
            "diffusion_limit" => &mut self.diffusion_limit,
            "xuv_flux" => &mut self.xuv_flux,
            "xuv_exponent" => &mut self.xuv_exponent,
            "xuv_saturation_age" => &mut self.xuv_saturation_age,
            "heating_efficiency" => &mut self.heating_efficiency,
            "tropics_latitude" => &mut self.tropics_latitude,
            "oxygen_sink" => &mut self.oxygen_sink,
            "min_ocean_fraction" => &mut self.min_ocean_fraction,
            _ => anyhow::bail!("unknown escape parameter {name}"),
        };
        *field = value;
        Ok(())
    }
}

/// Saturation vapour pressure (Pa) at `temperature` (K): Clausius–Clapeyron over water above
/// the triple point with a latent heat falling linearly with the temperature (Kirchhoff), which
/// keeps it within ~2% up to 100 °C, and over ice below with a constant latent heat.
pub fn saturation_pressure(temperature: f64) -> f64 {
    let t0 = TRIPLE_POINT_TEMPERATURE;
    let exponent = if temperature >= t0 {
        let a = EVAPORATION_HEAT_SLOPE;
        (EVAPORATION_HEAT + a * t0) / VAPOUR_GAS_CONSTANT * (1.0 / t0 - 1.0 / temperature)
            - a / VAPOUR_GAS_CONSTANT * math::ln(temperature / t0)
    } else {
        SUBLIMATION_HEAT / VAPOUR_GAS_CONSTANT * (1.0 / t0 - 1.0 / temperature)
    };
    TRIPLE_POINT_PRESSURE * math::exp(exponent)
}

/// Latent heat (J/kg) of evaporation above the triple point, of sublimation below.
fn latent_heat(temperature: f64) -> f64 {
    if temperature >= TRIPLE_POINT_TEMPERATURE {
        EVAPORATION_HEAT - EVAPORATION_HEAT_SLOPE * (temperature - TRIPLE_POINT_TEMPERATURE)
    } else {
        SUBLIMATION_HEAT
    }
}

/// The cold trap: the water vapour mixing ratio (by number) above the cold point at `cold_point`
/// K, of a saturated atmosphere over a surface at `surface` K with the non-condensing `air`, and
/// the non-condensing gas's pressure (Pa) at the cold point.
///
/// Follows the moist adiabat of a non-dilute condensable (Pierrehumbert, 2010, eq. 2.33) from
/// the surface up to the cold point:
/// `d ln p_a / d ln T = (c_pa/R_a) [1 + (c_pv/c_pa + (L/(R_v T) − 1) L/(c_pa T)) r] /
/// [1 + L r/(R_a T)]` with the saturation mass ratio `r = (R_a/R_v) e_s/p_a`.
pub fn stratospheric_water(surface: f64, air: Air, cold_point: f64) -> (f64, f64) {
    let surface = surface.min(MAX_SURFACE_TEMPERATURE);
    let top = cold_point.min(surface);
    let slope = |temperature: f64, ln_air: f64| {
        let ratio = air.gas_constant / VAPOUR_GAS_CONSTANT * saturation_pressure(temperature)
            / math::exp(ln_air);
        let heat = latent_heat(temperature);
        let numerator = 1.0
            + (VAPOUR_HEAT_CAPACITY / air.heat_capacity
                + (heat / (VAPOUR_GAS_CONSTANT * temperature) - 1.0) * heat
                    / (air.heat_capacity * temperature))
                * ratio;
        let denominator = 1.0 + heat * ratio / (air.gas_constant * temperature);
        air.heat_capacity / air.gas_constant * numerator / denominator
    };
    let mut ln_air = math::ln(air.pressure);
    let mut ln_temperature = math::ln(surface);
    let step = (math::ln(top) - ln_temperature) / ADIABAT_STEPS as f64;
    for _ in 0..ADIABAT_STEPS {
        let k1 = slope(math::exp(ln_temperature), ln_air);
        let k2 = slope(
            math::exp(ln_temperature + 0.5 * step),
            ln_air + 0.5 * step * k1,
        );
        ln_air += step * k2;
        ln_temperature += step;
    }
    let air_at_top = math::exp(ln_air);
    let vapour = saturation_pressure(top);
    (vapour / (air_at_top + vapour), air_at_top)
}

/// The cold trap of [`stratospheric_water`] over the surface temperature, tabulated for one
/// air and cold point, to average it over many cells: the mixing ratio is very convex in the
/// surface temperature, so the warm cells count more than their mean.
pub struct ColdTrapTable {
    /// The coldest surface (K) of the table: the cold point.
    start: f64,
    /// ln of the mixing ratio and the pressure (Pa) at the cold point at `start + i` K.
    ln_water: Vec<f64>,
    pressure: Vec<f64>,
}

impl ColdTrapTable {
    /// The table for surfaces from the cold point up to `hottest` K.
    pub fn new(air: Air, cold_point: f64, hottest: f64) -> Self {
        let hottest = hottest.clamp(cold_point, MAX_SURFACE_TEMPERATURE);
        let count = ((hottest - cold_point) / TABLE_SPACING).ceil() as usize + 2;
        let (ln_water, pressure) = (0..count)
            .map(|i| {
                let (water, pressure) =
                    stratospheric_water(cold_point + i as f64 * TABLE_SPACING, air, cold_point);
                (math::ln(water), pressure)
            })
            .unzip();
        Self {
            start: cold_point,
            ln_water,
            pressure,
        }
    }

    /// The mixing ratio and the cold point's pressure (Pa) over a surface at `surface` K,
    /// interpolated linearly in ln f and in the pressure.
    pub fn at(&self, surface: f64) -> (f64, f64) {
        let last = self.ln_water.len() - 1;
        let x = ((surface - self.start) / TABLE_SPACING).clamp(0.0, last as f64);
        let i = (x as usize).min(last.saturating_sub(1));
        let t = x - i as f64;
        let j = (i + 1).min(last);
        (
            math::exp(self.ln_water[i] + t * (self.ln_water[j] - self.ln_water[i])),
            self.pressure[i] + t * (self.pressure[j] - self.pressure[i]),
        )
    }
}

/// The XUV flux (W/m²) at the planet at the sun's age `age` (Gyr): Ribas et al. (2005),
/// `F ∝ τ^−1.23`, scaled to [`EscapeParams::xuv_flux`] today and constant before the
/// saturation age.
pub fn xuv_flux(params: &EscapeParams, age: f64) -> f64 {
    let age = age.max(params.xuv_saturation_age);
    params.xuv_flux * math::powf(age / SOLAR_AGE, params.xuv_exponent)
}

/// The diffusion-limited escape in H atoms per m² and second at the stratospheric water mixing
/// ratio `water` (two H per H₂O).
pub fn diffusion_limited_flux(params: &EscapeParams, water: f64) -> f64 {
    2.0 * params.diffusion_limit * water / (1.0 + water)
}

/// The energy-limited escape in H atoms per m² and second at the XUV flux `xuv` (W/m²) on a
/// planet of `radius` m: `ε F / (4 g R m_H)`.
pub fn energy_limited_flux(params: &EscapeParams, xuv: f64, radius: f64) -> f64 {
    params.heating_efficiency * xuv / (4.0 * GRAVITY * radius * HYDROGEN_MASS)
}

/// What one step of the escape needs of the planet.
pub struct StepInput<'a> {
    /// Length of the step in years.
    pub years: f64,
    /// Annual mean surface temperatures (°C) of the cells that set the cold trap: the tropical
    /// ocean (equal areas). None: no water reaches the stratosphere.
    pub tropical_temperatures: &'a [f64],
    /// Partial pressure of CO₂ in bar.
    pub co2_pressure: f64,
    /// Age of the sun in Gyr.
    pub sun_age: f64,
    /// Radius of the planet in m.
    pub planet_radius: f64,
    /// Volume of the ocean in m³.
    pub ocean_volume: f64,
}

/// The state of the escape after a step, for the log and the statistics.
#[derive(Clone, Copy, Debug, Default)]
pub struct EscapeState {
    /// Mean tropical surface temperature (°C) of the cold trap, the stratospheric water mixing
    /// ratio averaged over the cells, and the mean pressure (Pa) of the non-condensing air at the
    /// cold point.
    pub tropical_temperature: f64,
    pub stratospheric_water: f64,
    pub cold_point_pressure: f64,
    /// XUV flux (W/m²) and the diffusion and energy limits and the escape in H atoms per m² and
    /// second, averaged over the planet's surface.
    pub xuv: f64,
    pub diffusion_limit: f64,
    pub energy_limit: f64,
    pub hydrogen_flux: f64,
    /// Water lost in the step (kg/yr) and so far (kg).
    pub water_loss_rate: f64,
    pub water_lost: f64,
    /// O₂ left behind so far, taken up by the sinks so far, and in the air (mol), and the
    /// partial pressure of that (bar).
    pub oxygen_produced: f64,
    pub oxygen_consumed: f64,
    pub oxygen: f64,
    pub oxygen_pressure: f64,
    /// Whether the ocean has reached [`EscapeParams::min_ocean_fraction`].
    pub at_floor: bool,
}

impl EscapeState {
    /// Whether the energy limit sets the escape.
    pub fn energy_limited(&self) -> bool {
        self.energy_limit < self.diffusion_limit
    }

    /// One line for the log.
    pub fn summary(&self) -> String {
        format!(
            "escape: tropics {:.1} °C, stratospheric H₂O {:.2e} (cold point at {:.3e} Pa) | H {:.2e} cm⁻² s⁻¹ ({}; \
             diffusion limit {:.2e}, energy limit {:.2e} at XUV {:.2} mW/m²) | water {:.3e} \
             kg/yr, lost so far {:.3e} kg | O₂ produced {:.3e} mol, sinks {:.3e}, in the air \
             {:.3e} mol ({:.2e} bar){}",
            self.tropical_temperature,
            self.stratospheric_water,
            self.cold_point_pressure,
            self.hydrogen_flux * 1e-4,
            if self.energy_limited() {
                "energy-limited"
            } else {
                "diffusion-limited"
            },
            self.diffusion_limit * 1e-4,
            self.energy_limit * 1e-4,
            self.xuv * 1e3,
            self.water_loss_rate,
            self.water_lost,
            self.oxygen_produced,
            self.oxygen_consumed,
            self.oxygen,
            self.oxygen_pressure,
            if self.at_floor {
                " | ocean at its floor"
            } else {
                ""
            },
        )
    }
}

/// The escape (see the module documentation).
#[derive(Clone, Debug)]
pub struct Escape {
    params: EscapeParams,
    /// The ocean's volume (m³) below which nothing more is lost.
    floor: f64,
    state: EscapeState,
}

impl Escape {
    /// The escape of a planet whose water is `water_volume` m³ at the start.
    pub fn new(params: EscapeParams, water_volume: f64) -> Self {
        let floor = params.min_ocean_fraction.max(0.0) * water_volume;
        Self {
            params,
            floor,
            state: EscapeState::default(),
        }
    }

    pub fn params(&self) -> &EscapeParams {
        &self.params
    }

    pub fn state(&self) -> &EscapeState {
        &self.state
    }

    /// Takes one step and returns the volume of water (m³) to take from the ocean.
    pub fn step(&mut self, input: &StepInput) -> f64 {
        let params = &self.params;
        let area = 4.0 * std::f64::consts::PI * input.planet_radius * input.planet_radius;
        let air = Air::mix(
            params.background_pressure,
            input.co2_pressure * 1e5,
            self.state.oxygen_pressure * 1e5,
        );
        let temperatures = input.tropical_temperatures;
        let (mut water, mut cold_point_pressure, mut tropical_temperature) = (0.0, 0.0, f64::NAN);
        if !temperatures.is_empty() {
            let hottest = temperatures.iter().fold(f64::MIN, |a, &b| a.max(b)) + ZERO_CELSIUS;
            let table = ColdTrapTable::new(air, params.cold_point_temperature, hottest);
            let count = temperatures.len() as f64;
            tropical_temperature = temperatures.iter().sum::<f64>() / count;
            for &temperature in temperatures {
                let (f, pressure) = table.at(temperature + ZERO_CELSIUS);
                water += f / count;
                cold_point_pressure += pressure / count;
            }
        }
        let xuv = xuv_flux(params, input.sun_age);
        let diffusion_limit = diffusion_limited_flux(params, water);
        let energy_limit = energy_limited_flux(params, xuv, input.planet_radius);
        let mut hydrogen_flux = diffusion_limit.min(energy_limit);
        // Water lost (kg): two H per H₂O.
        let seconds = input.years * 365.25 * 86400.0;
        let mut water_mass = hydrogen_flux * area * seconds / AVOGADRO / 2.0 * WATER_MOLAR_MASS;
        let available = ((input.ocean_volume - self.floor) * SEA_WATER_DENSITY).max(0.0);
        let at_floor = water_mass > 0.0 && water_mass >= available;
        if at_floor {
            if !self.state.at_floor {
                warn!(
                    "the ocean is down to {:.0}% of the planet's water at the start; it loses \
                     no more by escape (the climate model needs an ocean)",
                    100.0 * params.min_ocean_fraction
                );
            }
            hydrogen_flux *= if water_mass > 0.0 {
                available / water_mass
            } else {
                0.0
            };
            water_mass = available;
        }
        // One O₂ per two H₂O.
        let produced = water_mass / WATER_MOLAR_MASS / 2.0;
        let oxygen = self.state.oxygen + produced;
        let consumed = oxygen.min(params.oxygen_sink.max(0.0) * input.years);
        let oxygen = oxygen - consumed;
        self.state = EscapeState {
            tropical_temperature,
            stratospheric_water: water,
            cold_point_pressure,
            xuv,
            diffusion_limit,
            energy_limit,
            hydrogen_flux,
            water_loss_rate: water_mass / input.years,
            water_lost: self.state.water_lost + water_mass,
            oxygen_produced: self.state.oxygen_produced + produced,
            oxygen_consumed: self.state.oxygen_consumed + consumed,
            oxygen,
            oxygen_pressure: oxygen * OXYGEN_MOLAR_MASS * GRAVITY / area / 1e5,
            at_floor,
        };
        water_mass / SEA_WATER_DENSITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RADIUS: f64 = 6.371e6;
    const OCEAN: f64 = 1.335e18;

    #[test]
    fn saturation_pressure_values() {
        // 3169 Pa at 25 °C, 101 kPa at 100 °C (the linear latent heat gives ~2% less),
        // 0.03 Pa over ice at 190 K.
        assert!((saturation_pressure(298.15) / 3169.0 - 1.0).abs() < 0.05);
        assert!((saturation_pressure(373.15) / 101_325.0 - 1.0).abs() < 0.1);
        assert!((saturation_pressure(190.0) / 0.032 - 1.0).abs() < 0.15);
    }

    /// Today's tropics give a few ppm; the mixing ratio rises steeply with the surface
    /// temperature and passes the moist-greenhouse limit of 3e-3 between 65 and 80 °C.
    #[test]
    fn cold_trap() {
        let air = Air::background(1e5);
        let at = |celsius: f64| stratospheric_water(celsius + ZERO_CELSIUS, air, 190.0).0;
        let today = at(27.0);
        assert!((2e-6..6e-6).contains(&today), "{today}");
        let mut previous = 0.0;
        for celsius in (0..=120).step_by(5) {
            let f = at(celsius as f64);
            assert!(f > previous && f < 1.0, "{celsius} °C: {f}");
            previous = f;
        }
        assert!(at(65.0) < 3e-3 && at(80.0) > 3e-3);
        // More air dilutes the vapour; CO₂ a little less than the same pressure of N₂ (its larger
        // c_p/R makes the adiabat reach the cold point higher up).
        let thick = stratospheric_water(340.0, Air::background(1.5e5), 190.0).0;
        let co2 = stratospheric_water(340.0, Air::mix(1e5, 0.5e5, 0.0), 190.0).0;
        let thin = at(340.0 - ZERO_CELSIUS);
        assert!(thick < co2 && co2 < 0.2 * thin, "{thick} {co2} {thin}");
        // The table follows the adiabat.
        let table = ColdTrapTable::new(air, 190.0, 400.0);
        for celsius in [-20.0, 0.0, 27.3, 61.7, 99.5] {
            let exact = at(celsius);
            let (f, _) = table.at(celsius + ZERO_CELSIUS);
            assert!((f / exact - 1.0).abs() < 0.02, "{celsius}: {f} {exact}");
        }
    }

    /// Today's escape (~3e8 H cm⁻² s⁻¹ with methane; the water alone gives about half), the
    /// energy limit of today's XUV (~1.6e11), the young sun's XUV 13 times today's at epoch 0.
    #[test]
    fn todays_limits() {
        let params = EscapeParams::default();
        let diffusion = diffusion_limited_flux(&params, 7e-6) * 1e-4;
        assert!((2e8..5e8).contains(&diffusion), "{diffusion}");
        let energy = energy_limited_flux(&params, xuv_flux(&params, SOLAR_AGE), RADIUS) * 1e-4;
        assert!((1e11..2.5e11).contains(&energy), "{energy}");
        let young = xuv_flux(&params, SOLAR_AGE - 4.0) / params.xuv_flux;
        assert!((11.0..15.0).contains(&young), "{young}");
    }

    fn input(celsius: &[f64], ocean_volume: f64) -> StepInput<'_> {
        StepInput {
            years: 1e7,
            tropical_temperatures: celsius,
            co2_pressure: 0.0,
            sun_age: SOLAR_AGE,
            planet_radius: RADIUS,
            ocean_volume,
        }
    }

    /// A temperate planet loses ~1 m of ocean per Gyr; a moist greenhouse is energy-limited and
    /// loses an ocean in Gyr. Water, hydrogen and oxygen balance.
    #[test]
    fn water_loss_and_oxygen_balance() {
        let mut escape = Escape::new(EscapeParams::default(), OCEAN);
        let volume = escape.step(&input(&[27.0], OCEAN));
        let ocean_area = 3.6e14;
        let per_gyr = volume * 1e2 / ocean_area;
        assert!((0.1..5.0).contains(&per_gyr), "{per_gyr} m/Gyr");
        let state = *escape.state();
        assert!(!state.energy_limited());
        // Far below the sinks: no O₂ in the air.
        assert_eq!(state.oxygen, 0.0);
        assert_eq!(state.oxygen_consumed, state.oxygen_produced);

        // Sinks that take all the O₂ (it would otherwise dry the stratosphere, see below).
        let params = EscapeParams {
            oxygen_sink: 1e20,
            ..EscapeParams::default()
        };
        let mut escape = Escape::new(params, OCEAN);
        let mut ocean = OCEAN;
        let mut years = 0.0;
        while ocean > 0.5 * OCEAN && years < 1e11 {
            ocean -= escape.step(&input(&[100.0], ocean));
            years += 1e7;
        }
        let state = *escape.state();
        assert!(state.energy_limited());
        // Half an ocean in ~2 Gyr at today's XUV.
        assert!((1e9..4e9).contains(&years), "{years}");
        let lost = (OCEAN - ocean) * SEA_WATER_DENSITY;
        assert!((state.water_lost / lost - 1.0).abs() < 1e-9);
        // O₂ balance, one per two H₂O.
        assert!(
            (state.oxygen_produced - state.water_lost / WATER_MOLAR_MASS / 2.0).abs()
                < 1e-9 * state.oxygen_produced
        );
        assert!(
            (state.oxygen + state.oxygen_consumed - state.oxygen_produced).abs()
                < 1e-9 * state.oxygen_produced
        );
        assert_eq!(state.oxygen, 0.0);

        // With the placeholder sink the O₂ builds up (bars), dilutes the vapour and ends the
        // fast loss: the escape becomes diffusion-limited.
        let mut escape = Escape::new(EscapeParams::default(), OCEAN);
        let mut ocean = OCEAN;
        let first = escape.step(&input(&[100.0], ocean));
        assert!(escape.state().energy_limited());
        for _ in 0..300 {
            ocean -= escape.step(&input(&[100.0], ocean));
        }
        let state = *escape.state();
        assert!(state.oxygen_pressure > 1.0, "{}", state.oxygen_pressure);
        assert!(!state.energy_limited());
        assert!(escape.step(&input(&[100.0], ocean)) < 0.5 * first);
        assert!(
            (state.oxygen + state.oxygen_consumed - state.oxygen_produced).abs()
                < 1e-9 * state.oxygen_produced
        );
    }

    /// The ocean never falls below the floor.
    #[test]
    fn ocean_floor() {
        let params = EscapeParams {
            min_ocean_fraction: 0.5,
            heating_efficiency: 1e3,
            oxygen_sink: 1e20,
            ..EscapeParams::default()
        };
        let mut escape = Escape::new(params, OCEAN);
        let mut ocean = OCEAN;
        for _ in 0..100 {
            ocean -= escape.step(&input(&[110.0], ocean));
            assert!(ocean >= 0.5 * OCEAN * (1.0 - 1e-12));
        }
        assert!(escape.state().at_floor);
        assert_eq!(escape.state().water_loss_rate, 0.0);
    }

    /// Averaging over cells counts the warm ones more than the mean temperature would; no
    /// cells, no water.
    #[test]
    fn average_over_cells() {
        let mut escape = Escape::new(EscapeParams::default(), OCEAN);
        escape.step(&input(&[63.0, 77.0], OCEAN));
        let spread = escape.state().stratospheric_water;
        escape.step(&input(&[70.0], OCEAN));
        assert!(spread > 1.5 * escape.state().stratospheric_water);
        assert_eq!(escape.step(&input(&[], OCEAN)), 0.0);
    }
}
