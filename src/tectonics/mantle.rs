//! Large-scale mantle flow below the plates.
//!
//! The flow at the base of the plates is approximated by point sources and sinks on the sphere:
//!
//! * **Downwellings** where slabs have sunk into the mantle. Every trench feeds the downwelling
//!   below it with its subduction rate (trench length × convergence speed, an area per time),
//!   and the mantle remembers this with an exponential decay, so slab graveyards keep drawing
//!   mantle flow towards them for a while after subduction stopped.
//! * **Plumes**: hot upwellings that rise preferentially far from the slab graveyards, grow,
//!   last for of the order of 100 Myr, and fade. Mantle flows radially away from them.
//!
//! A source with area flux `Q` drives a radial flow of speed `Q cot(θ/2) / (4π R)` at angular
//! distance `θ`: the flow of a point source on a sphere whose flux returns uniformly over the
//! whole sphere (the Green's function of the surface divergence), softened within
//! [`MantleParams::core_radius`] to stay finite. Unlike `Q / (2π R sin θ)`, it decays to zero at
//! the antipode instead of converging there into a phantom sink. Sinks work the same way with
//! negative flux. The plates feel
//! this flow through basal drag (see [`super::dynamics`]).

use glam::DVec3;
use rand::{Rng, RngExt};

use crate::sphere::SphereMesh;

use super::generation::random_unit_vector;

/// Parameters of the mantle flow.
#[derive(Clone, Debug)]
pub struct MantleParams {
    /// Subdivisions of the coarse grid that collects downwellings.
    pub grid_subdivisions: u32,
    /// Subdivisions of the grid on which the (smooth) flow is evaluated before it is mapped to
    /// the cells of the simulation.
    pub evaluation_subdivisions: u32,
    /// Time constant in Myr of the mantle's memory of subducted slabs.
    pub slab_memory: f64,
    /// Fraction of the subduction rate that returns as mantle flow towards the downwelling.
    pub return_flow_fraction: f64,
    /// Mean time in Myr between the births of new plumes.
    pub plume_interval: f64,
    /// Range of plume lifetimes in Myr.
    pub plume_lifetime: (f64, f64),
    /// Range of peak plume fluxes in m²/s (area of new surface flow per second).
    pub plume_flux: (f64, f64),
    /// Angular radius in radians within which the flow of a source or sink is softened.
    pub core_radius: f64,
    /// Number of random candidate locations for a new plume, of which the one farthest from
    /// downwellings is taken.
    pub plume_candidates: usize,
}

impl Default for MantleParams {
    fn default() -> Self {
        Self {
            grid_subdivisions: 2,
            evaluation_subdivisions: 4,
            slab_memory: 100.0,
            return_flow_fraction: 0.5,
            plume_interval: 15.0,
            plume_lifetime: (80.0, 200.0),
            plume_flux: (2e-3, 6e-3),
            core_radius: 0.1,
            plume_candidates: 16,
        }
    }
}

/// A mantle plume.
#[derive(Clone, Debug)]
pub struct Plume {
    /// Unit vector below which the plume rises.
    pub position: DVec3,
    /// Age in Myr.
    pub age: f64,
    pub lifetime: f64,
    /// Peak area flux in m²/s.
    pub peak_flux: f64,
    /// Whether the plume has already rifted the plate above it.
    pub has_rifted: bool,
}

impl Plume {
    /// Current area flux in m²/s: the plume head grows quickly and the plume fades towards the
    /// end of its life.
    pub fn flux(&self) -> f64 {
        let growth = (self.age / 10.0).min(1.0);
        let decay = ((self.lifetime - self.age) / 20.0).clamp(0.0, 1.0);
        self.peak_flux * growth * decay
    }
}

pub struct MantleFlow {
    params: MantleParams,
    grid: SphereMesh,
    /// Remembered subduction rate (m²/s) collected in each grid cell.
    downwelling: Vec<f64>,
    plumes: Vec<Plume>,
    /// Evaluation grid and, per cell of the simulation mesh, its nearest evaluation cell.
    evaluation: Option<(SphereMesh, Vec<u32>)>,
}

impl MantleFlow {
    pub fn new(params: MantleParams) -> Self {
        let grid = SphereMesh::icosphere(params.grid_subdivisions);
        let downwelling = vec![0.0; grid.len()];
        Self {
            params,
            grid,
            downwelling,
            plumes: Vec::new(),
            evaluation: None,
        }
    }

    pub fn plumes(&self) -> &[Plume] {
        &self.plumes
    }

    /// Records that the plume with the given index has rifted the plate above it.
    pub fn mark_rifted(&mut self, plume: usize) {
        self.plumes[plume].has_rifted = true;
    }

    #[cfg(test)]
    pub fn insert_plume(&mut self, plume: Plume) {
        self.plumes.push(plume);
    }

    /// Advances the mantle by `dt` Myr. `subduction` lists the positions of trench segments with
    /// their subduction rate in m²/s.
    pub fn step(&mut self, dt: f64, subduction: &[(DVec3, f64)], rng: &mut impl Rng) {
        let params = &self.params;
        let keep = (-dt / params.slab_memory).exp();
        for downwelling in &mut self.downwelling {
            *downwelling *= keep;
        }
        // Exponential moving average of the subduction rate.
        let mut hint = 0;
        for &(position, rate) in subduction {
            let cell = self.grid.nearest_from(position, hint);
            hint = cell;
            self.downwelling[cell as usize] += rate * (1.0 - keep);
        }

        for plume in &mut self.plumes {
            plume.age += dt;
        }
        self.plumes.retain(|plume| plume.age < plume.lifetime);

        if rng.random_bool((dt / params.plume_interval).clamp(0.0, 1.0)) {
            // Plumes rise where no slabs have sunk recently.
            let position = (0..params.plume_candidates.max(1))
                .map(|_| random_unit_vector(rng))
                .max_by(|a, b| {
                    self.downwelling_inflow(*a)
                        .total_cmp(&self.downwelling_inflow(*b))
                        .reverse()
                })
                .expect("at least one candidate");
            self.plumes.push(Plume {
                position,
                age: 0.0,
                lifetime: rng.random_range(params.plume_lifetime.0..=params.plume_lifetime.1),
                peak_flux: rng.random_range(params.plume_flux.0..=params.plume_flux.1),
                has_rifted: false,
            });
        }
    }

    /// How strongly a position is influenced by downwellings (sum of their speeds there).
    fn downwelling_inflow(&self, position: DVec3) -> f64 {
        (0..self.grid.len() as u32)
            .map(|cell| {
                let flux = self.downwelling[cell as usize] * self.params.return_flow_fraction;
                flux * radial_speed_factor(
                    self.grid.vertex(cell).angle_between(position),
                    self.params.core_radius,
                )
            })
            .sum()
    }

    /// The mantle flow velocity in m/s, tangent to the sphere, at every cell of `mesh`. The flow
    /// is evaluated on a coarser grid and taken from the nearest grid cell.
    pub fn velocities(&mut self, mesh: &SphereMesh, planet_radius: f64) -> Vec<DVec3> {
        let stale = self
            .evaluation
            .as_ref()
            .is_none_or(|(_, mapping)| mapping.len() != mesh.len());
        if stale {
            let grid =
                SphereMesh::icosphere(self.params.evaluation_subdivisions.min(mesh.subdivisions()));
            let mut hint = 0;
            let mapping = (0..mesh.len() as u32)
                .map(|cell| {
                    hint = grid.nearest_from(mesh.vertex(cell), hint);
                    hint
                })
                .collect();
            self.evaluation = Some((grid, mapping));
        }
        let (grid, mapping) = self.evaluation.as_ref().expect("evaluation grid exists");
        let coarse = self.evaluate(grid, planet_radius);
        mapping
            .iter()
            .enumerate()
            .map(|(cell, &coarse_cell)| {
                // Keep the velocity tangent to the sphere at the fine cell.
                let r = mesh.vertex(cell as u32);
                let v = coarse[coarse_cell as usize];
                v - r * r.dot(v)
            })
            .collect()
    }

    /// Evaluates the flow at every cell of `mesh`.
    fn evaluate(&self, mesh: &SphereMesh, planet_radius: f64) -> Vec<DVec3> {
        // Sources (positive flux) and sinks (negative flux).
        let mut sources: Vec<(DVec3, f64)> = self
            .plumes
            .iter()
            .map(|plume| (plume.position, plume.flux()))
            .collect();
        let strongest = self.downwelling.iter().copied().fold(0.0, f64::max);
        for cell in 0..self.grid.len() as u32 {
            let flux = self.downwelling[cell as usize] * self.params.return_flow_fraction;
            if flux > 1e-3 * strongest && flux > 0.0 {
                sources.push((self.grid.vertex(cell), -flux));
            }
        }

        (0..mesh.len() as u32)
            .map(|cell| {
                let r = mesh.vertex(cell);
                sources
                    .iter()
                    .map(|&(position, flux)| {
                        // Unit tangent at r pointing away from the source along the great circle.
                        let away = (r * r.dot(position) - position).normalize_or_zero();
                        let angle = position.angle_between(r);
                        away * (flux * radial_speed_factor(angle, self.params.core_radius)
                            / planet_radius)
                    })
                    .sum()
            })
            .collect()
    }
}

/// Radial speed (times the planet radius, per unit flux) of the flow from a point source at
/// angular distance `angle`: `cot(θ/2) / (4π) = sin θ / (4π (1 − cos θ))`, softened within
/// `core_radius` (the near field becomes `θ / (2π (θ² + c²))`).
fn radial_speed_factor(angle: f64, core_radius: f64) -> f64 {
    angle.sin() / (4.0 * std::f64::consts::PI * (1.0 - angle.cos() + (1.0 - core_radius.cos())))
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand::rngs::ChaCha8Rng;

    use super::*;

    #[test]
    fn flow_points_towards_downwellings_and_away_from_plumes() {
        let mesh = SphereMesh::icosphere(3);
        let mut mantle = MantleFlow::new(MantleParams::default());
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        // A long-lived trench at +x.
        for _ in 0..200 {
            mantle.step(1.0, &[(DVec3::X, 0.05)], &mut rng);
        }
        mantle.plumes.clear();
        mantle.plumes.push(Plume {
            position: -DVec3::X,
            age: 50.0,
            lifetime: 200.0,
            peak_flux: 5e-3,
            has_rifted: false,
        });
        let velocities = mantle.velocities(&mesh, 6.371e6);
        let near = |target: DVec3| {
            (0..mesh.len() as u32)
                .filter(|&cell| (0.3..0.6).contains(&mesh.vertex(cell).angle_between(target)))
                .collect::<Vec<_>>()
        };
        for cell in near(DVec3::X) {
            let r = mesh.vertex(cell);
            let towards = (DVec3::X - r * r.x).normalize();
            assert!(velocities[cell as usize].dot(towards) > 0.0);
        }
        for cell in near(-DVec3::X) {
            let r = mesh.vertex(cell);
            // Tangent at r pointing away from -X.
            let away = (r * r.dot(-DVec3::X) - -DVec3::X).normalize();
            assert!(velocities[cell as usize].dot(away) > 0.0);
        }
        // Speeds of the order of cm/yr (1 cm/yr ≈ 3.2e-10 m/s).
        let max = velocities.iter().map(|v| v.length()).fold(0.0, f64::max);
        assert!((1e-10..1e-8).contains(&max), "{max}");
    }

    #[test]
    fn source_flow_returns_uniformly_without_antipodal_sink() {
        let core = 0.1;
        // Flux through the circle at θ (per unit source flux): 2π sin θ · v(θ). With uniform
        // return flow it must be (1 + cos θ) / 2 away from the core.
        for degrees in [30.0, 60.0, 90.0, 120.0, 150.0] {
            let angle: f64 = f64::to_radians(degrees);
            // A negligible core radius, so that the softening does not matter here.
            let flux = 2.0 * std::f64::consts::PI * angle.sin() * radial_speed_factor(angle, 1e-4);
            let expected = (1.0 + angle.cos()) / 2.0;
            assert!(
                (flux - expected).abs() < 0.01,
                "{degrees}: {flux} vs {expected}"
            );
        }
        // Speed falls monotonically beyond the core, to zero at the antipode.
        let mut previous = radial_speed_factor(0.3, core);
        for step in 1..=100 {
            let angle = 0.3 + (std::f64::consts::PI - 0.3) * f64::from(step) / 100.0;
            let speed = radial_speed_factor(angle, core);
            assert!(speed < previous + 1e-15);
            previous = speed;
        }
        assert!(radial_speed_factor(std::f64::consts::PI, core).abs() < 1e-12);
        // The near field matches a point source in the plane: 1 / (2π θ).
        let angle: f64 = 0.5;
        let planar = angle / (2.0 * std::f64::consts::PI * (angle * angle + core * core));
        assert!((radial_speed_factor(angle, core) - planar).abs() / planar < 0.05);
    }

    #[test]
    fn plumes_are_born_and_die() {
        let mut mantle = MantleFlow::new(MantleParams::default());
        let mut rng = ChaCha8Rng::seed_from_u64(2);
        let mut counts = Vec::new();
        for _ in 0..1000 {
            mantle.step(1.0, &[], &mut rng);
            counts.push(mantle.plumes().len());
        }
        let mean = counts[300..].iter().sum::<usize>() as f64 / 700.0;
        // About lifetime / interval ≈ 140 / 15 plumes at a time.
        assert!((4.0..16.0).contains(&mean), "{mean}");
    }
}
