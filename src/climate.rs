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
//!   easterlies. The pattern moves with the seasons and with longitude: the ITCZ lies at the
//!   maximum of the moist static energy of the near-surface air (`c_p T + L q`) in each sector of
//!   longitude, so it swings far over warm, moist summer continents (monsoons) and little over
//!   the ocean, but not over hot, dry deserts. In the tropics the mean Hadley circulation
//!   carries the moisture and eddy diffusion fades out (as in the moist energy balance model of
//!   Siler et al., 2018).
//! - **Evaporation** over the ocean follows the bulk aerodynamic formula
//!   `E = ρ C_E |U| (q_sat(SST) − RH q_sat(SST − ΔT))`, with the speed of the mean wind and of
//!   the fluctuations of weather systems, and surface air slightly colder than the sea. Over land,
//!   the actual evapotranspiration follows the Budyko curve in Fu's form (Fu, 1981; Zhang et al.,
//!   2004) from the annual precipitation and potential evaporation (the same bulk formula at the
//!   land surface temperature, with the humidity of the air above). Evapotranspiration returns to
//!   the air column (moisture recycling); the rest of the precipitation runs off. Lakes evaporate
//!   at the potential rate where they are free of ice: the water balance of the lakes (see
//!   [`crate::erosion`]) removes this evaporation, beyond the land's evapotranspiration, from the
//!   runoff, and it returns to the air column here, one time step later (#126).
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

use crate::math;
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

/// Specific heat capacity of air at constant pressure in J/kg/K.
const AIR_HEAT_CAPACITY: f64 = 1004.0;
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
    /// thermally driven winds; 0 (the default) disables them. At the coarse resolution of the
    /// temperature model (~440 km), 1500 m gave strong along-shore flows, doubled the arid land
    /// and pulled the storm tracks to 35°, so the monsoons come from the thermal equator per
    /// longitude instead.
    pub thermal_wind_depth: f64,
    /// Damping time (days) of the thermally driven winds by friction.
    pub thermal_wind_damping: f64,
    /// Largest speed (m/s) of the thermally driven winds.
    pub max_thermal_wind: f64,
    /// Width (degrees) of the equatorial band where the thermally driven winds fade out.
    pub thermal_wind_equator: f64,
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
    /// Whether lakes on cells the climate sees as ocean evaporate at the open-water rate there
    /// (#168): the climate's ocean and the erosion's lakes are found on surfaces that can differ
    /// at a sill on which the sea stands (by rounding, the sea-level change of the step, or the
    /// sample surface). Otherwise such lake cells have no potential evaporation (before #168).
    pub ocean_lake_evaporation: bool,
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
            thermal_wind_depth: 0.0,
            thermal_wind_damping: 1.5,
            max_thermal_wind: 10.0,
            thermal_wind_equator: 10.0,
            months: 4,
            spin_up_iterations: 40,
            update_iterations: 6,
            months_per_update: 1,
            tolerance: 1e-3,
            ocean_lake_evaporation: true,
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
            "thermal_wind_equator" => &mut self.thermal_wind_equator,
            "tolerance" => &mut self.tolerance,
            "ocean_lake_evaporation" => {
                self.ocean_lake_evaporation = value != 0.0;
                return Ok(());
            }
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
    /// Annual potential evaporation of land (zero over the ocean): the evaporation of open water
    /// or a wet surface at the land temperature.
    pub potential_evaporation: Vec<f64>,
    /// Annual potential evaporation of lake water (zero over the ocean): the potential
    /// evaporation of the months in which a lake is free of ice (see [`ice_free`]). The water
    /// balance of the lakes evaporates at this rate.
    pub lake_potential_evaporation: Vec<f64>,
    /// Annual evaporation of lake water that the climate received as a moisture source, in
    /// excess of the land's evapotranspiration (part of [`Self::evaporation`]). Each solved
    /// month keeps the lake evaporation of the update that last solved it, so after the first
    /// update this is the mean of the last `months / months_per_update` water balances.
    pub lake_evaporation: Vec<f64>,
    /// Annual runoff of land, `P − ET` from the Budyko curve with the current annual
    /// precipitation and potential evaporation (zero over the ocean). It feeds rivers.
    pub runoff: Vec<f64>,
    /// Latitude of the ITCZ (the thermal equator) in each solved month, in degrees.
    pub itcz: Vec<f64>,
    /// Linear solver iterations of each month in the last update (0 if not refreshed).
    pub iterations: Vec<usize>,
    /// Precipitation that came out negative where the solve had not converged and was set to
    /// zero, relative to the total precipitation.
    pub clipped: f64,
    /// Precipitation (m/yr) of every solved month.
    pub monthly_precipitation: Vec<Vec<f64>>,
    /// Evaporation and evapotranspiration (m/yr) of every solved month, as [`Self::evaporation`].
    pub monthly_evaporation: Vec<Vec<f64>>,
    /// The low-level wind (eastward, northward; m/s) that carries the moisture, as the mean of
    /// the solved months: the zonal profile of the three-cell circulation around each
    /// longitude's ITCZ (see [`low_level_wind`]), plus the thermally driven winds if they are
    /// enabled (`ClimateParams::thermal_wind_depth`).
    pub wind: Vec<[f64; 2]>,
    /// The low-level wind of every solved month.
    pub monthly_wind: Vec<Vec<[f64; 2]>>,
    /// Area-weighted global means of precipitation and evaporation in m/yr.
    pub global_precipitation: f64,
    pub global_evaporation: f64,
    /// [`Self::lake_evaporation`] summed over the planet, in m³/yr.
    pub total_lake_evaporation: f64,
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
    /// Low-level wind (eastward, northward) in m/s.
    wind: Vec<[f64; 2]>,
    evaporation: Vec<f64>,
    /// Potential evaporation of land.
    potential: Vec<f64>,
    /// Potential evaporation of ice-free lake water.
    lake_potential: Vec<f64>,
    /// Evaporation of lake water beyond the land's evapotranspiration.
    lake_evaporation: Vec<f64>,
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
    /// Potential evaporation of ice-free lake water in kg/m²/s (zero over the ocean).
    lake_potential: Vec<f64>,
    /// Saturation column water vapour in kg/m².
    saturation: Vec<f64>,
    /// Column water vapour per unit of surface specific humidity in kg/m².
    vapour_per_humidity: Vec<f64>,
    /// Eddy diffusivity in m²/s.
    diffusivity: Vec<f64>,
    /// Low-level wind of every cell in m/s.
    wind: Vec<DVec3>,
    /// Mean latitude of the ITCZ over the sectors of longitude, in degrees.
    itcz: f64,
}

impl Climate {
    pub fn new(mesh: &SphereMesh, planet_radius: f64, params: ClimateParams) -> Self {
        assert!(
            params.months > 0 && 12 % params.months == 0,
            "the number of months must divide 12"
        );
        let cell_count = mesh.len();
        let latitude: Vec<f64> = (0..cell_count as u32)
            .map(|cell| math::asin(mesh.vertex(cell).z.clamp(-1.0, 1.0)))
            .collect();
        let longitude: Vec<f64> = (0..cell_count as u32)
            .map(|cell| math::atan2(mesh.vertex(cell).y, mesh.vertex(cell).x))
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

    /// Solves the water cycle for the given surface elevation (m, sea level 0; land at or above
    /// 0), the evaporation of lakes beyond the land's evapotranspiration (m/yr over the mean cell
    /// area `4πR²/N`, as the water balance computes it; empty for none), monthly surface temperatures (°C; month 0 is January) and the
    /// thermal equator of each month (degrees).
    pub fn update(
        &mut self,
        elevation: &[f64],
        lake_evaporation: &[f64],
        monthly_temperature: &[Vec<f64>],
        itcz: &[Vec<f64>],
    ) {
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

        let lake_temperature: Vec<&[f64]> = months
            .iter()
            .map(|&month| &monthly_temperature[month][..])
            .collect();
        let mut iterations = vec![0; month_count];
        for _ in 0..rounds {
            for &slot in &slots {
                let thermal_equator: &[f64] = itcz.get(months[slot]).map_or(&[0.0], |s| s);
                let mut input = self.month_inputs(
                    elevation,
                    &monthly_temperature[months[slot]],
                    thermal_equator,
                    slot,
                );
                self.solved[slot].potential = input.potential.clone();
                self.solved[slot].lake_potential = input.lake_potential.clone();
                self.solved[slot].itcz = input.itcz;
                self.solved[slot].wind = self
                    .position
                    .iter()
                    .zip(&input.wind)
                    .map(|(&position, &wind)| east_north(position, wind))
                    .collect();
                self.set_evapotranspiration(elevation, slot, &mut input);
                let lake = self.add_lake_evaporation(
                    lake_evaporation,
                    &lake_temperature,
                    slot,
                    &mut input,
                );
                self.solved[slot].lake_evaporation = lake;
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
        cycle.potential_evaporation = (0..cell_count)
            .map(|cell| {
                self.solved
                    .iter()
                    .map(|m| m.potential.get(cell).copied().unwrap_or(0.0))
                    .sum::<f64>()
                    / month_count as f64
                    * to_metres_per_year
            })
            .collect();
        let annual_mean = |field: &dyn Fn(&SolvedMonth) -> &Vec<f64>| -> Vec<f64> {
            (0..cell_count)
                .map(|cell| {
                    self.solved
                        .iter()
                        .map(|m| field(m).get(cell).copied().unwrap_or(0.0))
                        .sum::<f64>()
                        / month_count as f64
                        * to_metres_per_year
                })
                .collect()
        };
        cycle.lake_potential_evaporation = annual_mean(&|m| &m.lake_potential);
        cycle.lake_evaporation = annual_mean(&|m| &m.lake_evaporation);
        cycle.total_lake_evaporation = cycle
            .lake_evaporation
            .iter()
            .zip(&self.area)
            .map(|(e, a)| e * a)
            .sum();
        cycle.runoff = (0..cell_count)
            .map(|cell| {
                if elevation[cell] < 0.0 {
                    0.0
                } else {
                    budyko_runoff(
                        cycle.precipitation[cell],
                        cycle.potential_evaporation[cell],
                        self.params.budyko_omega,
                    )
                }
            })
            .collect();
        let total_area: f64 = self.area.iter().sum();
        let area_mean = |field: &[f64]| {
            field
                .iter()
                .zip(&self.area)
                .map(|(v, a)| v * a)
                .sum::<f64>()
                / total_area
        };
        cycle.monthly_precipitation = self
            .solved
            .iter()
            .map(|m| {
                m.precipitation
                    .iter()
                    .map(|p| p * to_metres_per_year)
                    .collect()
            })
            .collect();
        cycle.monthly_evaporation = self
            .solved
            .iter()
            .map(|m| {
                m.evaporation
                    .iter()
                    .map(|e| e * to_metres_per_year)
                    .collect()
            })
            .collect();
        cycle.monthly_wind = self.solved.iter().map(|m| m.wind.clone()).collect();
        cycle.wind = (0..cell_count)
            .map(|cell| {
                let sum = self.solved.iter().fold([0.0; 2], |[e, n], m| {
                    let [east, north] = m.wind[cell];
                    [e + east, n + north]
                });
                sum.map(|component| component / month_count as f64)
            })
            .collect();
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
        thermal_equator: &[f64],
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
        let vapour = &self.vapour[slot];
        let mut saturation = vec![0.0; cell_count];
        let mut vapour_per_humidity = vec![0.0; cell_count];
        for cell in 0..cell_count {
            let (column, scale_height) =
                saturation_column(column_temperature[cell], params.lapse_rate);
            // Above high ground, the air column is shorter and colder.
            let height = math::exp(-elevation[cell].max(0.0) / scale_height);
            saturation[cell] = column * height;
            vapour_per_humidity[cell] = params.air_density * scale_height * height;
        }
        // The ITCZ lies where the moist static energy of the near-surface air peaks, which over
        // a hot, dry summer continent is equatorward of the temperature maximum. Before the first
        // solve, the thermal equator of the temperature model stands in.
        let moist_equator;
        let thermal_equator = if vapour.iter().any(|&w| w > 0.0) {
            moist_equator = self.moist_static_energy_equator(
                &|cell| {
                    AIR_HEAT_CAPACITY * sea_level(cell)
                        + LATENT_HEAT * vapour[cell] / vapour_per_humidity[cell]
                },
                thermal_equator.len().max(1),
            );
            &moist_equator[..]
        } else {
            thermal_equator
        };
        // The equator of the circulation over every cell, interpolated between the sectors.
        let local_equator: Vec<f64> = self
            .position
            .iter()
            .map(|p| {
                let sectors = thermal_equator.len();
                let x = (math::atan2(p.y, p.x) + std::f64::consts::PI) / std::f64::consts::TAU
                    * sectors as f64
                    - 0.5;
                let lower = x.floor();
                let f = x - lower;
                let index = |i: f64| (i as i64).rem_euclid(sectors as i64) as usize;
                thermal_equator[index(lower)] * (1.0 - f) + thermal_equator[index(lower + 1.0)] * f
            })
            .collect();
        let wind: Vec<DVec3> = (0..cell_count)
            .map(|cell| low_level_wind(self.position[cell], local_equator[cell]) + thermal[cell])
            .collect();

        let mut evaporation = vec![0.0; cell_count];
        let mut potential = vec![0.0; cell_count];
        let mut lake_potential = vec![0.0; cell_count];
        for cell in 0..cell_count {
            let speed = (wind[cell].length_squared() + params.wind_variability.powi(2)).sqrt();
            let transfer = params.air_density * params.moisture_exchange * speed;
            let saturated = saturation_specific_humidity(temperature[cell]);
            if elevation[cell] < 0.0 {
                let air = params.surface_humidity
                    * saturation_specific_humidity(
                        temperature[cell] - params.air_sea_temperature_difference,
                    );
                evaporation[cell] = transfer * (saturated - air).max(0.0);
                if params.ocean_lake_evaporation {
                    // A lake here (the erosion's surface differs at a sill) evaporates as open
                    // water.
                    lake_potential[cell] = evaporation[cell] * ice_free(temperature[cell]);
                }
            } else {
                // The specific humidity of the surface air follows from the column vapour, but
                // the air over a wet surface is no more humid than over the ocean.
                let air = (vapour[cell] / vapour_per_humidity[cell])
                    .min(params.surface_humidity * saturated);
                potential[cell] = transfer * (saturated - air);
                lake_potential[cell] = potential[cell] * ice_free(temperature[cell]);
            }
        }

        let diffusivity = self
            .position
            .iter()
            .zip(&local_equator)
            .map(|(position, &thermal_equator)| {
                let latitude = math::asin(position.z.clamp(-1.0, 1.0)).to_degrees();
                let distance =
                    circulation_latitude(latitude, thermal_equator) / params.hadley_width;
                // Weather systems also fade towards the poles, beyond the storm tracks.
                let polar = (90.0 - circulation_latitude(latitude, thermal_equator).abs())
                    / params.polar_width;
                params.eddy_diffusivity
                    * (1.0 - math::exp(-distance * distance))
                    * (1.0 - math::exp(-polar * polar))
            })
            .collect();
        Month {
            evaporation,
            potential,
            lake_potential,
            saturation,
            vapour_per_humidity,
            diffusivity,
            itcz: thermal_equator.iter().sum::<f64>() / thermal_equator.len().max(1) as f64,
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

    /// Adds the evaporation of lake water beyond the land's evapotranspiration (`lake`, annual
    /// m/yr per cell, from the lakes' water balance) to the evaporation of the month in `slot`.
    /// It is spread over the solved months (surface temperatures `temperature`, °C, one field
    /// per month) in proportion to [`lake_season`], which depends on the temperatures alone, so
    /// that the months' weights average to exactly one: once every month has been solved with
    /// the same `lake`, the annual mean returns what the balance removed. `lake` is in m/yr over
    /// the mean cell area; it is converted to the cell's own Voronoi area, so that the volumes
    /// match. Returns the added evaporation in kg/m²/s.
    fn add_lake_evaporation(
        &self,
        lake: &[f64],
        temperature: &[&[f64]],
        slot: usize,
        month: &mut Month,
    ) -> Vec<f64> {
        let mut added = vec![0.0; month.evaporation.len()];
        let mean_area = self.area.iter().sum::<f64>() / self.area.len() as f64;
        for (cell, &rate) in lake.iter().enumerate() {
            if rate <= 0.0 {
                continue;
            }
            let mean = temperature
                .iter()
                .map(|t| lake_season(t[cell]))
                .sum::<f64>()
                / temperature.len() as f64;
            // A lake frozen all year (or a cell that has become ocean since the water balance)
            // returns the water evenly over the year.
            let weight = if mean > 0.0 {
                lake_season(temperature[slot][cell]) / mean
            } else {
                1.0
            };
            added[cell] =
                rate * mean_area / self.area[cell] * WATER_DENSITY / SECONDS_PER_YEAR * weight;
            month.evaporation[cell] += added[cell];
        }
        added
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
                // Near the equator, where the balance would give unbounded winds, the convergence
                // into the ITCZ (which follows the local thermal equator) takes over.
                let latitude = math::asin(position.z.clamp(-1.0, 1.0)).to_degrees();
                let fade = 1.0 - math::exp(-(latitude / params.thermal_wind_equator).powi(2));
                let wind = (force * friction - position.cross(force) * coriolis)
                    / (friction * friction + coriolis * coriolis)
                    * fade;
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

    /// The latitude (degrees, between 30°S and 30°N) of the maximum of `energy` in each of
    /// `sectors` sectors of longitude (from 180°W eastwards), averaging over a Gaussian of 30° in
    /// longitude and 5° in latitude.
    fn moist_static_energy_equator(
        &self,
        energy: &dyn Fn(usize) -> f64,
        sectors: usize,
    ) -> Vec<f64> {
        const BINS: usize = 120;
        let mut sum = vec![[0.0; BINS]; sectors];
        let mut weight = vec![[0.0; BINS]; sectors];
        let width = 30f64.to_radians();
        for (cell, p) in self.position.iter().enumerate() {
            let latitude = self.latitude[cell].to_degrees();
            if latitude.abs() >= 30.0 {
                continue;
            }
            let bin = (((latitude + 30.0) * 2.0) as usize).min(BINS - 1);
            let longitude = math::atan2(p.y, p.x);
            let value = energy(cell);
            for sector in 0..sectors {
                let centre = (sector as f64 + 0.5) / sectors as f64 * std::f64::consts::TAU
                    - std::f64::consts::PI;
                let difference = (longitude - centre + std::f64::consts::PI)
                    .rem_euclid(std::f64::consts::TAU)
                    - std::f64::consts::PI;
                if difference.abs() > 2.0 * width {
                    continue;
                }
                let w = math::exp(-(difference / width).powi(2));
                sum[sector][bin] += w * value;
                weight[sector][bin] += w;
            }
        }
        (0..sectors)
            .map(|sector| {
                let smoothed = |bin: usize| {
                    let (mut s, mut c) = (0.0, 0.0);
                    for other in bin.saturating_sub(30)..(bin + 31).min(BINS) {
                        let d = (other as f64 - bin as f64) / 10.0;
                        let w = math::exp(-0.5 * d * d);
                        s += w * sum[sector][other];
                        c += w * weight[sector][other];
                    }
                    s / c.max(1e-12)
                };
                (0..BINS)
                    .map(|bin| (bin, smoothed(bin)))
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .map_or(0.0, |(bin, _)| bin as f64 / 2.0 - 30.0 + 0.25)
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
                    let w = math::exp(-0.5 * d * d);
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
        let factor = math::exp(self.sensitivity * (humidity - self.reference)) * self.rate;
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
    let root = math::powf(math::powf(p, omega) + math::powf(e, omega), 1.0 / omega);
    ((root - e) * scale).max(0.0)
}

/// The fraction of a month in which a lake is free of ice, at the monthly mean temperature `t`
/// (°C): lakes freeze over when the air stays below freezing, and ice cuts their evaporation
/// (sublimation is neglected). A linear ramp between −2 °C and +2 °C stands in for the
/// freezing and thawing within a month.
fn ice_free(t: f64) -> f64 {
    ((t + 2.0) / 4.0).clamp(0.0, 1.0)
}

/// The seasonal weight of lake evaporation at the monthly mean surface temperature `t` (°C):
/// the saturation specific humidity of the ice-free water surface, which sets how much it can
/// evaporate.
fn lake_season(t: f64) -> f64 {
    ice_free(t) * saturation_specific_humidity(t)
}

/// Saturation vapour pressure over water in Pa at `t` °C (Magnus formula, Alduchov & Eskridge,
/// 1996).
fn saturation_vapour_pressure(t: f64) -> f64 {
    let t = t.max(-80.0);
    610.94 * math::exp(17.625 * t / (t + 243.04))
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

/// The saturated air column of a free-troposphere temperature, for inverting the rain law of
/// [`ClimateParams`] (see [`ColumnSaturation::relative_humidity`]). The realistic view's clouds
/// (#92) use it; one per zonal temperature, since the column temperature is a zonal mean.
#[derive(Debug, Clone, Copy)]
pub struct ColumnSaturation {
    /// `ln(W_sat · rate) − a r₀` at sea level.
    offset: f64,
    /// Scale height of water vapour in m.
    scale_height: f64,
    /// `a` of the rain law.
    sensitivity: f64,
    /// `ln r₀`.
    log_reference: f64,
}

impl ColumnSaturation {
    /// The column of the free-troposphere temperature `column_temperature` (°C, the zonal mean of
    /// the sea-level temperature, as the water cycle uses it); `None` if it is not finite.
    pub fn new(column_temperature: f64, params: &ClimateParams) -> Option<Self> {
        if !column_temperature.is_finite() {
            return None;
        }
        let law = RainLaw::new(params);
        let (column, scale_height) = saturation_column(column_temperature, params.lapse_rate);
        Some(Self {
            offset: math::ln(column * law.rate) - law.sensitivity * law.reference,
            scale_height,
            sensitivity: law.sensitivity,
            log_reference: math::ln(law.reference),
        })
    }

    /// The column relative humidity `r = W / W_sat` at which the rain law gives the
    /// precipitation `precipitation` (m/yr) above a surface `elevation` m high: the inverse of
    /// [`RainLaw::rate`] with `W = r W_sat`. NaN for NaN inputs.
    ///
    /// `ln P(r) = ln r + ln(W_sat · rate) + a (min(r, 2) − r₀)` is concave and grows with r, so
    /// Newton's method converges monotonically after its first step, with one logarithm per
    /// step (2–3 steps from the first guess). Beyond twice saturation, where the law stops
    /// steepening, the humidity is large but finite.
    pub fn relative_humidity(&self, precipitation: f64, elevation: f64) -> f64 {
        if !(precipitation.is_finite() && elevation.is_finite()) {
            return f64::NAN;
        }
        // kg/m²/s; at least a trace, so that the logarithm stays finite.
        let target = precipitation.max(1e-4) * WATER_DENSITY / SECONDS_PER_YEAR;
        // The saturated column shrinks over high ground: ln W_sat drops by z / H.
        let offset = self.offset - elevation.max(0.0) / self.scale_height - math::ln(target);
        // First guess: the root with ln r frozen at the reference humidity (it varies slowly).
        let mut humidity = ((-offset - self.log_reference) / self.sensitivity).clamp(0.05, 2.0);
        for _ in 0..20 {
            let below_cap = humidity < 2.0;
            let residual = math::ln(humidity) + offset + self.sensitivity * humidity.min(2.0);
            let slope = 1.0 / humidity + if below_cap { self.sensitivity } else { 0.0 };
            let step = residual / slope;
            humidity = (humidity - step).clamp(1e-3, 50.0);
            // (Newton converges quadratically: the error after a step of 1e-4 is ~1e-8.)
            if step.abs() < 1e-4 {
                break;
            }
        }
        humidity
    }
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

/// The eastward and northward components of a vector tangent to the unit sphere at `position`.
fn east_north(position: DVec3, vector: DVec3) -> [f64; 2] {
    let east = DVec3::Z.cross(position);
    let east = if east.length_squared() < 1e-12 {
        DVec3::X
    } else {
        east.normalize()
    };
    let north = position.cross(east);
    [vector.dot(east), vector.dot(north)]
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
    let latitude = math::asin(position.z.clamp(-1.0, 1.0)).to_degrees();
    let circulation = circulation_latitude(latitude, thermal_equator);
    let (eastward, poleward) = zonal_mean_wind(circulation.abs());
    east * eastward + north * poleward * circulation.signum()
}

/// Areas (m²) of the Voronoi cells and their edges, from the circumcentres of the triangles.
fn voronoi_geometry(mesh: &SphereMesh, radius: f64) -> (Vec<f64>, Vec<Vec<Edge>>) {
    let cell_count = mesh.len();
    let mut corners: crate::hash::HashMap<(u32, u32), Vec<DVec3>> = Default::default();
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
                        length: math::angle_between(ends[0], ends[1]) * radius,
                        distance: math::angle_between(position, other) * radius,
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
            DVec3::new(math::cos(latitude), 0.0, math::sin(latitude))
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
    fn wind_components_are_east_and_north() {
        for (latitude, longitude) in [(15.0f64, 40.0f64), (-45.0, -120.0), (75.0, 170.0)] {
            let (lat, lon) = (latitude.to_radians(), longitude.to_radians());
            let position = DVec3::new(
                math::cos(lat) * math::cos(lon),
                math::cos(lat) * math::sin(lon),
                math::sin(lat),
            );
            let [east, north] = east_north(position, low_level_wind(position, 0.0));
            let (eastward, poleward) = zonal_mean_wind(latitude.abs());
            assert!((east - eastward).abs() < 1e-9, "{east} {eastward}");
            assert!((north - poleward * latitude.signum()).abs() < 1e-9);
        }
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
    fn column_relative_humidity_inverts_the_rain_law() {
        let column_relative_humidity = |p: f64, t: f64, z: f64, params: &ClimateParams| {
            ColumnSaturation::new(t, params)
                .map_or(f64::NAN, |column| column.relative_humidity(p, z))
        };
        // Forward: the precipitation of a column at humidity r (as the budget computes it, with
        // the saturation column over high ground), then back.
        let params = ClimateParams::default();
        let law = RainLaw::new(&params);
        for &(t, elevation) in &[(27.0, 0.0), (10.0, 0.0), (-20.0, 0.0), (5.0, 3000.0)] {
            let (column, scale_height) = saturation_column(t, params.lapse_rate);
            let saturation = column * math::exp(-elevation / scale_height);
            for &r in &[0.4, 0.6, 0.7, 0.8, 0.95, 1.2] {
                let (rate, _) = law.rate(r * saturation, saturation);
                let precipitation = rate * SECONDS_PER_YEAR / WATER_DENSITY;
                let back = column_relative_humidity(precipitation, t, elevation, &params);
                assert!(
                    (back - r).abs() < 1e-4,
                    "{t} °C, {elevation} m: {r} -> {back}"
                );
            }
        }
        // More rain at the same saturation means a more humid column; no rain a dry one.
        let wet = column_relative_humidity(2.0, 25.0, 0.0, &params);
        let dry = column_relative_humidity(0.1, 25.0, 0.0, &params);
        assert!(wet > dry);
        assert!(column_relative_humidity(0.0, 25.0, 0.0, &params) < dry);
        // The same rain over high ground (a smaller saturated column) is more humid.
        assert!(
            column_relative_humidity(0.5, 10.0, 2000.0, &params)
                > column_relative_humidity(0.5, 10.0, 0.0, &params)
        );
        assert!(column_relative_humidity(f64::NAN, 25.0, 0.0, &params).is_nan());
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
                let latitude = math::asin(mesh.vertex(c).z);
                -25.0 + 52.0 * math::cos(latitude) - 0.0065 * elevation[c as usize].max(0.0)
            })
            .collect()
    }

    fn solve(mesh: &SphereMesh, elevation: &[f64]) -> (Climate, Vec<f64>, Vec<f64>) {
        let mut climate = Climate::new(mesh, RADIUS, ClimateParams::default());
        let monthly = vec![temperatures(mesh, elevation); 12];
        climate.update(elevation, &[], &monthly, &vec![vec![0.0]; 12]);
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

        // The winds of the solved months and their mean: trades and westerlies.
        let cycle = climate.water_cycle();
        assert_eq!(cycle.monthly_wind.len(), ClimateParams::default().months);
        assert_eq!(cycle.wind.len(), mesh.len());
        for c in 0..mesh.len() {
            let latitude = climate.latitude[c].to_degrees().abs();
            let [east, _] = cycle.wind[c];
            if (10.0..20.0).contains(&latitude) {
                assert!(east < -3.0, "{latitude} {east}");
            } else if (40.0..55.0).contains(&latitude) {
                assert!(east > 4.0, "{latitude} {east}");
            }
        }
    }

    #[test]
    fn summer_continents_draw_air_in() {
        let mesh = SphereMesh::icosphere(5);
        let params = ClimateParams {
            thermal_wind_depth: 1500.0,
            ..ClimateParams::default()
        };
        let climate = Climate::new(&mesh, RADIUS, params);
        // A warm anomaly of 10 K centred at 20°N, 0°E with a radius of about 2000 km.
        let centre = DVec3::new(
            math::cos(20f64.to_radians()),
            0.0,
            math::sin(20f64.to_radians()),
        );
        let anomaly = |c: usize| {
            10.0 * math::exp(-(math::angle_between(climate.position[c], centre) / 0.3).powi(2))
        };
        let wind = climate.thermal_wind(&anomaly, &vec![0.0; mesh.len()]);
        // Inflow: the wind points towards the centre on its flanks.
        let mut inflow = 0.0;
        let mut count = 0.0;
        for c in 0..mesh.len() {
            let distance = math::angle_between(climate.position[c], centre);
            if (0.25..0.4).contains(&distance) {
                let towards = (centre - climate.position[c]).normalize();
                inflow += wind[c].dot(towards);
                count += 1.0;
            }
        }
        let inflow = inflow / count;
        assert!((0.2..5.0).contains(&inflow), "{inflow}");
        assert!(wind.iter().all(|w| w.length() <= 10.0 + 1e-9));
    }

    #[test]
    fn rain_belt_follows_the_warmest_air() {
        let mesh = SphereMesh::icosphere(5);
        let ocean = vec![-4000.0; mesh.len()];
        let mut climate = Climate::new(&mesh, RADIUS, ClimateParams::default());
        // The warmest (and moistest) air at 10°N.
        let warm: Vec<f64> = (0..mesh.len() as u32)
            .map(|c| -25.0 + 52.0 * math::cos(math::asin(mesh.vertex(c).z) - 10f64.to_radians()))
            .collect();
        climate.update(&ocean, &[], &vec![warm; 12], &vec![vec![10.0]; 12]);
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

    /// The continent of [`continents_conserve_water_rain_shadow_and_fetch`] without mountains,
    /// longitudes (degrees) and latitudes of the cells.
    fn plain_continent(mesh: &SphereMesh) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let longitude: Vec<f64> = (0..mesh.len() as u32)
            .map(|c| math::atan2(mesh.vertex(c).y, mesh.vertex(c).x).to_degrees())
            .collect();
        let latitude: Vec<f64> = (0..mesh.len() as u32)
            .map(|c| math::asin(mesh.vertex(c).z).to_degrees())
            .collect();
        let elevation = (0..mesh.len())
            .map(|c| {
                if (35.0..55.0).contains(&latitude[c]) && (-40.0..40.0).contains(&longitude[c]) {
                    200.0
                } else {
                    -4000.0
                }
            })
            .collect();
        (elevation, longitude, latitude)
    }

    #[test]
    fn lakes_are_a_moisture_source() {
        let mesh = SphereMesh::icosphere(5);
        let (elevation, longitude, latitude) = plain_continent(&mesh);
        let monthly = vec![temperatures(&mesh, &elevation); 12];
        let solve = |lake: &[f64]| {
            let mut climate = Climate::new(&mesh, RADIUS, ClimateParams::default());
            climate.update(&elevation, lake, &monthly, &vec![vec![0.0]; 12]);
            climate
        };
        // A large lake in the westerlies, evaporating 1 m/yr beyond the land's
        // evapotranspiration.
        let lake: Vec<f64> = (0..mesh.len())
            .map(|c| {
                let inside =
                    (43.0..47.0).contains(&latitude[c]) && (-24.0..-16.0).contains(&longitude[c]);
                if inside { 1.0 } else { 0.0 }
            })
            .collect();
        let dry = solve(&[]);
        let wet = solve(&lake);
        let total = |climate: &Climate, field: &[f64]| -> f64 {
            field.iter().zip(&climate.area).map(|(v, a)| v * a).sum()
        };
        // The climate returns exactly the lake evaporation it was given (m/yr over the mean
        // cell area).
        let cycle = wet.water_cycle();
        let given = lake.iter().sum::<f64>() * 4.0 * std::f64::consts::PI * RADIUS * RADIUS
            / mesh.len() as f64;
        assert!((cycle.total_lake_evaporation / given - 1.0).abs() < 1e-9);
        // Water is conserved with the extra source.
        let (p, e) = (
            total(&wet, &cycle.precipitation),
            total(&wet, &cycle.evaporation),
        );
        assert!((p / e - 1.0).abs() < 0.01, "ΣP {p:e} vs ΣE {e:e}");
        // More rain downwind (east) of the lake.
        let downwind = |climate: &Climate| {
            let cells: Vec<f64> = (0..mesh.len())
                .filter(|&c| {
                    (42.0..48.0).contains(&latitude[c]) && (-14.0..0.0).contains(&longitude[c])
                })
                .map(|c| climate.water_cycle().precipitation[c])
                .collect();
            cells.iter().sum::<f64>() / cells.len() as f64
        };
        assert!(
            downwind(&wet) > 1.05 * downwind(&dry),
            "{} vs {}",
            downwind(&wet),
            downwind(&dry)
        );
        // Later updates refresh one month each: twice the lake evaporation reaches the air in
        // full once all four months have been solved again.
        let mut climate = wet;
        let double: Vec<f64> = lake.iter().map(|e| 2.0 * e).collect();
        climate.update(&elevation, &double, &monthly, &vec![vec![0.0]; 12]);
        let partial = climate.water_cycle().total_lake_evaporation / given;
        assert!(partial > 1.0 && partial < 2.0, "{partial}");
        for _ in 0..3 {
            climate.update(&elevation, &double, &monthly, &vec![vec![0.0]; 12]);
        }
        let full = climate.water_cycle().total_lake_evaporation / given;
        assert!((full - 2.0).abs() < 1e-9, "{full}");

        // A frozen lake: no potential evaporation, but the climate still returns the water.
        let frozen = vec![vec![-20.0; mesh.len()]; 12];
        let mut climate = Climate::new(&mesh, RADIUS, ClimateParams::default());
        climate.update(&elevation, &lake, &frozen, &vec![vec![0.0]; 12]);
        let cycle = climate.water_cycle();
        assert!(cycle.lake_potential_evaporation.iter().all(|&e| e == 0.0));
        assert!((cycle.total_lake_evaporation / given - 1.0).abs() < 1e-9);
    }

    #[test]
    fn lake_ice_stops_evaporation() {
        assert_eq!(ice_free(-5.0), 0.0);
        assert_eq!(ice_free(0.0), 0.5);
        assert_eq!(ice_free(5.0), 1.0);
    }

    #[test]
    fn continents_conserve_water_rain_shadow_and_fetch() {
        let mesh = SphereMesh::icosphere(5);
        // A continent in the westerlies with a north-south mountain range.
        let mut elevation = vec![-4000.0; mesh.len()];
        let longitude = |c: u32| math::atan2(mesh.vertex(c).y, mesh.vertex(c).x).to_degrees();
        let latitude = |c: u32| math::asin(mesh.vertex(c).z).to_degrees();
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
