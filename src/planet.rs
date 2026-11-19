//! The planet: tectonics coupled with the water cycle and erosion.

use std::time::Instant;

use tracing::{debug, debug_span};

use crate::climate::{Climate, ClimateParams};
use crate::erosion::{CrustClass, ErosionParams, Surface, WaveErosion, erode};
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
    /// Sediment piled above the delta top in overfilled basins so far (thickness over one cell,
    /// summed).
    sediment_overfill: f64,
    /// Sediment deposited in the ocean per zone in the last step, in m³/yr.
    sediment_deposition: [f64; 4],
    /// Sediment moved by waves so far (rock-equivalent thickness over one cell, summed).
    wave_erosion: WaveErosion,
    /// Sediment delivered to the oceans so far (m³).
    sediment_delivered: f64,
}

/// Statistics of the planet, extending the tectonic statistics.
pub struct PlanetStatistics {
    pub tectonics: Statistics,
    /// Mean precipitation over land in m/yr.
    pub mean_land_precipitation: f64,
    /// Discharge of the largest river mouth in m³/yr.
    pub largest_river: f64,
    /// Sediment delivered to the oceans in m³/yr.
    pub sediment_flux: f64,
    /// Sediment piled above the delta top in overfilled basins so far (thickness over one cell,
    /// summed).
    pub sediment_overfill: f64,
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
    /// Fraction of the surface with an annual mean temperature below -10 °C (permanent ice).
    pub frozen_fraction: f64,
    /// Where the sediment sits and how much of the sea level it makes up.
    pub sediment_reservoirs: SedimentReservoirs,
    /// Sediment moved by waves so far, in Gt.
    pub wave_erosion: WaveErosion,
    /// Sediment delivered to the oceans by rivers so far, in Gt.
    pub sediment_delivered: f64,
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
    /// Sediment on cells above the storm wave base (land included).
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
        let climate = Climate::new(tectonics.mesh(), climate);
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
            sediment_overfill: 0.0,
            sediment_deposition: [0.0; 4],
            wave_erosion: WaveErosion::default(),
            sediment_delivered: 0.0,
        };
        planet.precipitation = planet.compute_precipitation(&planet.tectonics.surface_elevations());
        planet
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
        self.precipitation = self.compute_precipitation(&elevation);
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
                    precipitation: &self.precipitation,
                    plate: &plate,
                    class: &class,
                    ocean_volume: self.tectonics.ocean_volume(),
                },
                params,
                dt_years,
            );
            self.tectonics.apply_surface_changes(
                &result.bedrock_delta,
                &result.sediment_delta,
                result.sea_level_change,
            );
            self.discharge = result.discharge;
            self.sediment_flux = result.sediment_to_ocean / dt_years;
            self.sediment_overfill += result.sediment_overfill;
            self.sediment_delivered += result.sediment_to_ocean;
            let wave = &mut self.wave_erosion;
            wave.cliff_sediment += result.wave.cliff_sediment;
            wave.cliff_bedrock += result.wave.cliff_bedrock;
            wave.reworked += result.wave.reworked;
            wave.stayed += result.wave.stayed;
            let cell_area = self.cell_area();
            self.sediment_deposition = result.deposited.map(|d| d * cell_area / dt_years);
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

    fn compute_precipitation(&self, elevation: &[f64]) -> Vec<f64> {
        let cell_spacing = self.tectonics.mesh().mean_edge_angle() * self.planet_radius;
        self.climate.precipitation(
            elevation,
            cell_spacing,
            Some(&self.temperature.fields().sea_level_annual_mean),
        )
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
        let latitude = |cell: usize| mesh.vertex(cell as u32).z.asin().to_degrees();
        let mean = |cells: &mut dyn Iterator<Item = usize>, value: &dyn Fn(usize) -> f64| {
            let (sum, count) =
                cells.fold((0.0, 0usize), |(sum, count), c| (sum + value(c), count + 1));
            if count == 0 { 0.0 } else { sum / count as f64 }
        };
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
            frozen_fraction: all().filter(|&c| fields.annual_mean[c] < -10.0).count() as f64
                / elevation.len() as f64,
            tectonics: self.tectonics.statistics(),
            mean_land_precipitation,
            largest_river,
            sediment_flux: self.sediment_flux,
            sediment_overfill: self.sediment_overfill,
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
        }
    }

    fn sediment_reservoirs(&self, elevation: &[f64]) -> SedimentReservoirs {
        let tectonics = &self.tectonics;
        let part =
            |select: &dyn Fn(&Crust, f64) -> bool| tectonics.sediment_sea_level_of(select, false);
        let filled_basin = |crust: &Crust| crust.is_continental_origin() && crust.felsic < 0.8;
        let wave_base = self
            .erosion
            .as_ref()
            .map_or(ErosionParams::default().wave_base, |params| {
                params.wave_base
            });
        let (cell_sediment, all_sediment) = tectonics.sediment_totals();
        let cell_count = elevation.len();
        let land = elevation.iter().filter(|&&e| e >= 0.0).count();
        let low_land = elevation
            .iter()
            .filter(|&&e| (0.0..10.0).contains(&e))
            .count();
        let (mut shelf, mut shelf_sediment, mut shelf_oceanic, mut shelf_basins) = (0, 0.0, 0, 0);
        let mut basins = 0;
        for (cell, &e) in elevation.iter().enumerate() {
            let crust = tectonics.surface_crust(cell as u32);
            if filled_basin(&crust) {
                basins += 1;
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
            above_wave_base: part(&|_, e| e >= wave_base),
            with_metasediment: tectonics.sediment_sea_level_of(|_, _| true, true),
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
