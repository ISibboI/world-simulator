//! The planet: tectonics coupled with the water cycle and erosion.

use std::time::Instant;

use tracing::{debug, debug_span};

use crate::climate::{Climate, ClimateParams};
use crate::erosion::{CrustClass, ErosionParams, Surface, erode};
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
                },
                params,
                dt_years,
            );
            self.tectonics
                .apply_surface_changes(&result.bedrock_delta, &result.sediment_delta);
            self.discharge = result.discharge;
            self.sediment_flux = result.sediment_to_ocean / dt_years;
            self.sediment_overfill += result.sediment_overfill;
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
