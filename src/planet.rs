//! The planet: tectonics coupled with the water cycle and erosion.

use crate::math;
use std::time::Duration;

use web_time::Instant;

use tracing::{debug, debug_span, info};

use crate::carbon::{CarbonCycle, CarbonParams, CarbonState, LandCell, StepInput};
use crate::climate::{Climate, ClimateParams, WaterCycle};
use crate::erosion::{
    CrustClass, ErosionParams, Hydrology, RegolithStats, Surface, SurfaceWater, WaveErosion, erode,
};
use crate::escape::{Escape, EscapeParams, EscapeState};
use crate::ramp_up::{RampUp, RampUpParams, RampUpStatus};
use crate::tectonics::{
    CarbonateExchange, CohortMasses, CoverBudget, CoverMasses, Crust, Simulation, Statistics,
    TectonicStep,
};
use crate::temperature::{TemperatureFields, TemperatureModel, TemperatureParams};

pub struct Planet {
    tectonics: Simulation,
    climate: Climate,
    temperature: TemperatureModel,
    /// The carbon cycle, which sets the greenhouse forcing (#154).
    carbon: CarbonCycle,
    /// The ramp-up phase, during which the carbon cycle holds the CO₂ (#213).
    ramp_up: RampUp,
    /// Cells poleward of [`CarbonParams::deep_water_latitude`], whose ocean sets the bottom
    /// water temperature.
    deep_water_cells: Vec<bool>,
    /// The loss of water to space (#210).
    escape: Escape,
    /// Cells within [`EscapeParams::tropics_latitude`], whose ocean sets the cold trap.
    tropical_cells: Vec<bool>,
    /// `None` disables erosion.
    erosion: Option<ErosionParams>,
    planet_radius: f64,
    /// Precipitation per cell in m/yr.
    precipitation: Vec<f64>,
    /// Water discharge per cell in m³/yr; all zero while erosion is disabled.
    discharge: Vec<f64>,
    /// Where the water of every cell went in the last step, and the lakes (for the view).
    water: SurfaceWater,
    /// Sediment delivered to the oceans in the last step, in m³/yr.
    sediment_flux: f64,
    /// Lakes and endorheic basins in the last step.
    hydrology: Hydrology,
    /// Sediment piled above the delta top in overfilled basins so far (thickness over one cell,
    /// summed).
    sediment_overfill: f64,
    /// Sediment built into terminal fans and prograding deltas so far (thickness over one cell,
    /// summed).
    terminal_fans: f64,
    /// Sediment deposited in the ocean per zone in the last step, in m³/yr.
    sediment_deposition: [f64; 4],
    /// Sediment moved by waves so far (rock-equivalent thickness over one cell, summed).
    wave_erosion: WaveErosion,
    /// Sediment delivered to the oceans so far (m³).
    sediment_delivered: f64,
    /// Sediment that surface processes removed from and added to cells of crust of continental
    /// origin so far (rock-equivalent thickness over one cell, summed; net per cell and step).
    cover_turnover: CoverTurnover,
    /// Where the material eroded from continental crust went so far (Gt, see [`CoverExport`]).
    cover_export: CoverExport,
    /// When to start tracking the continental cover as a cohort (Myr), and once started, the
    /// time, the cohort's initial mass (Gt) and the budgets at the start.
    cover_cohort_time: f64,
    cover_cohort: Option<(f64, f64, CoverBudget, CoverExport)>,
    /// Changes of the area of closed depressions below sea level between steps, summed (cells).
    inland_change: f64,
    /// Cells that the last erosion step saw as ocean on the sample surface but not on the
    /// per-cell surface, or the other way round (#112 b; zero without it).
    ocean_mismatch: usize,
    /// The regolith of the last step (#197): per cell, the regolith at the bottom of the
    /// sediment (rock-equivalent m), the fraction of the step the rock was bare, the production
    /// scale `P₀` and the bedrock lowering (m/yr), and the step's statistics.
    regolith: RegolithFields,
    /// The carbonate precipitated (#200).
    carbonate: CarbonateFields,
}

/// A step of the planet in progress (see [`Planet::begin_step`]): which part comes next, and what
/// the later parts need from the earlier ones.
pub struct StepInProgress {
    dt: f64,
    part: StepPart,
    span: tracing::Span,
    /// Time spent in the tectonics, the climate, the erosion and the carbon cycle, for the debug
    /// log.
    times: [Duration; 4],
}

/// The next part of a step.
enum StepPart {
    /// The plate velocities, the deformation and the number of substeps.
    TectonicsStart,
    /// The next tectonic substep.
    Substeps(TectonicStep),
    /// Crustal flow, plate events, mantle and sea level.
    TectonicsEnd(TectonicStep),
    /// The ocean of the new topography and the temperature.
    Temperature,
    /// The water cycle on the climate's view of the surface.
    WaterCycle {
        elevation: Vec<f64>,
        seeds: Vec<usize>,
        view: Vec<f64>,
    },
    /// The erosion (or only the ocean, without erosion) and the carbon cycle.
    Erosion {
        elevation: Vec<f64>,
        seeds: Vec<usize>,
        view: Vec<f64>,
    },
    Done,
}

impl StepPart {
    /// For the log.
    fn name(&self) -> &'static str {
        match self {
            StepPart::TectonicsStart => "tectonics start",
            StepPart::Substeps(_) => "tectonic substep",
            StepPart::TectonicsEnd(_) => "tectonics end",
            StepPart::Temperature => "temperature",
            StepPart::WaterCycle { .. } => "water cycle",
            StepPart::Erosion { .. } => "erosion",
            StepPart::Done => "done",
        }
    }
}

/// The continental cover of [`CoverCohort::start`] and how much of it is left (Gt), per tracer
/// (`[well mixed, youngest first]`, see [`crate::tectonics::Cohort`]).
#[derive(Clone, Copy, Debug)]
pub struct CoverCohort {
    pub start: f64,
    pub initial: f64,
    /// Left as sediment on crust whose basement is of continental origin, and anywhere.
    pub left: CohortMasses,
    /// The budget of the cover and the cohort since the start (Gt, see [`CoverBudget`]).
    pub budget: CoverBudget,
    /// Where the material eroded from continental crust went since the start, with the cohort's
    /// share of it (Gt, see [`CoverExport`]).
    pub export: CoverExport,
}

/// Where the material eroded from crust of continental origin went (#35), as Gt summed over the
/// erosion steps. Erosion and deposition are net per cell and step (a lower bound of the gross
/// turnover), and a step's load is not traced from cell to cell, so the destinations are those of
/// all continental denudation of the step together.
#[derive(Clone, Copy, Debug, Default)]
pub struct CoverExport {
    /// Denudation of continental crust: sediment removed from its cells plus bedrock eroded on
    /// them.
    pub denudation: f64,
    /// The sediment part of [`Self::denudation`]: reworked cover.
    pub reworked: f64,
    /// Where it went ([`EXPORT_DESTINATIONS`]), by the surface at the start of the step: deposits
    /// on continental crust above sea level outside closed depressions, in closed depressions
    /// (lakes, endorheic and dry basins, also those the step fills), and
    /// in the ocean (flooded continental crust: shelves, epicontinental seas); and the net gain of
    /// all other crust (the ocean basins: transitional and oceanic crust, including the little
    /// land on it). They sum to [`Self::denudation`] because erosion conserves mass; the net
    /// gain is negative where non-continental land sheds more onto continental crust than it
    /// receives.
    pub to: [f64; 4],
    /// The cohort removed by surface processes per tracer (`[well mixed, youngest first]`), split
    /// per step in proportion to the step's deposits ([`Self::to`], the net gain counted where
    /// positive).
    pub cohort_to: [[f64; 4]; 2],
}

/// Names of the destinations of [`CoverExport::to`].
pub const EXPORT_DESTINATIONS: [&str; 4] = [
    "continental land",
    "closed depressions",
    "flooded continental crust",
    "ocean basins (net)",
];

impl CoverExport {
    /// The change from `earlier` to this.
    pub fn since(&self, earlier: &Self) -> Self {
        let mut to = self.to;
        let mut cohort_to = self.cohort_to;
        for (i, to) in to.iter_mut().enumerate() {
            *to -= earlier.to[i];
            for tracer in 0..2 {
                cohort_to[tracer][i] -= earlier.cohort_to[tracer][i];
            }
        }
        Self {
            denudation: self.denudation - earlier.denudation,
            reworked: self.reworked - earlier.reworked,
            to,
            cohort_to,
        }
    }

    /// Adds the export of one erosion step: per cell its class, its change of sediment and of
    /// bedrock (as mass, kg/m²) and whether it was ocean or in a closed depression; `to_gt`
    /// converts summed areal masses to Gt.
    fn add_step(
        &mut self,
        cells: impl Iterator<Item = (bool, f64, f64, bool, bool)>,
        to_gt: f64,
    ) -> [f64; 4] {
        let (mut denudation, mut reworked, mut to) = (0.0, 0.0, [0.0; 4]);
        for (continental, sediment, bedrock, ocean, depression) in cells {
            if continental {
                denudation += (-sediment).max(0.0) + (-bedrock).max(0.0);
                reworked += (-sediment).max(0.0);
                let destination = if ocean {
                    2
                } else if depression {
                    1
                } else {
                    0
                };
                to[destination] += sediment.max(0.0) + bedrock.max(0.0);
            } else {
                to[3] += sediment + bedrock;
            }
        }
        self.denudation += denudation * to_gt;
        self.reworked += reworked * to_gt;
        for (sum, to) in self.to.iter_mut().zip(to) {
            *sum += to * to_gt;
        }
        to
    }

    /// Splits the cohort removed in a step (`removed`, Gt per tracer) over the step's
    /// destinations `to` (positive parts; all on continental land if there were none).
    fn add_cohort(&mut self, removed: [f64; 2], to: [f64; 4]) {
        let weights = to.map(|to| to.max(0.0));
        let total: f64 = weights.iter().sum();
        for (tracer, removed) in removed.into_iter().enumerate() {
            for (destination, weight) in weights.iter().enumerate() {
                self.cohort_to[tracer][destination] += if total > 0.0 {
                    removed * weight / total
                } else if destination == 0 {
                    removed
                } else {
                    0.0
                };
            }
        }
    }
}

/// Gross sediment turnover on crust of continental origin (the continental sedimentary cover)
/// by surface processes: what left and what arrived at its cells, net per cell and step.
#[derive(Clone, Copy, Debug, Default)]
pub struct CoverTurnover {
    pub eroded: f64,
    pub deposited: f64,
}

/// Statistics of the planet, extending the tectonic statistics.
/// The regolith of the last erosion step, per cell (#197).
#[derive(Clone, Debug, Default)]
pub struct RegolithFields {
    /// Regolith at the bottom of the sediment (rock-equivalent m).
    pub thickness: Vec<f64>,
    /// Fraction of the step the rock was bare.
    pub bare: Vec<f64>,
    /// Production on bare rock `P₀` (m/yr of bedrock).
    pub production: Vec<f64>,
    /// Bedrock lowered by erosion and weathering (m/yr).
    pub bedrock_erosion: Vec<f64>,
    pub stats: RegolithStats,
}

/// Carbonate precipitated so far (#200), in kg (cumulative, for rates between snapshots).
#[derive(Clone, Copy, Debug, Default)]
pub struct CarbonateStats {
    /// Precipitated from the carbon cycle's burial (land weathering) and from dissolved
    /// carbonate the surface processes removed (recycled).
    pub new: f64,
    pub recycled: f64,
    /// Dissolved load taken back from the clastic sediment (cation oxide and recycled
    /// carbonate), and what it could not supply (mass created; expected 0).
    pub clastic_removed: f64,
    pub unmatched: f64,
    /// Carbonate that found no room up to the platform top and was put above it.
    pub overflow: f64,
    /// Precipitated by the depth of the sea floor (< 20, 20–200, 200–1000, 1000–3000,
    /// ≥ 3000 m), by latitude (|φ| < 20°, 20–40°, 40–60°, ≥ 60°), and on crust of continental
    /// origin (shelves and platforms).
    pub by_depth: [f64; 5],
    pub by_latitude: [f64; 4],
    pub on_continental: f64,
    /// Precipitation weighted with the saturation Ω of its sample (Σ mass · Ω).
    pub omega_mass: f64,
}

/// Carbon reservoirs (mol) and cumulative flows since the start (#200): the exogenic carbon,
/// the carbonate in the sediment (all samples), the altered sea floor, the buffers of the
/// carbonate sources, and carbonate locked in crystalline crust (wedges, marble); what went
/// into the mantle (unreturned subducted carbonate and altered crust, and carbonate lost with
/// removed crust) and came out of it (ridges, mantle wedge, plumes). The residual of
/// `reservoirs + to_mantle − mantle_degassing − initial` is the closure error (~0).
#[derive(Clone, Copy, Debug, Default)]
pub struct CarbonPartition {
    pub exogenic: f64,
    pub sediment: f64,
    pub altered_crust: f64,
    pub buffers: f64,
    pub crust: f64,
    pub to_mantle: f64,
    pub lost: f64,
    pub mantle_degassing: f64,
    pub initial: f64,
    pub residual: f64,
    /// Mass (kg) of CO₂ that decarbonation took out of the rock.
    pub decarbonated_co2: f64,
}

/// The carbonate of the last step and so far (#200).
pub struct CarbonateFields {
    /// Carbonate precipitated per cell in the last step (rock-equivalent m/Myr).
    pub deposit: Vec<f64>,
    /// Saturation level `S` of the last step, and the highest Ω of a sample that took carbonate.
    pub level: f64,
    pub max_omega: f64,
    pub stats: CarbonateStats,
}

impl CarbonateFields {
    fn new(cell_count: usize) -> Self {
        Self {
            deposit: vec![0.0; cell_count],
            level: f64::NAN,
            max_omega: f64::NAN,
            stats: CarbonateStats::default(),
        }
    }
}

impl RegolithFields {
    fn new(cell_count: usize) -> Self {
        Self {
            thickness: vec![0.0; cell_count],
            bare: vec![0.0; cell_count],
            production: vec![0.0; cell_count],
            bedrock_erosion: vec![0.0; cell_count],
            stats: RegolithStats::default(),
        }
    }
}

pub struct PlanetStatistics {
    pub tectonics: Statistics,
    /// Mean precipitation over land in m/yr.
    pub mean_land_precipitation: f64,
    /// Mean precipitation over land between 20° and 60° latitude (both hemispheres) in m/yr.
    pub midlatitude_land_precipitation: f64,
    /// Fraction of land with less than 0.25 m/yr of precipitation (arid).
    pub arid_land_fraction: f64,
    /// Global mean precipitation and evaporation in m/yr (equal when the water cycle is closed).
    pub global_precipitation: f64,
    pub global_evaporation: f64,
    /// Lake evaporation that the climate returned to the air (m³/yr): the evaporation of the
    /// lakes of the previous steps, as the solved months saw it.
    pub lake_evaporation_to_air: f64,
    /// Mean runoff (precipitation minus evapotranspiration, which feeds the rivers) of land in
    /// m/yr.
    pub land_runoff: f64,
    /// Latitude of the ITCZ in the solved months, in degrees.
    pub itcz: Vec<f64>,
    /// Negative precipitation of unconverged solves that was set to zero, relative to the total.
    pub clipped_precipitation: f64,
    /// Discharge of the largest river mouth in m³/yr.
    pub largest_river: f64,
    /// Sediment delivered to the oceans in m³/yr.
    pub sediment_flux: f64,
    /// Lakes and endorheic basins in the last erosion step.
    pub hydrology: Hydrology,
    /// Sediment piled above the delta top in overfilled basins so far (thickness over one cell,
    /// summed).
    pub sediment_overfill: f64,
    /// Sediment built into terminal fans and prograding deltas so far (thickness over one cell,
    /// summed).
    pub terminal_fans: f64,
    /// Sediment deposited in the ocean in the last step per [`crate::erosion::Zone`] (shelf,
    /// slope, rise, fan) in m³/yr.
    pub sediment_deposition: [f64; 4],
    /// Sediment mass (Gt) on crust of continental origin, transitional and oceanic crust (by
    /// [`CrustClass`], counting each visible cell).
    pub sediment_stock: [f64; 3],
    /// Area-weighted global annual mean surface temperature in °C.
    pub global_temperature: f64,
    /// Annual mean surface temperature between 10°S and 10°N in °C.
    pub equatorial_temperature: f64,
    /// Annual mean surface temperature poleward of 70° in °C.
    pub polar_temperature: f64,
    /// Mean difference between the warmest and coldest month over land in K.
    pub land_seasonal_range: f64,
    /// Largest northward and southward meridional heat transport (atmosphere and ocean) in PW,
    /// and the ocean's transport across the interfaces nearest to 20°N and 20°S.
    pub heat_transport: [f64; 4],
    /// Largest transport of the wind-driven ocean gyres in Sv.
    pub gyre_transport: f64,
    /// Solar constant of the climate in W/m² (the sun brightens during the run).
    pub solar_constant: f64,
    /// The carbon cycle after the last step (#154): CO₂, forcing, sources and sinks.
    pub carbon: CarbonState,
    /// The escape after the last step (#210); `None` when it is off.
    pub escape: Option<EscapeState>,
    /// Fraction of the surface covered by permanent ice (sea ice, ice sheets).
    pub frozen_fraction: f64,
    /// Where the sediment sits and how much of the sea level it makes up.
    pub sediment_reservoirs: SedimentReservoirs,
    /// Sediment moved by waves so far, in Gt.
    pub wave_erosion: WaveErosion,
    /// Sediment delivered to the oceans by rivers so far, in Gt.
    pub sediment_delivered: f64,
    /// Sediment removed from and added to the continental cover by surface processes so far,
    /// in Gt.
    pub cover_turnover: CoverTurnover,
    /// The tracked cohort of continental cover, once started.
    pub cover_cohort: Option<CoverCohort>,
    /// Where the material eroded from continental crust went so far (Gt).
    pub cover_export: CoverExport,
    /// The budget of the continental cover and the cohort by process so far (Gt), and the
    /// masses now (Gt; see [`crate::tectonics::CoverMasses`]).
    pub cover_budget: CoverBudget,
    pub cover_masses: CoverMasses,
    /// Fraction of the surface that is ocean, and that lies below sea level (the ocean and
    /// closed depressions, see [`crate::sea`]).
    pub ocean_fraction: f64,
    pub below_sea_level_fraction: f64,
    /// Changes of the area of closed depressions below sea level between steps so far, summed
    /// (cells): how much they come and go.
    pub inland_change: f64,
    /// The ocean volume (m³) that the ocean does not hold because the sea level stands at a
    /// pass into a basin it cannot fill (see [`crate::isostasy::SeaLevel::deficit`]).
    pub sea_level_deficit: f64,
}

/// The part of the sea level (m) due to the sediment of groups of cells (see
/// [`Simulation::sediment_sea_level_of`]; the parts are counterfactuals and do not add up
/// exactly), and related shares.
#[derive(Clone, Debug, Default)]
pub struct SedimentReservoirs {
    /// All sediment.
    pub all: f64,
    /// Sediment on crust of continental origin above and below sea level, on transitional and on
    /// oceanic crust (see [`CrustClass`]).
    pub continental_land: f64,
    pub continental_flooded: f64,
    pub transitional: f64,
    pub oceanic: f64,
    /// Sediment below sea level (comparable with Earth's marine sediment).
    pub marine: f64,
    /// Sediment on crust of continental origin with a felsic fraction below 0.8, which formed
    /// during the simulation (filled basins, arcs), and its share of the surface.
    pub filled_basins: f64,
    pub filled_basin_fraction: f64,
    /// Share of the surface that is of continental origin only with its metasediment (see
    /// [`Crust::is_continental_origin_column`]): basins filled on other lithosphere.
    pub metasediment_basin_fraction: f64,
    /// Sediment on sea floor shallower than its storm wave base (see
    /// [`crate::erosion::wave_bases`]).
    pub above_wave_base: f64,
    /// All sediment together with the metasediment formed from it during the simulation (see
    /// [`Crust::metasediment`]).
    pub with_metasediment: f64,
    /// Sediment summed over the cells as the sea-level solve sees them relative to the sediment
    /// of all samples (hidden ones included).
    pub solver_ratio: f64,
    /// Metasediment (Gt) on basement of continental origin and on other basement.
    pub metasediment: (f64, f64),
    /// Share of land lower than 10 m.
    pub low_land_fraction: f64,
    /// Cells between 0 and -200 m (shelves): their share of the surface, their mean sediment
    /// (rock-equivalent m), and the shares of them on oceanic crust and on filled basins.
    pub shelf_fraction: f64,
    pub shelf_sediment: f64,
    pub shelf_on_oceanic: f64,
    pub shelf_on_filled_basins: f64,
}

/// Means over a set of cells of the temperature fields (see [`Planet::lake_climate_contrast`]).
#[derive(Clone, Copy, Debug, Default)]
pub struct SurfaceStats {
    /// Annual mean surface temperature (°C).
    pub annual: f64,
    /// Difference between the warmest and the coldest month (K).
    pub range: f64,
    /// Share of the cells with permanent ice (cover > 0.5).
    pub frozen: f64,
}

/// The temperatures of the same state with lakes as open water and as land (#173): for each set
/// of cells, their number and the statistics with open water and with land.
#[derive(Clone, Copy, Debug, Default)]
pub struct LakeClimateContrast {
    /// All lake cells.
    pub lakes: (usize, SurfaceStats, SurfaceStats),
    /// Cells of lakes of at least [`Self::LARGE_LAKE`] connected cells.
    pub large_lakes: (usize, SurfaceStats, SurfaceStats),
    /// Land next to large lakes.
    pub near: (usize, SurfaceStats, SurfaceStats),
    /// Land two or three cells from large lakes.
    pub around: (usize, SurfaceStats, SurfaceStats),
}

impl LakeClimateContrast {
    pub const LARGE_LAKE: usize = 20;
}

impl Planet {
    pub fn new(
        tectonics: Simulation,
        climate: ClimateParams,
        temperature: TemperatureParams,
        carbon: CarbonParams,
        escape: EscapeParams,
        erosion: Option<ErosionParams>,
        planet_radius: f64,
    ) -> Self {
        let climate = Climate::new(tectonics.mesh(), planet_radius, climate);
        let deep_water_latitude = carbon.deep_water_latitude.to_radians();
        let deep_water_cells = (0..tectonics.mesh().len() as u32)
            .map(|cell| {
                math::asin(tectonics.mesh().vertex(cell).z.clamp(-1.0, 1.0)).abs()
                    >= deep_water_latitude
            })
            .collect();
        let tropics = escape.tropics_latitude.to_radians();
        let tropical_cells = (0..tectonics.mesh().len() as u32)
            .map(|cell| {
                math::asin(tectonics.mesh().vertex(cell).z.clamp(-1.0, 1.0)).abs() <= tropics
            })
            .collect();
        let escape = Escape::new(escape, tectonics.water_volume());
        let mut carbon = CarbonCycle::new(
            carbon,
            temperature.greenhouse_forcing,
            crate::carbon::ocean_mass(tectonics.ocean_volume()),
            4.0 * std::f64::consts::PI * planet_radius * planet_radius,
        );
        carbon.start(tectonics.carbon_drivers());
        let ramp_up = RampUp::new(RampUpParams::default());
        carbon.set_calibration_span(ramp_up.params().span);
        let mut temperature = TemperatureModel::new(tectonics.mesh(), temperature);
        let elevation = tectonics.surface_elevations();
        let seeds = crate::sea::ocean_seeds(
            tectonics.mesh(),
            &elevation,
            -crate::sea::SILL_TOLERANCE,
            tectonics.min_sea_fraction(),
        );
        let open = open_sills(&erosion);
        let view = climate_elevations(tectonics.mesh(), &elevation, &seeds, open, None);
        temperature.update(&view, None, None, tectonics.time());
        let cell_count = tectonics.mesh().len();
        let water = SurfaceWater::ocean_only(crate::sea::ocean_from(
            tectonics.mesh(),
            &elevation,
            seeds.iter().copied(),
            open,
        ));
        let mut planet = Self {
            tectonics,
            climate,
            temperature,
            carbon,
            ramp_up,
            deep_water_cells,
            escape,
            tropical_cells,
            erosion,
            planet_radius,
            precipitation: vec![0.0; cell_count],
            discharge: vec![0.0; cell_count],
            water,
            sediment_flux: 0.0,
            hydrology: Hydrology::default(),
            sediment_overfill: 0.0,
            terminal_fans: 0.0,
            sediment_deposition: [0.0; 4],
            wave_erosion: WaveErosion::default(),
            sediment_delivered: 0.0,
            cover_turnover: CoverTurnover::default(),
            cover_export: CoverExport::default(),
            cover_cohort_time: f64::INFINITY,
            cover_cohort: None,
            inland_change: 0.0,
            ocean_mismatch: 0,
            regolith: RegolithFields::new(cell_count),
            carbonate: CarbonateFields::new(cell_count),
        };
        planet.update_water_cycle(&view);
        planet
    }

    /// Starts tracking the continental cover as a cohort at the simulated time `time` (Myr), to
    /// measure how long it is preserved (see [`Simulation::start_cover_cohort`]).
    pub fn track_cover_cohort_from(&mut self, time: f64) {
        self.cover_cohort_time = time;
    }

    /// Replaces the parameters of the ramp-up phase; before the first step.
    pub fn set_ramp_up(&mut self, params: RampUpParams) {
        self.carbon.set_calibration_span(params.span);
        self.ramp_up = RampUp::new(params);
    }

    /// Whether the ramp-up phase runs or when it ended (#213).
    pub fn ramp_up_status(&self) -> RampUpStatus {
        self.ramp_up.status()
    }

    /// Whether the carbon cycle holds the CO₂ of the start (during the ramp-up phase).
    /// The parameters of the carbon cycle.
    pub fn carbon_params(&self) -> &CarbonParams {
        self.carbon.params()
    }

    pub fn holds_co2(&self) -> bool {
        self.carbon.holds_co2()
    }

    pub fn tectonics(&self) -> &Simulation {
        &self.tectonics
    }

    pub fn temperature(&self) -> &TemperatureFields {
        self.temperature.fields()
    }

    /// The regolith of the last erosion step (#197).
    pub fn regolith(&self) -> &RegolithFields {
        &self.regolith
    }

    /// The carbonate precipitated in the last step and so far (#200).
    pub fn carbonate(&self) -> &CarbonateFields {
        &self.carbonate
    }

    /// The carbon reservoirs and their closure (#200, see `notes/issue-200/plan.md` §4).
    pub fn carbon_partition(&self) -> CarbonPartition {
        use crate::carbonate::{CARBONATE_MOLAR_MASS as M, CO2_MOLAR_MASS};
        let state = self.carbon.state();
        let budget = *self.carbon.budget();
        let inventory = self.tectonics.carbonate_inventory();
        let (flows, [subducted, accreted, stacked, collided]) = self.tectonics.carbonate_flows();
        // In the crust: accreted, and metamorphosed or stacked without decarbonating.
        let crust = (accreted + stacked - collided - flows.metamorphosed - flows.decarbonated) / M;
        // Carbonate that left the sediment by no counted process (lost with removed crust).
        let counted = flows.initial + flows.deposited + flows.surface + flows.metamorphosed
            - subducted
            - accreted
            - stacked;
        let lost = (counted - inventory) / M;
        let partition = CarbonPartition {
            exogenic: state.carbon,
            sediment: inventory / M,
            altered_crust: budget.altered_crust,
            buffers: budget.buffers[0] + budget.buffers[1],
            crust,
            to_mantle: budget.carbonate_to_mantle
                + budget.altered_crust_to_mantle
                + budget.before_start
                + lost,
            lost,
            mantle_degassing: budget.mantle_degassing,
            initial: budget.initial + flows.initial / M,
            residual: 0.0,
            decarbonated_co2: (flows.decarbonated + collided) * CO2_MOLAR_MASS / M,
        };
        CarbonPartition {
            residual: partition.exogenic
                + partition.sediment
                + partition.altered_crust
                + partition.buffers
                + partition.crust
                + partition.to_mantle
                - partition.mantle_degassing
                - partition.initial,
            ..partition
        }
    }

    pub fn precipitation(&self) -> &[f64] {
        &self.precipitation
    }

    pub fn discharge(&self) -> &[f64] {
        &self.discharge
    }

    /// The hydrology of the last erosion step.
    pub fn hydrology(&self) -> &Hydrology {
        &self.hydrology
    }

    /// Surface elevation of a cell relative to sea level (m) with the load of the water that
    /// stands on it (#127): lake floors pressed down by their water, dry closed depressions
    /// below sea level without the ocean's load (see `SurfaceWater::load_correction`). For the
    /// view, the maps and the cell dump; the processes (and the statistics of the snapshot log)
    /// use
    /// [`Simulation::surface_elevation`], which loads every cell below sea level with the ocean.
    pub fn surface_elevation(&self, cell: u32) -> f64 {
        self.tectonics.surface_elevation(cell)
            + self
                .water
                .load_correction
                .get(cell as usize)
                .copied()
                .unwrap_or(0.0)
    }

    /// [`Self::surface_elevation`] of every cell.
    pub fn surface_elevations(&self) -> Vec<f64> {
        (0..self.tectonics.mesh().len() as u32)
            .map(|cell| self.surface_elevation(cell))
            .collect()
    }

    /// Where the water of every cell goes, the ocean and the lakes, as the last erosion step
    /// saw them (only the ocean while erosion is disabled); for the view.
    pub fn surface_water(&self) -> &SurfaceWater {
        &self.water
    }

    /// Cells that the last erosion step saw as ocean on the sample surface but not on the
    /// per-cell surface, or the other way round (#112 b).
    pub fn ocean_mismatch(&self) -> usize {
        self.ocean_mismatch
    }

    /// Land cells whose water ends in an endorheic depression all of whose cells show a single
    /// crust sample (#112; the sample may also be shown outside it), from the last erosion
    /// step's [`SurfaceWater`]. A depression is a tree of depression cells along the receivers;
    /// its root is the cell whose receiver leaves it (none for an endorheic one).
    pub fn endorheic_single_sample_drainage(&self) -> f64 {
        let water = &self.water;
        let cells = water.receiver.len();
        const UNKNOWN: u32 = u32::MAX;
        const OCEAN: u32 = u32::MAX - 1;
        // Follows the receivers from `cell` to the end of its path (`OCEAN` or an endorheic
        // exit), or within depressions to the root of its depression.
        let follow = |memo: &mut Vec<u32>, cell: usize, step: &dyn Fn(usize) -> Option<usize>| {
            let mut path = Vec::new();
            let mut current = cell;
            let end = loop {
                if memo[current] != UNKNOWN {
                    break memo[current];
                }
                match step(current) {
                    Some(next) => {
                        path.push(current);
                        current = next;
                    }
                    None => {
                        let end = if water.ocean[current] {
                            OCEAN
                        } else {
                            current as u32
                        };
                        memo[current] = end;
                        break end;
                    }
                }
            };
            for c in path {
                memo[c] = end;
            }
            end
        };
        let mut terminal = vec![UNKNOWN; cells];
        let downstream = |c: usize| water.receiver[c].map(|r| r as usize);
        let mut root = vec![UNKNOWN; cells];
        let within = |c: usize| {
            water.receiver[c]
                .map(|r| r as usize)
                .filter(|&r| water.depression[r])
        };
        // Per depression root: whether its cells all show one sample.
        let mut sample = vec![None; cells];
        let mut single_sample = vec![true; cells];
        for (cell, _) in water.depression.iter().enumerate().filter(|(_, d)| **d) {
            let r = follow(&mut root, cell, &within) as usize;
            let owner = self.tectonics.owner(cell as u32);
            match sample[r] {
                None => sample[r] = Some(owner),
                Some(o) if o.plate != owner.plate || o.local != owner.local => {
                    single_sample[r] = false;
                }
                Some(_) => {}
            }
        }
        let mut into_sample = 0.0;
        for cell in 0..cells {
            if water.ocean[cell] {
                continue;
            }
            let end = follow(&mut terminal, cell, &downstream);
            if end == OCEAN || !water.depression[end as usize] {
                continue;
            }
            if single_sample[end as usize] {
                into_sample += 1.0;
            }
        }
        into_sample
    }

    /// A fingerprint of the state, the same on every platform for the same state: the time and,
    /// per cell, the surface elevation, crust, owning plate, precipitation, discharge and annual
    /// mean temperature, hashed bit for bit (FNV-1a, all NaNs alike). Logged with each snapshot, so that two runs
    /// (e.g. on the web and with `--web-math`) can be checked for identical results.
    pub fn state_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut add = |value: u64| {
            for byte in value.to_le_bytes() {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        let simulation = &self.tectonics;
        let carbonate = self.carbon.params().carbonate_sediment;
        add(simulation.time().to_bits());
        let elevation = simulation.surface_elevations();
        let temperature = &self.temperature().annual_mean;
        for cell in 0..simulation.mesh().len() {
            let crust = simulation.surface_crust(cell as u32);
            let owner = simulation.owner(cell as u32);
            for value in [
                elevation[cell],
                crust.thickness,
                crust.density,
                crust.birth,
                crust.sediment,
                crust.felsic,
                crust.metasediment,
                self.precipitation[cell],
                self.discharge[cell],
                temperature[cell],
            ] {
                // NaN payloads differ between platforms; all NaNs count as one.
                add(if value.is_nan() { f64::NAN } else { value }.to_bits());
            }
            add(u64::from(owner.plate) << 32 | u64::from(owner.local));
            // The carbonate (#200) only when it is on, so that off the hash stays that of the
            // model without it.
            if carbonate {
                add(crust.cohort.carbonate.to_bits());
            }
        }
        hash
    }

    /// Advances the planet by `dt` Myr: plate motion first, then the climate and one erosion
    /// step. The same as [`Planet::begin_step`] and [`Planet::continue_step`] until it is
    /// complete (which the simulation runs use).
    #[cfg(test)]
    pub fn step(&mut self, dt: f64) {
        let mut step = self.begin_step(dt);
        while !self.continue_step(&mut step) {}
    }

    /// Starts a step of `dt` Myr, to be taken part by part with [`Planet::continue_step`], so
    /// that a caller can stop between the parts (to pause the simulation, #116). Nothing is
    /// computed yet.
    pub fn begin_step(&self, dt: f64) -> StepInProgress {
        StepInProgress {
            dt,
            part: StepPart::TectonicsStart,
            span: debug_span!("step", time = self.tectonics.time() + dt),
            times: [Duration::ZERO; 4],
        }
    }

    /// Takes the next part of `step` and returns whether the step is complete. Between the
    /// parts the planet is in a consistent state, but part-way through the step (its time too):
    /// it can be read (e.g. hashed), but should not be shown as a state of the simulation.
    /// Taking the parts with anything else in between gives the same result as
    /// the whole step at once.
    pub fn continue_step(&mut self, step: &mut StepInProgress) -> bool {
        let span = step.span.clone();
        let _entered = span.enter();
        let start = Instant::now();
        let name = step.part.name();
        let (next, phase) = match std::mem::replace(&mut step.part, StepPart::Done) {
            StepPart::TectonicsStart => (StepPart::Substeps(self.tectonics.begin_step(step.dt)), 0),
            StepPart::Substeps(mut tectonic) => {
                self.tectonics.next_substep(&mut tectonic);
                if tectonic.substeps_left() > 0 {
                    (StepPart::Substeps(tectonic), 0)
                } else {
                    (StepPart::TectonicsEnd(tectonic), 0)
                }
            }
            StepPart::TectonicsEnd(tectonic) => {
                self.tectonics.end_step(tectonic);
                (StepPart::Temperature, 0)
            }
            StepPart::Temperature => {
                let elevation = self.tectonics.surface_elevations();
                // Ice sheets depend on last step's snowfall.
                // The seas of this topography, for the climate and the erosion.
                let seeds = crate::sea::ocean_seeds(
                    self.tectonics.mesh(),
                    &elevation,
                    -crate::sea::SILL_TOLERANCE,
                    self.tectonics.min_sea_fraction(),
                );
                let open = open_sills(&self.erosion);
                // Lakes of the last erosion step, at their water surface if they are open water.
                let lakes = self
                    .temperature
                    .params()
                    .open_water_lakes
                    .then_some(&self.water);
                let view =
                    climate_elevations(self.tectonics.mesh(), &elevation, &seeds, open, lakes);
                self.temperature.update(
                    &view,
                    Some(&self.precipitation),
                    lakes.map(|water| &water.lake_depth[..]),
                    self.tectonics.time(),
                );
                (
                    StepPart::WaterCycle {
                        elevation,
                        seeds,
                        view,
                    },
                    1,
                )
            }
            StepPart::WaterCycle {
                elevation,
                seeds,
                view,
            } => {
                self.update_water_cycle(&view);
                (
                    StepPart::Erosion {
                        elevation,
                        seeds,
                        view,
                    },
                    1,
                )
            }
            StepPart::Erosion {
                elevation,
                seeds,
                view,
            } => {
                let carbon_time = self.erode_step(step.dt, &elevation, &seeds, &view);
                step.times[3] += carbon_time;
                step.times[2] = step.times[2].saturating_sub(carbon_time);
                (StepPart::Done, 2)
            }
            StepPart::Done => (StepPart::Done, 2),
        };
        let elapsed = start.elapsed();
        tracing::trace!(part = name, ?elapsed, "Part of the step");
        step.times[phase] += elapsed;
        step.part = next;
        let done = matches!(step.part, StepPart::Done);
        if done {
            let [tectonics_time, climate_time, erosion_time, carbon_time] = step.times;
            debug!(
                ?tectonics_time,
                ?climate_time,
                ?erosion_time,
                ?carbon_time,
                "Step finished"
            );
        }
        done
    }

    /// The erosion of a step (or only the ocean without erosion), the carbon cycle and the
    /// bookkeeping after them, on the surface `elevation` with the ocean `seeds` of the step's
    /// climate, whose view of the surface is `view`. Returns the time spent in the carbon cycle.
    fn erode_step(
        &mut self,
        dt: f64,
        elevation: &[f64],
        seeds: &[usize],
        view: &[f64],
    ) -> Duration {
        let open = open_sills(&self.erosion);
        // Bedrock and sediment eroded per cell in the step (m/yr), and the fraction of the step
        // its rock was bare, for the weathering.
        let mut eroded = vec![(0.0, 0.0, 0.0, 0.0); elevation.len()];
        // Sediment deposited per cell (rock-equivalent m) and the carbonate the surface
        // processes removed (kg), for the carbonate (#200).
        let mut deposits = Vec::new();
        let mut recycled = 0.0;
        if let Some(params) = &self.erosion {
            let dt_years = dt * 1e6;
            let (plate, class) = self.plates_and_classes();
            // Cells showing one crust sample are one column for the surface processes (#112 b).
            let sample_surface;
            let surface: &[f64] = if params.sample_surface {
                sample_surface = self.tectonics.sample_surface_elevations();
                &sample_surface
            } else {
                elevation
            };
            let bedrock_density = self.tectonics.bedrock_densities();
            let result = erode(
                self.tectonics.mesh(),
                &Surface {
                    elevation: surface,
                    sediment: &self.tectonics.sediment_thicknesses(),
                    bedrock_density: &bedrock_density,
                    bedrock_isostatic_density: &self.tectonics.bedrock_isostatic_densities(),
                    dense_crust: &self.tectonics.dense_crust_thicknesses(),
                    precipitation: &self.precipitation,
                    runoff: &self.climate.water_cycle().runoff,
                    potential_evaporation: &self.climate.water_cycle().lake_potential_evaporation,
                    plate: &plate,
                    class: &class,
                    ocean_volume: self.tectonics.ocean_volume(),
                    ocean_seeds: seeds,
                    water_volume: self.tectonics.water_volume(),
                    previous_lakes: &self
                        .water
                        .lake_depth
                        .iter()
                        .map(|&depth| depth > 0.0)
                        .collect::<Vec<_>>(),
                    sample: &if params.sample_surface {
                        self.tectonics.sample_ids()
                    } else {
                        Vec::new()
                    },
                    temperature: &self.temperature.fields().annual_mean,
                    ice: &self.temperature.fields().ice,
                    mafic: &(0..elevation.len() as u32)
                        .map(|cell| 1.0 - self.tectonics.surface_crust(cell).felsic)
                        .collect::<Vec<_>>(),
                    regolith: &self.regolith.thickness,
                },
                params,
                dt_years,
            );
            for (cell, eroded) in eroded.iter_mut().enumerate() {
                // The regolith was counted as fresh rock when it was made (the bedrock lowering),
                // so its removal is not counted again as eroded sediment.
                let regolith_removed = result.regolith_start[cell] - result.regolith[cell];
                *eroded = (
                    (-result.bedrock_delta[cell]).max(0.0) / dt_years,
                    (-result.sediment_delta[cell] - regolith_removed).max(0.0) / dt_years,
                    result.bare[cell],
                    self.tectonics.surface_crust(cell as u32).cohort.carbonate,
                );
            }
            self.regolith = RegolithFields {
                bedrock_erosion: result
                    .bedrock_delta
                    .iter()
                    .map(|d| (-d).max(0.0) / dt_years)
                    .collect(),
                thickness: result.regolith,
                bare: result.bare,
                production: result.production,
                stats: result.regolith_stats,
            };
            for (delta, class) in result.sediment_delta.iter().zip(&class) {
                if *class == CrustClass::Continental {
                    if *delta < 0.0 {
                        self.cover_turnover.eroded -= delta;
                    } else {
                        self.cover_turnover.deposited += delta;
                    }
                }
            }
            self.ocean_mismatch = if params.sample_surface {
                let per_cell = crate::sea::ocean_from(
                    self.tectonics.mesh(),
                    elevation,
                    seeds.iter().copied(),
                    open,
                );
                // The erosion's ocean at the start of the step (its result has the one at the
                // end).
                let sample = crate::sea::ocean_from(
                    self.tectonics.mesh(),
                    surface,
                    seeds.iter().copied(),
                    open,
                );
                per_cell.iter().zip(&sample).filter(|(a, b)| a != b).count()
            } else {
                0
            };
            let cell_area = self.cell_area();
            let to = self.cover_export.add_step(
                (0..class.len()).map(|cell| {
                    (
                        class[cell] == CrustClass::Continental,
                        result.sediment_delta[cell] * crate::isostasy::SEDIMENT_ROCK_DENSITY,
                        result.bedrock_delta[cell] * bedrock_density[cell],
                        result.water.ocean[cell],
                        result.water.depression_at_start[cell],
                    )
                }),
                cell_area / 1e12,
            );
            let cohort_before = self.tectonics.cover_cohort_eroded();
            let carbonate_before = self.tectonics.carbonate_flows().0.surface;
            self.tectonics.apply_surface_changes(
                &result.bedrock_delta,
                &result.sediment_delta,
                result.sea_level_change,
            );
            let cohort_after = self.tectonics.cover_cohort_eroded();
            recycled = carbonate_before - self.tectonics.carbonate_flows().0.surface;
            if self.carbon.params().carbonate_sediment {
                deposits = result
                    .sediment_delta
                    .iter()
                    .map(|delta| delta.max(0.0))
                    .collect();
            }
            self.cover_export.add_cohort(
                [
                    cohort_after[0] - cohort_before[0],
                    cohort_after[1] - cohort_before[1],
                ],
                to,
            );
            // The lakes' water comes from the ocean.
            self.tectonics.set_lake_water(result.hydrology.lake_volume);
            self.discharge = result.discharge;
            self.water = result.water;
            self.sediment_flux = result.sediment_to_ocean / dt_years;
            if self.tectonics.time() > dt + 1e-9 {
                self.inland_change +=
                    (result.hydrology.inland_cells - self.hydrology.inland_cells).abs();
            }
            // The lake evaporation the balance removed in this step, and what the climate
            // returned to the air in it (from the lakes of earlier steps; #126).
            debug!(
                removed = result.hydrology.lake_evaporation / 1e9,
                into_air = self.climate.water_cycle().total_lake_evaporation / 1e9,
                "Lake evaporation (km³/yr)"
            );
            debug!(
                inland_cells = result.hydrology.inland_cells,
                inland_depressions = result.hydrology.inland_depressions,
                dry = result.hydrology.dry_inland_depressions,
                "Closed depressions below sea level"
            );
            self.hydrology = result.hydrology;
            self.sediment_overfill += result.sediment_overfill;
            self.terminal_fans += result.terminal_fan;
            self.sediment_delivered += result.sediment_to_ocean;
            let wave = &mut self.wave_erosion;
            wave.cliff_sediment += result.wave.cliff_sediment;
            wave.cliff_bedrock += result.wave.cliff_bedrock;
            wave.reworked += result.wave.reworked;
            wave.stayed += result.wave.stayed;
            self.sediment_deposition = result.deposited.map(|d| d * cell_area / dt_years);
        }
        if self.erosion.is_none() {
            self.water = SurfaceWater::ocean_only(crate::sea::ocean_from(
                self.tectonics.mesh(),
                elevation,
                seeds.iter().copied(),
                open,
            ));
        }
        let start_carbon = Instant::now();
        let land = self.update_carbon(view, &eroded, dt);
        if self.carbon.params().carbonate_sediment {
            self.precipitate_carbonate(dt, &deposits, recycled);
        }
        self.update_escape(dt, &land);
        let carbon_time = start_carbon.elapsed();
        if self.cover_cohort.is_none() && self.tectonics.time() >= self.cover_cohort_time - 1e-9 {
            let mass = self.tectonics.start_cover_cohort();
            self.cover_cohort = Some((
                self.tectonics.time(),
                mass,
                self.tectonics.cover_budget(),
                self.cover_export,
            ));
        }
        carbon_time
    }

    /// One step of the carbon cycle on the step's climate (`elevation` as the climate saw it)
    /// and erosion (`eroded`: bedrock and sediment per cell in m/yr), which sets the greenhouse
    /// forcing of the next climate update unless the cycle is off. Returns the land cells it saw.
    fn update_carbon(
        &mut self,
        elevation: &[f64],
        eroded: &[(f64, f64, f64, f64)],
        dt: f64,
    ) -> Vec<LandCell> {
        let fields = self.temperature.fields();
        let runoff = &self.climate.water_cycle().runoff;
        let bedrock_density = self.tectonics.bedrock_densities();
        let area = self.cell_area();
        let land: Vec<LandCell> = (0..elevation.len())
            .filter(|&cell| elevation[cell] >= 0.0)
            .map(|cell| LandCell {
                mafic: 1.0 - self.tectonics.surface_crust(cell as u32).felsic,
                area,
                temperature: fields.annual_mean[cell],
                runoff: runoff[cell],
                ice: fields.ice[cell],
                bedrock_erosion: eroded[cell].0,
                sediment_erosion: eroded[cell].1,
                bare: eroded[cell].2,
                carbonate: eroded[cell].3,
                bedrock_density: bedrock_density[cell],
            })
            .collect();
        let deep_water = |cells: &dyn Fn(usize) -> bool| {
            let (sum, count) = (0..elevation.len())
                .filter(|&cell| elevation[cell] < 0.0 && cells(cell))
                .fold((0.0, 0usize), |(sum, count), cell| {
                    (sum + fields.annual_mean[cell], count + 1)
                });
            (count > 0).then_some(sum / count as f64)
        };
        let deep_water_temperature = deep_water(&|cell| self.deep_water_cells[cell])
            .or_else(|| deep_water(&|_| true))
            .unwrap_or(0.0);
        let params = self.temperature.params();
        let time = self.tectonics.time();
        let sunlight_change = (params.solar_constant(time + dt) - params.solar_constant(time))
            * (1.0 - crate::presets::BOND_ALBEDO)
            / 4.0;
        let holds_co2 = self.carbon.holds_co2();
        self.ramp_up.log_start(holds_co2.then_some("CO₂"));
        let forcing = self.carbon.step(&StepInput {
            years: dt * 1e6,
            drivers: self.tectonics.carbon_drivers(),
            land: &land,
            deep_water_temperature,
            ocean_mass: crate::carbon::ocean_mass(self.tectonics.ocean_volume()),
            planet_area: 4.0 * std::f64::consts::PI * self.planet_radius * self.planet_radius,
            sunlight_change,
            toa_imbalance: fields.toa_imbalance,
        });
        if self.carbon.params().enabled {
            self.temperature.params_mut().greenhouse_forcing = forcing;
        }
        info!(time = %format_args!("{time:.1}"), "{}", self.carbon.state().summary());
        // The step that ends the phase is the last of the calibration.
        if self.ramp_up.observe(
            time,
            dt * 1e6,
            self.tectonics.effective_plate_count(),
            self.carbon.tectonic_degassing(),
        ) {
            self.carbon.end_hold();
            if holds_co2 {
                info!(
                    phi = self.carbon.state().phi,
                    "The carbon cycle sets the CO₂ from now on"
                );
            }
        }
        land
    }

    /// Precipitates the carbonate of the step (#200): the carbon cycle's land burial and the
    /// carbonate the surface processes removed (`recycled`, kg, dissolved), on the sea floor by
    /// its saturation (see [`crate::carbonate`]); the same mass of dissolved load (the cation
    /// oxide of the burial and the recycled carbonate, which the erosion routed as clastic
    /// sediment) is taken back from the step's clastic `deposits` (rock-equivalent m per
    /// cell), in the sea first. Everything per crust sample, so the masses are exact.
    fn precipitate_carbonate(&mut self, dt: f64, deposits: &[f64], recycled: f64) {
        use crate::carbonate::{self, CARBONATE_MOLAR_MASS, CO2_MOLAR_MASS};
        use crate::isostasy::SEDIMENT_ROCK_DENSITY;
        let years = dt * 1e6;
        let new = self.carbon.state().land_burial.max(0.0) * years;
        let recycled = recycled.max(0.0);
        let total = new + recycled / CARBONATE_MOLAR_MASS;
        self.carbonate.deposit.fill(0.0);
        self.carbonate.level = f64::NAN;
        self.carbonate.max_omega = f64::NAN;
        if total <= 0.0 {
            return;
        }
        let cell_area = self.cell_area();
        // mol per rock-equivalent m on one sample.
        let per_metre = SEDIMENT_ROCK_DENSITY * cell_area / CARBONATE_MOLAR_MASS;
        let simulation = &self.tectonics;
        let mesh = simulation.mesh();
        let elevation = simulation.surface_elevations();
        let ocean = &self.water.ocean;
        let ids = simulation.sample_ids();
        let groups = ids.iter().copied().max().map_or(0, |max| max as usize + 1);
        let mut cells: Vec<Vec<u32>> = vec![Vec::new(); groups];
        for (cell, &id) in ids.iter().enumerate() {
            cells[id as usize].push(cell as u32);
        }
        let params = &self.carbon.params().precipitation;
        let wave_base = self
            .erosion
            .as_ref()
            .filter(|erosion| erosion.waves)
            .map(|erosion| {
                crate::erosion::wave_bases(mesh, &elevation, simulation.min_sea_fraction(), erosion)
            });
        let temperature = &self.temperature.fields().annual_mean;
        let deep_water = self.carbon.state().deep_water_temperature;
        let deep_water = if deep_water.is_finite() {
            deep_water
        } else {
            2.0
        };
        // The sea-floor units: samples with cells in the ocean, their sea cells and room.
        let mut units = Vec::new();
        let mut unit_group = Vec::new();
        for (group, group_cells) in cells.iter().enumerate() {
            let sea: Vec<u32> = group_cells
                .iter()
                .copied()
                .filter(|&cell| ocean[cell as usize] && elevation[cell as usize] < 0.0)
                .collect();
            if sea.is_empty() {
                continue;
            }
            let sediment = simulation.surface_crust(sea[0]).sediment;
            let room = sea
                .iter()
                .map(|&cell| {
                    let top = wave_base.as_ref().map_or(params.platform_top, |bases| {
                        bases[cell as usize].min(params.platform_top)
                    });
                    crate::isostasy::sediment_for_rise(elevation[cell as usize], top, sediment)
                        .max(0.0)
                })
                .sum::<f64>()
                / sea.len() as f64;
            units.push(carbonate::Unit {
                cells: sea
                    .iter()
                    .map(|&cell| carbonate::SeaCell {
                        depth: -elevation[cell as usize],
                        surface_temperature: temperature[cell as usize],
                    })
                    .collect(),
                room: room * per_metre,
            });
            unit_group.push(group);
        }
        let distribution =
            carbonate::distribute(params, &units, deep_water, cell_area, years, total);

        // The dissolved load, from the step's deposits: in the sea first, then on land, then
        // from the clastic sediment of the sea floor.
        let clastic_mass = new * (CARBONATE_MOLAR_MASS - CO2_MOLAR_MASS) + recycled;
        let target = clastic_mass / (SEDIMENT_ROCK_DENSITY * cell_area);
        let mean = |group: usize, value: &dyn Fn(usize) -> f64| -> f64 {
            let group_cells = &cells[group];
            group_cells.iter().map(|&c| value(c as usize)).sum::<f64>()
                / group_cells.len().max(1) as f64
        };
        let clastic: Vec<f64> = (0..groups)
            .map(|group| simulation.surface_crust(cells[group][0]).clastic())
            .collect();
        let mut caps = clastic.clone();
        let mut removal = vec![0.0; groups];
        let mut left = target;
        for pass in 0..3 {
            if left <= 0.0 || (deposits.is_empty() && pass < 2) {
                continue;
            }
            let weights: Vec<f64> = (0..groups)
                .map(|group| match pass {
                    0 => mean(group, &|cell| {
                        if ocean[cell] { deposits[cell] } else { 0.0 }
                    }),
                    1 => mean(group, &|cell| deposits[cell]),
                    _ => mean(group, &|cell| f64::from(u8::from(ocean[cell]))) * clastic[group],
                })
                .collect();
            let taken = carbonate::capped_fill(&weights, &caps, left);
            for ((removal, cap), taken) in removal.iter_mut().zip(&mut caps).zip(taken) {
                *removal += taken;
                *cap -= taken;
                left -= taken;
            }
        }

        // Apply, with the diagnostics.
        let mut deposit = vec![0.0; groups];
        let mut max_omega = f64::NAN;
        let stats = &mut self.carbonate.stats;
        for (unit, (&group, &amount)) in unit_group.iter().zip(&distribution.amounts).enumerate() {
            deposit[group] = amount / per_metre;
            if amount <= 0.0 {
                continue;
            }
            let mass = amount * CARBONATE_MOLAR_MASS;
            let omega = distribution.omega[unit];
            stats.omega_mass += mass * omega;
            max_omega = if max_omega.is_nan() {
                omega
            } else {
                max_omega.max(omega)
            };
            let cell = cells[group][0];
            let depth = units[unit].cells.iter().map(|c| c.depth).sum::<f64>()
                / units[unit].cells.len() as f64;
            let bin = [20.0, 200.0, 1000.0, 3000.0]
                .iter()
                .filter(|&&limit| depth >= limit)
                .count();
            stats.by_depth[bin] += mass;
            let latitude = math::asin(mesh.vertex(cell).z.clamp(-1.0, 1.0))
                .to_degrees()
                .abs();
            let bin = [20.0, 40.0, 60.0]
                .iter()
                .filter(|&&limit| latitude >= limit)
                .count();
            stats.by_latitude[bin] += mass;
            if simulation.surface_crust(cell).is_continental_origin() {
                stats.on_continental += mass;
            }
        }
        stats.new += new * CARBONATE_MOLAR_MASS;
        stats.recycled += recycled;
        stats.overflow += distribution.overflow * CARBONATE_MOLAR_MASS;
        let changes: Vec<CarbonateExchange> = (0..groups)
            .filter(|&group| deposit[group] > 0.0 || removal[group] > 0.0)
            .map(|group| CarbonateExchange {
                cell: cells[group][0],
                clastic: removal[group],
                carbonate: deposit[group],
            })
            .collect();
        for (group, group_cells) in cells.iter().enumerate() {
            for &cell in group_cells {
                self.carbonate.deposit[cell as usize] = deposit[group] / dt;
            }
        }
        let removed = self.tectonics.exchange_carbonate(&changes);
        let stats = &mut self.carbonate.stats;
        stats.clastic_removed += removed * SEDIMENT_ROCK_DENSITY * cell_area;
        stats.unmatched += clastic_mass - removed * SEDIMENT_ROCK_DENSITY * cell_area;
        self.carbonate.level = distribution.level;
        self.carbonate.max_omega = max_omega;
        let state = self.carbon.state();
        info!(
            time = %format_args!("{:.1}", self.tectonics.time()),
            "carbonate: burial land {:.3} Tmol/yr, sea floor {:.3} | precipitated {:.3} Gt/yr \
             (recycled {:.3}), saturation level {:.3}, max Ω {:.2}, overflow {:.2e} Gt/yr",
            state.land_burial / 1e12,
            state.seafloor_burial / 1e12,
            total * CARBONATE_MOLAR_MASS / 1e12 / years,
            recycled / 1e12 / years,
            distribution.level,
            max_omega,
            distribution.overflow * CARBONATE_MOLAR_MASS / 1e12 / years,
        );
    }

    /// One step of the escape (#210) on the step's climate and CO₂: the water it loses is taken
    /// from the ocean, its O₂ meets the sinks of the step's tectonics and erosion (`land`).
    /// Nothing happens when it is off.
    fn update_escape(&mut self, dt: f64, land: &[LandCell]) {
        if !self.escape.params().enabled {
            return;
        }
        let fields = self.temperature.fields();
        let ocean = &self.water.ocean;
        // The tropical ocean, or any ocean if the tropics have none.
        let mut temperatures: Vec<f64> = (0..ocean.len())
            .filter(|&cell| ocean[cell] && self.tropical_cells[cell])
            .map(|cell| fields.annual_mean[cell])
            .collect();
        if temperatures.is_empty() {
            temperatures = (0..ocean.len())
                .filter(|&cell| ocean[cell])
                .map(|cell| fields.annual_mean[cell])
                .collect();
        }
        let time = self.tectonics.time();
        let volume = self.escape.step(&crate::escape::StepInput {
            years: dt * 1e6,
            tropical_temperatures: &temperatures,
            co2_pressure: self.carbon.state().pressure,
            sun_age: crate::sun::age_of_run(self.temperature.params().epoch, time),
            planet_radius: self.planet_radius,
            ocean_volume: self.tectonics.ocean_volume(),
            volcanic_degassing: self.carbon.state().arc_plume_mantle_degassing,
            new_crust: self.carbon.state().new_crust,
            land,
        });
        if volume > 0.0 {
            self.tectonics.remove_water(volume);
        }
        debug!(time = %format_args!("{time:.1}"), "{}", self.escape.state().summary());
    }

    /// The plate and the [`CrustClass`] of every cell.
    fn plates_and_classes(&self) -> (Vec<u32>, Vec<CrustClass>) {
        (0..self.tectonics.mesh().len() as u32)
            .map(|cell| {
                (
                    self.tectonics.owner(cell).plate,
                    crust_class(&self.tectonics.surface_crust(cell)),
                )
            })
            .unzip()
    }

    fn cell_area(&self) -> f64 {
        4.0 * std::f64::consts::PI * self.planet_radius.powi(2) / self.tectonics.mesh().len() as f64
    }

    /// Solves the water cycle on the climate's `elevation`, with the evaporation of the lakes of
    /// the last erosion step as a moisture source (#126).
    fn update_water_cycle(&mut self, elevation: &[f64]) {
        let fields = self.temperature.fields();
        self.climate.update(
            elevation,
            &self.water.lake_evaporation,
            &fields.monthly,
            &fields.monthly_lake_ice,
            &fields.thermal_equator,
        );
        let cycle = self.climate.water_cycle();
        debug!(iterations = ?cycle.iterations, clipped = cycle.clipped, "Water cycle solved");
        self.precipitation = cycle.precipitation.clone();
    }

    pub fn water_cycle(&self) -> &WaterCycle {
        self.climate.water_cycle()
    }

    /// A paired diagnostic of the lakes' effect on the temperatures (#173): the temperature
    /// model is run again from its current state on the current topography and lakes, once
    /// with lakes as open water and once as land, for `updates` updates each, and the fields
    /// are compared on the lakes and the land around them.
    pub fn lake_climate_contrast(&self, updates: usize) -> LakeClimateContrast {
        let mesh = self.tectonics.mesh();
        let elevation = self.tectonics.surface_elevations();
        let seeds = crate::sea::ocean_seeds(
            mesh,
            &elevation,
            -crate::sea::SILL_TOLERANCE,
            self.tectonics.min_sea_fraction(),
        );
        let open = open_sills(&self.erosion);
        let run = |on: bool| {
            let mut model = self.temperature.clone();
            model.params_mut().open_water_lakes = on;
            let lakes = on.then_some(&self.water);
            let view = climate_elevations(mesh, &elevation, &seeds, open, lakes);
            for _ in 0..updates {
                model.update(
                    &view,
                    Some(&self.precipitation),
                    lakes.map(|water| &water.lake_depth[..]),
                    self.tectonics.time(),
                );
            }
            (model.fields().clone(), view)
        };
        let (water, view) = run(true);
        let (land, _) = run(false);
        let lake: Vec<bool> = (0..elevation.len())
            .map(|cell| {
                self.water.lake_depth[cell] >= crate::temperature::LAKE_DEPTH && view[cell] >= 0.0
            })
            .collect();
        // Large lakes: connected lake cells of at least `LARGE_LAKE` cells.
        let mut large = vec![false; lake.len()];
        let mut seen = vec![false; lake.len()];
        for start in 0..lake.len() {
            if !lake[start] || seen[start] {
                continue;
            }
            seen[start] = true;
            let mut component = vec![start];
            let mut next = 0;
            while next < component.len() {
                let cell = component[next];
                next += 1;
                for &n in mesh.neighbors(cell as u32) {
                    let n = n as usize;
                    if lake[n] && !seen[n] {
                        seen[n] = true;
                        component.push(n);
                    }
                }
            }
            if component.len() >= LakeClimateContrast::LARGE_LAKE {
                for cell in component {
                    large[cell] = true;
                }
            }
        }
        // Rings of land (not lakes, not ocean) around the large lakes.
        let mut ring = vec![0u8; lake.len()];
        let mut reached = large.clone();
        let mut front: Vec<usize> = (0..lake.len()).filter(|&c| large[c]).collect();
        for distance in 1..=3u8 {
            let mut next = Vec::new();
            for &cell in &front {
                for &n in mesh.neighbors(cell as u32) {
                    let n = n as usize;
                    if !reached[n] && !lake[n] && view[n] >= 0.0 {
                        reached[n] = true;
                        ring[n] = distance;
                        next.push(n);
                    }
                }
            }
            front = next;
        }
        let compare = |cells: &dyn Fn(usize) -> bool| {
            let cells: Vec<usize> = (0..lake.len()).filter(|&c| cells(c)).collect();
            let mean = |fields: &TemperatureFields,
                        value: &dyn Fn(&TemperatureFields, usize) -> f64| {
                cells.iter().map(|&c| value(fields, c)).sum::<f64>() / cells.len().max(1) as f64
            };
            let stats = |fields: &TemperatureFields| SurfaceStats {
                annual: mean(fields, &|f, c| f.annual_mean[c]),
                range: mean(fields, &|f, c| f.seasonal_range(c)),
                frozen: mean(fields, &|f, c| f64::from(u8::from(f.ice[c] > 0.5))),
            };
            (cells.len(), stats(&water), stats(&land))
        };
        LakeClimateContrast {
            lakes: compare(&|c| lake[c]),
            large_lakes: compare(&|c| large[c]),
            near: compare(&|c| ring[c] == 1),
            around: compare(&|c| ring[c] >= 2),
        }
    }

    /// Whether a sill on which the sea stands connects the basin behind it to the ocean (see
    /// [`crate::sea::ocean_from`]).
    pub fn open_sills(&self) -> bool {
        open_sills(&self.erosion)
    }

    pub fn statistics(&self) -> PlanetStatistics {
        let elevation = self.tectonics.surface_elevations();
        let ocean = crate::sea::ocean_mask(
            self.tectonics.mesh(),
            &elevation,
            self.tectonics.min_sea_fraction(),
            self.open_sills(),
        );
        // Land includes closed depressions below sea level (dry or holding lakes).
        let land: Vec<usize> = (0..elevation.len()).filter(|&cell| !ocean[cell]).collect();
        let mean_land_precipitation = if land.is_empty() {
            0.0
        } else {
            land.iter().map(|&c| self.precipitation[c]).sum::<f64>() / land.len() as f64
        };
        let largest_river = (0..elevation.len())
            .filter(|&cell| ocean[cell])
            .map(|cell| self.discharge[cell])
            .fold(0.0, f64::max);

        let mesh = self.tectonics.mesh();
        let fields = self.temperature.fields();
        let latitude = |cell: usize| math::asin(mesh.vertex(cell as u32).z).to_degrees();
        let mean = |cells: &mut dyn Iterator<Item = usize>, value: &dyn Fn(usize) -> f64| {
            let (sum, count) =
                cells.fold((0.0, 0usize), |(sum, count), c| (sum + value(c), count + 1));
            if count == 0 { 0.0 } else { sum / count as f64 }
        };
        let precipitation = |c: usize| self.precipitation[c];
        let midlatitude_land_precipitation = mean(
            &mut land
                .iter()
                .copied()
                .filter(|&c| (20.0..60.0).contains(&latitude(c).abs())),
            &precipitation,
        );
        let cycle = self.climate.water_cycle();
        let global_precipitation = cycle.global_precipitation;
        let global_evaporation = cycle.global_evaporation;
        let land_runoff = mean(&mut land.iter().copied(), &|c| cycle.runoff[c]);
        let arid_land_fraction = mean(&mut land.iter().copied(), &|c| {
            f64::from(u8::from(self.precipitation[c] < 0.25))
        });
        let all = || 0..elevation.len();
        let annual = |c: usize| fields.annual_mean[c];

        let mut sediment_stock = [0.0; 3];
        let cell_area = self.cell_area();
        for cell in 0..elevation.len() as u32 {
            let crust = self.tectonics.surface_crust(cell);
            sediment_stock[crust_class(&crust) as usize] +=
                crust.sediment * cell_area * crate::isostasy::SEDIMENT_ROCK_DENSITY / 1e12;
        }

        PlanetStatistics {
            // Mesh cells have nearly equal areas, so plain means are area-weighted.
            global_temperature: mean(&mut all(), &annual),
            equatorial_temperature: mean(&mut all().filter(|&c| latitude(c).abs() < 10.0), &annual),
            polar_temperature: mean(&mut all().filter(|&c| latitude(c).abs() > 70.0), &annual),
            land_seasonal_range: mean(&mut land.iter().copied(), &|c| fields.seasonal_range(c)),
            gyre_transport: fields.gyre_transport,
            solar_constant: fields.solar_constant,
            carbon: *self.carbon.state(),
            // Once it has taken a step.
            escape: (self.escape.params().enabled && self.escape.state().xuv > 0.0)
                .then_some(*self.escape.state()),
            heat_transport: {
                let transport = &fields.meridional_transport;
                let to_petawatts = |t: f64| t * self.planet_radius.powi(2) / 1e15;
                let nearest = |latitude: f64| {
                    transport
                        .iter()
                        .min_by(|a, b| (a.0 - latitude).abs().total_cmp(&(b.0 - latitude).abs()))
                        .map_or(0.0, |&(_, _, ocean)| to_petawatts(ocean))
                };
                let total = || transport.iter().map(|t| to_petawatts(t.1 + t.2));
                [
                    total().fold(0.0, f64::max),
                    total().fold(0.0, f64::min),
                    nearest(20.0),
                    nearest(-20.0),
                ]
            },
            frozen_fraction: all().filter(|&c| fields.ice[c] > 0.5).count() as f64
                / elevation.len() as f64,
            tectonics: self.tectonics.statistics(),
            mean_land_precipitation,
            midlatitude_land_precipitation,
            arid_land_fraction,
            global_precipitation,
            global_evaporation,
            lake_evaporation_to_air: cycle.total_lake_evaporation,
            land_runoff,
            itcz: cycle.itcz.clone(),
            clipped_precipitation: cycle.clipped,
            largest_river,
            sediment_flux: self.sediment_flux,
            hydrology: self.hydrology,
            sediment_overfill: self.sediment_overfill,
            terminal_fans: self.terminal_fans,
            sediment_deposition: self.sediment_deposition,
            sediment_stock,
            sediment_reservoirs: self.sediment_reservoirs(&elevation, &ocean),
            ocean_fraction: ocean.iter().filter(|&&o| o).count() as f64 / ocean.len() as f64,
            below_sea_level_fraction: elevation.iter().filter(|&&e| e < 0.0).count() as f64
                / elevation.len() as f64,
            inland_change: self.inland_change,
            sea_level_deficit: self.tectonics.sea_level_deficit(),
            wave_erosion: {
                let to_gt = cell_area * crate::isostasy::SEDIMENT_ROCK_DENSITY / 1e12;
                let w = &self.wave_erosion;
                WaveErosion {
                    cliff_sediment: w.cliff_sediment * to_gt,
                    cliff_bedrock: w.cliff_bedrock * to_gt,
                    reworked: w.reworked * to_gt,
                    stayed: w.stayed * to_gt,
                }
            },
            sediment_delivered: self.sediment_delivered * crate::isostasy::SEDIMENT_ROCK_DENSITY
                / 1e12,
            cover_cohort: self
                .cover_cohort
                .map(|(start, initial, budget, export)| CoverCohort {
                    start,
                    initial,
                    left: self.tectonics.cover_cohort(),
                    budget: self.tectonics.cover_budget().since(&budget),
                    export: self.cover_export.since(&export),
                }),
            cover_export: self.cover_export,
            cover_budget: self.tectonics.cover_budget(),
            cover_masses: self.tectonics.cover_masses_gt(),
            cover_turnover: {
                let to_gt = cell_area * crate::isostasy::SEDIMENT_ROCK_DENSITY / 1e12;
                CoverTurnover {
                    eroded: self.cover_turnover.eroded * to_gt,
                    deposited: self.cover_turnover.deposited * to_gt,
                }
            },
        }
    }

    fn sediment_reservoirs(&self, elevation: &[f64], ocean: &[bool]) -> SedimentReservoirs {
        let tectonics = &self.tectonics;
        // The selection gets the crust and whether the cell is ocean.
        let part = |select: &dyn Fn(&Crust, bool) -> bool| {
            tectonics.sediment_sea_level_of(|cell, c, _| select(c, ocean[cell as usize]), false)
        };
        let filled_basin = |crust: &Crust| crust.is_continental_origin() && crust.felsic < 0.8;
        let default = ErosionParams::default();
        let wave_base = crate::erosion::wave_bases(
            tectonics.mesh(),
            elevation,
            tectonics.min_sea_fraction(),
            self.erosion.as_ref().unwrap_or(&default),
        );
        let (cell_sediment, all_sediment) = tectonics.sediment_totals();
        let cell_count = elevation.len();
        let land = ocean.iter().filter(|&&o| !o).count();
        let low_land = elevation
            .iter()
            .filter(|&&e| (0.0..10.0).contains(&e))
            .count();
        let (mut shelf, mut shelf_sediment, mut shelf_oceanic, mut shelf_basins) = (0, 0.0, 0, 0);
        let mut basins = 0;
        let mut metasediment_basins = 0;
        for (cell, &e) in elevation.iter().enumerate() {
            let crust = tectonics.surface_crust(cell as u32);
            if filled_basin(&crust) {
                basins += 1;
            }
            if crust.is_continental_origin_column() && !crust.is_continental_origin() {
                metasediment_basins += 1;
            }
            if ocean[cell] && e >= -200.0 {
                shelf += 1;
                shelf_sediment += crust.sediment;
                shelf_oceanic += usize::from(crust.is_oceanic());
                shelf_basins += usize::from(filled_basin(&crust));
            }
        }
        SedimentReservoirs {
            all: part(&|_, _| true),
            continental_land: part(&|c, marine| c.is_continental_origin() && !marine),
            continental_flooded: part(&|c, marine| c.is_continental_origin() && marine),
            transitional: part(&|c, _| crust_class(c) == CrustClass::Transitional),
            oceanic: part(&|c, _| crust_class(c) == CrustClass::Oceanic),
            marine: part(&|_, marine| marine),
            filled_basins: part(&|c, _| filled_basin(c)),
            filled_basin_fraction: basins as f64 / cell_count as f64,
            metasediment_basin_fraction: metasediment_basins as f64 / cell_count as f64,
            above_wave_base: tectonics.sediment_sea_level_of(
                |cell, _, e| ocean[cell as usize] && e >= wave_base[cell as usize],
                false,
            ),
            with_metasediment: tectonics.sediment_sea_level_of(|_, _, _| true, true),
            solver_ratio: cell_sediment / all_sediment.max(1e-9),
            metasediment: tectonics.metasediment_masses(),
            low_land_fraction: low_land as f64 / land.max(1) as f64,
            shelf_fraction: shelf as f64 / cell_count as f64,
            shelf_sediment: shelf_sediment / shelf.max(1) as f64,
            shelf_on_oceanic: shelf_oceanic as f64 / shelf.max(1) as f64,
            shelf_on_filled_basins: shelf_basins as f64 / shelf.max(1) as f64,
        }
    }
}

/// The [`CrustClass`] of a crust sample.
fn crust_class(crust: &Crust) -> CrustClass {
    if crust.is_continental_origin() {
        CrustClass::Continental
    } else if crust.is_oceanic() {
        CrustClass::Oceanic
    } else {
        CrustClass::Transitional
    }
}

/// Whether a sill on which the sea stands connects the basin behind it to the ocean (see
/// [`crate::sea::ocean_from`]): only without [`ErosionParams::sea_spill`].
fn open_sills(erosion: &Option<ErosionParams>) -> bool {
    erosion.as_ref().is_none_or(|params| !params.sea_spill)
}

/// The surface elevations as the climate and temperature models see them: closed depressions
/// below sea level are land (raised to sea level; the lapse rate ignores elevations below it
/// anyway), so that they get runoff and potential evaporation instead of ocean evaporation.
/// With `lakes` (the surface water of the last erosion step), lake cells outside the ocean are at
/// their water surface (#173), at least at sea level.
fn climate_elevations(
    mesh: &crate::sphere::SphereMesh,
    elevation: &[f64],
    seeds: &[usize],
    open_sills: bool,
    lakes: Option<&SurfaceWater>,
) -> Vec<f64> {
    let ocean = crate::sea::ocean_from(mesh, elevation, seeds.iter().copied(), open_sills);
    let mut elevation = elevation.to_vec();
    for (cell, (e, ocean)) in elevation.iter_mut().zip(ocean).enumerate() {
        if !ocean {
            if let Some(water) = lakes
                && water
                    .lake_depth
                    .get(cell)
                    .is_some_and(|&depth| depth >= crate::temperature::LAKE_DEPTH)
            {
                // The floor as the lake loads it, plus the water on it.
                *e += water.load_correction[cell] + water.lake_depth[cell];
            }
            *e = e.max(0.0);
        }
    }
    elevation
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sphere::SphereMesh;
    use crate::tectonics::{CoverTerm, TectonicParams};

    const RADIUS: f64 = 6.371e6;

    fn small_planet() -> Planet {
        let simulation = Simulation::new(SphereMesh::icosphere(3), 6, TectonicParams::default(), 2);
        Planet::new(
            simulation,
            ClimateParams::default(),
            TemperatureParams::default(),
            CarbonParams::default(),
            EscapeParams::default(),
            Some(ErosionParams {
                planet_radius: RADIUS,
                ..ErosionParams::default()
            }),
            RADIUS,
        )
    }

    fn carbonate_planet(carbon: CarbonParams, carbonate: bool) -> Planet {
        let mut params = TectonicParams::default();
        params.initial_sediment.carbonate = carbonate;
        let simulation = Simulation::new(SphereMesh::icosphere(4), 8, params, 5);
        Planet::new(
            simulation,
            ClimateParams::default(),
            TemperatureParams::default(),
            carbon,
            EscapeParams::default(),
            Some(ErosionParams {
                planet_radius: RADIUS,
                ..ErosionParams::default()
            }),
            RADIUS,
        )
    }

    /// The carbonate sediment (#200) closes the carbon books, while the CO₂ is held and when
    /// the cycle is active: the reservoirs and what went into the mantle equal the start and
    /// what came out of it. The burial precipitates, and the dissolved load is fully taken from
    /// the clastic sediment.
    #[test]
    fn carbonate_sediment_closes_the_carbon_books() {
        for phi in [f64::NAN, 1.0] {
            let carbon = CarbonParams {
                phi,
                ..CarbonParams::default()
            };
            let mut planet = carbonate_planet(carbon, true);
            let start = planet.carbon_partition();
            assert!(start.sediment > 0.0);
            assert!(start.residual.abs() <= 1e-9 * start.sediment, "{start:?}");
            for _ in 0..8 {
                planet.step(1.0);
            }
            let p = planet.carbon_partition();
            let scale = p.exogenic + p.sediment;
            assert!(p.residual.abs() <= 1e-9 * scale, "{phi}: {p:?}");
            assert!(p.lost.abs() <= 1e-3 * p.sediment, "{phi}: {p:?}");
            let stats = planet.carbonate().stats;
            assert!(stats.new > 0.0, "{phi}: {stats:?}");
            assert!(
                stats.unmatched.abs() <= 1e-9 * stats.clastic_removed,
                "{phi}: {stats:?}"
            );
            let deposited: f64 = stats.by_depth.iter().sum();
            assert!(
                (deposited / (stats.new + stats.recycled) - 1.0).abs() < 1e-9,
                "{phi}: {stats:?}"
            );
        }
    }

    /// Switched off, no sediment carries carbonate and nothing precipitates.
    #[test]
    fn carbonate_sediment_off_leaves_no_carbonate() {
        let carbon = CarbonParams {
            carbonate_sediment: false,
            ..CarbonParams::default()
        };
        let mut planet = carbonate_planet(carbon, false);
        for _ in 0..3 {
            planet.step(1.0);
        }
        assert_eq!(planet.tectonics().carbonate_inventory(), 0.0);
        assert_eq!(planet.carbonate().stats.new, 0.0);
        assert!(planet.carbonate().deposit.iter().all(|&d| d == 0.0));
    }

    /// A step taken part by part with reads in between (as by a simulation paused inside a
    /// step, #116) ends as one taken at once.
    #[test]
    fn steps_taken_in_parts_end_alike() {
        let mut whole = small_planet();
        let mut parts = small_planet();
        let mut counts = Vec::new();
        for _ in 0..3 {
            whole.step(1.0);
            let mut step = parts.begin_step(1.0);
            let mut count = 1;
            while !parts.continue_step(&mut step) {
                // What a paused simulation may do in between.
                parts.state_hash();
                parts.statistics();
                crate::viewer::cell_infos(&parts);
                count += 1;
            }
            counts.push(count);
        }
        assert_eq!(parts.state_hash(), whole.state_hash());
        assert_eq!(parts.tectonics().time(), 3.0);
        // Tectonics start, at least one substep, tectonics end, temperature, water, erosion.
        assert!(counts.iter().all(|&count| count >= 6), "{counts:?}");
    }

    /// The cover's export split and budget close (#35): the destinations of the continental
    /// denudation add up to it, the budget terms to the change of the masses, and the cohort's
    /// fate to its initial mass.
    #[test]
    fn cover_export_and_budget_close() {
        let simulation = Simulation::new(SphereMesh::icosphere(4), 8, TectonicParams::default(), 5);
        let start = simulation.cover_masses_gt();
        let mut planet = Planet::new(
            simulation,
            ClimateParams::default(),
            TemperatureParams::default(),
            CarbonParams::default(),
            EscapeParams::default(),
            Some(ErosionParams {
                planet_radius: RADIUS,
                ..ErosionParams::default()
            }),
            RADIUS,
        );
        planet.track_cover_cohort_from(4.0);
        for _ in 0..9 {
            planet.step(1.0);
        }
        let statistics = planet.statistics();
        let close = |a: f64, b: f64, scale: f64| (a - b).abs() <= 1e-9 * scale;

        let export = &statistics.cover_export;
        assert!(export.denudation > 0.0 && export.reworked > 0.0);
        assert!(close(
            export.to.iter().sum::<f64>(),
            export.denudation,
            export.denudation
        ));

        let masses = statistics.cover_masses;
        let total = statistics.cover_budget.total();
        assert!(close(masses[0] - start[0], total[0], masses[0]));
        // The cohort only started later: its tracers were empty before.
        assert_eq!(start[1..], [0.0, 0.0]);

        let cohort = statistics.cover_cohort.expect("the cohort started");
        assert!(cohort.initial > 0.0);
        for tracer in 0..2 {
            // Now = initial + the budget since the start.
            assert!(close(
                cohort.left.anywhere[tracer],
                cohort.initial + cohort.budget.total()[1 + tracer],
                cohort.initial
            ));
            // What surface processes removed is split over the destinations.
            let eroded: f64 = cohort.export.cohort_to[tracer].iter().sum();
            assert!(eroded >= 0.0);
            assert!(close(
                eroded,
                -cohort.budget.term(CoverTerm::Surface)[1 + tracer],
                cohort.initial
            ));
        }
        // Youngest first keeps at least as much as well mixed.
        assert!(cohort.left.on_continents[1] >= cohort.left.on_continents[0] - 1e-9);
        assert!(cohort.left.on_continents[0] < cohort.initial);
    }
}
