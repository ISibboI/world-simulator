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
    /// Median, 90th percentile and maximum age of oceanic crust in Myr.
    pub oceanic_age_percentiles: [f64; 3],
    /// Fraction of the surface deeper than 6000 m.
    pub deep_fraction: f64,
    /// Fraction of the oceanic crust older than 150 Myr.
    pub old_ocean_fraction: f64,
    /// Mean surface speed over oceanic and over continental crust in rad/Myr.
    pub mean_oceanic_speed: f64,
    pub mean_continental_speed: f64,
    /// Fastest plate surface speed in rad/Myr.
    pub max_speed: f64,
    /// Mean surface elevation of cells above sea level.
    pub mean_land_elevation: f64,
    pub max_elevation: f64,
    pub min_elevation: f64,
    /// Mean surface speed in rad/Myr (multiply by the planet radius for a linear speed).
    pub mean_speed: f64,
    /// Number of cells covered by each plate.
    pub plate_areas: Vec<usize>,
    /// Number of active mantle plumes.
    pub plumes: usize,
    /// How uncovered cells were filled so far.
    pub gaps: GapStatistics,
    /// Plate births and deaths so far, by mechanism.
    pub events: EventCounts,
    /// Length of trenches and ridges in km.
    pub trench_length: f64,
    pub ridge_length: f64,
    /// Total magnitude of slab pull and ridge push in N.
    pub slab_pull: f64,
    pub ridge_push: f64,
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
        let mut oceanic_ages = Vec::new();
        let mut deep = 0;
        let (mut oceanic_speed, mut continental_speed) = (0.0, 0.0);
        let mut max_speed: f64 = 0.0;

        for cell in 0..cell_count as u32 {
            let plate = self.owner(cell).plate as usize;
            plate_areas[plate] += 1;
            let speed = self.plates[plate]
                .angular_velocity
                .cross(self.mesh.vertex(cell))
                .length();
            speed_sum += speed;
            max_speed = max_speed.max(speed);
            let crust = self.surface_crust(cell);
            if crust.is_continental() {
                continental += 1;
                continental_speed += speed;
            } else if crust.is_oceanic() {
                oceanic_age_sum += crust.age;
                oceanic_ages.push(crust.age);
                oceanic_speed += speed;
            }
            let elevation = crust.surface_elevation();
            if elevation < -6000.0 {
                deep += 1;
            }
            if elevation > 0.0 {
                land += 1;
                land_elevation_sum += elevation;
            }
            max_elevation = max_elevation.max(elevation);
            min_elevation = min_elevation.min(elevation);
        }

        let oceanic = oceanic_ages.len();
        let boundary_length = self.params.planet_radius * self.mesh.mean_edge_angle() / 1000.0;
        let (mut trench_length, mut ridge_length, mut slab_pull, mut ridge_push) =
            (0.0, 0.0, 0.0, 0.0);
        for boundary in self.boundary_cells() {
            let force = self.boundary_force(&boundary).length();
            match boundary.kind {
                super::dynamics::BoundaryKind::Trench { .. } => {
                    trench_length += boundary_length;
                    slab_pull += force;
                }
                super::dynamics::BoundaryKind::Ridge => {
                    ridge_length += boundary_length;
                    ridge_push += force;
                }
                _ => {}
            }
        }
        oceanic_ages.sort_by(f64::total_cmp);
        let percentile = |p: f64| {
            if oceanic_ages.is_empty() {
                0.0
            } else {
                oceanic_ages[((oceanic_ages.len() - 1) as f64 * p).round() as usize]
            }
        };
        Statistics {
            oceanic_age_percentiles: [percentile(0.5), percentile(0.9), percentile(1.0)],
            deep_fraction: deep as f64 / cell_count as f64,
            old_ocean_fraction: oceanic_ages.iter().filter(|&&age| age > 150.0).count() as f64
                / oceanic_ages.len().max(1) as f64,
            mean_oceanic_speed: oceanic_speed / oceanic.max(1) as f64,
            mean_continental_speed: continental_speed / continental.max(1) as f64,
            max_speed,
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
            plumes: self.plumes().len(),
            gaps: self.gap_statistics.clone(),
            events: self.event_counts.clone(),
            trench_length,
            ridge_length,
            slab_pull,
            ridge_push,
            land_masses: components(&self.mesh, |cell| {
                self.surface_crust(cell).surface_elevation() >= 0.0
            }),
            continental_masses: components(&self.mesh, |cell| {
                self.surface_crust(cell).is_continental()
            }),
        }
    }
}
