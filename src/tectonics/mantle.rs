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
//! (the mass excess of the cold lithosphere) sink through four depth stages to the core–mantle
//! boundary and pull the surface down through the surface kernels of a layered viscous mantle
//! (see [`super::kernels`]; no calibration factor); plume heads ponding below the lithosphere
//! lift it in swells. Known simplifications: no self-gravitation (~10% at the lowest degrees),
//! no compensation by the 660 km phase boundary (~25% of the upper-mantle stage while slabs
//! cross it, partly offset at 410 km), radial viscosity only (no weak mantle wedge). The field lives in the mantle frame, so plates move over it: continental interiors
//! rise and fall by a few hundred metres over 10–100 Myr, and rivers strip the cover of uplifted
//! platforms (see [`MantleFlow::dynamic_topography`]).

use crate::math;
use glam::DVec3;
use rand::{Rng, RngExt};

use crate::sphere::SphereMesh;

use super::generation::random_unit_vector;
use super::kernels::{GreenTable, LANES, LayeredMantle, stage_kernels};
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
    /// Meer et al. 2010). Mass leaving the last stage rests on the core–mantle boundary, where
    /// it no longer deflects the surface (the kernel is zero there).
    pub slab_stages: Vec<SlabStage>,
    /// Scales the slab part of the dynamic topography (1 = physical, 0 disables it, for
    /// comparison runs). Not a calibration: the response follows from the kernels.
    pub topography_response: f64,
    /// Viscosity layers of the mantle from the top for the dynamic-topography kernels (see
    /// [`super::kernels`]): bottom depth in metres and viscosity relative to the upper mantle;
    /// the last layer reaches the core–mantle boundary. Lithosphere 100× (Hager & Richards
    /// 1989), weak asthenosphere 0.1× (post-glacial rebound and geoid models, e.g. Steinberger
    /// & Calderwood 2006), lower mantle 30× (Hager 1984; inversions give 30–100×, Mitrovica &
    /// Forte 2004).
    pub viscosity_layers: Vec<(f64, f64)>,
    /// Thermal expansivity of the mantle (1/K) against depth (m), piecewise linear: the excess
    /// mass of a slab follows it as the slab sinks. The value at the surface is the one implied
    /// by the depth–age curve of the slab mass (GDH1). Lower-mantle values along the adiabat
    /// after Katsura et al. 2009 and Steinberger & Calderwood 2006 (±20%).
    pub expansivity: Vec<(f64, f64)>,
    /// Depth of the core–mantle boundary in metres.
    pub cmb_depth: f64,
    /// Standard deviation in metres of the horizontal extent of a slab load (the upper-mantle
    /// slab spans ~660 km horizontally, lower-mantle slabs fold into piles several hundred km
    /// wide, van der Meer et al. 2018). It also ends the truncated kernel sum smoothly.
    pub slab_load_width: f64,
    /// Highest spherical-harmonic degree of the slab Green's functions.
    pub kernel_degree: u32,
    /// Dry height in metres of the swell above a plume at the top of the range of
    /// [`MantleParams::plume_flux`]: the buoyancy of a ponded plume head, α ΔT L with
    /// α = 3 × 10⁻⁵ K⁻¹, ΔT ≈ 150 K and L ≈ 150 km (hotspot swells are 0.5–1.5 km high under
    /// water, i.e. ~0.35–1 km dry; Crough 1983; Sleep 1990). Weaker plumes lift less. Zero
    /// disables plume swells.
    pub plume_swell_height: f64,
    /// Standard deviation in metres of a plume swell (swell half-widths 500–1000 km).
    pub plume_swell_width: f64,
    /// Start the mantle in its steady state (#213): at the first step, the downwellings and the
    /// sunken slabs are filled as if the trenches of that step had been active for longer than
    /// the mantle remembers, and the plumes that a steady birth process leaves alive are there
    /// (see [`MantleFlow::prime`]). Without it (`mantle.steady_start=0`), the mantle starts
    /// empty and fills over ~150 Myr.
    pub steady_start: bool,
}

/// A depth range of the mantle that sunken slabs pass through. Its mass loads the surface
/// through the stage's dynamic-topography kernel (see [`super::kernels::stage_kernels`]).
#[derive(Clone, Debug)]
pub struct SlabStage {
    /// Mean time in Myr a slab spends in the range (its thickness over the sinking speed).
    pub residence: f64,
    /// Top of the depth range in metres.
    pub top: f64,
    /// Bottom of the depth range in metres.
    pub bottom: f64,
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
            slab_grid_subdivisions: 4,
            slab_offset: 660e3,
            slab_stages: vec![
                // The slab from the base of the lithosphere to 660 km at ~5 cm/yr.
                SlabStage {
                    residence: 13.0,
                    top: 100e3,
                    bottom: 660e3,
                },
                // Lower mantle at ~1.2 cm/yr.
                SlabStage {
                    residence: 45.0,
                    top: 660e3,
                    bottom: 1200e3,
                },
                SlabStage {
                    residence: 67.0,
                    top: 1200e3,
                    bottom: 2000e3,
                },
                SlabStage {
                    residence: 74.0,
                    top: 2000e3,
                    bottom: 2891e3,
                },
            ],
            topography_response: 1.0,
            viscosity_layers: vec![(100e3, 100.0), (300e3, 0.1), (660e3, 1.0), (2891e3, 30.0)],
            expansivity: vec![
                (0.0, 3.1e-5),
                (660e3, 2.0e-5),
                (1500e3, 1.45e-5),
                (2891e3, 1.0e-5),
            ],
            cmb_depth: 2891e3,
            slab_load_width: 300e3,
            kernel_degree: 100,
            plume_swell_height: 650.0,
            plume_swell_width: 450e3,
            steady_start: true,
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
    /// Whether the mantle was primed (see [`MantleFlow::prime`]).
    primed: bool,
    /// Green's functions of the slab stages, for the planet radius they were computed for.
    slab_green: Option<(f64, GreenTable)>,
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
            primed: false,
            slab_green: None,
        }
    }

    pub fn plumes(&self) -> &[Plume] {
        &self.plumes
    }

    /// Whether the mantle still waits for [`Self::prime`].
    pub fn needs_priming(&self) -> bool {
        self.params.steady_start && !self.primed
    }

    /// Puts the mantle into the steady state of the current trenches, after the first step
    /// `dt` (Myr) with its `subduction` rates and sinking `slabs` (as for [`Self::step`]),
    /// instead of an empty start (#213):
    ///
    /// * the downwellings hold the trenches' subduction rates (the fixed point of their
    ///   moving average);
    /// * every slab stage holds what a constant inflow leaves in it, the step's deposit over
    ///   the fraction of its mass that leaves it per step (≈ mass flux × residence time), as
    ///   if the trenches had been active for longer than the mantle remembers;
    /// * the plumes are those that the birth process of [`Self::step`] (one per
    ///   `plume_interval` on average) would have left alive over the longest plume lifetime
    ///   before: ~mean lifetime / interval plumes of all ages. Their positions follow the same
    ///   rule as new plumes (far from the downwellings).
    ///
    /// Happens once; later calls do nothing.
    pub fn prime(
        &mut self,
        dt: f64,
        subduction: &[(DVec3, f64)],
        slabs: &[(DVec3, f64)],
        rng: &mut impl Rng,
    ) {
        if self.primed || dt <= 0.0 {
            return;
        }
        self.primed = true;
        let mut hint = 0;
        self.downwelling.fill(0.0);
        for &(position, rate) in subduction {
            let cell = self.grid.nearest_from(position, hint);
            hint = cell;
            self.downwelling[cell as usize] += rate;
        }
        let seconds = dt * SECONDS_PER_MYR;
        let mut deposit = vec![0.0; self.slab_grid.len()];
        let mut hint = 0;
        for &(position, flux) in slabs {
            hint = self.slab_grid.nearest_from(position, hint);
            for (cell, weight) in tent_weights(&self.slab_grid, hint, position) {
                deposit[cell as usize] += flux * seconds * weight;
            }
        }
        for (stage, masses) in self.params.slab_stages.iter().zip(&mut self.slab_mass) {
            let leave = 1.0 - math::exp(-dt / stage.residence);
            for (mass, &deposit) in masses.iter_mut().zip(&deposit) {
                *mass = deposit / leave;
            }
        }
        // The plumes born `age` ago that are still alive, oldest first.
        let params = self.params.clone();
        let oldest = params.plume_lifetime.0.max(params.plume_lifetime.1);
        let steps = (oldest / dt).ceil() as usize;
        let mut plumes = Vec::new();
        for step in (1..=steps).rev() {
            let age = step as f64 * dt;
            if rng.random_bool((dt / params.plume_interval).clamp(0.0, 1.0)) {
                let mut plume = self.new_plume(rng);
                if age < plume.lifetime {
                    plume.age = age;
                    plumes.push(plume);
                }
            }
        }
        self.plumes.extend(plumes);
    }

    /// A new plume, where no slabs have sunk recently.
    fn new_plume(&self, rng: &mut impl Rng) -> Plume {
        let params = &self.params;
        let candidates: Vec<(DVec3, f64)> = (0..params.plume_candidates.max(1))
            .map(|_| random_unit_vector(rng))
            .map(|candidate| (candidate, self.downwelling_inflow(candidate)))
            .collect();
        let (position, _) = candidates
            .into_iter()
            .max_by(|a, b| a.1.total_cmp(&b.1).reverse())
            .expect("at least one candidate");
        Plume {
            position,
            age: 0.0,
            lifetime: rng.random_range(params.plume_lifetime.0..=params.plume_lifetime.1),
            peak_flux: rng.random_range(params.plume_flux.0..=params.plume_flux.1),
        }
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
        // sank out of it during the step (mass-conserving); what leaves the last one rests on
        // the core–mantle boundary, where it no longer deflects the surface.
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
            let plume = self.new_plume(rng);
            self.plumes.push(plume);
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
        if params.topography_response != 0.0
            && !params.slab_stages.is_empty()
            && self
                .slab_green
                .as_ref()
                .is_none_or(|(radius, _)| *radius != planet_radius)
        {
            self.slab_green = Some((planet_radius, slab_green_table(params, planet_radius)));
        }
        let (grid, weights) = self.interpolation.as_ref().expect("interpolation exists");
        // Sunken slabs: their masses through the Green's function of each stage.
        let slab_field = self
            .slab_green
            .as_ref()
            .filter(|_| params.topography_response != 0.0)
            .map(|(_, green)| slab_field(grid, &self.slab_grid, &self.slab_mass, green));
        let mut sources: Vec<(DVec3, f64, f64)> = Vec::new();
        sources.extend(self.plumes.iter().map(|plume| {
            (
                plume.position,
                self.plume_swell_height(plume),
                params.plume_swell_width / planet_radius,
            )
        }));
        let coarse: Vec<f64> = (0..grid.len() as u32)
            .map(|cell| {
                let r = grid.vertex(cell);
                let slab = slab_field.as_ref().map_or(0.0, |field| {
                    params.topography_response * field[cell as usize]
                });
                slab + sources
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
                    .sum::<f64>()
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

    /// Dry height (m) of the swell above `plume` at its centre (see
    /// [`MantleParams::plume_swell_height`]).
    fn plume_swell_height(&self, plume: &Plume) -> f64 {
        let params = &self.params;
        params.plume_swell_height * plume.flux() / params.plume_flux.1.max(1e-12)
    }

    /// Gradient of the plumes' swells (dry height, m per radian of the unit sphere, tangent at
    /// `r`, pointing uphill towards the plumes): the same Gaussian swells as in
    /// [`Self::dynamic_topography`] (without its global mean, which has no gradient), cut off
    /// at the same 4 widths. The swells' potential energy loads the lithosphere (#51).
    pub fn plume_swell_gradient(&self, r: DVec3, planet_radius: f64) -> DVec3 {
        let width = self.params.plume_swell_width / planet_radius;
        if width <= 0.0 {
            return DVec3::ZERO;
        }
        let mut gradient = DVec3::ZERO;
        for plume in &self.plumes {
            // θ² ≈ 2 (1 − cos θ), so h = H exp(−(1 − r·p) / w²) and ∇h = h / w² ∇(r·p).
            let cos = r.dot(plume.position);
            let angle_squared = 2.0 * (1.0 - cos);
            if angle_squared > 16.0 * width * width {
                continue;
            }
            let height =
                self.plume_swell_height(plume) * math::exp(-angle_squared / (2.0 * width * width));
            gradient += (plume.position - r * cos) * (height / (width * width));
        }
        gradient
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

/// The Green's functions of the slab stages of `params` on a planet of `planet_radius`.
fn slab_green_table(params: &MantleParams, planet_radius: f64) -> GreenTable {
    let mantle = LayeredMantle {
        radius: planet_radius,
        cmb_depth: params.cmb_depth,
        layers: &params.viscosity_layers,
    };
    let ranges: Vec<(f64, f64)> = params
        .slab_stages
        .iter()
        .map(|stage| (stage.top, stage.bottom))
        .collect();
    let kernels = stage_kernels(&mantle, &ranges, &params.expansivity, params.kernel_degree);
    for (stage, kernel) in kernels.iter().enumerate() {
        let at = |l: usize| kernel.get(l).copied().unwrap_or(0.0);
        tracing::debug!(
            stage,
            "slab kernel × α/α₀ at l = 2, 5, 10, 20: {:.3}, {:.3}, {:.3}, {:.3}",
            at(2),
            at(5),
            at(10),
            at(20),
        );
    }
    GreenTable::new(
        &kernels,
        planet_radius,
        params.slab_load_width,
        MANTLE_DENSITY,
    )
}

/// Slab cells whose load is below this fraction of the largest are left out of the slab field
/// (they would change it by far less than a millimetre).
const NEGLIGIBLE_SLAB_LOAD: f64 = 1e-9;

/// Dynamic topography (m) of the slab masses (`masses[stage][slab cell]`) at the cells of `grid`.
/// When `grid` is the slab grid itself, each pair of cells is visited once.
fn slab_field(
    grid: &SphereMesh,
    slab_grid: &SphereMesh,
    masses: &[Vec<f64>],
    green: &GreenTable,
) -> Vec<f64> {
    let chunks = green.chunks();
    let cells = slab_grid.len();
    let total = |cell: usize| masses.iter().map(|stage| stage[cell]).sum::<f64>();
    let largest = (0..cells).map(total).fold(0.0, f64::max);
    let occupied: Vec<bool> = (0..cells)
        .map(|cell| total(cell) > NEGLIGIBLE_SLAB_LOAD * largest)
        .collect();
    // Masses per cell and stage, in chunks of `LANES` stages.
    let loads: Vec<[f64; LANES]> = (0..cells)
        .flat_map(|cell| {
            (0..chunks).map(move |chunk| {
                std::array::from_fn(|lane| {
                    masses
                        .get(chunk * LANES + lane)
                        .map_or(0.0, |stage| stage[cell])
                })
            })
        })
        .collect();
    let load = |cell: usize| &loads[cell * chunks..(cell + 1) * chunks];
    if grid.len() == cells {
        // The same icosphere: symmetric pairs.
        let mut sums = vec![[0.0; LANES]; cells];
        for i in 0..cells {
            let r = slab_grid.vertex(i as u32);
            let mut sum_i = sums[i];
            if occupied[i] {
                green.add(r, r, load(i), &mut sum_i);
            }
            for j in i + 1..cells {
                if occupied[i] || occupied[j] {
                    let p = slab_grid.vertex(j as u32);
                    green.add_pair(r, p, load(i), load(j), &mut sum_i, &mut sums[j]);
                }
            }
            sums[i] = sum_i;
        }
        return sums.iter().map(|sum| sum.iter().sum()).collect();
    }
    let sources: Vec<usize> = (0..cells).filter(|&cell| occupied[cell]).collect();
    (0..grid.len() as u32)
        .map(|cell| {
            let r = grid.vertex(cell);
            let mut sum = [0.0; LANES];
            for &source in &sources {
                green.add(r, slab_grid.vertex(source as u32), load(source), &mut sum);
            }
            sum.iter().sum()
        })
        .collect()
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
        for _ in 0..1000 {
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
        });
        let field = mantle.dynamic_topography(&mesh, 6.371e6);
        let at = |target: DVec3| field[mesh.nearest_from(target, 0) as usize];
        let mean = field.iter().sum::<f64>() / field.len() as f64;
        assert!(mean.abs() < 1e-6);
        // About −1.7 km dry on the slab line with the physical kernels (converged; ~10% less
        // on the default subdivision-4 grids, see `slab_field_sensitivity`), up over the plume.
        assert!(
            (-1900.0..-1350.0).contains(&at(DVec3::X)),
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

    /// The slab field of the 5000 km trench of [`slabs_depress_and_plumes_lift_the_surface`],
    /// rotated by `tilt` about +x (to test trenches oblique to the grids), on a subdivision-6
    /// mesh, without plumes: (field, mesh).
    fn trench_field(params: MantleParams, tilt: f64) -> (Vec<f64>, SphereMesh, Vec<DVec3>) {
        let mesh = SphereMesh::icosphere(6);
        let mut mantle = MantleFlow::new(params);
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        let flux = 5e6 * 0.05 / (365.25 * 24.0 * 3600.0) * 7e6;
        let (sin_tilt, cos_tilt) = (math::sin(tilt), math::cos(tilt));
        let line: Vec<DVec3> = (-25..=25)
            .map(|i| {
                let angle = f64::from(i) * 100e3 / 6.371e6;
                let (s, c) = (math::sin(angle), math::cos(angle));
                DVec3::new(c, -s * sin_tilt, s * cos_tilt)
            })
            .collect();
        let slabs: Vec<(DVec3, f64)> = line.iter().map(|&p| (p, flux / 51.0)).collect();
        for _ in 0..1000 {
            mantle.step(1.0, &[], &slabs, &mut rng);
        }
        mantle.plumes.clear();
        (mantle.dynamic_topography(&mesh, 6.371e6), mesh, line)
    }

    /// Diagnostics of the slab field's sensitivity to the grids and the load width (see
    /// `notes/issue-37/results-kernels.md`): `cargo test --release slab_field_sensitivity --
    /// --ignored --nocapture`.
    #[test]
    #[ignore]
    fn slab_field_sensitivity() {
        let profile = |params: MantleParams, tilt: f64| {
            let start = std::time::Instant::now();
            let (field, mesh, line) = trench_field(params, tilt);
            let seconds = start.elapsed().as_secs_f64();
            let along: Vec<f64> = line[10..41]
                .iter()
                .map(|&p| field[mesh.nearest_from(p, 0) as usize])
                .collect();
            (along, field.iter().copied().fold(0.0, f64::min), seconds)
        };
        // Reference: fine slab grid, evaluated on the cells.
        let references: Vec<Vec<f64>> = [0.0, 0.4]
            .iter()
            .map(|&tilt| {
                let params = MantleParams {
                    slab_grid_subdivisions: 5,
                    evaluation_subdivisions: 6,
                    ..MantleParams::default()
                };
                profile(params, tilt).0
            })
            .collect();
        let report = |name: &str, params: MantleParams| {
            for (tilt, reference) in [0.0, 0.4].into_iter().zip(&references) {
                let (along, deepest, seconds) = profile(params.clone(), tilt);
                let centre = along[15];
                let error = along
                    .iter()
                    .zip(reference)
                    .map(|(a, r)| (a - r).abs())
                    .fold(0.0, f64::max);
                tracing::info!(
                    "{name:42} tilt {tilt:.1}: centre {centre:6.0} m (reference {:6.0}), deepest {deepest:6.0} m, largest deviation along the central 3000 km {:4.1}% ({seconds:.2} s)",
                    reference[15],
                    100.0 * error / reference[15].abs()
                );
            }
        };
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
        report("default", MantleParams::default());
        report(
            "evaluation on the cells (subdivision 6)",
            MantleParams {
                evaluation_subdivisions: 6,
                ..MantleParams::default()
            },
        );
        report(
            "evaluation subdivision 5",
            MantleParams {
                evaluation_subdivisions: 5,
                ..MantleParams::default()
            },
        );
        report(
            "slab grid 4, evaluation on the cells",
            MantleParams {
                slab_grid_subdivisions: 4,
                evaluation_subdivisions: 6,
                ..MantleParams::default()
            },
        );
        report(
            "slab grid 4, evaluation 4",
            MantleParams {
                slab_grid_subdivisions: 4,
                ..MantleParams::default()
            },
        );
        report(
            "slab grid 6, evaluation on the cells",
            MantleParams {
                slab_grid_subdivisions: 6,
                evaluation_subdivisions: 6,
                ..MantleParams::default()
            },
        );
        // Mixed: stage 1 on slab grid 4, stages 2–4 on slab grid 3.
        let only = |keep: &[usize], slab: u32, evaluation: u32| {
            let mut params = MantleParams {
                slab_grid_subdivisions: slab,
                evaluation_subdivisions: evaluation,
                ..MantleParams::default()
            };
            for (s, slab_stage) in params.slab_stages.iter_mut().enumerate() {
                if !keep.contains(&s) {
                    slab_stage.top = 2890e3;
                    slab_stage.bottom = 2890.9e3;
                }
            }
            params
        };
        for evaluation in [4, 6] {
            for (stage1, rest) in [(4, 3), (3, 4)] {
                let (a, _, _) = profile(only(&[0], stage1, evaluation), 0.0);
                let (b, _, _) = profile(only(&[1, 2, 3], rest, evaluation), 0.0);
                tracing::info!(
                    "stage 1 on grid {stage1}, rest on grid {rest}, evaluation {evaluation}: centre {:.0} m (stage 1 {:.0}, rest {:.0})",
                    a[15] + b[15],
                    a[15],
                    b[15]
                );
            }
        }
        // Cost of one evaluation with every slab cell loaded (the worst case).
        for (slab, evaluation) in [(3, 4), (4, 4), (4, 5), (5, 4)] {
            let mut mantle = MantleFlow::new(MantleParams {
                slab_grid_subdivisions: slab,
                evaluation_subdivisions: evaluation,
                ..MantleParams::default()
            });
            for stage in &mut mantle.slab_mass {
                stage.fill(1e15);
            }
            let mesh = SphereMesh::icosphere(6);
            mantle.dynamic_topography(&mesh, 6.371e6);
            let start = std::time::Instant::now();
            for _ in 0..10 {
                std::hint::black_box(mantle.dynamic_topography(&mesh, 6.371e6));
            }
            tracing::info!(
                "slab grid {slab}, evaluation {evaluation}: {:.1} ms per call, all slab cells loaded",
                start.elapsed().as_secs_f64() * 100.0
            );
        }
        report(
            "slab grid 4, evaluation 5",
            MantleParams {
                slab_grid_subdivisions: 4,
                evaluation_subdivisions: 5,
                ..MantleParams::default()
            },
        );
        for width in [200e3, 450e3] {
            report(
                &format!("load width {} km, evaluation on the cells", width / 1e3),
                MantleParams {
                    slab_load_width: width,
                    evaluation_subdivisions: 6,
                    ..MantleParams::default()
                },
            );
        }
        report(
            "lower mantle 100×, evaluation on the cells",
            MantleParams {
                viscosity_layers: vec![(100e3, 100.0), (300e3, 0.1), (660e3, 1.0), (2891e3, 100.0)],
                evaluation_subdivisions: 6,
                ..MantleParams::default()
            },
        );
        report(
            "no asthenosphere, evaluation on the cells",
            MantleParams {
                viscosity_layers: vec![(100e3, 100.0), (660e3, 1.0), (2891e3, 30.0)],
                evaluation_subdivisions: 6,
                ..MantleParams::default()
            },
        );
        for stage in 0..4 {
            let mut params = MantleParams {
                evaluation_subdivisions: 6,
                ..MantleParams::default()
            };
            // Only one stage loads the surface: the others keep their residence but get a
            // depth range at the CMB, where the kernel is zero.
            for (s, slab_stage) in params.slab_stages.iter_mut().enumerate() {
                if s != stage {
                    slab_stage.top = 2890e3;
                    slab_stage.bottom = 2890.9e3;
                }
            }
            report(&format!("only stage {}, on the cells", stage + 1), params);
        }
    }

    #[test]
    fn swell_gradients_point_uphill_to_the_plume() {
        let mut mantle = MantleFlow::new(MantleParams::default());
        let plume = Plume {
            position: DVec3::Z,
            age: 50.0,
            lifetime: 200.0,
            peak_flux: 6e-3,
        };
        mantle.plumes.push(plume.clone());
        let radius = 6.371e6;
        let width = mantle.params.plume_swell_width / radius;
        let height = |r: DVec3| {
            mantle.plume_swell_height(&plume)
                * math::exp(-2.0 * (1.0 - r.dot(DVec3::Z)) / (2.0 * width * width))
        };
        for angle in [0.02, 0.05, 0.1, 0.2] {
            let r = DVec3::new(math::sin(angle), 0.0, math::cos(angle));
            let g = mantle.plume_swell_gradient(r, radius);
            // Towards the plume (−x at this point), and as steep as the swell.
            let towards = DVec3::new(-math::cos(angle), 0.0, math::sin(angle));
            let e = 1e-5;
            let step = |a: f64| DVec3::new(math::sin(a), 0.0, math::cos(a));
            let slope = (height(step(angle - e)) - height(step(angle + e))) / (2.0 * e);
            assert!(
                (g.dot(towards) / slope - 1.0).abs() < 1e-3,
                "{angle}: {g} vs {slope}"
            );
            assert!(g.dot(r).abs() < 1e-9 * g.length().max(1.0));
        }
        // Nothing beyond four widths.
        let far = DVec3::new(math::sin(5.0 * width), 0.0, math::cos(5.0 * width));
        assert_eq!(mantle.plume_swell_gradient(far, radius), DVec3::ZERO);
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

    /// Priming gives the steady state at once: the downwellings hold the rates, every slab stage
    /// the fixed point of its turnover (so a further step changes nothing), and the plumes are a
    /// steady population of all ages.
    #[test]
    fn priming_starts_in_the_steady_state() {
        let subduction = [(DVec3::X, 0.05), (DVec3::Y, 0.02)];
        let slabs = [(DVec3::X, 4e7), (DVec3::Z, 1e7)];
        let mut counts = Vec::new();
        for seed in 0..40 {
            let params = MantleParams {
                plume_interval: 1e12,
                ..MantleParams::default()
            };
            let mut mantle = MantleFlow::new(params);
            assert!(mantle.needs_priming());
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            mantle.step(1.0, &subduction, &slabs, &mut rng);
            // Births only in the priming (they are switched off in the steps).
            mantle.params.plume_interval = 15.0;
            mantle.prime(1.0, &subduction, &slabs, &mut rng);
            mantle.params.plume_interval = 1e12;
            assert!(!mantle.needs_priming());
            let total: f64 = mantle.downwelling.iter().sum();
            assert!((total - 0.07).abs() < 1e-12, "{total}");
            let before = mantle.slab_mass.clone();
            let plumes = mantle.plumes.clone();
            for plume in &plumes {
                assert!(plume.age > 0.0 && plume.age < plume.lifetime);
            }
            counts.push(plumes.len());
            mantle.step(1.0, &subduction, &slabs, &mut rng);
            for (stage, (now, then)) in mantle.slab_mass.iter().zip(&before).enumerate() {
                let (now, then): (f64, f64) = (now.iter().sum(), then.iter().sum());
                assert!(then > 0.0 && (now / then - 1.0).abs() < 1e-9, "{stage}");
            }
            let total: f64 = mantle.downwelling.iter().sum();
            assert!((total - 0.07).abs() < 1e-12, "{total}");
        }
        let mean = counts.iter().sum::<usize>() as f64 / counts.len() as f64;
        // Mean lifetime / interval = 140 / 15 ≈ 9.3.
        assert!((7.5..11.0).contains(&mean), "{mean}");
    }
}
