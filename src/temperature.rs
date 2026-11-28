//! Surface temperature from an energy balance model with a seasonal cycle.
//!
//! The planet orbits its sun on an Earth-like eccentric orbit with an inclined rotation axis.
//! For each part of the year, the daily mean insolation at every latitude follows from the
//! solar declination and the distance to the sun. The temperature `T` of every column then
//! evolves as
//!
//! ```text
//! C dT/dt = Q (1 - α) - (A + B T) + D ∇²T
//! ```
//!
//! with absorbed sunlight `Q (1 - α)`, outgoing longwave radiation `A + B T` (Budyko, 1969),
//! poleward heat transport by diffusion `D ∇²T` (Sellers, 1969; North, 1975), and the heat
//! capacity `C`. Most of the transport acts on the means of latitude bands (the one-dimensional
//! model of North, 1975), and only a weak part acts as two-dimensional diffusion between
//! neighbouring cells; applying the full transport two-dimensionally would tie the temperature of
//! land to the surrounding ocean over thousands of kilometres within a season.
//!
//! Every cell has a **land surface** and an **ocean column**, coupled through the air above them.
//! Land has a small heat capacity (the air column, about 1e7 J/m²/K), so continental interiors get
//! hot summers and cold winters; ice sheets form on land below an annual mean of −10 °C if it
//! snows enough (Budyko's criterion, scaled by snowfall).
//!
//! The ocean column follows Wagner & Eisenman (2015): its enthalpy is either the heat of the
//! mixed layer above the freezing point (open water) or the latent heat of sea ice. Ice insulates
//! the ocean, so its surface cools far below freezing in winter, from the balance of conduction
//! through the ice and the surface fluxes; in summer the surface stays at the melting point while
//! the ice melts. Sea ice is bright, melting ice less so, and the open ocean of high latitudes is
//! already bright from clouds and low sun (the planetary albedo is `a + b P₂(sin φ)`), which masks
//! much of the contrast to sea ice.
//!
//! **Ocean currents** carry heat in the ocean columns: the shallow overturning cells of the
//! subtropics (poleward Ekman transport under the trade winds, returning in the thermocline) as a
//! zonal transport, and the wind-driven gyres with their western boundary currents (see
//! [`crate::ocean`]).
//!
//! Over cold surfaces, the air that radiates to space lies above a surface inversion and stays
//! warmer than the surface. The linear law `A + B T`, fitted to Earth's mid and low latitudes,
//! underestimates the outgoing radiation there (by about 10 W/m² over the Arctic Ocean, 20 W/m²
//! over Antarctica). Below the inversion temperature `T_i`, the emitting air can be taken to be
//! warmer than the surface by `k (T_i − T)`. This is off by default (`k = 0`): with k ≈ 1/3, the
//! weaker radiative damping of cold climates moves the solar constant at which the planet falls
//! into a snowball from about 91% to about 94% of today's.
//!
//! The model evolves sea-level temperatures on a coarse icosphere, since temperature varies
//! smoothly. Surface temperatures on the fine mesh are interpolated from the land or the ocean
//! surface, depending on the fine cell, and lowered with elevation according to the lapse rate.

use crate::math;
use std::f64::consts::{PI, TAU};

use crate::sphere::SphereMesh;

/// Orbit and rotation axis of the planet. Defaults are Earth's.
#[derive(Clone, Debug)]
pub struct OrbitParams {
    /// Solar irradiance at the mean distance in W/m².
    pub solar_constant: f64,
    pub eccentricity: f64,
    /// Axial tilt in degrees.
    pub obliquity: f64,
    /// Solar longitude (angle from the March equinox) at perihelion in degrees.
    pub perihelion_longitude: f64,
}

impl Default for OrbitParams {
    fn default() -> Self {
        Self {
            solar_constant: 1361.0,
            eccentricity: 0.0167,
            obliquity: 23.44,
            perihelion_longitude: 282.94,
        }
    }
}

/// The position of the planet on its orbit at some time of the year.
#[derive(Clone, Copy, Debug)]
pub struct OrbitalPosition {
    /// Solar declination in radians: the latitude where the sun is at the zenith at noon.
    pub declination: f64,
    /// Distance to the sun relative to the semi-major axis.
    pub distance: f64,
}

impl OrbitParams {
    /// The orbital position at `fraction` (0..1) of the year after perihelion.
    pub fn position(&self, fraction: f64) -> OrbitalPosition {
        let e = self.eccentricity;
        let mean_anomaly = TAU * fraction;
        // Solve Kepler's equation `M = E - e sin E` with Newton's method.
        let mut eccentric_anomaly = mean_anomaly;
        for _ in 0..8 {
            eccentric_anomaly -=
                (eccentric_anomaly - e * math::sin(eccentric_anomaly) - mean_anomaly)
                    / (1.0 - e * math::cos(eccentric_anomaly));
        }
        let true_anomaly = 2.0
            * math::atan2(
                (1.0 + e).sqrt() * math::sin(eccentric_anomaly / 2.0),
                (1.0 - e).sqrt() * math::cos(eccentric_anomaly / 2.0),
            );
        let solar_longitude = true_anomaly + self.perihelion_longitude.to_radians();
        OrbitalPosition {
            declination: math::asin(
                math::sin(self.obliquity.to_radians()) * math::sin(solar_longitude),
            ),
            distance: 1.0 - e * math::cos(eccentric_anomaly),
        }
    }

    /// Daily mean insolation at the top of the atmosphere in W/m² at `latitude` (radians).
    pub fn daily_insolation(&self, latitude: f64, position: OrbitalPosition) -> f64 {
        let declination = position.declination;
        // Hour angle of sunset; 0 during polar night, π during polar day.
        let cos_sunset = -math::tan(latitude) * math::tan(declination);
        let sunset = if cos_sunset >= 1.0 {
            0.0
        } else if cos_sunset <= -1.0 {
            PI
        } else {
            math::acos(cos_sunset)
        };
        self.solar_constant / PI / position.distance.powi(2)
            * (sunset * math::sin(latitude) * math::sin(declination)
                + math::cos(latitude) * math::cos(declination) * math::sin(sunset))
    }
}

/// Parameters of the energy balance model. Temperatures in °C.
#[derive(Clone, Debug)]
pub struct TemperatureParams {
    /// The orbit; its solar constant is the one at the start of the run (time 0), which the sun's
    /// brightening raises during the run.
    pub orbit: OrbitParams,
    /// The epoch of the run's start (`--epoch`, see [`crate::presets::Epoch`]), from which
    /// [`crate::sun::age_of_run`] gives the age of the sun; today ([`crate::sun::PRESENT_EPOCH`])
    /// in the struct default; a run starts at epoch 0 unless `--epoch` says otherwise (#190).
    pub epoch: f64,
    /// Scales the ageing of the sun during the run (the simulated time it sees): 1 follows the
    /// sun model ([`crate::sun::brightening`], ~1% per 100 Myr), 0 keeps the solar constant of
    /// the start. The view always shows the unscaled sun model.
    pub solar_brightening: f64,
    /// Outgoing longwave radiation at 0 °C in W/m².
    pub radiation_a: f64,
    /// Increase of outgoing longwave radiation per kelvin in W/m²/K.
    pub radiation_b: f64,
    /// Radiative forcing of greenhouse gases relative to today's (pre-industrial) atmosphere, in
    /// W/m²: it lowers the outgoing longwave radiation to `A − F + B T`. 0 is today's
    /// atmosphere, about 3.7 W/m² a doubling of CO₂ (Myhre et al., 1998); see
    /// [`crate::presets`] for the forcing of a young planet under a faint Sun.
    pub greenhouse_forcing: f64,
    /// Below this surface temperature (°C), a surface inversion forms: the air that emits to
    /// space stays warmer than the surface.
    pub inversion_temperature: f64,
    /// Strength of the surface inversion: kelvin of inversion per kelvin of surface cooling below
    /// [`Self::inversion_temperature`]. About 1/3 fits Earth's polar outgoing radiation; 0 (the
    /// default) disables it, see the module documentation.
    pub inversion_strength: f64,
    /// Coefficient of meridional heat transport between latitude bands, for the Laplacian on the
    /// unit sphere, in W/m²/K.
    pub heat_transport: f64,
    /// Coefficient of local two-dimensional heat diffusion, in W/m²/K.
    pub local_heat_transport: f64,
    /// Number of equal-area latitude bands for the meridional heat transport.
    pub latitude_bands: usize,
    /// Planetary albedo (including clouds) over ice-free ocean: `a + b P₂(sin φ)`, the mean `a`
    /// and the latitude dependence `b`. Clouds and the low sun make the open ocean of high
    /// latitudes much brighter than the tropical ocean (Earth's ice-free ocean: about 0.22 in the
    /// tropics, 0.4–0.45 at 60°), which also masks the contrast to sea ice.
    pub ocean_albedo: f64,
    pub ocean_albedo_latitude: f64,
    /// Planetary albedo over cold sea ice (with the clouds above it; Earth's ice-covered polar
    /// ocean: about 0.5–0.6 at the top of the atmosphere).
    pub sea_ice_albedo: f64,
    /// Planetary albedo over melting sea ice (wet surface, melt ponds).
    pub melting_ice_albedo: f64,
    /// Sea ice thicker than this (m) is exported by drift and melts elsewhere.
    pub max_ice_thickness: f64,
    /// Wind-driven ocean heat transport by the shallow subtropical overturning cells: poleward
    /// Ekman transport under the trade winds, returning equatorward in the thermocline at the
    /// temperature of the water subducted at this latitude (degrees) in late winter, when the
    /// mixed layer is deepest.
    pub subduction_latitude: f64,
    /// Friction (1/s) that limits the Ekman transport near the equator, where f vanishes.
    pub equatorial_friction: f64,
    /// Bottom friction of the wind-driven gyres (1/s); sets the width of the western boundary
    /// currents (`r / β`), which the coarse mesh resolves only at ~500 km (see [`crate::ocean`]).
    pub gyre_friction: f64,
    /// Share of the gyres' volume transport that carries the temperature of the mixed layer.
    pub gyre_heat_share: f64,
    /// Planetary albedo (including clouds) over snow-free land.
    pub land_albedo: f64,
    /// Planetary albedo over ice sheets.
    pub ice_albedo: f64,
    /// Freezing temperature of sea water in °C.
    pub freezing_temperature: f64,
    /// Latent heat of fusion of sea ice per volume in J/m³ (917 kg/m³ × 3.34e5 J/kg).
    pub ice_latent_heat: f64,
    /// Thermal conductivity of sea ice in W/m/K.
    pub ice_conductivity: f64,
    /// Sea ice reaches the full ice albedo at this thickness in m (thinner ice is partly open
    /// water and bare).
    pub ice_albedo_thickness: f64,
    /// Heat exchange between the land and the ocean surface of the same coarse cell through the
    /// air, in W/m²/K.
    pub land_ocean_exchange: f64,
    /// Ice sheets form below this annual mean land surface temperature; the cover changes
    /// linearly over ±5 K around it.
    pub land_ice_temperature: f64,
    /// Ice sheets need snowfall: below this annual precipitation in m/yr, the ice cover is reduced
    /// proportionally, so cold deserts (like Siberia) stay free of ice sheets.
    pub ice_sheet_precipitation: f64,
    /// Heat capacity of the ocean mixed layer in J/m²/K.
    pub ocean_heat_capacity: f64,
    /// Effective heat capacity of land in J/m²/K: the air column (p/g c_p ≈ 1.03e7). The soil
    /// within the seasonal skin depth would add ≈ 0.3e7, but the coarse model already
    /// underestimates continental summers, and with 1.3e7 or more the ice-albedo feedback ran away
    /// into a snowball for some continental configurations.
    pub land_heat_capacity: f64,
    /// Temperature decrease with elevation in K/m.
    pub lapse_rate: f64,
    /// Elevation (m) from which low-latitude land is at least as warm as the free troposphere
    /// at its height. The energy balance gives every coarse cell's land a temperature of its
    /// own, with little exchange between cells, so that continents keep their seasons. For
    /// high ground in low latitudes that is wrong in one direction: it stands above the
    /// boundary layer (1–3 km deep) in air that the tropical circulation keeps nearly uniform
    /// along a latitude circle (weak temperature gradients where the Coriolis force is small;
    /// Sobel et al. 2001), and the sea surface sets the temperature of that air by convection.
    /// A plateau cannot stay tens of K colder than the air around it, however bright its own
    /// ice. So between [`Self::free_air_base`] and this elevation, the annual mean of land that
    /// is colder than the sea surface of its latitude band is raised linearly towards it (the
    /// seasonal cycle stays the local one). Land that is warmer keeps its own temperature: plateaus are
    /// elevated heat sources. Zero turns it off.
    pub free_air_height: f64,
    /// Elevation (m) below which land keeps the temperature of its own energy balance: it lies
    /// inside the boundary layer, which the surface below it controls.
    pub free_air_base: f64,
    /// Latitude (degrees) up to which [`Self::free_air_height`] applies in full; it fades out
    /// over the next 15°. Beyond the tropics and subtropics the free troposphere is not uniform,
    /// and a large cold plateau makes its own climate under a surface inversion (the Antarctic
    /// plateau is ~30 K colder than the free air at its height over the ocean around it).
    pub free_air_latitude: f64,
    /// Time steps per year; must be a multiple of 12.
    pub seasons_per_year: usize,
    /// Substeps per season.
    pub substeps_per_season: usize,
    /// Years simulated when the model is first run.
    pub spin_up_years: usize,
    /// Years simulated per update afterwards, starting from the previous state.
    pub years_per_update: usize,
    /// Subdivisions of the coarse icosphere the model runs on.
    pub coarse_subdivisions: u32,
    /// Whether lakes are open water (#173): a lake column per coarse cell beside the land and
    /// the ocean column, with the heat capacity of the lake's mixed layer, lake ice and the
    /// open-water albedo, but no ocean heat transport (see [`TemperatureModel::update`]).
    /// Otherwise lake cells are land (before #173).
    pub open_water_lakes: bool,
    /// Cloud nuclei and their effect on the brightness of clouds (#204), which correct the
    /// albedos calibrated on Earth for a planet with other sources ([`crate::aerosol`]).
    pub aerosol: crate::aerosol::AerosolParams,
}

impl Default for TemperatureParams {
    fn default() -> Self {
        Self {
            orbit: OrbitParams::default(),
            epoch: crate::sun::PRESENT_EPOCH,
            solar_brightening: 1.0,
            radiation_a: 203.3,
            radiation_b: 2.09,
            greenhouse_forcing: 0.0,
            inversion_temperature: -5.0,
            inversion_strength: 0.0,
            heat_transport: 0.45,
            local_heat_transport: 0.05,
            latitude_bands: 18,
            ocean_albedo: 0.31,
            ocean_albedo_latitude: 0.18,
            sea_ice_albedo: 0.55,
            melting_ice_albedo: 0.45,
            max_ice_thickness: 10.0,
            subduction_latitude: 30.0,
            equatorial_friction: 1.0 / (2.0 * 86_400.0),
            gyre_friction: 1e-5,
            gyre_heat_share: 0.5,
            land_albedo: 0.32,
            ice_albedo: 0.62,
            freezing_temperature: -1.8,
            ice_latent_heat: 3.06e8,
            ice_conductivity: 2.0,
            ice_albedo_thickness: 0.5,
            land_ocean_exchange: 10.0,
            land_ice_temperature: -10.0,
            ice_sheet_precipitation: 0.3,
            ocean_heat_capacity: 4.18e6 * 50.0,
            land_heat_capacity: 1e7,
            lapse_rate: 0.0065,
            free_air_height: 3000.0,
            free_air_base: 1000.0,
            free_air_latitude: 30.0,
            seasons_per_year: 24,
            substeps_per_season: 8,
            spin_up_years: 20,
            years_per_update: 2,
            coarse_subdivisions: 4,
            open_water_lakes: true,
            aerosol: crate::aerosol::AerosolParams::default(),
        }
    }
}

impl TemperatureParams {
    /// The solar constant (W/m²) `time` Myr into the run: the one of the start, raised by the
    /// sun's brightening since then (scaled by [`Self::solar_brightening`]). Exactly the start's
    /// at time 0.
    pub fn solar_constant(&self, time: f64) -> f64 {
        let start = crate::sun::age_of_run(self.epoch, 0.0);
        let now = crate::sun::age_of_run(self.epoch, self.solar_brightening * time);
        self.orbit.solar_constant * crate::sun::brightening(start, now)
    }

    /// Sets a parameter by its field name, for calibration from the command line.
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        if name == "open_water_lakes" {
            self.open_water_lakes = value != 0.0;
            return Ok(());
        }
        if let Some(name) = name.strip_prefix("aerosol.") {
            return self.aerosol.set(name, value);
        }
        let field = match name {
            "radiation_a" => &mut self.radiation_a,
            "radiation_b" => &mut self.radiation_b,
            "greenhouse_forcing" => &mut self.greenhouse_forcing,
            "inversion_temperature" => &mut self.inversion_temperature,
            "inversion_strength" => &mut self.inversion_strength,
            "heat_transport" => &mut self.heat_transport,
            "local_heat_transport" => &mut self.local_heat_transport,
            "ocean_albedo" => &mut self.ocean_albedo,
            "ocean_albedo_latitude" => &mut self.ocean_albedo_latitude,
            "sea_ice_albedo" => &mut self.sea_ice_albedo,
            "melting_ice_albedo" => &mut self.melting_ice_albedo,
            "max_ice_thickness" => &mut self.max_ice_thickness,
            "subduction_latitude" => &mut self.subduction_latitude,
            "equatorial_friction" => &mut self.equatorial_friction,
            "gyre_friction" => &mut self.gyre_friction,
            "gyre_heat_share" => &mut self.gyre_heat_share,
            "land_albedo" => &mut self.land_albedo,
            "ice_albedo" => &mut self.ice_albedo,
            "freezing_temperature" => &mut self.freezing_temperature,
            "ice_latent_heat" => &mut self.ice_latent_heat,
            "ice_conductivity" => &mut self.ice_conductivity,
            "ice_albedo_thickness" => &mut self.ice_albedo_thickness,
            "land_ocean_exchange" => &mut self.land_ocean_exchange,
            "land_ice_temperature" => &mut self.land_ice_temperature,
            "ice_sheet_precipitation" => &mut self.ice_sheet_precipitation,
            "ocean_heat_capacity" => &mut self.ocean_heat_capacity,
            "land_heat_capacity" => &mut self.land_heat_capacity,
            "lapse_rate" => &mut self.lapse_rate,
            "free_air_height" => &mut self.free_air_height,
            "free_air_base" => &mut self.free_air_base,
            "free_air_latitude" => &mut self.free_air_latitude,
            "solar_constant" => &mut self.orbit.solar_constant,
            "solar_brightening" => &mut self.solar_brightening,
            _ => anyhow::bail!("unknown temperature parameter {name}"),
        };
        *field = value;
        Ok(())
    }
}

const SECONDS_PER_YEAR: f64 = 365.25 * 24.0 * 3600.0;
/// Number of longitude sectors of the thermal equator.
pub const THERMAL_EQUATOR_SECTORS: usize = 24;

/// Latitude bins of 0.5° between 60°S and 60°N for the thermal equator.
const EQUATOR_BINS: usize = 240;
/// The model assumes an Earth-sized planet for the ocean heat transport.
const EARTH_RADIUS: f64 = 6.371e6;

/// Annual mean land temperature (°C, at sea level) assumed before the first year of the
/// spin-up, which decides the ice sheets of that year: warm enough that there are none.
const ICE_FREE_START: f64 = 40.0;
/// Angular velocity of the rotation in rad/s.
const EARTH_ROTATION: f64 = 7.292e-5;
/// Specific heat capacity of sea water in J/kg/K.
const WATER_HEAT_CAPACITY: f64 = 3990.0;
/// Heat capacity of water per volume in J/m³/K, which turns a mixed-layer depth into a heat
/// capacity per area (the default [`TemperatureParams::ocean_heat_capacity`] is 50 m of it).
const WATER_VOLUMETRIC_HEAT_CAPACITY: f64 = 4.18e6;
/// Freezing and melting point of the fresh water of lakes in °C.
const LAKE_FREEZING_TEMPERATURE: f64 = 0.0;
/// Cells with at least this much lake water (m) are lakes, as everywhere else (the view, the
/// climate classes).
pub const LAKE_DEPTH: f64 = crate::render::LAKE_DEPTH;

/// Temperature fields on the fine mesh, in °C.
#[derive(Clone, Debug, Default)]
pub struct TemperatureFields {
    /// Annual mean surface temperature (at the land surface or the sea surface).
    pub annual_mean: Vec<f64>,
    /// Mean surface temperature of each month; month 0 starts at perihelion (early January).
    pub monthly: Vec<Vec<f64>>,
    /// Latitude (degrees) of the maximum of the sea-level temperature in each month (the thermal
    /// equator), for [`THERMAL_EQUATOR_SECTORS`] sectors of longitude from 180°W eastwards: over a
    /// summer continent it lies further poleward than over the ocean (monsoons).
    pub thermal_equator: Vec<Vec<f64>>,
    /// Permanent ice cover (sea ice, lake ice, ice sheets), 0 to 1, as the albedo of the last
    /// year saw it.
    pub ice: Vec<f64>,
    /// Mean ice cover (0 to 1) of every month on the lake cells, NaN elsewhere; empty unless
    /// lakes are open water ([`TemperatureParams::open_water_lakes`]).
    pub monthly_lake_ice: Vec<Vec<f64>>,
    /// Annual mean northward heat transport across the band interfaces in the last year: latitude
    /// (degrees), and the transport by the atmosphere and by the ocean in W per square metre of
    /// the planet's radius squared (multiply by R² for watts).
    pub meridional_transport: Vec<(f64, f64, f64)>,
    /// Largest volume transport of the wind-driven gyres in Sv (1e6 m³/s).
    pub gyre_transport: f64,
    /// The solar constant (W/m²) of the last update, raised by the sun's brightening.
    pub solar_constant: f64,
    /// Global mean net radiation at the top of the atmosphere (absorbed sunlight minus outgoing
    /// longwave radiation, W/m²) over the last simulated year of the update: positive while the
    /// climate still warms towards its equilibrium (the mixed layer takes a few years, an
    /// update simulates [`TemperatureParams::years_per_update`]). The carbon cycle uses it for
    /// the warming still to come (#154).
    pub toa_imbalance: f64,
    /// Global planetary (Bond) albedo over the last simulated year of the update: reflected over
    /// incoming sunlight.
    pub planetary_albedo: f64,
    /// The cloud nuclei of the update and how they changed the albedos (#204).
    pub aerosol: crate::aerosol::AerosolDiagnostics,
    /// Depth-integrated volume transport of the wind-driven gyres (eastward, northward) in m²/s
    /// at every fine ocean cell: the value of its coarse cell, NaN where that is not ocean in the
    /// gyre model (mostly land) or the fine cell is land.
    pub ocean_current: Vec<[f64; 2]>,
    /// Streamfunction of the gyres in Sv at every fine ocean cell (interpolated, zero at the
    /// coasts of the coarse mesh), NaN on land.
    pub gyre_streamfunction: Vec<f64>,
    /// Annual mean heat the gyres deliver to the mixed layer (W/m² of ocean; negative where they
    /// take heat away) at every fine ocean cell, from its coarse cell as `ocean_current`.
    pub gyre_heating: Vec<f64>,
}

impl TemperatureFields {
    /// Difference between the warmest and the coldest month.
    pub fn seasonal_range(&self, cell: usize) -> f64 {
        let (min, max) = self
            .monthly
            .iter()
            .map(|month| month[cell])
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), t| {
                (min.min(t), max.max(t))
            });
        max - min
    }
}

#[derive(Clone)]
pub struct TemperatureModel {
    params: TemperatureParams,
    coarse: SphereMesh,
    coarse_latitude: Vec<f64>,
    /// What [`TemperatureModel::thermal_equator`] needs of the coarse mesh, which does not
    /// change: per coarse cell its latitude bin (`u32::MAX` outside 60°S–60°N), per sector of
    /// longitude the weight of every coarse cell, and the smoothing kernel over the bins.
    equator_bins: Vec<u32>,
    equator_weights: Vec<Vec<f64>>,
    equator_kernel: Vec<f64>,
    /// The latitude band of every coarse cell.
    band: Vec<usize>,
    /// The coarse cell each fine cell belongs to.
    fine_to_coarse: Vec<u32>,
    /// Interpolation weights from coarse to fine cells.
    interpolation: Vec<[(u32, f64); 7]>,
    /// Sea-level temperature of the land surface of each coarse cell (also where the cell has no
    /// land: the temperature land there would have).
    land: Vec<f64>,
    /// Enthalpy of the ocean column of each coarse cell in J/m², relative to water at the
    /// freezing point: positive for open water (heat in the mixed layer), negative for sea ice
    /// (latent heat of the ice).
    ocean: Vec<f64>,
    /// Surface temperature of the ocean (water or ice) of each coarse cell.
    ocean_surface: Vec<f64>,
    /// Enthalpy of the lake column of each coarse cell in J/m², as [`Self::ocean`] relative to
    /// water at the lake's freezing point (see [`TemperatureParams::open_water_lakes`]); also
    /// where the cell has no lake: the state a lake there would have.
    lake: Vec<f64>,
    /// Surface temperature of the lakes (water or ice) of each coarse cell, at sea level (see
    /// [`TemperatureModel::update`]).
    lake_surface: Vec<f64>,
    /// Heat capacity (J/m²/K) and freezing point (°C at sea level) of the lake column of each
    /// coarse cell in the last update: when the lakes change, the column keeps its temperature,
    /// not its heat.
    lake_column: Vec<(f64, f64)>,
    /// Annual mean land temperature of each coarse cell in the previous year, which determines
    /// the ice sheets.
    previous_annual_land: Vec<f64>,
    /// How much warmer the free troposphere around each coarse cell was than its land surface
    /// in the previous year (annual means at sea level; see
    /// [`TemperatureParams::free_air_height`]).
    previous_free_air_excess: Vec<f64>,
    /// Heat delivered to the ocean mixed layer by ocean currents, per coarse cell, in W/m² of
    /// ocean.
    ocean_heat_convergence: Vec<f64>,
    gyre_model: crate::ocean::GyreModel,
    /// Temperature of the water subducted into the thermocline in the south and the north: the
    /// sea surface at the subduction latitude in late winter, when the mixed layer is deepest,
    /// in the previous year.
    subduction_temperature: [f64; 2],
    spun_up: bool,
    /// The volcanic and dust sources of cloud nuclei the next update uses (#204).
    aerosol_sources: crate::aerosol::Sources,
    fields: TemperatureFields,
}

impl TemperatureModel {
    pub fn new(fine: &SphereMesh, params: TemperatureParams) -> Self {
        assert!(
            params.seasons_per_year.is_multiple_of(12) && params.seasons_per_year > 0,
            "seasons per year must be a positive multiple of 12"
        );
        let coarse = SphereMesh::icosphere(params.coarse_subdivisions.min(fine.subdivisions()));
        let coarse_latitude: Vec<f64> = (0..coarse.len() as u32)
            .map(|cell| math::asin(coarse.vertex(cell).z.clamp(-1.0, 1.0)))
            .collect();
        assert!(
            params.latitude_bands >= 2,
            "at least two latitude bands are needed"
        );
        // Bands of equal width in sin(latitude) have equal area.
        let band = coarse_latitude
            .iter()
            .map(|latitude| {
                let x = (math::sin(*latitude) + 1.0) / 2.0;
                ((x * params.latitude_bands as f64) as usize).min(params.latitude_bands - 1)
            })
            .collect();

        let mut fine_to_coarse = Vec::with_capacity(fine.len());
        let mut interpolation = Vec::with_capacity(fine.len());
        let mut hint = 0;
        let radius = 1.5 * coarse.mean_edge_angle();
        for cell in 0..fine.len() as u32 {
            let position = fine.vertex(cell);
            let nearest = coarse.nearest_from(position, hint);
            hint = nearest;
            fine_to_coarse.push(nearest);

            let mut weights = [(nearest, 0.0); 7];
            let candidates =
                std::iter::once(nearest).chain(coarse.neighbors(nearest).iter().copied());
            for (slot, candidate) in weights.iter_mut().zip(candidates) {
                let distance = math::angle_between(coarse.vertex(candidate), position);
                *slot = (candidate, (1.0 - distance / radius).max(0.0).powi(2));
            }
            let total: f64 = weights.iter().map(|(_, w)| w).sum();
            for (_, weight) in &mut weights {
                *weight /= total;
            }
            interpolation.push(weights);
        }

        // Precomputed for the thermal equator (identical to computing them for every month).
        let equator_bins = coarse_latitude
            .iter()
            .map(|latitude| {
                let position = (latitude.to_degrees() + 60.0) * 2.0;
                if (0.0..EQUATOR_BINS as f64).contains(&position) {
                    position as u32
                } else {
                    u32::MAX
                }
            })
            .collect();
        let equator_weights = (0..THERMAL_EQUATOR_SECTORS)
            .map(|sector| {
                let longitude = (sector as f64 + 0.5) / THERMAL_EQUATOR_SECTORS as f64 * TAU - PI;
                (0..coarse.len() as u32)
                    .map(|cell| {
                        let p = coarse.vertex(cell);
                        let difference =
                            (math::atan2(p.y, p.x) - longitude + PI).rem_euclid(TAU) - PI;
                        math::exp(-(difference / 30f64.to_radians()).powi(2))
                    })
                    .collect()
            })
            .collect();
        let equator_kernel = (-30i32..=30)
            .map(|offset| {
                let distance = offset as f64 / 10.0;
                math::exp(-0.5 * distance * distance)
            })
            .collect();

        let count = coarse.len();
        let gyre_model = crate::ocean::GyreModel::new(&coarse, EARTH_RADIUS);
        let ocean_enthalpy = params.ocean_heat_capacity * (15.0 - params.freezing_temperature);
        let lake_enthalpy = params.ocean_heat_capacity * (15.0 - LAKE_FREEZING_TEMPERATURE);
        let lake_column = vec![(params.ocean_heat_capacity, LAKE_FREEZING_TEMPERATURE); count];
        Self {
            params,
            coarse,
            coarse_latitude,
            equator_bins,
            equator_weights,
            equator_kernel,
            band,
            fine_to_coarse,
            interpolation,
            land: vec![15.0; count],
            ocean: vec![ocean_enthalpy; count],
            ocean_surface: vec![15.0; count],
            lake: vec![lake_enthalpy; count],
            lake_surface: vec![15.0; count],
            lake_column,
            // The spin-up starts without ice sheets: they form where the land turns out cold
            // enough. Starting from a cold land would put ice on every plateau in the first
            // year, and ice sheets that keep themselves cold would then stay.
            previous_annual_land: vec![ICE_FREE_START; count],
            previous_free_air_excess: vec![0.0; count],
            ocean_heat_convergence: vec![0.0; count],
            gyre_model,
            subduction_temperature: [15.0; 2],
            spun_up: false,
            aerosol_sources: crate::aerosol::Sources::default(),
            fields: TemperatureFields::default(),
        }
    }

    pub fn fields(&self) -> &TemperatureFields {
        &self.fields
    }

    pub fn params(&self) -> &TemperatureParams {
        &self.params
    }

    pub fn params_mut(&mut self) -> &mut TemperatureParams {
        &mut self.params
    }

    /// The volcanic and dust sources of cloud nuclei of the next update (#204); Earth's until
    /// they are set.
    pub fn aerosol_sources(&self) -> crate::aerosol::Sources {
        self.aerosol_sources
    }

    pub fn set_aerosol_sources(&mut self, sources: crate::aerosol::Sources) {
        self.aerosol_sources = sources;
    }

    /// Simulates the seasonal cycle for the given surface elevation (in metres, sea level is 0)
    /// of the fine mesh, `time` Myr into the run (for the sun's brightening), and updates the
    /// temperature fields. `precipitation` (m/yr) limits where ice sheets can grow; without it,
    /// 1 m/yr is assumed everywhere.
    ///
    /// `lake_depth` (m, per fine cell) marks the lakes: cells with water on them that are not
    /// ocean (elevation ≥ 0, which for a lake is its water surface). With
    /// [`TemperatureParams::open_water_lakes`], they are open water: each coarse cell has a lake
    /// column beside its land and ocean, with the physics of the ocean column (mixed layer or
    /// ice, the same albedos) but the heat capacity of the lakes' mixed layer (their mean depth,
    /// at most the ocean's mixed layer), fresh water freezing at 0 °C, and no ocean heat
    /// transport. Like the land, the column works with sea-level temperatures, so it freezes at
    /// the lapse rate times the mean elevation of its lakes' water surfaces. The surfaces of a
    /// coarse cell exchange heat through the air in proportion to their area shares, and the
    /// lakes' share enters the air above the cell, so a large lake gives the land around it a
    /// maritime climate.
    pub fn update(
        &mut self,
        elevation: &[f64],
        precipitation: Option<&[f64]>,
        lake_depth: Option<&[f64]>,
        time: f64,
    ) {
        let params = &self.params;
        let orbit = OrbitParams {
            solar_constant: params.solar_constant(time),
            ..params.orbit.clone()
        };
        let coarse_count = self.coarse.len();

        let is_lake: Vec<bool> = match lake_depth {
            Some(depth) if params.open_water_lakes => (0..elevation.len())
                .map(|fine| depth[fine] >= LAKE_DEPTH && elevation[fine] >= 0.0)
                .collect(),
            _ => vec![false; elevation.len()],
        };
        // Land and lake fractions of every coarse cell; the heat capacity and the freezing point
        // (at sea level) of its lake column.
        let mixed_layer_depth = params.ocean_heat_capacity / WATER_VOLUMETRIC_HEAT_CAPACITY;
        // A shallow lake stores at least the heat of the land around it (the air column, ~2.4 m
        // of water): the wet ground below it takes part too.
        let min_lake_depth =
            (params.land_heat_capacity / WATER_VOLUMETRIC_HEAT_CAPACITY).min(mixed_layer_depth);
        let mut cells = vec![0usize; coarse_count];
        let mut land_cells = vec![0usize; coarse_count];
        let mut lake_cells = vec![0usize; coarse_count];
        let mut lake_depth_sum = vec![0.0; coarse_count];
        let mut lake_height_sum = vec![0.0; coarse_count];
        for (fine, &coarse) in self.fine_to_coarse.iter().enumerate() {
            let coarse = coarse as usize;
            cells[coarse] += 1;
            if is_lake[fine] {
                let depth = lake_depth.map_or(0.0, |d| d[fine]);
                lake_cells[coarse] += 1;
                lake_depth_sum[coarse] += depth.clamp(min_lake_depth, mixed_layer_depth);
                lake_height_sum[coarse] += elevation[fine];
            } else if elevation[fine] >= 0.0 {
                land_cells[coarse] += 1;
            }
        }
        let land_cells_list: Vec<usize> = (0..elevation.len())
            .filter(|&fine| elevation[fine] >= 0.0 && !is_lake[fine])
            .collect();
        let land_fraction: Vec<f64> = (0..coarse_count)
            .map(|c| land_cells[c] as f64 / cells[c].max(1) as f64)
            .collect();
        let lake_fraction: Vec<f64> = (0..coarse_count)
            .map(|c| lake_cells[c] as f64 / cells[c].max(1) as f64)
            .collect();
        // Written so that it is exactly `1 − f` without lakes.
        let ocean_fraction: Vec<f64> = (0..coarse_count)
            .map(|c| 1.0 - land_fraction[c] - lake_fraction[c])
            .collect();
        // A cell without lakes keeps a virtual lake column (as every cell has an ocean column)
        // with the ocean's mixed layer at sea level, from which a new lake starts.
        let lake_heat_capacity: Vec<f64> = (0..coarse_count)
            .map(|c| {
                if lake_cells[c] > 0 {
                    WATER_VOLUMETRIC_HEAT_CAPACITY * lake_depth_sum[c] / lake_cells[c] as f64
                } else {
                    params.ocean_heat_capacity
                }
            })
            .collect();
        let lake_freezing: Vec<f64> = (0..coarse_count)
            .map(|c| {
                LAKE_FREEZING_TEMPERATURE
                    + params.lapse_rate * lake_height_sum[c] / lake_cells[c].max(1) as f64
            })
            .collect();
        if params.open_water_lakes {
            // A lake column whose lakes changed keeps the temperature of its water (starting at
            // its freezing point if that is now higher); its ice stays as it is.
            for c in 0..coarse_count {
                let (capacity, freezing) = self.lake_column[c];
                if self.lake[c] >= 0.0 {
                    let t = freezing + self.lake[c] / capacity;
                    self.lake[c] = lake_heat_capacity[c] * (t - lake_freezing[c]).max(0.0);
                }
                self.lake_column[c] = (lake_heat_capacity[c], lake_freezing[c]);
            }
        }
        let heat_capacity: Vec<f64> = (0..coarse_count)
            .map(|c| {
                land_fraction[c] * params.land_heat_capacity
                    + ocean_fraction[c] * params.ocean_heat_capacity
                    + lake_fraction[c] * lake_heat_capacity[c]
            })
            .collect();

        // Hexagonal-lattice discretisation of the Laplacian on the unit sphere.
        let spacing = self.coarse.mean_edge_angle();
        let coupling = params.local_heat_transport * 2.0 / (3.0 * spacing * spacing);

        let bands = params.latitude_bands;
        let mut band_cells = vec![0usize; bands];
        let mut band_heat_capacity = vec![0.0; bands];
        for cell in 0..coarse_count {
            band_cells[self.band[cell]] += 1;
            band_heat_capacity[self.band[cell]] += heat_capacity[cell];
        }
        for band in 0..bands {
            band_heat_capacity[band] /= band_cells[band].max(1) as f64;
        }
        let mut band_temperature = vec![0.0; bands];
        let mut band_flux = vec![0.0; bands];
        let band_diffusion = vec![params.heat_transport; bands - 1];
        let mut interface_flux = vec![0.0; bands - 1];
        let mut transport_sum = vec![0.0; bands - 1];

        let seasons = params.seasons_per_year;
        let insolation: Vec<Vec<f64>> = (0..seasons)
            .map(|season| {
                let position = orbit.position((season as f64 + 0.5) / seasons as f64);
                self.coarse_latitude
                    .iter()
                    .map(|&latitude| orbit.daily_insolation(latitude, position))
                    .collect()
            })
            .collect();

        let years = if self.spun_up {
            params.years_per_update
        } else {
            params.spin_up_years
        };
        self.spun_up = true;

        // Outgoing longwave radiation `A − F + B (T + inversion)`, linear in T on either side of
        // the inversion temperature: (A − F, B) for a surface at `t`. The greenhouse forcing F is
        // 0 by default, which leaves A exactly as it is.
        let radiation_a = params.radiation_a - params.greenhouse_forcing;
        let radiation = |t: f64| {
            if t < params.inversion_temperature {
                let k = params.inversion_strength;
                (
                    radiation_a + params.radiation_b * k * params.inversion_temperature,
                    params.radiation_b * (1.0 - k),
                )
            } else {
                (radiation_a, params.radiation_b)
            }
        };
        let ice_thickness = |enthalpy: f64| (-enthalpy / params.ice_latent_heat).max(0.0);
        // The clouds of the calibrated albedos, brightened or darkened by the cloud nuclei of the
        // planet (#204): open ocean by latitude (sea salt), land and lakes in continental air.
        let clouds = crate::aerosol::CloudBrightening::new(&params.aerosol, self.aerosol_sources);
        let marine: Vec<(crate::aerosol::AirMass, f64)> = self
            .coarse_latitude
            .iter()
            .map(|&latitude| clouds.ocean(latitude))
            .collect();
        let (continental, land_change) = clouds.land();
        let lake_change = clouds.lake();
        let ocean_albedo: Vec<f64> = self
            .coarse_latitude
            .iter()
            .zip(&marine)
            .map(|(latitude, (_, change))| {
                let x = math::sin(*latitude);
                params.ocean_albedo
                    + params.ocean_albedo_latitude * (3.0 * x * x - 1.0) / 2.0
                    + change
            })
            .collect();
        // Open lake water: the calibrated open water of its latitude in continental air.
        let lake_albedo: Vec<f64> = (0..coarse_count)
            .map(|cell| ocean_albedo[cell] - marine[cell].1 + lake_change)
            .collect();
        let aerosol = {
            let (mut weight, mut ccn, mut droplets, mut cloud, mut change) =
                (0.0, 0.0, 0.0, 0.0, 0.0);
            for cell in 0..coarse_count {
                let w = ocean_fraction[cell];
                let (air, c) = marine[cell];
                weight += w;
                ccn += w * air.ccn;
                droplets += w * air.droplets;
                cloud += w * air.cloud_albedo;
                change += w * c;
            }
            let weight = weight.max(1e-12);
            crate::aerosol::AerosolDiagnostics {
                sources: self.aerosol_sources,
                marine: crate::aerosol::AirMass {
                    ccn: ccn / weight,
                    droplets: droplets / weight,
                    cloud_albedo: cloud / weight,
                },
                continental,
                ocean_change: change / weight,
                land_change,
            }
        };

        // Wind-driven ocean heat transport by the subtropical cells across every band interface:
        // the poleward Ekman mass transport under the trade winds, M = −τ_x f / (f² + r²) per
        // metre (kg/m/s, northward), times the width of the ocean.
        let mut band_ocean = vec![0.0; bands];
        for cell in 0..coarse_count {
            band_ocean[self.band[cell]] += ocean_fraction[cell];
        }
        for band in 0..bands {
            band_ocean[band] /= band_cells[band].max(1) as f64;
        }
        let ekman: Vec<f64> = (0..bands - 1)
            .map(|b| {
                let x = -1.0 + (b + 1) as f64 * 2.0 / bands as f64;
                let latitude = math::asin(x);
                let stress = crate::climate::surface_wind_stress(latitude.to_degrees(), 4.5);
                if stress >= 0.0 || latitude.to_degrees().abs() > params.subduction_latitude {
                    return 0.0;
                }
                let f = 2.0 * EARTH_ROTATION * math::sin(latitude);
                let mass = -stress * f / (f * f + params.equatorial_friction.powi(2));
                let width = 2.0
                    * PI
                    * EARTH_RADIUS
                    * math::cos(latitude)
                    * 0.5
                    * (band_ocean[b] + band_ocean[b + 1]);
                mass * width
            })
            .collect();
        // The bands holding the subduction latitude in the south and the north.
        let subduction_band = |sign: f64| {
            let x = math::sin(sign * params.subduction_latitude.to_radians());
            (((x + 1.0) / 2.0 * bands as f64) as usize).min(bands - 1)
        };
        let subduction = [subduction_band(-1.0), subduction_band(1.0)];
        let band_area = 4.0 * PI * EARTH_RADIUS * EARTH_RADIUS / bands as f64;
        // Wind-driven gyres in the cells that are mostly ocean (not lakes).
        let is_ocean: Vec<bool> = (0..coarse_count)
            .map(|c| land_fraction[c] + lake_fraction[c] < 0.5)
            .collect();
        let gyres = self.gyre_model.solve(
            &self.coarse,
            &is_ocean,
            &|latitude| crate::climate::surface_wind_stress(latitude, 4.5),
            params.gyre_friction,
        );
        let cell_area = 4.0 * PI * EARTH_RADIUS * EARTH_RADIUS / coarse_count as f64;
        let mut gyre_heat = vec![0.0; coarse_count];
        let mut mixed_layer = vec![0.0; coarse_count];
        let mut band_gyre = vec![0.0; bands];
        let mut band_sst = vec![0.0; bands];
        let mut band_ocean_flux = vec![0.0; bands];
        let mut ocean_transport_sum = vec![0.0; bands - 1];

        let dt = SECONDS_PER_YEAR / (seasons * params.substeps_per_season) as f64;
        let substeps = params.substeps_per_season as f64;
        let mut seasonal_land = vec![vec![0.0; coarse_count]; seasons];
        let mut seasonal_ocean = vec![vec![0.0; coarse_count]; seasons];
        let mut seasonal_mean = vec![vec![0.0; coarse_count]; seasons];
        let mut seasonal_lake = vec![vec![0.0; coarse_count]; seasons];
        let mut seasonal_lake_ice = vec![vec![0.0; coarse_count]; seasons];
        let mut annual_lake_ice = vec![0.0; coarse_count];
        let mut annual_land = vec![0.0; coarse_count];
        let mut annual_sea_ice = vec![0.0; coarse_count];
        let mut annual_gyre_heat = vec![0.0; coarse_count];
        let mut mean = vec![0.0; coarse_count];
        // Global mean net radiation at the top of the atmosphere in the last year (W/m²).
        let mut toa_imbalance = 0.0;
        // Sunlight reflected and received in the last year (summed W/m² over cells and steps).
        let (mut reflected, mut incoming) = (0.0, 0.0);
        for year in 0..years {
            let last_year = year + 1 == years;
            for season in seasonal_land
                .iter_mut()
                .chain(&mut seasonal_ocean)
                .chain(&mut seasonal_mean)
                .chain(&mut seasonal_lake)
                .chain(&mut seasonal_lake_ice)
            {
                season.fill(0.0);
            }
            transport_sum.fill(0.0);
            ocean_transport_sum.fill(0.0);
            annual_land.fill(0.0);
            annual_sea_ice.fill(0.0);
            annual_lake_ice.fill(0.0);
            annual_gyre_heat.fill(0.0);
            // Albedo of the land from the ice sheets, which depend on last year's temperatures:
            // the mean over the fine land cells of every coarse cell, each with its own elevation
            // and snowfall.
            let mut land_ice = vec![0.0; coarse_count];
            for &fine in &land_cells_list {
                let t = self.interpolate(&self.previous_annual_land, fine)
                    + self.free_air_shift(fine, elevation[fine])
                    - params.lapse_rate * elevation[fine];
                let p = precipitation.map_or(f64::INFINITY, |p| p[fine]);
                land_ice[self.fine_to_coarse[fine] as usize] += ice_sheet_cover(t, p, params);
            }
            let land_albedo: Vec<f64> = (0..coarse_count)
                .map(|cell| {
                    let ice = land_ice[cell] / land_cells[cell].max(1) as f64;
                    let open = params.land_albedo + land_change;
                    open + (params.ice_albedo - open) * ice
                })
                .collect();
            for (season, insolation) in insolation.iter().enumerate() {
                for _ in 0..params.substeps_per_season {
                    // The air above every cell: the area mean of its land, ocean and lake
                    // surfaces.
                    for cell in 0..coarse_count {
                        mean[cell] = land_fraction[cell] * self.land[cell]
                            + ocean_fraction[cell] * self.ocean_surface[cell]
                            + lake_fraction[cell] * self.lake_surface[cell];
                    }
                    // Meridional transport between latitude bands, solved implicitly.
                    // Ocean heat transport of the subtropical cells.
                    band_sst.fill(0.0);
                    for cell in 0..coarse_count {
                        band_sst[self.band[cell]] +=
                            ocean_fraction[cell] * self.ocean_surface[cell];
                    }
                    for band in 0..bands {
                        band_sst[band] /= (band_ocean[band] * band_cells[band] as f64).max(1e-9);
                    }
                    band_ocean_flux.fill(0.0);
                    for (b, &mass) in ekman.iter().enumerate() {
                        if mass == 0.0 {
                            continue;
                        }
                        let hemisphere = usize::from(b + 1 > bands / 2);
                        let surface = 0.5 * (band_sst[b] + band_sst[b + 1]);
                        let heat = WATER_HEAT_CAPACITY
                            * mass
                            * (surface - self.subduction_temperature[hemisphere]);
                        band_ocean_flux[b] -= heat;
                        band_ocean_flux[b + 1] += heat;
                        ocean_transport_sum[b] += heat;
                    }
                    // Gyres, carrying the temperature of the mixed layer (the freezing point under
                    // sea ice).
                    for cell in 0..coarse_count {
                        mixed_layer[cell] = params.freezing_temperature
                            + self.ocean[cell].max(0.0) / params.ocean_heat_capacity;
                    }
                    gyres.heat_convergence(
                        &mixed_layer,
                        params.freezing_temperature,
                        params.gyre_heat_share,
                        WATER_HEAT_CAPACITY,
                        &mut gyre_heat,
                    );
                    // Northward transport of the gyres across the band interfaces: what the bands
                    // south of an interface lose.
                    band_gyre.fill(0.0);
                    for cell in 0..coarse_count {
                        band_gyre[self.band[cell]] += gyre_heat[cell];
                    }
                    let mut south = 0.0;
                    for (b, sum) in ocean_transport_sum.iter_mut().enumerate() {
                        south += band_gyre[b];
                        *sum -= south;
                    }
                    for cell in 0..coarse_count {
                        let band = self.band[cell];
                        let ocean_area = ocean_fraction[cell] * cell_area;
                        if is_ocean[cell] {
                            annual_gyre_heat[cell] += gyre_heat[cell] / ocean_area.max(1.0);
                        }
                        self.ocean_heat_convergence[cell] = band_ocean_flux[band]
                            / (band_ocean[band] * band_area).max(1.0)
                            + if is_ocean[cell] {
                                gyre_heat[cell] / ocean_area.max(1.0)
                            } else {
                                0.0
                            };
                    }

                    band_temperature.fill(0.0);
                    for cell in 0..coarse_count {
                        band_temperature[self.band[cell]] += mean[cell];
                    }
                    for band in 0..bands {
                        band_temperature[band] /= band_cells[band].max(1) as f64;
                    }
                    meridional_transport(
                        &band_temperature,
                        &band_heat_capacity,
                        &band_diffusion,
                        dt,
                        &mut band_flux,
                        &mut interface_flux,
                    );
                    for (sum, flux) in transport_sum.iter_mut().zip(&interface_flux) {
                        *sum += flux;
                    }

                    for cell in 0..coarse_count {
                        let f = land_fraction[cell];
                        let q = insolation[cell];
                        // Local transport towards the neighbouring air, implicit in the
                        // surface's own temperature; stable for any time step.
                        let neighbors = self.coarse.neighbors(cell as u32);
                        let neighbor_sum: f64 = neighbors.iter().map(|&n| mean[n as usize]).sum();
                        let local = coupling * neighbors.len() as f64;
                        let transport = band_flux[self.band[cell]] + coupling * neighbor_sum;
                        // Heat exchange through the air with the other surfaces of the cell, in
                        // proportion to their shares (exactly today's land–ocean exchange
                        // without lakes).
                        let k = params.land_ocean_exchange;
                        let (to_land, to_ocean, to_lake) =
                            (k * f, k * ocean_fraction[cell], k * lake_fraction[cell]);

                        // Land: C dT/dt = Q (1 − α) − OLR + transport + exchange with the ocean
                        // and the lakes.
                        let land = self.land[cell];
                        let (a, b) = radiation(land);
                        if last_year {
                            toa_imbalance += f * (q * (1.0 - land_albedo[cell]) - a - b * land);
                            reflected += f * q * land_albedo[cell];
                            incoming += q;
                        }
                        let rate = dt / params.land_heat_capacity;
                        self.land[cell] = (land
                            + rate
                                * (q * (1.0 - land_albedo[cell]) - a
                                    + transport
                                    + to_ocean * self.ocean_surface[cell]
                                    + to_lake * self.lake_surface[cell]))
                            / (1.0 + rate * (b + local + to_ocean + to_lake));

                        // Ocean column (Wagner & Eisenman, 2015).
                        let ocean = WaterColumn {
                            heat_capacity: params.ocean_heat_capacity,
                            freezing: params.freezing_temperature,
                            melting: 0.0,
                        };
                        let albedo = ocean.albedo(
                            params,
                            ocean_albedo[cell],
                            self.ocean[cell],
                            self.ocean_surface[cell],
                        );
                        let (a, b) = radiation(self.ocean_surface[cell]);
                        if last_year {
                            toa_imbalance += ocean_fraction[cell]
                                * (q * (1.0 - albedo) - a - b * self.ocean_surface[cell]);
                            reflected += ocean_fraction[cell] * q * albedo;
                        }
                        // Net heating of the surface: `forcing − damping · T_surface`.
                        let forcing = q * (1.0 - albedo) - a
                            + transport
                            + to_land * self.land[cell]
                            + to_lake * self.lake_surface[cell];
                        // Heat from the currents reaches the mixed layer, under ice its base.
                        let currents = self.ocean_heat_convergence[cell];
                        let damping = b + local + to_land + to_lake;
                        (self.ocean_surface[cell], self.ocean[cell]) =
                            ocean.step(params, self.ocean[cell], forcing, damping, currents, dt);

                        // Lake column: the same physics without currents, in fresh water.
                        if params.open_water_lakes {
                            let lake = WaterColumn {
                                heat_capacity: lake_heat_capacity[cell],
                                freezing: lake_freezing[cell],
                                melting: lake_freezing[cell],
                            };
                            let albedo = lake.albedo(
                                params,
                                lake_albedo[cell],
                                self.lake[cell],
                                self.lake_surface[cell],
                            );
                            let (a, b) = radiation(self.lake_surface[cell]);
                            if last_year {
                                toa_imbalance += lake_fraction[cell]
                                    * (q * (1.0 - albedo) - a - b * self.lake_surface[cell]);
                                reflected += lake_fraction[cell] * q * albedo;
                            }
                            let forcing = q * (1.0 - albedo) - a
                                + transport
                                + to_land * self.land[cell]
                                + to_ocean * self.ocean_surface[cell];
                            let damping = b + local + to_land + to_ocean;
                            (self.lake_surface[cell], self.lake[cell]) =
                                lake.step(params, self.lake[cell], forcing, damping, 0.0, dt);
                        }
                    }

                    for cell in 0..coarse_count {
                        let surface = self.ocean_surface[cell];
                        seasonal_land[season][cell] += self.land[cell] / substeps;
                        seasonal_ocean[season][cell] += surface / substeps;
                        seasonal_mean[season][cell] += (land_fraction[cell] * self.land[cell]
                            + ocean_fraction[cell] * surface
                            + lake_fraction[cell] * self.lake_surface[cell])
                            / substeps;
                        annual_land[cell] += self.land[cell];
                        annual_sea_ice[cell] += (ice_thickness(self.ocean[cell])
                            / params.ice_albedo_thickness)
                            .min(1.0);
                        if params.open_water_lakes {
                            let ice = (ice_thickness(self.lake[cell])
                                / params.ice_albedo_thickness)
                                .min(1.0);
                            seasonal_lake[season][cell] += self.lake_surface[cell] / substeps;
                            seasonal_lake_ice[season][cell] += ice / substeps;
                            annual_lake_ice[cell] += ice;
                        }
                    }
                }
            }
            let steps_per_year = (seasons * params.substeps_per_season) as f64;
            for (hemisphere, &band) in subduction.iter().enumerate() {
                self.subduction_temperature[hemisphere] = seasonal_ocean
                    .iter()
                    .map(|season| {
                        let (sum, weight) = (0..coarse_count)
                            .filter(|&cell| self.band[cell] == band)
                            .fold((0.0, 0.0), |(sum, weight), cell| {
                                let w = ocean_fraction[cell];
                                (sum + w * season[cell], weight + w)
                            });
                        sum / weight.max(1e-9)
                    })
                    .fold(f64::INFINITY, f64::min);
            }
            for (previous, &sum) in self.previous_annual_land.iter_mut().zip(&annual_land) {
                *previous = sum / steps_per_year;
            }
            // The free troposphere: the annual mean temperature of the sea surface of each
            // latitude band, interpolated between the bands.
            let annual = |seasonal: &[Vec<f64>], cell: usize| {
                seasonal.iter().map(|season| season[cell]).sum::<f64>() / seasons as f64
            };
            let mut band_sea = vec![(0.0, 0.0); bands];
            let mut band_air = vec![(0.0, 0.0); bands];
            for cell in 0..coarse_count {
                let sea = ocean_fraction[cell];
                let band = self.band[cell];
                band_sea[band].0 += sea * annual(&seasonal_ocean, cell);
                band_sea[band].1 += sea;
                band_air[band].0 += annual(&seasonal_mean, cell);
                band_air[band].1 += 1.0;
            }
            // A band with little sea leans on the mean of all its surfaces instead.
            const AIR_SHARE: f64 = 0.05;
            let free_air: Vec<f64> = (0..bands)
                .map(|band| {
                    (band_sea[band].0 + AIR_SHARE * band_air[band].0)
                        / (band_sea[band].1 + AIR_SHARE * band_air[band].1).max(1e-9)
                })
                .collect();
            for cell in 0..coarse_count {
                // Position among the band centres, which are equally spaced in sin(latitude).
                let position = ((math::sin(self.coarse_latitude[cell]) + 1.0) / 2.0 * bands as f64
                    - 0.5)
                    .clamp(0.0, (bands - 1) as f64);
                let lower = (position as usize).min(bands - 2);
                let t = position - lower as f64;
                let regional = free_air[lower] * (1.0 - t) + free_air[lower + 1] * t;
                self.previous_free_air_excess[cell] = regional - self.previous_annual_land[cell];
            }
            for sea_ice in annual_sea_ice.iter_mut().chain(&mut annual_lake_ice) {
                *sea_ice /= steps_per_year;
            }
            for heat in &mut annual_gyre_heat {
                *heat /= steps_per_year;
            }
        }

        // Monthly means of the sea-level temperatures on the coarse mesh.
        let seasons_per_month = seasons / 12;
        let monthly_means = |seasonal: &[Vec<f64>]| -> Vec<Vec<f64>> {
            seasonal
                .chunks(seasons_per_month)
                .map(|chunk| {
                    (0..coarse_count)
                        .map(|cell| {
                            chunk.iter().map(|season| season[cell]).sum::<f64>()
                                / seasons_per_month as f64
                        })
                        .collect()
                })
                .collect()
        };
        let monthly_land = monthly_means(&seasonal_land);
        let monthly_ocean = monthly_means(&seasonal_ocean);
        let monthly_coarse = monthly_means(&seasonal_mean);
        let monthly_lake = monthly_means(&seasonal_lake);
        let monthly_lake_ice = monthly_means(&seasonal_lake_ice);
        // Lake cells interpolate the lake columns of the coarse cells that have lakes (their own
        // coarse cell always has).
        let interpolate_lake = |coarse_field: &[f64], fine: usize| {
            let (sum, weight) = self.interpolation[fine]
                .iter()
                .filter(|&&(cell, _)| lake_cells[cell as usize] > 0)
                .fold((0.0, 0.0), |(sum, weight), &(cell, w)| {
                    (sum + w * coarse_field[cell as usize], weight + w)
                });
            if weight > 0.0 {
                sum / weight
            } else {
                coarse_field[self.fine_to_coarse[fine] as usize]
            }
        };

        // High ground follows the free troposphere around it in the annual mean (see
        // [`TemperatureParams::free_air_height`]).
        let lapse = |fine: usize, sea_level: f64| {
            sea_level + self.free_air_shift(fine, elevation[fine])
                - params.lapse_rate * elevation[fine].max(0.0)
        };
        // Every fine cell takes the temperature of its own surface type.
        let surface = |fine: usize, month: usize| {
            if is_lake[fine] {
                // The lapse rate alone, as the lake column freezes by it (no free-air shift).
                interpolate_lake(&monthly_lake[month], fine)
                    - params.lapse_rate * elevation[fine].max(0.0)
            } else if elevation[fine] >= 0.0 {
                lapse(fine, self.interpolate(&monthly_land[month], fine))
            } else {
                self.interpolate(&monthly_ocean[month], fine)
            }
        };
        let monthly: Vec<Vec<f64>> = (0..12)
            .map(|month| {
                (0..elevation.len())
                    .map(|fine| surface(fine, month))
                    .collect()
            })
            .collect();
        let annual_mean = (0..elevation.len())
            .map(|fine| monthly.iter().map(|month| month[fine]).sum::<f64>() / 12.0)
            .collect();

        // Ice cover of the fine cells: sea ice and lake ice (annual mean cover), ice sheets by
        // temperature and snowfall.
        let ice = (0..elevation.len())
            .map(|fine| {
                if is_lake[fine] {
                    interpolate_lake(&annual_lake_ice, fine)
                } else if elevation[fine] < 0.0 {
                    self.interpolate(&annual_sea_ice, fine)
                } else {
                    let t = lapse(fine, self.interpolate(&self.previous_annual_land, fine));
                    let p = precipitation.map_or(f64::INFINITY, |p| p[fine]);
                    ice_sheet_cover(t, p, params)
                }
            })
            .collect();
        let monthly_lake_ice = if params.open_water_lakes {
            monthly_lake_ice
                .iter()
                .map(|month| {
                    (0..elevation.len())
                        .map(|fine| {
                            if is_lake[fine] {
                                interpolate_lake(month, fine)
                            } else {
                                f64::NAN
                            }
                        })
                        .collect()
                })
                .collect()
        } else {
            Vec::new()
        };

        let thermal_equator = monthly_coarse
            .iter()
            .map(|month| {
                (0..THERMAL_EQUATOR_SECTORS)
                    .map(|sector| self.thermal_equator(month, sector))
                    .collect()
            })
            .collect();

        // The gyres on the fine mesh: the transport and heating of the coarse cell (the model's
        // resolution), the streamfunction interpolated.
        let transport = self.gyre_model.transport(&self.coarse, &is_ocean, &gyres);
        let of_coarse = |fine: usize| {
            let coarse = self.fine_to_coarse[fine] as usize;
            (elevation[fine] < 0.0 && is_ocean[coarse]).then_some(coarse)
        };
        let ocean_current = (0..elevation.len())
            .map(|fine| of_coarse(fine).map_or([f64::NAN; 2], |coarse| transport[coarse]))
            .collect();
        let gyre_heating = (0..elevation.len())
            .map(|fine| of_coarse(fine).map_or(f64::NAN, |coarse| annual_gyre_heat[coarse]))
            .collect();
        let gyre_streamfunction = (0..elevation.len())
            .map(|fine| {
                if elevation[fine] < 0.0 {
                    self.interpolate(&gyres.streamfunction, fine) / 1e6
                } else {
                    f64::NAN
                }
            })
            .collect();

        self.fields = TemperatureFields {
            ocean_current,
            gyre_streamfunction,
            gyre_heating,
            annual_mean,
            monthly,
            thermal_equator,
            ice,
            monthly_lake_ice,
            solar_constant: orbit.solar_constant,
            toa_imbalance: toa_imbalance
                / ((seasons * params.substeps_per_season) as f64 * coarse_count as f64),
            planetary_albedo: reflected / incoming.max(1e-12),
            aerosol,
            gyre_transport: gyres
                .streamfunction
                .iter()
                .fold(0.0, |max: f64, psi| max.max(psi.abs()))
                / 1e6,
            meridional_transport: (0..bands - 1)
                .map(|b| {
                    let x = -1.0 + (b + 1) as f64 * 2.0 / bands as f64;
                    let steps = (seasons * params.substeps_per_season) as f64;
                    (
                        math::asin(x).to_degrees(),
                        transport_sum[b] / steps * 4.0 * PI / bands as f64,
                        ocean_transport_sum[b] / steps / (EARTH_RADIUS * EARTH_RADIUS),
                    )
                })
                .collect(),
        };
    }

    /// The latitude (degrees) between 30°S and 30°N where the mean of a coarse field over the
    /// longitudes around the centre of sector `sector` (of [`THERMAL_EQUATOR_SECTORS`] from
    /// 180°W eastwards; weighted with a Gaussian of 30° width), smoothed with a Gaussian of 5°
    /// width in latitude, is largest.
    fn thermal_equator(&self, field: &[f64], sector: usize) -> f64 {
        // Means in bins of 0.5° between 60°S and 60°N, then smoothed.
        let mut sum = [0.0; EQUATOR_BINS];
        let mut count = [0.0; EQUATOR_BINS];
        let weights = &self.equator_weights[sector];
        for ((&bin, &weight), &value) in self.equator_bins.iter().zip(weights).zip(field) {
            if bin != u32::MAX {
                sum[bin as usize] += weight * value;
                count[bin as usize] += weight;
            }
        }
        let smoothed = |bin: usize| {
            let (mut s, mut c) = (0.0, 0.0);
            for other in bin.saturating_sub(30)..(bin + 31).min(EQUATOR_BINS) {
                let weight = self.equator_kernel[other + 30 - bin];
                s += weight * sum[other];
                c += weight * count[other];
            }
            s / c.max(1e-12)
        };
        // Bins from 30°S to 30°N.
        (60..=180)
            .map(|bin| (bin, smoothed(bin)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map_or(0.0, |(bin, _)| bin as f64 / 2.0 - 60.0 + 0.25)
    }

    /// Shift (K) of the annual mean temperature of the fine land cell `fine` at `elevation` (m)
    /// towards the free troposphere (see [`TemperatureParams::free_air_height`]): never
    /// negative, growing with elevation, and fading out between
    /// [`TemperatureParams::free_air_latitude`] and 15° beyond it.
    fn free_air_shift(&self, fine: usize, elevation: f64) -> f64 {
        let params = &self.params;
        if params.free_air_height <= 0.0 {
            return 0.0;
        }
        let latitude = self.coarse_latitude[self.fine_to_coarse[fine] as usize]
            .abs()
            .to_degrees();
        let tropical = ((params.free_air_latitude + 15.0 - latitude) / 15.0).clamp(0.0, 1.0);
        let base = params.free_air_base.min(params.free_air_height - 1.0);
        let high = ((elevation - base) / (params.free_air_height - base)).clamp(0.0, 1.0);
        high * tropical
            * self
                .interpolate(&self.previous_free_air_excess, fine)
                .max(0.0)
    }

    fn interpolate(&self, coarse_field: &[f64], fine: usize) -> f64 {
        self.interpolation[fine]
            .iter()
            .map(|&(cell, weight)| coarse_field[cell as usize] * weight)
            .sum()
    }
}

/// Computes the heat flux convergence (W/m²) of each latitude band caused by diffusion
/// `D d/dx (1 - x²) dT/dx` in `x = sin(latitude)`, with bands of equal width in `x`. Solved
/// implicitly over the time step `dt`, so it is stable for any step.
/// `interface_flux` receives the northward flux across the interface above every band, per unit
/// of band area (W/m²).
fn meridional_transport(
    temperature: &[f64],
    heat_capacity: &[f64],
    diffusion: &[f64],
    dt: f64,
    flux: &mut [f64],
    interface_flux: &mut [f64],
) {
    let n = temperature.len();
    let width = 2.0 / n as f64;
    // Coupling across the interface above band `b`, which lies at x = -1 + (b + 1) * width.
    let interface = |b: usize| {
        let x = -1.0 + (b + 1) as f64 * width;
        diffusion[b] * (1.0 - x * x) / (width * width)
    };

    // Tridiagonal system: (C/dt + k_lo + k_hi) T'_b - k_lo T'_{b-1} - k_hi T'_{b+1} = C/dt T_b.
    let mut lower = vec![0.0; n];
    let mut diagonal = vec![0.0; n];
    let mut upper = vec![0.0; n];
    let mut rhs = vec![0.0; n];
    for b in 0..n {
        let k_lo = if b > 0 { interface(b - 1) } else { 0.0 };
        let k_hi = if b + 1 < n { interface(b) } else { 0.0 };
        let inertia = heat_capacity[b] / dt;
        lower[b] = -k_lo;
        upper[b] = -k_hi;
        diagonal[b] = inertia + k_lo + k_hi;
        rhs[b] = inertia * temperature[b];
    }
    // Thomas algorithm.
    for b in 1..n {
        let factor = lower[b] / diagonal[b - 1];
        diagonal[b] -= factor * upper[b - 1];
        rhs[b] -= factor * rhs[b - 1];
    }
    let mut solution = vec![0.0; n];
    solution[n - 1] = rhs[n - 1] / diagonal[n - 1];
    for b in (0..n - 1).rev() {
        solution[b] = (rhs[b] - upper[b] * solution[b + 1]) / diagonal[b];
    }
    for b in 0..n {
        flux[b] = heat_capacity[b] * (solution[b] - temperature[b]) / dt;
    }
    for b in 0..n - 1 {
        interface_flux[b] = interface(b) * (solution[b] - solution[b + 1]);
    }
}

/// Ice-sheet cover (0 to 1) of land with the annual mean temperature `t` (°C) and the
/// precipitation `precipitation` (m/yr).
fn ice_sheet_cover(t: f64, precipitation: f64, params: &TemperatureParams) -> f64 {
    blend(0.0, 1.0, t, params.land_ice_temperature)
        * (precipitation / params.ice_sheet_precipitation).clamp(0.0, 1.0)
}

/// A column of open water or ice after Wagner & Eisenman (2015): its enthalpy (J/m²) is the heat
/// of the mixed layer above the freezing point (positive, open water) or the latent heat of its
/// ice (negative). The ocean column and the lake column differ in these parameters only.
struct WaterColumn {
    /// Heat capacity of the mixed layer in J/m²/K.
    heat_capacity: f64,
    /// Temperature (°C) of the water at which it freezes.
    freezing: f64,
    /// Temperature (°C) the ice surface cannot exceed.
    melting: f64,
}

impl WaterColumn {
    /// Planetary albedo of the column with the given enthalpy and last surface temperature, over
    /// open water of the albedo `open`: towards the sea-ice albedo (melting ice when the surface
    /// is at the melting point) with the ice cover.
    fn albedo(&self, params: &TemperatureParams, open: f64, enthalpy: f64, surface: f64) -> f64 {
        let h = (-enthalpy / params.ice_latent_heat).max(0.0);
        let ice_cover = (h / params.ice_albedo_thickness).min(1.0);
        let ice_albedo = if surface >= self.melting {
            params.melting_ice_albedo
        } else {
            params.sea_ice_albedo
        };
        open + (ice_albedo - open) * ice_cover
    }

    /// One implicit time step `dt` (s) of the column under the net surface heating
    /// `forcing − damping · T_surface` (W/m²) and the heat `currents` (W/m²) delivered to the mixed
    /// layer (under ice its base). Returns the new surface temperature and enthalpy.
    fn step(
        &self,
        params: &TemperatureParams,
        enthalpy: f64,
        forcing: f64,
        damping: f64,
        currents: f64,
        dt: f64,
    ) -> (f64, f64) {
        if enthalpy >= 0.0 {
            // Open water: the mixed layer, implicit in its temperature.
            let c = self.heat_capacity;
            let t = self.freezing + enthalpy / c;
            let t = (t + dt / c * (forcing + currents)) / (1.0 + dt / c * damping);
            // Cooling below the freezing point forms ice instead.
            let next = c * (t - self.freezing);
            (t.max(self.freezing), next)
        } else {
            // Ice: the surface temperature balances conduction through the ice with the surface
            // fluxes; it cannot exceed melting.
            let h = (-enthalpy / params.ice_latent_heat).max(0.0);
            let conductance = params.ice_conductivity / h.max(0.01);
            let t = ((conductance * self.freezing + forcing) / (conductance + damping))
                .min(self.melting);
            let enthalpy = enthalpy + dt * (forcing - damping * t + currents);
            // Thick ice drifts away and melts elsewhere (on a lake: the cap stands in for the
            // ice that a lake's heat from below would melt).
            let limit = -params.max_ice_thickness * params.ice_latent_heat;
            (t, enthalpy.max(limit))
        }
    }
}

/// Linear transition from `warm` to `cold` over ±5 K around `threshold`.
fn blend(warm: f64, cold: f64, temperature: f64, threshold: f64) -> f64 {
    let coldness = ((threshold + 5.0 - temperature) / 10.0).clamp(0.0, 1.0);
    warm + (cold - warm) * coldness
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbit_follows_earth() {
        let orbit = OrbitParams::default();
        // Early January: near perihelion, southern summer.
        let january = orbit.position(0.0);
        assert!(january.distance < 0.985);
        assert!(january.declination.to_degrees() < -22.0);
        // About half a year later: aphelion, northern summer.
        let july = orbit.position(0.5);
        assert!(july.distance > 1.015);
        assert!(july.declination.to_degrees() > 22.0);
        // The maximum declination is the obliquity.
        let max = (0..1000)
            .map(|i| orbit.position(i as f64 / 1000.0).declination.to_degrees())
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((max - 23.44).abs() < 0.01);
    }

    #[test]
    fn insolation_has_polar_day_and_night() {
        let orbit = OrbitParams::default();
        let july = orbit.position(0.5);
        let north_pole = orbit.daily_insolation(90f64.to_radians(), july);
        let equator = orbit.daily_insolation(0.0, july);
        // At the summer solstice, the pole receives more energy than the equator.
        assert!(north_pole > equator);
        assert!(north_pole > 500.0);
        assert_eq!(orbit.daily_insolation(-80f64.to_radians(), july), 0.0);

        // The global annual mean is a quarter of the solar constant.
        let mut sum = 0.0;
        let mut weight = 0.0;
        for i in 0..365 {
            let position = orbit.position(i as f64 / 365.0);
            for j in 0..180 {
                let latitude = ((j as f64 + 0.5) - 90.0).to_radians();
                sum += orbit.daily_insolation(latitude, position) * math::cos(latitude);
                weight += math::cos(latitude);
            }
        }
        assert!((sum / weight - 1361.0 / 4.0).abs() < 2.0);
    }

    #[test]
    fn earth_like_temperatures() {
        let mesh = SphereMesh::icosphere(5);
        // A continent covering the northern hemisphere between 30° and 70° at longitudes
        // -60°..60°, the rest is ocean.
        let elevation: Vec<f64> = (0..mesh.len() as u32)
            .map(|cell| {
                let p = mesh.vertex(cell);
                let latitude = math::asin(p.z).to_degrees();
                let longitude = math::atan2(p.y, p.x).to_degrees();
                if (30.0..70.0).contains(&latitude) && longitude.abs() < 60.0 {
                    300.0
                } else {
                    -4000.0
                }
            })
            .collect();
        let mut model = TemperatureModel::new(&mesh, TemperatureParams::default());
        model.update(&elevation, None, None, 0.0);
        let fields = model.fields();

        let mean_where = |filter: &dyn Fn(f64, f64) -> bool, field: &dyn Fn(usize) -> f64| {
            let cells: Vec<usize> = (0..mesh.len())
                .filter(|&c| {
                    let p = mesh.vertex(c as u32);
                    filter(
                        math::asin(p.z).to_degrees(),
                        math::atan2(p.y, p.x).to_degrees(),
                    )
                })
                .collect();
            cells.iter().map(|&c| field(c)).sum::<f64>() / cells.len() as f64
        };
        let annual = |c: usize| fields.annual_mean[c];
        let global = mean_where(&|_, _| true, &annual);
        let equator = mean_where(&|lat, _| lat.abs() < 10.0, &annual);
        let poles = mean_where(&|lat, _| lat.abs() > 75.0, &annual);
        assert!((5.0..25.0).contains(&global), "global mean {global}");
        assert!((20.0..32.0).contains(&equator), "equator {equator}");
        assert!(poles < -5.0, "poles {poles}");

        // Continental interiors have larger seasonal ranges than the ocean at the same latitude.
        let range = |c: usize| fields.seasonal_range(c);
        let continental = mean_where(
            &|lat, lon| (45.0..55.0).contains(&lat) && lon.abs() < 30.0,
            &range,
        );
        let oceanic = mean_where(
            &|lat, lon| (45.0..55.0).contains(&lat) && (120.0..180.0).contains(&lon.abs()),
            &range,
        );
        assert!(continental > 2.0 * oceanic, "{continental} vs {oceanic}");

        // July is warmer than January in the north, and vice versa in the south.
        let north_july = mean_where(&|lat, _| (40.0..60.0).contains(&lat), &|c| {
            fields.monthly[6][c]
        });
        let north_january = mean_where(&|lat, _| (40.0..60.0).contains(&lat), &|c| {
            fields.monthly[0][c]
        });
        assert!(north_july > north_january + 5.0);
        let south_july = mean_where(&|lat, _| (-60.0..-40.0).contains(&lat), &|c| {
            fields.monthly[6][c]
        });
        let south_january = mean_where(&|lat, _| (-60.0..-40.0).contains(&lat), &|c| {
            fields.monthly[0][c]
        });
        assert!(south_january > south_july);
    }

    /// A continent at 300 m between the latitudes `continent` (|longitude| < 50°) with a plateau
    /// of `height` m between the latitudes `plateau` (|longitude| < 12°). Returns the annual
    /// mean temperature of the plateau's inner part reduced to sea level with the lapse rate,
    /// that of the open ocean at the plateau's latitudes, and the ice cover of the plateau.
    fn plateau_climate(
        params: TemperatureParams,
        (continent, plateau): ((f64, f64), (f64, f64)),
        height: f64,
        glaciated_start: bool,
    ) -> (f64, f64, f64) {
        let mesh = SphereMesh::icosphere(5);
        let place = |cell: u32| {
            let p = mesh.vertex(cell);
            (
                math::asin(p.z).to_degrees(),
                math::atan2(p.y, p.x).to_degrees(),
            )
        };
        let within = |range: (f64, f64), lat: f64| (range.0..range.1).contains(&lat);
        let elevation: Vec<f64> = (0..mesh.len() as u32)
            .map(|cell| {
                let (lat, lon) = place(cell);
                if within(plateau, lat) && lon.abs() < 12.0 {
                    height
                } else if within(continent, lat) && lon.abs() < 50.0 {
                    300.0
                } else {
                    -4000.0
                }
            })
            .collect();
        let lapse_rate = params.lapse_rate;
        let mut model = TemperatureModel::new(&mesh, params);
        if glaciated_start {
            // Cold enough that the plateau starts under ice.
            model.previous_annual_land.fill(0.0);
        }
        model.update(&elevation, None, None, 0.0);
        let fields = model.fields();
        let mean = |filter: &dyn Fn(f64, f64) -> bool, field: &dyn Fn(usize) -> f64| {
            let cells: Vec<usize> = (0..mesh.len())
                .filter(|&c| {
                    let (lat, lon) = place(c as u32);
                    filter(lat, lon)
                })
                .collect();
            assert!(!cells.is_empty());
            cells.iter().map(|&c| field(c)).sum::<f64>() / cells.len() as f64
        };
        let inner = (plateau.0 + 3.0, plateau.1 - 3.0);
        let on_plateau = |lat: f64, lon: f64| within(inner, lat) && lon.abs() < 8.0;
        (
            mean(&on_plateau, &|c| {
                fields.annual_mean[c] + lapse_rate * elevation[c]
            }),
            mean(&|lat, lon| within(inner, lat) && lon.abs() > 90.0, &|c| {
                fields.annual_mean[c]
            }),
            mean(&on_plateau, &|c| fields.ice[c]),
        )
    }

    /// A continent at 300 m between the latitudes `continent` (|longitude| < 50°) with a lake
    /// of `depth` m between the latitudes `lake` (|longitude| < 15°; none if `depth` is 0), with
    /// the given parameters. Returns the mesh's latitude and longitude of every cell, the lake
    /// depths and the fields.
    fn lake_climate(
        params: TemperatureParams,
        continent: (f64, f64),
        lake: (f64, f64),
        depth: f64,
        lakes_known: bool,
    ) -> (Vec<(f64, f64)>, Vec<f64>, TemperatureFields) {
        let mesh = SphereMesh::icosphere(5);
        let place: Vec<(f64, f64)> = (0..mesh.len() as u32)
            .map(|cell| {
                let p = mesh.vertex(cell);
                (
                    math::asin(p.z).to_degrees(),
                    math::atan2(p.y, p.x).to_degrees(),
                )
            })
            .collect();
        let within = |range: (f64, f64), lat: f64| (range.0..range.1).contains(&lat);
        let elevation: Vec<f64> = place
            .iter()
            .map(|&(lat, lon)| {
                if within(continent, lat) && lon.abs() < 50.0 {
                    300.0
                } else {
                    -4000.0
                }
            })
            .collect();
        let lake_depth: Vec<f64> = place
            .iter()
            .map(|&(lat, lon)| {
                if within(lake, lat) && lon.abs() < 15.0 {
                    depth
                } else {
                    0.0
                }
            })
            .collect();
        let mut model = TemperatureModel::new(&mesh, params);
        model.update(
            &elevation,
            None,
            lakes_known.then_some(&lake_depth[..]),
            0.0,
        );
        (place, lake_depth, model.fields().clone())
    }

    fn mean_over(cells: &[usize], value: &dyn Fn(usize) -> f64) -> f64 {
        assert!(!cells.is_empty());
        cells.iter().map(|&c| value(c)).sum::<f64>() / cells.len() as f64
    }

    #[test]
    fn large_lakes_are_open_water_with_a_maritime_climate() {
        let continent = (25.0, 65.0);
        let lake = (37.0, 53.0);
        let run = |params: TemperatureParams, depth: f64| {
            lake_climate(params, continent, lake, depth, true)
        };
        let (place, depth, water) = run(TemperatureParams::default(), 100.0);
        let off = TemperatureParams {
            open_water_lakes: false,
            ..TemperatureParams::default()
        };
        let (_, _, land) = run(off, 100.0);
        let lake_cells: Vec<usize> = (0..place.len()).filter(|&c| depth[c] > 0.0).collect();
        // Land east and west of the lake, at its latitudes.
        let shore: Vec<usize> = (0..place.len())
            .filter(|&c| {
                let (lat, lon) = place[c];
                (lake.0..lake.1).contains(&lat) && (16.0..24.0).contains(&lon.abs())
            })
            .collect();
        let range = |fields: &TemperatureFields, cells: &[usize]| {
            mean_over(cells, &|c| fields.seasonal_range(c))
        };
        // The lake has the seasonal inertia of water: a much smaller range than as land.
        let (lake_water, lake_land) = (range(&water, &lake_cells), range(&land, &lake_cells));
        assert!(lake_water < 0.6 * lake_land, "{lake_water} {lake_land}");
        // The land on its shores gets a more maritime climate.
        let (shore_water, shore_land) = (range(&water, &shore), range(&land, &shore));
        assert!(shore_water < shore_land - 1.0, "{shore_water} {shore_land}");
        // A shallow lake follows the seasons more closely than a deep one.
        let (_, _, shallow) = run(TemperatureParams::default(), 3.0);
        let lake_shallow = range(&shallow, &lake_cells);
        assert!(
            lake_shallow > lake_water + 2.0,
            "{lake_shallow} {lake_water}"
        );
        // Lake ice is reported on the lakes only.
        assert_eq!(water.monthly_lake_ice.len(), 12);
        assert!(
            (0..place.len()).all(|c| water.monthly_lake_ice[0][c].is_nan() == (depth[c] <= 0.0))
        );
        assert!(land.monthly_lake_ice.is_empty());
    }

    #[test]
    fn a_new_shallow_lake_keeps_the_temperature_of_its_column() {
        // One year per update, so that the fields show the first months after the lake appears.
        let params = TemperatureParams {
            spin_up_years: 2,
            years_per_update: 1,
            ..TemperatureParams::default()
        };
        let mesh = SphereMesh::icosphere(5);
        let latitude: Vec<f64> = (0..mesh.len() as u32)
            .map(|c| math::asin(mesh.vertex(c).z).to_degrees())
            .collect();
        let elevation: Vec<f64> = latitude
            .iter()
            .map(|&lat| if lat.abs() < 30.0 { 100.0 } else { -4000.0 })
            .collect();
        let mut model = TemperatureModel::new(&mesh, params);
        let none = vec![0.0; mesh.len()];
        model.update(&elevation, None, Some(&none), 0.0);
        // A 3 m lake appears on the tropical land: its column (until now one of the ocean's
        // mixed layer) must not carry 50 m of heat into 3 m of water.
        let lake: Vec<f64> = latitude
            .iter()
            .map(|&lat| if lat.abs() < 10.0 { 3.0 } else { 0.0 })
            .collect();
        model.update(&elevation, None, Some(&lake), 0.0);
        let fields = model.fields();
        let hottest = (0..mesh.len())
            .filter(|&c| lake[c] > 0.0)
            .flat_map(|c| fields.monthly.iter().map(move |month| month[c]))
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(hottest < 40.0, "{hottest}");
    }

    #[test]
    fn high_latitude_lakes_freeze_in_winter() {
        let lake = (62.0, 70.0);
        let (place, depth, fields) =
            lake_climate(TemperatureParams::default(), (50.0, 80.0), lake, 50.0, true);
        let lake_cells: Vec<usize> = (0..place.len()).filter(|&c| depth[c] > 0.0).collect();
        let ice = |month: usize| mean_over(&lake_cells, &|c| fields.monthly_lake_ice[month][c]);
        // Frozen in the northern winter (month 1: February), open in late summer (August).
        assert!(ice(1) > 0.9, "{}", ice(1));
        assert!(ice(7) < 0.5, "{}", ice(7));
    }

    #[test]
    fn without_lakes_the_lake_columns_change_nothing() {
        let continent = (25.0, 65.0);
        let off = TemperatureParams {
            open_water_lakes: false,
            ..TemperatureParams::default()
        };
        let (_, _, on) = lake_climate(
            TemperatureParams::default(),
            continent,
            (0.0, 0.0),
            0.0,
            true,
        );
        let (_, _, unknown) = lake_climate(off.clone(), continent, (37.0, 53.0), 100.0, true);
        let (_, _, old) = lake_climate(off, continent, (37.0, 53.0), 100.0, false);
        for fields in [&on, &unknown] {
            assert_eq!(fields.annual_mean, old.annual_mean);
            assert_eq!(fields.monthly, old.monthly);
            assert_eq!(fields.ice, old.ice);
        }
    }

    fn without_free_air() -> TemperatureParams {
        TemperatureParams {
            free_air_height: 0.0,
            ..TemperatureParams::default()
        }
    }

    #[test]
    fn low_latitude_plateaus_are_not_colder_than_the_free_air_around_them() {
        for (shape, height) in [
            // On the equator, 4500 m: about −5 °C by the lapse rate, where ice begins to form.
            (((-25.0, 25.0), (-8.0, 8.0)), 4500.0),
            // In the subtropics, 3500 m: well above freezing by the lapse rate.
            (((5.0, 40.0), (12.0, 28.0)), 3500.0),
        ] {
            // Once glaciated, a plateau with only its own energy balance stays glaciated: its
            // ice keeps it tens of K colder than the air at its height over the ocean.
            let (land, ocean, ice) = plateau_climate(without_free_air(), shape, height, true);
            assert!(ocean - land > 10.0, "{land} {ocean}");
            assert!(ice > 0.9, "{ice}");
            // Standing in the free troposphere, it cannot be colder than the air at its
            // height, which the sea surface of its latitude sets: the ice melts.
            let (land, ocean, ice) =
                plateau_climate(TemperatureParams::default(), shape, height, true);
            assert!(land > ocean - 2.0, "{land} {ocean}");
            assert!(ice < 0.05, "{ice}");
            // From an ice-free start, the free air changes nothing, or warms: it never cools.
            let local = plateau_climate(without_free_air(), shape, height, false);
            let free = plateau_climate(TemperatureParams::default(), shape, height, false);
            assert!(free.0 >= local.0 - 1e-9, "{free:?} {local:?}");
            assert!(free.2 <= local.2 + 1e-9, "{free:?} {local:?}");
        }
    }

    #[test]
    fn low_ground_keeps_the_temperature_of_its_own_energy_balance() {
        // A tropical continent with a "plateau" of 900 m, inside the boundary layer: the free
        // air changes nothing.
        let shape = ((-25.0, 25.0), (-8.0, 8.0));
        let local = plateau_climate(without_free_air(), shape, 900.0, false);
        let free = plateau_climate(TemperatureParams::default(), shape, 900.0, false);
        assert_eq!(local, free);
    }

    #[test]
    fn polar_plateaus_keep_their_own_climate() {
        // A polar continent with a plateau of 3000 m: a large cold plateau makes its own
        // climate, so it is the same with and without the free air, and it is covered by ice
        // although the spin-up starts without ice.
        let shape = ((60.0, 91.0), (72.0, 91.0));
        let local = plateau_climate(without_free_air(), shape, 3000.0, false);
        let free = plateau_climate(TemperatureParams::default(), shape, 3000.0, false);
        assert_eq!(local, free);
        assert!(free.2 > 0.9, "{}", free.2);
    }

    #[test]
    fn inversions_cool_the_poles() {
        let mesh = SphereMesh::icosphere(4);
        let elevation = vec![-4000.0; mesh.len()];
        let polar = |strength: f64| {
            let params = TemperatureParams {
                inversion_strength: strength,
                ..TemperatureParams::default()
            };
            let mut model = TemperatureModel::new(&mesh, params);
            model.update(&elevation, None, None, 0.0);
            let fields = model.fields();
            let cells: Vec<usize> = (0..mesh.len())
                .filter(|&c| mesh.vertex(c as u32).z.abs() > math::sin(70f64.to_radians()))
                .collect();
            cells.iter().map(|&c| fields.annual_mean[c]).sum::<f64>() / cells.len() as f64
        };
        let with = polar(0.33);
        let without = polar(0.0);
        assert!(with < without - 0.5, "{with} vs {without}");
    }

    #[test]
    fn sea_ice_has_a_seasonal_cycle_and_ocean_carries_heat_poleward() {
        let mesh = SphereMesh::icosphere(4);
        let elevation = vec![-4000.0; mesh.len()];
        let mut model = TemperatureModel::new(&mesh, TemperatureParams::default());
        model.update(&elevation, None, None, 0.0);
        let fields = model.fields();
        let mean_where = |filter: &dyn Fn(f64) -> bool, field: &dyn Fn(usize) -> f64| {
            let cells: Vec<usize> = (0..mesh.len())
                .filter(|&c| filter(math::asin(mesh.vertex(c as u32).z).to_degrees()))
                .collect();
            cells.iter().map(|&c| field(c)).sum::<f64>() / cells.len() as f64
        };
        // Polar ocean: ice all year, cold winters, summers near the melting point.
        let arctic = |l: f64| l > 75.0;
        let january = mean_where(&arctic, &|c| fields.monthly[0][c]);
        let july = mean_where(&arctic, &|c| fields.monthly[6][c]);
        assert!(july - january > 15.0, "January {january}, July {july}");
        assert!(july > -8.0, "July {july}");
        assert!(mean_where(&arctic, &|c| fields.ice[c]) > 0.5);
        assert!(mean_where(&|l: f64| l.abs() < 30.0, &|c| fields.ice[c]) < 0.01);
        // The ocean carries heat poleward in both hemispheres (an ocean planet: up to ~4 PW).
        let ocean = |latitude: f64| {
            fields
                .meridional_transport
                .iter()
                .min_by(|a, b| (a.0 - latitude).abs().total_cmp(&(b.0 - latitude).abs()))
                .map(|t| t.2 * EARTH_RADIUS * EARTH_RADIUS / 1e15)
                .unwrap()
        };
        assert!((0.5..4.0).contains(&ocean(15.0)), "{}", ocean(15.0));
        assert!((-4.0..-0.5).contains(&ocean(-15.0)), "{}", ocean(-15.0));
    }
}

#[cfg(test)]
mod probe {
    use super::*;

    /// Solar-constant sweep on planet snapshots: `SWEEP_CELLS=a.csv,b.csv cargo test --release
    /// solar_sweep -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn solar_sweep() {
        let Ok(files) = std::env::var("SWEEP_CELLS") else {
            return;
        };
        let mesh = SphereMesh::icosphere(6);
        for file in files.split(',') {
            let text = std::fs::read_to_string(file).expect("cell dump");
            let mut lines = text.lines();
            let header: Vec<&str> = lines.next().unwrap().split(',').collect();
            let column = |name: &str| header.iter().position(|&h| h == name).unwrap();
            let (e, p) = (column("elev"), column("precip"));
            let (mut elevation, mut precipitation) = (Vec::new(), Vec::new());
            for line in lines {
                let fields: Vec<&str> = line.split(',').collect();
                elevation.push(fields[e].parse::<f64>().unwrap());
                precipitation.push(fields[p].parse::<f64>().unwrap());
            }
            let mut params = TemperatureParams::default();
            if let Ok(overrides) = std::env::var("SWEEP_SET") {
                for assignment in overrides.split(',') {
                    let (name, value) = assignment.split_once('=').unwrap();
                    params.set(name, value.parse().unwrap()).unwrap();
                }
            }
            let mut model = TemperatureModel::new(&mesh, params);
            let mut line = String::new();
            for percent in [100, 98, 96, 94, 92, 90, 92, 94, 96, 98, 100] {
                model.params.orbit.solar_constant = 1361.0 * percent as f64 / 100.0;
                model.spun_up = false;
                model.params.spin_up_years = 40;
                model.update(&elevation, Some(&precipitation), None, 0.0);
                let f = model.fields();
                let global = f.annual_mean.iter().sum::<f64>() / f.annual_mean.len() as f64;
                let ice = f.ice.iter().filter(|&&i| i > 0.5).count() as f64 / f.ice.len() as f64;
                line += &format!(" {percent}%: {global:.1}°C/{:.0}%", 100.0 * ice);
            }
            eprintln!("SWEEP {file}:{line}");
        }
    }
}
