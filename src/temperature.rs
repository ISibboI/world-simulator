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
//! capacity `C` of the ocean mixed layer or of land. Permanent ice (sea ice, ice sheets) forms
//! where the annual mean temperature of the previous year is low (Budyko's criterion), which
//! avoids the runaway cooling that seasonal snow albedo causes in energy balance models. Most of the transport acts on the means of
//! latitude bands (the one-dimensional model of North, 1975), and only a weak part acts as
//! two-dimensional diffusion between neighbouring cells. Applying the full transport
//! two-dimensionally would tie the temperature of land to the surrounding ocean over thousands of
//! kilometres within a season, suppressing continental climates. Snow and sea ice raise the albedo `α`
//! (ice-albedo feedback). Land has a much smaller heat capacity than the ocean, so continental
//! interiors get hot summers and cold winters.
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
    /// Coefficient of meridional heat transport between latitude bands, for the Laplacian on the
    /// unit sphere, in W/m²/K.
    pub heat_transport: f64,
    /// Coefficient of local two-dimensional heat diffusion, in W/m²/K.
    pub local_heat_transport: f64,
    /// Number of equal-area latitude bands for the meridional heat transport.
    pub latitude_bands: usize,
    /// Planetary albedo (including clouds) over ice-free ocean.
    pub ocean_albedo: f64,
    /// Planetary albedo (including clouds) over snow-free land.
    pub land_albedo: f64,
    /// Planetary albedo over snow and ice.
    pub ice_albedo: f64,
    /// Permanent sea ice forms below this annual mean sea surface temperature; the albedo changes
    /// linearly over ±5 K around it.
    pub sea_ice_temperature: f64,
    /// Ice sheets form below this annual mean land surface temperature; the albedo changes
    /// linearly over ±5 K around it.
    pub land_ice_temperature: f64,
    /// Heat capacity of the ocean mixed layer in J/m²/K.
    pub ocean_heat_capacity: f64,
    /// Effective heat capacity of land in J/m²/K.
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
            heat_transport: 0.45,
            local_heat_transport: 0.05,
            latitude_bands: 18,
            ocean_albedo: 0.29,
            land_albedo: 0.32,
            ice_albedo: 0.62,
            sea_ice_temperature: -10.0,
            land_ice_temperature: -10.0,
            ocean_heat_capacity: 4.18e6 * 50.0,
            land_heat_capacity: 8e6,
            lapse_rate: 0.0065,
            seasons_per_year: 24,
            substeps_per_season: 8,
            spin_up_years: 20,
            years_per_update: 2,
            coarse_subdivisions: 4,
        }
    }
}

const SECONDS_PER_YEAR: f64 = 365.25 * 24.0 * 3600.0;

/// Temperature fields on the fine mesh, in °C.
#[derive(Clone, Debug, Default)]
pub struct TemperatureFields {
    /// Annual mean surface temperature (at the land surface or the sea surface).
    pub annual_mean: Vec<f64>,
    /// Mean surface temperature of each month; month 0 starts at perihelion (early January).
    pub monthly: Vec<Vec<f64>>,
    /// Annual mean temperature reduced to sea level, i.e. without the lapse rate correction.
    pub sea_level_annual_mean: Vec<f64>,
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
    /// Sea-level temperature of each coarse cell.
    temperature: Vec<f64>,
    /// Annual mean sea-level temperature of each coarse cell in the previous year, which
    /// determines ice cover.
    previous_annual_mean: Vec<f64>,
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

        let temperature = vec![15.0; coarse.len()];
        let previous_annual_mean = temperature.clone();
        Self {
            params,
            coarse,
            coarse_latitude,
            band,
            fine_to_coarse,
            interpolation,
            temperature,
            previous_annual_mean,
            spun_up: false,
            fields: TemperatureFields::default(),
        }
    }

    pub fn fields(&self) -> &TemperatureFields {
        &self.fields
    }

    /// Simulates the seasonal cycle for the given surface elevation (in metres, sea level is 0)
    /// of the fine mesh, and updates the temperature fields.
    pub fn update(&mut self, elevation: &[f64]) {
        let params = &self.params;
        let coarse_count = self.coarse.len();

        // Land fraction and mean land elevation of every coarse cell.
        let mut cells = vec![0usize; coarse_count];
        let mut land_cells = vec![0usize; coarse_count];
        let mut land_elevation = vec![0.0; coarse_count];
        for (fine, &coarse) in self.fine_to_coarse.iter().enumerate() {
            cells[coarse as usize] += 1;
            if elevation[fine] >= 0.0 {
                land_cells[coarse as usize] += 1;
                land_elevation[coarse as usize] += elevation[fine];
            }
        }
        let land_fraction: Vec<f64> = (0..coarse_count)
            .map(|c| land_cells[c] as f64 / cells[c].max(1) as f64)
            .collect();
        for c in 0..coarse_count {
            land_elevation[c] /= land_cells[c].max(1) as f64;
        }
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

        let dt = SECONDS_PER_YEAR / (seasons * params.substeps_per_season) as f64;
        let mut seasonal = vec![vec![0.0; coarse_count]; seasons];
        let mut next = vec![0.0; coarse_count];
        let mut annual_sum = vec![0.0; coarse_count];
        for year in 0..years {
            let last_year = year + 1 == years;
            // Planetary albedo from the ice cover, which depends on last year's temperatures.
            let albedo: Vec<f64> = (0..coarse_count)
                .map(|cell| {
                    let t = self.previous_annual_mean[cell];
                    let land_t = t - params.lapse_rate * land_elevation[cell];
                    let f = land_fraction[cell];
                    f * blend(
                        params.land_albedo,
                        params.ice_albedo,
                        land_t,
                        params.land_ice_temperature,
                    ) + (1.0 - f)
                        * blend(
                            params.ocean_albedo,
                            params.ice_albedo,
                            t,
                            params.sea_ice_temperature,
                        )
                })
                .collect();
            annual_sum.fill(0.0);
            for (season, insolation) in insolation.iter().enumerate() {
                for _ in 0..params.substeps_per_season {
                    // Meridional transport between latitude bands, solved implicitly.
                    band_temperature.fill(0.0);
                    for cell in 0..coarse_count {
                        band_temperature[self.band[cell]] += self.temperature[cell];
                    }
                    for band in 0..bands {
                        band_temperature[band] /= band_cells[band].max(1) as f64;
                    }
                    meridional_transport(
                        &band_temperature,
                        &band_heat_capacity,
                        params.heat_transport,
                        dt,
                        &mut band_flux,
                    );

                    for cell in 0..coarse_count {
                        let t = self.temperature[cell];
                        let albedo = albedo[cell];

                        // Implicit in the cell's own temperature, explicit in its neighbours;
                        // stable for any time step.
                        let neighbors = self.coarse.neighbors(cell as u32);
                        let neighbor_sum: f64 = neighbors
                            .iter()
                            .map(|&n| self.temperature[n as usize])
                            .sum();
                        let rate = dt / heat_capacity[cell];
                        next[cell] = (t + rate
                            * (insolation[cell] * (1.0 - albedo) - params.radiation_a
                                + band_flux[self.band[cell]]
                                + coupling * neighbor_sum))
                            / (1.0
                                + rate * (params.radiation_b + coupling * neighbors.len() as f64));
                    }
                    std::mem::swap(&mut self.temperature, &mut next);
                    for (sum, &t) in annual_sum.iter_mut().zip(&self.temperature) {
                        *sum += t;
                    }
                    if last_year {
                        for (sum, &t) in seasonal[season].iter_mut().zip(&self.temperature) {
                            *sum += t / params.substeps_per_season as f64;
                        }
                    }
                }
            }
            let steps_per_year = (seasons * params.substeps_per_season) as f64;
            for (previous, &sum) in self.previous_annual_mean.iter_mut().zip(&annual_sum) {
                *previous = sum / steps_per_year;
            }
        }

        // Monthly means of the sea-level temperature on the coarse mesh.
        let seasons_per_month = seasons / 12;
        let monthly_coarse: Vec<Vec<f64>> = seasonal
            .chunks(seasons_per_month)
            .map(|chunk| {
                (0..coarse_count)
                    .map(|cell| {
                        chunk.iter().map(|season| season[cell]).sum::<f64>()
                            / seasons_per_month as f64
                    })
                    .collect()
            })
            .collect();

        let lapse =
            |fine: usize, sea_level: f64| sea_level - params.lapse_rate * elevation[fine].max(0.0);
        let monthly: Vec<Vec<f64>> = monthly_coarse
            .iter()
            .map(|month| {
                (0..elevation.len())
                    .map(|fine| lapse(fine, self.interpolate(month, fine)))
                    .collect()
            })
            .collect();
        let sea_level_annual_mean: Vec<f64> = (0..elevation.len())
            .map(|fine| {
                monthly_coarse
                    .iter()
                    .map(|month| self.interpolate(month, fine))
                    .sum::<f64>()
                    / 12.0
            })
            .collect();
        let annual_mean = sea_level_annual_mean
            .iter()
            .enumerate()
            .map(|(fine, &t)| lapse(fine, t))
            .collect();

        self.fields = TemperatureFields {
            annual_mean,
            monthly,
            sea_level_annual_mean,
        };
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
fn meridional_transport(
    temperature: &[f64],
    heat_capacity: &[f64],
    diffusion: f64,
    dt: f64,
    flux: &mut [f64],
) {
    let n = temperature.len();
    let width = 2.0 / n as f64;
    // Coupling across the interface above band `b`, which lies at x = -1 + (b + 1) * width.
    let interface = |b: usize| {
        let x = -1.0 + (b + 1) as f64 * width;
        diffusion * (1.0 - x * x) / (width * width)
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
        model.update(&elevation);
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
}
