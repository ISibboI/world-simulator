//! The planet: tectonics coupled with the water cycle and erosion.

use crate::climate::{Climate, ClimateParams};
use crate::erosion::{ErosionParams, erode};
use crate::tectonics::{Simulation, Statistics};

pub struct Planet {
    tectonics: Simulation,
    climate: Climate,
    /// `None` disables erosion.
    erosion: Option<ErosionParams>,
    planet_radius: f64,
    /// Precipitation per cell in m/yr.
    precipitation: Vec<f64>,
    /// Water discharge per cell in m³/yr; all zero while erosion is disabled.
    discharge: Vec<f64>,
    /// Sediment delivered to the oceans in the last step, in m³/yr.
    sediment_flux: f64,
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
}

impl Planet {
    pub fn new(
        tectonics: Simulation,
        climate: ClimateParams,
        erosion: Option<ErosionParams>,
        planet_radius: f64,
    ) -> Self {
        let climate = Climate::new(tectonics.mesh(), climate);
        let cell_count = tectonics.mesh().len();
        let mut planet = Self {
            tectonics,
            climate,
            erosion,
            planet_radius,
            precipitation: vec![0.0; cell_count],
            discharge: vec![0.0; cell_count],
            sediment_flux: 0.0,
        };
        planet.precipitation = planet.compute_precipitation(&planet.tectonics.surface_elevations());
        planet
    }

    pub fn tectonics(&self) -> &Simulation {
        &self.tectonics
    }

    pub fn precipitation(&self) -> &[f64] {
        &self.precipitation
    }

    pub fn discharge(&self) -> &[f64] {
        &self.discharge
    }

    /// Advances the planet by `dt` Myr: plate motion first, then one erosion step.
    pub fn step(&mut self, dt: f64) {
        self.tectonics.step(dt);
        let elevation = self.tectonics.surface_elevations();
        self.precipitation = self.compute_precipitation(&elevation);

        if let Some(params) = &self.erosion {
            let dt_years = dt * 1e6;
            let result = erode(
                self.tectonics.mesh(),
                &elevation,
                &self.tectonics.sediment_thicknesses(),
                &self.precipitation,
                params,
                dt_years,
            );
            self.tectonics
                .apply_surface_changes(&result.bedrock_delta, &result.sediment_delta);
            self.discharge = result.discharge;
            self.sediment_flux = result.sediment_to_ocean / dt_years;
        }
    }

    fn compute_precipitation(&self, elevation: &[f64]) -> Vec<f64> {
        let cell_spacing = self.tectonics.mesh().mean_edge_angle() * self.planet_radius;
        self.climate.precipitation(elevation, cell_spacing)
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

        PlanetStatistics {
            tectonics: self.tectonics.statistics(),
            mean_land_precipitation,
            largest_river,
            sediment_flux: self.sediment_flux,
        }
    }
}
