//! The water cycle: a steady-state budget of the water vapour in the air column.
//!
//! The column water vapour `W` (kg/m²) of every cell obeys
//!
//! ```text
//! 0 = −∇·(u W) + ∇·((p/g) K ∇q) + E − P
//! ```
//!
//! with advection by the low-level winds `u`, transport by weather systems (eddies) as diffusion
//! of the specific humidity `q` with diffusivity `K` over the mass `p/g` of the air column,
//! evaporation `E` and precipitation `P`. The budget is discretised with finite volumes on the
//! Voronoi cells of the icosphere: every flux across an edge leaves one cell and enters its
//! neighbour, so the precipitation over the planet equals the evaporation, and the moisture
//! evaporated over the ocean travels with the wind (fetch) until it rains out.
//!
//! - **Winds** follow Earth's zonal-mean low-level circulation: easterly trade winds that converge
//!   in the intertropical convergence zone (ITCZ), westerlies in mid-latitudes and weak polar
//!   easterlies. The pattern moves with the seasons, following the thermal equator of the energy
//!   balance model (the ITCZ lies near the energy-flux equator, which in a diffusive energy
//!   balance model is the temperature maximum). In the tropics the mean Hadley circulation
//!   carries the moisture and eddy diffusion fades out (as in the moist energy balance model of
//!   Siler et al., 2018).
//! - **Evaporation** over the ocean follows the bulk aerodynamic formula
//!   `E = ρ C_E |U| (q_sat(SST) − RH q_sat(SST − ΔT))`, with the speed of the mean wind and of
//!   the fluctuations of weather systems, and surface air slightly colder than the sea. Over land,
//!   the actual evapotranspiration follows the Budyko curve in Fu's form (Fu, 1981; Zhang et al.,
//!   2004) from the annual precipitation and potential evaporation (the same bulk formula at the
//!   land surface temperature, with the humidity of the air above). Evapotranspiration returns to
//!   the air column (moisture recycling); the rest of the precipitation runs off.
//! - **Precipitation** grows steeply with the column relative humidity `r = W / W_sat`, as
//!   observed by Bretherton et al. (2004): `P = W / τ · exp(a (r − r₀))`. The saturation column
//!   `W_sat = ρ_vs(T) H_v(T) exp(−z / H_v)` depends on the temperature of the free troposphere
//!   (the zonal mean of the sea-level temperature, since the free troposphere is much more uniform
//!   than the surface) through the saturation vapour density and the scale height `H_v` of water
//!   vapour, and on the surface elevation `z`. Rain therefore falls where moist air is carried
//!   into colder air (the mid-latitude storm tracks), where air converges (ITCZ) and where air is
//!   forced up over high ground, whose shorter, colder air column holds less vapour (orographic
//!   rain). Behind mountains and in the diverging air of the subtropics the air is dry.
//!
//! The budget is solved for several months of the year, with that month's temperatures, and
//! averaged to annual precipitation. Newton's method solves the nonlinear budget, with BiCGSTAB
//! and a multigrid preconditioner (see [`crate::sparse`]) for the linear systems. After the first
//! update, one month is refreshed per update in turn, starting from its previous state.

use glam::DVec3;

use crate::sparse::{Hierarchy, Multigrid};
use crate::sphere::SphereMesh;

const SECONDS_PER_YEAR: f64 = 365.25 * 24.0 * 3600.0;
/// Density of liquid water in kg/m³, to convert water fluxes into m/yr.
const WATER_DENSITY: f64 = 1000.0;
/// Gas constant of water vapour in J/kg/K.
const VAPOUR_GAS_CONSTANT: f64 = 461.5;
/// Latent heat of vaporisation in J/kg.
const LATENT_HEAT: f64 = 2.5e6;
/// Surface air pressure in Pa.
const SURFACE_PRESSURE: f64 = 101_325.0;

/// Angular velocity of the planet's rotation in rad/s.
const EARTH_ROTATION: f64 = 7.292e-5;
/// Gravitational acceleration in m/s².
const GRAVITY: f64 = 9.81;

/// Parameters of the water cycle.
#[derive(Clone, Debug)]
pub struct ClimateParams {
    /// Density of the surface air in kg/m³.
    pub air_density: f64,
    /// Bulk transfer coefficient of moisture (Dalton number).
    pub moisture_exchange: f64,
    /// Speed of the wind fluctuations of weather systems in m/s, added in quadrature to the mean
    /// wind for evaporation (the mean scalar wind is faster than the mean wind vector).
    pub wind_variability: f64,
    /// Relative humidity of the surface air over the ocean.
    pub surface_humidity: f64,
    /// How much colder the surface air is than the sea surface, in K (about 1 K on Earth).
    pub air_sea_temperature_difference: f64,
    /// Shape parameter ω of Fu's Budyko curve.
    pub budyko_omega: f64,
    /// Time scale in days in which the air column rains out at the reference column relative
    /// humidity.
    pub rain_time_scale: f64,
    /// Reference column relative humidity of [`Self::rain_time_scale`].
    pub rain_reference_humidity: f64,
    /// Growth rate of the rain rate with the column relative humidity (per unit of humidity).
    pub rain_humidity_sensitivity: f64,
    /// Diffusivity of the moisture transport by weather systems in m²/s, acting on the specific
    /// humidity of the whole air column.
    pub eddy_diffusivity: f64,
    /// Width (degrees from the ITCZ) of the tropical band where the mean Hadley circulation
    /// rather than eddies carries the moisture; the eddy diffusivity is reduced there by the
    /// factor `1 − exp(−(φ/width)²)`.
    pub hadley_width: f64,
    /// Width (degrees from the pole) of the polar cap where the eddy diffusivity fades out in the
    /// same way.
    pub polar_width: f64,
    /// Temperature decrease with height in K/m, for the scale height of water vapour and to
    /// reduce surface temperatures to sea level.
    pub lapse_rate: f64,
    /// Depth (m) of the boundary layer whose temperature anomaly sets the surface pressure of the
    /// thermally driven winds; 0 disables them.
    pub thermal_wind_depth: f64,
    /// Damping time (days) of the thermally driven winds by friction.
    pub thermal_wind_damping: f64,
    /// Largest speed (m/s) of the thermally driven winds.
    pub max_thermal_wind: f64,
    /// Number of months per year for which the budget is solved, evenly spaced from January; must
    /// divide 12.
    pub months: usize,
    /// Most Newton iterations per month when the climate is first computed.
    pub spin_up_iterations: usize,
    /// Most Newton iterations per month in later updates, starting from the previous state.
    pub update_iterations: usize,
    /// Months refreshed per later update, in turn.
    pub months_per_update: usize,
    /// The solve stops when the summed imbalance of the cells' vapour budgets is below this
    /// fraction of the evaporation.
    pub tolerance: f64,
}

impl Default for ClimateParams {
    fn default() -> Self {
        Self {
            air_density: 1.2,
            moisture_exchange: 1.2e-3,
            wind_variability: 4.5,
            surface_humidity: 0.8,
            air_sea_temperature_difference: 1.0,
            budyko_omega: 2.6,
            rain_time_scale: 10.0,
            rain_reference_humidity: 0.7,
            rain_humidity_sensitivity: 14.0,
            eddy_diffusivity: 1.06e6,
            hadley_width: 15.0,
            polar_width: 15.0,
            lapse_rate: 0.0065,
            thermal_wind_depth: 1500.0,
            thermal_wind_damping: 1.5,
            max_thermal_wind: 10.0,
            months: 4,
            spin_up_iterations: 40,
            update_iterations: 6,
            months_per_update: 1,
            tolerance: 1e-3,
        }
    }
}

impl ClimateParams {
    /// Sets a parameter by its field name, for calibration from the command line.
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        let field = match name {
            "air_density" => &mut self.air_density,
            "moisture_exchange" => &mut self.moisture_exchange,
            "wind_variability" => &mut self.wind_variability,
            "surface_humidity" => &mut self.surface_humidity,
            "air_sea_temperature_difference" => &mut self.air_sea_temperature_difference,
            "budyko_omega" => &mut self.budyko_omega,
            "rain_time_scale" => &mut self.rain_time_scale,
            "rain_reference_humidity" => &mut self.rain_reference_humidity,
            "rain_humidity_sensitivity" => &mut self.rain_humidity_sensitivity,
            "eddy_diffusivity" => &mut self.eddy_diffusivity,
            "hadley_width" => &mut self.hadley_width,
            "polar_width" => &mut self.polar_width,
            "lapse_rate" => &mut self.lapse_rate,
            "thermal_wind_depth" => &mut self.thermal_wind_depth,
            "thermal_wind_damping" => &mut self.thermal_wind_damping,
            "max_thermal_wind" => &mut self.max_thermal_wind,
            "tolerance" => &mut self.tolerance,
            "months" | "spin_up_iterations" | "update_iterations" | "months_per_update" => {
                anyhow::ensure!(value >= 1.0, "{name} must be at least 1");
                let count = value as usize;
                match name {
                    "months" => {
                        anyhow::ensure!(12 % count == 0, "months must divide 12");
                        self.months = count;
                    }
                    "spin_up_iterations" => self.spin_up_iterations = count,
                    "months_per_update" => self.months_per_update = count,
                    _ => self.update_iterations = count,
                }
                return Ok(());
            }
            _ => anyhow::bail!("unknown climate parameter {name}"),
        };
        *field = value;
        Ok(())
    }
}

/// The water cycle of the last update; fluxes in m/yr of water.
#[derive(Clone, Debug, Default)]
pub struct WaterCycle {
    /// Annual precipitation.
    pub precipitation: Vec<f64>,
    /// Annual evaporation from the ocean and evapotranspiration from land.
    pub evaporation: Vec<f64>,
    /// Latitude of the ITCZ (the thermal equator) in each solved month, in degrees.
    pub itcz: Vec<f64>,
    /// Linear solver iterations of each month in the last update (0 if not refreshed).
    pub iterations: Vec<usize>,
    /// Precipitation that came out negative where the solve had not converged and was set to
    /// zero, relative to the total precipitation.
    pub clipped: f64,
    /// Area-weighted global means of precipitation and evaporation in m/yr.
    pub global_precipitation: f64,
    pub global_evaporation: f64,
}

/// An edge between two neighbouring Voronoi cells.
#[derive(Clone, Copy, Debug)]
struct Edge {
    neighbor: u32,
    /// Length of the shared Voronoi edge in m.
    length: f64,
    /// Distance between the cell centres in m.
    distance: f64,
    /// Unit vector from this cell towards the neighbour.
    direction: DVec3,
}

pub struct Climate {
    params: ClimateParams,
    /// Position of every cell on the unit sphere.
    position: Vec<DVec3>,
    /// Latitude of every cell in radians.
    latitude: Vec<f64>,
    /// Area of every Voronoi cell in m².
    area: Vec<f64>,
    /// Edges of every cell.
    edges: Vec<Vec<Edge>>,
    /// Cell orders of the Gauss–Seidel sweeps (by longitude and latitude, both directions).
    orders: [Vec<u32>; 4],
    /// The coarser icosphere levels, for the multigrid preconditioner.
    hierarchy: Hierarchy,
    /// Column water vapour (kg/m²) of every solved month, kept for warm starts.
    vapour: Vec<Vec<f64>>,
    /// Annual mean precipitation of the last update in kg/m²/s, for the evapotranspiration.
    annual_precipitation: Vec<f64>,
    /// The last solution of every month.
    solved: Vec<SolvedMonth>,
    /// The month to refresh next.
    next_month: usize,
    cycle: WaterCycle,
}

/// The last solution of one month; fluxes in kg/m²/s.
#[derive(Clone, Debug, Default)]
struct SolvedMonth {
    precipitation: Vec<f64>,
    evaporation: Vec<f64>,
    /// Potential evaporation of land.
    potential: Vec<f64>,
    /// Latitude of the ITCZ in degrees.
    itcz: f64,
    /// Negative precipitation set to zero, in kg/s.
    clipped: f64,
}

/// The surface and the winds of one month.
struct Month {
    /// Evaporation from the ocean or evapotranspiration from land in kg/m²/s.
    evaporation: Vec<f64>,
    /// Potential evaporation of land in kg/m²/s (zero over the ocean).
    potential: Vec<f64>,
    /// Saturation column water vapour in kg/m².
    saturation: Vec<f64>,
    /// Column water vapour per unit of surface specific humidity in kg/m².
    vapour_per_humidity: Vec<f64>,
    /// Eddy diffusivity in m²/s.
    diffusivity: Vec<f64>,
    /// Low-level wind of every cell in m/s.
    wind: Vec<DVec3>,
}

impl Climate {
    pub fn new(mesh: &SphereMesh, planet_radius: f64, params: ClimateParams) -> Self {
        assert!(
            params.months > 0 && 12 % params.months == 0,
            "the number of months must divide 12"
        );
        let cell_count = mesh.len();
        let latitude: Vec<f64> = (0..cell_count as u32)
            .map(|cell| mesh.vertex(cell).z.clamp(-1.0, 1.0).asin())
            .collect();
        let longitude: Vec<f64> = (0..cell_count as u32)
            .map(|cell| mesh.vertex(cell).y.atan2(mesh.vertex(cell).x))
            .collect();
        let (area, edges) = voronoi_geometry(mesh, planet_radius);

        let sorted = |key: &dyn Fn(usize) -> f64| {
            let mut order: Vec<u32> = (0..cell_count as u32).collect();
            order.sort_by(|&a, &b| key(a as usize).total_cmp(&key(b as usize)));
            order
        };
        let by_longitude = sorted(&|c| longitude[c]);
        let by_latitude = sorted(&|c| latitude[c]);
        let reversed = |order: &Vec<u32>| order.iter().rev().copied().collect::<Vec<_>>();
        let orders = [
            reversed(&by_longitude),
            by_latitude.clone(),
            by_longitude.clone(),
            reversed(&by_latitude),
        ];
        Self {
            hierarchy: Hierarchy::new(mesh, mesh.subdivisions().min(2)),
            vapour: Vec::new(),
            annual_precipitation: Vec::new(),
            solved: Vec::new(),
            next_month: 0,
            position: (0..cell_count as u32).map(|c| mesh.vertex(c)).collect(),
            params,
            latitude,
            area,
            edges,
            orders,
            cycle: WaterCycle::default(),
        }
    }

    pub fn water_cycle(&self) -> &WaterCycle {
        &self.cycle
    }

    /// Solves the water cycle for the given surface elevation (m, sea level 0), monthly surface
    /// temperatures (°C; month 0 is January) and the thermal equator of each month (degrees).
    pub fn update(&mut self, elevation: &[f64], monthly_temperature: &[Vec<f64>], itcz: &[f64]) {
        let cell_count = elevation.len();
        let month_count = self.params.months;
        let months: Vec<usize> = (0..month_count).map(|m| m * 12 / month_count).collect();
        let first = self.vapour.is_empty();
        if first {
            self.vapour = vec![vec![0.0; cell_count]; month_count];
            self.solved = vec![SolvedMonth::default(); month_count];
        }
        // The first update solves every month and iterates, because evapotranspiration depends on
        // the precipitation. Later updates refresh some of the months in turn, starting from
        // their previous state; the climate changes little from one step to the next.
        let (max_newton, rounds, slots): (usize, usize, Vec<usize>) = if first {
            (
                self.params.spin_up_iterations,
                4,
                (0..month_count).collect(),
            )
        } else {
            let count = self.params.months_per_update.clamp(1, month_count);
            let slots = (0..count)
                .map(|i| (self.next_month + i) % month_count)
                .collect();
            self.next_month = (self.next_month + count) % month_count;
            (self.params.update_iterations, 1, slots)
        };

        let mut iterations = vec![0; month_count];
        for _ in 0..rounds {
            for &slot in &slots {
                let thermal_equator = itcz.get(months[slot]).copied().unwrap_or(0.0);
                let mut input = self.month_inputs(
                    elevation,
                    &monthly_temperature[months[slot]],
                    thermal_equator,
                    slot,
                );
                self.solved[slot].potential = input.potential.clone();
                self.solved[slot].itcz = thermal_equator;
                self.set_evapotranspiration(elevation, slot, &mut input);
                let (precipitation, used, clipped) = self.solve_month(&input, slot, max_newton);
                iterations[slot] = used;
                let solved = &mut self.solved[slot];
                solved.precipitation = precipitation;
                solved.evaporation = input.evaporation;
                solved.clipped = clipped;
            }
            self.annual_precipitation = (0..cell_count)
                .map(|cell| {
                    self.solved
                        .iter()
                        .map(|m| m.precipitation[cell])
                        .sum::<f64>()
                        / month_count as f64
                })
                .collect();
        }

        let to_metres_per_year = SECONDS_PER_YEAR / WATER_DENSITY;
        let total: f64 = self
            .annual_precipitation
            .iter()
            .zip(&self.area)
            .map(|(p, a)| p * a)
            .sum();
        let clipped: f64 = self.solved.iter().map(|m| m.clipped).sum::<f64>() / month_count as f64;
        let mut cycle = WaterCycle {
            precipitation: self
                .annual_precipitation
                .iter()
                .map(|p| p * to_metres_per_year)
                .collect(),
            evaporation: (0..cell_count)
                .map(|cell| {
                    self.solved.iter().map(|m| m.evaporation[cell]).sum::<f64>()
                        / month_count as f64
                        * to_metres_per_year
                })
                .collect(),
            itcz: self.solved.iter().map(|m| m.itcz).collect(),
            iterations,
            clipped: clipped / total.max(1e-30),
            ..WaterCycle::default()
        };
        let total_area: f64 = self.area.iter().sum();
        let area_mean = |field: &[f64]| {
            field
                .iter()
                .zip(&self.area)
                .map(|(v, a)| v * a)
                .sum::<f64>()
                / total_area
        };
        cycle.global_precipitation = area_mean(&cycle.precipitation);
        cycle.global_evaporation = area_mean(&cycle.evaporation);
        self.cycle = cycle;
    }

    /// Evaporation, saturation and winds of one month. The evapotranspiration of land is left at
    /// zero; see [`Self::set_evapotranspiration`].
    fn month_inputs(
        &self,
        elevation: &[f64],
        temperature: &[f64],
        thermal_equator: f64,
        slot: usize,
    ) -> Month {
        let params = &self.params;
        let cell_count = elevation.len();
        // The temperature of the free troposphere, which sets the saturation of the column, is
        // much more uniform horizontally than the surface temperature: use the zonal mean of the
        // sea-level temperature.
        let sea_level =
            |cell: usize| temperature[cell] + params.lapse_rate * elevation[cell].max(0.0);
        let column_temperature = self.zonal_mean(&sea_level);
        let thermal = self.thermal_wind(&sea_level, &column_temperature);
        let wind: Vec<DVec3> = (0..cell_count)
            .map(|cell| low_level_wind(self.position[cell], thermal_equator) + thermal[cell])
            .collect();

        let vapour = &self.vapour[slot];
        let mut evaporation = vec![0.0; cell_count];
        let mut potential = vec![0.0; cell_count];
        let mut saturation = vec![0.0; cell_count];
        let mut vapour_per_humidity = vec![0.0; cell_count];
        for cell in 0..cell_count {
            let (column, scale_height) =
                saturation_column(column_temperature[cell], params.lapse_rate);
            // Above high ground, the air column is shorter and colder.
            let height = (-elevation[cell].max(0.0) / scale_height).exp();
            saturation[cell] = column * height;
            vapour_per_humidity[cell] = params.air_density * scale_height * height;
            let speed = (wind[cell].length_squared() + params.wind_variability.powi(2)).sqrt();
            let transfer = params.air_density * params.moisture_exchange * speed;
            let saturated = saturation_specific_humidity(temperature[cell]);
            if elevation[cell] < 0.0 {
                let air = params.surface_humidity
                    * saturation_specific_humidity(
                        temperature[cell] - params.air_sea_temperature_difference,
                    );
                evaporation[cell] = transfer * (saturated - air).max(0.0);
            } else {
                // The specific humidity of the surface air follows from the column vapour.
                // The specific humidity of the surface air follows from the column vapour, but
                // the air over a wet surface is no more humid than over the ocean.
                let air = (vapour[cell] / vapour_per_humidity[cell])
                    .min(params.surface_humidity * saturated);
                potential[cell] = transfer * (saturated - air);
            }
        }

        let diffusivity = self
            .position
            .iter()
            .map(|position| {
                let latitude = position.z.clamp(-1.0, 1.0).asin().to_degrees();
                let distance =
                    circulation_latitude(latitude, thermal_equator) / params.hadley_width;
                // Weather systems also fade towards the poles, beyond the storm tracks.
                let polar = (90.0 - circulation_latitude(latitude, thermal_equator).abs())
                    / params.polar_width;
                params.eddy_diffusivity
                    * (1.0 - (-distance * distance).exp())
                    * (1.0 - (-polar * polar).exp())
            })
            .collect();
        Month {
            evaporation,
            potential,
            saturation,
            vapour_per_humidity,
            diffusivity,
            wind,
        }
    }

    /// Sets the evapotranspiration of land in one month. The Budyko curve describes annual means
    /// (soils store water between seasons), so the annual evapotranspiration follows from the
    /// annual precipitation of the last update and the annual potential evaporation, and is
    /// spread over the months in proportion to their potential evaporation.
    fn set_evapotranspiration(&self, elevation: &[f64], slot: usize, month: &mut Month) {
        if self.annual_precipitation.is_empty() {
            return;
        }
        let count = self.solved.len() as f64;
        for cell in 0..elevation.len() {
            if elevation[cell] < 0.0 {
                continue;
            }
            let potential = self
                .solved
                .iter()
                .map(|m| m.potential.get(cell).copied().unwrap_or(0.0))
                .sum::<f64>()
                / count;
            if potential <= 0.0 {
                continue;
            }
            let precipitation = self.annual_precipitation[cell];
            let actual =
                precipitation - budyko_runoff(precipitation, potential, self.params.budyko_omega);
            month.evaporation[cell] = actual * self.solved[slot].potential[cell] / potential;
        }
    }

    /// Thermally driven boundary-layer winds (m/s) after Lindzen & Nigam (1987): air warmer than
    /// the zonal mean (`sea_level` minus `zonal`, in K) lowers the surface pressure by
    /// `p' = −ρ g H T'/T₀`, and the wind balances the pressure gradient with friction and the
    /// Coriolis force, `ε u + f k × u = −∇p'/ρ`. Summer continents draw air in (monsoons), winter
    /// continents push it out.
    fn thermal_wind(&self, sea_level: &dyn Fn(usize) -> f64, zonal: &[f64]) -> Vec<DVec3> {
        let params = &self.params;
        if params.thermal_wind_depth <= 0.0 {
            return vec![DVec3::ZERO; zonal.len()];
        }
        let anomaly: Vec<f64> = (0..zonal.len()).map(|c| sea_level(c) - zonal[c]).collect();
        let friction = 1.0 / (params.thermal_wind_damping * 86_400.0);
        (0..zonal.len())
            .map(|cell| {
                // Least-squares gradient of the anomaly (K/m).
                let edges = &self.edges[cell];
                let mut gradient = DVec3::ZERO;
                for edge in edges {
                    gradient += edge.direction * (anomaly[edge.neighbor as usize] - anomaly[cell])
                        / edge.distance;
                }
                gradient *= 2.0 / edges.len() as f64;
                let position = self.position[cell];
                // Remove the radial part of the chord directions.
                let gradient = gradient - position * gradient.dot(position);
                let force = gradient * GRAVITY * params.thermal_wind_depth / 288.0;
                let coriolis = 2.0 * EARTH_ROTATION * position.z;
                let wind = (force * friction - position.cross(force) * coriolis)
                    / (friction * friction + coriolis * coriolis);
                // Without the cumulus friction of the deep tropics, the balance overestimates the
                // winds near the equator; real monsoon flows reach about 10 m/s.
                let speed = wind.length();
                if speed > params.max_thermal_wind {
                    wind * (params.max_thermal_wind / speed)
                } else {
                    wind
                }
            })
            .collect()
    }

    /// The zonal mean of a field at the latitude of every cell, smoothed over a few degrees.
    fn zonal_mean(&self, value: &dyn Fn(usize) -> f64) -> Vec<f64> {
        const BINS: usize = 180;
        let bin = |latitude: f64| {
            (((latitude.to_degrees() + 90.0) / 180.0 * BINS as f64) as usize).min(BINS - 1)
        };
        let mut sum = [0.0; BINS];
        let mut count = [0.0; BINS];
        for (cell, &latitude) in self.latitude.iter().enumerate() {
            sum[bin(latitude)] += value(cell);
            count[bin(latitude)] += 1.0;
        }
        // Gaussian smoothing with a width of 3 bins (3°).
        let smoothed: Vec<f64> = (0..BINS)
            .map(|b| {
                let (mut s, mut c) = (0.0, 0.0);
                for other in b.saturating_sub(9)..(b + 10).min(BINS) {
                    let d = (other as f64 - b as f64) / 3.0;
                    let w = (-0.5 * d * d).exp();
                    s += w * sum[other];
                    c += w * count[other];
                }
                s / c.max(1e-12)
            })
            .collect();
        self.latitude
            .iter()
            .map(|&latitude| smoothed[bin(latitude)])
            .collect()
    }

    /// Solves the vapour budget of one month in place, starting from the previous solution.
    /// Returns the precipitation (kg/m²/s), the number of linear iterations and the clipped negative
    /// precipitation (kg/s).
    fn solve_month(
        &mut self,
        month: &Month,
        slot: usize,
        max_newton: usize,
    ) -> (Vec<f64>, usize, f64) {
        let params = &self.params;
        let cell_count = self.area.len();
        // Eddy diffusion of the specific humidity `q = W / m` (m: column vapour per unit of
        // specific humidity) acting on the mass of the whole air column, p/g.
        let column_mass = SURFACE_PRESSURE / GRAVITY;
        let mass = &month.vapour_per_humidity;

        let rain = RainLaw::new(params);
        let vapour = &mut self.vapour[slot];
        // Coefficients of the linear part of every cell's budget:
        // `own · W_i − Σ coupling_ij · W_j + area · P(W_i) = area · E_i`.
        let own_and_couplings = |cell: usize| {
            let mut own = 0.0;
            let mut couplings = [(0u32, 0.0); 6];
            for (slot, edge) in self.edges[cell].iter().enumerate() {
                let j = edge.neighbor as usize;
                // Advective volume flux across the edge, positive outwards (m²/s).
                let q =
                    (0.5 * (month.wind[cell] + month.wind[j])).dot(edge.direction) * edge.length;
                let diffusivity = 0.5 * (month.diffusivity[cell] + month.diffusivity[j]);
                let c = column_mass * diffusivity * edge.length / edge.distance;
                own += q.max(0.0) + c / mass[cell];
                couplings[slot] = (edge.neighbor, (-q).max(0.0) + c / mass[j]);
            }
            (own, couplings, self.edges[cell].len())
        };
        let coefficients: Vec<Coefficients> = (0..cell_count).map(own_and_couplings).collect();

        let budget = Budget {
            coefficients: &coefficients,
            area: &self.area,
            evaporation: &month.evaporation,
            saturation: &month.saturation,
            rain: &rain,
        };
        let total_evaporation: f64 = self
            .area
            .iter()
            .zip(&month.evaporation)
            .map(|(a, e)| a * e)
            .sum();
        let tolerance = params.tolerance * total_evaporation.max(1e-30);

        // Gauss–Seidel sweeps bring a cold start close enough for Newton's method. Later, where the
        // surface changed (uplift, new land), they resolve the stiff local balance of columns
        // that would now be supersaturated.
        let sweeps = if vapour.iter().all(|&w| w == 0.0) {
            100
        } else {
            2
        };
        for sweep in 0..sweeps {
            budget.gauss_seidel(vapour, &self.orders[sweep % self.orders.len()]);
        }
        // Newton's method; the linear systems are solved with BiCGSTAB, preconditioned with
        // symmetric Gauss–Seidel.
        let mut iterations = 0;
        let mut residual = budget.residual(vapour);
        let mut norm = l1(&residual);
        for _ in 0..max_newton {
            if norm < tolerance {
                break;
            }
            let diagonal: Vec<f64> = (0..cell_count)
                .map(|cell| {
                    let (_, dp) = rain.rate(vapour[cell], month.saturation[cell]);
                    coefficients[cell].0 + self.area[cell] * dp
                })
                .collect();
            let rhs: Vec<f64> = residual.iter().map(|r| -r).collect();
            let mut values = Vec::with_capacity(cell_count * 7);
            for (cell, (_, couplings, count)) in coefficients.iter().enumerate() {
                values.push(diagonal[cell]);
                values.extend(couplings[..*count].iter().map(|&(_, c)| -c));
            }
            let multigrid = Multigrid::new(&self.hierarchy, values);
            let (step, used) = multigrid.solve(&rhs, 1e-2, 100);
            iterations += used;
            tracing::trace!(norm = norm / total_evaporation, used, "Newton step");
            // Damped update that keeps the vapour positive.
            let mut factor = 1.0;
            loop {
                let trial: Vec<f64> = vapour
                    .iter()
                    .zip(&step)
                    .map(|(w, d)| (w + factor * d).max(0.2 * w))
                    .collect();
                let trial_residual = budget.residual(&trial);
                let trial_norm = l1(&trial_residual);
                if trial_norm < norm || factor < 0.05 {
                    *vapour = trial;
                    residual = trial_residual;
                    norm = trial_norm;
                    break;
                }
                factor *= 0.5;
            }
        }

        // Precipitation as the residual of the budget: evaporation plus moisture convergence.
        // It equals the rain law at convergence, and closes the budget exactly regardless.
        let mut precipitation = vec![0.0; cell_count];
        let mut clipped = 0.0;
        for cell in 0..cell_count {
            let (own, couplings, count) = &coefficients[cell];
            let convergence = couplings[..*count]
                .iter()
                .map(|&(j, c)| c * vapour[j as usize])
                .sum::<f64>()
                - own * vapour[cell];
            let p = month.evaporation[cell] + convergence / self.area[cell];
            if p < 0.0 {
                clipped -= p * self.area[cell];
            }
            precipitation[cell] = p.max(0.0);
        }
        (precipitation, iterations, clipped)
    }
}

/// The vapour budget of every cell: `own · W_i − Σ coupling_ij · W_j + area · P(W_i) = area · E_i`.
struct Budget<'a> {
    coefficients: &'a [Coefficients],
    area: &'a [f64],
    evaporation: &'a [f64],
    saturation: &'a [f64],
    rain: &'a RainLaw,
}

/// The outflow coefficient of a cell, the inflow coefficients of its neighbours, and their
/// number.
type Coefficients = (f64, [(u32, f64); 6], usize);

impl Budget<'_> {
    /// The imbalance of every cell's budget in kg/s.
    fn residual(&self, vapour: &[f64]) -> Vec<f64> {
        (0..vapour.len())
            .map(|cell| {
                let (own, couplings, count) = &self.coefficients[cell];
                let inflow: f64 = couplings[..*count]
                    .iter()
                    .map(|&(j, c)| c * vapour[j as usize])
                    .sum();
                let (p, _) = self.rain.rate(vapour[cell], self.saturation[cell]);
                own * vapour[cell] - inflow + self.area[cell] * (p - self.evaporation[cell])
            })
            .collect()
    }

    /// One nonlinear Gauss–Seidel sweep in the given cell order.
    fn gauss_seidel(&self, vapour: &mut [f64], order: &[u32]) {
        for &cell in order {
            let cell = cell as usize;
            let area = self.area[cell];
            let (own, couplings, count) = &self.coefficients[cell];
            let source = couplings[..*count]
                .iter()
                .map(|&(j, c)| c * vapour[j as usize])
                .sum::<f64>()
                + area * self.evaporation[cell];
            // Newton's method on own·W + area·P(W) = source; the left side is convex and
            // increasing.
            let mut w = vapour[cell];
            for _ in 0..6 {
                let (p, dp) = self.rain.rate(w, self.saturation[cell]);
                let next = w - (own * w + area * p - source) / (own + area * dp);
                let next = if next < 0.0 { 0.5 * w } else { next };
                let done = (next - w).abs() < 1e-7 * (1.0 + w);
                w = next;
                if done {
                    break;
                }
            }
            vapour[cell] = w;
        }
    }
}

fn l1(values: &[f64]) -> f64 {
    values.iter().map(|v| v.abs()).sum()
}

/// Precipitation as a function of the column water vapour and its saturation value.
struct RainLaw {
    /// Inverse rain-out time scale at the reference humidity, 1/s.
    rate: f64,
    reference: f64,
    sensitivity: f64,
}

impl RainLaw {
    fn new(params: &ClimateParams) -> Self {
        Self {
            rate: 1.0 / (params.rain_time_scale * 86_400.0),
            reference: params.rain_reference_humidity,
            sensitivity: params.rain_humidity_sensitivity,
        }
    }

    /// Precipitation (kg/m²/s) and its derivative with respect to the column vapour `w` (kg/m²).
    fn rate(&self, w: f64, saturation: f64) -> (f64, f64) {
        // Beyond twice saturation the growth is capped to avoid overflow; the column never gets
        // there in a solution.
        let humidity = (w / saturation).min(2.0);
        let factor = (self.sensitivity * (humidity - self.reference)).exp() * self.rate;
        let p = w * factor;
        let dp = if w / saturation < 2.0 {
            factor * (1.0 + self.sensitivity * humidity)
        } else {
            factor
        };
        (p, dp)
    }
}

/// Runoff `P − ET` from Fu's form of the Budyko curve: `ET = P + PET − (P^ω + PET^ω)^(1/ω)`.
fn budyko_runoff(precipitation: f64, potential: f64, omega: f64) -> f64 {
    if precipitation <= 0.0 {
        return 0.0;
    }
    if potential <= 0.0 {
        return precipitation;
    }
    // Scale to avoid under- and overflow of the powers.
    let scale = precipitation.max(potential);
    let p = precipitation / scale;
    let e = potential / scale;
    let root = (p.powf(omega) + e.powf(omega)).powf(1.0 / omega);
    ((root - e) * scale).max(0.0)
}

/// Saturation vapour pressure over water in Pa at `t` °C (Magnus formula, Alduchov & Eskridge,
/// 1996).
fn saturation_vapour_pressure(t: f64) -> f64 {
    let t = t.max(-80.0);
    610.94 * (17.625 * t / (t + 243.04)).exp()
}

/// Saturation specific humidity (kg/kg) at the surface at `t` °C.
fn saturation_specific_humidity(t: f64) -> f64 {
    0.622 * saturation_vapour_pressure(t) / SURFACE_PRESSURE
}

/// Water vapour (kg/m²) of a saturated air column above a surface at `t` °C: the saturation
/// vapour density at the surface times the scale height of vapour, which follows from the
/// Clausius–Clapeyron relation and the lapse rate.
/// Returns the column and the scale height (m).
fn saturation_column(t: f64, lapse_rate: f64) -> (f64, f64) {
    let kelvin = t.max(-80.0) + 273.15;
    let density = saturation_vapour_pressure(t) / (VAPOUR_GAS_CONSTANT * kelvin);
    let scale_height =
        1.0 / (lapse_rate * (LATENT_HEAT / (VAPOUR_GAS_CONSTANT * kelvin * kelvin) - 1.0 / kelvin));
    (density * scale_height, scale_height)
}

/// Zonal-mean low-level wind (eastward, poleward) in m/s at a latitude (degrees, positive) of the
/// circulation, i.e. measured from the ITCZ. Representative of Earth's moisture-carrying lower
/// troposphere: trade winds converging on the ITCZ (Hadley cell), westerlies with a weak
/// poleward drift (Ferrel cell) and polar easterlies.
fn zonal_mean_wind(latitude: f64) -> (f64, f64) {
    const TABLE: [(f64, f64, f64); 10] = [
        (0.0, -4.0, 0.0),
        (10.0, -6.0, -0.6),
        (20.0, -5.0, -0.5),
        (30.0, 0.0, 0.0),
        (40.0, 6.0, 0.3),
        (50.0, 8.0, 0.3),
        (60.0, 3.0, 0.0),
        (70.0, -2.0, -0.2),
        (80.0, -2.0, -0.1),
        (90.0, 0.0, 0.0),
    ];
    let latitude = latitude.clamp(0.0, 90.0);
    let index = ((latitude / 10.0) as usize).min(TABLE.len() - 2);
    let (l0, u0, v0) = TABLE[index];
    let (l1, u1, v1) = TABLE[index + 1];
    let f = (latitude - l0) / (l1 - l0);
    (u0 + (u1 - u0) * f, v0 + (v1 - v0) * f)
}

/// The zonal-mean eastward surface wind stress (N/m²) at `latitude` (degrees) for a circulation
/// centred on the equator: `ρ_a C_D |U| u`, with the speed of the mean wind and of the
/// fluctuations `variability` (m/s) of weather systems.
pub fn surface_wind_stress(latitude: f64, variability: f64) -> f64 {
    const AIR_DENSITY: f64 = 1.2;
    const DRAG_COEFFICIENT: f64 = 1.3e-3;
    let (eastward, poleward) = zonal_mean_wind(latitude.abs());
    let speed = (eastward * eastward + poleward * poleward + variability * variability).sqrt();
    AIR_DENSITY * DRAG_COEFFICIENT * speed * eastward
}

/// The latitude of the circulation (degrees): the geographic latitude stretched so that the ITCZ
/// at `thermal_equator` maps to 0 while the poles stay at ±90°.
fn circulation_latitude(latitude: f64, thermal_equator: f64) -> f64 {
    if latitude >= thermal_equator {
        (latitude - thermal_equator) * 90.0 / (90.0 - thermal_equator)
    } else {
        (latitude - thermal_equator) * 90.0 / (90.0 + thermal_equator)
    }
}

/// The low-level wind vector (m/s) at `position` on the unit sphere, with the ITCZ at
/// `thermal_equator` (degrees).
pub fn low_level_wind(position: DVec3, thermal_equator: f64) -> DVec3 {
    let east = DVec3::Z.cross(position);
    let east = if east.length_squared() < 1e-12 {
        DVec3::X
    } else {
        east.normalize()
    };
    let north = position.cross(east);
    let latitude = position.z.clamp(-1.0, 1.0).asin().to_degrees();
    let circulation = circulation_latitude(latitude, thermal_equator);
    let (eastward, poleward) = zonal_mean_wind(circulation.abs());
    east * eastward + north * poleward * circulation.signum()
}

/// Areas (m²) of the Voronoi cells and their edges, from the circumcentres of the triangles.
fn voronoi_geometry(mesh: &SphereMesh, radius: f64) -> (Vec<f64>, Vec<Vec<Edge>>) {
    let cell_count = mesh.len();
    let mut corners: std::collections::HashMap<(u32, u32), Vec<DVec3>> = Default::default();
    let mut area = vec![0.0; cell_count];
    let triangle = |a: DVec3, b: DVec3, c: DVec3| 0.5 * (b - a).cross(c - a).length();
    for &[a, b, c] in mesh.faces() {
        let (pa, pb, pc) = (mesh.vertex(a), mesh.vertex(b), mesh.vertex(c));
        let mut centre = (pb - pa).cross(pc - pa).normalize();
        if centre.dot(pa) < 0.0 {
            centre = -centre;
        }
        for (i, j, k) in [(a, b, c), (b, c, a), (c, a, b)] {
            corners
                .entry((i.min(j), i.max(j)))
                .or_default()
                .push(centre);
            let (pi, pj, pk) = (mesh.vertex(i), mesh.vertex(j), mesh.vertex(k));
            let mid_ij = (pi + pj).normalize();
            let mid_ik = (pi + pk).normalize();
            area[i as usize] += triangle(pi, mid_ij, centre) + triangle(pi, centre, mid_ik);
        }
    }
    let total: f64 = area.iter().sum();
    let scale = 4.0 * std::f64::consts::PI * radius * radius / total;
    for a in &mut area {
        *a *= scale;
    }
    let edges = (0..cell_count as u32)
        .map(|cell| {
            let position = mesh.vertex(cell);
            mesh.neighbors(cell)
                .iter()
                .map(|&neighbor| {
                    let other = mesh.vertex(neighbor);
                    let ends = &corners[&(cell.min(neighbor), cell.max(neighbor))];
                    Edge {
                        neighbor,
                        length: ends[0].angle_between(ends[1]) * radius,
                        distance: position.angle_between(other) * radius,
                        direction: (other - position).normalize(),
                    }
                })
                .collect()
        })
        .collect();
    (area, edges)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RADIUS: f64 = 6.371e6;

    #[test]
    fn winds_follow_three_cell_circulation() {
        let at = |latitude: f64| {
            let latitude = latitude.to_radians();
            DVec3::new(latitude.cos(), 0.0, latitude.sin())
        };
        // At longitude 0, east is +y.
        assert!(low_level_wind(at(15.0), 0.0).y < 0.0);
        assert!(low_level_wind(at(45.0), 0.0).y > 0.0);
        assert!(low_level_wind(at(-45.0), 0.0).y > 0.0);
        assert!(low_level_wind(at(75.0), 0.0).y < 0.0);
        // Trade winds blow towards the ITCZ in both hemispheres.
        assert!(low_level_wind(at(15.0), 0.0).z < 0.0);
        assert!(low_level_wind(at(-15.0), 0.0).z > 0.0);
        // With the ITCZ at 10°N, the air at 5°N flows north towards it.
        assert!(low_level_wind(at(5.0), 10.0).z > 0.0);
    }

    #[test]
    fn budyko_curve_limits() {
        // Humid: runoff a little above P − PET; arid: almost everything evaporates.
        let humid = budyko_runoff(2.0, 0.5, 2.6);
        assert!(humid > 1.5 && humid < 1.6, "{humid}");
        let arid = budyko_runoff(0.1, 2.0, 2.6);
        assert!(arid < 0.001, "{arid}");
    }

    #[test]
    fn saturation_column_is_earth_like() {
        // About 60 kg/m² in the tropics, about 10 kg/m² at the freezing point.
        let tropics = saturation_column(27.0, 0.0065).0;
        assert!((50.0..75.0).contains(&tropics), "{tropics}");
        let freezing = saturation_column(0.0, 0.0065).0;
        assert!((7.0..14.0).contains(&freezing), "{freezing}");
    }

    /// Zonally symmetric Earth-like temperatures (°C) of a month.
    fn temperatures(mesh: &SphereMesh, elevation: &[f64]) -> Vec<f64> {
        (0..mesh.len() as u32)
            .map(|c| {
                let latitude = mesh.vertex(c).z.asin();
                -25.0 + 52.0 * latitude.cos() - 0.0065 * elevation[c as usize].max(0.0)
            })
            .collect()
    }

    fn solve(mesh: &SphereMesh, elevation: &[f64]) -> (Climate, Vec<f64>, Vec<f64>) {
        let mut climate = Climate::new(mesh, RADIUS, ClimateParams::default());
        let monthly = vec![temperatures(mesh, elevation); 12];
        climate.update(elevation, &monthly, &[0.0; 12]);
        let precipitation = climate.water_cycle().precipitation.clone();
        let evaporation = climate.water_cycle().evaporation.clone();
        (climate, precipitation, evaporation)
    }

    #[test]
    fn aquaplanet_conserves_water_and_has_rain_belts() {
        let mesh = SphereMesh::icosphere(5);
        let ocean = vec![-4000.0; mesh.len()];
        let (climate, precipitation, evaporation) = solve(&mesh, &ocean);
        let total = |field: &[f64]| -> f64 {
            field
                .iter()
                .zip(&climate.area)
                .map(|(value, area)| value * area)
                .sum()
        };
        let (p, e) = (total(&precipitation), total(&evaporation));
        assert!((p / e - 1.0).abs() < 0.01, "ΣP {p:e} vs ΣE {e:e}");
        let global_mean = p / (4.0 * std::f64::consts::PI * RADIUS * RADIUS);
        assert!((0.7..1.5).contains(&global_mean), "{global_mean} m/yr");

        let mean_at = |from: f64, to: f64| {
            let cells: Vec<f64> = (0..mesh.len())
                .filter(|&c| (from..to).contains(&climate.latitude[c].to_degrees().abs()))
                .map(|c| precipitation[c])
                .collect();
            cells.iter().sum::<f64>() / cells.len() as f64
        };
        // Wet ITCZ, dry subtropics, wetter storm tracks, dry poles.
        assert!(mean_at(0.0, 5.0) > 2.0 * mean_at(20.0, 30.0));
        assert!(mean_at(40.0, 55.0) > mean_at(20.0, 30.0));
        assert!(mean_at(75.0, 90.0) < mean_at(40.0, 55.0));
    }

    #[test]
    fn rain_belt_follows_the_thermal_equator() {
        let mesh = SphereMesh::icosphere(5);
        let ocean = vec![-4000.0; mesh.len()];
        let mut climate = Climate::new(&mesh, RADIUS, ClimateParams::default());
        let monthly = vec![temperatures(&mesh, &ocean); 12];
        climate.update(&ocean, &monthly, &[10.0; 12]);
        let precipitation = &climate.water_cycle().precipitation;
        let mean_at = |from: f64, to: f64| {
            let cells: Vec<f64> = (0..mesh.len())
                .filter(|&c| (from..to).contains(&climate.latitude[c].to_degrees()))
                .map(|c| precipitation[c])
                .collect();
            cells.iter().sum::<f64>() / cells.len() as f64
        };
        assert!(mean_at(5.0, 15.0) > 1.5 * mean_at(-15.0, -5.0));
    }

    #[test]
    fn continents_conserve_water_rain_shadow_and_fetch() {
        let mesh = SphereMesh::icosphere(5);
        // A continent in the westerlies with a north-south mountain range.
        let mut elevation = vec![-4000.0; mesh.len()];
        let longitude = |c: u32| mesh.vertex(c).y.atan2(mesh.vertex(c).x).to_degrees();
        let latitude = |c: u32| mesh.vertex(c).z.asin().to_degrees();
        for cell in 0..mesh.len() as u32 {
            if (35.0..55.0).contains(&latitude(cell)) && (-40.0..40.0).contains(&longitude(cell)) {
                elevation[cell as usize] = if longitude(cell).abs() < 3.0 {
                    3000.0
                } else {
                    200.0
                };
            }
        }
        let (climate, precipitation, evaporation) = solve(&mesh, &elevation);
        let total = |field: &[f64]| -> f64 {
            field
                .iter()
                .zip(&climate.area)
                .map(|(value, area)| value * area)
                .sum()
        };
        let (p, e) = (total(&precipitation), total(&evaporation));
        assert!((p / e - 1.0).abs() < 0.01, "ΣP {p:e} vs ΣE {e:e}");

        let mean_land = |from: f64, to: f64| {
            let cells: Vec<f64> = (0..mesh.len() as u32)
                .filter(|&c| elevation[c as usize] > 0.0 && (from..to).contains(&longitude(c)))
                .map(|c| precipitation[c as usize])
                .collect();
            cells.iter().sum::<f64>() / cells.len() as f64
        };
        // Wet windward side, dry lee side.
        assert!(
            mean_land(-8.0, -3.0) > 1.5 * mean_land(3.0, 8.0),
            "windward {} lee {}",
            mean_land(-8.0, -3.0),
            mean_land(3.0, 8.0)
        );
        // Moisture from the ocean reaches the west coast; precipitation decreases inland.
        assert!(mean_land(-40.0, -32.0) > mean_land(-25.0, -10.0));
        // Land evapotranspiration never exceeds precipitation.
        for cell in 0..mesh.len() {
            if elevation[cell] >= 0.0 {
                assert!(evaporation[cell] <= precipitation[cell] + 1e-12);
            }
        }
    }
}
