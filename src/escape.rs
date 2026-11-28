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
//!   O₂, and this module keeps the abiotic O₂ budget of #201 ([`OxygenSinks`]; #212 decision 1):
//!   reduced gases of arc and plume volcanoes (in proportion to their mantle CO₂, Holland,
//!   2002), the oxidation of new oceanic crust by hydrothermal alteration (per kg of new crust,
//!   Lécuyer & Ricard, 1999) and the oxidation of the Fe²⁺ and sulfide that erosion exposes on
//!   land, which needs O₂ (`1 − exp(−√(p/p_ox))`). The first two are capacities set by the
//!   tectonics; what they cannot take, and the weathering at the new O₂ (an implicit step), adds
//!   to the air.
//! - **Not modelled**: ion pickup and sputtering by the solar wind, impact erosion and Jeans
//!   escape of heavy gases (all minor for an Earth-sized planet with a magnetic field after the
//!   heavy bombardment) and the runaway greenhouse.
//! - **Reduced gases** (#202, [`crate::reduced`]): volcanic H₂ and CO and the H₂ and CH₄ of
//!   serpentinization meet the O₂ together with the other sinks; what the O₂ leaves stays in
//!   the air, and its hydrogen escapes with the water's (`f_T = f_H₂O + f_H₂ + 2 f_CH₄`).
//!
//! See `notes/issue-210/plan.md` for the values and their sources.

use tracing::warn;

use crate::carbon::{LandCell, SEA_WATER_DENSITY};
use crate::math;
use crate::reduced::{GasInput, GasState, Gases, ReducedParams, SPREADING_RATE_BINS};
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
    /// O₂ taken up by the reduced gases (H₂, CO, H₂S, SO₂) of arc and plume volcanoes, in mol
    /// O₂ per mol of their mantle CO₂ (Holland, 2002: somewhat below 0.2; Catling & Kasting,
    /// 2017). The ridges' reductants are part of [`Self::seafloor_oxidation`]. With the reduced
    /// gases (#202) only their sulfur part
    /// ([`crate::reduced::ReducedParams::volcanic_sulfur`]) stays here; the H₂ and CO become
    /// gases.
    pub volcanic_reductants: f64,
    /// O₂ taken up by the hydrothermal oxidation of new oceanic crust (Fe²⁺ → Fe³⁺ and sulfide
    /// of its upper ~0.6 km of basalt), mol O₂ per kg of new crust (Lécuyer & Ricard, 1999).
    /// The serpentinization of peridotite at slow ridges (#202) is a separate source of H₂;
    /// basalt-hosted alteration releases little H₂, so this stays a fixed capacity.
    pub seafloor_oxidation: f64,
    /// O₂ that the oxidative weathering of eroded felsic and basaltic bedrock takes up, mol O₂
    /// per kg: half its Fe²⁺ to Fe³⁺, all its sulfide to sulfate. Eroded sediment counts as
    /// oxidised already.
    pub felsic_oxygen_demand: f64,
    pub basalt_oxygen_demand: f64,
    /// O₂ partial pressure (bar) of the weathering's oxidation, `1 − exp(−√(p/p_ox))` of the
    /// exposed Fe²⁺ and sulfide (rate ∝ \[O₂\]^½, Williamson & Rimstidt, 1994): detrital pyrite
    /// survives below ~1e-5 PAL and is gone above ~1e-3 PAL.
    pub oxidation_pressure: f64,
    /// The escape stops when the ocean is down to this share of the planet's water at the start
    /// (the climate model needs an ocean).
    pub min_ocean_fraction: f64,
    /// The reduced gases (#202; `reduced.NAME`), which need the escape.
    pub gases: ReducedParams,
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
            volcanic_reductants: 0.15,
            seafloor_oxidation: 0.008,
            felsic_oxygen_demand: 0.089,
            basalt_oxygen_demand: 0.23,
            oxidation_pressure: 2e-5,
            min_ocean_fraction: 0.01,
            gases: ReducedParams::default(),
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
            "volcanic_reductants" => &mut self.volcanic_reductants,
            "seafloor_oxidation" => &mut self.seafloor_oxidation,
            "felsic_oxygen_demand" => &mut self.felsic_oxygen_demand,
            "basalt_oxygen_demand" => &mut self.basalt_oxygen_demand,
            "oxidation_pressure" => &mut self.oxidation_pressure,
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
    /// Mantle CO₂ of arc and plume volcanoes (mol C/yr) and new oceanic crust (kg/yr), from the
    /// carbon cycle.
    pub volcanic_degassing: f64,
    pub new_crust: f64,
    /// The new oceanic crust so far by spreading rate (kg; for the reduced gases).
    pub new_crust_by_rate: [f64; SPREADING_RATE_BINS],
    /// The land cells with their erosion in the step.
    pub land: &'a [LandCell],
}

/// The sinks of O₂ in mol O₂/yr (rates) or mol (totals).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OxygenSinks {
    /// Reduced gases of arc and plume volcanoes.
    pub volcanic: f64,
    /// Oxidation of new oceanic crust.
    pub seafloor: f64,
    /// Oxidative weathering on land.
    pub weathering: f64,
    /// The reduced gases in the air (#202: H₂ and CH₄, ½ and 2 O₂ each); 0 without them, when
    /// [`Self::volcanic`] holds all the volcanic reductants.
    pub gases: f64,
}

impl OxygenSinks {
    pub fn total(&self) -> f64 {
        self.volcanic + self.seafloor + self.weathering + self.gases
    }

    fn add(&self, other: &OxygenSinks) -> OxygenSinks {
        OxygenSinks {
            volcanic: self.volcanic + other.volcanic,
            seafloor: self.seafloor + other.seafloor,
            weathering: self.weathering + other.weathering,
            gases: self.gases + other.gases,
        }
    }
}

/// The O₂ (mol/yr) that the weathering of the land's eroded bedrock takes up when all of it is
/// oxidised.
pub fn weathering_oxygen_demand(params: &EscapeParams, land: &[LandCell]) -> f64 {
    land.iter()
        .filter(|cell| cell.ice < 1.0)
        .map(|cell| {
            let mafic = cell.mafic.clamp(0.0, 1.0);
            let demand = params.felsic_oxygen_demand
                + mafic * (params.basalt_oxygen_demand - params.felsic_oxygen_demand);
            cell.area * cell.bedrock_erosion.max(0.0) * cell.bedrock_density * demand
        })
        .sum()
}

/// Share of the exposed Fe²⁺ and sulfide that is oxidised at the O₂ partial pressure `pressure`
/// (bar): `1 − exp(−√(p/p_ox))`.
pub fn oxidised_share(params: &EscapeParams, pressure: f64) -> f64 {
    if pressure <= 0.0 || params.oxidation_pressure <= 0.0 {
        return if pressure > 0.0 { 1.0 } else { 0.0 };
    }
    -math::exp_m1(-(pressure / params.oxidation_pressure).sqrt())
}

/// The O₂ budget of a step of `years`: `available` mol (the air's and the step's source) meet
/// the fixed capacities (mol/yr) of the volcanic and seafloor sinks and the weathering's demand
/// `weathering` (mol/yr at full oxidation), which needs O₂ (`pressure_per_mol`: bar per mol).
/// Returns what each sink takes (mol) and the O₂ left; the sinks and the rest add up to
/// `available` (to rounding).
fn oxygen_step(
    params: &EscapeParams,
    available: f64,
    years: f64,
    capacity: OxygenSinks,
    pressure_per_mol: f64,
) -> (OxygenSinks, f64) {
    let fixed = years * (capacity.volcanic + capacity.seafloor + capacity.gases);
    if available <= fixed {
        // All goes to the fixed sinks, in proportion to their capacity (W(0) = 0).
        let share = if fixed > 0.0 { available / fixed } else { 0.0 };
        let volcanic = years * capacity.volcanic * share;
        let gases = years * capacity.gases * share;
        let taken = OxygenSinks {
            volcanic,
            seafloor: available - volcanic - gases,
            weathering: 0.0,
            gases,
        };
        return (taken, 0.0);
    }
    // Backward Euler: N + y (G + F + W(p(N))) = available, increasing in N.
    let weathering =
        |n: f64| years * capacity.weathering * oxidised_share(params, n * pressure_per_mol);
    let excess = |n: f64| n + fixed + weathering(n) - available;
    let (mut low, mut high) = (0.0, available - fixed);
    for _ in 0..200 {
        let middle = 0.5 * (low + high);
        if middle <= low || middle >= high {
            break;
        }
        if excess(middle) > 0.0 {
            high = middle;
        } else {
            low = middle;
        }
    }
    let taken = OxygenSinks {
        volcanic: years * capacity.volcanic,
        seafloor: years * capacity.seafloor,
        weathering: weathering(high).min(available - fixed),
        gases: years * capacity.gases,
    };
    let left = (available - taken.total()).max(0.0);
    (taken, left)
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
    pub oxygen_consumed: OxygenSinks,
    pub oxygen: f64,
    /// The sinks' capacity in the step (mol/yr; the weathering's at full oxidation) and what
    /// they took (mol/yr).
    pub sink_capacity: OxygenSinks,
    pub sink_rates: OxygenSinks,
    pub oxygen_pressure: f64,
    /// Whether the ocean has reached [`EscapeParams::min_ocean_fraction`].
    pub at_floor: bool,
    /// The reduced gases (#202), when they are simulated.
    pub gases: Option<GasState>,
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
             {:.3e} mol ({:.2e} bar) | O₂ sinks {:.3} Tmol/yr (volcanic {:.3} of {:.3}, \
             seafloor {:.3} of {:.3}, weathering {:.3} of {:.3}), so far volcanic {:.3e}, \
             seafloor {:.3e}, weathering {:.3e} mol{}",
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
            self.oxygen_consumed.total(),
            self.oxygen,
            self.oxygen_pressure,
            self.sink_rates.total() / 1e12,
            self.sink_rates.volcanic / 1e12,
            self.sink_capacity.volcanic / 1e12,
            self.sink_rates.seafloor / 1e12,
            self.sink_capacity.seafloor / 1e12,
            self.sink_rates.weathering / 1e12,
            self.sink_capacity.weathering / 1e12,
            self.oxygen_consumed.volcanic,
            self.oxygen_consumed.seafloor,
            self.oxygen_consumed.weathering,
            if self.at_floor {
                " | ocean at its floor"
            } else {
                ""
            },
        ) + &self.gases.map_or(String::new(), |gases| {
            format!(
                " | O₂ to the gases {:.4} of {:.4} Tmol/yr | {}",
                self.sink_rates.gases / 1e12,
                self.sink_capacity.gases / 1e12,
                gases.summary()
            )
        })
    }
}

/// The escape (see the module documentation).
#[derive(Clone, Debug)]
pub struct Escape {
    params: EscapeParams,
    /// The reduced gases (#202), when they are simulated.
    gases: Option<Gases>,
    /// The ocean's volume (m³) below which nothing more is lost.
    floor: f64,
    state: EscapeState,
}

impl Escape {
    /// The escape of a planet whose water is `water_volume` m³ at the start.
    pub fn new(params: EscapeParams, water_volume: f64) -> Self {
        let floor = params.min_ocean_fraction.max(0.0) * water_volume;
        let gases =
            (params.enabled && params.gases.enabled).then(|| Gases::new(params.gases.clone()));
        Self {
            params,
            gases,
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

    /// The reduced gases (#202), when they are simulated.
    pub fn gases(&self) -> Option<&Gases> {
        self.gases.as_ref()
    }

    /// Records the new crust per spreading rate at the start (see [`Gases::start`]).
    pub fn start(&mut self, new_crust_by_rate: [f64; SPREADING_RATE_BINS]) {
        if let Some(gases) = &mut self.gases {
            gases.start(new_crust_by_rate);
        }
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
        let mut diffusion_limit = diffusion_limited_flux(params, water);
        let energy_limit = energy_limited_flux(params, xuv, input.planet_radius);
        let mut hydrogen_flux = diffusion_limit.min(energy_limit);
        if let Some(gases) = &self.gases {
            // The gases of the last step share the diffusion limit (`f_T`) and the XUV energy
            // (in proportion to their H) with the water.
            let f_gases = gases.hydrogen_equivalent();
            let f_total = water + f_gases;
            diffusion_limit = 2.0 * params.diffusion_limit * f_total / (1.0 + f_total);
            let water_limit = 2.0 * params.diffusion_limit * water / (1.0 + f_total);
            let water_energy = if f_total > 0.0 {
                energy_limit * water / f_total
            } else {
                energy_limit
            };
            hydrogen_flux = water_limit.min(water_energy);
        }
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
        let mut capacity = OxygenSinks {
            volcanic: params.volcanic_reductants.max(0.0) * input.volcanic_degassing.max(0.0),
            seafloor: params.seafloor_oxidation.max(0.0) * input.new_crust.max(0.0),
            weathering: weathering_oxygen_demand(params, input.land),
            gases: 0.0,
        };
        // The reduced gases (#202): the volcanic H₂ and CO and the serpentinization's H₂ and
        // CH₄ meet the O₂ with the gases already in the air; the sulfur gases stay a capacity.
        let gas_sources = self.gases.as_mut().map(|gases| {
            let (sources, serpentinite) = gases.sources(
                input.volcanic_degassing,
                input.new_crust,
                input.new_crust_by_rate,
            );
            let (hydrogen, methane) = gases.pools(&sources, input.years);
            capacity.volcanic =
                gases.params().volcanic_sulfur.max(0.0) * input.volcanic_degassing.max(0.0);
            capacity.gases = (0.5 * hydrogen + 2.0 * methane) / input.years;
            (sources, serpentinite)
        });
        let pressure_per_mol = OXYGEN_MOLAR_MASS * GRAVITY / area / 1e5;
        let (taken, oxygen) = oxygen_step(
            params,
            self.state.oxygen + produced,
            input.years,
            capacity,
            pressure_per_mol,
        );
        let rates = OxygenSinks {
            volcanic: taken.volcanic / input.years,
            seafloor: taken.seafloor / input.years,
            weathering: taken.weathering / input.years,
            gases: taken.gases / input.years,
        };
        let mut gas_state = None;
        let mut water_used = 0.0;
        if let (Some(gases), Some((sources, serpentinite))) = (&mut self.gases, gas_sources) {
            let demand = input.years * capacity.gases;
            let titrated = if demand > 0.0 {
                taken.gases / demand
            } else {
                1.0
            };
            let per_year = area * 365.25 * 86400.0 / AVOGADRO;
            let water_hydrogen = hydrogen_flux * per_year / 2.0;
            let molar_mass = GAS_CONSTANT / air.gas_constant;
            gases.step(
                &GasInput {
                    years: input.years,
                    area,
                    column: air.pressure / (GRAVITY * molar_mass),
                    diffusion_limit: params.diffusion_limit * per_year,
                    water,
                    energy_room: (energy_limit * per_year / 2.0 - water_hydrogen).max(0.0),
                    background_oxygen: gases.params().background_oxygen,
                    nitrogen: (params.background_pressure / 1e5 - gases.params().background_oxygen)
                        .max(0.0),
                    pressure: air.pressure / 1e5,
                },
                sources,
                serpentinite,
                titrated,
                input.co2_pressure,
            );
            water_used = gases.water_used();
            hydrogen_flux += gases.state().hydrogen_flux;
            // The diffusion limit of the step's gases (the water's share used the last step's).
            let f_total = water + gases.hydrogen_equivalent();
            diffusion_limit = 2.0 * params.diffusion_limit * f_total / (1.0 + f_total);
            gas_state = Some(*gases.state());
        }
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
            oxygen_consumed: self.state.oxygen_consumed.add(&taken),
            oxygen,
            oxygen_pressure: oxygen * pressure_per_mol,
            sink_capacity: capacity,
            sink_rates: rates,
            at_floor,
            gases: gas_state,
        };
        if self.gases.is_some() {
            (water_mass + water_used * WATER_MOLAR_MASS) / SEA_WATER_DENSITY
        } else {
            water_mass / SEA_WATER_DENSITY
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RADIUS: f64 = 6.371e6;
    const OCEAN: f64 = 1.335e18;

    /// The escape without the reduced gases (#202), as #210 and #212 tested it.
    fn old_params() -> EscapeParams {
        let mut params = EscapeParams::default();
        params.gases.enabled = false;
        params
    }

    /// The escape of a barren planet with the reduced gases.
    fn barren_params() -> EscapeParams {
        let mut params = EscapeParams::default();
        params.gases.background_oxygen = 0.0;
        params.gases.background_methane = 0.0;
        params
    }

    /// New crust of an Earth-like spread of rates (kg so far after `step` steps of 1 Myr):
    /// 65 Gt/yr, ~5% ultraslow, ~25% slow, ~30% intermediate, ~40% fast.
    fn crust_by_rate(step: f64, scale: f64) -> [f64; SPREADING_RATE_BINS] {
        let mut bins = [0.0; SPREADING_RATE_BINS];
        let shares = [(1, 0.05), (3, 0.25), (6, 0.30), (10, 0.40)];
        for (bin, share) in shares {
            bins[bin] = step * 1e6 * 65e12 * scale * share;
        }
        bins
    }

    /// The budgets of the gases close over many steps with changing sources, climates and O₂:
    /// redox (H₂-equivalents), carbon, hydrogen, and the escape's O₂.
    #[test]
    fn gas_budgets_close() {
        for (redox, background) in [(0.0, 0.0), (-4.0, 0.0), (0.0, 0.21), (-2.0, 1e-3)] {
            let mut params = barren_params();
            params.gases.mantle_redox = redox;
            params.gases.background_oxygen = background;
            let mut escape = Escape::new(params, OCEAN);
            let mut ocean = OCEAN;
            for step in 0..300 {
                let celsius = [match step {
                    0..100 => 20.0,
                    100..200 => 95.0 + (step % 7) as f64,
                    _ => 25.0,
                }];
                let mut step_input = input(&celsius, ocean);
                step_input.years = 1e6;
                step_input.volcanic_degassing = 1e12 * (1.0 + (step % 5) as f64);
                step_input.new_crust = 1e14 * (1.0 + (step % 3) as f64);
                step_input.new_crust_by_rate =
                    crust_by_rate(step as f64 + 1.0, 1.0 + (step % 4) as f64);
                ocean -= escape.step(&step_input);
                let state = *escape.state();
                let gases = state.gases.expect("gases are on");
                let t = gases.totals;
                let tolerance = |x: f64| 1e-9 * x.abs().max(1.0);
                let redox_in = t.volcanic_hydrogen + t.volcanic_monoxide + t.serpentinization;
                let redox_out = t.hydrogen_titrated
                    + 4.0 * t.methane_titrated
                    + t.hydrogen_oxidised
                    + 4.0 * t.methane_oxidised
                    + t.hydrogen_escaped
                    + 2.0 * t.methane_escaped
                    + gases.hydrogen
                    + 4.0 * gases.methane;
                assert!(
                    (redox_in - redox_out).abs() <= tolerance(redox_in),
                    "{redox}: {step} redox {redox_in} {redox_out}"
                );
                let carbon = t.methane_destroyed() + gases.methane;
                assert!(
                    (t.methane_made - carbon).abs() <= tolerance(t.methane_made),
                    "{redox}: {step} carbon"
                );
                let hydrogen_in = 2.0 * t.volcanic_hydrogen + 2.0 * t.water_used;
                let hydrogen_out =
                    2.0 * gases.hydrogen + 4.0 * gases.methane + t.escaped_hydrogen_atoms();
                assert!(
                    (hydrogen_in - hydrogen_out).abs() <= tolerance(redox_in),
                    "{redox}: {step} hydrogen {hydrogen_in} {hydrogen_out}"
                );
                assert!(
                    (state.oxygen + state.oxygen_consumed.total() - state.oxygen_produced).abs()
                        <= 1e-9 * state.oxygen_produced.max(1.0)
                );
                assert!(gases.hydrogen >= 0.0 && gases.methane >= 0.0);
                assert!(gases.hydrogen_ratio < 1.0 && gases.methane_ratio < 1.0);
                // Never more escape than the XUV allows (to the lag of the water's share).
                assert!(
                    state.hydrogen_flux <= state.energy_limit * (1.0 + 1e-9),
                    "{redox}: {step}"
                );
            }
        }
    }

    /// A temperate barren planet: the H₂ is set by its sources and the diffusion limit
    /// (~150 ppm per Tmol/yr), CH₄ by its anoxic lifetime; a living planet's O₂ oxidises both.
    #[test]
    fn gas_steady_state() {
        let mut escape = Escape::new(barren_params(), OCEAN);
        let mut step_input = input(&[27.0], OCEAN);
        step_input.years = 1e6;
        step_input.volcanic_degassing = 3.5e12;
        step_input.new_crust = 65e12;
        for step in 1..=5 {
            step_input.new_crust_by_rate = crust_by_rate(step as f64, 1.0);
            escape.step(&step_input);
        }
        let gases = escape.state().gases.unwrap();
        // ~0.45 Tmol/yr volcanic, ~0.4 Tmol/yr serpentinization.
        let sources = gases.sources;
        assert!(
            (0.3e12..0.6e12).contains(&(sources.volcanic_hydrogen + sources.volcanic_monoxide)),
            "{sources:?}"
        );
        assert!(
            (0.25e12..0.55e12).contains(&sources.serpentinization),
            "{sources:?}"
        );
        assert!((1e9..1e10).contains(&sources.methane), "{sources:?}");
        let hydrogen = sources.hydrogen() / 1e12 * 150e-6;
        assert!(
            (gases.hydrogen_ratio / hydrogen - 1.0).abs() < 0.15,
            "{} {hydrogen}",
            gases.hydrogen_ratio
        );
        // CH₄: ~7 kyr with the escape, ppb to ppm.
        let lifetime = gases.methane / sources.methane;
        assert!((5e3..1e4).contains(&lifetime), "{lifetime}");
        assert!(
            (1e-8..1e-6).contains(&gases.methane_ratio),
            "{}",
            gases.methane_ratio
        );
        assert!(gases.methane_forcing < 0.0 && gases.methane_forcing > -0.95);
        // Essentially all of it escapes; the escape's O₂ takes a tiny share.
        assert!(gases.rates.hydrogen_titrated < 0.05 * sources.hydrogen());

        let mut escape = Escape::new(EscapeParams::default(), OCEAN);
        for step in 1..=5 {
            step_input.new_crust_by_rate = crust_by_rate(step as f64, 1.0);
            escape.step(&step_input);
        }
        let modern = escape.state().gases.unwrap();
        assert!(
            modern.hydrogen_ratio < 1e-8 && modern.methane_ratio < 1e-9,
            "{modern:?}"
        );
        assert!(modern.methane_forcing.abs() < 1e-3);
    }

    /// A reduced mantle outgasses more H₂ than the XUV of a late sun can remove: it builds up,
    /// the escape stays at the energy limit, the mixing ratio below 1.
    #[test]
    fn energy_limited_hydrogen() {
        let mut params = barren_params();
        params.gases.mantle_redox = -4.0;
        let mut escape = Escape::new(params, OCEAN);
        let mut step_input = input(&[20.0], OCEAN);
        step_input.years = 1e6;
        step_input.volcanic_degassing = 4e12;
        step_input.sun_age = 9.0;
        let mut previous = 0.0;
        for step in 1..=200 {
            step_input.new_crust_by_rate = crust_by_rate(step as f64, 1.0);
            escape.step(&step_input);
            let state = escape.state();
            let gases = state.gases.unwrap();
            assert!(gases.hydrogen >= previous);
            previous = gases.hydrogen;
            assert!(state.hydrogen_flux <= state.energy_limit * (1.0 + 1e-9));
        }
        let state = escape.state();
        let gases = state.gases.unwrap();
        assert!(state.energy_limited());
        assert!((state.hydrogen_flux / state.energy_limit - 1.0).abs() < 0.01);
        assert!(
            gases.hydrogen_ratio > 0.01 && gases.hydrogen_ratio < 1.0,
            "{}",
            gases.hydrogen_ratio
        );
        assert!(gases.hydrogen_forcing > 0.0);
    }

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
        let params = old_params();
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
            // ~0.5 and ~1.3 Tmol O₂/yr.
            volcanic_degassing: 3e12,
            new_crust: 1.6e14,
            new_crust_by_rate: [0.0; SPREADING_RATE_BINS],
            land: &[],
        }
    }

    /// A temperate planet loses ~1 m of ocean per Gyr; a moist greenhouse is energy-limited and
    /// loses an ocean in Gyr. Water, hydrogen and oxygen balance.
    #[test]
    fn water_loss_and_oxygen_balance() {
        let mut escape = Escape::new(old_params(), OCEAN);
        let volume = escape.step(&input(&[27.0], OCEAN));
        let ocean_area = 3.6e14;
        let per_gyr = volume * 1e2 / ocean_area;
        assert!((0.1..5.0).contains(&per_gyr), "{per_gyr} m/Gyr");
        let state = *escape.state();
        assert!(!state.energy_limited());
        // Far below the sinks: no O₂ in the air.
        assert_eq!(state.oxygen, 0.0);
        assert!(
            (state.oxygen_consumed.total() - state.oxygen_produced).abs()
                <= 1e-12 * state.oxygen_produced
        );

        // Sinks that take all the O₂ (it would otherwise dry the stratosphere, see below).
        let params = EscapeParams {
            volcanic_reductants: 1e20,
            ..old_params()
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
            (state.oxygen + state.oxygen_consumed.total() - state.oxygen_produced).abs()
                < 1e-9 * state.oxygen_produced
        );
        assert_eq!(state.oxygen, 0.0);

        // With sinks of a few Tmol/yr the O₂ builds up (bars), dilutes the vapour and ends the
        // fast loss: the escape becomes diffusion-limited.
        let mut escape = Escape::new(old_params(), OCEAN);
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
            (state.oxygen + state.oxygen_consumed.total() - state.oxygen_produced).abs()
                < 1e-9 * state.oxygen_produced
        );
    }

    /// The ocean never falls below the floor.
    #[test]
    fn ocean_floor() {
        let params = EscapeParams {
            min_ocean_fraction: 0.5,
            heating_efficiency: 1e3,
            volcanic_reductants: 1e20,
            ..old_params()
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
        let mut escape = Escape::new(old_params(), OCEAN);
        escape.step(&input(&[63.0, 77.0], OCEAN));
        let spread = escape.state().stratospheric_water;
        escape.step(&input(&[70.0], OCEAN));
        assert!(spread > 1.5 * escape.state().stratospheric_water);
        assert_eq!(escape.step(&input(&[], OCEAN)), 0.0);
    }

    fn eroding_land() -> Vec<LandCell> {
        (0..10)
            .map(|cell| LandCell {
                area: 1e12,
                temperature: 15.0,
                runoff: 0.3,
                ice: if cell == 0 { 1.0 } else { 0.0 },
                bedrock_erosion: 5e-5,
                sediment_erosion: 1e-4,
                bedrock_density: 2800.0,
                mafic: cell as f64 / 9.0,
                carbonate: 0.0,
                bare: 0.0,
            })
            .collect()
    }

    /// The weathering's demand follows the eroded bedrock and its rock; its oxidation the O₂.
    #[test]
    fn weathering_demand_and_oxidation() {
        let params = old_params();
        let land = eroding_land();
        // 9 ice-free cells of 1.4e11 kg/yr each, half-way between felsic and basalt on average.
        let demand = weathering_oxygen_demand(&params, &land);
        let rock = 9.0 * 1e12 * 5e-5 * 2800.0;
        let mean = land[1..]
            .iter()
            .map(|cell| 0.089 + cell.mafic * (0.23 - 0.089))
            .sum::<f64>()
            / 9.0;
        assert!((demand / (rock * mean) - 1.0).abs() < 1e-12, "{demand}");
        assert_eq!(oxidised_share(&params, 0.0), 0.0);
        let pal = 0.21;
        let low = oxidised_share(&params, 1e-5 * pal);
        let high = oxidised_share(&params, 1e-3 * pal);
        assert!((0.2..0.35).contains(&low), "{low}");
        assert!(high > 0.95, "{high}");
        assert!(oxidised_share(&params, 1.0) > 0.999_999);
    }

    /// The O₂ budget closes over many steps with changing sources and sinks: produced = taken
    /// by each sink + left in the air, the sinks never take more than there is, and nothing is
    /// left while the fixed sinks exceed the source.
    #[test]
    fn oxygen_budget_closes() {
        let params = old_params();
        let land = eroding_land();
        let mut escape = Escape::new(params.clone(), OCEAN);
        let mut ocean = OCEAN;
        let mut produced_before = 0.0;
        let mut oxygen_before = 0.0;
        let mut peak = 0.0;
        for step in 0..400 {
            // Temperate, then a moist greenhouse, then temperate again; the tectonics vary.
            let celsius = [match step {
                0..100 => 20.0,
                100..300 => 95.0 + (step % 7) as f64,
                _ => 25.0,
            }];
            let mut step_input = input(&celsius, ocean);
            step_input.years = 1e6;
            step_input.volcanic_degassing = 1e12 * (1.0 + (step % 5) as f64);
            step_input.new_crust = 1e14 * (1.0 + (step % 3) as f64);
            step_input.land = &land;
            ocean -= escape.step(&step_input);
            let state = *escape.state();
            let available = oxygen_before + state.oxygen_produced - produced_before;
            let taken = state.sink_rates.total() * 1e6;
            assert!(
                taken <= available * (1.0 + 1e-12),
                "{step}: {taken} > {available}"
            );
            assert!(state.oxygen >= 0.0);
            assert!(
                (state.oxygen + state.oxygen_consumed.total() - state.oxygen_produced).abs()
                    <= 1e-12 * state.oxygen_produced,
                "{step}"
            );
            // The fixed sinks take what they can before the weathering, which needs O₂.
            if state.oxygen == 0.0 {
                assert_eq!(state.sink_rates.weathering, 0.0);
            } else {
                assert_eq!(state.sink_rates.volcanic, state.sink_capacity.volcanic);
                assert_eq!(state.sink_rates.seafloor, state.sink_capacity.seafloor);
                assert!(state.sink_rates.weathering <= state.sink_capacity.weathering);
            }
            match step {
                99 => assert_eq!(state.oxygen, 0.0),
                299 => {
                    // Bars of O₂: the weathering is fully oxidising.
                    assert!(state.oxygen_pressure > 0.1, "{}", state.oxygen_pressure);
                    let share = state.sink_rates.weathering / state.sink_capacity.weathering;
                    assert!(share > 0.99, "{share}");
                    peak = state.oxygen;
                }
                _ => {}
            }
            produced_before = state.oxygen_produced;
            oxygen_before = state.oxygen;
        }
        // Afterwards the sinks draw the O₂ down again.
        let state = *escape.state();
        assert!(state.oxygen < peak, "{} {peak}", state.oxygen);
        let consumed = state.oxygen_consumed;
        assert!(consumed.volcanic > 0.0 && consumed.seafloor > 0.0 && consumed.weathering > 0.0);
    }

    /// The implicit step meets the root: with only the weathering (no fixed sinks), the O₂ that
    /// stays and the weathering at its pressure add up to the O₂ there was.
    #[test]
    fn implicit_weathering_step() {
        let params = old_params();
        let capacity = OxygenSinks {
            volcanic: 0.0,
            seafloor: 0.0,
            weathering: 2e12,
            gases: 0.0,
        };
        let per_mol =
            OXYGEN_MOLAR_MASS * GRAVITY / (4.0 * std::f64::consts::PI * RADIUS * RADIUS) / 1e5;
        // (Where nearly all is taken, the steep √p makes the check need more than rounding.)
        for available in [1e18, 1e20, 1e21, 1e22] {
            let (taken, left) = oxygen_step(&params, available, 1e6, capacity, per_mol);
            assert!(left > 0.0);
            let expected = 1e6 * 2e12 * oxidised_share(&params, left * per_mol);
            assert!(
                (taken.weathering - expected).abs() <= 1e-6 * expected,
                "{available}: {} vs {expected}",
                taken.weathering
            );
            assert!((taken.total() + left - available).abs() <= 1e-12 * available);
        }
    }
}
