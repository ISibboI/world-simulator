//! The planet: tectonics coupled with the water cycle and erosion.

use crate::math;
use web_time::Instant;

use tracing::{debug, debug_span};

use crate::climate::{Climate, ClimateParams, WaterCycle};
use crate::erosion::{CrustClass, ErosionParams, Hydrology, Surface, WaveErosion, erode};
use crate::tectonics::{Crust, Simulation, Statistics};
use crate::temperature::{TemperatureFields, TemperatureModel, TemperatureParams};

pub struct Planet {
    tectonics: Simulation,
    climate: Climate,
    temperature: TemperatureModel,
    /// `None` disables erosion.
    erosion: Option<ErosionParams>,
    planet_radius: f64,
    /// Precipitation per cell in m/yr.
    precipitation: Vec<f64>,
    /// Water discharge per cell in m³/yr; all zero while erosion is disabled.
    discharge: Vec<f64>,
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
    /// When to start tracking the continental cover as a cohort (Myr), and once started, the
    /// time and the cohort's initial mass (Gt).
    cover_cohort_time: f64,
    cover_cohort: Option<(f64, f64)>,
}

/// The continental cover of [`CoverCohort::start`] and how much of it is left (Gt).
#[derive(Clone, Copy, Debug)]
pub struct CoverCohort {
    pub start: f64,
    pub initial: f64,
    /// Left as sediment on crust whose basement is of continental origin.
    pub on_continents: f64,
    /// Left as sediment anywhere.
    pub anywhere: f64,
}

/// Gross sediment turnover on crust of continental origin (the continental sedimentary cover)
/// by surface processes: what left and what arrived at its cells, net per cell and step.
#[derive(Clone, Copy, Debug, Default)]
pub struct CoverTurnover {
    pub eroded: f64,
    pub deposited: f64,
}

/// Statistics of the planet, extending the tectonic statistics.
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

impl Planet {
    pub fn new(
        tectonics: Simulation,
        climate: ClimateParams,
        temperature: TemperatureParams,
        erosion: Option<ErosionParams>,
        planet_radius: f64,
    ) -> Self {
        let climate = Climate::new(tectonics.mesh(), planet_radius, climate);
        let mut temperature = TemperatureModel::new(tectonics.mesh(), temperature);
        temperature.update(&tectonics.surface_elevations(), None);
        let cell_count = tectonics.mesh().len();
        let mut planet = Self {
            tectonics,
            climate,
            temperature,
            erosion,
            planet_radius,
            precipitation: vec![0.0; cell_count],
            discharge: vec![0.0; cell_count],
            sediment_flux: 0.0,
            hydrology: Hydrology::default(),
            sediment_overfill: 0.0,
            terminal_fans: 0.0,
            sediment_deposition: [0.0; 4],
            wave_erosion: WaveErosion::default(),
            sediment_delivered: 0.0,
            cover_turnover: CoverTurnover::default(),
            cover_cohort_time: f64::INFINITY,
            cover_cohort: None,
        };
        planet.update_water_cycle(&planet.tectonics.surface_elevations());
        planet
    }

    /// Starts tracking the continental cover as a cohort at the simulated time `time` (Myr), to
    /// measure how long it is preserved (see [`Simulation::start_cover_cohort`]).
    pub fn track_cover_cohort_from(&mut self, time: f64) {
        self.cover_cohort_time = time;
    }

    pub fn tectonics(&self) -> &Simulation {
        &self.tectonics
    }

    pub fn temperature(&self) -> &TemperatureFields {
        self.temperature.fields()
    }

    pub fn precipitation(&self) -> &[f64] {
        &self.precipitation
    }

    pub fn discharge(&self) -> &[f64] {
        &self.discharge
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
        }
        hash
    }

    /// Advances the planet by `dt` Myr: plate motion first, then one erosion step.
    pub fn step(&mut self, dt: f64) {
        let _span = debug_span!("step", time = self.tectonics.time() + dt).entered();
        let start = Instant::now();
        self.tectonics.step(dt);
        let tectonics_time = start.elapsed();

        let start = Instant::now();
        let elevation = self.tectonics.surface_elevations();
        // Ice sheets depend on last step's snowfall.
        self.temperature
            .update(&elevation, Some(&self.precipitation));
        self.update_water_cycle(&elevation);
        let climate_time = start.elapsed();

        if let Some(params) = &self.erosion {
            let dt_years = dt * 1e6;
            let (plate, class) = self.plates_and_classes();
            let result = erode(
                self.tectonics.mesh(),
                &Surface {
                    elevation: &elevation,
                    sediment: &self.tectonics.sediment_thicknesses(),
                    bedrock_density: &self.tectonics.bedrock_densities(),
                    bedrock_isostatic_density: &self.tectonics.bedrock_isostatic_densities(),
                    dense_crust: &self.tectonics.dense_crust_thicknesses(),
                    precipitation: &self.precipitation,
                    runoff: &self.climate.water_cycle().runoff,
                    potential_evaporation: &self.climate.water_cycle().potential_evaporation,
                    plate: &plate,
                    class: &class,
                    ocean_volume: self.tectonics.ocean_volume(),
                },
                params,
                dt_years,
            );
            for (delta, class) in result.sediment_delta.iter().zip(&class) {
                if *class == CrustClass::Continental {
                    if *delta < 0.0 {
                        self.cover_turnover.eroded -= delta;
                    } else {
                        self.cover_turnover.deposited += delta;
                    }
                }
            }
            self.tectonics.apply_surface_changes(
                &result.bedrock_delta,
                &result.sediment_delta,
                result.sea_level_change,
            );
            self.discharge = result.discharge;
            self.sediment_flux = result.sediment_to_ocean / dt_years;
            self.hydrology = result.hydrology;
            self.sediment_overfill += result.sediment_overfill;
            self.terminal_fans += result.terminal_fan;
            self.sediment_delivered += result.sediment_to_ocean;
            let wave = &mut self.wave_erosion;
            wave.cliff_sediment += result.wave.cliff_sediment;
            wave.cliff_bedrock += result.wave.cliff_bedrock;
            wave.reworked += result.wave.reworked;
            wave.stayed += result.wave.stayed;
            let cell_area = self.cell_area();
            self.sediment_deposition = result.deposited.map(|d| d * cell_area / dt_years);
        }
        if self.cover_cohort.is_none() && self.tectonics.time() >= self.cover_cohort_time - 1e-9 {
            let mass = self.tectonics.start_cover_cohort();
            self.cover_cohort = Some((self.tectonics.time(), mass));
        }
        debug!(
            ?tectonics_time,
            ?climate_time,
            erosion_time = ?start.elapsed() - climate_time,
            "Step finished"
        );
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

    fn update_water_cycle(&mut self, elevation: &[f64]) {
        let fields = self.temperature.fields();
        self.climate
            .update(elevation, &fields.monthly, &fields.thermal_equator);
        let cycle = self.climate.water_cycle();
        debug!(iterations = ?cycle.iterations, clipped = cycle.clipped, "Water cycle solved");
        self.precipitation = cycle.precipitation.clone();
    }

    pub fn water_cycle(&self) -> &WaterCycle {
        self.climate.water_cycle()
    }

    pub fn statistics(&self) -> PlanetStatistics {
        let elevation = self.tectonics.surface_elevations();
        let land: Vec<usize> = (0..elevation.len())
            .filter(|&cell| elevation[cell] >= 0.0)
            .collect();
        let mean_land_precipitation = if land.is_empty() {
            0.0
        } else {
            land.iter().map(|&c| self.precipitation[c]).sum::<f64>() / land.len() as f64
        };
        let largest_river = (0..elevation.len())
            .filter(|&cell| elevation[cell] < 0.0)
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
            sediment_reservoirs: self.sediment_reservoirs(&elevation),
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
            cover_cohort: self.cover_cohort.map(|(start, initial)| {
                let (on_continents, anywhere) = self.tectonics.cover_cohort();
                CoverCohort {
                    start,
                    initial,
                    on_continents,
                    anywhere,
                }
            }),
            cover_turnover: {
                let to_gt = cell_area * crate::isostasy::SEDIMENT_ROCK_DENSITY / 1e12;
                CoverTurnover {
                    eroded: self.cover_turnover.eroded * to_gt,
                    deposited: self.cover_turnover.deposited * to_gt,
                }
            },
        }
    }

    fn sediment_reservoirs(&self, elevation: &[f64]) -> SedimentReservoirs {
        let tectonics = &self.tectonics;
        let part = |select: &dyn Fn(&Crust, f64) -> bool| {
            tectonics.sediment_sea_level_of(|_, c, e| select(c, e), false)
        };
        let filled_basin = |crust: &Crust| crust.is_continental_origin() && crust.felsic < 0.8;
        let default = ErosionParams::default();
        let wave_base = crate::erosion::wave_bases(
            tectonics.mesh(),
            elevation,
            self.erosion.as_ref().unwrap_or(&default),
        );
        let (cell_sediment, all_sediment) = tectonics.sediment_totals();
        let cell_count = elevation.len();
        let land = elevation.iter().filter(|&&e| e >= 0.0).count();
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
            if (-200.0..0.0).contains(&e) {
                shelf += 1;
                shelf_sediment += crust.sediment;
                shelf_oceanic += usize::from(crust.is_oceanic());
                shelf_basins += usize::from(filled_basin(&crust));
            }
        }
        SedimentReservoirs {
            all: part(&|_, _| true),
            continental_land: part(&|c, e| c.is_continental_origin() && e >= 0.0),
            continental_flooded: part(&|c, e| c.is_continental_origin() && e < 0.0),
            transitional: part(&|c, _| crust_class(c) == CrustClass::Transitional),
            oceanic: part(&|c, _| crust_class(c) == CrustClass::Oceanic),
            marine: part(&|_, e| e < 0.0),
            filled_basins: part(&|c, _| filled_basin(c)),
            filled_basin_fraction: basins as f64 / cell_count as f64,
            metasediment_basin_fraction: metasediment_basins as f64 / cell_count as f64,
            above_wave_base: tectonics.sediment_sea_level_of(
                |cell, _, e| e < 0.0 && e >= wave_base[cell as usize],
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
