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
//! capacity `C` of the ocean mixed layer or of land. Permanent sea ice forms where the annual
//! mean temperature of the previous year is low (Budyko's criterion), which avoids the runaway
//! cooling that seasonal snow albedo causes in energy balance models; ice sheets also need
//! snowfall. Most of the transport acts on the means of
//! latitude bands (the one-dimensional model of North, 1975), and only a weak part acts as
//! two-dimensional diffusion between neighbouring cells. Applying the full transport
//! two-dimensionally would tie the temperature of land to the surrounding ocean over thousands of
//! kilometres within a season, suppressing continental climates. Snow and sea ice raise the albedo `α`
//! (ice-albedo feedback). Land has a much smaller heat capacity than the ocean (the air column,
//! about 1e7 J/m²/K), so continental
//! interiors get hot summers and cold winters.
//!
//! Over cold surfaces, the air that radiates to space lies above a surface inversion and stays
//! warmer than the surface, which the transport of heat from lower latitudes maintains. The
//! linear law `A + B T`, fitted to Earth's mid and low latitudes, would underestimate the outgoing
//! radiation there (by about 10 W/m² over the Arctic Ocean, 20 W/m² over Antarctica). Below the
//! inversion temperature `T_i`, the emitting air is taken to be warmer than the surface by
//! `k (T_i − T)`, so the outgoing radiation is `A + B (T + k (T_i − T))`. This is off by default
//! (`k = 0`): with k ≈ 1/3, as fitted to the observed outgoing radiation, the weaker radiative
//! damping of cold climates lets the ice-albedo feedback run away into a snowball for some
//! continental configurations. Sea-ice thermodynamics (thin, insulating ice that holds the summer
//! surface at the melting point) would be needed first.
//!
//! The model evolves the sea-level temperature on a coarse icosphere, since temperature varies
//! smoothly. Surface temperatures on the fine mesh are interpolated and lowered with elevation
//! according to the lapse rate.

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
            eccentric_anomaly -= (eccentric_anomaly - e * eccentric_anomaly.sin() - mean_anomaly)
                / (1.0 - e * eccentric_anomaly.cos());
        }
        let true_anomaly = 2.0
            * ((1.0 + e).sqrt() * (eccentric_anomaly / 2.0).sin())
                .atan2((1.0 - e).sqrt() * (eccentric_anomaly / 2.0).cos());
        let solar_longitude = true_anomaly + self.perihelion_longitude.to_radians();
        OrbitalPosition {
            declination: (self.obliquity.to_radians().sin() * solar_longitude.sin()).asin(),
            distance: 1.0 - e * eccentric_anomaly.cos(),
        }
    }

    /// Daily mean insolation at the top of the atmosphere in W/m² at `latitude` (radians).
    pub fn daily_insolation(&self, latitude: f64, position: OrbitalPosition) -> f64 {
        let declination = position.declination;
        // Hour angle of sunset; 0 during polar night, π during polar day.
        let cos_sunset = -latitude.tan() * declination.tan();
        let sunset = if cos_sunset >= 1.0 {
            0.0
        } else if cos_sunset <= -1.0 {
            PI
        } else {
            cos_sunset.acos()
        };
        self.solar_constant / PI / position.distance.powi(2)
            * (sunset * latitude.sin() * declination.sin()
                + latitude.cos() * declination.cos() * sunset.sin())
    }
}

/// Parameters of the energy balance model. Temperatures in °C.
#[derive(Clone, Debug)]
pub struct TemperatureParams {
    pub orbit: OrbitParams,
    /// Outgoing longwave radiation at 0 °C in W/m².
    pub radiation_a: f64,
    /// Increase of outgoing longwave radiation per kelvin in W/m²/K.
    pub radiation_b: f64,
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
    /// temperature of the water subducted at this latitude (degrees).
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
}

impl Default for TemperatureParams {
    fn default() -> Self {
        Self {
            orbit: OrbitParams::default(),
            radiation_a: 203.3,
            radiation_b: 2.09,
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
            seasons_per_year: 24,
            substeps_per_season: 8,
            spin_up_years: 20,
            years_per_update: 2,
            coarse_subdivisions: 4,
        }
    }
}

impl TemperatureParams {
    /// Sets a parameter by its field name, for calibration from the command line.
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        let field = match name {
            "radiation_a" => &mut self.radiation_a,
            "radiation_b" => &mut self.radiation_b,
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
            "solar_constant" => &mut self.orbit.solar_constant,
            _ => anyhow::bail!("unknown temperature parameter {name}"),
        };
        *field = value;
        Ok(())
    }
}

const SECONDS_PER_YEAR: f64 = 365.25 * 24.0 * 3600.0;
/// Number of longitude sectors of the thermal equator.
pub const THERMAL_EQUATOR_SECTORS: usize = 24;
/// The model assumes an Earth-sized planet for the ocean heat transport.
const EARTH_RADIUS: f64 = 6.371e6;
/// Angular velocity of the rotation in rad/s.
const EARTH_ROTATION: f64 = 7.292e-5;
/// Specific heat capacity of sea water in J/kg/K.
const WATER_HEAT_CAPACITY: f64 = 3990.0;

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
    /// Permanent ice cover (sea ice, ice sheets), 0 to 1, as the albedo of the last year saw it.
    pub ice: Vec<f64>,
    /// Annual mean northward heat transport across the band interfaces in the last year: latitude
    /// (degrees), and the transport by the atmosphere and by the ocean in W per square metre of
    /// the planet's radius squared (multiply by R² for watts).
    pub meridional_transport: Vec<(f64, f64, f64)>,
    /// Largest volume transport of the wind-driven gyres in Sv (1e6 m³/s).
    pub gyre_transport: f64,
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

pub struct TemperatureModel {
    params: TemperatureParams,
    coarse: SphereMesh,
    coarse_latitude: Vec<f64>,
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
    /// Annual mean land temperature of each coarse cell in the previous year, which determines
    /// the ice sheets.
    previous_annual_land: Vec<f64>,
    /// Heat delivered to the ocean mixed layer by ocean currents, per coarse cell, in W/m² of
    /// ocean.
    ocean_heat_convergence: Vec<f64>,
    gyre_model: crate::ocean::GyreModel,
    spun_up: bool,
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
            .map(|cell| coarse.vertex(cell).z.clamp(-1.0, 1.0).asin())
            .collect();
        assert!(
            params.latitude_bands >= 2,
            "at least two latitude bands are needed"
        );
        // Bands of equal width in sin(latitude) have equal area.
        let band = coarse_latitude
            .iter()
            .map(|latitude| {
                let x = (latitude.sin() + 1.0) / 2.0;
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
                let distance = coarse.vertex(candidate).angle_between(position);
                *slot = (candidate, (1.0 - distance / radius).max(0.0).powi(2));
            }
            let total: f64 = weights.iter().map(|(_, w)| w).sum();
            for (_, weight) in &mut weights {
                *weight /= total;
            }
            interpolation.push(weights);
        }

        let count = coarse.len();
        let gyre_model = crate::ocean::GyreModel::new(&coarse, EARTH_RADIUS);
        let ocean_enthalpy = params.ocean_heat_capacity * (15.0 - params.freezing_temperature);
        Self {
            params,
            coarse,
            coarse_latitude,
            band,
            fine_to_coarse,
            interpolation,
            land: vec![15.0; count],
            ocean: vec![ocean_enthalpy; count],
            ocean_surface: vec![15.0; count],
            previous_annual_land: vec![15.0; count],
            ocean_heat_convergence: vec![0.0; count],
            gyre_model,
            spun_up: false,
            fields: TemperatureFields::default(),
        }
    }

    pub fn fields(&self) -> &TemperatureFields {
        &self.fields
    }

    /// Simulates the seasonal cycle for the given surface elevation (in metres, sea level is 0)
    /// of the fine mesh, and updates the temperature fields. `precipitation` (m/yr) limits where
    /// ice sheets can grow; without it, 1 m/yr is assumed everywhere.
    pub fn update(&mut self, elevation: &[f64], precipitation: Option<&[f64]>) {
        let params = &self.params;
        let coarse_count = self.coarse.len();

        // Land fraction of every coarse cell.
        let mut cells = vec![0usize; coarse_count];
        let mut land_cells = vec![0usize; coarse_count];
        for (fine, &coarse) in self.fine_to_coarse.iter().enumerate() {
            cells[coarse as usize] += 1;
            if elevation[fine] >= 0.0 {
                land_cells[coarse as usize] += 1;
            }
        }
        let land_cells_list: Vec<usize> = (0..elevation.len())
            .filter(|&fine| elevation[fine] >= 0.0)
            .collect();
        let land_fraction: Vec<f64> = (0..coarse_count)
            .map(|c| land_cells[c] as f64 / cells[c].max(1) as f64)
            .collect();
        // Heat capacities for the implicit zonal transport: the column under sea ice still holds
        // the mixed layer, so the ocean's is used throughout (it only sets the stability of the
        // implicit step).
        let heat_capacity: Vec<f64> = land_fraction
            .iter()
            .map(|&f| f * params.land_heat_capacity + (1.0 - f) * params.ocean_heat_capacity)
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
                let position = params
                    .orbit
                    .position((season as f64 + 0.5) / seasons as f64);
                self.coarse_latitude
                    .iter()
                    .map(|&latitude| params.orbit.daily_insolation(latitude, position))
                    .collect()
            })
            .collect();

        let years = if self.spun_up {
            params.years_per_update
        } else {
            params.spin_up_years
        };
        self.spun_up = true;

        // Outgoing longwave radiation `A + B (T + inversion)`, linear in T on either side of the
        // inversion temperature: (A, B) for a surface at `t`.
        let radiation = |t: f64| {
            if t < params.inversion_temperature {
                let k = params.inversion_strength;
                (
                    params.radiation_a + params.radiation_b * k * params.inversion_temperature,
                    params.radiation_b * (1.0 - k),
                )
            } else {
                (params.radiation_a, params.radiation_b)
            }
        };
        let ice_thickness = |enthalpy: f64| (-enthalpy / params.ice_latent_heat).max(0.0);
        let ocean_albedo: Vec<f64> = self
            .coarse_latitude
            .iter()
            .map(|latitude| {
                let x = latitude.sin();
                params.ocean_albedo + params.ocean_albedo_latitude * (3.0 * x * x - 1.0) / 2.0
            })
            .collect();

        // Wind-driven ocean heat transport by the subtropical cells across every band interface:
        // the poleward Ekman mass transport under the trade winds, M = −τ_x f / (f² + r²) per
        // metre (kg/m/s, northward), times the width of the ocean.
        let mut band_ocean = vec![0.0; bands];
        for cell in 0..coarse_count {
            band_ocean[self.band[cell]] += 1.0 - land_fraction[cell];
        }
        for band in 0..bands {
            band_ocean[band] /= band_cells[band].max(1) as f64;
        }
        let ekman: Vec<f64> = (0..bands - 1)
            .map(|b| {
                let x = -1.0 + (b + 1) as f64 * 2.0 / bands as f64;
                let latitude = x.asin();
                let stress = crate::climate::surface_wind_stress(latitude.to_degrees(), 4.5);
                if stress >= 0.0 || latitude.to_degrees().abs() > params.subduction_latitude {
                    return 0.0;
                }
                let f = 2.0 * EARTH_ROTATION * latitude.sin();
                let mass = -stress * f / (f * f + params.equatorial_friction.powi(2));
                let width = 2.0
                    * PI
                    * EARTH_RADIUS
                    * latitude.cos()
                    * 0.5
                    * (band_ocean[b] + band_ocean[b + 1]);
                mass * width
            })
            .collect();
        // The bands holding the subduction latitude in the south and the north.
        let subduction_band = |sign: f64| {
            let x = (sign * params.subduction_latitude.to_radians()).sin();
            (((x + 1.0) / 2.0 * bands as f64) as usize).min(bands - 1)
        };
        let subduction = [subduction_band(-1.0), subduction_band(1.0)];
        let band_area = 4.0 * PI * EARTH_RADIUS * EARTH_RADIUS / bands as f64;
        // Wind-driven gyres in the cells that are mostly ocean.
        let is_ocean: Vec<bool> = land_fraction.iter().map(|&f| f < 0.5).collect();
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
        let mut annual_land = vec![0.0; coarse_count];
        let mut annual_sea_ice = vec![0.0; coarse_count];
        let mut mean = vec![0.0; coarse_count];
        for _ in 0..years {
            for season in seasonal_land
                .iter_mut()
                .chain(&mut seasonal_ocean)
                .chain(&mut seasonal_mean)
            {
                season.fill(0.0);
            }
            transport_sum.fill(0.0);
            ocean_transport_sum.fill(0.0);
            annual_land.fill(0.0);
            annual_sea_ice.fill(0.0);
            // Albedo of the land from the ice sheets, which depend on last year's temperatures:
            // the mean over the fine land cells of every coarse cell, each with its own elevation
            // and snowfall.
            let mut land_ice = vec![0.0; coarse_count];
            for &fine in &land_cells_list {
                let t = self.interpolate(&self.previous_annual_land, fine)
                    - params.lapse_rate * elevation[fine];
                let p = precipitation.map_or(f64::INFINITY, |p| p[fine]);
                land_ice[self.fine_to_coarse[fine] as usize] += ice_sheet_cover(t, p, params);
            }
            let land_albedo: Vec<f64> = (0..coarse_count)
                .map(|cell| {
                    let ice = land_ice[cell] / land_cells[cell].max(1) as f64;
                    params.land_albedo + (params.ice_albedo - params.land_albedo) * ice
                })
                .collect();
            for (season, insolation) in insolation.iter().enumerate() {
                for _ in 0..params.substeps_per_season {
                    // The air above every cell: the area mean of its land and ocean surface.
                    for cell in 0..coarse_count {
                        let f = land_fraction[cell];
                        mean[cell] = f * self.land[cell] + (1.0 - f) * self.ocean_surface[cell];
                    }
                    // Meridional transport between latitude bands, solved implicitly.
                    // Ocean heat transport of the subtropical cells.
                    band_sst.fill(0.0);
                    for cell in 0..coarse_count {
                        band_sst[self.band[cell]] +=
                            (1.0 - land_fraction[cell]) * self.ocean_surface[cell];
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
                            * (surface - band_sst[subduction[hemisphere]]);
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
                        let ocean_area = (1.0 - land_fraction[cell]) * cell_area;
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

                        // Land: C dT/dt = Q (1 − α) − OLR + transport + exchange with the ocean.
                        let land = self.land[cell];
                        let (a, b) = radiation(land);
                        let exchange = params.land_ocean_exchange * (1.0 - f);
                        let rate = dt / params.land_heat_capacity;
                        self.land[cell] = (land
                            + rate
                                * (q * (1.0 - land_albedo[cell]) - a
                                    + transport
                                    + exchange * self.ocean_surface[cell]))
                            / (1.0 + rate * (b + local + exchange));

                        // Ocean column (Wagner & Eisenman, 2015).
                        let exchange = params.land_ocean_exchange * f;
                        let enthalpy = self.ocean[cell];
                        let h = ice_thickness(enthalpy);
                        let ice_cover = (h / params.ice_albedo_thickness).min(1.0);
                        let ice_albedo = if self.ocean_surface[cell] >= 0.0 {
                            params.melting_ice_albedo
                        } else {
                            params.sea_ice_albedo
                        };
                        let albedo =
                            ocean_albedo[cell] + (ice_albedo - ocean_albedo[cell]) * ice_cover;
                        let (a, b) = radiation(self.ocean_surface[cell]);
                        // Net heating of the surface: `forcing − damping · T_surface`.
                        let forcing = q * (1.0 - albedo) - a
                            + transport
                            + exchange * self.land[cell]
                            + self.ocean_heat_convergence[cell];
                        let damping = b + local + exchange;
                        let (surface, enthalpy) = if enthalpy >= 0.0 {
                            // Open water: the mixed layer, implicit in its temperature.
                            let c = params.ocean_heat_capacity;
                            let t = params.freezing_temperature + enthalpy / c;
                            let t = (t + dt / c * forcing) / (1.0 + dt / c * damping);
                            // Cooling below the freezing point forms ice instead.
                            let next = c * (t - params.freezing_temperature);
                            (t.max(params.freezing_temperature), next)
                        } else {
                            // Sea ice: the surface temperature balances conduction through the
                            // ice with the surface fluxes; it cannot exceed melting (0 °C).
                            let conductance = params.ice_conductivity / h.max(0.01);
                            let t = ((conductance * params.freezing_temperature + forcing)
                                / (conductance + damping))
                                .min(0.0);
                            let enthalpy = enthalpy + dt * (forcing - damping * t);
                            // Thick ice drifts away and melts elsewhere.
                            let limit = -params.max_ice_thickness * params.ice_latent_heat;
                            (t, enthalpy.max(limit))
                        };
                        self.ocean_surface[cell] = surface;
                        self.ocean[cell] = enthalpy;
                    }

                    for cell in 0..coarse_count {
                        let f = land_fraction[cell];
                        let surface = self.ocean_surface[cell];
                        seasonal_land[season][cell] += self.land[cell] / substeps;
                        seasonal_ocean[season][cell] += surface / substeps;
                        seasonal_mean[season][cell] +=
                            (f * self.land[cell] + (1.0 - f) * surface) / substeps;
                        annual_land[cell] += self.land[cell];
                        annual_sea_ice[cell] += (ice_thickness(self.ocean[cell])
                            / params.ice_albedo_thickness)
                            .min(1.0);
                    }
                }
            }
            let steps_per_year = (seasons * params.substeps_per_season) as f64;
            for (previous, &sum) in self.previous_annual_land.iter_mut().zip(&annual_land) {
                *previous = sum / steps_per_year;
            }
            for sea_ice in &mut annual_sea_ice {
                *sea_ice /= steps_per_year;
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

        let lapse =
            |fine: usize, sea_level: f64| sea_level - params.lapse_rate * elevation[fine].max(0.0);
        // Every fine cell takes the temperature of its own surface type.
        let surface = |fine: usize, month: usize| {
            if elevation[fine] >= 0.0 {
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

        // Ice cover of the fine cells: sea ice (annual mean cover), ice sheets by temperature and
        // snowfall.
        let ice = (0..elevation.len())
            .map(|fine| {
                if elevation[fine] < 0.0 {
                    self.interpolate(&annual_sea_ice, fine)
                } else {
                    let t = lapse(fine, self.interpolate(&self.previous_annual_land, fine));
                    let p = precipitation.map_or(f64::INFINITY, |p| p[fine]);
                    ice_sheet_cover(t, p, params)
                }
            })
            .collect();

        let thermal_equator = monthly_coarse
            .iter()
            .map(|month| {
                (0..THERMAL_EQUATOR_SECTORS)
                    .map(|sector| {
                        let centre =
                            (sector as f64 + 0.5) / THERMAL_EQUATOR_SECTORS as f64 * TAU - PI;
                        self.thermal_equator(month, centre)
                    })
                    .collect()
            })
            .collect();

        self.fields = TemperatureFields {
            annual_mean,
            monthly,
            thermal_equator,
            ice,
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
                        x.asin().to_degrees(),
                        transport_sum[b] / steps * 4.0 * PI / bands as f64,
                        ocean_transport_sum[b] / steps / (EARTH_RADIUS * EARTH_RADIUS),
                    )
                })
                .collect(),
        };
    }

    /// The latitude (degrees) between 30°S and 30°N where the mean of a coarse field over the
    /// longitudes around `longitude` (radians; weighted with a Gaussian of 30° width), smoothed
    /// with a Gaussian of 5° width in latitude, is largest.
    fn thermal_equator(&self, field: &[f64], longitude: f64) -> f64 {
        // Means in bins of 0.5° between 60°S and 60°N, then smoothed.
        const BINS: usize = 240;
        let mut sum = [0.0; BINS];
        let mut count = [0.0; BINS];
        for (cell, (&latitude, &value)) in self.coarse_latitude.iter().zip(field).enumerate() {
            let position = (latitude.to_degrees() + 60.0) * 2.0;
            if (0.0..BINS as f64).contains(&position) {
                let p = self.coarse.vertex(cell as u32);
                let difference = (p.y.atan2(p.x) - longitude + PI).rem_euclid(TAU) - PI;
                let weight = (-(difference / 30f64.to_radians()).powi(2)).exp();
                sum[position as usize] += weight * value;
                count[position as usize] += weight;
            }
        }
        let smoothed = |bin: usize| {
            let (mut s, mut c) = (0.0, 0.0);
            for other in bin.saturating_sub(30)..(bin + 31).min(BINS) {
                let distance = (other as f64 - bin as f64) / 10.0;
                let weight = (-0.5 * distance * distance).exp();
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
                sum += orbit.daily_insolation(latitude, position) * latitude.cos();
                weight += latitude.cos();
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
                let latitude = p.z.asin().to_degrees();
                let longitude = p.y.atan2(p.x).to_degrees();
                if (30.0..70.0).contains(&latitude) && longitude.abs() < 60.0 {
                    300.0
                } else {
                    -4000.0
                }
            })
            .collect();
        let mut model = TemperatureModel::new(&mesh, TemperatureParams::default());
        model.update(&elevation, None);
        let fields = model.fields();

        let mean_where = |filter: &dyn Fn(f64, f64) -> bool, field: &dyn Fn(usize) -> f64| {
            let cells: Vec<usize> = (0..mesh.len())
                .filter(|&c| {
                    let p = mesh.vertex(c as u32);
                    filter(p.z.asin().to_degrees(), p.y.atan2(p.x).to_degrees())
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
            model.update(&elevation, None);
            let fields = model.fields();
            let cells: Vec<usize> = (0..mesh.len())
                .filter(|&c| mesh.vertex(c as u32).z.abs() > 70f64.to_radians().sin())
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
        model.update(&elevation, None);
        let fields = model.fields();
        let mean_where = |filter: &dyn Fn(f64) -> bool, field: &dyn Fn(usize) -> f64| {
            let cells: Vec<usize> = (0..mesh.len())
                .filter(|&c| filter(mesh.vertex(c as u32).z.asin().to_degrees()))
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
        // The ocean carries heat poleward in both hemispheres, about 0.5–3 PW at 10–20°.
        let ocean = |latitude: f64| {
            fields
                .meridional_transport
                .iter()
                .min_by(|a, b| (a.0 - latitude).abs().total_cmp(&(b.0 - latitude).abs()))
                .map(|t| t.2 * EARTH_RADIUS * EARTH_RADIUS / 1e15)
                .unwrap()
        };
        assert!((0.5..3.0).contains(&ocean(15.0)), "{}", ocean(15.0));
        assert!((-3.0..-0.5).contains(&ocean(-15.0)), "{}", ocean(-15.0));
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
                model.update(&elevation, Some(&precipitation));
                let f = model.fields();
                let global = f.annual_mean.iter().sum::<f64>() / f.annual_mean.len() as f64;
                let ice = f.ice.iter().filter(|&&i| i > 0.5).count() as f64 / f.ice.len() as f64;
                line += &format!(" {percent}%: {global:.1}°C/{:.0}%", 100.0 * ice);
            }
            eprintln!("SWEEP {file}:{line}");
        }
    }

    #[test]
    #[ignore]
    fn probe_aquaplanet() {
        let mesh = SphereMesh::icosphere(4);
        let elevation = vec![-4000.0; mesh.len()];
        let params = TemperatureParams {
            spin_up_years: 1,
            ..TemperatureParams::default()
        };
        let mut model = TemperatureModel::new(&mesh, params);
        for year in 0..30 {
            model.update(&elevation, None);
            let f = model.fields();
            let lat = |c: usize| mesh.vertex(c as u32).z.asin().to_degrees().abs();
            let band = |a: f64, b: f64, field: &dyn Fn(usize) -> f64| {
                let cells: Vec<usize> = (0..mesh.len())
                    .filter(|&c| (a..b).contains(&lat(c)))
                    .collect();
                cells.iter().map(|&c| field(c)).sum::<f64>() / cells.len() as f64
            };
            eprintln!(
                "year {year}: T 0-10 {:.1} 30-40 {:.1} 50-60 {:.1} 70-90 {:.1} | ice 50-60 {:.2} 70-90 {:.2} | Jan/Jul 70-90 {:.1}/{:.1}",
                band(0.0, 10.0, &|c| f.annual_mean[c]),
                band(30.0, 40.0, &|c| f.annual_mean[c]),
                band(50.0, 60.0, &|c| f.annual_mean[c]),
                band(70.0, 90.1, &|c| f.annual_mean[c]),
                band(50.0, 60.0, &|c| f.ice[c]),
                band(70.0, 90.1, &|c| f.ice[c]),
                band(70.0, 90.1, &|c| f.monthly[0][c]),
                band(70.0, 90.1, &|c| f.monthly[6][c]),
            );
        }
    }
}
