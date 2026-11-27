//! Surface dynamic-topography kernels of a radially layered viscous mantle.
//!
//! A density anomaly in the mantle drives viscous flow that deflects the surface. For an
//! incompressible, radially layered Newtonian sphere with free-slip surface and core–mantle
//! boundary (CMB), the response to a load of spherical-harmonic degree `l` at depth `d` is the
//! kernel `K_l(d)`: the fraction of the load's weight that the surface supports by its
//! deflection (1 for a load at the surface, 0 at the CMB, where the CMB supports all of it).
//! The surface deflection is then `h_l = −K_l(d) σ_l / ρ_m` for a load of `σ_l` kg/m² (dry, no
//! self-gravitation).
//!
//! The kernels come from the propagator method of Hager & O'Connell (1981) and Hager & Clayton
//! (1989): the poloidal state `y = (u_r, u_θ, r σ_rr / η₀, r σ_rθ / η₀)` obeys
//! `dy / d ln r = A(l, η(r)) y` (see [`a_matrix`]). Two solutions are integrated up from the
//! CMB and two down from the surface (both with `u_r = 0` and `σ_rθ = 0`), and they are joined
//! at the load by the jump of the normal stress. The columns are re-orthonormalised after every
//! step, so that the fast-growing solutions of high degrees do not swamp the others.
//! See `notes/issue-37/plan-kernels.md`.

use crate::math;

/// Coefficient matrix `A(l, η)` of the poloidal state, with `L = l (l + 1)` and the viscosity
/// relative to the reference viscosity (non-dimensional: radius 1, η₀ = 1). Verified
/// symbolically against the analytic constant-viscosity Stokes solutions
/// (`notes/issue-37/kernels_prototype.py`).
fn a_matrix(l: u32, viscosity: f64) -> [[f64; 4]; 4] {
    let big_l = f64::from(l) * f64::from(l + 1);
    let eta = viscosity;
    [
        [-2.0, big_l, 0.0, 0.0],
        [-1.0, 1.0, 0.0, 1.0 / eta],
        [12.0 * eta, -6.0 * big_l * eta, 1.0, big_l],
        [-6.0 * eta, 2.0 * (2.0 * big_l - 1.0) * eta, -1.0, -2.0],
    ]
}

fn apply(a: &[[f64; 4]; 4], y: &[f64; 4]) -> [f64; 4] {
    std::array::from_fn(|i| (0..4).map(|j| a[i][j] * y[j]).sum())
}

fn axpy(y: &[f64; 4], h: f64, k: &[f64; 4]) -> [f64; 4] {
    std::array::from_fn(|i| y[i] + h * k[i])
}

fn dot(a: &[f64; 4], b: &[f64; 4]) -> f64 {
    (0..4).map(|i| a[i] * b[i]).sum()
}

/// A pair of propagated solutions, with the surface normal stress of each (tracked through the
/// orthonormalisation; only used for the solutions coming from the surface).
#[derive(Clone, Copy)]
struct Pair {
    columns: [[f64; 4]; 2],
    surface_stress: [f64; 2],
}

impl Pair {
    /// The two free-slip solutions at a boundary: `u_θ = 1` and `r σ_rr / η₀ = 1`.
    fn free_slip() -> Self {
        Self {
            columns: [[0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]],
            surface_stress: [0.0, 1.0],
        }
    }

    /// One classical Runge–Kutta step of `h` in `ln r` with the constant matrix `a`.
    fn step(&mut self, a: &[[f64; 4]; 4], h: f64) {
        for y in &mut self.columns {
            let k1 = apply(a, y);
            let k2 = apply(a, &axpy(y, h / 2.0, &k1));
            let k3 = apply(a, &axpy(y, h / 2.0, &k2));
            let k4 = apply(a, &axpy(y, h, &k3));
            *y = std::array::from_fn(|i| {
                y[i] + h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i])
            });
        }
    }

    /// Gram–Schmidt orthonormalisation of the two columns. It only changes the basis of the
    /// solution space they span; the surface stresses follow the same linear map.
    fn orthonormalise(&mut self) {
        let [y0, y1] = self.columns;
        let n0 = dot(&y0, &y0).sqrt();
        let q0: [f64; 4] = std::array::from_fn(|i| y0[i] / n0);
        let projection = dot(&q0, &y1);
        let r1: [f64; 4] = std::array::from_fn(|i| y1[i] - projection * q0[i]);
        let n1 = dot(&r1, &r1).sqrt();
        let [s0, s1] = self.surface_stress;
        self.columns = [q0, std::array::from_fn(|i| r1[i] / n1)];
        self.surface_stress = [s0 / n0, (s1 - projection * s0 / n0) / n1];
    }
}

/// A radially layered mantle.
#[derive(Clone, Debug)]
pub struct LayeredMantle<'a> {
    /// Planet radius in metres.
    pub radius: f64,
    /// Depth of the core–mantle boundary in metres.
    pub cmb_depth: f64,
    /// Viscosity layers from the top: bottom depth in metres and viscosity relative to a
    /// reference. The last layer extends to the CMB.
    pub layers: &'a [(f64, f64)],
}

impl LayeredMantle<'_> {
    fn viscosity(&self, depth: f64) -> f64 {
        self.layers
            .iter()
            .find(|&&(bottom, _)| depth < bottom)
            .or(self.layers.last())
            .map_or(1.0, |&(_, eta)| eta)
    }

    /// `ln(r / R)` at a depth.
    fn log_radius(&self, depth: f64) -> f64 {
        math::ln((self.radius - depth) / self.radius)
    }

    /// Surface kernels `K_l(d)` of degree `l` (≥ 1) at the given depths (metres, strictly
    /// between the surface and the CMB).
    pub fn kernels(&self, l: u32, depths: &[f64]) -> Vec<f64> {
        self.kernels_with_resolution(l, depths, 6.0)
    }

    /// [`Self::kernels`] with `resolution (l + 3)` Runge–Kutta steps per unit of `ln r`.
    fn kernels_with_resolution(&self, l: u32, depths: &[f64], resolution: f64) -> Vec<f64> {
        // Integration knots: the surface, the layer boundaries, the sample depths and the CMB,
        // from the top. Each interval has a constant viscosity.
        let mut knots: Vec<f64> = std::iter::once(0.0)
            .chain(self.layers.iter().map(|&(bottom, _)| bottom))
            .chain(depths.iter().copied())
            .filter(|&depth| (0.0..self.cmb_depth).contains(&depth))
            .chain(std::iter::once(self.cmb_depth))
            .collect();
        knots.sort_by(f64::total_cmp);
        knots.dedup();
        let x: Vec<f64> = knots.iter().map(|&depth| self.log_radius(depth)).collect();
        // Steps per unit of ln r: the solutions vary like r^(±l), so `h l` stays ≈ 1/6.
        let per_unit = resolution * f64::from(l + 3);
        let propagate = |pair: &mut Pair, from: usize, to: usize| {
            let middle = 0.5 * (knots[from] + knots[to]);
            let a = a_matrix(l, self.viscosity(middle));
            let length = x[to] - x[from];
            let steps = ((length.abs() * per_unit).ceil() as usize).max(2);
            let h = length / steps as f64;
            for _ in 0..steps {
                pair.step(&a, h);
                pair.orthonormalise();
            }
        };
        let index = |depth: f64| {
            knots
                .iter()
                .position(|&knot| knot == depth)
                .expect("sample depths are knots")
        };
        // Downwards from the surface and upwards from the CMB, keeping the pair at every knot.
        let n = knots.len();
        let mut from_surface = vec![Pair::free_slip(); n];
        for k in 1..n {
            let mut pair = from_surface[k - 1];
            propagate(&mut pair, k - 1, k);
            from_surface[k] = pair;
        }
        let mut from_cmb = vec![Pair::free_slip(); n];
        for k in (0..n - 1).rev() {
            let mut pair = from_cmb[k + 1];
            propagate(&mut pair, k + 1, k);
            from_cmb[k] = pair;
        }
        depths
            .iter()
            .map(|&depth| {
                let k = index(depth);
                let r = (self.radius - depth) / self.radius;
                let above = from_surface[k];
                let below = from_cmb[k];
                // above · c_a − below · c_b = jump of r σ_rr / η₀ for a unit load (m g / η₀ = 1).
                let mut matrix = [[0.0; 5]; 4];
                for (i, row) in matrix.iter_mut().enumerate() {
                    *row = [
                        above.columns[0][i],
                        above.columns[1][i],
                        -below.columns[0][i],
                        -below.columns[1][i],
                        if i == 2 { r } else { 0.0 },
                    ];
                }
                let c = solve4(matrix);
                (above.surface_stress[0] * c[0] + above.surface_stress[1] * c[1]) / (r * r)
            })
            .collect()
    }
}

/// Number of sample depths per stage in [`stage_kernels`].
const STAGE_SAMPLES: usize = 8;

/// Kernels of depth stages, `[stage][l]` for `l` up to `l_max`: the mean of
/// `K_l(d) α(d) / α(0)` over the stage's depth range `(top, bottom)` (metres), i.e. for slab
/// mass spread evenly over the range whose excess mass shrinks with the thermal expansivity
/// `α` (`expansivity`: piecewise-linear `(depth, α)`; the heat deficit of a slab is conserved,
/// so its excess mass `∫ρ α ΔT dV = (α / c_p) ∫ρ c_p ΔT dV` follows `α`).
///
/// Degrees 0 and 1 are zero. Degree 0 because the mantle's mass is conserved; degree 1 because
/// the response to it depends on how the core is held (here: fixed, `u_r = 0` at the CMB, no
/// self-gravitation), i.e. on the reference frame, and would tilt the whole surface towards the
/// slab graveyards.
pub fn stage_kernels(
    mantle: &LayeredMantle,
    stages: &[(f64, f64)],
    expansivity: &[(f64, f64)],
    l_max: u32,
) -> Vec<Vec<f64>> {
    let depths: Vec<f64> = stages
        .iter()
        .flat_map(|&(top, bottom)| {
            (0..STAGE_SAMPLES)
                .map(move |i| top + (bottom - top) * (i as f64 + 0.5) / STAGE_SAMPLES as f64)
        })
        .collect();
    let surface_expansivity = interpolate(expansivity, 0.0);
    let factors: Vec<f64> = depths
        .iter()
        .map(|&depth| interpolate(expansivity, depth) / surface_expansivity)
        .collect();
    let mut result = vec![vec![0.0; l_max as usize + 1]; stages.len()];
    for l in 2..=l_max {
        let kernels = mantle.kernels(l, &depths);
        for (s, stage) in result.iter_mut().enumerate() {
            let range = s * STAGE_SAMPLES..(s + 1) * STAGE_SAMPLES;
            stage[l as usize] = kernels[range.clone()]
                .iter()
                .zip(&factors[range])
                .map(|(k, f)| k * f)
                .sum::<f64>()
                / STAGE_SAMPLES as f64;
        }
    }
    result
}

/// Solves a 4×4 linear system given as an augmented matrix, by Gaussian elimination with
/// partial pivoting.
fn solve4(mut m: [[f64; 5]; 4]) -> [f64; 4] {
    for column in 0..4 {
        let pivot = (column..4)
            .max_by(|&a, &b| m[a][column].abs().total_cmp(&m[b][column].abs()))
            .expect("rows left");
        m.swap(column, pivot);
        for row in column + 1..4 {
            let factor = m[row][column] / m[column][column];
            for k in column..5 {
                m[row][k] -= factor * m[column][k];
            }
        }
    }
    let mut x = [0.0; 4];
    for row in (0..4).rev() {
        let sum: f64 = (row + 1..4).map(|k| m[row][k] * x[k]).sum();
        x[row] = (m[row][4] - sum) / m[row][row];
    }
    x
}

/// Piecewise-linear interpolation in a table of `(depth, value)` sorted by depth, constant
/// beyond its ends.
pub fn interpolate(table: &[(f64, f64)], depth: f64) -> f64 {
    let Some(&(first_depth, first)) = table.first() else {
        return 1.0;
    };
    if depth <= first_depth {
        return first;
    }
    for pair in table.windows(2) {
        let [(d0, v0), (d1, v1)] = [pair[0], pair[1]];
        if depth <= d1 {
            return v0 + (v1 - v0) * (depth - d0) / (d1 - d0);
        }
    }
    table.last().map_or(first, |&(_, value)| value)
}

/// Stages per chunk of a [`GreenTable`] entry: the loads and the table come in fixed-size chunks
/// of this many stages (unused stages zero), which keeps the inner loop free of bounds checks.
pub const LANES: usize = 4;

/// One chunk of a [`GreenTable`] node: the values of its stages and their increase to the next
/// node.
#[derive(Clone, Copy, Default)]
struct Node {
    value: [f64; LANES],
    slope: [f64; LANES],
}

/// Surface Green's functions of slab loads, one per depth stage, tabulated against the chord
/// length `|r − p|` (0 to 2 on the unit sphere) between the surface point `r` and the load `p`.
pub struct GreenTable {
    /// Chunks of [`LANES`] stages per node.
    chunks: usize,
    /// `nodes[i · chunks + c]`: deflection in metres per kg of the stages of chunk `c` at chord
    /// node `i`.
    nodes: Vec<Node>,
}

/// Number of chord nodes of a [`GreenTable`] (spacing ~6 km on an Earth-sized planet).
const GREEN_NODES: usize = 2048;

impl GreenTable {
    /// Tabulates `G_s(θ) = −Σ_{l=2}^{l_max} (2l + 1) / (4π R² ρ_m) K_{s,l} e^{−l(l+1) w² / 2R²}
    /// P_l(cos θ)` from the stage kernels `kernels[s][l]` (see [`stage_kernels`]), with the
    /// Gaussian load width `w`. Without degree 0 (the mantle's mass is conserved) the field of
    /// each load has zero mean.
    pub fn new(kernels: &[Vec<f64>], radius: f64, load_width: f64, density: f64) -> Self {
        let stages = kernels.len();
        let chunks = stages.div_ceil(LANES);
        let l_max = kernels
            .iter()
            .map(|k| k.len().saturating_sub(1))
            .max()
            .unwrap_or(0);
        let width = load_width / radius;
        let weight = |s: usize, l: usize| {
            let l_f = l as f64;
            let k = if l == 0 {
                0.0
            } else {
                kernels[s].get(l).copied().unwrap_or(0.0)
            };
            -(2.0 * l_f + 1.0) * k * math::exp(-l_f * (l_f + 1.0) * width * width / 2.0)
                / (4.0 * std::f64::consts::PI * radius * radius * density)
        };
        let weights: Vec<Vec<f64>> = (0..stages)
            .map(|s| (0..=l_max.max(1)).map(|l| weight(s, l)).collect())
            .collect();
        let mut values = vec![0.0; GREEN_NODES * stages];
        for i in 0..GREEN_NODES {
            let chord = 2.0 * i as f64 / (GREEN_NODES - 1) as f64;
            let cos = 1.0 - chord * chord / 2.0;
            for s in 0..stages {
                values[i * stages + s] = weights[s][0] + weights[s][1] * cos;
            }
            // Legendre recursion.
            let (mut previous, mut current) = (1.0, cos);
            for l in 1..l_max {
                let l_f = l as f64;
                let next = ((2.0 * l_f + 1.0) * cos * current - l_f * previous) / (l_f + 1.0);
                previous = current;
                current = next;
                for s in 0..stages {
                    values[i * stages + s] += weights[s][l + 1] * next;
                }
            }
        }
        let mut nodes = vec![Node::default(); GREEN_NODES * chunks];
        for i in 0..GREEN_NODES {
            for s in 0..stages {
                let node = &mut nodes[i * chunks + s / LANES];
                let value = values[i * stages + s];
                let next = values.get((i + 1) * stages + s).copied().unwrap_or(value);
                node.value[s % LANES] = value;
                node.slope[s % LANES] = next - value;
            }
        }
        Self { chunks, nodes }
    }

    /// Number of load chunks per position (see [`LANES`]).
    pub fn chunks(&self) -> usize {
        self.chunks
    }

    /// The deflections between two positions `r` and `p` (unit vectors): `(at r by the loads
    /// at p, at p by the loads at r)`, each as per-lane sums (see [`Self::add`]).
    #[inline]
    pub fn add_pair(
        &self,
        r: glam::DVec3,
        p: glam::DVec3,
        loads_r: &[[f64; LANES]],
        loads_p: &[[f64; LANES]],
        sum_r: &mut [f64; LANES],
        sum_p: &mut [f64; LANES],
    ) {
        let chord = (2.0 - 2.0 * r.dot(p)).max(0.0).sqrt();
        let position = chord * ((GREEN_NODES - 1) as f64 / 2.0);
        let i = (position as usize).min(GREEN_NODES - 1);
        let f = position - i as f64;
        let nodes = &self.nodes[i * self.chunks..(i + 1) * self.chunks];
        for ((node, load_r), load_p) in nodes.iter().zip(loads_r).zip(loads_p) {
            for lane in 0..LANES {
                let g = node.value[lane] + f * node.slope[lane];
                sum_r[lane] += load_p[lane] * g;
                sum_p[lane] += load_r[lane] * g;
            }
        }
    }

    /// Adds the deflection (m) at the unit vector `r` of the loads at the unit vector `p` to
    /// `sum`: `loads` holds [`Self::chunks`] chunks of masses (kg) per stage.
    #[inline]
    pub fn add(
        &self,
        r: glam::DVec3,
        p: glam::DVec3,
        loads: &[[f64; LANES]],
        sum: &mut [f64; LANES],
    ) {
        let chord = (2.0 - 2.0 * r.dot(p)).max(0.0).sqrt();
        let position = chord * ((GREEN_NODES - 1) as f64 / 2.0);
        let i = (position as usize).min(GREEN_NODES - 1);
        let f = position - i as f64;
        let nodes = &self.nodes[i * self.chunks..(i + 1) * self.chunks];
        for (node, load) in nodes.iter().zip(loads) {
            for lane in 0..LANES {
                sum[lane] += load[lane] * (node.value[lane] + f * node.slope[lane]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: f64 = 6371e3;
    const CMB: f64 = R - 3480e3;

    fn uniform() -> LayeredMantle<'static> {
        LayeredMantle {
            radius: R,
            cmb_depth: CMB,
            layers: &[(1e9, 1.0)],
        }
    }

    fn layered() -> LayeredMantle<'static> {
        LayeredMantle {
            radius: R,
            cmb_depth: CMB,
            layers: &[(100e3, 100.0), (300e3, 0.1), (660e3, 1.0), (1e9, 30.0)],
        }
    }

    #[test]
    fn kernels_match_the_prototype() {
        // Values of notes/issue-37/kernels_prototype.py (independent integration, no
        // orthonormalisation, matrix verified against analytic Stokes solutions), with the
        // viscosity taken constant per layer (the prototype read the deeper layer's viscosity at
        // the Runge–Kutta points on a layer boundary, ~1% off in layered profiles).
        let cases = [
            (uniform(), 2, 1000e3, 0.82586),
            (uniform(), 8, 500e3, 0.87904),
            (uniform(), 1, 1000e3, 0.77588),
            (uniform(), 40, 330e3, 0.37594),
            (layered(), 2, 400e3, 0.86039),
            (layered(), 2, 1500e3, 0.56873),
            (layered(), 10, 800e3, 0.09057),
            (layered(), 2, 2500e3, 0.20198),
        ];
        for (mantle, l, depth, expected) in cases {
            let k = mantle.kernels(l, &[depth])[0];
            assert!(
                (k - expected).abs() < 1e-4,
                "l {l} d {depth}: {k} vs {expected}"
            );
        }
    }

    #[test]
    fn kernels_converge() {
        let depths = [150e3, 400e3, 900e3, 1600e3, 2400e3];
        for l in [1, 2, 5, 20, 100] {
            let coarse = layered().kernels(l, &depths);
            let fine = layered().kernels_with_resolution(l, &depths, 48.0);
            for (c, f) in coarse.iter().zip(&fine) {
                assert!((c - f).abs() < 1e-5, "l {l}: {c} vs {f}");
            }
        }
    }

    #[test]
    fn kernels_have_the_right_limits() {
        let mantle = layered();
        let depths = [1e3, CMB - 1e3];
        for l in [1, 2, 10, 50, 100] {
            let k = mantle.kernels(l, &depths);
            assert!((k[0] - 1.0).abs() < 0.01, "l {l} surface {}", k[0]);
            assert!(k[1].abs() < 0.01, "l {l} CMB {}", k[1]);
        }
        // Half-space limit at short wavelengths: (1 + kd) e^(−kd).
        let (l, depth) = (100, 150e3);
        let kd = (f64::from(l) + 0.5) / R * depth;
        let expected = (1.0 + kd) * math::exp(-kd);
        let k = uniform().kernels(l, &[depth])[0];
        assert!((k - expected).abs() < 0.03, "{k} vs {expected}");
    }

    #[test]
    fn green_function_integrates_to_zero_and_peaks_above_the_load() {
        let kernels: Vec<Vec<f64>> =
            vec![(0..=60).map(|l| if l < 2 { 0.0 } else { 0.8 }).collect()];
        let table = GreenTable::new(&kernels, R, 300e3, 3300.0);
        // ∫ G dA = 2π R² ∫ G(θ) sin θ dθ = π R² ∫ G d(chord²) over chord ∈ [0, 2].
        let n = 20000;
        let mut integral = 0.0;
        let mut peak = 0.0;
        for i in 0..n {
            let chord = 2.0 * (i as f64 + 0.5) / n as f64;
            let cos = 1.0 - chord * chord / 2.0;
            let sin = (1.0 - cos * cos).max(0.0).sqrt();
            let r = glam::DVec3::new(cos, sin, 0.0);
            let mut sum = [0.0; LANES];
            table.add(r, glam::DVec3::X, &[[1.0, 0.0, 0.0, 0.0]], &mut sum);
            let g = sum[0];
            integral += g * std::f64::consts::PI * R * R * (4.0 / n as f64) * chord;
            if i == 0 {
                peak = g;
            }
        }
        // Without degree 0 a load deflects by 0.8 / ρ_m m³ per kg near it and nothing on average.
        assert!(integral.abs() < 1e-3 * 0.8 / 3300.0, "{integral}");
        assert!(peak < 0.0);
    }
}
