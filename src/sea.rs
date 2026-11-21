//! Which water below sea level is the ocean (#58).
//!
//! The ocean is the water connected to the world ocean: the cells below sea level that are
//! connected through cells below sea level (neighbours sharing a cell edge) to the deepest
//! cell of the largest body of water. Closed depressions below sea level that the ocean does
//! not reach are land: dry basins or lakes with their own water balance (see
//! [`crate::erosion`]), like the Caspian Sea, the Dead Sea or the Qattara Depression.
//!
//! On the hexagonal-dual icosphere every connection goes through a shared cell edge, so a
//! strait is resolved only if the cells along it lie below sea level on average, i.e. if it is
//! about a cell (~110 km) wide. Narrower straits (Bosporus, Gibraltar) are sub-grid: a sea
//! behind such a strait is a lake here unless a cell of the strait lies below sea level.
//!
//! The sea level solve counts only the ocean: every cell has a *pass level*, the lowest sea
//! level at which the ocean reaches it (the lowest possible highest point of a path from the
//! ocean's seed, found by a priority flood), and joins the ocean when the sea level rises above
//! it (see [`sea_level`]).

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use crate::isostasy::{self, SeaLevel};
use crate::sphere::SphereMesh;

/// Cells whose surface lies within this distance (m) of sea level are sills on which the sea
/// level stands: they connect the ocean to the basin behind them (whatever the rounding of the
/// elevations relative to the sea level), but are not ocean themselves.
///
/// The sea level stands exactly on a pass when the ocean volume does not suffice to fill the
/// basin behind it (see [`sea_level`]): the sea spills over
/// the sill into the basin, which holds the water the solve could not place (its deficit), and
/// the basin is an arm of the sea fed over its sill, like the Mediterranean, not a lake.
pub const SILL_TOLERANCE: f64 = 1e-6;

/// The seed of the ocean: the deepest cell of the largest connected body (by cell count) of the
/// cells with `level < reference`, or the lowest cell of all if there is none.
fn seed(mesh: &SphereMesh, level: &[f64], reference: f64) -> usize {
    let cell_count = mesh.len();
    let mut body = vec![u32::MAX; cell_count];
    let mut queue = VecDeque::new();
    // (size, deepest cell) of the largest body so far.
    let mut best: Option<(usize, usize)> = None;
    for start in 0..cell_count {
        if body[start] != u32::MAX || level[start] >= reference {
            continue;
        }
        body[start] = start as u32;
        queue.push_back(start);
        let (mut size, mut deepest) = (0, start);
        while let Some(cell) = queue.pop_front() {
            size += 1;
            if level[cell] < level[deepest] {
                deepest = cell;
            }
            for &neighbor in mesh.neighbors(cell as u32) {
                let n = neighbor as usize;
                if body[n] == u32::MAX && level[n] < reference {
                    body[n] = start as u32;
                    queue.push_back(n);
                }
            }
        }
        if best.is_none_or(|(largest, _)| size > largest) {
            best = Some((size, deepest));
        }
    }
    best.map_or_else(
        || {
            (0..cell_count)
                .min_by(|&a, &b| level[a].total_cmp(&level[b]))
                .unwrap_or(0)
        },
        |(_, deepest)| deepest,
    )
}

/// The pass level of every cell: the lowest possible highest `level` on a path of neighbouring
/// cells from `seed` (Barnes et al., 2014: priority flood). [`sea_level`] floods the same way.
#[cfg(test)]
fn pass_levels(mesh: &SphereMesh, level: &[f64], seed: usize) -> Vec<f64> {
    let cell_count = mesh.len();
    let mut pass = vec![f64::INFINITY; cell_count];
    let mut reached = vec![false; cell_count];
    let mut heap = BinaryHeap::new();
    if cell_count == 0 {
        return pass;
    }
    // Cells leave the queue in the order of their pass levels, so the first path to reach a
    // cell is the lowest one.
    pass[seed] = level[seed];
    reached[seed] = true;
    heap.push(Reverse((Ordered(level[seed]), seed as u32)));
    while let Some(Reverse((Ordered(at), cell))) = heap.pop() {
        for &neighbor in mesh.neighbors(cell) {
            let n = neighbor as usize;
            if !reached[n] {
                reached[n] = true;
                pass[n] = level[n].max(at);
                heap.push(Reverse((Ordered(pass[n]), neighbor)));
            }
        }
    }
    pass
}

/// The ocean among cells with the given surface elevations relative to sea level (m): the cells
/// below sea level connected to the largest body of water below sea level, also over sills on
/// which the sea level stands (see [`SILL_TOLERANCE`]).
pub fn ocean_mask(mesh: &SphereMesh, elevation: &[f64]) -> Vec<bool> {
    let seed = seed(mesh, elevation, -SILL_TOLERANCE);
    ocean_from(mesh, elevation, std::iter::once(seed))
}

/// The cells below sea level (`elevation < 0`, less [`SILL_TOLERANCE`]) connected to the
/// `seeds` through cells below sea level or sills at sea level.
pub fn ocean_from(
    mesh: &SphereMesh,
    elevation: &[f64],
    seeds: impl IntoIterator<Item = usize>,
) -> Vec<bool> {
    let passable = |cell: usize| elevation[cell] < SILL_TOLERANCE;
    let mut reached = vec![false; mesh.len()];
    let mut queue = VecDeque::new();
    for seed in seeds {
        if passable(seed) && !reached[seed] {
            reached[seed] = true;
            queue.push_back(seed);
        }
    }
    while let Some(cell) = queue.pop_front() {
        for &neighbor in mesh.neighbors(cell as u32) {
            let n = neighbor as usize;
            if !reached[n] && passable(n) {
                reached[n] = true;
                queue.push_back(n);
            }
        }
    }
    reached
        .iter()
        .zip(elevation)
        .map(|(&reached, &e)| reached && e < -SILL_TOLERANCE)
        .collect()
}

/// The sea level (an offset of dry elevations, m) at which the ocean holds `volume` (m³), for
/// cells of equal `cell_area` (m²) with the given dry elevations and the pore water `pores`
/// (m) of their sediment. The ocean is seeded in the largest body of water below the sea level
/// `reference` (normally the current one), and a cell joins it once the sea level is above its
/// pass level.
///
/// The water depth over an ocean cell with dry elevation `d` below sea level `S` is
/// `(S − d) · ρm / (ρm − ρw)` (the water load depresses the floor), so the volume is piecewise
/// linear and increasing in `S`, with a jump where a cell joins below its pass (a basin behind
/// a sill joins at once, filled to the sill) or its pores fill; it is solved exactly. If the
/// volume falls into such a jump, the sea level is that pass and the returned deficit is the
/// part of the volume the cells below the pass do not hold: the sea spills over the sill into
/// the basin, which holds the deficit (see [`SILL_TOLERANCE`]).
pub fn sea_level(
    mesh: &SphereMesh,
    dry: &[f64],
    pores: &[f64],
    cell_area: f64,
    volume: f64,
    reference: f64,
) -> SeaLevel {
    if dry.is_empty() {
        return SeaLevel {
            level: 0.0,
            deficit: 0.0,
        };
    }
    // The priority flood of `pass_levels` hands out the cells in the order of their passes,
    // which is the order in which `isostasy::sea_level_for_volume_with_passes` takes them: solve
    // while flooding and stop at the level, without passes for the cells above it.
    let seed = seed(mesh, dry, reference);
    let target = volume / (cell_area * isostasy::WATER_LOADING);
    if target <= 0.0 {
        return SeaLevel {
            level: dry[seed],
            deficit: 0.0,
        };
    }
    let mut reached = vec![false; dry.len()];
    let mut heap = BinaryHeap::new();
    reached[seed] = true;
    heap.push(Reverse((Ordered(dry[seed]), seed as u32)));
    let (mut count, mut prefix, mut pore_prefix) = (0usize, 0.0, 0.0);
    // The cells with a lower pass than the current one: their count and sums.
    let mut lower = (0, 0.0, 0.0);
    let mut previous = f64::NEG_INFINITY;
    while let Some(Reverse((Ordered(pass), cell))) = heap.pop() {
        for &neighbor in mesh.neighbors(cell) {
            let n = neighbor as usize;
            if !reached[n] {
                reached[n] = true;
                heap.push(Reverse((Ordered(dry[n].max(pass)), neighbor)));
            }
        }
        if previous < pass {
            lower = (count, prefix, pore_prefix);
            previous = pass;
        }
        count += 1;
        prefix += dry[cell as usize];
        pore_prefix += pores[cell as usize] / isostasy::WATER_LOADING;
        // With these `count` cells in the ocean: count · S − Σ dᵢ + Σ pᵢ / W = target.
        let level = (target + prefix - pore_prefix) / count as f64;
        if level < pass {
            // The volume runs out while this cell joins (its basin below the pass, or its
            // pores): the cells with a lower pass hold their share, the rest is the deficit.
            let (below, dry_sum, pore_sum) = lower;
            let held = below as f64 * pass - dry_sum + pore_sum;
            return SeaLevel {
                level: pass,
                deficit: ((target - held) * cell_area * isostasy::WATER_LOADING).max(0.0),
            };
        }
        match heap.peek() {
            Some(Reverse((Ordered(next), _))) if level > *next => {}
            _ => {
                return SeaLevel {
                    level,
                    deficit: 0.0,
                };
            }
        }
    }
    unreachable!("the flood returns at the last cell")
}

/// The sea level (as an offset of dry elevations, m) at which the ocean holds `volume` (m³),
/// relative to the sea level that the surface `elevations` (m) refer to, for cells of equal
/// `cell_area` (m²) carrying the given rock-equivalent `sediment` (m), whose pores hold water.
pub fn relative_sea_level(
    mesh: &SphereMesh,
    elevations: &[f64],
    sediment: &[f64],
    cell_area: f64,
    volume: f64,
) -> f64 {
    let dry: Vec<f64> = elevations.iter().map(|&e| isostasy::to_dry(e)).collect();
    let pores: Vec<f64> = sediment
        .iter()
        .map(|&s| isostasy::sediment_pores(s))
        .collect();
    sea_level(mesh, &dry, &pores, cell_area, volume, 0.0).level
}

/// A totally ordered `f64` for the priority queue.
#[derive(Clone, Copy)]
struct Ordered(f64);

impl PartialEq for Ordered {
    fn eq(&self, other: &Self) -> bool {
        self.0.total_cmp(&other.0).is_eq()
    }
}

impl Eq for Ordered {}

impl PartialOrd for Ordered {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ordered {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isostasy::{WATER_LOADING, to_surface};

    /// A mesh with a deep ocean in the southern hemisphere, a continent in the north, and a
    /// closed depression on the continent around the north pole (dry elevations).
    fn world(depression: f64) -> (SphereMesh, Vec<f64>) {
        let mesh = SphereMesh::icosphere(4);
        let dry = (0..mesh.len() as u32)
            .map(|cell| {
                let z = mesh.vertex(cell).z;
                if z < 0.0 {
                    -2000.0
                } else if z > 0.95 {
                    depression
                } else {
                    500.0
                }
            })
            .collect();
        (mesh, dry)
    }

    fn held(dry: &[f64], ocean: &[bool], level: f64, area: f64) -> f64 {
        dry.iter()
            .zip(ocean)
            .filter(|&(_, &o)| o)
            .map(|(&d, _)| -to_surface(d - level).min(0.0))
            .sum::<f64>()
            * area
    }

    #[test]
    fn a_closed_depression_below_sea_level_is_not_ocean() {
        let (mesh, dry) = world(-1500.0);
        let area = 1e10;
        let pores = vec![0.0; dry.len()];
        let southern = dry.iter().filter(|&&d| d == -2000.0).count() as f64;
        // Enough water for 1000 m over the southern hemisphere.
        let volume = southern * 1000.0 * WATER_LOADING * area;
        let solved = sea_level(&mesh, &dry, &pores, area, volume, 0.0);
        assert!((solved.level + 1000.0).abs() < 1e-6, "{solved:?}");
        assert_eq!(solved.deficit, 0.0);
        // At the same volume, counting all water below sea level would also fill the
        // depression and lower the sea level: it is not counted.
        let all = isostasy::sea_level_for_volume_with_passes(&dry, &dry, &pores, area, volume);
        assert!(all.level < solved.level);
        // The mask agrees: the depression is below sea level but not ocean.
        let elevation: Vec<f64> = dry.iter().map(|&d| to_surface(d - solved.level)).collect();
        let ocean = ocean_mask(&mesh, &elevation);
        for cell in 0..dry.len() {
            assert_eq!(ocean[cell], dry[cell] == -2000.0, "{cell}");
        }
        assert!(elevation.iter().zip(&ocean).any(|(&e, &o)| e < 0.0 && !o));
        assert!((held(&dry, &ocean, solved.level, area) / volume - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_connected_depression_is_ocean() {
        let (mesh, mut dry) = world(-300.0);
        // A channel below sea level from the depression to the ocean along a meridian.
        for (cell, d) in dry.iter_mut().enumerate() {
            let v = mesh.vertex(cell as u32);
            if v.z >= 0.0 && v.y.abs() < 0.08 && v.x > 0.0 {
                *d = -300.0;
            }
        }
        let area = 1e10;
        let pores = vec![0.0; dry.len()];
        let volume = 1e17;
        let solved = sea_level(&mesh, &dry, &pores, area, volume, 0.0);
        assert!(solved.level > -300.0 && solved.deficit == 0.0, "{solved:?}");
        let elevation: Vec<f64> = dry.iter().map(|&d| to_surface(d - solved.level)).collect();
        let ocean = ocean_mask(&mesh, &elevation);
        for cell in 0..dry.len() {
            assert_eq!(ocean[cell], elevation[cell] < 0.0, "{cell}");
        }
        assert!((held(&dry, &ocean, solved.level, area) / volume - 1.0).abs() < 1e-9);
        // With all water counted it is the same: everything below sea level is connected.
        let all = isostasy::sea_level_for_volume_with_passes(&dry, &dry, &pores, area, volume);
        assert!((all.level - solved.level).abs() < 1e-6);
    }

    #[test]
    fn the_sea_stands_at_a_sill_until_the_basin_behind_it_is_full() {
        // The depression's rim is at 500 m. Water that just reaches the rim floods the basin
        // behind it at once: in between, the sea stands at the rim and the deficit is the
        // water that would spill over.
        let (mesh, dry) = world(-300.0);
        let area = 1e10;
        let pores = vec![0.0; dry.len()];
        let at_rim = |extra: f64| {
            let southern: f64 = dry
                .iter()
                .filter(|&&d| d < 500.0)
                .map(|&d| if d < -1000.0 { 2500.0 } else { 0.0 })
                .sum();
            (southern + extra) * WATER_LOADING * area
        };
        let basin = dry.iter().filter(|&&d| d == -300.0).count() as f64 * 800.0;
        let solved = sea_level(&mesh, &dry, &pores, area, at_rim(0.5 * basin), 0.0);
        assert_eq!(solved.level, 500.0);
        let expected = 0.5 * basin * WATER_LOADING * area;
        assert!((solved.deficit / expected - 1.0).abs() < 1e-6, "{solved:?}");
        // The sea spills over the rim: the basin is an arm of the sea holding the deficit, the
        // rim (at sea level) is not ocean.
        let elevation: Vec<f64> = dry.iter().map(|&d| to_surface(d - solved.level)).collect();
        let ocean = ocean_mask(&mesh, &elevation);
        for (cell, &d) in dry.iter().enumerate() {
            assert_eq!(ocean[cell], d < 500.0, "{cell}");
        }
        // Once the basin is full, it is ocean.
        let solved = sea_level(&mesh, &dry, &pores, area, at_rim(1.5 * basin), 0.0);
        assert!(solved.level > 500.0 && solved.deficit == 0.0, "{solved:?}");
    }

    #[test]
    fn the_flooding_solve_equals_the_sorted_one() {
        let mesh = SphereMesh::icosphere(4);
        // Rough topography with many basins and passes, and pores.
        let dry: Vec<f64> = (0..mesh.len() as u32)
            .map(|cell| {
                let v = mesh.vertex(cell);
                -1500.0
                    + 2500.0 * v.z
                    + 400.0 * crate::math::sin(17.0 * v.x) * crate::math::cos(13.0 * v.y)
            })
            .collect();
        let pores: Vec<f64> = (0..dry.len()).map(|c| ((c * 37) % 11) as f64).collect();
        for volume in [0.0, 1e14, 1e16, 3e16, 1e17, 1e19] {
            let solved = sea_level(&mesh, &dry, &pores, 1e10, volume, 0.0);
            let seed = seed(&mesh, &dry, 0.0);
            let passes = pass_levels(&mesh, &dry, seed);
            let sorted =
                isostasy::sea_level_for_volume_with_passes(&dry, &passes, &pores, 1e10, volume);
            assert!(
                (solved.level - sorted.level).abs() < 1e-9,
                "{solved:?} {sorted:?}"
            );
            assert!(
                (solved.deficit - sorted.deficit).abs() <= 1e-9 * volume.max(1.0),
                "{solved:?} {sorted:?}"
            );
        }
    }

    #[test]
    fn an_isolated_pit_deeper_than_the_ocean_is_not_the_seed() {
        let (mesh, mut dry) = world(500.0);
        let pit = (0..mesh.len())
            .max_by(|&a, &b| mesh.vertex(a as u32).z.total_cmp(&mesh.vertex(b as u32).z))
            .unwrap();
        dry[pit] = -8000.0;
        let elevation: Vec<f64> = dry.iter().map(|&d| to_surface(d + 1000.0)).collect();
        let ocean = ocean_mask(&mesh, &elevation);
        assert!(!ocean[pit]);
        assert_eq!(
            ocean.iter().filter(|&&o| o).count(),
            dry.iter().filter(|&&d| d == -2000.0).count()
        );
        let pores = vec![0.0; dry.len()];
        let volume = 1e16;
        let solved = sea_level(&mesh, &dry, &pores, 1e10, volume, -1000.0);
        assert!(solved.level < -1000.0, "{solved:?}");
        let elevation: Vec<f64> = dry.iter().map(|&d| to_surface(d - solved.level)).collect();
        let ocean = ocean_mask(&mesh, &elevation);
        assert!(!ocean[pit]);
        assert!((held(&dry, &ocean, solved.level, 1e10) / volume - 1.0).abs() < 1e-9);
    }
}
