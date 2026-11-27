//! Fluvial erosion, sediment transport and deposition.
//!
//! 1. **Drainage.** A priority flood from the ocean assigns every land cell a receiver, the cell
//!    its water drains into. Closed depressions are lakes, which drain over their lowest pass.
//! 2. **Discharge.** Runoff (precipitation minus evapotranspiration) is accumulated downstream
//!    along the receivers. Lakes evaporate at the potential rate: a lake rises until its
//!    evaporation balances its inflow, and only spills if it reaches its pass first; otherwise
//!    its basin is endorheic and its water and sediment stay in it.
//! 3. **Incision.** Rivers cut into the land following the stream-power law
//!    `E = K · Q^m · S`, solved implicitly (Braun & Willett, 2013), which is stable for any time
//!    step. Rivers cannot cut below the water surface they flow into (sea level or a lake), their
//!    base level. Loose sediment is removed before bedrock. Removing bedrock thins the crust,
//!    which then rises isostatically, so the surface only drops by a fraction of the removed
//!    thickness; the implicit solve accounts for that rebound.
//! 4. **Deposition.** Sediment is carried downstream. Lakes trap it, and from river mouths it
//!    is routed onto continental margins, where Earth keeps most of its sediment: the shelf on
//!    continental and transitional crust fills up to [`ErosionParams::shelf_depth`], the oceanic
//!    crust next to it (by distance from the margin crust of the same plate) only up to the
//!    depth of the continental slope and rise, each with a cap on the sediment thickness. The
//!    river's own plate fills first, other plates' margins only when it is full. What does not
//!    fit forms deep-sea fans that thin with distance from the mouth. When everything within
//!    reach is full, deltas build up to [`ErosionParams::delta_top`] slightly above sea level on
//!    the margin crust. Sediment loads the crust, so the sea floor rises by only ~0.44 of the
//!    deposited thickness (see [`crate::isostasy`]).

use crate::math;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use crate::isostasy::{self, SEDIMENT_ROCK_DENSITY};
use crate::sea;
use crate::sphere::SphereMesh;
use crate::tectonics::SurfaceSeaLevel;

mod basins;
use basins::Basins;
mod regolith;
pub use regolith::{RegolithParams, RegolithStats};

/// Parameters of erosion. Lengths in metres, times in years unless noted otherwise.
#[derive(Clone, Debug)]
pub struct ErosionParams {
    /// Planet radius in metres.
    pub planet_radius: f64,
    /// Erodibility `K` of the stream-power law, in m^(1-3m) yr^(m-1).
    pub erodibility: f64,
    /// Discharge exponent `m` of the stream-power law.
    pub discharge_exponent: f64,
    /// Sediment fills flooded continental and transitional crust up to this elevation (negative,
    /// below sea level): the shelf.
    pub shelf_depth: f64,
    /// Oceanic crust within [`Self::slope_width`] of the margin crust of its plate (and always
    /// its first ring of cells) fills up to this elevation: the continental slope.
    pub slope_depth: f64,
    /// Oceanic crust within [`Self::rise_width`] of the margin crust of its plate fills up to
    /// this elevation: the continental rise.
    pub rise_depth: f64,
    pub slope_width: f64,
    pub rise_width: f64,
    /// Maximum sediment thickness on the shelf, slope and rise, and at the head of a deep-sea
    /// fan, which thins with distance from the river mouth over [`Self::fan_length`] (large
    /// fans keep km of sediment over 1000–2000 km: Bengal, Indus, Amazon; Curray et al. 2003).
    pub max_shelf_sediment: f64,
    pub max_slope_sediment: f64,
    pub max_rise_sediment: f64,
    pub max_fan_sediment: f64,
    pub fan_length: f64,
    /// Erodibility of sediment relative to bedrock: sedimentary rock and loose sediment erode
    /// ~2–4× faster than crystalline rock (cosmogenic rates ~20 vs 9–11 mm/kyr; stream-power K
    /// ~4× in Neogene sediments vs metamorphic rock), so exposed sediment is recycled before new
    /// crust is eroded, as on Earth (half-mass age of sedimentary rocks ~600 Myr).
    pub sediment_erodibility_factor: f64,
    /// Diffusivity (m²/yr) of transport-limited sediment spreading on land (deltas ~4–6 × 10³
    /// m²/yr; alluvial basins 10³–10⁴).
    pub sediment_diffusivity: f64,
    /// Diffusivity of sediment below sea level relative to [`Self::sediment_diffusivity`]
    /// (marine diffusion is 10–100× slower than on land; Kaufman et al. 1991).
    pub marine_diffusivity_ratio: f64,
    /// Elevation of delta plains above sea level, built when the margin within reach is full.
    pub delta_top: f64,
    /// Whether sediment that does not fit anywhere progrades the shelf over the slope and rise
    /// within reach (filling them to the shelf depth, beyond their caps) before it piles up
    /// on the shelf.
    pub prograde: bool,
    /// Maximum number of shelf, slope and rise cells the sediment of one river mouth spreads
    /// over.
    pub max_margin_cells: usize,
    /// Maximum number of ocean cells a deep-sea fan of one river mouth spreads over.
    pub max_fan_cells: usize,
    /// Delta plains build up on shelf cells at most this many rings from a river mouth.
    pub delta_reach: u32,
    /// Farthest distance (m) that turbidity currents carry sediment from a river mouth into
    /// deep-sea fans, along their path.
    pub max_bypass_distance: f64,
    /// Height (m) that turbidity currents can run up above the lowest point of their path.
    pub fan_climb: f64,
    /// Whether waves erode coasts and rework the sea floor (see `wave_erosion`).
    pub waves: bool,
    /// Storm wave base (m, negative) on shelves open to the ocean, within
    /// [`Self::fetch_rings`] of water deeper than [`Self::open_water_depth`]: waves rework the
    /// sea floor down to this depth (Earth 30–100 m on open shelves; Immenhauser 2009, Peters &
    /// Loss 2012).
    pub wave_base: f64,
    /// Storm wave base (m, negative) in enclosed and epicontinental seas, where waves are
    /// damped over long shallow fetches (Earth ~5–30 m; Shaw 1964, Irwin 1965).
    pub sheltered_wave_base: f64,
    pub open_water_depth: f64,
    pub fetch_rings: u32,
    /// Long-term (10⁵–10⁶ yr) retreat rates (m/yr) of cliffs whose waterline is in sediment
    /// (sedimentary rock) and in crystalline bedrock. Measured rates are cm–dm/yr (Prémaillon
    /// et al. 2018: medians 23 cm/yr weak, 2.9 cm/yr hard rock), but over decades to centuries;
    /// cosmogenic platform ages give several times less over millennia (Hurst et al. 2016;
    /// Swirad et al. 2020), and under a stable sea level the widening shore platform absorbs
    /// the waves and retreat slows (Sunamura 1992; Trenhaile 2000; Anderson et al. 1999).
    /// Averaged over sea-level cycles: ~1–20 mm/yr for sedimentary and ~0.05–1 mm/yr for
    /// crystalline coasts.
    pub cliff_retreat_sediment: f64,
    pub cliff_retreat_bedrock: f64,
    /// Whether a deep-sea fan whose lobes are full aggrades up to its graded surface (see
    /// [`fill_fan`]); otherwise sediment beyond the lobes' thickness cap is refused (the model
    /// before #47, for attribution).
    pub graded_fans: bool,
    /// Fall (m) of the graded surface of a deep-sea fan from its head, at
    /// [`Self::slope_depth`], to its far end; the surface approaches it over
    /// [`Self::fan_length`], so fans are concave: ~3 m/km at the head, below 1 m/km beyond
    /// 1500 km (Bengal, Indus, Amazon and Mississippi fans average 1–5 m/km, steeper on the
    /// upper fan; Curray et al. 2003; Damuth & Flood 1985; Bouma et al. 1985).
    pub fan_relief: f64,
    /// Whether sediment that finds no room in the sea builds a terminal fan or delta around the
    /// river mouth (see [`build_terminal_fan`]); otherwise it is piled on the river's last land
    /// cell (the model before #47, for attribution).
    pub terminal_fans: bool,
    /// Surface slope of terminal fans and prograding deltas: fluvial megafans and delta plains
    /// fall by 10⁻⁴–10⁻³ (Kosi, Okavango, Taquari and Pilcomayo megafans ~2–6 × 10⁻⁴; large
    /// delta plains ~10⁻⁴; Leier et al. 2005; Hartley et al. 2010).
    pub alluvial_slope: f64,
    /// Whether rivers carry the runoff (`P − ET`); otherwise all precipitation (the model before
    /// #7, for attribution).
    pub runoff_discharge: bool,
    /// Whether the implicit incision accounts for the isostatic rebound of the eroded column;
    /// otherwise it assumes the surface drops by the removed thickness (the model before #7, for
    /// attribution).
    pub isostatic_incision: bool,
    /// A closed depression that held no lake in the previous step holds water only if its
    /// deepest floor lies at least this deep (m) below its spill level (#160). Shallower new
    /// depressions are within the elevation noise of one step on the grid (plates resampled
    /// onto the cells, uplift, isostasy) and fill with sediment within the step; their water
    /// passes the spill like that of a filled basin (their sediment is still trapped). A
    /// depression continuing a lake of the previous step holds water down to
    /// [`LAKE_MIN_DEPTH`].
    pub lake_onset_depth: f64,
    /// Whether the surface processes see every crust sample at one elevation (#112, option b;
    /// see [`crate::tectonics::Simulation::sample_surface_elevations`]): cells showing the same
    /// sample are one column, so the lower of two such twins is no pit of its own. Otherwise
    /// each cell has its own surface (the model before #112 b).
    pub sample_surface: bool,
    /// Whether water inside a closed depression fills its sub-basins separately (#168,
    /// fill–spill–merge, see [`basins`]): a sub-basin holds the water draining into it and
    /// spills over its inner sill only when full. Otherwise the water of a depression covers
    /// its cells from the lowest floor up, whatever the inner sills (the model before #168).
    pub lake_hierarchy: bool,
    /// Whether the sea pours over a sill into the basin behind it when the ocean holds more
    /// water than fits below the sill (#168): the basin holds that water below the sill (a lake
    /// fed by the sea) or, once full, joins the ocean. Otherwise the water that would pour in is
    /// not placed (the sea-level solve's deficit; the model before #168).
    pub sea_spill: bool,
    /// Weathering of bare rock into regolith and weathering-limited erosion (#197; see
    /// [`regolith`]).
    pub regolith: RegolithParams,
}

impl Default for ErosionParams {
    fn default() -> Self {
        Self {
            planet_radius: 6.371e6,
            erodibility: 1.5e-7,
            discharge_exponent: 0.5,
            shelf_depth: -120.0,
            slope_depth: -2000.0,
            rise_depth: -3500.0,
            slope_width: 150e3,
            rise_width: 500e3,
            max_shelf_sediment: 10e3,
            max_slope_sediment: 8e3,
            max_rise_sediment: 5e3,
            max_fan_sediment: 1.5e3,
            fan_length: 1000e3,
            sediment_erodibility_factor: 2.0,
            sediment_diffusivity: 5e3,
            marine_diffusivity_ratio: 0.02,
            delta_top: 2.0,
            prograde: false,
            max_margin_cells: 256,
            max_fan_cells: 256,
            delta_reach: 1,
            max_bypass_distance: 3000e3,
            fan_climb: 50.0,
            waves: true,
            wave_base: -50.0,
            sheltered_wave_base: -15.0,
            open_water_depth: -200.0,
            fetch_rings: 2,
            cliff_retreat_sediment: 5e-3,
            cliff_retreat_bedrock: 3e-4,
            graded_fans: true,
            fan_relief: 3000.0,
            terminal_fans: true,
            alluvial_slope: 2e-4,
            runoff_discharge: true,
            isostatic_incision: true,
            lake_onset_depth: 30.0,
            sample_surface: true,
            lake_hierarchy: true,
            sea_spill: true,
            regolith: RegolithParams::default(),
        }
    }
}

impl ErosionParams {
    /// Sets the tunable parameter `name` (the field name) to `value`.
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        if let Some(result) = self.regolith.set(name, value) {
            return result;
        }
        let field = match name {
            "erodibility" => &mut self.erodibility,
            "lake_onset_depth" => &mut self.lake_onset_depth,
            "shelf_depth" => &mut self.shelf_depth,
            "slope_depth" => &mut self.slope_depth,
            "rise_depth" => &mut self.rise_depth,
            "slope_width" => &mut self.slope_width,
            "rise_width" => &mut self.rise_width,
            "max_shelf_sediment" => &mut self.max_shelf_sediment,
            "max_slope_sediment" => &mut self.max_slope_sediment,
            "max_rise_sediment" => &mut self.max_rise_sediment,
            "max_fan_sediment" => &mut self.max_fan_sediment,
            "fan_length" => &mut self.fan_length,
            "sediment_erodibility_factor" => &mut self.sediment_erodibility_factor,
            "sediment_diffusivity" => &mut self.sediment_diffusivity,
            "marine_diffusivity_ratio" => &mut self.marine_diffusivity_ratio,
            "wave_base" => &mut self.wave_base,
            "sheltered_wave_base" => &mut self.sheltered_wave_base,
            "open_water_depth" => &mut self.open_water_depth,
            "cliff_retreat_sediment" => &mut self.cliff_retreat_sediment,
            "cliff_retreat_bedrock" => &mut self.cliff_retreat_bedrock,
            "max_bypass_distance" => &mut self.max_bypass_distance,
            "fan_climb" => &mut self.fan_climb,
            "fan_relief" => &mut self.fan_relief,
            "alluvial_slope" => &mut self.alluvial_slope,
            "graded_fans" => {
                self.graded_fans = value != 0.0;
                return Ok(());
            }
            "terminal_fans" => {
                self.terminal_fans = value != 0.0;
                return Ok(());
            }
            "delta_reach" => {
                self.delta_reach = value as u32;
                return Ok(());
            }
            "waves" => {
                self.waves = value != 0.0;
                return Ok(());
            }
            "lake_hierarchy" => {
                self.lake_hierarchy = value != 0.0;
                return Ok(());
            }
            "sea_spill" => {
                self.sea_spill = value != 0.0;
                return Ok(());
            }
            "fetch_rings" => {
                self.fetch_rings = value as u32;
                return Ok(());
            }
            "prograde" => {
                self.prograde = value != 0.0;
                return Ok(());
            }
            "runoff_discharge" => {
                self.runoff_discharge = value != 0.0;
                return Ok(());
            }
            "isostatic_incision" => {
                self.isostatic_incision = value != 0.0;
                return Ok(());
            }
            "sample_surface" => {
                self.sample_surface = value != 0.0;
                return Ok(());
            }
            "max_margin_cells" | "max_fan_cells" => {
                anyhow::ensure!(value >= 1.0, "{name} must be at least 1");
                let cells = value as usize;
                if name == "max_margin_cells" {
                    self.max_margin_cells = cells;
                } else {
                    self.max_fan_cells = cells;
                }
                return Ok(());
            }
            _ => anyhow::bail!("unknown erosion parameter {name}"),
        };
        *field = value;
        Ok(())
    }
}

/// The kind of crust below a cell, which decides how much sediment it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrustClass {
    /// Crust of continental origin, including stretched margins.
    Continental = 0,
    /// Neither continental nor normal oceanic crust: thinned margins, arcs, plateaus.
    Transitional = 1,
    /// Normal oceanic crust.
    Oceanic = 2,
}

/// Where sediment settles in the ocean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    /// Flooded continental or transitional crust.
    Shelf = 0,
    /// Oceanic crust next to the margin crust of its plate.
    Slope = 1,
    /// Oceanic crust further out.
    Rise = 2,
    /// Oceanic crust far from any margin of its plate.
    Fan = 3,
}

/// Per-cell input of an erosion step.
pub struct Surface<'a> {
    /// Surface elevation relative to sea level (m).
    pub elevation: &'a [f64],
    /// Thickness of loose sediment (m).
    pub sediment: &'a [f64],
    /// Density of the crust below the sediment, for mass (kg/m³).
    pub bedrock_density: &'a [f64],
    /// Density giving the isostatic response of removing crust from the top (see
    /// [`crate::tectonics::Crust::removal_isostatic_density`]).
    pub bedrock_isostatic_density: &'a [f64],
    /// Thickness (m) of the densified lower crust, which removal from the top thins first with
    /// [`Self::bedrock_isostatic_density`]; below it, removal acts with
    /// [`Self::bedrock_density`] (see [`crate::tectonics::Crust::dense_crust_thickness`]).
    pub dense_crust: &'a [f64],
    /// Precipitation (m/yr).
    pub precipitation: &'a [f64],
    /// Runoff of land (m/yr): the precipitation that is not evapotranspired, which feeds the
    /// rivers.
    pub runoff: &'a [f64],
    /// Potential evaporation of open lake water (m/yr; see
    /// [`crate::climate::WaterCycle::lake_potential_evaporation`]).
    pub potential_evaporation: &'a [f64],
    /// The plate the cell belongs to.
    pub plate: &'a [u32],
    /// The kind of crust.
    pub class: &'a [CrustClass],
    /// Volume of the ocean (m³), to solve the sea level (see
    /// [`sea::sea_level`]) as sediment fills it.
    pub ocean_volume: f64,
    /// The seeds of the ocean for this topography (see [`sea::ocean_seeds`]): the ocean is the
    /// water below sea level connected to them.
    pub ocean_seeds: &'a [usize],
    /// The planet's water (m³): the ocean's and the lakes' (see
    /// [`ErosionParams::sea_spill`]).
    pub water_volume: f64,
    /// The cells covered by lakes at the end of the previous step (empty if unknown): a
    /// depression on or next to them continues a lake (see [`ErosionParams::lake_onset_depth`]).
    pub previous_lakes: &'a [bool],
    /// Cells showing one crust sample share an id here (#112 b; empty: every cell is its own
    /// column). The lakes at the end of the step see such cells at their mean level.
    pub sample: &'a [u32],
    /// Annual mean surface temperature (°C), permanent ice cover (0 to 1) and the mafic share
    /// of the bedrock (0 to 1), for the regolith production (empty: the reference temperature,
    /// no ice, felsic rock).
    pub temperature: &'a [f64],
    pub ice: &'a [f64],
    pub mafic: &'a [f64],
    /// Regolith at the bottom of each cell's sediment (rock-equivalent m) from the previous
    /// step (empty: none); clamped to the sediment.
    pub regolith: &'a [f64],
}

/// Changes caused by one erosion step, and diagnostic fields.
pub struct ErosionResult {
    /// Change of the crust (bedrock) thickness of each cell in metres.
    pub bedrock_delta: Vec<f64>,
    /// Change of the sediment thickness of each cell in metres.
    pub sediment_delta: Vec<f64>,
    /// Water discharge through each cell in m³/yr, on the topography at the end of the step.
    pub discharge: Vec<f64>,
    /// Lakes and endorheic basins.
    pub hydrology: Hydrology,
    /// Where the water of every cell goes, and the lakes, for the view (nothing in the
    /// simulation reads it).
    pub water: SurfaceWater,
    /// Total sediment delivered to the ocean, in m³.
    pub sediment_to_ocean: f64,
    /// Sediment piled above the delta top because the ocean within reach was full (thickness
    /// over one cell, summed); it becomes land and is eroded again.
    pub sediment_overfill: f64,
    /// Sediment that found no room in the sea and built terminal fans and prograding deltas
    /// (thickness over one cell, summed).
    pub terminal_fan: f64,
    /// Sediment deposited in the ocean per [`Zone`] (thickness over one cell, summed; deltas
    /// count as shelf, overfill not at all).
    pub deposited: [f64; 4],
    /// Change of the sea level by the step (solved on the cells).
    pub sea_level_change: SurfaceSeaLevel,
    /// Sediment moved by waves.
    pub wave: WaveErosion,
    /// Regolith at the bottom of each cell's sediment at the start of the step as the step saw
    /// it ([`Surface::regolith`] clamped to the sediment, zero at sea) and at its end
    /// (rock-equivalent m, at most the sediment; zero without regolith).
    pub regolith_start: Vec<f64>,
    pub regolith: Vec<f64>,
    /// Fraction of the step each land cell's rock was bare (0 where a deposit or lake covered
    /// it, and for the sea).
    pub bare: Vec<f64>,
    /// Regolith production on bare rock `P₀` (m/yr of bedrock) of each land cell.
    pub production: Vec<f64>,
    pub regolith_stats: RegolithStats,
}

/// Water balance of the land in one step.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hydrology {
    /// Runoff of all land (m³/yr).
    pub runoff: f64,
    /// Water reaching the ocean (m³/yr): the runoff less the evaporation of lakes.
    pub to_ocean: f64,
    /// Evaporation of lakes in excess of the evapotranspiration of the land they cover (m³/yr).
    pub lake_evaporation: f64,
    /// Land cells whose water ends in an endorheic lake (cells).
    pub endorheic_cells: f64,
    /// Land covered by lakes, spilling or endorheic (cells, fractional).
    pub lake_cells: f64,
    /// Land cells.
    pub land_cells: f64,
    /// Sediment trapped in lakes (m³/yr, rock equivalent).
    pub lake_sediment: f64,
    /// Closed depressions (with or without water) and their cells.
    pub depressions: usize,
    pub depression_cells: usize,
    /// Endorheic depressions whose water covers less than a tenth of their area (salt flats,
    /// playas), and their cells.
    pub playas: usize,
    pub playa_cells: usize,
    /// Depressions filled up to their spill level by sediment in this step, and their cells.
    pub filled_depressions: usize,
    pub filled_depression_cells: usize,
    /// Depressions holding water (lakes, also nearly dry endorheic basins; see
    /// [`Self::depressions`] for all), those that do not spill, and those with more than one
    /// local minimum (inner sills; their water is treated as one lake, a known simplification).
    pub lakes: usize,
    pub endorheic_lakes: usize,
    pub nested_lakes: usize,
    /// Potential evaporation of lake water summed over the covered cells (m/yr · cells); of the
    /// ice-free months only (#126).
    pub lake_potential_evaporation: f64,
    /// Endorheic land draining into single-cell depressions (cells).
    pub endorheic_pit_cells: f64,
    /// Land below sea level: closed depressions that the ocean does not reach (cells).
    pub inland_cells: f64,
    /// The part of [`Self::inland_cells`] covered by lake water (cells, fractional).
    pub inland_water_cells: f64,
    /// Depressions reaching below sea level, and those of them that are dry (lake water covers
    /// less than a tenth of a cell).
    pub inland_depressions: usize,
    pub dry_inland_depressions: usize,
    /// Water in lakes (m³; by the surface elevations, without the load of the water), sea
    /// water poured into basins below their sill included.
    pub lake_volume: f64,
    /// Separate water bodies (not empty) in the depressions holding water (#168: sub-basins
    /// below inner sills hold their own lakes), and the depressions holding more than one.
    pub lake_bodies: usize,
    pub split_lakes: usize,
    /// Lakes fed by the sea over their sill (#168, see [`ErosionParams::sea_spill`]), the cells
    /// they cover, the sea water in them (m³) and their evaporation in excess of their own
    /// inflow (m³/yr), which the sea supplies.
    pub sea_fed_lakes: usize,
    pub sea_fed_cells: f64,
    pub sea_water: f64,
    pub sea_fed_evaporation: f64,
    /// Endorheic land draining into lakes fed by the sea (cells).
    pub endorheic_sea_fed_cells: f64,
    /// The water (m³) the ocean held above the sill of a basin it could not fill at the end of
    /// the step and poured into that basin (the sea-level solve's deficit); zero if none, or if
    /// the basin joined the ocean.
    pub sea_spill_deficit: f64,
}

/// Where the water of every cell goes, the ocean, closed depressions and lakes, as the erosion
/// step saw them (before its changes of the surface), for the hydrology render mode (#102).
#[derive(Clone, Debug, Default)]
pub struct SurfaceWater {
    /// The neighbour the water of a cell flows to: the receiver of the drainage, but none for
    /// ocean cells and for the exit of an endorheic lake, whose water does not leave.
    pub receiver: Vec<Option<u32>>,
    /// The ocean: the water below sea level connected to the world ocean (see [`sea`]).
    pub ocean: Vec<bool>,
    /// Land in a closed depression (at least [`LAKE_MIN_DEPTH`] below its spill level), dry
    /// or under a lake.
    pub depression: Vec<bool>,
    /// Depth of lake water over the cell's floor (m), zero where there is no lake. The floor is
    /// pressed down by the water's own load (see [`lake_water_depth`]). It is the floor of the
    /// surface at the end of the step as the erosion sees it: with
    /// [`ErosionParams::sample_surface`], cells showing one crust sample share one floor (their
    /// mean), so on the per-cell surface their water surfaces differ by the dynamic topography
    /// between them (a few metres; #112 b).
    pub lake_depth: Vec<f64>,
    /// Depth of the cell's floor below the spill level of its depression (m), zero outside
    /// depressions; of the floor without the load of lake water.
    pub spill_depth: Vec<f64>,
    /// Correction of the surface elevation (m) for the water that actually stands on the cell
    /// (#127): the elevations of the processes load every cell below sea level with the ocean
    /// and none above it; a lake presses its floor down by its own water, a dry closed
    /// depression below sea level carries no load. Zero elsewhere (and where unknown).
    pub load_correction: Vec<f64>,
    /// The depressions at the start of the erosion step, before its deposition (for analysis).
    pub depression_at_start: Vec<bool>,
    /// Evaporation of lake water in excess of the land's evapotranspiration (m/yr over the mean
    /// cell area), as the water balance removes it from the runoff; times the cell area it sums
    /// to [`Hydrology::lake_evaporation`]. The climate returns it to the air in the following
    /// steps, as it refreshes its months in turn (#126).
    pub lake_evaporation: Vec<f64>,
}

impl SurfaceWater {
    /// The ocean alone, without rivers or lakes (while erosion is disabled).
    pub fn ocean_only(ocean: Vec<bool>) -> Self {
        let cells = ocean.len();
        Self {
            receiver: vec![None; cells],
            ocean,
            depression: vec![false; cells],
            lake_depth: vec![0.0; cells],
            spill_depth: vec![0.0; cells],
            depression_at_start: vec![false; cells],
            load_correction: vec![0.0; cells],
            lake_evaporation: vec![0.0; cells],
        }
    }
}

/// Sediment moved by waves in one step, as rock-equivalent thickness over one cell, summed.
#[derive(Clone, Copy, Debug, Default)]
pub struct WaveErosion {
    /// Sediment eroded from coastal cliffs.
    pub cliff_sediment: f64,
    /// Sediment made from bedrock eroded at coastal cliffs.
    pub cliff_bedrock: f64,
    /// Sediment reworked from the sea floor above the wave base.
    pub reworked: f64,
    /// The part of all of it that found no room below the wave base within reach and stayed.
    pub stayed: f64,
}

/// Performs one erosion step of `dt_years` years.
pub fn erode(
    mesh: &SphereMesh,
    surface: &Surface,
    params: &ErosionParams,
    dt_years: f64,
) -> ErosionResult {
    let Surface {
        elevation,
        sediment,
        bedrock_density,
        bedrock_isostatic_density,
        dense_crust,
        plate,
        class,
        ocean_volume,
        ocean_seeds,
        sample,
        runoff,
        ..
    } = *surface;
    let cell_count = mesh.len();
    let cell_area = 4.0 * std::f64::consts::PI * params.planet_radius.powi(2) / cell_count as f64;
    // The ocean: water below sea level connected to the world ocean. Closed depressions below
    // sea level are land, dry or holding lakes (#58).
    let ocean = sea::ocean_from(
        mesh,
        elevation,
        ocean_seeds.iter().copied(),
        !params.sea_spill,
    );
    let is_ocean = |cell: usize| ocean[cell];

    let WaterState {
        receiver,
        filled,
        order,
        lakes,
        discharge,
        water_level,
        mut hydrology,
        ..
    } = water_state(mesh, elevation, &ocean, surface, params, cell_area, &[]);
    // The routing and deposition below work on the processes' elevations, which load closed
    // depressions below sea level with the ocean: spill and water levels in that frame.
    let to_process = |levels: Vec<f64>| -> Vec<f64> {
        levels
            .into_iter()
            .enumerate()
            .map(|(cell, level)| {
                if ocean[cell] {
                    // The drainage starts from the ocean cells at their own elevation.
                    elevation[cell]
                } else {
                    isostasy::to_surface(level)
                }
            })
            .collect()
    };
    let filled = to_process(filled);
    let water_level = to_process(water_level);

    // Implicit stream-power incision, from the ocean upstream so receivers are updated first.
    let mut new_elevation = elevation.to_vec();
    let mut bedrock_delta = vec![0.0; cell_count];
    let mut sediment_delta = vec![0.0; cell_count];
    let mut produced = vec![0.0; cell_count];
    let weathering = &params.regolith;
    let mut regolith = vec![0.0; cell_count];
    let mut bare = vec![0.0; cell_count];
    let mut production = vec![0.0; cell_count];
    let mut regolith_stats = RegolithStats::default();
    let mut regolith_start = vec![0.0; cell_count];
    if weathering.enabled {
        for cell in 0..cell_count {
            if ocean[cell] {
                continue;
            }
            regolith[cell] = surface
                .regolith
                .get(cell)
                .map_or(0.0, |&r| r.clamp(0.0, sediment[cell]));
            regolith_start[cell] = regolith[cell];
            production[cell] = weathering.production_scale(
                surface
                    .temperature
                    .get(cell)
                    .copied()
                    .unwrap_or(weathering.reference_temperature),
                runoff.get(cell).copied().unwrap_or(0.0),
                surface.ice.get(cell).copied().unwrap_or(0.0),
                surface.mafic.get(cell).copied().unwrap_or(0.0),
            );
        }
    }
    for &cell in &order {
        let cell = cell as usize;
        let Some(receiver) = receiver[cell] else {
            continue;
        };
        // Lake floors and the floors of endorheic basins collect sediment; they are not incised.
        if lakes.lake[cell] != NO_LAKE {
            continue;
        }
        // Rivers cannot cut below the water surface they flow into: the sea, a lake, or the
        // floor of a dry depression.
        let receiver = receiver as usize;
        let base = if is_ocean(receiver) {
            new_elevation[receiver].max(0.0)
        } else {
            new_elevation[receiver].max(water_level[receiver])
        };
        if weathering.enabled {
            regolith_stats.land_cells += 1;
            let rate = if elevation[cell] <= base {
                0.0
            } else {
                let distance =
                    math::angle_between(mesh.vertex(cell as u32), mesh.vertex(receiver as u32))
                        * params.planet_radius;
                params.erodibility * math::powf(discharge[cell], params.discharge_exponent)
                    / distance
            };
            let bedrock = Bedrock {
                dense: dense_crust[cell],
                dense_density: bedrock_isostatic_density[cell],
                density: bedrock_density[cell],
            };
            let weathered = weather(
                weathering,
                (elevation[cell], base),
                (sediment[cell], regolith[cell]),
                &bedrock,
                (production[cell], rate, params.sediment_erodibility_factor),
                dt_years,
                params.isostatic_incision,
            );
            let step = &weathered.step;
            let left = weathered.deposit + step.regolith;
            sediment_delta[cell] += left - sediment[cell];
            bedrock_delta[cell] -= step.bedrock;
            // What leaves the cell: the eroded deposit and regolith, the same rock mass.
            produced[cell] = (sediment[cell] - left
                + step.bedrock * bedrock_density[cell] / SEDIMENT_ROCK_DENSITY)
                .max(0.0);
            regolith[cell] = step.regolith;
            bare[cell] = step.exposure / dt_years;
            if weathered.transported {
                if step.transport_limited {
                    regolith_stats.transport_limited_cells += 1;
                    regolith_stats.transport_limited_bedrock += step.bedrock / dt_years;
                } else {
                    regolith_stats.weathering_limited_cells += 1;
                    regolith_stats.weathering_limited_bedrock += step.bedrock / dt_years;
                }
            } else {
                regolith_stats.untransported_bedrock += step.bedrock / dt_years;
            }
            let without_sediment = isostasy::change_sediment(elevation[cell], sediment[cell], left);
            let from_dense = step.bedrock.min(dense_crust[cell]);
            new_elevation[cell] = isostasy::add_layer(
                isostasy::add_layer(
                    without_sediment,
                    -from_dense,
                    bedrock_isostatic_density[cell],
                ),
                -(step.bedrock - from_dense),
                bedrock_density[cell],
            );
            continue;
        }
        if elevation[cell] <= base {
            continue; // Flat: no incision.
        }
        let distance = math::angle_between(mesh.vertex(cell as u32), mesh.vertex(receiver as u32))
            * params.planet_radius;
        let rate =
            params.erodibility * math::powf(discharge[cell], params.discharge_exponent) / distance;
        let Incision {
            bulk: from_bulk,
            bedrock: from_bedrock,
        } = incise(
            elevation[cell],
            base,
            sediment[cell],
            &Bedrock {
                dense: dense_crust[cell],
                dense_density: bedrock_isostatic_density[cell],
                density: bedrock_density[cell],
            },
            (rate * params.sediment_erodibility_factor, rate),
            dt_years,
            params.isostatic_incision,
        );

        // The top of the column is removed: sediment (bulk, compacted) first.
        let bulk = isostasy::sediment_bulk_thickness(sediment[cell]);
        let left = isostasy::sediment_for_bulk_thickness(bulk - from_bulk);
        let from_sediment = sediment[cell] - left;
        sediment_delta[cell] -= from_sediment;
        bedrock_delta[cell] -= from_bedrock;
        // Eroded bedrock becomes sediment of the same rock mass.
        produced[cell] =
            from_sediment + from_bedrock * bedrock_density[cell] / SEDIMENT_ROCK_DENSITY;
        // The column rebounds isostatically, so the surface drops less than the removed
        // thickness.
        let without_sediment = isostasy::change_sediment(elevation[cell], sediment[cell], left);
        let from_dense = from_bedrock.min(dense_crust[cell]);
        new_elevation[cell] = isostasy::add_layer(
            isostasy::add_layer(
                without_sediment,
                -from_dense,
                bedrock_isostatic_density[cell],
            ),
            -(from_bedrock - from_dense),
            bedrock_density[cell],
        );
    }

    // The sea level (relative to the current one) before the step, to measure its changes;
    // zero up to rounding.
    let initial_sea_level = sea::relative_sea_level_from(
        mesh,
        elevation,
        sediment,
        cell_area,
        ocean_volume,
        ocean_seeds,
    );
    let mut seabed = Seabed {
        sediment: sediment
            .iter()
            .zip(&sediment_delta)
            .map(|(s, d)| s + d)
            .collect(),
        elevation: new_elevation,
        delta: sediment_delta,
    };

    // Route sediment downstream. Lakes trap what their rivers bring (see `fill_lake`); what
    // does not fit passes the outlet.
    let mut flux = produced;
    let mut delivered = vec![0.0; cell_count];
    // The last land cell of a river reaching each mouth (u32::MAX for none).
    let mut source = vec![u32::MAX; cell_count];
    for &cell in order.iter().rev() {
        let cell = cell as usize;
        if is_ocean(cell) {
            continue;
        }
        if let Some(lake) = lakes.exit_of(cell) {
            // The pass may have been incised in this step.
            let spill = receiver[cell].map_or(filled[cell], |pass| {
                filled[cell].min(seabed.elevation[pass as usize])
            });
            let (trapped, full) = fill_lake(&mut seabed, &lakes.cells[lake], spill, flux[cell]);
            if full {
                hydrology.filled_depressions += 1;
                hydrology.filled_depression_cells += lakes.cells[lake].len();
            }
            hydrology.lake_sediment += trapped * cell_area / dt_years;
            flux[cell] -= trapped;
        }
        let remaining = flux[cell];
        match receiver[cell] {
            Some(receiver) if is_ocean(receiver as usize) => {
                delivered[receiver as usize] += remaining;
                source[receiver as usize] = cell as u32;
            }
            Some(receiver) => flux[receiver as usize] += remaining,
            None => {
                // A land cell without receiver only exists on planets without ocean.
                seabed.deposit(cell, remaining);
            }
        }
    }

    let zones = zones(mesh, plate, class, params);

    let mut sediment_to_ocean = 0.0;
    let mut sediment_overfill = 0.0;
    let mut terminal_fan = 0.0;
    let mut deposited = [0.0; 4];
    let mut margin_mark = vec![usize::MAX; cell_count];
    let mut fan_mark = vec![usize::MAX; cell_count];
    let mut queue = VecDeque::new();
    let mut fan_queue = VecDeque::new();
    let mut margin = Vec::new();
    let mut water = Vec::new();
    let mut fan_scratch = FanScratch::new(cell_count);

    // The shelf is graded: next to the coast it fills up to the wave base (the inner shelf),
    // further out up to the shelf break (Harris et al. 2014: ~130 m).
    let wave_base = wave_bases_in(mesh, elevation, &ocean, params);
    let shelf_target: Vec<f64> = (0..cell_count)
        .map(|cell| {
            let coastal = mesh
                .neighbors(cell as u32)
                .iter()
                .any(|&m| !ocean[m as usize]);
            if coastal {
                wave_base[cell].max(params.shelf_depth)
            } else {
                params.shelf_depth
            }
        })
        .collect();

    // Pass A: river sediment fills the marine accommodation (margins and open-sea fans).
    let mut leftovers = Vec::new();
    for (mouth, &delivered) in delivered.iter().enumerate() {
        let mut remaining = delivered;
        if remaining <= 0.0 {
            continue;
        }
        sediment_to_ocean += remaining * cell_area;
        let source_cell = source[mouth] as usize;
        let own_plate = plate[source_cell];
        margin_within_reach(
            mesh,
            mouth,
            &zones,
            params,
            &is_ocean,
            &mut margin_mark,
            &mut queue,
            &mut margin,
        );
        // Fill the margin up to the depth of each zone, nearest cells first, so that shelves
        // prograde: the river's own plate first, other plates' margins once it is full.
        for own in [true, false] {
            for &cell in &margin {
                if remaining <= 0.0 {
                    break;
                }
                if (plate[cell] == own_plate) != own {
                    continue;
                }
                let (target, cap) = match zones[cell] {
                    // Deeply buried sediment on continental crust metamorphoses (see
                    // `Simulation::metamorphose_sediment`), so it needs no cap.
                    Zone::Shelf if class[cell] == CrustClass::Continental => {
                        (shelf_target[cell], f64::INFINITY)
                    }
                    Zone::Shelf => (shelf_target[cell], params.max_shelf_sediment),
                    Zone::Slope => (params.slope_depth, params.max_slope_sediment),
                    Zone::Rise => (params.rise_depth, params.max_rise_sediment),
                    Zone::Fan => continue,
                };
                let thickness = remaining.min(seabed.room(cell, target, cap));
                if thickness > 0.0 {
                    seabed.deposit(cell, thickness);
                    deposited[zones[cell] as usize] += thickness;
                    remaining -= thickness;
                }
            }
        }

        // What does not fit bypasses the margin as turbidity currents down the slope into a
        // deep-sea fan (see `fill_fan`).
        if remaining > 0.0 {
            let fan = fill_fan(
                mesh,
                &mut seabed,
                &zones,
                params,
                mouth,
                remaining,
                &mut fan_scratch,
            );
            deposited[Zone::Fan as usize] += fan;
            remaining -= fan;
        }
        if remaining > 0.0 {
            leftovers.push((mouth, source_cell, remaining));
        }
    }

    // The deposits displace water: the coastal processes below work against the sea level
    // they raised, so that delta plains are not built for a sea level that their own sediment
    // floods (all elevations are shifted; land lower than the rise becomes sea).
    let fluvial = seabed.sea_level(mesh, cell_area, ocean_volume, ocean_seeds) - initial_sea_level;
    seabed.shift_sea_level(fluvial);
    // The sea now also covers the land lower than its rise that it reaches.
    let risen: Vec<f64> = (0..cell_count)
        .map(|cell| {
            if ocean[cell] {
                f64::NEG_INFINITY
            } else {
                seabed.elevation[cell]
            }
        })
        .collect();
    let reached = sea::ocean_from(
        mesh,
        &risen,
        (0..cell_count).filter(|&cell| ocean[cell]),
        !params.sea_spill,
    );

    // Pass B: what did not fit fills small enclosed seas, builds delta plains and piles up.
    for &(mouth, source_cell, mut remaining) in &leftovers {
        let is_ocean = |cell: usize| ocean[cell] || (reached[cell] && seabed.elevation[cell] < 0.0);
        let open = water_within_reach(
            mesh,
            mouth,
            params,
            &is_ocean,
            &mut fan_mark,
            &mut fan_queue,
            &mut water,
        );
        // A small enclosed sea or depression fills up completely, nearest cells first.
        if !open {
            for &(cell, _) in &water {
                if remaining <= 0.0 {
                    break;
                }
                let thickness = remaining.min(seabed.room(cell, params.delta_top, f64::INFINITY));
                if thickness > 0.0 {
                    seabed.deposit(cell, thickness);
                    deposited[zones[cell] as usize] += thickness;
                    remaining -= thickness;
                }
            }
        }

        // When the margin and the fans within reach are full, a delta plain builds up slightly
        // above sea level on the shelf around the mouth, at most [`ErosionParams::delta_reach`]
        // rings wide (Earth's largest delta plains are ~10⁵ km²). What is left piles up on the
        // river's last land cell: it is land, and rivers and waves carry it on later.
        let is_ocean = |cell: usize| ocean[cell] || (reached[cell] && seabed.elevation[cell] < 0.0);
        water.clear();
        water.push((mouth, 0));
        fan_scratch.mark[mouth] = mouth;
        let mut next = 0;
        while next < water.len() {
            let (cell, hops) = water[next];
            next += 1;
            if hops >= params.delta_reach {
                continue;
            }
            for &neighbor in mesh.neighbors(cell as u32) {
                let neighbor = neighbor as usize;
                if is_ocean(neighbor) && fan_scratch.mark[neighbor] != mouth {
                    fan_scratch.mark[neighbor] = mouth;
                    water.push((neighbor, hops + 1));
                }
            }
        }
        for &(cell, _) in &water {
            fan_scratch.mark[cell] = usize::MAX;
        }
        for &(cell, _) in &water {
            if remaining <= 0.0 {
                break;
            }
            if zones[cell] != Zone::Shelf {
                continue;
            }
            let thickness = remaining.min(seabed.room(cell, params.delta_top, f64::INFINITY));
            if thickness > 0.0 {
                seabed.deposit(cell, thickness);
                deposited[Zone::Shelf as usize] += thickness;
                remaining -= thickness;
            }
        }
        if remaining > 0.0 && params.terminal_fans {
            let fan = build_terminal_fan(
                mesh,
                &mut seabed,
                params,
                mouth,
                remaining,
                &mut fan_scratch,
            );
            terminal_fan += fan;
            remaining -= fan;
        }
        if remaining > 0.0 {
            seabed.deposit(source_cell, remaining);
            sediment_overfill += remaining;
        }
    }

    let ocean = sea::ocean_from(
        mesh,
        &seabed.elevation,
        ocean_seeds.iter().copied(),
        !params.sea_spill,
    );
    let fixed: Vec<f64> = (0..cell_count)
        .map(|cell| if ocean[cell] { 0.0 } else { regolith[cell] })
        .collect();
    diffuse_sediment(mesh, &mut seabed, (&ocean, &fixed), params, dt_years);

    let wave = if params.waves {
        wave_erosion(
            (mesh, ocean_seeds),
            &mut seabed,
            &mut bedrock_delta,
            (bedrock_density, bedrock_isostatic_density),
            (&zones, class),
            params,
            dt_years,
        )
    } else {
        WaveErosion::default()
    };

    // Relative to the shifted sea level, less the sea level before the step (not zero when the
    // erosion sees another surface than the tectonics' sea-level solve, #112 b).
    let coastal = seabed.sea_level(mesh, cell_area, ocean_volume, ocean_seeds) - initial_sea_level;

    // The lakes at the end of the step (#160): the depressions the topography had at its start
    // trapped the step's sediment, and a shallow one fills up within much less than a step
    // (a 15 m deep basin of one cell takes ~200 km³, a fraction of what a few eroding cells
    // deliver per Myr). Only what is left of the depressions after the deposition holds
    // water, with the water balance on the new floors. The routing above used the depressions
    // at the start, which is where the water and sediment went during the step.
    // Cells showing one crust sample are one column (#112 b): the step changed them separately,
    // but the sample takes the mean of their changes, so the lakes at the end see them at their
    // mean level (in dry elevation, so that the water load stays right).
    let end_elevation = if sample.is_empty() {
        seabed.elevation.clone()
    } else {
        sample_means(&seabed.elevation, sample)
    };
    let end_ocean = sea::ocean_from(
        mesh,
        &end_elevation,
        ocean_seeds.iter().copied(),
        !params.sea_spill,
    );
    let end = water_state(
        mesh,
        &end_elevation,
        &end_ocean,
        surface,
        params,
        cell_area,
        &[],
    );
    let end = if params.sea_spill {
        spill_sea(
            (mesh, &end_elevation, &seabed.sediment),
            end_ocean,
            end,
            surface,
            params,
            cell_area,
        )
    } else {
        end
    };
    // The regolith is the bottom of the sediment: what the deposition, diffusion and waves left.
    for (regolith, &sediment) in regolith.iter_mut().zip(&seabed.sediment) {
        *regolith = regolith.min(sediment.max(0.0));
    }
    let mut water = end.surface_water;
    water.depression_at_start = lakes.lake.iter().map(|&lake| lake != NO_LAKE).collect();
    let hydrology = Hydrology {
        lake_sediment: hydrology.lake_sediment,
        filled_depressions: hydrology.filled_depressions,
        filled_depression_cells: hydrology.filled_depression_cells,
        ..end.hydrology
    };
    ErosionResult {
        bedrock_delta,
        sediment_delta: seabed.delta,
        // For the view and statistics, consistent with the lakes and receivers at the end.
        discharge: end.discharge,
        hydrology,
        water,
        sediment_to_ocean,
        sediment_overfill,
        terminal_fan,
        deposited,
        sea_level_change: SurfaceSeaLevel { fluvial, coastal },
        wave,
        regolith_start,
        regolith,
        bare,
        production,
        regolith_stats,
    }
}

/// The sea pours over a sill into the basin behind it (#168, see [`ErosionParams::sea_spill`]):
/// if the ocean, holding the planet's water less that of the lakes of `state`, stands at the
/// pass into a basin it cannot fill (the sea-level solve's deficit), that water flows into the
/// basin and stays there below the sill, as a lake fed by the sea. If the basin is full before,
/// it joins the ocean. Returns the water state with these changes, on the surface `elevation`
/// (relative to sea level, with `sediment`) whose ocean was `ocean`.
fn spill_sea(
    (mesh, elevation, sediment): (&SphereMesh, &[f64], &[f64]),
    mut ocean: Vec<bool>,
    mut state: WaterState,
    surface: &Surface,
    params: &ErosionParams,
    cell_area: f64,
) -> WaterState {
    let seeds = surface.ocean_seeds;
    let dry: Vec<f64> = elevation.iter().map(|&e| isostasy::to_dry(e)).collect();
    let pores: Vec<f64> = sediment
        .iter()
        .map(|&s| isostasy::sediment_pores(s))
        .collect();
    // At most: basins the sea reaches join it, a basin on whose sill the sea stands leaves the
    // ocean (rounding at the sill), then it takes the sea water.
    for _ in 0..4 {
        let volume = surface.water_volume - state.hydrology.lake_volume;
        let solved = sea::sea_level_from(mesh, &dry, &pores, cell_area, volume, seeds);
        let level = solved.level;
        let reachable = (0..dry.len()).any(|c| state.lakes.lake[c] != NO_LAKE && dry[c] < level);
        if solved.deficit <= 0.0 && !reachable {
            break;
        }
        let pass = sea::pass_levels(mesh, &dry, seeds);
        // Closed depressions below the solved sea level behind lower sills: the sea floods them
        // (their lakes become sea), with the cells between them and the sea below that level
        // (sills, rims too shallow to count as depression).
        let below = |c: usize| !ocean[c] && pass[c] < level && dry[c] < level;
        let mut flooded: Vec<usize> = (0..dry.len())
            .filter(|&c| state.lakes.lake[c] != NO_LAKE && below(c))
            .collect();
        let mut marked = vec![false; dry.len()];
        for &c in &flooded {
            marked[c] = true;
        }
        let mut next = 0;
        while next < flooded.len() {
            let cell = flooded[next];
            next += 1;
            for &neighbor in mesh.neighbors(cell as u32) {
                let n = neighbor as usize;
                if !marked[n] && below(n) {
                    marked[n] = true;
                    flooded.push(n);
                }
            }
        }
        if !flooded.is_empty() {
            for &c in &flooded {
                ocean[c] = true;
            }
            state = water_state(mesh, elevation, &ocean, surface, params, cell_area, &[]);
            continue;
        }
        if solved.deficit <= 0.0 {
            break;
        }
        // The basin behind the pass on which the sea stands: the cells the ocean reaches only
        // over it, below it (none if only the pores of a cell at the pass fill).
        let basin: Vec<usize> = (0..dry.len())
            .filter(|&c| pass[c] == level && dry[c] < level)
            .collect();
        if basin.is_empty() {
            break;
        }
        if basin.iter().any(|&c| ocean[c]) {
            // The sill lies just below sea level in this frame (rounding, the sea-level change
            // of the step, or the sample surface's means; #112 b): the sea stands at it, so the
            // basin behind it is not ocean.
            for (c, ocean) in ocean.iter_mut().enumerate() {
                if pass[c] == level {
                    *ocean = false;
                }
            }
            state = water_state(mesh, elevation, &ocean, surface, params, cell_area, &[]);
            continue;
        }
        // The depressions holding the basin's cells take the water, the lowest first (one
        // basin can be split by cells less than LAKE_MIN_DEPTH below the spill level).
        let mut targets: Vec<usize> = Vec::new();
        let mut by_floor = basin.clone();
        by_floor.sort_by(|&a, &b| dry[a].total_cmp(&dry[b]));
        for c in by_floor {
            let lake = state.lakes.lake[c];
            if lake != NO_LAKE && !targets.iter().any(|&t| state.lakes.lake[t] == lake) {
                targets.push(c);
            }
        }
        let deficit = solved.deficit;
        let mut pours: Vec<(usize, f64)> = Vec::new();
        let mut left = deficit;
        let mut poured = None;
        for &target in &targets {
            pours.push((target, left));
            let next = water_state(mesh, elevation, &ocean, surface, params, cell_area, &pours);
            left = next.sea_leftover;
            pours.last_mut().expect("just pushed").1 -= left;
            poured = Some(next);
            if left <= 0.0 {
                break;
            }
        }
        tracing::debug!(
            deficit,
            level,
            basin = basin.len(),
            in_lakes = basin
                .iter()
                .filter(|&&c| state.lakes.lake[c] != NO_LAKE)
                .count(),
            targets = targets.len(),
            left,
            lowest = basin.iter().map(|&c| dry[c]).fold(f64::INFINITY, f64::min),
            "Sea poured over a sill"
        );
        let Some(poured) = poured else {
            break;
        };
        // Water that found no room: the sea-level solve also fills the pores of the basin's
        // sediment, which the lakes do not count, and the cells less than LAKE_MIN_DEPTH below
        // the spill level are left out of the depressions; that water stays in the basin
        // (counted with the lakes' water). More means the basin is full: it joins the ocean,
        // which then rises above the sill.
        let unseen: f64 = basin
            .iter()
            .map(|&c| pores[c] + LAKE_MIN_DEPTH * isostasy::WATER_LOADING)
            .sum::<f64>()
            * cell_area;
        if left > unseen {
            for &c in &basin {
                ocean[c] = true;
            }
            state = water_state(mesh, elevation, &ocean, surface, params, cell_area, &[]);
        } else {
            state = poured;
            state.hydrology.lake_volume += left;
            state.hydrology.sea_water += left;
            state.hydrology.sea_spill_deficit = deficit;
        }
        break;
    }
    state
}

/// `elevation` (m, relative to sea level) with the cells of each `sample` id at their mean dry
/// elevation (see [`isostasy::to_dry`]); ids are below the cell count.
fn sample_means(elevation: &[f64], sample: &[u32]) -> Vec<f64> {
    debug_assert_eq!(sample.len(), elevation.len());
    let mut sums = vec![(0.0, 0u32); elevation.len()];
    for (&e, &id) in elevation.iter().zip(sample) {
        let sum = &mut sums[id as usize];
        sum.0 += isostasy::to_dry(e);
        sum.1 += 1;
    }
    elevation
        .iter()
        .zip(sample)
        .map(|(&e, &id)| match sums[id as usize] {
            (_, 1) => e,
            (dry, count) => isostasy::to_surface(dry / f64::from(count)),
        })
        .collect()
}

/// The water of the land on a topography: drainage, closed depressions, discharge and the
/// water balance of the lakes in them, with the hydrology statistics and the surface water for
/// the view.
struct WaterState {
    receiver: Vec<Option<u32>>,
    filled: Vec<f64>,
    order: Vec<u32>,
    lakes: Lakes,
    /// Water discharge through each cell (m³/yr).
    discharge: Vec<f64>,
    /// Water surface of the cells covered by lakes (negative infinity elsewhere).
    water_level: Vec<f64>,
    hydrology: Hydrology,
    surface_water: SurfaceWater,
    /// Sea water (m³) that did not fit below the spill level of its depression.
    sea_leftover: f64,
}

/// Computes the [`WaterState`] of the land on `elevation` with the ocean `ocean`.
///
/// `sea_water` pours sea water (m³) over the spill level into the depressions holding the given
/// cells (see [`ErosionParams::sea_spill`]); what does not fit is
/// [`WaterState::sea_leftover`].
fn water_state(
    mesh: &SphereMesh,
    elevation: &[f64],
    ocean: &[bool],
    surface: &Surface,
    params: &ErosionParams,
    cell_area: f64,
    sea_water: &[(usize, f64)],
) -> WaterState {
    let Surface {
        precipitation,
        runoff,
        potential_evaporation,
        previous_lakes,
        ..
    } = *surface;
    let cell_count = mesh.len();
    let is_ocean = |cell: usize| ocean[cell];
    // The water of the land stands on the unloaded floors (#127): the processes' elevations load
    // every cell below sea level with the ocean (see `isostasy::to_surface`), also closed
    // depressions that the ocean does not reach. The conversion keeps the order of the
    // elevations, so the drainage and the depressions are the same. With
    // [`ErosionParams::sea_spill`] the ocean cells are unloaded too: a basin on whose sill the
    // sea stands is land even where that sill lies below the sea level of this frame (the sea
    // level of the step's end, see `spill_sea`), and its spill level is the sill, not the
    // loaded floor of the sea next to it.
    let process = elevation;
    let unloaded: Vec<f64> = (0..cell_count)
        .map(|cell| {
            if is_ocean(cell) && !params.sea_spill {
                process[cell]
            } else {
                isostasy::to_dry(process[cell])
            }
        })
        .collect();
    let elevation = &unloaded[..];
    // Cells on or next to last step's lakes: lakes move with their plates by up to about half a
    // cell per step.
    let mut continuing = vec![false; cell_count];
    for (cell, _) in previous_lakes.iter().enumerate().filter(|&(_, &lake)| lake) {
        continuing[cell] = true;
        for &neighbor in mesh.neighbors(cell as u32) {
            continuing[neighbor as usize] = true;
        }
    }
    let Drainage {
        receiver,
        filled,
        order,
    } = drainage(mesh, elevation, ocean);
    let lakes = Lakes::new(&order, &receiver, &filled, elevation, ocean);

    // Discharge of the runoff, accumulated from the sources downstream. Lakes evaporate water:
    // a lake whose evaporation at its spill level exceeds its inflow does not spill
    // (endorheic), its water surface stays where evaporation balances the inflow.
    let mut discharge = vec![0.0; cell_count];
    let mut hydrology = Hydrology::default();
    for cell in 0..cell_count {
        if !is_ocean(cell) {
            if elevation[cell] < 0.0 {
                hydrology.inland_cells += 1.0;
            }
            let source = if params.runoff_discharge {
                runoff[cell]
            } else {
                precipitation[cell]
            };
            discharge[cell] = source.max(0.0) * cell_area;
            hydrology.runoff += discharge[cell];
            hydrology.land_cells += 1.0;
        }
    }
    // Water surface of the cells covered by lakes (negative infinity elsewhere).
    let mut water_level = vec![f64::NEG_INFINITY; cell_count];
    // Excess evaporation of the lake water over each cell (m³/yr).
    let mut lake_evaporation = vec![0.0; cell_count];
    let mut endorheic = vec![false; lakes.cells.len()];
    let mut sea_fed = vec![false; lakes.cells.len()];
    // The runoff of each cell plus the water arriving from outside its depression (rivers,
    // overflow of lakes upstream): the inflow of the depression's sub-basins.
    let own = discharge.clone();
    let mut arriving = vec![0.0; cell_count];
    let mut local = vec![u32::MAX; cell_count];
    let mut sea_leftover = 0.0;
    for &cell in order.iter().rev() {
        let cell = cell as usize;
        // Shallow new depressions hold no water (see `ErosionParams::lake_onset_depth`). Any
        // member on or next to last step's lake continues it (also a shallow basin just across
        // a sill from it: at ~110 km cells, accepted).
        let holds_water = |lake: &usize| {
            let members = &lakes.cells[*lake];
            filled[cell] - elevation[members[0] as usize] >= params.lake_onset_depth
                || members.iter().any(|&m| continuing[m as usize])
        };
        let exit_lake = lakes.exit_of(cell);
        if let Some(lake) = exit_lake {
            hydrology.depressions += 1;
            hydrology.depression_cells += lakes.cells[lake].len();
        }
        // Sea water poured over the spill level into this depression (m³).
        let poured: f64 = exit_lake.map_or(0.0, |lake| {
            sea_water
                .iter()
                .filter(|&&(c, _)| lakes.lake[c] as usize == lake)
                .map(|&(_, volume)| volume)
                .sum()
        });
        if let Some(lake) = exit_lake.filter(|lake| holds_water(lake) || poured > 0.0) {
            // Open water evaporates at the potential rate instead of the land's
            // evapotranspiration.
            let excess = |c: usize| {
                let evapotranspiration = (precipitation[c] - runoff[c]).max(0.0);
                (potential_evaporation[c] - evapotranspiration).max(0.0) * cell_area
            };
            let members = &lakes.cells[lake];
            let mut basins = Basins::new(
                mesh,
                members,
                elevation,
                filled[cell],
                &mut local,
                params.lake_hierarchy,
            );
            let inflow: Vec<f64> = members
                .iter()
                .map(|&m| own[m as usize] + arriving[m as usize])
                .collect();
            basins.solve(&inflow, discharge[cell], &excess, elevation);
            if poured > 0.0 {
                let entry = members
                    .iter()
                    .position(|&m| m as usize == cell)
                    .expect("the exit is a member") as u32;
                let left = basins.pour(entry, poured / cell_area, elevation) * cell_area;
                sea_leftover += left;
            }
            let spills = basins.spills();
            let (mut covered_total, mut flux_evaporation, mut bodies) = (0.0, 0.0, 0);
            for body in basins.lakes() {
                let balance = &body.balance;
                for (i, &member) in body.cells.iter().enumerate() {
                    let covered = (balance.covered - i as f64).clamp(0.0, 1.0);
                    lake_evaporation[member as usize] = covered * excess(member as usize);
                }
                for &member in &body.cells[..balance.covered.ceil() as usize] {
                    water_level[member as usize] = balance.level;
                }
                for (i, &member) in body.cells.iter().enumerate() {
                    let covered = (balance.covered - i as f64).clamp(0.0, 1.0);
                    let depth = lake_water_depth(balance.level, elevation[member as usize]);
                    hydrology.lake_volume += covered * depth * cell_area;
                    if elevation[member as usize] < 0.0 {
                        hydrology.inland_water_cells += covered;
                    }
                }
                for (i, &member) in body.cells.iter().enumerate() {
                    let covered = (balance.covered - i as f64).clamp(0.0, 1.0);
                    hydrology.lake_potential_evaporation +=
                        covered * potential_evaporation[member as usize];
                }
                if body.sea_water > 0.0 {
                    // Fed by the sea: it evaporates more than its own inflow.
                    let evaporation: f64 = body
                        .cells
                        .iter()
                        .map(|&member| lake_evaporation[member as usize])
                        .sum();
                    hydrology.lake_evaporation += evaporation;
                    hydrology.sea_fed_evaporation += (evaporation - balance.evaporation).max(0.0);
                    hydrology.sea_fed_lakes += 1;
                    sea_fed[lake] = true;
                    hydrology.sea_fed_cells += balance.covered;
                    hydrology.sea_water += body.sea_water * cell_area;
                } else {
                    hydrology.lake_evaporation += balance.evaporation;
                }
                covered_total += balance.covered;
                flux_evaporation += balance.evaporation;
                bodies += usize::from(balance.covered > 0.0);
            }
            // Depressions below sea level outside the ocean.
            let below = members
                .iter()
                .filter(|&&m| elevation[m as usize] < 0.0)
                .count();
            if below > 0 {
                hydrology.inland_depressions += 1;
                hydrology.dry_inland_depressions += usize::from(covered_total < 0.1);
            }
            endorheic[lake] = !spills;
            if !spills && covered_total < 0.1 * members.len() as f64 {
                // A salt flat or playa: the water covers less than a tenth of the basin.
                hydrology.playas += 1;
                hydrology.playa_cells += members.len();
            }
            hydrology.lakes += 1;
            hydrology.endorheic_lakes += usize::from(!spills);
            let minima = members
                .iter()
                .filter(|&&m| {
                    mesh.neighbors(m)
                        .iter()
                        .all(|&n| elevation[n as usize] > elevation[m as usize])
                })
                .count();
            hydrology.nested_lakes += usize::from(minima > 1);
            hydrology.lake_bodies += bodies;
            hydrology.split_lakes += usize::from(bodies > 1);
            hydrology.lake_cells += covered_total;
            // A depression that does not spill lets nothing out; one that spills passes on its
            // inflow less the evaporation of its lake.
            discharge[cell] = if spills {
                discharge[cell] - flux_evaporation
            } else {
                0.0
            };
        }
        if let Some(receiver) = receiver[cell] {
            if !is_ocean(cell) && is_ocean(receiver as usize) {
                hydrology.to_ocean += discharge[cell];
            }
            discharge[receiver as usize] += discharge[cell];
            let into = lakes.lake[receiver as usize];
            if into != NO_LAKE && into != lakes.lake[cell] {
                arriving[receiver as usize] += discharge[cell];
            }
        }
    }

    // Land whose water ends in an endorheic lake: the lake itself and everything upstream.
    // The endorheic lake that the water of each cell ends in.
    let mut terminal = vec![NO_LAKE; cell_count];
    for &cell in &order {
        let cell = cell as usize;
        let lake = lakes.lake[cell];
        terminal[cell] = if lake != NO_LAKE && endorheic[lake as usize] {
            lake
        } else {
            receiver[cell].map_or(NO_LAKE, |r| terminal[r as usize])
        };
        if terminal[cell] != NO_LAKE && !is_ocean(cell) {
            hydrology.endorheic_cells += 1.0;
            if lakes.cells[terminal[cell] as usize].len() == 1 {
                hydrology.endorheic_pit_cells += 1.0;
            }
            if sea_fed[terminal[cell] as usize] {
                hydrology.endorheic_sea_fed_cells += 1.0;
            }
        }
    }

    // For the view only: the receivers as the water sees them, the closed depressions and the
    // depth of their lakes.
    let surface_water = SurfaceWater {
        receiver: (0..cell_count)
            .map(|cell| {
                let closed = lakes.exit_of(cell).is_some_and(|lake| endorheic[lake]);
                if is_ocean(cell) || closed {
                    None
                } else {
                    receiver[cell]
                }
            })
            .collect(),
        ocean: ocean.to_vec(),
        depression: lakes.lake.iter().map(|&lake| lake != NO_LAKE).collect(),
        lake_depth: (0..cell_count)
            .map(|cell| lake_water_depth(water_level[cell], elevation[cell]))
            .collect(),
        spill_depth: (0..cell_count)
            .map(|cell| {
                if lakes.lake[cell] == NO_LAKE {
                    0.0
                } else {
                    filled[cell] - elevation[cell]
                }
            })
            .collect(),
        depression_at_start: Vec::new(),
        lake_evaporation: lake_evaporation.iter().map(|e| e / cell_area).collect(),
        load_correction: (0..cell_count)
            .map(|cell| {
                // The floor as the water presses it (or unloaded where dry) minus the
                // processes' elevation.
                let depth = lake_water_depth(water_level[cell], elevation[cell]);
                if is_ocean(cell) {
                    0.0
                } else if depth > 0.0 {
                    water_level[cell] - depth - process[cell]
                } else {
                    elevation[cell] - process[cell]
                }
            })
            .collect(),
    };

    WaterState {
        receiver,
        filled,
        order,
        lakes,
        discharge,
        water_level,
        hydrology,
        surface_water,
        sea_leftover,
    }
}

/// Thickness removed from the top of a column in one step of implicit stream-power incision.
struct Incision {
    /// Bulk (compacted) thickness of sediment.
    bulk: f64,
    /// Thickness of bedrock.
    bedrock: f64,
}

/// Implicit stream-power incision `E = K Q^m S` (Braun & Willett, 2013) of a column whose
/// surface is at `elevation` above the water surface `base` it drains to, over `dt` years, with
/// the rate `K Q^m / L` (1/yr) of its sediment and of its bedrock in `rates`.
///
/// The slope is that of the surface at the end of the step, and the surface drops by much less
/// than the removed thickness: the column rebounds isostatically (by ~85% for crust). The
/// removed thickness `h` therefore solves `h = F (z(h) − z_r)` with `F = K Q^m dt / L` and the
/// surface `z(h)` after removing `h`; for bedrock with the surface response `f` per metre this
/// is `h = F (z − z_r) / (1 + F f)`. Sediment (more erodible, compacted) is cut first; when the
/// cut goes through it, the rest of the step cuts the bedrock below.
fn incise(
    elevation: f64,
    base: f64,
    sediment: f64,
    bedrock: &Bedrock,
    (sediment_rate, bedrock_rate): (f64, f64),
    dt: f64,
    isostatic: bool,
) -> Incision {
    let bulk = isostasy::sediment_bulk_thickness(sediment);
    let mut years = dt;
    let mut surface = elevation;
    if bulk > 0.0 {
        let after = |cut: f64| {
            if isostatic {
                isostasy::change_sediment(
                    elevation,
                    sediment,
                    isostasy::sediment_for_bulk_thickness((bulk - cut).max(0.0)),
                )
            } else {
                elevation - cut
            }
        };
        let factor = sediment_rate * dt;
        let bare = after(bulk);
        if bulk >= factor * (bare - base) {
            // The step ends in the sediment: `cut − F (z(cut) − z_r)` grows with the cut.
            let (mut low, mut high) = (0.0, bulk);
            for _ in 0..50 {
                let cut = 0.5 * (low + high);
                if cut < factor * (after(cut) - base) {
                    low = cut;
                } else {
                    high = cut;
                }
            }
            return Incision {
                bulk: low,
                bedrock: 0.0,
            };
        }
        // Time to cut through the sediment at the rate at its base (implicit).
        years -= bulk / (sediment_rate * (bare - base));
        surface = bare;
    }
    // The bedrock response is piecewise linear: the densified lower crust of thick orogens
    // first, then normal crust.
    let factor = bedrock_rate * years;
    let response = |density: f64| {
        if isostatic {
            isostasy::dry_response(density).max(1e-3)
        } else {
            1.0
        }
    };
    let dense_response = response(bedrock.dense_density);
    let cut = factor * (surface - base) / (1.0 + factor * dense_response);
    let cut = if cut <= bedrock.dense {
        cut
    } else {
        // `h = F (z₁ − f (h − h₁) − z_r)` with the surface `z₁` after removing the dense part.
        let normal = response(bedrock.density);
        let below = surface - dense_response * bedrock.dense;
        factor * (below + normal * bedrock.dense - base) / (1.0 + factor * normal)
    };
    Incision {
        bulk,
        bedrock: cut.max(0.0),
    }
}

/// What [`weather`] did to a cell.
struct Weathered {
    /// Rock-equivalent thickness of the deposit left above the regolith.
    deposit: f64,
    step: regolith::RegolithStep,
    /// Whether the deposit was cut through and the regolith carried away for part of the step.
    transported: bool,
}

/// One step of a land cell with regolith (#197): the deposit above the regolith is cut first
/// with the sediment's erodibility (`sediment_factor` times the bedrock `rate`, as in
/// [`incise`]); for the rest of the step the regolith is carried away and the bare rock
/// weathers and erodes (see [`regolith::regolith_step`]). Where the step ends in the deposit,
/// the rock below it still weathers under the deposit's cover. `production` is `P₀` (m/yr of
/// bedrock); the column's surface is at `elevation`, its base level at `base`.
fn weather(
    params: &RegolithParams,
    (elevation, base): (f64, f64),
    (sediment, regolith): (f64, f64),
    bedrock: &Bedrock,
    (production, rate, sediment_factor): (f64, f64, f64),
    dt: f64,
    isostatic: bool,
) -> Weathered {
    let total_bulk = isostasy::sediment_bulk_thickness(sediment);
    let top_bulk = (total_bulk - isostasy::sediment_bulk_thickness(regolith)).max(0.0);
    let after = |cut: f64| {
        if isostatic {
            isostasy::change_sediment(
                elevation,
                sediment,
                isostasy::sediment_for_bulk_thickness((total_bulk - cut).max(0.0)),
            )
        } else {
            elevation - cut
        }
    };
    let response = |density: f64| {
        if isostatic {
            isostasy::dry_response(density).max(1e-3)
        } else {
            1.0
        }
    };
    let column = |surface: f64| regolith::Column {
        height: (surface - base).max(0.0),
        dense: bedrock.dense,
        dense_response: response(bedrock.dense_density),
        response: response(bedrock.density),
        density: bedrock.density,
    };
    let mut years = dt;
    let mut surface = elevation;
    if top_bulk > 0.0 {
        let factor = rate * sediment_factor * dt;
        let bare = after(top_bulk);
        if rate <= 0.0 || top_bulk >= factor * (bare - base) {
            // The step ends in the deposit (see `incise`); the rock below weathers under it.
            let cut = if rate <= 0.0 {
                0.0
            } else {
                let (mut low, mut high) = (0.0, top_bulk);
                for _ in 0..50 {
                    let cut = 0.5 * (low + high);
                    if cut < factor * (after(cut) - base) {
                        low = cut;
                    } else {
                        high = cut;
                    }
                }
                low
            };
            let remaining = isostasy::sediment_for_bulk_thickness((total_bulk - cut).max(0.0));
            let deposit = (remaining - regolith).max(0.0);
            let step = regolith::regolith_step(
                params,
                &column(after(cut)),
                regolith,
                top_bulk - cut,
                production,
                0.0,
                dt,
            );
            return Weathered {
                deposit,
                step,
                transported: false,
            };
        }
        years -= top_bulk / (rate * sediment_factor * (bare - base));
        surface = bare;
    }
    let step = regolith::regolith_step(
        params,
        &column(surface),
        regolith,
        0.0,
        production,
        rate,
        years,
    );
    Weathered {
        deposit: 0.0,
        step,
        transported: rate > 0.0,
    }
}

/// The crystalline crust below the sediment, as far as erosion from the top sees it.
struct Bedrock {
    /// Thickness (m) of the densified part that removal thins first.
    dense: f64,
    /// Density giving its isostatic response.
    dense_density: f64,
    /// Density of the crust below (and its isostatic response).
    density: f64,
}

/// Marks cells that are not in a lake.
const NO_LAKE: u32 = u32::MAX;

/// Lakes are depressions of at least this depth (m) below their spill level; shallower
/// irregularities (flat filled lake floors) are part of the river network.
const LAKE_MIN_DEPTH: f64 = 1.0;

/// The depressions of the drainage: connected land cells below their spill level.
struct Lakes {
    /// The lake of every cell, or [`NO_LAKE`].
    lake: Vec<u32>,
    /// The cells of every lake, from the lowest floor up.
    cells: Vec<Vec<u32>>,
    /// The cell of every lake through which its water leaves (its receiver is the pass).
    exit: Vec<u32>,
}

impl Lakes {
    fn new(
        order: &[u32],
        receiver: &[Option<u32>],
        filled: &[f64],
        elevation: &[f64],
        ocean: &[bool],
    ) -> Self {
        let mut lake = vec![NO_LAKE; elevation.len()];
        let mut cells: Vec<Vec<u32>> = Vec::new();
        let mut exit = Vec::new();
        // The flood reaches every depression through its pass, so every lake is a subtree of
        // the receivers whose root (the exit) drains to the pass.
        for &cell in order {
            let c = cell as usize;
            if ocean[c] || filled[c] - elevation[c] <= LAKE_MIN_DEPTH {
                continue;
            }
            match receiver[c].map(|r| lake[r as usize]) {
                Some(id) if id != NO_LAKE => {
                    lake[c] = id;
                    cells[id as usize].push(cell);
                }
                _ => {
                    lake[c] = cells.len() as u32;
                    cells.push(vec![cell]);
                    exit.push(cell);
                }
            }
        }
        for members in &mut cells {
            members.sort_by(|&a, &b| elevation[a as usize].total_cmp(&elevation[b as usize]));
        }
        Self { lake, cells, exit }
    }

    /// The lake whose exit is `cell`.
    fn exit_of(&self, cell: usize) -> Option<usize> {
        let lake = self.lake[cell];
        (lake != NO_LAKE && self.exit[lake as usize] as usize == cell).then_some(lake as usize)
    }
}

/// Depth (m) of lake water standing at `level` over a cell whose unloaded floor is at `floor`,
/// zero if the water does not reach it. The water presses the floor down by its own weight
/// (Airy): it stands `WATER_LOADING · (level − floor)` deep. The lake covers the same cells at
/// the same level, so the water balance on the unloaded floors is unchanged (#127).
fn lake_water_depth(level: f64, floor: f64) -> f64 {
    if level > floor {
        isostasy::WATER_LOADING * (level - floor)
    } else {
        0.0
    }
}

/// The water balance of a lake.
#[derive(Debug)]
struct LakeBalance {
    /// Water surface (m).
    level: f64,
    /// Cells covered by water, from the lowest floor up (the last one possibly in part).
    covered: f64,
    /// Evaporation in excess of the land's evapotranspiration (m³/yr).
    evaporation: f64,
    /// Whether the lake does not spill.
    endorheic: bool,
}

/// The water balance of a lake with the given `cells` (from the lowest floor up) and spill
/// level `spill`, fed by `inflow` (m³/yr) of runoff, where open water over a cell evaporates
/// `excess(cell)` m³/yr more than the land would (closed-basin lakes: Mason et al. 1994). The
/// lake rises until its evaporation balances the inflow; if it reaches the spill level first,
/// it spills the rest.
fn lake_balance(
    cells: &[u32],
    elevation: &[f64],
    spill: f64,
    inflow: f64,
    excess: impl Fn(usize) -> f64,
) -> LakeBalance {
    let mut evaporation = 0.0;
    for (i, &cell) in cells.iter().enumerate() {
        let loss = excess(cell as usize);
        if evaporation + loss >= inflow {
            // The water surface lies between this floor and the next one (or the spill level).
            let fraction = if loss > 0.0 {
                ((inflow - evaporation) / loss).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let floor = elevation[cell as usize];
            let next = cells
                .get(i + 1)
                .map_or(spill, |&c| elevation[c as usize])
                .min(spill);
            return LakeBalance {
                level: floor + fraction * (next - floor),
                covered: i as f64 + fraction,
                evaporation: inflow.max(0.0),
                endorheic: true,
            };
        }
        evaporation += loss;
    }
    LakeBalance {
        level: spill,
        covered: cells.len() as f64,
        evaporation,
        endorheic: false,
    }
}

/// Deposits up to `amount` m (rock-equivalent thickness over one cell) of sediment in a lake:
/// it settles on the lowest floors first and levels the lake floor up, at most to the spill
/// level `spill` (lakes trap nearly all of their sediment; Brune 1953). Returns the amount
/// deposited (the rest passes the outlet) and whether the lake is filled up to the spill.
fn fill_lake(seabed: &mut Seabed, cells: &[u32], spill: f64, amount: f64) -> (f64, bool) {
    if amount <= 0.0 {
        return (0.0, false);
    }
    let room = |seabed: &Seabed, level: f64| {
        cells
            .iter()
            .map(|&c| seabed.room(c as usize, level, f64::INFINITY))
            .sum::<f64>()
    };
    let full = room(seabed, spill) <= amount;
    let level = if full {
        spill
    } else {
        let mut low = cells
            .iter()
            .map(|&c| seabed.elevation[c as usize])
            .fold(f64::INFINITY, f64::min);
        let mut high = spill;
        for _ in 0..50 {
            let mid = 0.5 * (low + high);
            if room(seabed, mid) <= amount {
                low = mid;
            } else {
                high = mid;
            }
        }
        low
    };
    let mut deposited = 0.0;
    for &cell in cells {
        let thickness = seabed.room(cell as usize, level, f64::INFINITY);
        if thickness > 0.0 {
            seabed.deposit(cell as usize, thickness);
            deposited += thickness;
        }
    }
    (deposited, full)
}

/// Wave erosion of coasts (Sunamura 1992; Trenhaile 2000) at the sea level after the step's
/// deposition:
///
/// - **Cliffs.** At a coast (a land cell next to the sea) a sub-grid cliff retreats through the
///   cell by `rate · dt`, removing that part of the cell's column above sea level (with the
///   isostatic rebound). The rate is the long-term rate of the material at the waterline:
///   sediment if the sediment reaches below sea level, else bedrock. Eroded bedrock becomes
///   sediment of the same mass.
/// - **Wave base.** Sediment on the sea floor above the storm wave base is reworked (Myr steps
///   are far longer than its reworking time of 10³–10⁴ yr). The wave base is deep on shelves
///   open to the ocean and shallow in enclosed and epicontinental seas.
///
/// The moved sediment goes offshore through the sea, nearest cells first, to cells below their
/// wave base with room (zone targets and caps). What finds no room within reach stays.
fn wave_erosion(
    (mesh, ocean_seeds): (&SphereMesh, &[usize]),
    seabed: &mut Seabed,
    bedrock_delta: &mut [f64],
    (bedrock_density, bedrock_isostatic_density): (&[f64], &[f64]),
    (zones, class): (&[Zone], &[CrustClass]),
    params: &ErosionParams,
    dt_years: f64,
) -> WaveErosion {
    let cell_count = mesh.len();
    let spacing = mesh.mean_edge_angle() * params.planet_radius;
    let mut result = WaveErosion::default();

    // Waves act on the ocean only, not on lakes or dry depressions.
    let ocean = sea::ocean_from(
        mesh,
        &seabed.elevation,
        ocean_seeds.iter().copied(),
        !params.sea_spill,
    );
    let wave_base = wave_bases_in(mesh, &seabed.elevation, &ocean, params);
    let wave_base = |cell: usize| wave_base[cell];

    // Sources: (cell, rock-equivalent sediment), taken off at once.
    let mut sources: Vec<(usize, f64)> = Vec::new();
    let elevation = seabed.elevation.clone();
    for cell in 0..cell_count {
        let e = elevation[cell];
        if !ocean[cell] {
            if e < 0.0 {
                continue; // A depression below sea level: no waves.
            }
            let coastal = mesh
                .neighbors(cell as u32)
                .iter()
                .any(|&m| ocean[m as usize]);
            if !coastal {
                continue;
            }
            let sediment = seabed.sediment[cell];
            let bulk = isostasy::sediment_bulk_thickness(sediment);
            let rate = if bulk >= e {
                params.cliff_retreat_sediment
            } else {
                params.cliff_retreat_bedrock
            };
            let fraction = (rate * dt_years / spacing).min(1.0);
            if fraction <= 0.0 {
                continue;
            }
            // The column above sea level: sediment first, then bedrock.
            let from_sediment = if bulk >= e {
                -isostasy::sediment_for_rise(e, 0.0, sediment)
            } else {
                sediment
            }
            .clamp(0.0, sediment);
            let bare = isostasy::change_sediment(e, sediment, sediment - from_sediment);
            let from_bedrock = (isostasy::to_dry(bare).max(0.0)
                / isostasy::dry_response(bedrock_isostatic_density[cell]))
            .max(0.0);
            let (from_sediment, from_bedrock) = (fraction * from_sediment, fraction * from_bedrock);
            let mut amount = 0.0;
            if from_sediment > 0.0 {
                seabed.remove(cell, from_sediment);
                result.cliff_sediment += from_sediment;
                amount += from_sediment;
            }
            if from_bedrock > 0.0 {
                bedrock_delta[cell] -= from_bedrock;
                seabed.elevation[cell] = isostasy::add_layer(
                    seabed.elevation[cell],
                    -from_bedrock,
                    bedrock_isostatic_density[cell],
                );
                let made = from_bedrock * bedrock_density[cell] / SEDIMENT_ROCK_DENSITY;
                result.cliff_bedrock += made;
                amount += made;
            }
            if amount > 0.0 {
                sources.push((cell, amount));
            }
        } else if e > wave_base(cell) && seabed.sediment[cell] > 0.0 {
            let amount = (-isostasy::sediment_for_rise(e, wave_base(cell), seabed.sediment[cell]))
                .clamp(0.0, seabed.sediment[cell]);
            if amount > 0.0 {
                seabed.remove(cell, amount);
                result.reworked += amount;
                sources.push((cell, amount));
            }
        }
    }

    // Offshore transport, nearest cells below the wave base first.
    let mut mark = vec![usize::MAX; cell_count];
    let limit = params.max_margin_cells + params.max_fan_cells;
    let mut queue = VecDeque::new();
    for (index, &(source, amount)) in sources.iter().enumerate() {
        let mut remaining = amount;
        queue.clear();
        queue.push_back((source, 0u32));
        mark[source] = index;
        let mut visited = 0;
        while let Some((cell, hops)) = queue.pop_front() {
            if remaining <= 0.0 || visited >= limit {
                break;
            }
            visited += 1;
            let base = wave_base(cell);
            if cell != source && seabed.elevation[cell] < base {
                let (target, cap) = match zones[cell] {
                    Zone::Shelf if class[cell] == CrustClass::Continental => {
                        (params.shelf_depth, f64::INFINITY)
                    }
                    Zone::Shelf => (params.shelf_depth, params.max_shelf_sediment),
                    Zone::Slope => (params.slope_depth, params.max_slope_sediment),
                    Zone::Rise => (params.rise_depth, params.max_rise_sediment),
                    Zone::Fan => (
                        params.slope_depth,
                        params.max_fan_sediment
                            * math::exp(-f64::from(hops) * spacing / params.fan_length),
                    ),
                };
                let thickness = remaining.min(seabed.room(cell, target.min(base), cap));
                if thickness > 0.0 {
                    seabed.deposit(cell, thickness);
                    remaining -= thickness;
                }
            }
            for &neighbor in mesh.neighbors(cell as u32) {
                let neighbor = neighbor as usize;
                if mark[neighbor] != index && ocean[neighbor] && seabed.elevation[neighbor] < 0.0 {
                    mark[neighbor] = index;
                    queue.push_back((neighbor, hops + 1));
                }
            }
        }
        if remaining > 0.0 {
            seabed.deposit(source, remaining);
            result.stayed += remaining;
        }
    }
    result
}

/// The storm wave base of every cell (m): [`ErosionParams::wave_base`] within
/// [`ErosionParams::fetch_rings`] rings (through the sea) of open water deeper than
/// [`ErosionParams::open_water_depth`], [`ErosionParams::sheltered_wave_base`] in the enclosed
/// and epicontinental seas beyond.
pub fn wave_bases(
    mesh: &SphereMesh,
    elevation: &[f64],
    min_sea_fraction: f64,
    params: &ErosionParams,
) -> Vec<f64> {
    wave_bases_in(
        mesh,
        elevation,
        &sea::ocean_mask(mesh, elevation, min_sea_fraction, !params.sea_spill),
        params,
    )
}

/// [`wave_bases`] for the given ocean mask (see [`sea::ocean_mask`]); only ocean cells are
/// open water.
fn wave_bases_in(
    mesh: &SphereMesh,
    elevation: &[f64],
    ocean: &[bool],
    params: &ErosionParams,
) -> Vec<f64> {
    let cell_count = mesh.len();
    let mut ring = vec![u32::MAX; cell_count];
    let mut queue = VecDeque::new();
    for cell in 0..cell_count {
        if ocean[cell] && elevation[cell] < params.open_water_depth {
            ring[cell] = 0;
            queue.push_back(cell);
        }
    }
    while let Some(cell) = queue.pop_front() {
        if ring[cell] >= params.fetch_rings {
            continue;
        }
        for &neighbor in mesh.neighbors(cell as u32) {
            let neighbor = neighbor as usize;
            if ring[neighbor] == u32::MAX && ocean[neighbor] {
                ring[neighbor] = ring[cell] + 1;
                queue.push_back(neighbor);
            }
        }
    }
    ring.iter()
        .map(|&ring| {
            if ring <= params.fetch_rings {
                params.wave_base
            } else {
                params.sheltered_wave_base
            }
        })
        .collect()
}

/// Scratch space of [`fill_fan`].
struct FanScratch {
    /// The mouth that last reached each cell.
    mark: Vec<usize>,
    /// Cells reached, with their distance in hops and the lowest elevation on the path there.
    path: Vec<(usize, u32, f64)>,
}

impl FanScratch {
    fn new(cell_count: usize) -> Self {
        Self {
            mark: vec![usize::MAX; cell_count],
            path: Vec::new(),
        }
    }
}

/// Deposits up to `amount` of sediment from a river mouth into deep-sea fans and returns the
/// deposited amount. Turbidity currents run downhill: from the mouth, the flow spreads through
/// the sea to neighbours no higher than [`ErosionParams::fan_climb`] above the lowest point of
/// its path so far, up to [`ErosionParams::max_bypass_distance`] (the Bengal fan reaches
/// ~3000 km; Curray et al. 2003). It fills the cells far from margins (the fan zone) nearest
/// first along the paths, up to the slope depth and a thickness cap that falls with the
/// distance from the mouth, so that fans pond in basins and thin into lobes.
fn fill_fan(
    mesh: &SphereMesh,
    seabed: &mut Seabed,
    zones: &[Zone],
    params: &ErosionParams,
    mouth: usize,
    amount: f64,
    scratch: &mut FanScratch,
) -> f64 {
    let spacing = mesh.mean_edge_angle() * params.planet_radius;
    let max_hops = (params.max_bypass_distance / spacing).floor() as u32;
    let path = &mut scratch.path;
    path.clear();
    path.push((mouth, 0, seabed.elevation[mouth]));
    scratch.mark[mouth] = mouth;
    let mut remaining = amount;
    let mut next = 0;
    while next < path.len() && remaining > 0.0 {
        let (cell, hops, lowest) = path[next];
        next += 1;
        if cell != mouth && zones[cell] == Zone::Fan {
            let cap =
                params.max_fan_sediment * math::exp(-f64::from(hops) * spacing / params.fan_length);
            let thickness = remaining.min(seabed.room(cell, params.slope_depth, cap));
            if thickness > 0.0 {
                seabed.deposit(cell, thickness);
                remaining -= thickness;
            }
        }
        if hops >= max_hops {
            continue;
        }
        for &neighbor in mesh.neighbors(cell as u32) {
            let neighbor = neighbor as usize;
            let height = seabed.elevation[neighbor];
            if height < 0.0
                && height <= lowest + params.fan_climb
                && scratch.mark[neighbor] != mouth
            {
                scratch.mark[neighbor] = mouth;
                path.push((neighbor, hops + 1, lowest.min(height)));
            }
        }
    }
    // The lobes are full (the search above ended without placing everything): the fan aggrades
    // up to its graded surface, nearest cells first. Slope and rise cells on the paths take
    // part, so a narrow basin without a fan zone fills like a fan.
    if params.graded_fans && remaining > 0.0 {
        for &(cell, hops, _) in path.iter() {
            if remaining <= 0.0 {
                break;
            }
            if cell == mouth || zones[cell] == Zone::Shelf {
                continue;
            }
            let distance = f64::from(hops) * spacing;
            let surface = params.slope_depth
                - params.fan_relief * (1.0 - math::exp(-distance / params.fan_length));
            let thickness = remaining.min(seabed.room(cell, surface, f64::INFINITY));
            if thickness > 0.0 {
                seabed.deposit(cell, thickness);
                remaining -= thickness;
            }
        }
    }
    for &(cell, ..) in path.iter() {
        scratch.mark[cell] = usize::MAX;
    }
    amount - remaining
}

/// Deposits `amount` of sediment that found no room in the sea as a terminal fan or prograding
/// delta around the river mouth `apex`, and returns the deposited amount. The deposit is a cone
/// whose surface falls from the apex with [`ErosionParams::alluvial_slope`]; its height is the
/// one that holds the amount, with the isostatic response of every column. The cone only
/// spreads through cells that it covers, so it does not reach lows behind higher ground. Where
/// it meets the sea, the sea becomes land: the delta progrades, slowly into deep water, which
/// takes far more sediment per cell. Fluvial megafans and terminal fans in filled basins are
/// such cones (Kosi, Pilcomayo, Okavango; Leier et al. 2005; Hartley et al. 2010).
fn build_terminal_fan(
    mesh: &SphereMesh,
    seabed: &mut Seabed,
    params: &ErosionParams,
    apex: usize,
    amount: f64,
    scratch: &mut FanScratch,
) -> f64 {
    /// The cone reaches at most this many cells from the apex.
    const MAX_HOPS: u32 = 64;
    if amount <= 0.0 || params.alluvial_slope <= 0.0 {
        return 0.0;
    }
    let drop = params.alluvial_slope * mesh.mean_edge_angle() * params.planet_radius;
    // The cells covered by the cone with its apex at `top` (into `scratch.path`, with their
    // distance in cells), and the sediment it holds on them; the search ends early once it
    // holds more than `limit`.
    let cone = |seabed: &Seabed, scratch: &mut FanScratch, top: f64, limit: f64| {
        let FanScratch { mark, path } = scratch;
        path.clear();
        path.push((apex, 0, 0.0));
        mark[apex] = apex;
        let mut room = 0.0;
        let mut next = 0;
        while next < path.len() && room <= limit {
            let (cell, hops, _) = path[next];
            next += 1;
            room += seabed.room(cell, top - drop * f64::from(hops), f64::INFINITY);
            if hops >= MAX_HOPS {
                continue;
            }
            let surface = top - drop * f64::from(hops + 1);
            for &neighbor in mesh.neighbors(cell as u32) {
                let neighbor = neighbor as usize;
                if mark[neighbor] != apex && seabed.elevation[neighbor] < surface {
                    mark[neighbor] = apex;
                    path.push((neighbor, hops + 1, 0.0));
                }
            }
        }
        for &(cell, ..) in path.iter() {
            mark[cell] = usize::MAX;
        }
        room
    };
    // The highest cone that holds at most the amount.
    let mut low = seabed.elevation[apex];
    let mut step = 1.0;
    while cone(seabed, scratch, low + step, amount) <= amount && step < 1e6 {
        step *= 2.0;
    }
    let mut high = low + step;
    while high - low > 1e-4 {
        let mid = 0.5 * (low + high);
        if cone(seabed, scratch, mid, amount) <= amount {
            low = mid;
        } else {
            high = mid;
        }
    }
    let mut remaining = amount;
    cone(seabed, scratch, low, f64::INFINITY);
    for &(cell, hops, _) in &scratch.path {
        let thickness = seabed
            .room(cell, low - drop * f64::from(hops), f64::INFINITY)
            .min(remaining);
        if thickness > 0.0 {
            seabed.deposit(cell, thickness);
            remaining -= thickness;
        }
    }
    // A cone that just overtops a sill covers a low beyond it, which holds more than is left:
    // the rest is shared out there in proportion to the room.
    if remaining > 0.0 {
        let room = cone(seabed, scratch, high, f64::INFINITY);
        if room > 0.0 {
            let share = (remaining / room).min(1.0);
            for &(cell, hops, _) in &scratch.path {
                let thickness =
                    share * seabed.room(cell, high - drop * f64::from(hops), f64::INFINITY);
                if thickness > 0.0 {
                    seabed.deposit(cell, thickness);
                    remaining -= thickness;
                }
            }
        }
    }
    // Rounding dust stays at the apex.
    if remaining > 0.0 && remaining <= 1e-9 * amount.max(1.0) {
        seabed.deposit(apex, remaining);
        remaining = 0.0;
    }
    amount - remaining.max(0.0)
}

/// The margin within reach of a river mouth: shelf, slope and rise cells connected to it
/// through the ocean, nearest first, at most [`ErosionParams::max_margin_cells`] (into
/// `margin`).
#[allow(clippy::too_many_arguments)]
fn margin_within_reach(
    mesh: &SphereMesh,
    mouth: usize,
    zones: &[Zone],
    params: &ErosionParams,
    is_ocean: &dyn Fn(usize) -> bool,
    mark: &mut [usize],
    queue: &mut VecDeque<usize>,
    margin: &mut Vec<usize>,
) {
    queue.clear();
    margin.clear();
    queue.push_back(mouth);
    mark[mouth] = mouth;
    while let Some(cell) = queue.pop_front() {
        margin.push(cell);
        if margin.len() >= params.max_margin_cells {
            break;
        }
        for &neighbor in mesh.neighbors(cell as u32) {
            let neighbor = neighbor as usize;
            if is_ocean(neighbor) && zones[neighbor] != Zone::Fan && mark[neighbor] != mouth {
                mark[neighbor] = mouth;
                queue.push_back(neighbor);
            }
        }
    }
    // Reset the marks so that a second search from the same mouth works.
    for &cell in margin.iter() {
        mark[cell] = usize::MAX;
    }
    for &cell in queue.iter() {
        mark[cell] = usize::MAX;
    }
}

/// The ocean cells within reach of a river mouth with their distance in hops, nearest first,
/// at most [`ErosionParams::max_fan_cells`] (into `water`). Returns whether the reach limit cut
/// the search short, i.e. the mouth is not on a small enclosed water body.
fn water_within_reach(
    mesh: &SphereMesh,
    mouth: usize,
    params: &ErosionParams,
    is_ocean: &dyn Fn(usize) -> bool,
    mark: &mut [usize],
    queue: &mut VecDeque<(usize, u32)>,
    water: &mut Vec<(usize, u32)>,
) -> bool {
    queue.clear();
    water.clear();
    queue.push_back((mouth, 0));
    mark[mouth] = mouth;
    let mut reached = 1;
    let mut open = false;
    while let Some((cell, hops)) = queue.pop_front() {
        water.push((cell, hops));
        for &neighbor in mesh.neighbors(cell as u32) {
            let neighbor = neighbor as usize;
            if is_ocean(neighbor) && mark[neighbor] != mouth {
                if reached >= params.max_fan_cells {
                    open = true;
                    continue;
                }
                reached += 1;
                mark[neighbor] = mouth;
                queue.push_back((neighbor, hops + 1));
            }
        }
    }
    for &(cell, _) in water.iter() {
        mark[cell] = usize::MAX;
    }
    open
}

/// Surface elevation and sediment of every cell while sediment is deposited.
struct Seabed {
    elevation: Vec<f64>,
    /// Sediment thickness now.
    sediment: Vec<f64>,
    /// Change of the sediment thickness in this erosion step.
    delta: Vec<f64>,
}

impl Seabed {
    /// Room (m) to fill a cell up to `target` without exceeding `cap` m of sediment.
    fn room(&self, cell: usize, target: f64, cap: f64) -> f64 {
        if self.elevation[cell] < target {
            isostasy::sediment_for_rise(self.elevation[cell], target, self.sediment[cell])
                .min(cap - self.sediment[cell])
                .max(0.0)
        } else {
            0.0
        }
    }

    /// The sea level relative to the one the elevations refer to (m, an offset of dry
    /// elevations).
    fn sea_level(&self, mesh: &SphereMesh, cell_area: f64, volume: f64, seeds: &[usize]) -> f64 {
        sea::relative_sea_level_from(
            mesh,
            &self.elevation,
            &self.sediment,
            cell_area,
            volume,
            seeds,
        )
    }

    /// Makes the elevations relative to a sea level `rise` m (dry offset) higher.
    fn shift_sea_level(&mut self, rise: f64) {
        for elevation in &mut self.elevation {
            *elevation = isostasy::to_surface(isostasy::to_dry(*elevation) - rise);
        }
    }

    fn remove(&mut self, cell: usize, thickness: f64) {
        let before = self.sediment[cell];
        let after = (before - thickness).max(0.0);
        self.delta[cell] -= before - after;
        self.sediment[cell] = after;
        self.elevation[cell] = isostasy::change_sediment(self.elevation[cell], before, after);
    }

    fn deposit(&mut self, cell: usize, thickness: f64) {
        let before = self.sediment[cell];
        self.delta[cell] += thickness;
        self.sediment[cell] += thickness;
        self.elevation[cell] =
            isostasy::change_sediment(self.elevation[cell], before, self.sediment[cell]);
    }
}

/// Transport-limited spreading of sediment (Kenyon & Turcotte 1985; Kaufman et al. 1991; Paola
/// et al. 1992): sediment moves to lower neighbours with a flux proportional to the difference in
/// surface elevation (linear diffusion), as far as the cell has sediment. On land the diffusivity
/// is [`ErosionParams::sediment_diffusivity`] and land is lowered at most to sea level: delta and
/// coastal plains spread seaward and grade to sea level, they are not drowned. Below sea level
/// sediment spreads [`ErosionParams::marine_diffusivity_ratio`] times as fast (below wave base
/// only gravity flows move it). The regolith at the bottom of the sediment (`fixed`, #197) is
/// rock weathered in place and stays: its transport is part of the weathering step.
fn diffuse_sediment(
    mesh: &SphereMesh,
    seabed: &mut Seabed,
    (ocean, fixed): (&[bool], &[f64]),
    params: &ErosionParams,
    dt_years: f64,
) {
    if params.sediment_diffusivity <= 0.0 {
        return;
    }
    let spacing = mesh.mean_edge_angle() * params.planet_radius;
    // Explicit scheme: each cell may pass at most ~1/2 of its excess over ~6 neighbours.
    let rate = params.sediment_diffusivity * dt_years / (spacing * spacing);
    let substeps = (rate * 6.0 / 0.5).ceil().max(1.0) as usize;
    let rate = rate / substeps as f64;
    let cell_count = mesh.len();
    let mut outflow = vec![0.0; cell_count];
    let mut transfers: Vec<(usize, usize, f64)> = Vec::new();
    for _ in 0..substeps {
        transfers.clear();
        outflow.iter_mut().for_each(|x| *x = 0.0);
        for cell in 0..cell_count {
            let height = seabed.elevation[cell];
            if seabed.sediment[cell] - fixed[cell] <= 0.0 {
                continue;
            }
            // Land (also in depressions below sea level) is lowered at most to sea level towards
            // the ocean, and down to its neighbour towards land.
            let marine = ocean[cell] && height < 0.0;
            let rate = if marine {
                rate * params.marine_diffusivity_ratio
            } else {
                rate
            };
            for &neighbor in mesh.neighbors(cell as u32) {
                let neighbor = neighbor as usize;
                let floor = if !marine && ocean[neighbor] {
                    0.0
                } else {
                    f64::NEG_INFINITY
                };
                let drop = height - seabed.elevation[neighbor].max(floor);
                if drop > 0.0 {
                    let amount = rate * drop;
                    outflow[cell] += amount;
                    transfers.push((cell, neighbor, amount));
                }
            }
        }
        for &(from, to, amount) in &transfers {
            // No cell gives more sediment than it has.
            let scale = ((seabed.sediment[from] - fixed[from]) / outflow[from]).min(1.0);
            let amount = amount * scale;
            if amount > 0.0 {
                seabed.remove(from, amount);
                seabed.deposit(to, amount);
            }
        }
    }
}

/// The [`Zone`] of each cell: oceanic crust by its distance from the nearest crust of its own
/// plate that is not oceanic (counted in rings of cells within the plate).
fn zones(
    mesh: &SphereMesh,
    plate: &[u32],
    class: &[CrustClass],
    params: &ErosionParams,
) -> Vec<Zone> {
    let cell_count = mesh.len();
    let spacing = mesh.mean_edge_angle() * params.planet_radius;
    let mut ring = vec![u32::MAX; cell_count];
    let mut queue = VecDeque::new();
    for cell in 0..cell_count {
        if class[cell] != CrustClass::Oceanic {
            ring[cell] = 0;
            queue.push_back(cell);
        }
    }
    while let Some(cell) = queue.pop_front() {
        for &neighbor in mesh.neighbors(cell as u32) {
            let neighbor = neighbor as usize;
            if ring[neighbor] == u32::MAX && plate[neighbor] == plate[cell] {
                ring[neighbor] = ring[cell] + 1;
                queue.push_back(neighbor);
            }
        }
    }
    ring.iter()
        .map(|&ring| {
            let distance = f64::from(ring) * spacing;
            if ring == 0 {
                Zone::Shelf
            } else if ring == 1 || distance <= params.slope_width {
                Zone::Slope
            } else if distance <= params.rise_width {
                Zone::Rise
            } else {
                Zone::Fan
            }
        })
        .collect()
}

struct Drainage {
    /// The cell each cell drains into; `None` for ocean cells.
    receiver: Vec<Option<u32>>,
    /// Elevation of the water surface: the cell elevation, or the spill level of a lake.
    filled: Vec<f64>,
    /// Cells ordered such that every cell comes after its receiver.
    order: Vec<u32>,
}

/// Priority-flood drainage (Barnes et al., 2014), starting from all ocean cells.
fn drainage(mesh: &SphereMesh, elevation: &[f64], ocean: &[bool]) -> Drainage {
    /// Minimal rise of lake surfaces per cell, so that flow directions inside lakes are defined.
    const EPSILON: f64 = 1e-3;

    let cell_count = mesh.len();
    let mut receiver = vec![None; cell_count];
    let mut filled = elevation.to_vec();
    let mut visited = vec![false; cell_count];
    let mut order = Vec::with_capacity(cell_count);
    let mut heap = BinaryHeap::new();

    for cell in 0..cell_count {
        if ocean[cell] {
            visited[cell] = true;
            heap.push(Reverse((OrderedFloat(elevation[cell]), cell as u32)));
        }
    }
    if heap.is_empty() {
        let lowest = (0..cell_count)
            .min_by(|&a, &b| elevation[a].total_cmp(&elevation[b]))
            .expect("mesh has cells");
        visited[lowest] = true;
        heap.push(Reverse((OrderedFloat(elevation[lowest]), lowest as u32)));
    }

    while let Some(Reverse((OrderedFloat(level), cell))) = heap.pop() {
        order.push(cell);
        for &neighbor in mesh.neighbors(cell) {
            let n = neighbor as usize;
            if visited[n] {
                continue;
            }
            visited[n] = true;
            receiver[n] = Some(cell);
            filled[n] = elevation[n].max(level + EPSILON);
            heap.push(Reverse((OrderedFloat(filled[n]), neighbor)));
        }
    }

    Drainage {
        receiver,
        filled,
        order,
    }
}

#[derive(Clone, Copy)]
struct OrderedFloat(f64);

impl PartialEq for OrderedFloat {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for OrderedFloat {}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cone-shaped island (radius 20°) with a shelf sloping to −1500 m at 30°, surrounded by
    /// a deep ocean.
    fn island(mesh: &SphereMesh) -> Vec<f64> {
        (0..mesh.len() as u32)
            .map(|cell| {
                let angle = math::angle_between(mesh.vertex(cell), glam::DVec3::Z).to_degrees();
                if angle <= 30.0 {
                    3000.0 - angle * 150.0
                } else {
                    -4500.0 - angle
                }
            })
            .collect()
    }

    /// The model without regolith (#197), for the tests of routing, lakes and waves that check
    /// exact amounts on bare bedrock.
    fn without_regolith() -> RegolithParams {
        RegolithParams {
            enabled: false,
            ..RegolithParams::default()
        }
    }

    /// Continental crust down to `depth` below sea level, oceanic crust below, all on one plate.
    fn classes(elevation: &[f64], depth: f64) -> Vec<CrustClass> {
        elevation
            .iter()
            .map(|&e| {
                if e >= depth {
                    CrustClass::Continental
                } else {
                    CrustClass::Oceanic
                }
            })
            .collect()
    }

    /// One erosion step of 1 Myr with uniform precipitation and bedrock density, without
    /// sediment diffusion and waves (routing tests).
    fn run(
        mesh: &SphereMesh,
        elevation: &[f64],
        precipitation: f64,
        plate: &[u32],
        class: &[CrustClass],
    ) -> ErosionResult {
        let params = ErosionParams {
            sediment_diffusivity: 0.0,
            waves: false,
            ..ErosionParams::default()
        };
        run_with(
            mesh,
            elevation,
            &vec![0.0; mesh.len()],
            precipitation,
            plate,
            class,
            &params,
        )
    }

    fn run_with(
        mesh: &SphereMesh,
        elevation: &[f64],
        sediment: &[f64],
        precipitation: f64,
        plate: &[u32],
        class: &[CrustClass],
        params: &ErosionParams,
    ) -> ErosionResult {
        let n = mesh.len();
        erode(
            mesh,
            &Surface {
                elevation,
                sediment,
                bedrock_density: &vec![2800.0; n],
                bedrock_isostatic_density: &vec![2800.0; n],
                dense_crust: &vec![0.0; n],
                precipitation: &vec![precipitation; n],
                runoff: &vec![precipitation; n],
                potential_evaporation: &vec![0.0; n],
                plate,
                class,
                ocean_volume: ocean_volume(mesh, elevation, sediment, params.planet_radius),
                water_volume: ocean_volume(mesh, elevation, sediment, params.planet_radius),
                ocean_seeds: &sea::ocean_seeds(
                    mesh,
                    elevation,
                    -sea::SILL_TOLERANCE,
                    sea::MIN_SEA_FRACTION,
                ),
                previous_lakes: &[],
                sample: &[],
                temperature: &[],
                ice: &[],
                mafic: &[],
                regolith: &[],
            },
            params,
            1e6,
        )
    }

    /// Surface elevations after an erosion step, relative to the sea level after it.
    fn after_step(
        mesh: &SphereMesh,
        elevation: &[f64],
        sediment: &[f64],
        result: &ErosionResult,
        params: &ErosionParams,
    ) -> Vec<f64> {
        let new_sediment: Vec<f64> = sediment
            .iter()
            .zip(&result.sediment_delta)
            .map(|(s, d)| s + d)
            .collect();
        let new_elevation: Vec<f64> = (0..elevation.len())
            .map(|c| {
                let e = isostasy::change_sediment(elevation[c], sediment[c], new_sediment[c]);
                isostasy::add_layer(e, result.bedrock_delta[c], 2800.0)
            })
            .collect();
        let cell_area =
            4.0 * std::f64::consts::PI * params.planet_radius.powi(2) / elevation.len() as f64;
        let volume = ocean_volume(mesh, elevation, sediment, params.planet_radius);
        let rise = sea::relative_sea_level(
            mesh,
            &new_elevation,
            &new_sediment,
            cell_area,
            (volume, sea::MIN_SEA_FRACTION),
        );
        let total = result.sea_level_change.fluvial + result.sea_level_change.coastal;
        assert!((rise - total).abs() < 1e-6, "{rise} vs {total}");
        new_elevation
            .iter()
            .map(|&e| isostasy::to_surface(isostasy::to_dry(e) - rise))
            .collect()
    }

    /// The ocean volume (m³) over the cells of a mesh of the given radius, including the pores of
    /// submerged sediment: the sea level stays where it is. Only the ocean counts, not closed
    /// depressions below sea level.
    fn ocean_volume(mesh: &SphereMesh, elevation: &[f64], sediment: &[f64], radius: f64) -> f64 {
        let cell_area = 4.0 * std::f64::consts::PI * radius * radius / elevation.len() as f64;
        let ocean = sea::ocean_mask(mesh, elevation, sea::MIN_SEA_FRACTION, true);
        elevation
            .iter()
            .zip(sediment)
            .zip(&ocean)
            .filter(|&(_, &o)| o)
            .map(|(e, _)| e)
            .map(|(&e, &s)| -e + isostasy::sediment_pores(s))
            .sum::<f64>()
            * cell_area
    }

    #[test]
    fn sediment_erodes_faster_than_bedrock() {
        let mesh = SphereMesh::icosphere(4);
        let elevation = island(&mesh);
        let class = classes(&elevation, -1000.0);
        let plate = vec![0; mesh.len()];
        let params = ErosionParams {
            sediment_diffusivity: 0.0,
            waves: false,
            regolith: without_regolith(),
            ..ErosionParams::default()
        };
        // The same island with a thick sediment cover on land, at the same elevations.
        let covered: Vec<f64> = elevation
            .iter()
            .map(|&e| if e > 0.0 { 5000.0 } else { 0.0 })
            .collect();
        let bare = run_with(
            &mesh,
            &elevation,
            &vec![0.0; mesh.len()],
            1.0,
            &plate,
            &class,
            &params,
        );
        let soft = run_with(&mesh, &elevation, &covered, 1.0, &plate, &class, &params);
        let removed = |r: &ErosionResult| -> f64 {
            (0..mesh.len())
                .filter(|&c| elevation[c] > 0.0)
                .map(|c| -(r.bedrock_delta[c] + r.sediment_delta[c]))
                .sum()
        };
        assert!(
            removed(&soft) > 1.5 * removed(&bare),
            "{} {}",
            removed(&soft),
            removed(&bare)
        );
        // The covered island loses sediment, not bedrock, where the cover is thick.
        assert!(soft.bedrock_delta.iter().all(|&d| d == 0.0));
    }

    #[test]
    fn sediment_spreads_from_coastal_plains_into_the_sea() {
        let mesh = SphereMesh::icosphere(4);
        // A flat coastal plain at +50 m with 2 km of sediment next to a shelf at −1000 m.
        let elevation: Vec<f64> = (0..mesh.len() as u32)
            .map(|cell| {
                if mesh.vertex(cell).z > 0.3 {
                    50.0
                } else {
                    -1000.0
                }
            })
            .collect();
        let sediment: Vec<f64> = elevation
            .iter()
            .map(|&e| if e > 0.0 { 2000.0 } else { 0.0 })
            .collect();
        let class = classes(&elevation, -2000.0);
        let plate = vec![0; mesh.len()];
        let params = ErosionParams {
            sediment_erodibility_factor: 1.0,
            ..ErosionParams::default()
        };
        let result = run_with(&mesh, &elevation, &sediment, 0.0, &plate, &class, &params);
        let to_sea: f64 = (0..mesh.len())
            .filter(|&c| elevation[c] < 0.0)
            .map(|c| result.sediment_delta[c])
            .sum();
        let from_land: f64 = (0..mesh.len())
            .filter(|&c| elevation[c] > 0.0)
            .map(|c| -result.sediment_delta[c])
            .sum();
        assert!(to_sea > 0.0);
        // Conserved, and no cell gives more than it has.
        assert!((to_sea - from_land).abs() < 1e-6 * from_land);
        assert!((0..mesh.len()).all(|c| sediment[c] + result.sediment_delta[c] >= -1e-9));
        // The plain spreads seaward but is not drowned: diffusion lowers land at most to sea
        // level.
        for c in 0..mesh.len() {
            if elevation[c] > 0.0 {
                let after = isostasy::change_sediment(
                    elevation[c],
                    sediment[c],
                    sediment[c] + result.sediment_delta[c],
                );
                assert!(after >= -1e-6, "cell {c}: {after}");
            }
        }
        // Only cells next to the coast change much: diffusion over ~1 cell per Myr.
        let deep_inland = (0..mesh.len() as u32)
            .filter(|&c| mesh.vertex(c).z > 0.8)
            .map(|c| result.sediment_delta[c as usize].abs())
            .fold(0.0, f64::max);
        assert!(deep_inland < 1.0, "{deep_inland}");
    }

    #[test]
    fn all_water_reaches_the_ocean() {
        let mesh = SphereMesh::icosphere(4);
        let mut elevation = island(&mesh);
        // Dig a pit on the flank.
        let pit = mesh.nearest(glam::DVec3::new(0.05, 0.0, 1.0).normalize()) as usize;
        elevation[pit] -= 1500.0;
        assert!(
            mesh.neighbors(pit as u32)
                .iter()
                .all(|&n| elevation[n as usize] > elevation[pit])
        );
        let class = classes(&elevation, -1000.0);
        let result = run(&mesh, &elevation, 1.0, &vec![0; mesh.len()], &class);

        let cell_area = 4.0 * std::f64::consts::PI * 6.371e6f64.powi(2) / mesh.len() as f64;
        let land_cells = elevation.iter().filter(|&&e| e >= 0.0).count() as f64;
        let into_ocean: f64 = (0..mesh.len())
            .filter(|&c| elevation[c] < 0.0)
            .map(|c| result.discharge[c])
            .sum();
        assert!((into_ocean - land_cells * cell_area).abs() / into_ocean < 1e-9);

        // The pit collects sediment.
        assert!(result.sediment_delta[pit] > 0.0);
    }

    const CRUST: Bedrock = Bedrock {
        dense: 0.0,
        dense_density: 2800.0,
        density: 2800.0,
    };

    #[test]
    fn incision_through_dense_lower_crust_matches_the_crust_model() {
        // 2 km of densified lower crust (response ~0.10 per m) above normal crust (0.15): a big
        // river cuts through both, and the balance holds with the piecewise response.
        let bedrock = Bedrock {
            dense: 2000.0,
            dense_density: 2970.0,
            density: 2800.0,
        };
        let (elevation, base, factor) = (3000.0, 100.0, 50.0);
        let cut = incise(
            elevation,
            base,
            0.0,
            &bedrock,
            (2.0 * factor, factor),
            1.0,
            true,
        );
        assert!(cut.bedrock > bedrock.dense);
        let after = isostasy::add_layer(
            isostasy::add_layer(elevation, -bedrock.dense, bedrock.dense_density),
            -(cut.bedrock - bedrock.dense),
            bedrock.density,
        );
        assert!((cut.bedrock - factor * (after - base)).abs() < 1e-6);
        assert!(after > base);
        // A small cut stays in the dense part.
        let small = incise(elevation, base, 0.0, &bedrock, (0.02, 0.01), 1.0, true);
        let after = isostasy::add_layer(elevation, -small.bedrock, bedrock.dense_density);
        assert!(small.bedrock < bedrock.dense);
        assert!((small.bedrock - 0.01 * (after - base)).abs() < 1e-9);
    }

    #[test]
    fn incision_accounts_for_the_isostatic_rebound() {
        // Bedrock: the removed thickness solves h = F (z(h) − z_r) with the surface after the
        // rebound, so a big river (F = 10) removes ~4 km and lowers the surface to ~400 m,
        // instead of removing ~0.9 km as if the surface dropped by the removed thickness.
        let (elevation, base, factor) = (1000.0, 0.0, 10.0);
        let cut = incise(
            elevation,
            base,
            0.0,
            &CRUST,
            (2.0 * factor, factor),
            1.0,
            true,
        );
        let after = isostasy::add_layer(elevation, -cut.bedrock, 2800.0);
        assert_eq!(cut.bulk, 0.0);
        assert!((cut.bedrock - factor * (after - base)).abs() < 1e-6);
        assert!((cut.bedrock - 3976.0).abs() < 1.0, "{}", cut.bedrock);
        // A small stream (F ≪ 1) barely notices the rebound.
        let small = incise(elevation, base, 0.0, &CRUST, (0.02, 0.01), 1.0, true);
        assert!(
            (small.bedrock - 0.01 * 1000.0).abs() < 0.2,
            "{}",
            small.bedrock
        );

        // Sediment: the step ends in the sediment and the same balance holds with the
        // compacting column.
        let sediment = 3000.0;
        let cut = incise(elevation, base, sediment, &CRUST, (0.5, 0.25), 1.0, true);
        assert_eq!(cut.bedrock, 0.0);
        let bulk = isostasy::sediment_bulk_thickness(sediment);
        let left = isostasy::sediment_for_bulk_thickness(bulk - cut.bulk);
        let after = isostasy::change_sediment(elevation, sediment, left);
        assert!(cut.bulk < bulk);
        assert!((cut.bulk - 0.5 * (after - base)).abs() < 1e-6);

        // Through thin sediment into bedrock: the surface never drops below the base level.
        for factor in [0.1, 1.0, 10.0, 1e3] {
            let cut = incise(
                elevation,
                200.0,
                50.0,
                &CRUST,
                (2.0 * factor, factor),
                1.0,
                true,
            );
            let bulk = isostasy::sediment_bulk_thickness(50.0);
            assert_eq!(cut.bulk, bulk);
            let bare = isostasy::change_sediment(elevation, 50.0, 0.0);
            let after = isostasy::add_layer(bare, -cut.bedrock, 2800.0);
            assert!(after >= 200.0 - 1e-9 && after < bare, "{factor}: {after}");
        }
    }

    #[test]
    fn lakes_rise_until_evaporation_balances_the_inflow() {
        let elevation = [100.0, 200.0, 300.0];
        let cells = [0, 1, 2];
        // Each cell evaporates 10 units as open water.
        let balance = lake_balance(&cells, &elevation, 400.0, 15.0, |_| 10.0);
        assert!(balance.endorheic);
        assert!((balance.covered - 1.5).abs() < 1e-12);
        assert!((balance.level - 250.0).abs() < 1e-9);
        assert_eq!(balance.evaporation, 15.0);
        // A wetter lake reaches its pass and spills the rest.
        let balance = lake_balance(&cells, &elevation, 400.0, 45.0, |_| 10.0);
        assert!(!balance.endorheic);
        assert_eq!(balance.level, 400.0);
        assert_eq!(balance.evaporation, 30.0);
    }

    /// One erosion step with uniform precipitation, runoff and potential evaporation (m/yr).
    fn run_climate(
        mesh: &SphereMesh,
        elevation: &[f64],
        (precipitation, runoff, potential): (f64, f64, f64),
    ) -> ErosionResult {
        let n = mesh.len();
        let params = ErosionParams {
            sediment_diffusivity: 0.0,
            waves: false,
            ..ErosionParams::default()
        };
        let sediment = vec![0.0; n];
        erode(
            mesh,
            &Surface {
                elevation,
                sediment: &sediment,
                bedrock_density: &vec![2800.0; n],
                bedrock_isostatic_density: &vec![2800.0; n],
                dense_crust: &vec![0.0; n],
                precipitation: &vec![precipitation; n],
                runoff: &vec![runoff; n],
                potential_evaporation: &vec![potential; n],
                plate: &vec![0; n],
                class: &classes(elevation, -1000.0),
                ocean_volume: ocean_volume(mesh, elevation, &sediment, params.planet_radius),
                water_volume: ocean_volume(mesh, elevation, &sediment, params.planet_radius),
                ocean_seeds: &sea::ocean_seeds(
                    mesh,
                    elevation,
                    -sea::SILL_TOLERANCE,
                    sea::MIN_SEA_FRACTION,
                ),
                previous_lakes: &[],
                sample: &[],
                temperature: &[],
                ice: &[],
                mafic: &[],
                regolith: &[],
            },
            &params,
            1e6,
        )
    }

    #[test]
    fn arid_closed_basins_keep_their_water_and_sediment() {
        let mesh = SphereMesh::icosphere(4);
        let mut elevation = island(&mesh);
        // A closed basin on the flank, below its rim on all sides.
        let pit = mesh.nearest(glam::DVec3::new(0.05, 0.0, 1.0).normalize()) as usize;
        elevation[pit] -= 1500.0;
        let cell_area = 4.0 * std::f64::consts::PI * 6.371e6f64.powi(2) / mesh.len() as f64;
        let into_ocean = |result: &ErosionResult| -> f64 {
            (0..mesh.len())
                .filter(|&c| elevation[c] < 0.0)
                .map(|c| result.discharge[c])
                .sum()
        };

        // Humid: the lake evaporates a little and spills the rest.
        let humid = run_climate(&mesh, &elevation, (1.0, 0.6, 0.5));
        assert_eq!(humid.hydrology.endorheic_cells, 0.0);
        assert!(humid.hydrology.lake_evaporation > 0.0);
        let expected = humid.hydrology.runoff - humid.hydrology.lake_evaporation;
        assert!((into_ocean(&humid) - expected).abs() / expected < 1e-9);
        assert!((humid.hydrology.to_ocean - expected).abs() / expected < 1e-9);
        assert!(humid.sediment_delta[pit] > 0.0);

        // Arid: the basin keeps its water; its catchment is endorheic, the rest still drains.
        let arid = run_climate(&mesh, &elevation, (0.2, 0.01, 2.0));
        assert!(arid.hydrology.endorheic_cells >= 1.0);
        assert!(arid.hydrology.lake_cells < 1.0);
        let expected = arid.hydrology.runoff - arid.hydrology.lake_evaporation;
        assert!((into_ocean(&arid) - expected).abs() / expected < 1e-9);
        assert!(
            (arid.hydrology.lake_evaporation - arid.hydrology.endorheic_cells * 0.01 * cell_area)
                .abs()
                < 1e-6 * arid.hydrology.lake_evaporation
        );
        // All the sediment its catchment produced stays in the basin.
        assert!(arid.sediment_delta[pit] > 0.0);
        assert!(arid.hydrology.lake_sediment > 0.0);

        // The lake evaporation per cell, which the climate returns to the air (#126), adds up
        // to the evaporation the balance removed, spilling or endorheic.
        for result in [&humid, &arid] {
            let per_cell: f64 = result.water.lake_evaporation.iter().sum::<f64>() * cell_area;
            let removed = result.hydrology.lake_evaporation;
            assert!(
                (per_cell - removed).abs() <= 1e-9 * removed,
                "{per_cell} {removed}"
            );
            assert!(
                (0..mesh.len())
                    .all(|c| result.water.lake_evaporation[c] == 0.0 || !result.water.ocean[c])
            );
        }
    }

    /// The island with a closed depression below sea level near its top (#58), and the
    /// depression's cell.
    fn island_with_depression(mesh: &SphereMesh) -> (Vec<f64>, usize) {
        let mut elevation = island(mesh);
        let pit = mesh.nearest(glam::DVec3::new(0.05, 0.0, 1.0).normalize()) as usize;
        elevation[pit] = -400.0;
        (elevation, pit)
    }

    /// One erosion step with all processes and uniform precipitation, runoff and potential
    /// evaporation (m/yr).
    fn run_all(
        mesh: &SphereMesh,
        elevation: &[f64],
        sediment: &[f64],
        (precipitation, runoff, potential): (f64, f64, f64),
        params: &ErosionParams,
    ) -> ErosionResult {
        let n = mesh.len();
        erode(
            mesh,
            &Surface {
                elevation,
                sediment,
                bedrock_density: &vec![2800.0; n],
                bedrock_isostatic_density: &vec![2800.0; n],
                dense_crust: &vec![0.0; n],
                precipitation: &vec![precipitation; n],
                runoff: &vec![runoff; n],
                potential_evaporation: &vec![potential; n],
                plate: &vec![0; n],
                class: &classes(elevation, -1000.0),
                ocean_volume: ocean_volume(mesh, elevation, sediment, params.planet_radius),
                water_volume: ocean_volume(mesh, elevation, sediment, params.planet_radius),
                ocean_seeds: &sea::ocean_seeds(
                    mesh,
                    elevation,
                    -sea::SILL_TOLERANCE,
                    sea::MIN_SEA_FRACTION,
                ),
                previous_lakes: &[],
                sample: &[],
                temperature: &[],
                ice: &[],
                mafic: &[],
                regolith: &[],
            },
            params,
            1e6,
        )
    }

    #[test]
    fn closed_depressions_below_sea_level_are_lakes_not_sea() {
        let mesh = SphereMesh::icosphere(4);
        let (elevation, pit) = island_with_depression(&mesh);
        assert!(!sea::ocean_mask(&mesh, &elevation, sea::MIN_SEA_FRACTION, true)[pit]);
        let sediment = vec![0.0; mesh.len()];
        let params = ErosionParams::default();

        // Arid: the depression is a dry basin (or a small lake) that keeps the water and the
        // sediment of its catchment; it is not a river mouth.
        let arid = run_all(&mesh, &elevation, &sediment, (0.2, 0.01, 2.0), &params);
        let hydrology = arid.hydrology;
        assert_eq!(hydrology.inland_cells, 1.0);
        assert_eq!(hydrology.inland_depressions, 1);
        assert!(hydrology.inland_water_cells < 1.0);
        assert!(hydrology.endorheic_cells >= 1.0);
        assert!(arid.sediment_delta[pit] > 0.0);
        assert!(hydrology.lake_sediment > 0.0);
        // The sea-level changes are those of the ocean only (`after_step` checks them).
        after_step(&mesh, &elevation, &sediment, &arid, &params);

        // Humid: the lake fills the depression and spills over its rim.
        let humid = run_all(&mesh, &elevation, &sediment, (1.0, 0.6, 0.5), &params);
        let hydrology = humid.hydrology;
        assert_eq!(hydrology.endorheic_cells, 0.0);
        assert!((hydrology.inland_water_cells - 1.0).abs() < 1e-12);
        assert_eq!(hydrology.dry_inland_depressions, 0);
        assert!(hydrology.lake_volume > 400.0 * 1e9);
        let expected = hydrology.runoff - hydrology.lake_evaporation;
        assert!((hydrology.to_ocean - expected).abs() / expected < 1e-9);
        after_step(&mesh, &elevation, &sediment, &humid, &params);

        // Rivers cut down towards the dry floor, not only to sea level: with the same
        // depression connected to the ocean, its neighbours would be cut to sea level at most.
        assert!(arid.bedrock_delta.iter().any(|&d| d < 0.0));
    }

    #[test]
    fn the_sea_pours_over_its_sill_into_the_basin_behind_it() {
        // A basin of one cell 400 m deep on the island, joined to the ocean by a channel whose
        // floor lies exactly at sea level: the sea stands on that sill.
        let mesh = SphereMesh::icosphere(4);
        let (mut elevation, pit) = island_with_depression(&mesh);
        let towards = mesh.vertex(pit as u32);
        for (cell, e) in elevation.iter_mut().enumerate() {
            let v = mesh.vertex(cell as u32);
            if cell != pit && *e > 0.0 && v.y.abs() < 0.06 && v.x > towards.x {
                *e = 0.0;
            }
        }
        let sediment = vec![0.0; mesh.len()];
        let params = ErosionParams::default();
        let n = mesh.len();
        let cell_area = 4.0 * std::f64::consts::PI * params.planet_radius.powi(2) / n as f64;
        // The basin holds 400 m of water per cell area below the sill (as deep as the ocean
        // would stand over it).
        let capacity = 400.0 * cell_area;
        // The ocean below sea level (`ocean_volume` counts the basin behind the sill as ocean).
        let ocean_volume =
            ocean_volume(&mesh, &elevation, &sediment, params.planet_radius) - capacity;
        let seeds = sea::ocean_seeds(&mesh, &elevation, -sea::SILL_TOLERANCE, 1.0);
        let class = classes(&elevation, -1000.0);
        let arid = |water_volume: f64| {
            let surface = Surface {
                elevation: &elevation,
                sediment: &sediment,
                bedrock_density: &vec![2800.0; n],
                bedrock_isostatic_density: &vec![2800.0; n],
                dense_crust: &vec![0.0; n],
                precipitation: &vec![0.2; n],
                runoff: &vec![0.0; n],
                potential_evaporation: &vec![2.0; n],
                plate: &vec![0; n],
                class: &class,
                ocean_volume,
                water_volume,
                ocean_seeds: &seeds,
                previous_lakes: &[],
                sample: &[],
                temperature: &[],
                ice: &[],
                mafic: &[],
                regolith: &[],
            };
            let ocean = sea::ocean_from(&mesh, &elevation, seeds.iter().copied(), false);
            assert!(!ocean[pit]);
            let state = water_state(&mesh, &elevation, &ocean, &surface, &params, cell_area, &[]);
            assert_eq!(state.hydrology.lake_volume, 0.0);
            spill_sea(
                (&mesh, &elevation, &sediment),
                ocean,
                state,
                &surface,
                &params,
                cell_area,
            )
        };
        // A sea with a fifth of that more water than fits below the sill: it pours into the
        // basin and stays at the sill. The ocean and the lake hold all the water.
        let state = arid(ocean_volume + 0.2 * capacity);
        let hydrology = state.hydrology;
        assert_eq!(hydrology.sea_fed_lakes, 1);
        assert!(!state.surface_water.ocean[pit]);
        assert!(state.surface_water.lake_depth[pit] > 0.0);
        assert!((hydrology.lake_volume / (0.2 * capacity) - 1.0).abs() < 1e-6);
        assert!((hydrology.sea_water / hydrology.lake_volume - 1.0).abs() < 1e-9);
        assert!((hydrology.sea_spill_deficit / (0.2 * capacity) - 1.0).abs() < 1e-6);
        // It evaporates more than it receives from the land; the sea supplies the rest.
        assert!(hydrology.sea_fed_evaporation > 0.0);
        let dry: Vec<f64> = elevation.iter().map(|&e| isostasy::to_dry(e)).collect();
        let pores = vec![0.0; n];
        let ocean = sea::sea_level_from(
            &mesh,
            &dry,
            &pores,
            cell_area,
            ocean_volume + 0.2 * capacity - hydrology.lake_volume,
            &seeds,
        );
        assert!(
            ocean.level.abs() < 1e-9 && ocean.deficit < 1e-6 * capacity,
            "{ocean:?}"
        );
        // With more water than the basin holds, it joins the ocean.
        let state = arid(ocean_volume + 2.0 * capacity);
        assert!(state.surface_water.ocean[pit]);
        assert_eq!(state.hydrology.sea_fed_lakes, 0);
        assert_eq!(state.hydrology.lake_volume, 0.0);
    }

    #[test]
    fn the_sea_pours_over_a_sill_below_the_sea_level_of_the_frame() {
        // As above, but the channel lies 3 m below the sea level the elevations refer to (the
        // sea fell in the step): in this frame the basin is ocean, but the sea stands on the
        // channel. The basin's sediment holds pores, which the lakes do not count.
        let mesh = SphereMesh::icosphere(4);
        let (mut elevation, pit) = island_with_depression(&mesh);
        let towards = mesh.vertex(pit as u32);
        let mut channel = Vec::new();
        for (cell, e) in elevation.iter_mut().enumerate() {
            let v = mesh.vertex(cell as u32);
            if cell != pit && *e > 0.0 && v.y.abs() < 0.06 && v.x > towards.x {
                *e = -3.0;
                channel.push(cell);
            }
        }
        let n = mesh.len();
        let mut sediment = vec![0.0; n];
        sediment[pit] = 2000.0;
        let params = ErosionParams::default();
        let cell_area = 4.0 * std::f64::consts::PI * params.planet_radius.powi(2) / n as f64;
        let dry: Vec<f64> = elevation.iter().map(|&e| isostasy::to_dry(e)).collect();
        let pores: Vec<f64> = sediment
            .iter()
            .map(|&s| isostasy::sediment_pores(s))
            .collect();
        let sill = dry[channel[0]];
        let seeds = sea::ocean_seeds(&mesh, &elevation, -sea::SILL_TOLERANCE, 1.0);
        // The ocean up to the sill, and the basin below it.
        let below_sill: f64 = (0..n)
            .filter(|&c| c != pit && dry[c] < sill)
            .map(|c| sill - dry[c])
            .sum::<f64>()
            * isostasy::WATER_LOADING
            * cell_area;
        let capacity = (sill - dry[pit]) * isostasy::WATER_LOADING * cell_area;
        let water_volume = below_sill + 0.3 * capacity;
        let class = classes(&elevation, -1000.0);
        let surface = Surface {
            elevation: &elevation,
            sediment: &sediment,
            bedrock_density: &vec![2800.0; n],
            bedrock_isostatic_density: &vec![2800.0; n],
            dense_crust: &vec![0.0; n],
            precipitation: &vec![0.2; n],
            runoff: &vec![0.0; n],
            potential_evaporation: &vec![2.0; n],
            plate: &vec![0; n],
            class: &class,
            ocean_volume: water_volume,
            water_volume,
            ocean_seeds: &seeds,
            previous_lakes: &[],
            sample: &[],
            temperature: &[],
            ice: &[],
            mafic: &[],
            regolith: &[],
        };
        let ocean = sea::ocean_from(&mesh, &elevation, seeds.iter().copied(), false);
        assert!(ocean[pit]);
        let state = water_state(&mesh, &elevation, &ocean, &surface, &params, cell_area, &[]);
        let state = spill_sea(
            (&mesh, &elevation, &sediment),
            ocean,
            state,
            &surface,
            &params,
            cell_area,
        );
        let hydrology = state.hydrology;
        assert!(!state.surface_water.ocean[pit]);
        assert_eq!(hydrology.sea_fed_lakes, 1);
        // The lake stands below the sill and holds the water the ocean has above it.
        assert!((hydrology.lake_volume / (0.3 * capacity) - 1.0).abs() < 1e-6);
        let level = isostasy::to_dry(elevation[pit])
            + state.surface_water.lake_depth[pit] / isostasy::WATER_LOADING;
        assert!(level < sill, "{level} {sill}");
        // The ocean, holding the rest, stands on the sill without a deficit.
        let solved = sea::sea_level_from(
            &mesh,
            &dry,
            &pores,
            cell_area,
            water_volume - hydrology.lake_volume,
            &seeds,
        );
        assert!(
            (solved.level - sill).abs() < 1e-9 && solved.deficit < 1e-6 * capacity,
            "{solved:?} {sill}"
        );
    }

    #[test]
    fn waves_leave_closed_depressions_alone() {
        let mesh = SphereMesh::icosphere(4);
        let (elevation, pit) = island_with_depression(&mesh);
        let sediment = vec![0.0; mesh.len()];
        let params = ErosionParams {
            sediment_diffusivity: 0.0,
            regolith: without_regolith(),
            ..ErosionParams::default()
        };
        // No rain: only waves act. The rim of the depression is no coast.
        let result = run_all(&mesh, &elevation, &sediment, (0.0, 0.0, 1.0), &params);
        for &neighbor in mesh.neighbors(pit as u32) {
            assert_eq!(result.bedrock_delta[neighbor as usize], 0.0);
        }
        assert_eq!(result.bedrock_delta[pit], 0.0);
        assert_eq!(result.sediment_delta[pit], 0.0);
        // The real coast does retreat.
        assert!(result.wave.cliff_bedrock > 0.0);
        // A deep depression is no open water for the wave base either.
        let mut deep = elevation.clone();
        deep[pit] = -3000.0;
        let bases = wave_bases(&mesh, &deep, sea::MIN_SEA_FRACTION, &params);
        let ocean = sea::ocean_mask(&mesh, &deep, sea::MIN_SEA_FRACTION, true);
        let mut open = deep.clone();
        open[pit] = 0.0;
        assert!(!ocean[pit]);
        assert_eq!(
            bases,
            wave_bases(&mesh, &open, sea::MIN_SEA_FRACTION, &params)
        );
    }

    #[test]
    fn land_sediment_spreads_down_into_dry_depressions() {
        let mesh = SphereMesh::icosphere(4);
        let (elevation, pit) = island_with_depression(&mesh);
        let neighbor = mesh.neighbors(pit as u32)[0] as usize;
        let params = ErosionParams::default();
        let spread = |ocean: &[bool]| {
            let mut sediment = vec![0.0; mesh.len()];
            sediment[neighbor] = 100.0;
            let mut seabed = Seabed {
                elevation: elevation.clone(),
                sediment,
                delta: vec![0.0; mesh.len()],
            };
            diffuse_sediment(
                &mesh,
                &mut seabed,
                (ocean, &vec![0.0; mesh.len()]),
                &params,
                1e6,
            );
            seabed.delta[pit]
        };
        let ocean = sea::ocean_mask(&mesh, &elevation, sea::MIN_SEA_FRACTION, true);
        let dry = spread(&ocean);
        // As sea (the old treatment), land would be lowered at most to sea level.
        let mut as_sea = ocean.clone();
        as_sea[pit] = true;
        let sea = spread(&as_sea);
        assert!(sea > 0.0 && dry > sea, "{dry} vs {sea}");
    }

    #[test]
    fn erosion_lowers_land_and_builds_shelves() {
        let mesh = SphereMesh::icosphere(4);
        let elevation = island(&mesh);
        let class = classes(&elevation, -1000.0);
        let result = run(&mesh, &elevation, 1.0, &vec![0; mesh.len()], &class);

        assert!(result.bedrock_delta.iter().all(|&d| d <= 0.0));
        assert!(result.bedrock_delta.iter().any(|&d| d < 0.0));
        assert!(result.sediment_to_ocean > 0.0);
        // Sediment ends up in the ocean, building the shelf around the island.
        assert!((0..mesh.len()).any(|c| elevation[c] < 0.0 && result.sediment_delta[c] > 0.0));
        // Land is not raised except by lake deposits; this island has no lakes.
        for cell in 0..mesh.len() {
            if elevation[cell] >= 0.0 {
                assert!(result.bedrock_delta[cell] + result.sediment_delta[cell] <= 1e-9);
            }
        }
    }

    #[test]
    fn erosion_conserves_rock_mass() {
        let mesh = SphereMesh::icosphere(4);
        let elevation = island(&mesh);
        // No margin crust at sea and heavy rain: fans fill too.
        let class = classes(&elevation, 0.0);
        let result = run(&mesh, &elevation, 20.0, &vec![0; mesh.len()], &class);
        assert!(result.deposited[Zone::Fan as usize] > 0.0);
        let change: f64 = (0..mesh.len())
            .map(|c| {
                result.bedrock_delta[c] * 2800.0 + result.sediment_delta[c] * SEDIMENT_ROCK_DENSITY
            })
            .sum();
        let eroded: f64 = result.bedrock_delta.iter().map(|d| -d * 2800.0).sum();
        assert!(eroded > 0.0);
        // Nothing is lost: eroded bedrock becomes sediment at equal mass.
        assert!(
            change.abs() < 1e-9 * eroded,
            "change {change}, eroded {eroded}"
        );
    }

    #[test]
    fn margins_fill_before_oceanic_crust() {
        let mesh = SphereMesh::icosphere(4);
        // Continental crust reaches far out to sea: the shelf has plenty of room.
        let elevation = island(&mesh);
        let class = classes(&elevation, -3000.0);
        let result = run(&mesh, &elevation, 1.0, &vec![0; mesh.len()], &class);
        assert!(result.sediment_to_ocean > 0.0);
        for cell in 0..mesh.len() {
            if class[cell] == CrustClass::Oceanic {
                assert_eq!(result.sediment_delta[cell], 0.0, "cell {cell}");
            }
        }
        assert!(result.deposited[Zone::Shelf as usize] > 0.0);
    }

    #[test]
    fn oceanic_crust_is_never_filled_to_shelf_depth() {
        let mesh = SphereMesh::icosphere(4);
        // A steep coast: deep ocean right off the island.
        let elevation: Vec<f64> = island(&mesh)
            .iter()
            .map(|&e| if e < 0.0 { -4500.0 } else { e })
            .collect();
        let class = classes(&elevation, 0.0);
        let params = ErosionParams::default();
        let result = run(&mesh, &elevation, 5.0, &vec![0; mesh.len()], &class);
        assert_eq!(result.sediment_overfill, 0.0);
        assert!(
            result.deposited[Zone::Slope as usize] + result.deposited[Zone::Rise as usize] > 0.0,
            "{:?}",
            result.deposited
        );
        for cell in 0..mesh.len() {
            if class[cell] == CrustClass::Oceanic && result.sediment_delta[cell] > 0.0 {
                let after =
                    isostasy::change_sediment(elevation[cell], 0.0, result.sediment_delta[cell]);
                assert!(after <= params.slope_depth + 1e-6, "cell {cell}: {after}");
                assert!(result.sediment_delta[cell] <= params.max_slope_sediment + 1e-6);
            }
        }
    }

    /// The island with a pit `depth` m below its lowest neighbour on the flank, and the pit.
    fn island_with_pit(mesh: &SphereMesh, depth: f64, angle: f64) -> (Vec<f64>, usize) {
        let mut elevation = island(mesh);
        let (s, c) = math::sin_cos(angle.to_radians());
        let pit = mesh.nearest(glam::DVec3::new(s, 0.0, c)) as usize;
        let lowest = mesh
            .neighbors(pit as u32)
            .iter()
            .map(|&n| elevation[n as usize])
            .fold(f64::INFINITY, f64::min);
        elevation[pit] = lowest - depth;
        (elevation, pit)
    }

    /// One erosion step of the island without any sediment (no incision, diffusion or waves),
    /// humid, with the lakes of the previous step.
    fn run_without_sediment(
        mesh: &SphereMesh,
        elevation: &[f64],
        previous_lakes: &[bool],
    ) -> ErosionResult {
        let n = mesh.len();
        let params = ErosionParams {
            erodibility: 0.0,
            sediment_diffusivity: 0.0,
            waves: false,
            regolith: without_regolith(),
            ..ErosionParams::default()
        };
        let sediment = vec![0.0; n];
        erode(
            mesh,
            &Surface {
                elevation,
                sediment: &sediment,
                bedrock_density: &vec![2800.0; n],
                bedrock_isostatic_density: &vec![2800.0; n],
                dense_crust: &vec![0.0; n],
                precipitation: &vec![1.0; n],
                runoff: &vec![0.6; n],
                potential_evaporation: &vec![0.5; n],
                plate: &vec![0; n],
                class: &classes(elevation, -1000.0),
                ocean_volume: ocean_volume(mesh, elevation, &sediment, params.planet_radius),
                water_volume: ocean_volume(mesh, elevation, &sediment, params.planet_radius),
                ocean_seeds: &sea::ocean_seeds(
                    mesh,
                    elevation,
                    -sea::SILL_TOLERANCE,
                    sea::MIN_SEA_FRACTION,
                ),
                previous_lakes,
                sample: &[],
                temperature: &[],
                ice: &[],
                mafic: &[],
                regolith: &[],
            },
            &params,
            1e6,
        )
    }

    #[test]
    fn shallow_new_depressions_hold_no_lake_but_lakes_persist() {
        let mesh = SphereMesh::icosphere(4);
        let n = mesh.len();
        let onset = ErosionParams::default().lake_onset_depth;
        // A new shallow depression: its water passes the spill, nothing is endorheic.
        let (shallow, pit) = island_with_pit(&mesh, 0.5 * onset, 10.0);
        let result = run_without_sediment(&mesh, &shallow, &[]);
        assert!(result.water.depression[pit]);
        assert_eq!(result.water.lake_depth[pit], 0.0);
        assert_eq!(result.hydrology.lakes, 0);
        assert_eq!(result.hydrology.depressions, 1);
        // The same depression where a lake was last step (on it or next to it, as a lake moves
        // with its plate): the lake persists.
        let mut previous = vec![false; n];
        previous[mesh.neighbors(pit as u32)[0] as usize] = true;
        let result = run_without_sediment(&mesh, &shallow, &previous);
        assert!(
            (result.water.lake_depth[pit] - isostasy::WATER_LOADING * 0.5 * onset).abs() < 0.02,
            "{} {:?}",
            result.water.lake_depth[pit],
            result.hydrology
        );
        assert_eq!(result.hydrology.lakes, 1);
        // A new deep depression holds a lake.
        let (deep, pit) = island_with_pit(&mesh, 2.0 * onset, 10.0);
        let result = run_without_sediment(&mesh, &deep, &[]);
        assert!(
            (result.water.lake_depth[pit] - isostasy::WATER_LOADING * 2.0 * onset).abs() < 0.02
        );
        // Its water presses the floor down: the lake is 1.45× deeper than the dry basin, its
        // surface stays at the spill.
        let correction = result.water.load_correction[pit];
        assert!(
            (correction + (isostasy::WATER_LOADING - 1.0) * 2.0 * onset).abs() < 0.02,
            "{correction}"
        );
        assert!(
            result
                .water
                .load_correction
                .iter()
                .filter(|&&c| c != 0.0)
                .count()
                == 1
        );
    }

    #[test]
    fn lakes_below_sea_level_stand_on_their_unloaded_floor() {
        let mesh = SphereMesh::icosphere(4);
        let (elevation, pit) = island_with_depression(&mesh);
        // Humid, no sediment: the lake fills the depression to its rim.
        let result = run_without_sediment(&mesh, &elevation, &[]);
        let rim = mesh
            .neighbors(pit as u32)
            .iter()
            .map(|&n| elevation[n as usize])
            .fold(f64::INFINITY, f64::min);
        let floor = isostasy::to_dry(elevation[pit]);
        let depth = result.water.lake_depth[pit];
        assert!(
            (depth - isostasy::WATER_LOADING * (rim - floor)).abs() < 0.1,
            "{depth} {rim} {floor}"
        );
        // The pressed floor lies the lake's depth below its surface.
        let shown = elevation[pit] + result.water.load_correction[pit];
        assert!((rim - depth - shown).abs() < 0.1, "{shown}");
        assert!(result.hydrology.lake_volume > 0.0);
    }

    #[test]
    fn dry_depressions_below_sea_level_carry_no_water_load() {
        let mesh = SphereMesh::icosphere(4);
        let (elevation, pit) = island_with_depression(&mesh);
        let sediment = vec![0.0; mesh.len()];
        // Hyper-arid and without runoff: the basin stays dry.
        let params = ErosionParams {
            erodibility: 0.0,
            sediment_diffusivity: 0.0,
            waves: false,
            ..ErosionParams::default()
        };
        let result = run_all(&mesh, &elevation, &sediment, (0.0, 0.0, 2.0), &params);
        assert_eq!(result.water.lake_depth[pit], 0.0);
        assert!(!result.water.ocean[pit]);
        // The processes see it loaded by the ocean (−400 m); unloaded it is 31% shallower.
        let shown = elevation[pit] + result.water.load_correction[pit];
        assert!(
            (shown - isostasy::to_dry(elevation[pit])).abs() < 1e-9,
            "{shown}"
        );
        // The ocean keeps its load.
        for cell in 0..mesh.len() {
            if result.water.ocean[cell] {
                assert_eq!(result.water.load_correction[cell], 0.0);
            }
        }
    }

    #[test]
    fn lakes_filled_by_the_steps_sediment_are_dry_at_its_end() {
        let mesh = SphereMesh::icosphere(4);
        let params = ErosionParams::default();
        // A depression on the lower flank, below the island's rivers.
        let (elevation, pit) = island_with_pit(&mesh, 2.0 * params.lake_onset_depth, 12.0);
        let sediment = vec![0.0; mesh.len()];
        let result = run_all(&mesh, &elevation, &sediment, (1.0, 0.6, 0.5), &params);
        assert!(
            result.hydrology.filled_depressions >= 1,
            "{:?}",
            result.hydrology
        );
        assert!(result.sediment_delta[pit] > 0.0);
        assert_eq!(result.water.lake_depth[pit], 0.0);
        assert_eq!(result.hydrology.lake_volume, 0.0);
    }

    #[test]
    fn closed_depressions_fill_as_lakes() {
        let mesh = SphereMesh::icosphere(4);
        let mut elevation = island(&mesh);
        // A small closed depression below sea level on the island's flank, on oceanic crust.
        // It used to be an enclosed sea that rivers ended in, filled to the delta top, with a
        // terminal fan around it for the rest; it is a lake now (#58).
        let pit = mesh.nearest(glam::DVec3::new(0.15, 0.0, 1.0).normalize()) as usize;
        elevation[pit] = -1000.0;
        let mut class = classes(&elevation, -1000.0);
        class[pit] = CrustClass::Oceanic;
        let result = run(&mesh, &elevation, 1.0, &vec![0; mesh.len()], &class);
        let params = ErosionParams {
            sediment_diffusivity: 0.0,
            waves: false,
            ..ErosionParams::default()
        };
        after_step(&mesh, &elevation, &vec![0.0; mesh.len()], &result, &params);
        assert_eq!(result.hydrology.inland_depressions, 1);
        assert!(result.hydrology.lake_sediment > 0.0);
        assert!(result.sediment_delta[pit] > 0.0);
        // No river ends in it, so no sediment is refused there.
        assert_eq!(result.terminal_fan, 0.0);
        assert_eq!(result.sediment_overfill, 0.0);
    }

    #[test]
    fn delta_plains_are_not_drowned_by_their_own_sediment() {
        // An island with a nearly full shelf (10 m of room) and the deep ocean beyond: the
        // river sediment goes to the slope and fans and raises the sea level, and the rest
        // builds delta plains on the shelf.
        let mesh = SphereMesh::icosphere(4);
        let elevation: Vec<f64> = (0..mesh.len() as u32)
            .map(|cell| {
                let angle = math::angle_between(mesh.vertex(cell), glam::DVec3::Z).to_degrees();
                if angle <= 60.0 {
                    3000.0 - angle * 50.0
                } else if angle <= 62.0 {
                    -130.0
                } else {
                    -4500.0
                }
            })
            .collect();
        let class = classes(&elevation, -1000.0);
        // Thin caps on the oceanic crust, so that the margin and fans fill.
        let params = ErosionParams {
            sediment_diffusivity: 0.0,
            waves: false,
            max_slope_sediment: 260.0,
            max_rise_sediment: 260.0,
            max_fan_sediment: 260.0,
            graded_fans: false,
            terminal_fans: false,
            ..ErosionParams::default()
        };
        let sediment = vec![0.0; mesh.len()];
        let result = run_with(
            &mesh,
            &elevation,
            &sediment,
            3.0,
            &vec![0; mesh.len()],
            &class,
            &params,
        );
        let change = result.sea_level_change;
        assert!(change.fluvial > params.delta_top, "{change:?}");
        // The delta deposits themselves raise it by less than the delta top.
        assert!(change.coastal < params.delta_top, "{change:?}");
        let after = after_step(&mesh, &elevation, &sediment, &result, &params);
        // Delta plains (cells of the former sea that were built up to the delta top) stay above
        // the new sea level.
        let mut deltas = 0;
        for cell in 0..mesh.len() {
            let built =
                isostasy::change_sediment(elevation[cell], 0.0, result.sediment_delta[cell]);
            if elevation[cell] < 0.0 && built > 0.0 {
                deltas += 1;
                assert!(after[cell] > 0.0, "{cell}: {}", after[cell]);
            }
        }
        assert!(deltas > 0);
    }

    #[test]
    fn routing_keeps_the_sea_level_budget() {
        let mesh = SphereMesh::icosphere(4);
        let elevation = island(&mesh);
        let class = classes(&elevation, -1000.0);
        let sediment = vec![0.0; mesh.len()];
        let params = ErosionParams::default();
        let result = run_with(
            &mesh,
            &elevation,
            &sediment,
            1.0,
            &vec![0; mesh.len()],
            &class,
            &params,
        );
        // Deposition in the sea raises the sea level; `after_step` checks that the reported
        // change is the solved one.
        assert!(result.sea_level_change.fluvial > 0.0);
        after_step(&mesh, &elevation, &sediment, &result, &params);
    }

    #[test]
    fn rivers_fill_their_own_plate_first() {
        let mesh = SphereMesh::icosphere(4);
        let elevation = island(&mesh);
        let class = classes(&elevation, -3000.0);
        // The island is on plate 0; the deep shelf on its southern side (x < 0) belongs to
        // plate 1, the rest of the shelf has room enough.
        let plate: Vec<u32> = (0..mesh.len() as u32)
            .map(|cell| u32::from(elevation[cell as usize] < 0.0 && mesh.vertex(cell).x < -0.05))
            .collect();
        let result = run(&mesh, &elevation, 0.2, &plate, &class);
        assert!(result.sediment_to_ocean > 0.0);
        let on_plate = |p: u32| -> f64 {
            (0..mesh.len())
                .filter(|&c| plate[c] == p)
                .map(|c| result.sediment_delta[c].max(0.0))
                .sum()
        };
        assert!(on_plate(0) > 0.0);
        assert_eq!(on_plate(1), 0.0);
    }

    #[test]
    fn fans_thin_with_distance() {
        let mesh = SphereMesh::icosphere(4);
        let elevation = island(&mesh);
        // No margin crust at sea: everything offshore is fan, so heavy rain feeds fans.
        let class = classes(&elevation, 0.0);
        let mut plate = vec![0; mesh.len()];
        for cell in 0..mesh.len() {
            if elevation[cell] < 0.0 {
                plate[cell] = 1;
            }
        }
        let result = run(&mesh, &elevation, 5.0, &plate, &class);
        assert!(result.deposited[Zone::Fan as usize] > 0.0);
        let params = ErosionParams::default();
        // Mean deposit by angular distance from the island's centre, in bands of 5° from the
        // foot of the shelf.
        let mut bands = [(0.0, 0); 6];
        for cell in 0..mesh.len() {
            if elevation[cell] < 0.0 {
                assert!(result.sediment_delta[cell] <= params.max_fan_sediment + 1e-6);
                let angle =
                    math::angle_between(mesh.vertex(cell as u32), glam::DVec3::Z).to_degrees();
                let band = ((angle - 30.0) / 5.0) as usize;
                if angle >= 30.0 && band < bands.len() {
                    bands[band].0 += result.sediment_delta[cell];
                    bands[band].1 += 1;
                }
            }
        }
        let means: Vec<f64> = bands
            .iter()
            .map(|&(sum, n)| sum / n.max(1) as f64)
            .collect();
        assert!(means[0] > 0.0, "{means:?}");
        for pair in means.windows(2) {
            assert!(pair[1] <= pair[0], "{means:?}");
        }
    }

    /// A flat coast at +50 m (north of z = 0.3) with `sediment` m on land, next to an open sea
    /// at −3000 m; no rain, so only waves act.
    fn coast(
        mesh: &SphereMesh,
        sediment: f64,
        params: &ErosionParams,
    ) -> (Vec<f64>, Vec<f64>, ErosionResult) {
        let elevation: Vec<f64> = (0..mesh.len() as u32)
            .map(|cell| {
                if mesh.vertex(cell).z > 0.3 {
                    50.0
                } else {
                    -3000.0
                }
            })
            .collect();
        let sediment: Vec<f64> = elevation
            .iter()
            .map(|&e| if e > 0.0 { sediment } else { 0.0 })
            .collect();
        let class = classes(&elevation, -1000.0);
        let result = run_with(
            mesh,
            &elevation,
            &sediment,
            0.0,
            &vec![0; mesh.len()],
            &class,
            params,
        );
        (elevation, sediment, result)
    }

    #[test]
    fn cliffs_retreat_at_the_rate_of_their_waterline() {
        let mesh = SphereMesh::icosphere(4);
        let params = ErosionParams {
            sediment_diffusivity: 0.0,
            regolith: without_regolith(),
            ..ErosionParams::default()
        };
        let spacing = mesh.mean_edge_angle() * params.planet_radius;
        let coastal = |elevation: &[f64], cell: usize| {
            elevation[cell] > 0.0
                && mesh
                    .neighbors(cell as u32)
                    .iter()
                    .any(|&m| elevation[m as usize] < 0.0)
        };
        // Sediment at the waterline: the cliff retreats by the sediment rate; the part of the
        // column above sea level it passes through goes offshore.
        let (elevation, sediment, result) = coast(&mesh, 2000.0, &params);
        let fraction = params.cliff_retreat_sediment * 1e6 / spacing;
        let need = -isostasy::sediment_for_rise(50.0, 0.0, 2000.0);
        let mut coast_cells = 0;
        for cell in 0..mesh.len() {
            if coastal(&elevation, cell) {
                coast_cells += 1;
                let removed = -result.sediment_delta[cell];
                assert!((removed - fraction * need).abs() < 1e-6 * need, "{removed}");
                assert_eq!(result.bedrock_delta[cell], 0.0);
            } else if elevation[cell] > 0.0 {
                assert_eq!(result.sediment_delta[cell], 0.0);
            }
        }
        assert!(coast_cells > 0);
        assert!((result.wave.cliff_sediment - coast_cells as f64 * fraction * need).abs() < 1e-3);
        assert_eq!(result.wave.stayed, 0.0);
        let _ = sediment;
        // Bedrock at the waterline: it retreats by the bedrock rate, ~17 times slower, and
        // becomes sediment of the same mass.
        let (elevation, _, result) = coast(&mesh, 0.0, &params);
        let fraction = params.cliff_retreat_bedrock * 1e6 / spacing;
        let need = 50.0 / isostasy::dry_response(2800.0);
        for (cell, &delta) in result.bedrock_delta.iter().enumerate() {
            if coastal(&elevation, cell) {
                let removed = -delta;
                assert!((removed - fraction * need).abs() < 1e-6 * need, "{removed}");
            }
        }
        let made = result.wave.cliff_bedrock;
        let eroded: f64 = result.bedrock_delta.iter().map(|d| -d).sum();
        assert!((made - eroded * 2800.0 / SEDIMENT_ROCK_DENSITY).abs() < 1e-6 * made);
    }

    #[test]
    fn waves_rework_the_sea_floor_down_to_the_wave_base() {
        let mesh = SphereMesh::icosphere(4);
        // A narrow shelf at −30 m with 1 km of sediment between land (z > 0.5) and the open
        // sea (z < 0.42), and an enclosed lagoon at −30 m inside the land.
        let lagoon = mesh.nearest(glam::DVec3::Z) as usize;
        let elevation: Vec<f64> = (0..mesh.len() as u32)
            .map(|cell| {
                let z = mesh.vertex(cell).z;
                if cell as usize == lagoon {
                    -30.0
                } else if z > 0.5 {
                    100.0
                } else if z > 0.42 {
                    -30.0
                } else {
                    -3000.0
                }
            })
            .collect();
        let sediment: Vec<f64> = elevation
            .iter()
            .map(|&e| if e > -100.0 && e < 0.0 { 1000.0 } else { 0.0 })
            .collect();
        let class = classes(&elevation, -1000.0);
        let params = ErosionParams {
            sediment_diffusivity: 0.0,
            regolith: without_regolith(),
            ..ErosionParams::default()
        };
        let result = run_with(
            &mesh,
            &elevation,
            &sediment,
            0.0,
            &vec![0; mesh.len()],
            &class,
            &params,
        );
        let after = after_step(&mesh, &elevation, &sediment, &result, &params);
        let rise = isostasy::to_surface(
            -(result.sea_level_change.fluvial + result.sea_level_change.coastal),
        );
        assert!(result.wave.reworked > 0.0);
        for (cell, &e) in after.iter().enumerate() {
            let z = mesh.vertex(cell as u32).z;
            if cell != lagoon && z > 0.42 && z <= 0.5 {
                // Near the open sea: down to the storm wave base (relative to the sea level
                // before the step's own rise).
                assert!(e - rise <= params.wave_base + 1e-6, "{cell}: {e}");
            }
        }
        // The sheltered lagoon is already below its shallow wave base: untouched.
        assert_eq!(result.sediment_delta[lagoon], 0.0);
        // Nothing is lost (the land's cliffs add sediment from bedrock).
        let gained: f64 = result.sediment_delta.iter().sum::<f64>() - result.wave.cliff_bedrock;
        assert!(gained.abs() < 1e-6 * result.wave.reworked, "{gained}");
    }

    #[test]
    fn wave_erosion_conserves_rock_mass() {
        let mesh = SphereMesh::icosphere(4);
        let elevation = island(&mesh);
        let class = classes(&elevation, 0.0);
        // Fast retreat, so that much moves.
        let params = ErosionParams {
            cliff_retreat_sediment: 0.05,
            cliff_retreat_bedrock: 0.01,
            ..ErosionParams::default()
        };
        let sediment = vec![300.0; mesh.len()];
        let result = run_with(
            &mesh,
            &elevation,
            &sediment,
            1.0,
            &vec![0; mesh.len()],
            &class,
            &params,
        );
        assert!(result.wave.cliff_bedrock > 0.0 && result.wave.cliff_sediment > 0.0);
        let change: f64 = (0..mesh.len())
            .map(|c| {
                result.bedrock_delta[c] * 2800.0 + result.sediment_delta[c] * SEDIMENT_ROCK_DENSITY
            })
            .sum();
        let eroded: f64 = result.bedrock_delta.iter().map(|d| -d * 2800.0).sum();
        assert!(
            change.abs() < 1e-9 * eroded,
            "change {change}, eroded {eroded}"
        );
    }

    #[test]
    fn delta_plains_stay_at_the_mouth_and_the_rest_piles_on_land() {
        // A wide shelf (20°–40°) with 10 m of room below the shelf break, and no room on the
        // oceanic crust beyond: delta plains build at the mouths only, the rest piles on land.
        let mesh = SphereMesh::icosphere(4);
        let elevation: Vec<f64> = (0..mesh.len() as u32)
            .map(|cell| {
                let angle = math::angle_between(mesh.vertex(cell), glam::DVec3::Z).to_degrees();
                if angle <= 20.0 {
                    3000.0 - angle * 150.0
                } else if angle <= 40.0 {
                    -130.0
                } else {
                    -4500.0
                }
            })
            .collect();
        let class = classes(&elevation, -1000.0);
        let params = ErosionParams {
            sediment_diffusivity: 0.0,
            waves: false,
            max_slope_sediment: 0.0,
            max_rise_sediment: 0.0,
            max_fan_sediment: 0.0,
            // The model before #47.
            graded_fans: false,
            terminal_fans: false,
            regolith: without_regolith(),
            ..ErosionParams::default()
        };
        let sediment = vec![0.0; mesh.len()];
        let result = run_with(
            &mesh,
            &elevation,
            &sediment,
            3.0,
            &vec![0; mesh.len()],
            &class,
            &params,
        );
        assert!(result.sediment_overfill > 0.0);
        let after = after_step(&mesh, &elevation, &sediment, &result, &params);
        // Rings through the sea from land: the mouths are at ring 1.
        let mut ring = vec![u32::MAX; mesh.len()];
        let mut queue = VecDeque::new();
        for cell in 0..mesh.len() {
            if elevation[cell] >= 0.0 {
                ring[cell] = 0;
                queue.push_back(cell);
            }
        }
        while let Some(cell) = queue.pop_front() {
            for &m in mesh.neighbors(cell as u32) {
                if ring[m as usize] == u32::MAX {
                    ring[m as usize] = ring[cell] + 1;
                    queue.push_back(m as usize);
                }
            }
        }
        let mut deltas = 0;
        for cell in 0..mesh.len() {
            if elevation[cell] < 0.0 && after[cell] >= 0.0 {
                deltas += 1;
                assert!(
                    ring[cell] <= 1 + params.delta_reach,
                    "{cell}: ring {}",
                    ring[cell]
                );
            }
            if elevation[cell] >= 0.0 && result.sediment_delta[cell] > 0.0 {
                // The pile is on the rivers' last land cells, at the coast.
                assert!(
                    mesh.neighbors(cell as u32)
                        .iter()
                        .any(|&m| ring[m as usize] == 1),
                    "{cell}"
                );
            }
        }
        assert!(deltas > 0);
    }

    #[test]
    fn fans_run_downhill() {
        // The island's ocean deepens outwards; a ridge at 40°–45° (−1000 m) separates it from a
        // deeper basin. Turbidity currents cannot climb the ridge.
        let mesh = SphereMesh::icosphere(4);
        let elevation: Vec<f64> = island(&mesh)
            .iter()
            .enumerate()
            .map(|(cell, &e)| {
                let angle =
                    math::angle_between(mesh.vertex(cell as u32), glam::DVec3::Z).to_degrees();
                if (40.0..45.0).contains(&angle) {
                    -1000.0
                } else if angle >= 45.0 {
                    -5000.0
                } else {
                    e
                }
            })
            .collect();
        let class = classes(&elevation, 0.0);
        let result = run(&mesh, &elevation, 20.0, &vec![0; mesh.len()], &class);
        assert!(result.deposited[Zone::Fan as usize] > 0.0);
        for (cell, &delta) in result.sediment_delta.iter().enumerate() {
            let angle = math::angle_between(mesh.vertex(cell as u32), glam::DVec3::Z).to_degrees();
            if angle >= 45.0 {
                assert_eq!(delta, 0.0, "{cell}");
            }
        }
    }

    #[test]
    fn full_fans_aggrade_to_their_graded_surface() {
        // An island in a deep ocean with a load far beyond the lobes' thickness cap: the fan
        // takes all of it, thicker than the cap at its head, and nowhere above its graded
        // surface.
        let mesh = SphereMesh::icosphere(5);
        let elevation = island(&mesh);
        let class = classes(&elevation, 0.0);
        let params = ErosionParams {
            sediment_diffusivity: 0.0,
            waves: false,
            max_fan_sediment: 100.0,
            max_slope_sediment: 100.0,
            max_rise_sediment: 100.0,
            ..ErosionParams::default()
        };
        let sediment = vec![0.0; mesh.len()];
        let plate = vec![0; mesh.len()];
        let result = run_with(&mesh, &elevation, &sediment, 1.0, &plate, &class, &params);
        assert_eq!(result.sediment_overfill, 0.0);
        assert_eq!(result.terminal_fan, 0.0);
        let thickest = result.sediment_delta.iter().copied().fold(0.0, f64::max);
        assert!(thickest > 2.0 * params.max_fan_sediment, "{thickest}");
        for cell in 0..mesh.len() {
            if class[cell] == CrustClass::Oceanic && result.sediment_delta[cell] > 0.0 {
                let built =
                    isostasy::change_sediment(elevation[cell], 0.0, result.sediment_delta[cell]);
                assert!(built <= params.slope_depth + 1e-6, "{cell}: {built}");
            }
        }
        // Without the graded surface, the same load does not fit.
        let capped = ErosionParams {
            graded_fans: false,
            terminal_fans: false,
            ..params
        };
        let result = run_with(&mesh, &elevation, &sediment, 1.0, &plate, &class, &capped);
        assert!(result.sediment_overfill > 0.0);
    }

    #[test]
    fn sediment_without_room_builds_a_cone_instead_of_a_pile() {
        // A wide full shelf and no room on the oceanic crust beyond: what the sea refuses
        // builds deltas and fans whose surface falls with the alluvial slope, so no cell
        // gains more than the fall over one cell above its lowest neighbour.
        let mesh = SphereMesh::icosphere(4);
        let elevation: Vec<f64> = (0..mesh.len() as u32)
            .map(|cell| {
                let angle = math::angle_between(mesh.vertex(cell), glam::DVec3::Z).to_degrees();
                if angle <= 20.0 {
                    3000.0 - angle * 150.0
                } else if angle <= 40.0 {
                    -130.0
                } else {
                    -4500.0
                }
            })
            .collect();
        let class = classes(&elevation, -1000.0);
        let params = ErosionParams {
            sediment_diffusivity: 0.0,
            waves: false,
            max_slope_sediment: 0.0,
            max_rise_sediment: 0.0,
            max_fan_sediment: 0.0,
            graded_fans: false,
            ..ErosionParams::default()
        };
        let sediment = vec![0.0; mesh.len()];
        let plate = vec![0; mesh.len()];
        let result = run_with(&mesh, &elevation, &sediment, 3.0, &plate, &class, &params);
        assert!(result.terminal_fan > 0.0);
        assert_eq!(result.sediment_overfill, 0.0);
        let after = after_step(&mesh, &elevation, &sediment, &result, &params);
        let drop = params.alluvial_slope * mesh.mean_edge_angle() * params.planet_radius;
        for cell in 0..mesh.len() {
            // Former sea that the deltas built into land.
            if elevation[cell] < 0.0 && after[cell] > 0.0 {
                let lowest = mesh
                    .neighbors(cell as u32)
                    .iter()
                    .map(|&m| after[m as usize])
                    .fold(f64::INFINITY, f64::min);
                // Several cones may overlap; each adds at most one fall per cell.
                assert!(
                    after[cell] - lowest.max(0.0) <= 6.0 * drop + params.delta_top,
                    "{cell}: {} above {lowest}",
                    after[cell]
                );
            }
        }
        // Rock mass is conserved.
        let change: f64 = (0..mesh.len())
            .map(|c| {
                result.bedrock_delta[c] * 2800.0 + result.sediment_delta[c] * SEDIMENT_ROCK_DENSITY
            })
            .sum();
        let eroded: f64 = result.bedrock_delta.iter().map(|d| -d * 2800.0).sum();
        assert!(change.abs() < 1e-9 * eroded, "{change} {eroded}");
    }

    /// A cell on the island's flank and its lowest neighbour, its twin (#112 b).
    fn twins(mesh: &SphereMesh, elevation: &[f64]) -> (usize, usize) {
        let pit = mesh.nearest(glam::DVec3::new(0.15, 0.0, 1.0).normalize()) as usize;
        let twin = mesh
            .neighbors(pit as u32)
            .iter()
            .map(|&n| n as usize)
            .min_by(|&a, &b| elevation[a].total_cmp(&elevation[b]))
            .unwrap();
        (pit, twin)
    }

    #[test]
    fn cells_of_one_sample_end_at_their_mean_dry_level() {
        // One sample at a land cell and a submerged cell, one at a single cell.
        let elevation = [10.0, -20.0, 5.0];
        let means = sample_means(&elevation, &[0, 0, 1]);
        let dry = 0.5 * (isostasy::to_dry(10.0) + isostasy::to_dry(-20.0));
        assert_eq!(means[0], means[1]);
        assert!((means[0] - isostasy::to_surface(dry)).abs() < 1e-12);
        assert_eq!(means[2].to_bits(), 5.0f64.to_bits());
    }

    #[test]
    fn twins_at_one_level_are_no_pit() {
        // Twins show one crust sample and differ only by dynamic topography: the lower one is
        // a 6 m pit of its own. Seen at the sample's level (both at the mean), it is part of
        // the river network and its water reaches the ocean.
        let mesh = SphereMesh::icosphere(4);
        let mut elevation = island(&mesh);
        let (pit, twin) = twins(&mesh, &elevation);
        elevation[pit] = elevation[twin] - 6.0;
        let plate = vec![0; mesh.len()];
        let class = classes(&elevation, -1000.0);
        let per_cell = run(&mesh, &elevation, 1.0, &plate, &class);
        assert!(per_cell.water.depression_at_start[pit]);
        let mean = 0.5 * (elevation[pit] + elevation[twin]);
        elevation[pit] = mean;
        elevation[twin] = mean;
        let merged = run(&mesh, &elevation, 1.0, &plate, &class);
        assert!(!merged.water.depression_at_start.iter().any(|&d| d));
        let mut cell = pit;
        while let Some(next) = merged.water.receiver[cell] {
            cell = next as usize;
        }
        assert!(merged.water.ocean[cell]);
    }

    #[test]
    fn a_depression_of_twins_fills_evenly() {
        // A real depression of a sample shown at two cells: one lake, whose sediment raises both
        // cells by the same amount, so the sample's mean change is each cell's change.
        let mesh = SphereMesh::icosphere(4);
        let mut elevation = island(&mesh);
        let (pit, twin) = twins(&mesh, &elevation);
        let rim = [pit, twin]
            .iter()
            .flat_map(|&c| mesh.neighbors(c as u32).iter().map(|&n| n as usize))
            .filter(|&n| n != pit && n != twin)
            .map(|n| elevation[n])
            .fold(f64::INFINITY, f64::min);
        elevation[pit] = rim - 200.0;
        elevation[twin] = rim - 200.0;
        let result = run(
            &mesh,
            &elevation,
            1.0,
            &vec![0; mesh.len()],
            &classes(&elevation, -1000.0),
        );
        assert!(result.water.depression_at_start[pit] && result.water.depression_at_start[twin]);
        assert!(result.sediment_delta[pit] > 0.0);
        assert!((result.sediment_delta[pit] - result.sediment_delta[twin]).abs() < 1e-6);
    }

    /// One erosion step of 1 Myr on the island with uniform rain, temperature (°C) and
    /// sediment, with `params`.
    fn run_climate_regolith(
        mesh: &SphereMesh,
        temperature: f64,
        precipitation: f64,
        sediment: f64,
        params: &ErosionParams,
    ) -> (Vec<f64>, ErosionResult) {
        let elevation = island(mesh);
        let n = mesh.len();
        let sediment = vec![sediment; n];
        let class = classes(&elevation, 0.0);
        let result = erode(
            mesh,
            &Surface {
                elevation: &elevation,
                sediment: &sediment,
                bedrock_density: &vec![2800.0; n],
                bedrock_isostatic_density: &vec![2800.0; n],
                dense_crust: &vec![0.0; n],
                precipitation: &vec![precipitation; n],
                runoff: &vec![precipitation; n],
                potential_evaporation: &vec![0.0; n],
                plate: &vec![0; n],
                class: &class,
                ocean_volume: ocean_volume(mesh, &elevation, &sediment, params.planet_radius),
                water_volume: ocean_volume(mesh, &elevation, &sediment, params.planet_radius),
                ocean_seeds: &sea::ocean_seeds(
                    mesh,
                    &elevation,
                    -sea::SILL_TOLERANCE,
                    sea::MIN_SEA_FRACTION,
                ),
                previous_lakes: &[],
                sample: &[],
                temperature: &vec![temperature; n],
                ice: &[],
                mafic: &[],
                regolith: &[],
            },
            params,
            1e6,
        );
        (sediment, result)
    }

    fn rock_mass_change(result: &ErosionResult) -> (f64, f64) {
        let change: f64 = (0..result.bedrock_delta.len())
            .map(|c| {
                result.bedrock_delta[c] * 2800.0 + result.sediment_delta[c] * SEDIMENT_ROCK_DENSITY
            })
            .sum();
        let eroded: f64 = result.bedrock_delta.iter().map(|d| -d * 2800.0).sum();
        (change, eroded)
    }

    #[test]
    fn regolith_conserves_rock_mass() {
        let mesh = SphereMesh::icosphere(4);
        let params = ErosionParams {
            waves: false,
            ..ErosionParams::default()
        };
        for (temperature, rain, sediment) in
            [(20.0, 1.0, 0.0), (-20.0, 0.05, 0.0), (10.0, 0.3, 1.0)]
        {
            let (start, result) = run_climate_regolith(&mesh, temperature, rain, sediment, &params);
            let (change, eroded) = rock_mass_change(&result);
            assert!(eroded > 0.0);
            assert!(
                change.abs() < 1e-9 * eroded,
                "change {change}, eroded {eroded}"
            );
            // The regolith is part of the sediment at the end.
            for c in 0..mesh.len() {
                assert!(result.regolith[c] <= start[c] + result.sediment_delta[c] + 1e-9);
                assert!(result.regolith[c] >= 0.0 && result.bare[c] >= 0.0);
                assert!(result.bare[c] <= 1.0 + 1e-12);
            }
            assert!(result.regolith.iter().any(|&r| r > 0.0));
            assert!(result.regolith_stats.land_cells > 0);
        }
    }

    #[test]
    fn cold_dry_rock_erodes_slower_than_without_regolith() {
        let mesh = SphereMesh::icosphere(4);
        let on = ErosionParams {
            waves: false,
            ..ErosionParams::default()
        };
        let mut off = on.clone();
        off.regolith.enabled = false;
        let eroded = |temperature: f64, rain: f64, params: &ErosionParams| {
            let (_, result) = run_climate_regolith(&mesh, temperature, rain, 0.0, params);
            rock_mass_change(&result).1
        };
        // Same rain (same transport), cold: the rock weathers slowly and erosion is
        // weathering-limited.
        let cold = eroded(-15.0, 0.3, &on);
        let warm = eroded(25.0, 0.3, &on);
        let old = eroded(-15.0, 0.3, &off);
        assert!(cold < warm, "{cold} vs {warm}");
        assert!(cold < 0.8 * old, "{cold} vs {old}");
        // Without regolith the climate does not matter (beyond the runoff).
        assert_eq!(old, eroded(25.0, 0.3, &off));
        assert!(warm <= old * (1.0 + 1e-9), "{warm} vs {old}");
    }

    #[test]
    fn regolith_off_keeps_the_old_model() {
        let mesh = SphereMesh::icosphere(4);
        let mut params = ErosionParams::default();
        params.regolith.enabled = false;
        let (_, result) = run_climate_regolith(&mesh, 10.0, 0.5, 2.0, &params);
        let elevation = island(&mesh);
        let old = run_with(
            &mesh,
            &elevation,
            &vec![2.0; mesh.len()],
            0.5,
            &vec![0; mesh.len()],
            &classes(&elevation, 0.0),
            &params,
        );
        assert_eq!(result.bedrock_delta, old.bedrock_delta);
        assert_eq!(result.sediment_delta, old.sediment_delta);
        assert!(result.regolith.iter().all(|&r| r == 0.0));
        assert!(result.bare.iter().all(|&b| b == 0.0));
    }
}
