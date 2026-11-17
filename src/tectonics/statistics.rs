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
    /// Where crust mass came from and went to so far.
    pub ledger: MassLedger,
    /// How plates were matched to cells so far.
    pub matching: MatchingStatistics,
    /// Crust mass in Gt: crystalline crust that is continental, transitional and oceanic,
    /// sediment, felsic crust (density < 2850 kg/m³), and crust hidden below other plates.
    pub continental_mass: f64,
    pub transitional_mass: f64,
    pub oceanic_mass: f64,
    pub sediment_mass: f64,
    pub felsic_mass: f64,
    pub hidden_mass: f64,
    /// Length of trenches and of ridge axes in km (corrected for the zig-zag of boundaries on
    /// the mesh; a ridge has two flanks, a trench one subducting side).
    pub trench_length: f64,
    pub ridge_length: f64,
    /// Length of continental collision boundaries in km (half of both sides' cells).
    pub collision_length: f64,
    /// Area of sea floor opened by diverging plates so far (one flank), from the plate motions,
    /// in km². Earth: ~3.4 km²/yr.
    pub opened_area: f64,
    /// Cells showing a crust sample that is also shown at another cell (fraction of all
    /// cells), and the most cells showing one sample.
    pub duplicated_cell_fraction: f64,
    pub max_cells_per_sample: usize,
    /// Empty, unconsumed local vertices of plates enclosed by the plate's crust (should be
    /// filled at once, see gap healing).
    pub enclosed_empty: usize,
    /// Continental crust as a fraction of the surface counted by distinct samples instead of
    /// cells (shared samples count once).
    pub continental_sample_fraction: f64,
    /// Sea level (m, an offset of dry elevations relative to the isostatic datum) and its
    /// changes so far by tectonics and by surface processes.
    pub sea_level: f64,
    pub sea_level_change: (f64, f64),
    /// Volume of the ocean (km³) from the surface elevations.
    pub ocean_volume: f64,
    /// Fraction of the crust of continental origin (see [`Crust::is_continental_origin`]) below
    /// sea level, and its mean surface elevation (freeboard, m), and its share of the surface.
    pub flooded_continental_fraction: f64,
    pub freeboard: f64,
    pub continental_origin_fraction: f64,
    /// Mass (Gt) carried into continental collisions by convergence so far.
    pub collision_inflow: f64,
    /// Thickness percentiles (10/50/90%, km) of the visible crust samples of continental origin,
    /// and the fraction of them thicker than 50 km.
    pub continental_thickness: [f64; 3],
    pub thick_continental_fraction: f64,
    /// Relative speed of the plates across boundaries (per boundary cell, both sides): median
    /// (rad/Myr) and fraction slower than 2 cm/yr.
    pub median_relative_speed: f64,
    pub slow_boundary_fraction: f64,
    /// Mean |cos| of the angle between relative motion and the smoothed boundary normal (2/π
    /// for random orientation), and the boundary fractions with |cos| > 0.7 (opening/closing)
    /// and < 0.3 (strike-slip).
    pub mean_obliquity_cos: f64,
    pub normal_motion_fraction: f64,
    pub strike_slip_fraction: f64,
    /// Power dissipated by boundary shear coupling relative to the power of slab pull.
    pub shear_power_ratio: f64,
    /// Boundary segments with their own polarity, those with a slab, and the mean slab length
    /// (km) at trench cells.
    pub segments: usize,
    pub slab_segments: usize,
    pub mean_slab_length: f64,
    /// Segment events so far.
    pub segment_events: super::polarity::SegmentStatistics,
    /// Area fraction of the oceanic crust per 20 Myr age bin (the last bin is ≥ 200 Myr).
    pub age_histogram: [f64; 11],
    /// 99th percentile of the surface speed over all cells in rad/Myr.
    pub speed_p99: f64,
    /// Root mean square speed of plates covering more than 1% of the surface, in rad/Myr.
    pub rms_plate_speed: f64,
    /// Number of plates covering more than 1% of the surface.
    pub plates_above_one_percent: usize,
    /// Total length of plate boundaries relative to that of circular plates of the same areas
    /// (1 for compact plates; ragged or fragmented plates have longer boundaries; Earth's 52
    /// plates of PB2002: ~1.6).
    pub boundary_ratio: f64,
    /// Length (km) of passive margins (oceanic next to continental crust of the same plate)
    /// old enough for subduction initiation, and the expected number of initiations per Myr.
    pub old_margin_length: f64,
    pub initiation_rate: f64,
    /// Expected number of subduction initiations per Myr at existing plate boundaries.
    pub induced_rate: f64,
    /// Fraction of the oceanic crust that is at least 200 Myr old, and the fraction of that
    /// next to continental crust of any plate.
    pub ancient_ocean_fraction: f64,
    pub ancient_ocean_at_continents: f64,
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

/// Relative motion across plate boundaries (see [`Statistics`]).
struct BoundaryMotion {
    median_relative_speed: f64,
    slow_fraction: f64,
    mean_cos: f64,
    normal_fraction: f64,
    strike_slip_fraction: f64,
    shear_power_ratio: f64,
}

impl Simulation {
    fn boundary_motion(&self, boundaries: &[super::dynamics::BoundaryCell]) -> BoundaryMotion {
        let params = &self.params;
        let radius = params.planet_radius;
        let cell_area = 4.0 * std::f64::consts::PI * radius * radius / self.mesh.len() as f64;
        // rad/Myr → m/s
        let to_m_per_s = radius / (1e6 * 365.25 * 24.0 * 3600.0);
        let slow = 2.0 / 100.0 / (radius / 1e6);
        let normals = self.smoothed_normals(boundaries);
        let mut speeds = Vec::with_capacity(boundaries.len());
        let (mut cos_sum, mut normal, mut strike, mut shear_power, mut pull_power) =
            (0.0, 0usize, 0usize, 0.0, 0.0);
        for (boundary, &normal_vector) in boundaries.iter().zip(&normals) {
            let r = self.mesh.vertex(boundary.cell);
            let relative = (self.plates[boundary.plate as usize].angular_velocity
                - self.plates[boundary.other as usize].angular_velocity)
                .cross(r);
            let speed = relative.length();
            speeds.push(speed);
            let cos = if speed > 0.0 {
                relative.dot(normal_vector).abs() / speed
            } else {
                0.0
            };
            cos_sum += cos;
            normal += usize::from(cos > 0.7);
            strike += usize::from(cos < 0.3);
            let strike_slip = relative.dot(r.cross(normal_vector)) * to_m_per_s;
            let k = 0.5
                * params.basal_drag
                * if boundary.kind == super::dynamics::BoundaryKind::Collision {
                    params.collision_resistance * cell_area
                } else {
                    params.boundary_coupling_width * self.boundary_length_per_cell()
                };
            shear_power += k * strike_slip * strike_slip;
            let velocity = self.plates[boundary.plate as usize]
                .angular_velocity
                .cross(r)
                * to_m_per_s;
            if let super::dynamics::BoundaryKind::Trench { .. } = boundary.kind {
                pull_power += self.boundary_force(boundary).dot(velocity).max(0.0);
            }
        }
        let count = speeds.len().max(1) as f64;
        speeds.sort_by(f64::total_cmp);
        BoundaryMotion {
            median_relative_speed: speeds.get(speeds.len() / 2).copied().unwrap_or(0.0),
            slow_fraction: speeds.iter().filter(|&&speed| speed < slow).count() as f64 / count,
            mean_cos: cos_sum / count,
            normal_fraction: normal as f64 / count,
            strike_slip_fraction: strike as f64 / count,
            shear_power_ratio: shear_power / pull_power.max(1e-30),
        }
    }

    /// Boundary length relative to circular plates of the same areas.
    fn boundary_ratio(&self, plate_areas: &[usize]) -> f64 {
        let cell_count = self.mesh.len();
        let boundary_cells = (0..cell_count as u32)
            .filter(|&cell| {
                let plate = self.owner(cell).plate;
                self.mesh
                    .neighbors(cell)
                    .iter()
                    .any(|&neighbor| self.owner(neighbor).plate != plate)
            })
            .count();
        // Both sides of every boundary, in radians on the unit sphere.
        let boundary = boundary_cells as f64 * self.mesh.mean_edge_angle() / BOUNDARY_ZIGZAG;
        // Perimeter of a spherical cap covering the fraction f of the sphere: 2π sin θ with
        // cos θ = 1 − 2f.
        let circles: f64 = plate_areas
            .iter()
            .filter(|&&area| area > 0)
            .map(|&area| {
                let fraction = area as f64 / cell_count as f64;
                2.0 * std::f64::consts::PI * (1.0 - 2.0 * fraction).clamp(-1.0, 1.0).acos().sin()
            })
            .sum();
        boundary / circles.max(1e-9)
    }

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
        let mut speeds = Vec::with_capacity(cell_count);
        let (mut freeboard_sum, mut flooded_continental, mut water_depth_sum) = (0.0, 0usize, 0.0);
        let mut origin_cells = 0usize;
        let mut plate_speed_sums = vec![0.0; self.plates.len()];

        for cell in 0..cell_count as u32 {
            let plate = self.owner(cell).plate as usize;
            plate_areas[plate] += 1;
            let speed = self.plates[plate]
                .angular_velocity
                .cross(self.mesh.vertex(cell))
                .length();
            speed_sum += speed;
            speeds.push(speed);
            plate_speed_sums[plate] += speed;
            max_speed = max_speed.max(speed);
            let crust = self.surface_crust(cell);
            if crust.is_continental() {
                continental += 1;
                continental_speed += speed;
            } else if crust.is_oceanic() {
                let age = crust.age(self.time);
                oceanic_age_sum += age;
                oceanic_ages.push(age);
                oceanic_speed += speed;
            }
            let elevation = crust.surface_elevation(self.time, self.sea_level);
            if crust.is_continental_origin() {
                origin_cells += 1;
                freeboard_sum += elevation;
                if elevation < 0.0 {
                    flooded_continental += 1;
                }
            }
            if elevation < 0.0 {
                water_depth_sum -= elevation;
            }
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
        // Masses by class over the visible crust samples (each counted once).
        let cell_area =
            4.0 * std::f64::consts::PI * self.params.planet_radius.powi(2) / cell_count as f64;
        let to_gigatonnes = cell_area / 1e12;
        let mut masses = [0.0; 5];
        let mut visible_mass = 0.0;
        let mut seen = HashSet::default();
        for owner in &self.owners {
            if !seen.insert(*owner) {
                continue;
            }
            let crust = self.plates[owner.plate as usize].crust[owner.local as usize]
                .expect("owners always point to existing crust");
            let crust_mass = crust.thickness * crust.density;
            let class = if crust.is_continental() {
                0
            } else if crust.is_oceanic() {
                2
            } else {
                1
            };
            masses[class] += crust_mass;
            masses[3] += crust.mass() - crust_mass;
            if crust.density < 2850.0 {
                masses[4] += crust_mass;
            }
            visible_mass += crust.mass();
        }
        let total_mass = self.total_mass();
        let boundary_length =
            self.params.planet_radius * self.mesh.mean_edge_angle() / 1000.0 / BOUNDARY_ZIGZAG;
        let (mut trench_length, mut ridge_flank_length, mut slab_pull, mut ridge_push) =
            (0.0, 0.0, 0.0, 0.0);
        let boundaries = self.boundary_cells();
        let (mut trench_cells, mut trench_slab) = (0usize, 0.0);
        let mut collision_length = 0.0;
        for boundary in &boundaries {
            let force = self.boundary_force(boundary).length();
            match boundary.kind {
                super::dynamics::BoundaryKind::Trench { .. } => {
                    trench_length += boundary_length;
                    slab_pull += force;
                    trench_cells += 1;
                    trench_slab += boundary
                        .segment
                        .and_then(|id| self.polarities.segment(boundary.plate, boundary.other, id))
                        .map_or(0.0, |segment| segment.slab_length);
                }
                super::dynamics::BoundaryKind::Ridge => {
                    ridge_flank_length += boundary_length;
                    ridge_push += force;
                }
                super::dynamics::BoundaryKind::Collision => {
                    collision_length += boundary_length;
                }
                _ => {}
            }
        }
        let slabs: Vec<f64> = self
            .polarities
            .segments()
            .map(|segment| segment.slab_length)
            .filter(|&length| length > 0.0)
            .collect();
        let (mut old_margin_length, mut initiation_rate) = (0.0, 0.0);
        let (mut ancient, mut ancient_at_continents) = (0usize, 0usize);
        let margin_length = self.boundary_length_per_cell();
        for cell in 0..cell_count as u32 {
            let crust = self.surface_crust(cell);
            if !crust.is_oceanic() {
                continue;
            }
            let age = crust.age(self.time);
            let plate = self.owner(cell).plate;
            let mut same_plate_continent = false;
            let mut any_continent = false;
            for &neighbor in self.mesh.neighbors(cell) {
                if self.surface_crust(neighbor).is_continental_origin() {
                    any_continent = true;
                    same_plate_continent |= self.owner(neighbor).plate == plate;
                }
            }
            if same_plate_continent && age >= self.params.events.initiation_age {
                old_margin_length += margin_length / 1000.0;
                initiation_rate += self.params.events.initiation_hazard(age) * margin_length;
            }
            if age >= 200.0 {
                ancient += 1;
                if any_continent {
                    ancient_at_continents += 1;
                }
            }
        }
        let induced_rate = self
            .initiation_candidates(u32::MAX, &boundaries)
            .iter()
            .filter(|(initiation, _)| {
                matches!(initiation, super::events::Initiation::Boundary { .. })
            })
            .map(|(_, hazard)| hazard)
            .sum();
        let boundary_ratio = self.boundary_ratio(&plate_areas);
        let mut cells_per_sample: HashMap<Ownership, usize> = HashMap::default();
        for owner in &self.owners {
            *cells_per_sample.entry(*owner).or_default() += 1;
        }
        let duplicated_cells: usize = cells_per_sample.values().filter(|&&count| count > 1).sum();
        let max_cells_per_sample = cells_per_sample.values().copied().max().unwrap_or(0);
        let mut origin_thickness: Vec<f64> = cells_per_sample
            .keys()
            .filter_map(|owner| self.plates[owner.plate as usize].crust[owner.local as usize])
            .filter(|crust| crust.is_continental_origin())
            .map(|crust| crust.thickness / 1000.0)
            .collect();
        origin_thickness.sort_by(f64::total_cmp);
        let thickness_percentile = |p: f64| {
            origin_thickness
                .get(((origin_thickness.len().max(1) - 1) as f64 * p).round() as usize)
                .copied()
                .unwrap_or(0.0)
        };
        let continental_thickness = [
            thickness_percentile(0.1),
            thickness_percentile(0.5),
            thickness_percentile(0.9),
        ];
        let thick_continental_fraction = origin_thickness.iter().filter(|&&t| t > 50.0).count()
            as f64
            / origin_thickness.len().max(1) as f64;
        let continental_samples = cells_per_sample
            .keys()
            .filter(|owner| {
                self.plates[owner.plate as usize].crust[owner.local as usize]
                    .is_some_and(|crust| crust.is_continental())
            })
            .count();
        let enclosed_empty = self
            .plates
            .iter()
            .map(|plate| {
                (0..cell_count)
                    .filter(|&local| {
                        plate.crust[local].is_none()
                            && !plate.consumed[local]
                            && self
                                .mesh
                                .neighbors(local as u32)
                                .iter()
                                .all(|&neighbor| plate.crust[neighbor as usize].is_some())
                    })
                    .count()
            })
            .sum();
        let motion = self.boundary_motion(&boundaries);
        let mut age_histogram = [0.0; 11];
        for &age in &oceanic_ages {
            age_histogram[((age / 20.0).max(0.0) as usize).min(10)] += 1.0;
        }
        for bin in &mut age_histogram {
            *bin /= oceanic.max(1) as f64;
        }
        speeds.sort_by(f64::total_cmp);
        let speed_p99 = speeds[((speeds.len() - 1) as f64 * 0.99).round() as usize];
        let (mut square_sum, mut weight) = (0.0, 0.0);
        let mut plates_above_one_percent = 0;
        for (&area, &speed_sum) in plate_areas.iter().zip(&plate_speed_sums) {
            if area as f64 > 0.01 * cell_count as f64 {
                plates_above_one_percent += 1;
                let mean = speed_sum / area as f64;
                square_sum += mean * mean;
                weight += 1.0;
            }
        }
        let rms_plate_speed = (square_sum / f64::max(weight, 1.0)).sqrt();
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
            ledger: self.ledger.checked(total_mass),
            matching: self.matching_statistics.clone(),
            continental_mass: masses[0] * to_gigatonnes,
            transitional_mass: masses[1] * to_gigatonnes,
            oceanic_mass: masses[2] * to_gigatonnes,
            sediment_mass: masses[3] * to_gigatonnes,
            felsic_mass: masses[4] * to_gigatonnes,
            hidden_mass: (total_mass - visible_mass) * to_gigatonnes,
            trench_length,
            ridge_length: ridge_flank_length / 2.0,
            collision_length: collision_length / 2.0,
            opened_area: self.opened_area,
            segments: self.polarities.segments().count(),
            slab_segments: slabs.len(),
            mean_slab_length: trench_slab / trench_cells.max(1) as f64 / 1000.0,
            segment_events: self.polarities.statistics.clone(),
            age_histogram,
            speed_p99,
            rms_plate_speed,
            plates_above_one_percent,
            boundary_ratio,
            duplicated_cell_fraction: duplicated_cells as f64 / cell_count as f64,
            max_cells_per_sample,
            enclosed_empty,
            continental_sample_fraction: continental_samples as f64 / cell_count as f64,
            sea_level: self.sea_level,
            sea_level_change: self.sea_level_change,
            ocean_volume: water_depth_sum * cell_area / 1e9,
            flooded_continental_fraction: flooded_continental as f64 / origin_cells.max(1) as f64,
            freeboard: freeboard_sum / origin_cells.max(1) as f64,
            continental_origin_fraction: origin_cells as f64 / cell_count as f64,
            collision_inflow: self.collision_inflow,
            continental_thickness,
            thick_continental_fraction,
            median_relative_speed: motion.median_relative_speed,
            slow_boundary_fraction: motion.slow_fraction,
            mean_obliquity_cos: motion.mean_cos,
            normal_motion_fraction: motion.normal_fraction,
            strike_slip_fraction: motion.strike_slip_fraction,
            shear_power_ratio: motion.shear_power_ratio,
            old_margin_length,
            initiation_rate,
            induced_rate,
            ancient_ocean_fraction: ancient as f64 / oceanic.max(1) as f64,
            ancient_ocean_at_continents: ancient_at_continents as f64 / ancient.max(1) as f64,
            slab_pull,
            ridge_push,
            land_masses: components(&self.mesh, |cell| self.surface_elevation(cell) >= 0.0),
            continental_masses: components(&self.mesh, |cell| {
                self.surface_crust(cell).is_continental()
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use glam::DVec3;

    use super::*;

    #[test]
    fn boundary_zigzag_factor_matches_the_mesh() {
        for subdivisions in [4, 5] {
            for normal in [
                DVec3::new(0.3, 0.5, 0.8).normalize(),
                DVec3::new(-0.7, 0.1, 0.2).normalize(),
                DVec3::Z,
            ] {
                let mut simulation = Simulation::new(
                    SphereMesh::icosphere(subdivisions),
                    1,
                    TectonicParams::default(),
                    1,
                );
                simulation.split_plate(0, normal).unwrap();
                let cells = simulation
                    .boundary_cells()
                    .iter()
                    .filter(|b| b.plate == 0)
                    .count();
                let factor =
                    cells as f64 * simulation.mesh.mean_edge_angle() / (2.0 * std::f64::consts::PI);
                assert!(
                    (factor / BOUNDARY_ZIGZAG - 1.0).abs() < 0.06,
                    "{subdivisions}: {factor}"
                );
            }
        }
    }
}
