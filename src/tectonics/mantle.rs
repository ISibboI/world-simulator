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
//!
//! The same density anomalies deflect the surface: **dynamic topography**
//! `h = −σ_rr / (Δρ g)` (Hager et al. 1985; Gurnis 1990; Flament et al. 2013). Sunken slabs
//! (the mass excess of the cold lithosphere, remembered like the downwellings) pull the surface
//! down over a width of about their depth; plume heads ponding below the lithosphere lift it in
//! swells. The field lives in the mantle frame, so plates move over it: continental interiors
//! rise and fall by a few hundred metres over 10–100 Myr, and rivers strip the cover of uplifted
//! platforms (see [`MantleFlow::dynamic_topography`]).

use crate::math;
use glam::DVec3;
use rand::{Rng, RngExt};

use crate::sphere::SphereMesh;

use super::generation::random_unit_vector;
use crate::isostasy::MANTLE_DENSITY;

const SECONDS_PER_MYR: f64 = 1e6 * 365.25 * 24.0 * 3600.0;

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
    /// Subdivisions of the grid that collects the mass of sunken slabs.
    pub slab_grid_subdivisions: u32,
    /// Horizontal distance in metres from the trench, under the overriding plate, at which the
    /// slab's mass sinks: the reach of a slab dipping 45° to the base of the upper mantle
    /// (660 km).
    pub slab_offset: f64,
    /// Depth stages a sunken slab passes through (see [`SlabStage`]), from the top. Slabs sink
    /// at ~5 cm/yr through the upper mantle and ~1.2 cm/yr through the lower mantle (van der
    /// Meer et al. 2010); below ~2000 km the surface hardly feels them.
    pub slab_stages: Vec<SlabStage>,
    /// Scales the slab part of the dynamic topography (1 = physical, 0 disables it, for
    /// comparison runs).
    pub topography_response: f64,
    /// Dry height in metres of the swell above a plume at the top of the range of
    /// [`MantleParams::plume_flux`]: the buoyancy of a ponded plume head, α ΔT L with
    /// α = 3 × 10⁻⁵ K⁻¹, ΔT ≈ 150 K and L ≈ 150 km (hotspot swells are 0.5–1.5 km high under
    /// water, i.e. ~0.35–1 km dry; Crough 1983; Sleep 1990). Weaker plumes lift less. Zero
    /// disables plume swells.
    pub plume_swell_height: f64,
    /// Standard deviation in metres of a plume swell (swell half-widths 500–1000 km).
    pub plume_swell_width: f64,
}

/// A depth range of the mantle that sunken slabs pass through.
#[derive(Clone, Debug)]
pub struct SlabStage {
    /// Mean time in Myr a slab spends in the range (its thickness over the sinking speed).
    pub residence: f64,
    /// Fraction of the slab's weight that the surface supports by its deflection, the rest
    /// being supported deeper (core–mantle boundary, viscous lower mantle): the surface
    /// response kernel for a viscosity increase at 660 km, which falls with depth from ~0.7 in
    /// the upper mantle to ~0.2 at mid-mantle depths (Hager 1984; Richards & Hager 1984).
    pub response: f64,
    /// Standard deviation in metres of the surface deflection: about the depth of the load.
    pub width: f64,
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
            slab_grid_subdivisions: 3,
            slab_offset: 660e3,
            slab_stages: vec![
                // 0–660 km at ~5 cm/yr; spread over the slab's own horizontal reach.
                SlabStage {
                    residence: 13.0,
                    response: 0.5,
                    width: 600e3,
                },
                // 660–1200 km at ~1.2 cm/yr.
                SlabStage {
                    residence: 45.0,
                    response: 0.4,
                    width: 900e3,
                },
                // 1200–2000 km.
                SlabStage {
                    residence: 67.0,
                    response: 0.2,
                    width: 1500e3,
                },
            ],
            topography_response: 1.0,
            plume_swell_height: 650.0,
            plume_swell_width: 450e3,
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
        let decay = ((self.lifetime - self.age) / 20.0).clamp(0.0, 1.0);
        self.peak_flux * self.head_growth() * decay
    }

    /// How far the plume head has arrived and spread below the plate (0 to 1 over its first
    /// 10 Myr).
    pub fn head_growth(&self) -> f64 {
        (self.age / 10.0).min(1.0)
    }
}

/// Grid cells with their interpolation weights.
type Weights = Vec<(u32, f64)>;

pub struct MantleFlow {
    params: MantleParams,
    grid: SphereMesh,
    /// Remembered subduction rate (m²/s) collected in each grid cell.
    downwelling: Vec<f64>,
    plumes: Vec<Plume>,
    /// Evaluation grid and, per cell of the simulation mesh, its nearest evaluation cell.
    evaluation: Option<(SphereMesh, Vec<u32>)>,
    /// Grid collecting sunken slabs, and the mass (kg) of the slabs remembered in each cell.
    slab_grid: SphereMesh,
    /// Per slab stage, per grid cell.
    slab_mass: Vec<Vec<f64>>,
    /// Interpolation from the evaluation grid to the cells of the simulation mesh: per cell,
    /// evaluation cells and their weights.
    interpolation: Option<(SphereMesh, Vec<Weights>)>,
}

impl MantleFlow {
    pub fn new(params: MantleParams) -> Self {
        let grid = SphereMesh::icosphere(params.grid_subdivisions);
        let downwelling = vec![0.0; grid.len()];
        let slab_grid = SphereMesh::icosphere(params.slab_grid_subdivisions);
        let slab_mass = vec![vec![0.0; slab_grid.len()]; params.slab_stages.len()];
        Self {
            params,
            grid,
            downwelling,
            plumes: Vec::new(),
            evaluation: None,
            slab_grid,
            slab_mass,
            interpolation: None,
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
    /// their subduction rate in m²/s, `slabs` where their slabs sink (under the overriding plate)
    /// with their mass flux in kg/s.
    pub fn step(
        &mut self,
        dt: f64,
        subduction: &[(DVec3, f64)],
        slabs: &[(DVec3, f64)],
        rng: &mut impl Rng,
    ) {
        let params = &self.params;
        let keep = math::exp(-dt / params.slab_memory);
        // Slabs pass through the depth stages: each stage hands on the share of its mass that
        // sank out of it during the step (mass-conserving), the last one loses it to depths the
        // surface does not feel.
        let mut handed_on: Option<Vec<f64>> = None;
        for (stage, masses) in params.slab_stages.iter().zip(&mut self.slab_mass) {
            let leave = 1.0 - math::exp(-dt / stage.residence);
            let outflow: Vec<f64> = masses.iter().map(|m| m * leave).collect();
            for (mass, out) in masses.iter_mut().zip(&outflow) {
                *mass -= out;
            }
            if let Some(inflow) = &handed_on {
                for (mass, inflow) in masses.iter_mut().zip(inflow) {
                    *mass += inflow;
                }
            }
            handed_on = Some(outflow);
        }
        if let Some(top) = self.slab_mass.first_mut() {
            let seconds = dt * SECONDS_PER_MYR;
            let mut hint = 0;
            for &(position, flux) in slabs {
                // Spread over the nearest grid vertex and its neighbours, so a migrating trench
                // moves the load smoothly.
                hint = self.slab_grid.nearest_from(position, hint);
                for (cell, weight) in tent_weights(&self.slab_grid, hint, position) {
                    top[cell as usize] += flux * seconds * weight;
                }
            }
        }
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
            let candidates: Vec<(DVec3, f64)> = (0..params.plume_candidates.max(1))
                .map(|_| random_unit_vector(rng))
                .map(|candidate| (candidate, self.downwelling_inflow(candidate)))
                .collect();
            let (position, _) = candidates
                .into_iter()
                .max_by(|a, b| a.1.total_cmp(&b.1).reverse())
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

    /// How strongly a position (unit vector) is influenced by downwellings (sum of their speeds
    /// there).
    fn downwelling_inflow(&self, position: DVec3) -> f64 {
        let core = core_term(self.params.core_radius);
        (0..self.grid.len() as u32)
            .map(|cell| {
                let flux = self.downwelling[cell as usize] * self.params.return_flow_fraction;
                let vertex = self.grid.vertex(cell);
                let sin = vertex.cross(position).length();
                flux * sin * radial_speed_per_sine(vertex.dot(position), core)
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

    /// Dynamic topography in metres (dry, i.e. without the water load) at every cell of `mesh`:
    /// depressions above sunken slabs and swells above plumes, with zero mean over the sphere
    /// (the mantle's mass is conserved; the return flow lifts the rest broadly). Evaluated on the
    /// evaluation grid and interpolated smoothly to the cells.
    pub fn dynamic_topography(&mut self, mesh: &SphereMesh, planet_radius: f64) -> Vec<f64> {
        let params = &self.params;
        if (params.topography_response == 0.0 || params.slab_stages.is_empty())
            && params.plume_swell_height == 0.0
        {
            return vec![0.0; mesh.len()];
        }
        let stale = self
            .interpolation
            .as_ref()
            .is_none_or(|(_, weights)| weights.len() != mesh.len());
        if stale {
            let grid =
                SphereMesh::icosphere(params.evaluation_subdivisions.min(mesh.subdivisions()));
            let weights = interpolation_weights(&grid, mesh);
            self.interpolation = Some((grid, weights));
        }
        let (grid, weights) = self.interpolation.as_ref().expect("interpolation exists");
        // Loads: sunken slabs pull down by f · M K(θ) / ρ_m with a normalised Gaussian K.
        let mut sources: Vec<(DVec3, f64, f64)> = Vec::new();
        for (stage, masses) in params.slab_stages.iter().zip(&self.slab_mass) {
            let scale = -params.topography_response * stage.response
                / (MANTLE_DENSITY * 2.0 * std::f64::consts::PI * stage.width.powi(2));
            sources.extend(
                (0..self.slab_grid.len() as u32)
                    .filter(|&cell| masses[cell as usize] > 0.0 && scale != 0.0)
                    .map(|cell| {
                        (
                            self.slab_grid.vertex(cell),
                            scale * masses[cell as usize],
                            stage.width / planet_radius,
                        )
                    }),
            );
        }
        let peak_flux = params.plume_flux.1.max(1e-12);
        sources.extend(self.plumes.iter().map(|plume| {
            (
                plume.position,
                params.plume_swell_height * plume.flux() / peak_flux,
                params.plume_swell_width / planet_radius,
            )
        }));
        let coarse: Vec<f64> = (0..grid.len() as u32)
            .map(|cell| {
                let r = grid.vertex(cell);
                sources
                    .iter()
                    .map(|&(position, height, width)| {
                        // θ² ≈ 2 (1 − cos θ), cut off beyond 4 widths.
                        let angle_squared = 2.0 * (1.0 - r.dot(position));
                        if angle_squared > 16.0 * width * width {
                            0.0
                        } else {
                            height * math::exp(-angle_squared / (2.0 * width * width))
                        }
                    })
                    .sum()
            })
            .collect();
        let mut field: Vec<f64> = weights
            .iter()
            .map(|cell| cell.iter().map(|&(c, w)| w * coarse[c as usize]).sum())
            .collect();
        let mean = field.iter().sum::<f64>() / field.len().max(1) as f64;
        for value in &mut field {
            *value -= mean;
        }
        field
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

        let core = core_term(self.params.core_radius);
        (0..mesh.len() as u32)
            .map(|cell| {
                let r = mesh.vertex(cell);
                sources
                    .iter()
                    .map(|&(position, flux)| {
                        // Tangent at r pointing away from the source along the great circle; its
                        // length is sin θ, which turns the factor below into the radial speed.
                        let cos = r.dot(position);
                        let away = r * cos - position;
                        away * (flux * radial_speed_per_sine(cos, core) / planet_radius)
                    })
                    .sum()
            })
            .collect()
    }
}

/// Weights that interpolate a field on `grid` to the cells of `mesh`: tent weights over the
/// nearest grid vertex and its neighbours, normalised, so the result has no steps at the edges
/// between grid cells.
fn interpolation_weights(grid: &SphereMesh, mesh: &SphereMesh) -> Vec<Weights> {
    let mut hint = 0;
    (0..mesh.len() as u32)
        .map(|cell| {
            let r = mesh.vertex(cell);
            hint = grid.nearest_from(r, hint);
            tent_weights(grid, hint, r)
        })
        .collect()
}

/// Normalised tent weights of the grid vertex `nearest` (the one nearest to `r`) and its
/// neighbours at the position `r`.
fn tent_weights(grid: &SphereMesh, nearest: u32, r: DVec3) -> Weights {
    let reach = 1.5 * grid.mean_edge_angle();
    let mut weights: Vec<(u32, f64)> = std::iter::once(nearest)
        .chain(grid.neighbors(nearest).iter().copied())
        .map(|c| {
            let angle = math::angle_between(grid.vertex(c), r);
            (c, (1.0 - angle / reach).max(0.0).powi(2))
        })
        .filter(|&(_, w)| w > 0.0)
        .collect();
    let total: f64 = weights.iter().map(|&(_, w)| w).sum();
    for (_, w) in &mut weights {
        *w /= total;
    }
    weights
}

/// Radial speed factor of [`radial_speed_per_sine`] at the angular distance `angle`, for tests.
#[cfg(test)]
fn radial_speed_factor(angle: f64, core_radius: f64) -> f64 {
    math::sin(angle) * radial_speed_per_sine(math::cos(angle), core_term(core_radius))
}

/// The softening term `1 − cos c` of the radial speed for the core radius `c`.
fn core_term(core_radius: f64) -> f64 {
    1.0 - math::cos(core_radius)
}

/// Radial speed (times the planet radius, per unit flux) of the flow from a point source,
/// divided by `sin θ` at the angular distance `θ`: `1 / (4π (1 − cos θ + (1 − cos c)))` with
/// the softening [`core_term`] `1 − cos c` (far field `cot(θ/2) / (4π)`, near field
/// `θ / (2π (θ² + c²))`). Takes `cos θ`, so no trigonometric functions are needed when
/// `cos θ` and `sin θ` come from dot and cross products.
fn radial_speed_per_sine(cos: f64, core_term: f64) -> f64 {
    1.0 / (4.0 * std::f64::consts::PI * (1.0 - cos + core_term))
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
            mantle.step(1.0, &[(DVec3::X, 0.05)], &[], &mut rng);
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
                .filter(|&cell| {
                    (0.3..0.6).contains(&math::angle_between(mesh.vertex(cell), target))
                })
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
            let flux =
                2.0 * std::f64::consts::PI * math::sin(angle) * radial_speed_factor(angle, 1e-4);
            let expected = (1.0 + math::cos(angle)) / 2.0;
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
    fn slabs_depress_and_plumes_lift_the_surface() {
        let mesh = SphereMesh::icosphere(6);
        let mut mantle = MantleFlow::new(MantleParams::default());
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        // 5000 km of trench at 5 cm/yr subducting old lithosphere (7e6 kg/m²) at +x.
        // The trench runs along a meridian through +x, ±2500 km, in 51 segments.
        let flux = 5e6 * 0.05 / (365.25 * 24.0 * 3600.0) * 7e6;
        let slabs: Vec<(DVec3, f64)> = (-25..=25)
            .map(|i| {
                let angle = f64::from(i) * 100e3 / 6.371e6;
                (
                    DVec3::new(math::cos(angle), 0.0, math::sin(angle)),
                    flux / 51.0,
                )
            })
            .collect();
        for _ in 0..300 {
            mantle.step(1.0, &[], &slabs, &mut rng);
        }
        // The slab mass approaches the steady state flux × total residence.
        let total: f64 = mantle.slab_mass.iter().flatten().sum();
        let residence: f64 = mantle.params.slab_stages.iter().map(|s| s.residence).sum();
        let expected = flux * residence * SECONDS_PER_MYR;
        assert!(
            (total / expected - 1.0).abs() < 0.05,
            "{total} vs {expected}"
        );
        mantle.plumes.clear();
        mantle.plumes.push(Plume {
            position: -DVec3::X,
            age: 50.0,
            lifetime: 200.0,
            peak_flux: 6e-3,
            has_rifted: false,
        });
        let field = mantle.dynamic_topography(&mesh, 6.371e6);
        let at = |target: DVec3| field[mesh.nearest_from(target, 0) as usize];
        let mean = field.iter().sum::<f64>() / field.len() as f64;
        assert!(mean.abs() < 1e-6);
        // Hundreds of metres down over the slabs, up over the plume.
        assert!(
            (-1500.0..-300.0).contains(&at(DVec3::X)),
            "{}",
            at(DVec3::X)
        );
        assert!(
            (300.0..1000.0).contains(&at(-DVec3::X)),
            "{}",
            at(-DVec3::X)
        );
        // Smooth at the cell scale (~110 km): no steps from the coarse evaluation grid.
        for cell in 0..mesh.len() as u32 {
            for &n in mesh.neighbors(cell) {
                assert!((field[cell as usize] - field[n as usize]).abs() < 150.0);
            }
        }
    }

    #[test]
    fn plumes_are_born_and_die() {
        let mut mantle = MantleFlow::new(MantleParams::default());
        let mut rng = ChaCha8Rng::seed_from_u64(2);
        let mut counts = Vec::new();
        for _ in 0..1000 {
            mantle.step(1.0, &[], &[], &mut rng);
            counts.push(mantle.plumes().len());
        }
        let mean = counts[300..].iter().sum::<usize>() as f64 / 700.0;
        // About lifetime / interval ≈ 140 / 15 plumes at a time.
        assert!((4.0..16.0).contains(&mean), "{mean}");
    }
}
