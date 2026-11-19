//! Random initial conditions: noise-shaped continents, the plate partition and great-circle
//! distance fields.
//!
//! Shapes come from correlated noise evaluated at each cell's position on the unit sphere and
//! distances are great-circle distances, so that nothing inherits the symmetry of the mesh
//! (region growing with independent random edge costs converges to rounded hexagons; see
//! `notes/research/initial-conditions.md`).

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

use glam::DVec3;
use rand::{Rng, RngExt};

use crate::sphere::SphereMesh;

/// How the continental crust is arranged initially.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum InitialConfiguration {
    /// Several continents of different sizes plus fragments, spread over the sphere.
    #[default]
    Dispersed,
    /// All continental crust in one connected supercontinent (Pangaea-like) in a superocean.
    Supercontinent,
}

pub(super) fn random_unit_vector(rng: &mut impl Rng) -> DVec3 {
    loop {
        let v = DVec3::new(
            rng.random_range(-1.0..1.0),
            rng.random_range(-1.0..1.0),
            rng.random_range(-1.0..1.0),
        );
        let length_squared = v.length_squared();
        if length_squared > 1e-6 && length_squared <= 1.0 {
            return v / length_squared.sqrt();
        }
    }
}

/// A standard normally distributed random number (Box–Muller).
fn standard_normal(rng: &mut impl Rng) -> f64 {
    let u: f64 = rng.random_range(f64::EPSILON..1.0);
    let v: f64 = rng.random_range(0.0..1.0);
    (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
}

/// The angle (radians) between two unit vectors, accurate also for small angles.
pub(super) fn angle_between(a: DVec3, b: DVec3) -> f64 {
    a.cross(b).length().atan2(a.dot(b))
}

/// A float ordered by [`f64::total_cmp`], for priority queues.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Key(f64);

impl Eq for Key {}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// Three-dimensional gradient noise (Perlin's improved noise) with a seeded permutation table.
/// Values lie roughly in [−1, 1], with features of about unit size.
pub(super) struct GradientNoise {
    permutation: [u8; 512],
}

impl GradientNoise {
    pub(super) fn new(rng: &mut impl Rng) -> Self {
        let mut table: [u8; 256] = std::array::from_fn(|i| i as u8);
        for i in (1..table.len()).rev() {
            let j = rng.random_range(0..=i);
            table.swap(i, j);
        }
        Self {
            permutation: std::array::from_fn(|i| table[i % 256]),
        }
    }

    fn hash(&self, x: usize, y: usize, z: usize) -> u8 {
        let p = &self.permutation;
        p[usize::from(p[usize::from(p[x & 255]) + (y & 255)]) + (z & 255)]
    }

    /// Dot product of the hashed edge gradient with the offset `(x, y, z)`.
    fn gradient(hash: u8, x: f64, y: f64, z: f64) -> f64 {
        let h = hash & 15;
        let u = if h < 8 { x } else { y };
        let v = if h < 4 {
            y
        } else if h == 12 || h == 14 {
            x
        } else {
            z
        };
        (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
    }

    pub(super) fn sample(&self, p: DVec3) -> f64 {
        let floor = p.floor();
        let (x, y, z) = (
            floor.x.rem_euclid(256.0) as usize,
            floor.y.rem_euclid(256.0) as usize,
            floor.z.rem_euclid(256.0) as usize,
        );
        let f = p - floor;
        let fade = |t: f64| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
        let (u, v, w) = (fade(f.x), fade(f.y), fade(f.z));
        let lerp = |t: f64, a: f64, b: f64| a + t * (b - a);
        let corner = |dx: usize, dy: usize, dz: usize| {
            Self::gradient(
                self.hash(x + dx, y + dy, z + dz),
                f.x - dx as f64,
                f.y - dy as f64,
                f.z - dz as f64,
            )
        };
        lerp(
            w,
            lerp(
                v,
                lerp(u, corner(0, 0, 0), corner(1, 0, 0)),
                lerp(u, corner(0, 1, 0), corner(1, 1, 0)),
            ),
            lerp(
                v,
                lerp(u, corner(0, 0, 1), corner(1, 0, 1)),
                lerp(u, corner(0, 1, 1), corner(1, 1, 1)),
            ),
        )
    }

    /// Fractional Brownian motion: `octaves` octaves of noise with doubling frequency and
    /// amplitudes falling by `gain`, normalised to the amplitude of the first octave. Each
    /// octave is shifted by `offset` times its index so that the lattices do not line up.
    pub(super) fn fbm(&self, p: DVec3, octaves: u32, gain: f64, offset: DVec3) -> f64 {
        let (mut sum, mut amplitude, mut norm, mut frequency) = (0.0, 1.0, 0.0, 1.0);
        for octave in 0..octaves {
            sum += amplitude * self.sample(p * frequency + offset * f64::from(octave + 1));
            norm += amplitude;
            amplitude *= gain;
            frequency *= 2.0;
        }
        sum / norm
    }
}

/// A smooth random displacement of points on the unit sphere (domain warping): the point is
/// moved by a vector of three fBm fields and projected back onto the sphere.
pub(super) struct Warp {
    noise: GradientNoise,
    frequency: f64,
    amplitude: f64,
    octaves: u32,
}

impl Warp {
    pub(super) fn new(rng: &mut impl Rng, frequency: f64, amplitude: f64, octaves: u32) -> Self {
        Self {
            noise: GradientNoise::new(rng),
            frequency,
            amplitude,
            octaves,
        }
    }

    pub(super) fn apply(&self, p: DVec3) -> DVec3 {
        let q = p * self.frequency;
        let component = |shift: f64| {
            self.noise.fbm(
                q + DVec3::splat(shift),
                self.octaves,
                0.5,
                DVec3::splat(7.31),
            )
        };
        let displacement = DVec3::new(component(0.0), component(31.7), component(63.1));
        (p + self.amplitude * displacement).normalize()
    }
}

/// A random spherical cap that seeds a continent in the continent field.
struct Craton {
    centre: DVec3,
    radius: f64,
}

/// Depth of the bowl that holds a supercontinent together (relative to the craton heights).
const SUPERCONTINENT_BOWL: f64 = 0.3;

/// Chooses the continental cells: exactly `count` cells (ties by index) with the highest values
/// of a field made of large random cratons (one per `continent_count`, so a few large
/// continents) and domain-warped fBm detail (irregular coasts, bays, peninsulas and
/// microcontinents). For a supercontinent, the cratons lie in one cap of the sphere, a bowl
/// around the cap (see [`SUPERCONTINENT_BOWL`]) keeps the shape compact, and the
/// continent is grown from the highest cell by a priority flood over the field, so it is one
/// connected cluster.
pub(super) fn continents(
    mesh: &SphereMesh,
    count: usize,
    continent_count: usize,
    configuration: InitialConfiguration,
    rng: &mut impl Rng,
) -> Vec<bool> {
    let cell_count = mesh.len();
    let count = count.min(cell_count);
    let continent_count = continent_count.max(1);
    let pole = random_unit_vector(rng);
    let cratons: Vec<Craton> = (0..continent_count)
        .map(|_| {
            let centre = match configuration {
                InitialConfiguration::Dispersed => random_unit_vector(rng),
                InitialConfiguration::Supercontinent => loop {
                    let centre = random_unit_vector(rng);
                    if angle_between(centre, pole) < 0.75 {
                        break centre;
                    }
                },
            };
            Craton {
                centre,
                radius: rng.random_range(0.2..0.5),
            }
        })
        .collect();
    let warp = Warp::new(rng, 1.2, 0.5, 4);
    let detail = GradientNoise::new(rng);
    // A supercontinent is held together by a bowl around the pole of about its own radius.
    let supercontinent_radius = (1.0 - 2.0 * count as f64 / cell_count as f64)
        .clamp(-1.0, 1.0)
        .acos()
        .max(0.1);
    let field: Vec<f64> = (0..cell_count as u32)
        .map(|cell| {
            let p = warp.apply(mesh.vertex(cell));
            let craton = cratons
                .iter()
                .map(|craton| {
                    let x = angle_between(p, craton.centre) / craton.radius;
                    (-x * x).exp()
                })
                .fold(0.0, f64::max);
            let bowl = match configuration {
                InitialConfiguration::Dispersed => 0.0,
                InitialConfiguration::Supercontinent => {
                    -SUPERCONTINENT_BOWL * (angle_between(p, pole) / supercontinent_radius).powi(2)
                }
            };
            craton + bowl + detail.fbm(p * 1.5, 7, 0.5, DVec3::new(11.3, 5.7, 3.1))
        })
        .collect();

    let mut is_continental = vec![false; cell_count];
    match configuration {
        InitialConfiguration::Dispersed => {
            let mut order: Vec<u32> = (0..cell_count as u32).collect();
            order.sort_unstable_by(|&a, &b| {
                field[b as usize]
                    .total_cmp(&field[a as usize])
                    .then(a.cmp(&b))
            });
            for &cell in &order[..count] {
                is_continental[cell as usize] = true;
            }
        }
        InitialConfiguration::Supercontinent => {
            let Some(start) = (0..cell_count as u32).max_by(|&a, &b| {
                field[a as usize]
                    .total_cmp(&field[b as usize])
                    .then(b.cmp(&a))
            }) else {
                return is_continental;
            };
            let mut queued = vec![false; cell_count];
            let mut heap = BinaryHeap::new();
            heap.push((Key(field[start as usize]), Reverse(start)));
            queued[start as usize] = true;
            let mut assigned = 0;
            while assigned < count
                && let Some((_, Reverse(cell))) = heap.pop()
            {
                is_continental[cell as usize] = true;
                assigned += 1;
                for &neighbor in mesh.neighbors(cell) {
                    if !queued[neighbor as usize] {
                        queued[neighbor as usize] = true;
                        heap.push((Key(field[neighbor as usize]), Reverse(neighbor)));
                    }
                }
            }
        }
    }
    is_continental
}

/// Partitions the sphere into `plate_count` connected plates: noise-warped, additively
/// weighted Voronoi regions around random centres (the score of a plate at a cell is the
/// great-circle angle from the warped cell position to the plate's centre minus the plate's
/// weight). The regions are built by a priority flood from the centres in order of the score,
/// so that every plate is connected and owns its centre. The weights are adjusted until the
/// plate areas approach power-law target areas (Zipf-like ranks with random scatter: a few
/// large and several small plates, Bird 2003, Morra et al. 2013).
pub(super) fn plate_partition(
    mesh: &SphereMesh,
    plate_count: usize,
    rng: &mut impl Rng,
) -> Vec<u32> {
    let cell_count = mesh.len();
    let plate_count = plate_count.min(cell_count).max(1);
    let mut targets: Vec<f64> = (0..plate_count)
        .map(|rank| (rank as f64 + 1.0).powf(-0.9) * (0.25 * standard_normal(rng)).exp())
        .collect();
    let total: f64 = targets.iter().sum();
    targets.iter_mut().for_each(|t| *t /= total);
    // Angular radius of a spherical cap covering `fraction` of the sphere.
    let cap_radius = |fraction: f64| (1.0 - 2.0 * fraction.clamp(0.0, 1.0)).acos();
    let warp = Warp::new(rng, 1.5, 0.35, 4);
    let warped: Vec<DVec3> = (0..cell_count as u32)
        .map(|cell| warp.apply(mesh.vertex(cell)))
        .collect();
    // Random centres (in the warped frame), kept apart by about the radii of their target
    // areas where possible, so that no plate starts inside a much larger one.
    let mut centres: Vec<u32> = Vec::with_capacity(plate_count);
    let mut attempts = 0;
    while centres.len() < plate_count {
        let cell = rng.random_range(0..cell_count as u32);
        attempts += 1;
        let radius = cap_radius(targets[centres.len()]);
        let separated = centres.iter().enumerate().all(|(other, &centre)| {
            angle_between(warped[cell as usize], warped[centre as usize])
                > 0.6 * (radius + cap_radius(targets[other]))
        });
        if !centres.contains(&cell) && (separated || attempts > 100 * plate_count) {
            centres.push(cell);
        }
    }
    let centre_positions: Vec<DVec3> = centres.iter().map(|&c| warped[c as usize]).collect();
    let mut weights = vec![0.0; plate_count];
    let mut plate_of_cell = vec![u32::MAX; cell_count];
    let mut heap = BinaryHeap::new();
    for iteration in 0..=20 {
        if iteration > 0 {
            let mut areas = vec![0usize; plate_count];
            for &plate in &plate_of_cell {
                areas[plate as usize] += 1;
            }
            for plate in 0..plate_count {
                let area = areas[plate] as f64 / cell_count as f64;
                weights[plate] += 0.5 * (cap_radius(targets[plate]) - cap_radius(area));
            }
        }
        plate_of_cell.fill(u32::MAX);
        for (plate, &centre) in centres.iter().enumerate() {
            heap.push(Reverse((Key(f64::NEG_INFINITY), centre, plate as u32)));
        }
        while let Some(Reverse((_, cell, plate))) = heap.pop() {
            if plate_of_cell[cell as usize] != u32::MAX {
                continue;
            }
            plate_of_cell[cell as usize] = plate;
            for &neighbor in mesh.neighbors(cell) {
                if plate_of_cell[neighbor as usize] == u32::MAX {
                    let score =
                        angle_between(warped[neighbor as usize], centre_positions[plate as usize])
                            - weights[plate as usize];
                    heap.push(Reverse((Key(score), neighbor, plate)));
                }
            }
        }
    }
    plate_of_cell
}

/// Great-circle distance (radians) of every cell to the closest cell satisfying `is_source`,
/// or infinity if there is none. A Dijkstra search that propagates the nearest source and ranks
/// cells by their true angle to it, so the distance is (nearly exactly) geodesic rather than a
/// graph distance with the mesh's hexagonal metric.
pub(super) fn great_circle_distance_to(
    mesh: &SphereMesh,
    is_source: impl Fn(u32) -> bool,
) -> Vec<f64> {
    let mut distance = vec![f64::INFINITY; mesh.len()];
    let mut nearest = vec![u32::MAX; mesh.len()];
    let mut heap = BinaryHeap::new();
    for cell in 0..mesh.len() as u32 {
        if is_source(cell) {
            distance[cell as usize] = 0.0;
            nearest[cell as usize] = cell;
            heap.push(Reverse((Key(0.0), cell)));
        }
    }
    while let Some(Reverse((Key(d), cell))) = heap.pop() {
        if d > distance[cell as usize] {
            continue;
        }
        let source = nearest[cell as usize];
        let source_position = mesh.vertex(source);
        for &neighbor in mesh.neighbors(cell) {
            let candidate = angle_between(mesh.vertex(neighbor), source_position);
            if candidate < distance[neighbor as usize] {
                distance[neighbor as usize] = candidate;
                nearest[neighbor as usize] = source;
                heap.push(Reverse((Key(candidate), neighbor)));
            }
        }
    }
    distance
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand::rngs::ChaCha8Rng;

    use super::*;

    fn continental_cells(mesh: &SphereMesh) -> usize {
        (0.4 * mesh.len() as f64).round() as usize
    }

    fn generate(mesh: &SphereMesh, seed: u64, configuration: InitialConfiguration) -> Vec<bool> {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        continents(mesh, continental_cells(mesh), 7, configuration, &mut rng)
    }

    /// The connected components of the cells with `member` set, largest first.
    fn components(mesh: &SphereMesh, member: impl Fn(u32) -> bool) -> Vec<Vec<u32>> {
        let mut seen = vec![false; mesh.len()];
        let mut parts = Vec::new();
        for start in 0..mesh.len() as u32 {
            if seen[start as usize] || !member(start) {
                continue;
            }
            seen[start as usize] = true;
            let mut stack = vec![start];
            let mut cells = Vec::new();
            while let Some(cell) = stack.pop() {
                cells.push(cell);
                for &neighbor in mesh.neighbors(cell) {
                    if !seen[neighbor as usize] && member(neighbor) {
                        seen[neighbor as usize] = true;
                        stack.push(neighbor);
                    }
                }
            }
            parts.push(cells);
        }
        parts.sort_unstable_by_key(|part| std::cmp::Reverse(part.len()));
        parts
    }

    fn sizes_of(parts: &[Vec<u32>]) -> Vec<usize> {
        parts.iter().map(Vec::len).collect()
    }

    /// Solidity of a region (its area over the area of its convex hull), measured in the
    /// gnomonic projection around its centroid (great circles are straight lines there), or
    /// `None` if the region reaches too far from its centroid.
    fn solidity(mesh: &SphereMesh, cells: &[u32]) -> Option<f64> {
        let centre = cells
            .iter()
            .map(|&cell| mesh.vertex(cell))
            .sum::<DVec3>()
            .normalize();
        let (e1, e2) = centre.any_orthonormal_pair();
        let project = |p: DVec3| {
            let height = p.dot(centre);
            (height > 0.3).then(|| (p.dot(e1) / height, p.dot(e2) / height))
        };
        let mut points: Vec<(f64, f64)> = cells
            .iter()
            .map(|&cell| project(mesh.vertex(cell)))
            .collect::<Option<_>>()?;
        // Convex hull (Andrew's monotone chain), counter-clockwise.
        points.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let cross = |o: (f64, f64), a: (f64, f64), b: (f64, f64)| {
            (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
        };
        let mut hull: Vec<(f64, f64)> = Vec::new();
        for pass in 0..2 {
            let start = hull.len();
            let ordered: Vec<(f64, f64)> = if pass == 0 {
                points.clone()
            } else {
                points.iter().rev().copied().collect()
            };
            for p in ordered {
                while hull.len() >= start + 2
                    && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0
                {
                    hull.pop();
                }
                hull.push(p);
            }
            hull.pop();
        }
        let inside = (0..mesh.len() as u32)
            .filter_map(|cell| project(mesh.vertex(cell)))
            .filter(|&p| {
                (0..hull.len()).all(|i| cross(hull[i], hull[(i + 1) % hull.len()], p) >= -1e-12)
            })
            .count();
        Some(cells.len() as f64 / inside.max(1) as f64)
    }

    /// Area-weighted mean solidity of the continents larger than 2% of the surface.
    fn mean_solidity(mesh: &SphereMesh, is_continental: &[bool]) -> f64 {
        let (mut sum, mut weight) = (0.0, 0.0);
        for part in components(mesh, |cell| is_continental[cell as usize]) {
            if part.len() * 50 < mesh.len() {
                continue;
            }
            if let Some(solidity) = solidity(mesh, &part) {
                sum += solidity * part.len() as f64;
                weight += part.len() as f64;
            }
        }
        sum / weight
    }

    #[test]
    fn the_continental_fraction_is_exact() {
        let mesh = SphereMesh::icosphere(4);
        for configuration in [
            InitialConfiguration::Dispersed,
            InitialConfiguration::Supercontinent,
        ] {
            for seed in 0..4 {
                let is_continental = generate(&mesh, seed, configuration);
                let count = is_continental.iter().filter(|&&c| c).count();
                assert_eq!(
                    count,
                    continental_cells(&mesh),
                    "{configuration:?} seed {seed}"
                );
            }
        }
    }

    #[test]
    fn a_supercontinent_is_connected_and_dispersed_continents_are_not() {
        let mesh = SphereMesh::icosphere(5);
        for seed in 0..4 {
            let is_continental = generate(&mesh, seed, InitialConfiguration::Supercontinent);
            let sizes = sizes_of(&components(&mesh, |cell| is_continental[cell as usize]));
            assert_eq!(sizes.len(), 1, "seed {seed}: {sizes:?}");

            let is_continental = generate(&mesh, seed, InitialConfiguration::Dispersed);
            let sizes = sizes_of(&components(&mesh, |cell| is_continental[cell as usize]));
            let total: usize = sizes.iter().sum();
            // A few large continents plus smaller fragments.
            assert!(sizes.len() >= 3, "seed {seed}: {sizes:?}");
            assert!(sizes[0] < total * 9 / 10, "seed {seed}: {sizes:?}");
        }
    }

    #[test]
    fn continents_are_not_convex() {
        // Regions grown with independent random edge costs converge to convex, rounded
        // hexagons: the former generator had solidities of 0.79–0.92 (mean ~0.86) per seed.
        // Noise-shaped continents have bays and peninsulas (0.57–0.86, mean ~0.69).
        let mesh = SphereMesh::icosphere(5);
        let solidities: Vec<f64> = (0..8)
            .map(|seed| {
                let is_continental = generate(&mesh, seed, InitialConfiguration::Dispersed);
                mean_solidity(&mesh, &is_continental)
            })
            .filter(|solidity| !solidity.is_nan())
            .collect();
        let mean = solidities.iter().sum::<f64>() / solidities.len() as f64;
        assert!(solidities.len() >= 5 && mean < 0.75, "{solidities:?}");
    }

    #[test]
    fn plates_are_connected_and_vary_in_size() {
        let mesh = SphereMesh::icosphere(5);
        let plate_count = 12;
        let mut ratios = Vec::new();
        for seed in 0..6 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let plate_of_cell = plate_partition(&mesh, plate_count, &mut rng);
            let mut sizes = Vec::new();
            for plate in 0..plate_count as u32 {
                let parts = components(&mesh, |cell| plate_of_cell[cell as usize] == plate);
                assert_eq!(parts.len(), 1, "seed {seed} plate {plate}");
                sizes.push(parts[0].len());
            }
            sizes.sort_unstable();
            let largest = *sizes.last().unwrap() as f64 / mesh.len() as f64;
            let smallest = sizes[0] as f64 / mesh.len() as f64;
            assert!(largest > 0.12, "seed {seed}: {sizes:?}");
            ratios.push(largest / smallest);
        }
        // A few large and several small plates (Earth: Pacific ~20%, the smallest of the 12
        // largest plates ~1%).
        let mean_ratio = ratios.iter().sum::<f64>() / ratios.len() as f64;
        assert!(mean_ratio > 6.0, "{ratios:?}");
    }

    #[test]
    fn initial_conditions_are_deterministic_per_seed() {
        use crate::tectonics::{Simulation, TectonicParams};
        let state = |seed: u64, initial: InitialConfiguration| {
            let params = TectonicParams {
                initial,
                ..TectonicParams::default()
            };
            let simulation = Simulation::new(SphereMesh::icosphere(4), 8, params, seed);
            let owners: Vec<_> = simulation.owners.clone();
            let crust: Vec<String> = simulation
                .plates
                .iter()
                .map(|plate| format!("{:?}", plate.crust))
                .collect();
            (owners, crust)
        };
        for initial in [
            InitialConfiguration::Dispersed,
            InitialConfiguration::Supercontinent,
        ] {
            assert_eq!(state(3, initial), state(3, initial), "{initial:?}");
            assert_ne!(state(3, initial), state(4, initial), "{initial:?}");
        }
    }

    #[test]
    fn great_circle_distances_are_isotropic() {
        let mesh = SphereMesh::icosphere(5);
        let source = mesh.nearest(DVec3::new(0.3, 0.5, 0.8).normalize());
        let distance = great_circle_distance_to(&mesh, |cell| cell == source);
        let max_error = (0..mesh.len() as u32)
            .map(|cell| {
                let exact = angle_between(mesh.vertex(cell), mesh.vertex(source));
                (distance[cell as usize] - exact).abs()
            })
            .fold(0.0, f64::max);
        assert!(max_error < 1e-9, "{max_error}");
    }
}
