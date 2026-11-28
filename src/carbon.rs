//! The long-term carbon cycle (#154): CO₂ of the atmosphere and ocean as a global state, with
//! sources from the tectonics and sinks by weathering, which sets the greenhouse forcing of the
//! temperature model in every step.
//!
//! On a planet without life, carbon leaves the interior by **tectonic degassing** (magmatic CO₂
//! at ridges, arcs and plumes, and decarbonation of buried and collided sediment) and returns to
//! the rock by **silicate weathering** followed by inorganic carbonate burial (on land: one C
//! per Ca or Mg released; Walker et al., 1981; Berner, 2004) and by the **alteration of young
//! oceanic crust** (seafloor weathering). Both sinks speed up as the climate warms and the CO₂
//! rises, which stabilises the climate while the sun brightens: the carbonate–silicate feedback.
//!
//! - **State**: the exogenic carbon `N` (mol) of the atmosphere and the ocean, which the fluxes
//!   change. The atmospheric CO₂ follows from it ([`exogenic_carbon`]): the atmosphere holds
//!   `p A / (g M̄)`, the ocean `M (DIC₀ r^{1/3} + K₀ p)` (calcite-saturated, so the bicarbonate
//!   grows with the cube root of the CO₂).
//! - **Forcing**: [`crate::presets::co2_forcing`], with a quadratic term at high CO₂ (Byrne &
//!   Goldblatt, 2014). The CO₂ at the start follows from the start's forcing, so the start is
//!   continuous.
//! - **Sources** ([`Sources`]): coefficients (mol C per unit of a tectonic rate) from Earth's
//!   fluxes and rates, applied to the model's rates per step: new sea floor, subducted crust and
//!   sediment, the plumes' flux, and metamorphosed and collided sediment.
//! - **Sinks** ([`Sinks`]): silicate weathering of every land cell, kinetic (runoff, Arrhenius
//!   temperature, CO₂, lithology, plants) but limited by the supply of fresh rock from the
//!   erosion (`w = w_sup (1 − e^{−w_kin/w_sup})`); seafloor weathering per kg of new crust.
//! - **Calibration**: one factor φ on both kinetic laws is set at the end of the planet's
//!   ramp-up phase ([`crate::ramp_up`], #213) so that the sinks balance the sources of its last
//!   window (the one that showed the tectonics settled) at the start's CO₂ and the climate of
//!   those steps ([`CarbonParams::phi`]). The CO₂ is held until then ([`CarbonCycle::end_hold`]).
//! - **Coupling**: an implicit (backward Euler) step for `ln r` with the temperature change the
//!   new forcing will cause predicted by a fixed sensitivity λ, which is stable at the 1 Myr
//!   step also when the feedback is faster than the step.
//!
//! See `notes/issue-154/plan.md` for the values and their sources.

use std::collections::VecDeque;

use tracing::warn;

use crate::math;
use crate::presets::{co2_forcing, co2_ratio_for_forcing};

/// The calibration span (Myr) until the planet sets it ([`CarbonCycle::set_calibration_span`]).
const DEFAULT_CALIBRATION_SPAN: f64 = 20.0;

/// Pre-industrial partial pressure of CO₂ in bar (280 ppm of a 1 bar atmosphere), the reference
/// of the CO₂ ratio `r`.
pub const REFERENCE_PRESSURE: f64 = 280e-6;
/// Mean molar mass of the air in kg/mol: a partial pressure `p` holds `p / (g M̄)` mol per m².
const AIR_MOLAR_MASS: f64 = 0.029;
/// Surface gravity in m/s².
const GRAVITY: f64 = 9.81;
/// Density of sea water in kg/m³.
pub const SEA_WATER_DENSITY: f64 = 1025.0;
/// Molar gas constant in J/(mol K).
const GAS_CONSTANT: f64 = 8.314;
/// 0 °C in K.
const ZERO_CELSIUS: f64 = 273.15;
/// Freezing point of sea water in °C: the coldest bottom water.
const SEA_WATER_FREEZING: f64 = -1.8;

/// Parameters of the carbon cycle; the defaults describe today's Earth (with plants, see
/// [`crate::presets::Biosphere`] for a barren planet). Rates in mol C per year.
#[derive(Clone, Debug)]
pub struct CarbonParams {
    /// Whether the CO₂ follows the carbon cycle (`--no-carbon-cycle` turns it off): otherwise
    /// the forcing stays that of the start, and the fluxes at that CO₂ are only logged.
    pub enabled: bool,
    /// Ridge degassing per kg of new oceanic crust (mol/kg): ~2 Tmol/yr on Earth's ~65 Gt/yr.
    pub ridge_carbon: f64,
    /// Arc degassing per kg of subducted oceanic crust from the mantle wedge's own carbon
    /// (mol/kg).
    pub wedge_carbon: f64,
    /// Share of the carbon of the altered oceanic crust (its seafloor uptake) that returns at
    /// arcs when it subducts, and the time (Myr) over which the uptake is averaged for the
    /// crust arriving at the trenches (about its mean age there).
    pub slab_return: f64,
    pub slab_age: f64,
    /// Arc degassing per kg of subducted sediment (mol/kg): its carbonate, about half returns.
    pub slab_sediment_carbon: f64,
    /// Plume degassing per m²/s of plume flux ([`crate::tectonics::Plume::flux`]), mol/yr per
    /// m²/s: 1 Tmol/yr for the model's typical ~9 plumes (0.036 m²/s).
    pub plume_carbon: f64,
    /// Degassing per kg of crust emplaced by large igneous provinces (mol/kg; #106): the CO₂ of
    /// a plume head's magma, erupted and intruded (~0.3 wt% CO₂; basaltic magmas carry
    /// 0.1–0.5 wt%). Booked into the plume source in the step of the LIP, as a pulse. The tails'
    /// magma is [`Self::plume_carbon`]'s, so their crust adds no carbon of its own.
    pub lip_carbon: f64,
    /// Metamorphic decarbonation per kg of sediment metamorphosed by burial or stacked in
    /// collisions (mol/kg).
    pub metamorphic_carbon: f64,
    /// Kinetic silicate weathering of felsic rock in mol Ca+Mg per m² and year at the reference
    /// temperature, today's CO₂, a runoff of 1 m/yr and with plants (scaled by
    /// [`Self::plant_factor`] and the runoff to the power of [`Self::runoff_exponent`]).
    pub land_weathering: f64,
    /// Weathering with the plants of today's land relative to [`Self::land_weathering`]: 1
    /// with plants, 0.25 on barren land.
    pub plant_factor: f64,
    /// Exponent of the CO₂ dependence of land weathering, and whether plants saturate it: with
    /// plants the dependence is `(2r / (1 + r))^e` (roots raise the soil's CO₂; GEOCARB), on
    /// barren land `r^e` (soil CO₂ is atmospheric). 1 or 0.
    pub co2_exponent: f64,
    pub soil_co2_saturation: f64,
    /// Exponent of the runoff (m/yr) in the kinetic weathering.
    pub runoff_exponent: f64,
    /// Activation energy of land weathering in J/mol and the temperature (°C) of
    /// [`Self::land_weathering`].
    pub activation_energy: f64,
    pub reference_temperature: f64,
    /// Kinetic weathering of basalt relative to felsic rock (5–10× faster).
    pub basalt_reactivity: f64,
    /// Ca + Mg content (mol/kg) of felsic and of basaltic rock; mixed crust is interpolated by
    /// its felsic fraction.
    pub felsic_cations: f64,
    pub basalt_cations: f64,
    /// Weatherable part of eroded sediment relative to fresh felsic rock.
    pub sediment_weatherable: f64,
    /// Seafloor weathering: carbon taken up per kg of new oceanic crust (mol/kg) at today's CO₂
    /// and the reference bottom water temperature (°C), its CO₂ exponent, activation energy
    /// (J/mol) and the most the crust can take up (mol/kg).
    pub seafloor_carbon: f64,
    pub seafloor_co2_exponent: f64,
    pub seafloor_activation_energy: f64,
    pub seafloor_reference_temperature: f64,
    pub seafloor_max_carbon: f64,
    /// Latitude (degrees) poleward of which the ocean's surface sets the bottom water
    /// temperature.
    pub deep_water_latitude: f64,
    /// Dissolved inorganic carbon of the ocean at today's CO₂ (mol/kg) and the solubility of
    /// CO₂ (mol/(kg bar)).
    pub ocean_dic: f64,
    pub co2_solubility: f64,
    /// Global temperature change (K) per W/m² of forcing that the implicit step predicts for
    /// the next climate update; at least the model's own sensitivity (stability, see
    /// `notes/issue-154/plan.md`).
    pub climate_sensitivity: f64,
    /// Relative change of the runoff per kelvin that the implicit step predicts.
    pub runoff_sensitivity: f64,
    /// Scale of both kinetic weathering laws. NaN (the default): set at the end of the
    /// ramp-up phase ([`CarbonCycle::end_hold`]) so that the sinks balance the sources over its
    /// last window, at the start's CO₂ (a continuous start); until then the
    /// CO₂ stays that of the start, as with `--no-carbon-cycle`. The ramp-up lasts until the
    /// tectonics has settled from the generated start ([`crate::ramp_up`]), so that φ balances
    /// the planet's degassing, not that of the model's transient. 1 takes the laws as they are,
    /// without a hold.
    pub phi: f64,
    /// The calibrated φ is clamped to `phi_min..=phi_max` (with a warning). φ only absorbs the
    /// residual uncertainty of the laws, mainly the seafloor uptake (×3), the kinetics (×2) and
    /// the degassing coefficients (×2), so a plausible φ lies within ~0.3–3. That holds for both
    /// biospheres: the barren laws add the plants' uncertainty, the modern ones put more of the
    /// sink on land (`notes/issue-154/phi-barren.md`, `phi-modern.md`), so the presets share the
    /// band. Where the clamp binds, the sinks do not balance the sources at the start's CO₂,
    /// and the CO₂ moves after the ramp-up to where they do.
    pub phi_min: f64,
    /// See [`Self::phi_min`].
    pub phi_max: f64,
    /// Time (Myr) over which the tectonic rates (of the degassing and of the new crust that the
    /// seafloor weathering alters) are smoothed: they jump with the grid. The smoothing starts at
    /// the end of the ramp-up from the mean rates of the calibration.
    pub degassing_smoothing: f64,
}

impl Default for CarbonParams {
    fn default() -> Self {
        Self {
            enabled: true,
            ridge_carbon: 0.03,
            wedge_carbon: 0.015,
            slab_return: 0.5,
            slab_age: 100.0,
            slab_sediment_carbon: 0.15,
            plume_carbon: 1e12 / 0.036,
            lip_carbon: 0.07,
            metamorphic_carbon: 0.5,
            land_weathering: 0.073,
            plant_factor: 1.0,
            co2_exponent: 0.4,
            soil_co2_saturation: 1.0,
            runoff_exponent: 0.65,
            activation_energy: 50e3,
            reference_temperature: 15.0,
            basalt_reactivity: 5.0,
            felsic_cations: 1.3,
            basalt_cations: 3.7,
            sediment_weatherable: 0.3,
            seafloor_carbon: 0.015,
            seafloor_co2_exponent: 0.23,
            seafloor_activation_energy: 92e3,
            seafloor_reference_temperature: 2.0,
            seafloor_max_carbon: 0.26,
            deep_water_latitude: 55.0,
            ocean_dic: 2.0e-3,
            co2_solubility: 0.04,
            climate_sensitivity: 0.8,
            runoff_sensitivity: 0.03,
            phi: f64::NAN,
            phi_min: 0.3,
            phi_max: 3.0,
            degassing_smoothing: 3.0,
        }
    }
}

impl CarbonParams {
    /// Sets a parameter by its field name (`enabled` as 0 or 1).
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        if name == "enabled" {
            self.enabled = value != 0.0;
            return Ok(());
        }
        let field = match name {
            "ridge_carbon" => &mut self.ridge_carbon,
            "wedge_carbon" => &mut self.wedge_carbon,
            "slab_return" => &mut self.slab_return,
            "slab_age" => &mut self.slab_age,
            "slab_sediment_carbon" => &mut self.slab_sediment_carbon,
            "plume_carbon" => &mut self.plume_carbon,
            "lip_carbon" => &mut self.lip_carbon,
            "metamorphic_carbon" => &mut self.metamorphic_carbon,
            "land_weathering" => &mut self.land_weathering,
            "plant_factor" => &mut self.plant_factor,
            "co2_exponent" => &mut self.co2_exponent,
            "soil_co2_saturation" => &mut self.soil_co2_saturation,
            "runoff_exponent" => &mut self.runoff_exponent,
            "activation_energy" => &mut self.activation_energy,
            "reference_temperature" => &mut self.reference_temperature,
            "basalt_reactivity" => &mut self.basalt_reactivity,
            "felsic_cations" => &mut self.felsic_cations,
            "basalt_cations" => &mut self.basalt_cations,
            "sediment_weatherable" => &mut self.sediment_weatherable,
            "seafloor_carbon" => &mut self.seafloor_carbon,
            "seafloor_co2_exponent" => &mut self.seafloor_co2_exponent,
            "seafloor_activation_energy" => &mut self.seafloor_activation_energy,
            "seafloor_reference_temperature" => &mut self.seafloor_reference_temperature,
            "seafloor_max_carbon" => &mut self.seafloor_max_carbon,
            "deep_water_latitude" => &mut self.deep_water_latitude,
            "ocean_dic" => &mut self.ocean_dic,
            "co2_solubility" => &mut self.co2_solubility,
            "climate_sensitivity" => &mut self.climate_sensitivity,
            "runoff_sensitivity" => &mut self.runoff_sensitivity,
            "phi" => &mut self.phi,
            "phi_min" => &mut self.phi_min,
            "phi_max" => &mut self.phi_max,
            "degassing_smoothing" => &mut self.degassing_smoothing,
            _ => anyhow::bail!("unknown carbon parameter {name}"),
        };
        *field = value;
        Ok(())
    }

    /// The land weathering's dependence on the CO₂ ratio `r` (1 at `r = 1`).
    fn land_co2_factor(&self, ratio: f64) -> f64 {
        let base = if self.soil_co2_saturation > 0.0 {
            2.0 * ratio / (1.0 + ratio)
        } else {
            ratio
        };
        math::powf(base, self.co2_exponent)
    }
}

/// Cumulative tectonic quantities from which the degassing is computed, as the tectonics
/// reports them (see [`crate::tectonics::Simulation::carbon_drivers`]): masses in kg so far,
/// the plume flux now.
#[derive(Clone, Copy, Debug, Default)]
pub struct TectonicDrivers {
    /// New oceanic crust at ridges.
    pub new_sea_floor: f64,
    /// Subducted oceanic and transitional crust.
    pub subducted_crust: f64,
    /// Subducted sediment.
    pub subducted_sediment: f64,
    /// Sediment metamorphosed by burial and stacked onto the upper plate in collisions.
    pub metamorphic_sediment: f64,
    /// Crust emplaced by large igneous provinces (#106).
    pub lip_crust: f64,
    /// Sum of the current plume fluxes in m²/s.
    pub plume_flux: f64,
}

/// Degassing in mol C/yr.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sources {
    pub ridges: f64,
    pub arcs: f64,
    pub plumes: f64,
    pub metamorphic: f64,
}

impl Sources {
    pub fn total(&self) -> f64 {
        self.ridges + self.arcs + self.plumes + self.metamorphic
    }
}

/// Carbon removal in mol C/yr: silicate weathering on land (buried as carbonate in the ocean)
/// and seafloor weathering (carbonate in the altered crust).
#[derive(Clone, Copy, Debug, Default)]
pub struct Sinks {
    pub land: f64,
    pub seafloor: f64,
}

impl Sinks {
    pub fn total(&self) -> f64 {
        self.land + self.seafloor
    }
}

/// The state of the carbon cycle after a step, for the log and the statistics.
#[derive(Clone, Copy, Debug, Default)]
pub struct CarbonState {
    /// CO₂ relative to the pre-industrial 280 ppm, and as a partial pressure in bar.
    pub ratio: f64,
    pub pressure: f64,
    /// Greenhouse forcing of that CO₂ (W/m²), which the next climate update uses.
    pub forcing: f64,
    /// Exogenic carbon (mol) and the part of it in the atmosphere.
    pub carbon: f64,
    pub atmosphere: f64,
    pub sources: Sources,
    /// The sinks at the end of the step (the implicit step), i.e. with the new CO₂ and the
    /// predicted temperature change.
    pub sinks: Sinks,
    /// The scale of the kinetic laws (see [`CarbonParams::phi`]); NaN before the first step.
    pub phi: f64,
    /// Temperature change (K) the step predicted for the next climate update.
    pub predicted_warming: f64,
    /// Bottom water temperature (°C) of the seafloor weathering.
    pub deep_water_temperature: f64,
    /// Share of the land weathering's supply limit that the kinetics use (0 to 1): near 1 the
    /// weathering is supply-limited.
    pub weathering_intensity: f64,
    /// New oceanic crust (kg/yr, smoothed like the degassing) and the degassing of the mantle at
    /// arcs and plumes (mol C/yr: the wedge's own carbon and the plumes', without recycled
    /// sediment and seafloor carbon), for the O₂ sinks of the escape (#210).
    pub new_crust: f64,
    pub arc_plume_mantle_degassing: f64,
}

impl CarbonState {
    /// One line for the log: CO₂, forcing, the sources and sinks in Tmol C/yr.
    pub fn summary(&self) -> String {
        let s = &self.sources;
        let k = &self.sinks;
        format!(
            "carbon: pCO₂ {:.4} bar ({:.1}× PAL), forcing {:+.2} W/m² | sources {:.2} Tmol/yr \
             (ridges {:.2}, arcs {:.2}, plumes {:.2}, metamorphic {:.2}) | sinks {:.2} (land \
             {:.2}, seafloor {:.2}) | φ {:.3}, weathering intensity {:.2}, bottom water {:.1} °C, \
             predicted ΔT {:+.3} K | exogenic carbon {:.3e} mol ({:.0}% in the air)",
            self.pressure,
            self.ratio,
            self.forcing,
            s.total() / 1e12,
            s.ridges / 1e12,
            s.arcs / 1e12,
            s.plumes / 1e12,
            s.metamorphic / 1e12,
            k.total() / 1e12,
            k.land / 1e12,
            k.seafloor / 1e12,
            self.phi,
            self.weathering_intensity,
            self.deep_water_temperature,
            self.predicted_warming,
            self.carbon,
            100.0 * self.atmosphere / self.carbon,
        )
    }
}

/// One land cell as the weathering sees it.
#[derive(Clone, Copy, Debug)]
pub struct LandCell {
    /// Area in m².
    pub area: f64,
    /// Annual mean surface temperature in °C.
    pub temperature: f64,
    /// Runoff in m/yr.
    pub runoff: f64,
    /// Permanent ice cover, 0 to 1.
    pub ice: f64,
    /// Bedrock and sediment eroded from the cell in the step, in m/yr (≥ 0).
    pub bedrock_erosion: f64,
    pub sediment_erosion: f64,
    /// Density of the bedrock in kg/m³.
    pub bedrock_density: f64,
    /// Mafic (basaltic) share of the bedrock, 0 to 1: one minus its felsic fraction.
    pub mafic: f64,
}

/// What one step of the carbon cycle needs of the planet.
pub struct StepInput<'a> {
    /// Length of the step in years.
    pub years: f64,
    pub drivers: TectonicDrivers,
    pub land: &'a [LandCell],
    /// Bottom water temperature in °C (before the predicted change).
    pub deep_water_temperature: f64,
    /// Mass of the ocean in kg.
    pub ocean_mass: f64,
    /// Area of the planet in m².
    pub planet_area: f64,
    /// Change of the absorbed sunlight (W/m², global mean) from this step's climate update to
    /// the next, by the brightening sun.
    pub sunlight_change: f64,
    /// Net radiation at the top of the atmosphere (W/m²) of this step's climate update: the
    /// warming it has not reached yet is about [`CarbonParams::climate_sensitivity`] times it.
    pub toa_imbalance: f64,
}

/// The per-cell parts of the land weathering that do not depend on the CO₂ or the temperature
/// change: supply limit (mol/m²/yr), the kinetic rate at φ = 1, r = 1 and no change, the
/// temperature in K, and the area.
#[derive(Clone, Debug)]
struct WeatheringCell {
    supply: f64,
    kinetic: f64,
    temperature: f64,
    area: f64,
}

/// The tectonic rates of a step in kg/yr: new and subducted oceanic crust, subducted sediment,
/// and metamorphosed or collided sediment.
#[derive(Clone, Copy, Debug, Default)]
struct Rates {
    ridge: f64,
    subducted: f64,
    sediment: f64,
    metamorphic: f64,
}

impl Rates {
    /// `self + weight (other − self)`.
    fn towards(self, other: Rates, weight: f64) -> Rates {
        let mix = |a: f64, b: f64| a + weight * (b - a);
        Rates {
            ridge: mix(self.ridge, other.ridge),
            subducted: mix(self.subducted, other.subducted),
            sediment: mix(self.sediment, other.sediment),
            metamorphic: mix(self.metamorphic, other.metamorphic),
        }
    }
}

/// What the calibration of φ keeps of a step of its window.
#[derive(Clone, Debug)]
struct CalibrationStep {
    years: f64,
    cells: Vec<WeatheringCell>,
    rates: Rates,
    deep_water_temperature: f64,
    /// The degassing without the return of the seafloor's carbon at arcs (mol/yr).
    degassing: f64,
}

/// The carbon cycle (see the module documentation).
#[derive(Clone, Debug)]
pub struct CarbonCycle {
    params: CarbonParams,
    /// Exogenic carbon in mol.
    carbon: f64,
    /// CO₂ ratio of the current forcing, and the forcing (W/m²): exactly the start's until the
    /// CO₂ first changes.
    ratio: f64,
    forcing: f64,
    /// The drivers at the end of the last step.
    previous: Option<TectonicDrivers>,
    /// Simulated years so far.
    years: f64,
    /// The steps of the last [`Self::calibration_span`], until φ is set.
    calibration: VecDeque<CalibrationStep>,
    /// The time (Myr) before the end of the hold whose sources and sinks φ balances: the last
    /// window of the ramp-up ([`Self::set_calibration_span`]).
    calibration_span: f64,
    /// The degassing of the last step without the seafloor carbon returning at arcs (mol/yr):
    /// a rate of the tectonics alone, independent of φ.
    tectonic_degassing: f64,
    /// φ once set.
    phi: Option<f64>,
    /// Carbon of the altered oceanic crust that arrives at the trenches (mol/kg): the seafloor
    /// uptake, averaged over [`CarbonParams::slab_age`]. NaN until known.
    slab_carbon: f64,
    /// The tectonic rates smoothed over [`CarbonParams::degassing_smoothing`], once φ is set
    /// (before, the raw rates of each step are used, as in the calibration).
    smoothed: Option<Rates>,
    state: CarbonState,
}

impl CarbonCycle {
    /// The carbon cycle of a planet whose climate starts with the greenhouse `forcing` (W/m²),
    /// with an ocean of `ocean_mass` kg on a planet of `planet_area` m².
    pub fn new(params: CarbonParams, forcing: f64, ocean_mass: f64, planet_area: f64) -> Self {
        let ratio = co2_ratio_for_forcing(forcing);
        let carbon = exogenic_carbon(&params, ratio, ocean_mass, planet_area);
        let state = CarbonState {
            ratio,
            pressure: ratio * REFERENCE_PRESSURE,
            forcing,
            carbon,
            atmosphere: atmospheric_carbon(ratio, planet_area),
            phi: f64::NAN,
            deep_water_temperature: f64::NAN,
            ..CarbonState::default()
        };
        let phi = (!params.phi.is_nan()).then_some(params.phi);
        Self {
            params,
            carbon,
            ratio,
            forcing,
            previous: None,
            years: 0.0,
            calibration: VecDeque::new(),
            calibration_span: DEFAULT_CALIBRATION_SPAN,
            tectonic_degassing: 0.0,
            phi,
            slab_carbon: f64::NAN,
            smoothed: None,
            state,
        }
    }

    pub fn params(&self) -> &CarbonParams {
        &self.params
    }

    pub fn state(&self) -> &CarbonState {
        &self.state
    }

    /// Records the tectonic drivers at the start of the run, so that the first step's degassing
    /// covers only that step.
    pub fn start(&mut self, drivers: TectonicDrivers) {
        self.previous = Some(drivers);
    }

    /// Whether the CO₂ follows the carbon cycle in the next step: it is enabled and φ is known.
    fn active(&self) -> bool {
        self.params.enabled && self.phi.is_some()
    }

    /// Sets the time (Myr) before the end of the hold whose steps φ balances: the last window
    /// of the ramp-up phase, the one that showed the tectonics settled.
    pub fn set_calibration_span(&mut self, span: f64) {
        self.calibration_span = span;
    }

    /// Whether the CO₂ is held until φ is calibrated ([`Self::end_hold`]): the cycle is
    /// enabled and φ was not given.
    pub fn holds_co2(&self) -> bool {
        self.params.enabled && self.phi.is_none()
    }

    /// The degassing of the last step without the seafloor carbon returning at arcs (mol/yr):
    /// ridges, arcs (mantle wedge and sediment), plumes and metamorphism, unsmoothed. A rate of
    /// the tectonics alone, which the ramp-up phase watches.
    pub fn tectonic_degassing(&self) -> f64 {
        self.tectonic_degassing
    }

    /// Adds `carbon` mol to the exogenic carbon (negative: takes it): the carbon that the
    /// reduced gases (#202) gave back as CO₂ or took to make CH₄ in a step.
    pub fn exchange(&mut self, carbon: f64) {
        self.carbon += carbon;
        self.state.carbon = self.carbon;
    }

    /// While the CO₂ is held ([`Self::holds_co2`]), sets it to the CO₂ whose forcing is
    /// `forcing` (W/m²), with its exogenic carbon in an ocean of `ocean_mass` kg on a planet
    /// of `planet_area` m²: the CO₂ then makes up the change of other greenhouse gases (#202),
    /// so that the total forcing of the start is held. Does nothing otherwise.
    pub fn hold_forcing(&mut self, forcing: f64, ocean_mass: f64, planet_area: f64) {
        if !self.holds_co2() {
            return;
        }
        self.ratio = co2_ratio_for_forcing(forcing);
        self.forcing = forcing;
        self.carbon = exogenic_carbon(&self.params, self.ratio, ocean_mass, planet_area);
        let state = &mut self.state;
        state.ratio = self.ratio;
        state.pressure = self.ratio * REFERENCE_PRESSURE;
        state.forcing = forcing;
        state.carbon = self.carbon;
        state.atmosphere = atmospheric_carbon(self.ratio, planet_area);
    }

    /// The forcing (W/m²) of the current CO₂.
    pub fn forcing(&self) -> f64 {
        self.forcing
    }

    /// Ends the hold of the CO₂ at the end of the ramp-up phase: φ is set so that the sinks
    /// balance the sources over the recorded steps (the last [`Self::calibration_span`]) at the
    /// current CO₂, and the smoothing of the tectonic
    /// rates starts from their mean over those steps. From the next step on, the CO₂ follows
    /// the cycle. Does nothing if φ is known.
    pub fn end_hold(&mut self) {
        if self.phi.is_some() || self.calibration.is_empty() {
            return;
        }
        let (phi, slab_carbon) = self.calibrate(math::ln(self.ratio));
        let years: f64 = self.calibration.iter().map(|step| step.years).sum();
        let mean = self
            .calibration
            .iter()
            .fold(Rates::default(), |sum, step| Rates {
                ridge: sum.ridge + step.years / years * step.rates.ridge,
                subducted: sum.subducted + step.years / years * step.rates.subducted,
                sediment: sum.sediment + step.years / years * step.rates.sediment,
                metamorphic: sum.metamorphic + step.years / years * step.rates.metamorphic,
            });
        self.smoothed = Some(mean);
        self.phi = Some(phi);
        self.slab_carbon = slab_carbon;
        self.calibration = VecDeque::new();
        self.state.phi = phi;
    }

    /// Takes one step: degassing from the tectonics of the step, weathering from its climate
    /// and erosion, and the new CO₂ by the implicit step. Returns the greenhouse forcing for the
    /// next climate update: the new CO₂'s if the cycle is active, else the current.
    ///
    /// Until φ is set ([`Self::end_hold`]), the CO₂ stays and the step is recorded for the
    /// calibration.
    pub fn step(&mut self, input: &StepInput) -> f64 {
        let previous = self.previous.unwrap_or(input.drivers);
        self.previous = Some(input.drivers);
        self.years += input.years;
        let rate = |now: f64, before: f64| (now - before).max(0.0) / input.years;
        let drivers = input.drivers;
        // LIPs degas in the step they erupt (a pulse, not smoothed).
        let lip_crust = rate(drivers.lip_crust, previous.lip_crust);
        let raw = Rates {
            ridge: rate(drivers.new_sea_floor, previous.new_sea_floor),
            subducted: rate(drivers.subducted_crust, previous.subducted_crust),
            sediment: rate(drivers.subducted_sediment, previous.subducted_sediment),
            metamorphic: rate(drivers.metamorphic_sediment, previous.metamorphic_sediment),
        };
        let cells = self.weathering_cells(input.land);
        let current_forcing = self.forcing;
        let x_now = math::ln(self.ratio);
        let params = &self.params;
        // Degassing without the slab's seafloor carbon (which depends on φ).
        let degassing_of = |rates: &Rates| Sources {
            ridges: params.ridge_carbon * rates.ridge,
            arcs: params.wedge_carbon * rates.subducted
                + params.slab_sediment_carbon * rates.sediment,
            plumes: params.plume_carbon * drivers.plume_flux + params.lip_carbon * lip_crust,
            metamorphic: params.metamorphic_carbon * rates.metamorphic,
        };

        self.tectonic_degassing = degassing_of(&raw).total();
        if self.phi.is_none() {
            // The last `calibration_span` of steps (at least this one).
            self.calibration.push_back(CalibrationStep {
                years: input.years,
                cells: cells.clone(),
                rates: raw,
                deep_water_temperature: input.deep_water_temperature,
                degassing: self.tectonic_degassing,
            });
            let span = self.calibration_span * 1e6;
            let mut recorded: f64 = self.calibration.iter().map(|step| step.years).sum();
            while self.calibration.len() > 1
                && let Some(oldest) = self.calibration.front()
                && recorded - oldest.years >= span - 1.0
            {
                recorded -= oldest.years;
                self.calibration.pop_front();
            }
        }
        let params = &self.params;
        let phi = self.phi.unwrap_or(1.0);
        // Smoothed over a few steps once φ is set (the tectonic rates jump with the grid).
        let weight = if params.degassing_smoothing > 0.0 {
            1.0 - math::exp(-input.years / (params.degassing_smoothing * 1e6))
        } else {
            1.0
        };
        let rates = match self.smoothed {
            Some(old) => old.towards(raw, weight),
            None => raw,
        };
        if self.smoothed.is_some() {
            self.smoothed = Some(rates);
        }
        let ridge_crust = rates.ridge;

        // The seafloor carbon returning at arcs: the slab's carbon, before it is known that
        // of the step at φ = 1 (only logged).
        let uptake_now = seafloor_uptake(params, phi, x_now, input.deep_water_temperature, 0.0);
        let slab_carbon = if self.slab_carbon.is_nan() {
            uptake_now
        } else {
            self.slab_carbon
        };
        let mut sources = degassing_of(&rates);
        sources.arcs += params.slab_return * rates.subducted * slab_carbon;
        let degassing = sources.total();

        let sinks_at = |x: f64, warming: f64| {
            sinks(
                params,
                &cells,
                ridge_crust,
                input.deep_water_temperature,
                phi,
                x,
                warming,
            )
        };
        // The equilibrium temperature change the forcing of `x` would bring: the change of the
        // forcing and the sunlight, and the warming the last update has not reached yet.
        let warming = |x: f64| {
            params.climate_sensitivity
                * (co2_forcing(math::exp(x)) - current_forcing
                    + input.sunlight_change
                    + input.toa_imbalance)
        };
        let active = self.active();
        let x_new = if active {
            let ocean_mass = input.ocean_mass;
            let area = input.planet_area;
            let target = self.carbon + input.years * degassing;
            let excess = |x: f64| {
                exogenic_carbon(params, math::exp(x), ocean_mass, area)
                    + input.years * sinks_at(x, warming(x)).0.total()
                    - target
            };
            solve_increasing(excess, x_now)
        } else {
            x_now
        };
        // Inactive, the sinks are those of the step's climate.
        let predicted = if active { warming(x_new) } else { 0.0 };
        let (sinks, intensity) = sinks_at(x_new, predicted);
        if active {
            // The carbon balance of the step: what the implicit step conserves.
            self.carbon += input.years * (degassing - sinks.total());
            self.ratio = math::exp(x_new);
        }
        if self.phi.is_some() && params.slab_age > 0.0 && ridge_crust > 0.0 {
            let uptake = sinks.seafloor / ridge_crust;
            let weight = 1.0 - math::exp(-input.years / (params.slab_age * 1e6));
            self.slab_carbon += weight * (uptake - self.slab_carbon);
        }
        if active {
            self.forcing = co2_forcing(self.ratio);
        }
        let forcing = self.forcing;
        self.state = CarbonState {
            ratio: self.ratio,
            pressure: self.ratio * REFERENCE_PRESSURE,
            forcing,
            carbon: self.carbon,
            atmosphere: atmospheric_carbon(self.ratio, input.planet_area),
            sources,
            sinks,
            phi: self.phi.unwrap_or(f64::NAN),
            predicted_warming: predicted,
            deep_water_temperature: input.deep_water_temperature,
            weathering_intensity: intensity,
            new_crust: ridge_crust,
            arc_plume_mantle_degassing: params.wedge_carbon * rates.subducted
                + params.plume_carbon * drivers.plume_flux
                + params.lip_carbon * lip_crust,
        };
        forcing
    }

    /// φ from the calibration window: the sinks of its steps, less the seafloor carbon that
    /// returns at arcs, balance its degassing at the current CO₂ and the steps' climates. Also
    /// returns the slab's carbon (mol/kg): the window's mean seafloor uptake at that φ.
    fn calibrate(&self, x: f64) -> (f64, f64) {
        let params = &self.params;
        let steps = &self.calibration;
        let degassing: f64 = steps.iter().map(|step| step.years * step.degassing).sum();
        let net_sinks = |phi: f64| {
            steps
                .iter()
                .map(|step| {
                    let (sinks, _) = sinks(
                        params,
                        &step.cells,
                        step.rates.ridge,
                        step.deep_water_temperature,
                        phi,
                        x,
                        0.0,
                    );
                    let uptake = seafloor_uptake(params, phi, x, step.deep_water_temperature, 0.0);
                    step.years
                        * (sinks.total() - params.slab_return * step.rates.subducted * uptake)
                })
                .sum::<f64>()
        };
        let phi = calibrate(net_sinks, degassing, params.phi_min, params.phi_max);
        let ridge: f64 = steps.iter().map(|s| s.years * s.rates.ridge).sum();
        let slab_carbon = if ridge > 0.0 {
            steps
                .iter()
                .map(|s| {
                    s.years
                        * s.rates.ridge
                        * seafloor_uptake(params, phi, x, s.deep_water_temperature, 0.0)
                })
                .sum::<f64>()
                / ridge
        } else {
            seafloor_uptake(params, phi, x, params.seafloor_reference_temperature, 0.0)
        };
        (phi, slab_carbon)
    }

    fn weathering_cells(&self, land: &[LandCell]) -> Vec<WeatheringCell> {
        let params = &self.params;
        land.iter()
            .filter(|cell| cell.ice < 1.0)
            .map(|cell| {
                let mafic = cell.mafic.clamp(0.0, 1.0);
                let cations =
                    params.felsic_cations + mafic * (params.basalt_cations - params.felsic_cations);
                let reactivity = 1.0 + mafic * (params.basalt_reactivity - 1.0);
                let supply = cell.bedrock_erosion * cell.bedrock_density * cations
                    + params.sediment_weatherable
                        * cell.sediment_erosion
                        * crate::isostasy::SEDIMENT_ROCK_DENSITY
                        * params.felsic_cations;
                let kinetic = params.land_weathering
                    * params.plant_factor
                    * reactivity
                    * math::powf(cell.runoff.max(0.0), params.runoff_exponent)
                    * (1.0 - cell.ice);
                WeatheringCell {
                    supply,
                    kinetic,
                    temperature: cell.temperature + ZERO_CELSIUS,
                    area: cell.area,
                }
            })
            .filter(|cell| cell.supply > 0.0 && cell.kinetic > 0.0)
            .collect()
    }
}

/// Carbon (mol/kg) that new oceanic crust takes up at the CO₂ `e^x`, the bottom water
/// temperature `deep` (°C) plus `warming` (K) and the scale `phi`.
fn seafloor_uptake(params: &CarbonParams, phi: f64, x: f64, deep: f64, warming: f64) -> f64 {
    let deep = (deep + warming).max(SEA_WATER_FREEZING) + ZERO_CELSIUS;
    let deep_reference = params.seafloor_reference_temperature + ZERO_CELSIUS;
    let kinetic = phi
        * params.seafloor_carbon
        * math::exp(params.seafloor_co2_exponent * x)
        * math::exp(
            params.seafloor_activation_energy / GAS_CONSTANT * (1.0 / deep_reference - 1.0 / deep),
        );
    let max = params.seafloor_max_carbon;
    if max > 0.0 {
        -max * math::exp_m1(-kinetic / max)
    } else {
        0.0
    }
}

/// The sinks (mol/yr) at the CO₂ `e^x` with a predicted temperature change `warming` (K), with
/// the share of the land's supply limit that is used.
fn sinks(
    params: &CarbonParams,
    cells: &[WeatheringCell],
    ridge_crust: f64,
    deep_water_temperature: f64,
    phi: f64,
    x: f64,
    warming: f64,
) -> (Sinks, f64) {
    let co2 = params.land_co2_factor(math::exp(x));
    let runoff = math::exp(params.runoff_exponent * params.runoff_sensitivity * warming);
    let reference = params.reference_temperature + ZERO_CELSIUS;
    let activation = params.activation_energy / GAS_CONSTANT;
    let mut land = 0.0;
    let mut supply = 0.0;
    for cell in cells {
        let arrhenius =
            math::exp(activation * (1.0 / reference - 1.0 / (cell.temperature + warming)));
        let kinetic = phi * cell.kinetic * runoff * arrhenius * co2;
        land -= cell.area * cell.supply * math::exp_m1(-kinetic / cell.supply);
        supply += cell.area * cell.supply;
    }
    let seafloor = ridge_crust * seafloor_uptake(params, phi, x, deep_water_temperature, warming);
    let intensity = if supply > 0.0 { land / supply } else { 0.0 };
    (Sinks { land, seafloor }, intensity)
}

/// The scale φ at which `sinks(φ)` (increasing) equals `target`, by bisection on ln φ, clamped
/// to `min..=max` with a warning that names the balancing φ.
fn calibrate(sinks: impl Fn(f64) -> f64, target: f64, min: f64, max: f64) -> f64 {
    let balancing = balancing_phi(sinks, target);
    let phi = balancing.clamp(min, max);
    if phi != balancing {
        warn!(
            calibrated = balancing,
            clamped = phi,
            "the calibrated φ lies outside its plausible range and is clamped: the sinks will not \
             balance the sources at the start's CO₂, which then drifts after the calibration"
        );
    }
    phi
}

/// The φ at which `sinks(φ)` (increasing) equals `target`, by bisection on ln φ within
/// 10^±6; that bound if there is none.
fn balancing_phi(sinks: impl Fn(f64) -> f64, target: f64) -> f64 {
    let (mut low, mut high) = (-math::ln(1e6), math::ln(1e6));
    if sinks(math::exp(high)) < target {
        return math::exp(high);
    }
    if sinks(math::exp(low)) > target {
        return math::exp(low);
    }
    for _ in 0..100 {
        let middle = 0.5 * (low + high);
        if sinks(math::exp(middle)) < target {
            low = middle;
        } else {
            high = middle;
        }
        if high - low < 1e-12 {
            break;
        }
    }
    math::exp(0.5 * (low + high))
}

/// The root of an increasing function, starting the bracket at `start`: bisection to the
/// precision of f64.
fn solve_increasing(f: impl Fn(f64) -> f64, start: f64) -> f64 {
    let at_start = f(start);
    if at_start == 0.0 {
        return start;
    }
    // Expand the bracket away from the start in the direction of the root.
    let direction = if at_start > 0.0 { -1.0 } else { 1.0 };
    let mut step = 0.5;
    let mut near = start;
    let mut far = start + direction * step;
    let mut expansions = 0;
    while (f(far) > 0.0) == (at_start > 0.0) {
        near = far;
        step *= 2.0;
        far = start + direction * step;
        expansions += 1;
        if expansions > 60 {
            return far;
        }
    }
    let (mut low, mut high) = if direction > 0.0 {
        (near, far)
    } else {
        (far, near)
    };
    for _ in 0..200 {
        let middle = 0.5 * (low + high);
        if middle <= low || middle >= high {
            break;
        }
        if f(middle) > 0.0 {
            high = middle;
        } else {
            low = middle;
        }
    }
    0.5 * (low + high)
}

/// Carbon (mol) in the atmosphere at the CO₂ ratio `ratio` on a planet of `area` m².
fn atmospheric_carbon(ratio: f64, area: f64) -> f64 {
    ratio * REFERENCE_PRESSURE * 1e5 * area / (GRAVITY * AIR_MOLAR_MASS)
}

/// Exogenic carbon (mol) of the atmosphere and the ocean at the CO₂ ratio `ratio`.
pub fn exogenic_carbon(params: &CarbonParams, ratio: f64, ocean_mass: f64, area: f64) -> f64 {
    let ocean = ocean_mass
        * (params.ocean_dic * math::cbrt(ratio)
            + params.co2_solubility * ratio * REFERENCE_PRESSURE);
    atmospheric_carbon(ratio, area) + ocean
}

/// Mass of an ocean of `volume` m³ in kg.
pub fn ocean_mass(volume: f64) -> f64 {
    volume * SEA_WATER_DENSITY
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::{ZERO_CO2_FORCING, zero_co2_forcing_ratio};

    const AREA: f64 = 5.1e14;
    const OCEAN: f64 = 1.37e21;

    #[test]
    fn todays_reservoirs() {
        let params = CarbonParams::default();
        let atmosphere = atmospheric_carbon(1.0, AREA);
        // 600 GtC.
        assert!(
            (atmosphere * 12e-3 / 1e12 - 600.0).abs() < 30.0,
            "{atmosphere}"
        );
        let total = exogenic_carbon(&params, 1.0, OCEAN, AREA);
        // ~38 000 GtC of the ocean.
        assert!((2.5e18..4.0e18).contains(&total), "{total}");
    }

    fn cell(temperature: f64) -> LandCell {
        LandCell {
            area: 1e12,
            temperature,
            runoff: 0.3,
            ice: 0.0,
            bedrock_erosion: 2e-5,
            sediment_erosion: 0.0,
            bedrock_density: 2800.0,
            mafic: 0.2,
        }
    }

    fn input(land: &[LandCell], drivers: TectonicDrivers, toa_imbalance: f64) -> StepInput<'_> {
        StepInput {
            years: 1e6,
            drivers,
            land,
            deep_water_temperature: 2.0,
            ocean_mass: OCEAN,
            planet_area: AREA,
            sunlight_change: 0.0,
            toa_imbalance,
        }
    }

    /// Cumulative drivers after `step` steps of `rate` times Earth's spreading.
    fn drivers(step: f64, rate: f64) -> TectonicDrivers {
        TectonicDrivers {
            new_sea_floor: step * 65e18 * rate,
            subducted_crust: step * 65e18 * rate,
            subducted_sediment: step * 2e18,
            metamorphic_sediment: step * 1e18,
            lip_crust: 0.0,
            plume_flux: 0.03,
        }
    }

    /// A LIP's crust degasses `lip_carbon` per kg into the plume source in its step only; the
    /// other sources do not change.
    #[test]
    fn lip_crust_degasses_once_into_the_plume_source() {
        let land = vec![cell(15.0); 10];
        let params = CarbonParams {
            degassing_smoothing: 0.0,
            ..CarbonParams::default()
        };
        let lip_carbon = params.lip_carbon;
        let mut plain = CarbonCycle::new(params.clone(), 20.0, OCEAN, AREA);
        let mut with_lip = CarbonCycle::new(params, 20.0, OCEAN, AREA);
        plain.start(drivers(0.0, 1.0));
        with_lip.start(drivers(0.0, 1.0));
        // 10⁶ km³ of basalt (~3 × 10¹⁸ kg) in the first step.
        let lip = 2.95e18;
        for step in 1..=2 {
            let base = drivers(step as f64, 1.0);
            plain.step(&input(&land, base, 0.0));
            with_lip.step(&input(
                &land,
                TectonicDrivers {
                    lip_crust: lip,
                    ..base
                },
                0.0,
            ));
            let (a, b) = (plain.state().sources, with_lip.state().sources);
            let expected = if step == 1 {
                lip_carbon * lip / 1e6
            } else {
                0.0
            };
            assert!(
                (b.plumes - a.plumes - expected).abs() <= 1e-9 * a.plumes,
                "{step}"
            );
            assert_eq!(
                (a.ridges, a.arcs, a.metamorphic),
                (b.ridges, b.arcs, b.metamorphic)
            );
        }
    }

    /// The CO₂ stays through the hold, which keeps the last `calibration_span` of steps; after
    /// it the sinks balance the sources; the implicit step conserves carbon; more degassing
    /// raises the CO₂ to a new balance without overshooting.
    #[test]
    fn calibrated_start_conservation_and_response() {
        let land: Vec<LandCell> = (0..100).map(|i| cell(i as f64 * 0.3)).collect();
        let forcing = 20.0;
        let params = CarbonParams {
            degassing_smoothing: 0.0,
            ..CarbonParams::default()
        };
        let span = DEFAULT_CALIBRATION_SPAN as usize;
        let held = 50;
        let mut cycle = CarbonCycle::new(params, forcing, OCEAN, AREA);
        cycle.start(drivers(0.0, 1.0));
        assert!(cycle.holds_co2());
        for step in 1..=held {
            // Exactly the start's while held, however the degassing changes.
            let rate = if step < 10 { 0.5 } else { 1.0 };
            let next = cycle.step(&input(&land, drivers(step as f64, rate), 0.0));
            assert_eq!(next, forcing);
            assert!(cycle.calibration.len() <= span);
        }
        assert_eq!(cycle.calibration.len(), span);
        cycle.end_hold();
        assert!(!cycle.holds_co2());
        // φ balances the last steps of the hold: the next such step keeps the CO₂.
        let window = held + 1;
        let next = cycle.step(&input(&land, drivers(window as f64, 1.0), 0.0));
        assert!((next - forcing).abs() < 1e-6, "{next}");
        let state = *cycle.state();
        assert!(state.phi.is_finite() && state.phi > 0.0);
        assert!((state.sinks.total() / state.sources.total() - 1.0).abs() < 1e-6);

        // Twice the spreading from now on.
        let base = drivers(window as f64, 1.0);
        let mut previous_ratio = state.ratio;
        let mut previous_rise = f64::INFINITY;
        for step in 1..80 {
            let carbon = cycle.state().carbon;
            let drivers = TectonicDrivers {
                new_sea_floor: base.new_sea_floor + step as f64 * 130e18,
                subducted_crust: base.subducted_crust + step as f64 * 130e18,
                ..drivers((window + step) as f64, 1.0)
            };
            cycle.step(&input(&land, drivers, 0.0));
            let state = cycle.state();
            let balance = carbon + 1e6 * (state.sources.total() - state.sinks.total());
            assert!((state.carbon - balance).abs() <= 1e-9 * carbon);
            let rise = math::ln(state.ratio / previous_ratio);
            assert!(rise >= -1e-12, "no overshoot");
            assert!(rise <= previous_rise + 1e-12, "monotone approach");
            previous_rise = rise;
            previous_ratio = state.ratio;
        }
        let state = cycle.state();
        assert!(state.ratio > 1.3 * co2_ratio_for_forcing(forcing));
        assert!((state.sinks.total() / state.sources.total() - 1.0).abs() < 0.05);
    }

    /// A climate that reaches only part of its equilibrium in an update (the mixed layer), with
    /// another sensitivity than the predicted one: the forcing settles without oscillating.
    #[test]
    fn lagged_climate_does_not_oscillate() {
        for (sensitivity, response) in [(0.6, 0.5), (1.0, 0.5), (0.8, 0.3)] {
            let params = CarbonParams {
                degassing_smoothing: 0.0,
                ..CarbonParams::default()
            };
            let mut cycle = CarbonCycle::new(params, 0.0, OCEAN, AREA);
            cycle.start(drivers(0.0, 1.0));
            let start = 10.0;
            let mut temperature = start;
            let mut forcing = 0.0;
            let mut forcings = Vec::new();
            for step in 1..120 {
                // The degassing doubles after the calibration.
                let rate = if step == 1 { 1.0 } else { 2.0 };
                let cumulative = drivers(1.0, 1.0);
                let now = TectonicDrivers {
                    new_sea_floor: cumulative.new_sea_floor + (step - 1) as f64 * 65e18 * rate,
                    subducted_crust: cumulative.subducted_crust + (step - 1) as f64 * 65e18 * rate,
                    ..drivers(step as f64, 1.0)
                };
                let land: Vec<LandCell> = (0..50).map(|_| cell(temperature)).collect();
                let equilibrium = start + sensitivity * forcing;
                let imbalance = (equilibrium - temperature) / sensitivity;
                forcing = cycle.step(&input(&land, now, imbalance));
                if step == 1 {
                    cycle.end_hold();
                }
                // The climate update: part of the way to the new equilibrium.
                temperature += response * (start + sensitivity * forcing - temperature);
                forcings.push(forcing);
            }
            let changes: Vec<f64> = forcings.windows(2).map(|w| w[1] - w[0]).collect();
            let reversals = changes
                .windows(2)
                .filter(|w| w[0] * w[1] < 0.0 && w[1].abs() > 1e-3)
                .count();
            assert_eq!(reversals, 0, "{sensitivity} {response}: {forcings:?}");
            assert!(forcings.last().unwrap() > &1.0);
        }
    }

    /// Without degassing the weathering draws the CO₂ down towards none, but the implicit step
    /// in ln r never makes it negative (the sinks vanish with the CO₂), and the forcing stops at
    /// that of an atmosphere without CO₂ ([`ZERO_CO2_FORCING`]) instead of diverging.
    #[test]
    fn co2_approaches_zero_without_negative_forcing_beyond_zero_co2() {
        let land = vec![cell(10.0); 50];
        // Balancing at this low CO₂ needs a φ beyond the plausible range: unclamped here, so
        // that the start is continuous.
        let params = CarbonParams {
            degassing_smoothing: 0.0,
            phi_min: 1e-3,
            phi_max: 1e3,
            ..CarbonParams::default()
        };
        let start = ZERO_CO2_FORCING + 5.0;
        let mut cycle = CarbonCycle::new(params, start, OCEAN, AREA);
        cycle.start(drivers(0.0, 1.0));
        let calibrated = cycle.step(&input(&land, drivers(1.0, 1.0), 0.0));
        assert!((calibrated - start).abs() < 1e-6, "{calibrated}");
        cycle.end_hold();
        // The tectonics stops: no new sea floor, subduction or plumes.
        let stopped = TectonicDrivers {
            plume_flux: 0.0,
            ..drivers(1.0, 1.0)
        };
        let mut previous = cycle.state().ratio;
        for _ in 0..40 {
            let carbon = cycle.state().carbon;
            let forcing = cycle.step(&input(&land, stopped, 0.0));
            let state = cycle.state();
            assert!(forcing >= ZERO_CO2_FORCING, "{forcing}");
            assert!(
                state.ratio > 0.0 && state.ratio <= previous,
                "{}",
                state.ratio
            );
            assert!(state.carbon > 0.0 && state.carbon <= carbon);
            assert!(state.sinks.total() >= 0.0);
            let balance = carbon + 1e6 * (state.sources.total() - state.sinks.total());
            assert!((state.carbon - balance).abs() <= 1e-9 * carbon);
            previous = state.ratio;
        }
        assert!(cycle.state().ratio < 0.5 * zero_co2_forcing_ratio());
        assert_eq!(cycle.state().forcing, ZERO_CO2_FORCING);
    }

    /// Switched off, the CO₂ and the forcing stay, the fluxes are still computed.
    #[test]
    fn disabled_keeps_the_forcing() {
        let land = vec![cell(10.0); 10];
        let params = CarbonParams {
            enabled: false,
            ..CarbonParams::default()
        };
        let mut cycle = CarbonCycle::new(params, 30.0, OCEAN, AREA);
        cycle.start(drivers(0.0, 1.0));
        assert!(!cycle.holds_co2());
        for step in 1..40 {
            assert_eq!(
                cycle.step(&input(&land, drivers(step as f64, 1.0 + step as f64), 0.0)),
                30.0
            );
            if step == 20 {
                cycle.end_hold();
            }
        }
        assert!(cycle.state().sources.total() > 0.0);
        assert!(cycle.state().phi.is_finite());
    }

    #[test]
    fn solver_finds_roots() {
        let root = solve_increasing(|x| x - 7.3, 0.0);
        assert!((root - 7.3).abs() < 1e-12);
        let root = solve_increasing(|x| math::exp(x) - 1e-3, 3.0);
        assert!((root - math::ln(1e-3)).abs() < 1e-12);
        let phi = calibrate(|phi| 2.0 * phi, 5.0, 0.3, 3.0);
        assert!((phi - 2.5).abs() < 1e-9);
    }

    #[test]
    fn calibrated_phi_is_clamped() {
        // Balancing would need φ = 8 (or 0.05): clamped to the plausible range.
        assert_eq!(calibrate(|phi| 2.0 * phi, 16.0, 0.3, 3.0), 3.0);
        assert_eq!(calibrate(|phi| 2.0 * phi, 0.1, 0.3, 3.0), 0.3);
        let balancing = balancing_phi(|phi| 2.0 * phi, 16.0);
        assert!((balancing - 8.0).abs() < 1e-9);
    }
}
