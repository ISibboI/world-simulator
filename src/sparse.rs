//! Sparse linear systems on the icosphere: BiCGSTAB preconditioned with a geometric multigrid
//! V-cycle over the nested icosphere levels.
//!
//! Every subdivision of the icosphere keeps the vertices of the coarser level (with the same
//! indices) and adds the midpoints of its edges, so the levels nest. Prolongation interpolates
//! linearly (a midpoint gets the mean of its two parents), restriction is its transpose, and the
//! coarse matrices are Galerkin products `Pᵀ A P`. Gauss–Seidel sweeps smooth on every level.

use crate::sphere::SphereMesh;

/// The sparsity pattern of a square matrix in compressed rows; the first entry of every row is
/// its diagonal.
#[derive(Clone, Debug, Default)]
struct Pattern {
    row_start: Vec<usize>,
    columns: Vec<u32>,
}

impl Pattern {
    fn len(&self) -> usize {
        self.row_start.len() - 1
    }

    /// `y = A x` for the matrix with the given entries.
    fn multiply(&self, values: &[f64], x: &[f64], y: &mut [f64]) {
        for (row, y) in y.iter_mut().enumerate() {
            let range = self.row_start[row]..self.row_start[row + 1];
            *y = self.columns[range.clone()]
                .iter()
                .zip(&values[range])
                .map(|(&c, &v)| v * x[c as usize])
                .sum();
        }
    }

    /// One Gauss–Seidel sweep for `A x = b`, forwards or backwards through the rows.
    fn gauss_seidel(&self, values: &[f64], b: &[f64], x: &mut [f64], forward: bool) {
        let mut sweep = |row: usize| {
            let start = self.row_start[row];
            let range = start + 1..self.row_start[row + 1];
            let off: f64 = self.columns[range.clone()]
                .iter()
                .zip(&values[range])
                .map(|(&c, &v)| v * x[c as usize])
                .sum();
            x[row] = (b[row] - off) / values[start];
        };
        if forward {
            (0..self.len()).for_each(&mut sweep);
        } else {
            (0..self.len()).rev().for_each(&mut sweep);
        }
    }
}

/// Linear interpolation from a coarse icosphere level to the next finer one.
#[derive(Clone, Debug)]
struct Prolongation {
    coarse_count: usize,
    /// The coarse cells (and weights) every fine cell interpolates from.
    weights: Vec<[(u32, f64); 2]>,
    /// Number of parents of every fine cell (1 or 2).
    counts: Vec<u8>,
}

impl Prolongation {
    fn parents(&self, fine: usize) -> &[(u32, f64)] {
        &self.weights[fine][..self.counts[fine] as usize]
    }

    fn prolong_add(&self, coarse: &[f64], fine: &mut [f64]) {
        for (cell, value) in fine.iter_mut().enumerate() {
            *value += self
                .parents(cell)
                .iter()
                .map(|&(c, w)| w * coarse[c as usize])
                .sum::<f64>();
        }
    }

    fn restrict(&self, fine: &[f64], coarse: &mut [f64]) {
        coarse.fill(0.0);
        for (cell, &value) in fine.iter().enumerate() {
            for &(c, w) in self.parents(cell) {
                coarse[c as usize] += w * value;
            }
        }
    }
}

/// One level of the hierarchy below the finest: how to get there and its pattern.
#[derive(Clone, Debug)]
struct Coarser {
    prolongation: Prolongation,
    pattern: Pattern,
    /// Contributions `(finer entry, coarser entry, weight)` of the Galerkin product `Pᵀ A P`.
    galerkin: Vec<(u32, u32, f64)>,
}

/// The nested icosphere levels below a mesh, for multigrid. The finest matrix has the pattern
/// of the mesh: every row starts with the diagonal, followed by the neighbours in the order of
/// [`SphereMesh::neighbors`].
#[derive(Clone, Debug)]
pub struct Hierarchy {
    finest: Pattern,
    /// Coarser levels, finest first.
    levels: Vec<Coarser>,
}

impl Hierarchy {
    /// Levels down to subdivision `coarsest`.
    pub fn new(mesh: &SphereMesh, coarsest: u32) -> Self {
        let mut row_start = vec![0];
        let mut columns = Vec::new();
        for cell in 0..mesh.len() as u32 {
            columns.push(cell);
            columns.extend_from_slice(mesh.neighbors(cell));
            row_start.push(columns.len());
        }
        let finest = Pattern { row_start, columns };

        let mut levels: Vec<Coarser> = Vec::new();
        let mut fine_subdivisions = mesh.subdivisions();
        while fine_subdivisions > coarsest {
            let built;
            let fine_mesh = if fine_subdivisions == mesh.subdivisions() {
                mesh
            } else {
                built = SphereMesh::icosphere(fine_subdivisions);
                &built
            };
            let prolongation = prolongation(fine_mesh, fine_subdivisions);
            let fine_pattern = levels.last().map_or(&finest, |level| &level.pattern);
            let (pattern, galerkin) = galerkin_pattern(fine_pattern, &prolongation);
            levels.push(Coarser {
                prolongation,
                pattern,
                galerkin,
            });
            fine_subdivisions -= 1;
        }
        Self { finest, levels }
    }
}

/// Prolongation from subdivision `subdivisions − 1` to the mesh with `subdivisions`.
fn prolongation(fine: &SphereMesh, subdivisions: u32) -> Prolongation {
    let coarse_count = 10 * 4usize.pow(subdivisions - 1) + 2;
    let mut weights = Vec::with_capacity(fine.len());
    let mut counts = Vec::with_capacity(fine.len());
    for cell in 0..fine.len() as u32 {
        if (cell as usize) < coarse_count {
            weights.push([(cell, 1.0), (cell, 0.0)]);
            counts.push(1);
        } else {
            let parents: Vec<u32> = fine
                .neighbors(cell)
                .iter()
                .copied()
                .filter(|&n| (n as usize) < coarse_count)
                .collect();
            assert_eq!(parents.len(), 2, "a midpoint has two parents");
            weights.push([(parents[0], 0.5), (parents[1], 0.5)]);
            counts.push(2);
        }
    }
    Prolongation {
        coarse_count,
        weights,
        counts,
    }
}

/// The pattern of `Pᵀ A P` and the contributions of the entries of `A` to it.
fn galerkin_pattern(
    fine: &Pattern,
    prolongation: &Prolongation,
) -> (Pattern, Vec<(u32, u32, f64)>) {
    let coarse_count = prolongation.coarse_count;
    let mut rows: Vec<Vec<u32>> = (0..coarse_count as u32).map(|c| vec![c]).collect();
    for fine_row in 0..fine.len() {
        let range = fine.row_start[fine_row]..fine.row_start[fine_row + 1];
        for &fine_column in &fine.columns[range] {
            let fine_column = fine_column as usize;
            for &(coarse_row, _) in prolongation.parents(fine_row) {
                for &(coarse_column, _) in prolongation.parents(fine_column) {
                    let row = &mut rows[coarse_row as usize];
                    if !row.contains(&coarse_column) {
                        row.push(coarse_column);
                    }
                }
            }
        }
    }
    let mut row_start = vec![0];
    let mut columns = Vec::new();
    for row in &rows {
        columns.extend_from_slice(row);
        row_start.push(columns.len());
    }
    let pattern = Pattern { row_start, columns };
    let mut galerkin = Vec::new();
    for fine_row in 0..fine.len() {
        let start = fine.row_start[fine_row];
        let range = start..fine.row_start[fine_row + 1];
        for (offset, &fine_column) in fine.columns[range].iter().enumerate() {
            let slot = start + offset;
            let fine_column = fine_column as usize;
            for &(coarse_row, w_row) in prolongation.parents(fine_row) {
                for &(coarse_column, w_column) in prolongation.parents(fine_column) {
                    let start = pattern.row_start[coarse_row as usize];
                    let end = pattern.row_start[coarse_row as usize + 1];
                    let position = pattern.columns[start..end]
                        .iter()
                        .position(|&c| c == coarse_column)
                        .expect("entry in the pattern");
                    galerkin.push((slot as u32, (start + position) as u32, w_row * w_column));
                }
            }
        }
    }
    (pattern, galerkin)
}

/// A matrix with the pattern of the finest level of a [`Hierarchy`], with its multigrid
/// preconditioner.
pub struct Multigrid<'a> {
    hierarchy: &'a Hierarchy,
    /// Entries of the matrix on every level, finest first.
    values: Vec<Vec<f64>>,
}

impl<'a> Multigrid<'a> {
    /// `values` are the entries of the finest matrix in the order of the hierarchy's pattern.
    pub fn new(hierarchy: &'a Hierarchy, values: Vec<f64>) -> Self {
        assert_eq!(values.len(), hierarchy.finest.columns.len());
        let mut all = vec![values];
        for level in &hierarchy.levels {
            let fine = all.last().expect("finest level");
            let mut coarse = vec![0.0; level.pattern.columns.len()];
            for &(from, to, weight) in &level.galerkin {
                coarse[to as usize] += weight * fine[from as usize];
            }
            all.push(coarse);
        }
        Self {
            hierarchy,
            values: all,
        }
    }

    fn pattern(&self, level: usize) -> &Pattern {
        if level == 0 {
            &self.hierarchy.finest
        } else {
            &self.hierarchy.levels[level - 1].pattern
        }
    }

    /// `y = A x` on the finest level.
    pub fn multiply(&self, x: &[f64], y: &mut [f64]) {
        self.pattern(0).multiply(&self.values[0], x, y);
    }

    /// Approximately solves `A z = r` with one V-cycle from zero.
    fn v_cycle(&self, level: usize, r: &[f64], z: &mut [f64]) {
        let pattern = self.pattern(level);
        let values = &self.values[level];
        z.fill(0.0);
        if level == self.hierarchy.levels.len() {
            for sweep in 0..20 {
                pattern.gauss_seidel(values, r, z, sweep % 2 == 0);
            }
            return;
        }
        let prolongation = &self.hierarchy.levels[level].prolongation;
        pattern.gauss_seidel(values, r, z, true);
        let mut residual = vec![0.0; r.len()];
        pattern.multiply(values, z, &mut residual);
        for (res, &b) in residual.iter_mut().zip(r) {
            *res = b - *res;
        }
        let mut coarse_r = vec![0.0; prolongation.coarse_count];
        prolongation.restrict(&residual, &mut coarse_r);
        let mut coarse_z = vec![0.0; prolongation.coarse_count];
        self.v_cycle(level + 1, &coarse_r, &mut coarse_z);
        prolongation.prolong_add(&coarse_z, z);
        pattern.gauss_seidel(values, r, z, false);
    }

    /// Solves `A x = b` with the right-preconditioned BiCGSTAB method (van der Vorst, 1992) to
    /// the relative tolerance `tolerance`. Returns the solution and the number of iterations.
    pub fn solve(&self, b: &[f64], tolerance: f64, max_iterations: usize) -> (Vec<f64>, usize) {
        let n = b.len();
        let mut x = vec![0.0; n];
        let b_norm = dot(b, b).sqrt();
        if b_norm == 0.0 {
            return (x, 0);
        }
        let mut r = b.to_vec();
        let r_hat = b.to_vec();
        let (mut rho, mut alpha, mut omega) = (1.0, 1.0, 1.0);
        let mut v = vec![0.0; n];
        let mut p = vec![0.0; n];
        let mut p_hat = vec![0.0; n];
        let mut s = vec![0.0; n];
        let mut s_hat = vec![0.0; n];
        let mut t = vec![0.0; n];
        for iteration in 1..=max_iterations {
            let rho_next = dot(&r_hat, &r);
            if rho_next == 0.0 {
                return (x, iteration);
            }
            let beta = (rho_next / rho) * (alpha / omega);
            rho = rho_next;
            for i in 0..n {
                p[i] = r[i] + beta * (p[i] - omega * v[i]);
            }
            self.v_cycle(0, &p, &mut p_hat);
            self.multiply(&p_hat, &mut v);
            alpha = rho / dot(&r_hat, &v);
            for i in 0..n {
                s[i] = r[i] - alpha * v[i];
            }
            if dot(&s, &s).sqrt() < tolerance * b_norm {
                for i in 0..n {
                    x[i] += alpha * p_hat[i];
                }
                return (x, iteration);
            }
            self.v_cycle(0, &s, &mut s_hat);
            self.multiply(&s_hat, &mut t);
            omega = dot(&t, &s) / dot(&t, &t);
            for i in 0..n {
                x[i] += alpha * p_hat[i] + omega * s_hat[i];
                r[i] = s[i] - omega * t[i];
            }
            if dot(&r, &r).sqrt() < tolerance * b_norm || omega == 0.0 {
                return (x, iteration);
            }
        }
        (x, max_iterations)
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multigrid_solves_diffusion_with_advection() {
        let mesh = SphereMesh::icosphere(5);
        let hierarchy = Hierarchy::new(&mesh, 2);
        // Diffusion with a weak sink and some advection towards +x.
        let n = mesh.len();
        let mut values = Vec::new();
        for cell in 0..n as u32 {
            let off: Vec<f64> = mesh
                .neighbors(cell)
                .iter()
                .map(|&j| -1.0 - (mesh.vertex(cell) - mesh.vertex(j)).x.max(0.0) * 30.0)
                .collect();
            values.push(1e-3 - off.iter().sum::<f64>());
            values.extend(off);
        }
        let b: Vec<f64> = (0..n as u32).map(|c| mesh.vertex(c).z + 0.1).collect();
        let multigrid = Multigrid::new(&hierarchy, values);
        let (x, iterations) = multigrid.solve(&b, 1e-8, 100);
        let mut ax = vec![0.0; n];
        multigrid.multiply(&x, &mut ax);
        let error = ax
            .iter()
            .zip(&b)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();
        assert!(error < 1e-6 * dot(&b, &b).sqrt(), "error {error}");
        assert!(iterations < 60, "{iterations} iterations");
    }
}
