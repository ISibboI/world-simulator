//! Aggregate statistics of the tectonic state.

use crate::sphere::SphereMesh;

use super::*;

/// Aggregate statistics of the current simulation state.
#[derive(Clone, Debug)]
pub struct Statistics {
    pub time: f64,
    pub continental_fraction: f64,
    pub land_fraction: f64,
    pub mean_oceanic_age: f64,
    /// Mean surface elevation of cells above sea level.
    pub mean_land_elevation: f64,
    pub max_elevation: f64,
    pub min_elevation: f64,
    /// Mean surface speed in rad/Myr (multiply by the planet radius for a linear speed).
    pub mean_speed: f64,
    /// Number of cells covered by each plate.
    pub plate_areas: Vec<usize>,
    /// Connected land masses (cells at or above sea level).
    pub land_masses: ComponentStatistics,
    /// Connected areas of continental crust.
    pub continental_masses: ComponentStatistics,
}

/// Sizes of the connected components of a set of cells.
#[derive(Clone, Debug, Default)]
pub struct ComponentStatistics {
    pub count: usize,
    /// Components of at most three cells.
    pub specks: usize,
    /// Fraction of the cells of the set that belong to its largest component.
    pub largest_share: f64,
}

/// Connected components of the cells for which `include` is true.
pub fn components(mesh: &SphereMesh, include: impl Fn(u32) -> bool) -> ComponentStatistics {
    let mut visited = vec![false; mesh.len()];
    let mut sizes = Vec::new();
    let mut stack = Vec::new();
    for start in 0..mesh.len() as u32 {
        if visited[start as usize] || !include(start) {
            continue;
        }
        visited[start as usize] = true;
        stack.push(start);
        let mut size = 0;
        while let Some(cell) = stack.pop() {
            size += 1;
            for &neighbor in mesh.neighbors(cell) {
                if !visited[neighbor as usize] && include(neighbor) {
                    visited[neighbor as usize] = true;
                    stack.push(neighbor);
                }
            }
        }
        sizes.push(size);
    }
    let total: usize = sizes.iter().sum();
    ComponentStatistics {
        count: sizes.len(),
        specks: sizes.iter().filter(|&&size| size <= 3).count(),
        largest_share: sizes
            .iter()
            .max()
            .map_or(0.0, |&max| max as f64 / total as f64),
    }
}

impl Simulation {
    pub fn statistics(&self) -> Statistics {
        let cell_count = self.mesh.len();
        let mut plate_areas = vec![0; self.plates.len()];
        let mut continental = 0;
        let mut land = 0;
        let mut oceanic_age_sum = 0.0;
        let mut max_elevation = f64::NEG_INFINITY;
        let mut min_elevation = f64::INFINITY;
        let mut speed_sum = 0.0;
        let mut land_elevation_sum = 0.0;

        for cell in 0..cell_count as u32 {
            let plate = self.owner(cell).plate as usize;
            plate_areas[plate] += 1;
            speed_sum += self.plates[plate]
                .angular_velocity
                .cross(self.mesh.vertex(cell))
                .length();
            let crust = self.surface_crust(cell);
            if crust.is_continental() {
                continental += 1;
            } else {
                oceanic_age_sum += crust.age;
            }
            let elevation = crust.surface_elevation();
            if elevation > 0.0 {
                land += 1;
                land_elevation_sum += elevation;
            }
            max_elevation = max_elevation.max(elevation);
            min_elevation = min_elevation.min(elevation);
        }

        let oceanic = cell_count - continental;
        Statistics {
            time: self.time,
            continental_fraction: continental as f64 / cell_count as f64,
            land_fraction: land as f64 / cell_count as f64,
            mean_oceanic_age: if oceanic > 0 {
                oceanic_age_sum / oceanic as f64
            } else {
                0.0
            },
            mean_land_elevation: if land > 0 {
                land_elevation_sum / land as f64
            } else {
                0.0
            },
            max_elevation,
            min_elevation,
            mean_speed: speed_sum / cell_count as f64,
            plate_areas,
            land_masses: components(&self.mesh, |cell| {
                self.surface_crust(cell).surface_elevation() >= 0.0
            }),
            continental_masses: components(&self.mesh, |cell| {
                self.surface_crust(cell).is_continental()
            }),
        }
    }
}
