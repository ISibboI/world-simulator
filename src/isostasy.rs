//! Airy isostasy: columns of crust and sediment float on the mantle.
//!
//! Adding a layer of thickness `h` and density `ρ` to a column raises its surface by
//! `h (ρ_m − ρ) / ρ_m` above sea level. Below sea level the column is additionally loaded by the
//! water that fills the space above it, which amplifies vertical motions by
//! `ρ_m / (ρ_m − ρ_w)`.
//!
//! To handle columns that cross sea level, elevations are converted to *dry* elevations (the
//! elevation the column would have without water loading), where contributions simply add up.

use crate::math;

/// Density of the upper mantle in kg/m³.
pub const MANTLE_DENSITY: f64 = 3300.0;
/// Density of sea water in kg/m³.
pub const WATER_DENSITY: f64 = 1030.0;
/// Bulk density of water-saturated sediment with [`SEDIMENT_POROSITY`] in kg/m³.
#[cfg(test)]
pub const SEDIMENT_DENSITY: f64 = 2300.0;
/// Reference rock mass per volume of sediment in kg/m³: 24% pores in 2700 kg/m³ grains.
/// Sediment amounts are stored as rock-equivalent thicknesses at this density (mass / 2050), so
/// mass conservation uses it; the actual (bulk) thickness follows from compaction (see
/// [`sediment_bulk_thickness`]).
pub const SEDIMENT_ROCK_DENSITY: f64 = 2050.0;
/// Density of sediment grains in kg/m³.
pub const SEDIMENT_GRAIN_DENSITY: f64 = 2700.0;
/// Pore fraction of the reference sediment (see [`SEDIMENT_ROCK_DENSITY`]).
pub const SEDIMENT_POROSITY: f64 = 1.0 - SEDIMENT_ROCK_DENSITY / SEDIMENT_GRAIN_DENSITY;
/// Porosity of sediment at the surface and its compaction length (m) after Athy's law
/// `φ(z) = φ0 exp(−z/λ)` for a shale–sand mix (Sclater & Christie, 1980: φ0 0.49–0.63,
/// λ 1.4–3.7 km).
pub const SURFACE_POROSITY: f64 = 0.6;
pub const COMPACTION_LENGTH: f64 = 2000.0;
/// Density of oceanic crust (basalt, gabbro) in kg/m³.
pub const OCEANIC_DENSITY: f64 = 2900.0;
/// Mean density of continental crust in kg/m³ (~2830 after Christensen & Mooney, 1995).
pub const CONTINENTAL_DENSITY: f64 = 2800.0;
/// Density of new arc crust (andesitic) in kg/m³.
pub const ARC_DENSITY: f64 = 2800.0;

/// Amplification of vertical motion below sea level by the water load.
pub const WATER_LOADING: f64 = MANTLE_DENSITY / (MANTLE_DENSITY - WATER_DENSITY);

/// Rise of the dry surface per metre of added material of the given density.
pub fn dry_response(density: f64) -> f64 {
    (MANTLE_DENSITY - density) / MANTLE_DENSITY
}

/// Converts a dry elevation into the actual surface elevation relative to sea level.
pub fn to_surface(dry_elevation: f64) -> f64 {
    if dry_elevation < 0.0 {
        dry_elevation * WATER_LOADING
    } else {
        dry_elevation
    }
}

/// Converts a surface elevation relative to sea level into a dry elevation.
pub fn to_dry(surface_elevation: f64) -> f64 {
    if surface_elevation < 0.0 {
        surface_elevation / WATER_LOADING
    } else {
        surface_elevation
    }
}

/// The surface elevation after adding a layer of `thickness` (negative to remove material) and
/// `density` to a column whose surface is at `elevation`.
pub fn add_layer(elevation: f64, thickness: f64, density: f64) -> f64 {
    to_surface(to_dry(elevation) + thickness * dry_response(density))
}

/// The thickness of material of `density` needed to raise a surface from `from` to `to`.
#[cfg(test)]
pub fn layer_for_rise(from: f64, to: f64, density: f64) -> f64 {
    (to_dry(to) - to_dry(from)) / dry_response(density)
}

/// Thickness of the grains in a sediment column of rock-equivalent thickness `sediment`.
fn sediment_grains(sediment: f64) -> f64 {
    sediment.max(0.0) * (1.0 - SEDIMENT_POROSITY)
}

/// Grain thickness of a compacted column of bulk thickness `bulk` (Athy's law integrated).
fn grains_in_bulk(bulk: f64) -> f64 {
    bulk - SURFACE_POROSITY * COMPACTION_LENGTH * (1.0 - math::exp(-bulk / COMPACTION_LENGTH))
}

/// Exact bulk thickness of a compacted column with `grains` m of grains (Newton).
fn bulk_for_grains(grains: f64) -> f64 {
    if grains <= 0.0 {
        return 0.0;
    }
    // Newton from an upper bound; the grain thickness is convex in the bulk thickness, so the
    // iteration decreases monotonically.
    let mut bulk = grains + SURFACE_POROSITY * COMPACTION_LENGTH;
    for _ in 0..100 {
        let slope = 1.0 - SURFACE_POROSITY * math::exp(-bulk / COMPACTION_LENGTH);
        let step = (grains_in_bulk(bulk) - grains) / slope;
        bulk -= step;
        if step < 1e-12 * (1.0 + bulk) {
            break;
        }
    }
    bulk
}

/// Dry elevation (see [`to_dry`]) contributed by a water-saturated sediment column of bulk
/// thickness `bulk`.
fn sediment_dry_elevation_of_bulk(bulk: f64) -> f64 {
    let grains = grains_in_bulk(bulk);
    bulk - (grains * SEDIMENT_GRAIN_DENSITY + (bulk - grains) * WATER_DENSITY) / MANTLE_DENSITY
}

/// Rock-equivalent sediment thickness between entries of [`compaction_table`] (m).
const TABLE_STEP: f64 = 5.0;
/// Number of entries of [`compaction_table`]; beyond it, columns are extrapolated linearly
/// (the porosity at 60 km is negligible).
const TABLE_LENGTH: usize = 12_001;

/// Bulk thickness and dry elevation of compacted sediment columns at rock-equivalent
/// thicknesses `0, TABLE_STEP, 2 TABLE_STEP, …`. All compaction functions interpolate this
/// table linearly, so that they are fast and exact inverses of each other.
fn compaction_table() -> &'static [(f64, f64)] {
    static TABLE: std::sync::OnceLock<Vec<(f64, f64)>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        (0..TABLE_LENGTH)
            .map(|i| {
                let bulk = bulk_for_grains(sediment_grains(i as f64 * TABLE_STEP));
                (bulk, sediment_dry_elevation_of_bulk(bulk))
            })
            .collect()
    })
}

/// Linear interpolation of column `field` of [`compaction_table`] at rock-equivalent thickness
/// `sediment`.
fn interpolate(sediment: f64, field: fn(&(f64, f64)) -> f64) -> f64 {
    let table = compaction_table();
    let position = (sediment.max(0.0) / TABLE_STEP).min((TABLE_LENGTH - 1) as f64);
    let index = (position as usize).min(TABLE_LENGTH - 2);
    let (a, b) = (field(&table[index]), field(&table[index + 1]));
    let fraction = sediment.max(0.0) / TABLE_STEP - index as f64;
    a + (b - a) * fraction
}

/// The rock-equivalent thickness at which column `field` of [`compaction_table`] (increasing)
/// reaches `value`: the inverse of [`interpolate`].
fn invert(value: f64, field: fn(&(f64, f64)) -> f64) -> f64 {
    let table = compaction_table();
    if value <= 0.0 {
        return 0.0;
    }
    let last = TABLE_LENGTH - 2;
    let index = if value >= field(&table[last]) {
        last
    } else {
        table.partition_point(|entry| field(entry) <= value) - 1
    };
    let (a, b) = (field(&table[index]), field(&table[index + 1]));
    (index as f64 + (value - a) / (b - a)) * TABLE_STEP
}

/// Bulk thickness (m) of a compacted sediment column of rock-equivalent thickness `sediment`
/// (see [`SEDIMENT_ROCK_DENSITY`]): pores make up [`SURFACE_POROSITY`] at its top and close
/// with depth. Porosity is a function of the present burial, so removing sediment from the top
/// decompacts the rest (no memory of the maximum burial).
pub fn sediment_bulk_thickness(sediment: f64) -> f64 {
    interpolate(sediment, |entry| entry.0)
}

/// Rock-equivalent thickness of a compacted sediment column of bulk thickness `bulk`: the
/// inverse of [`sediment_bulk_thickness`].
pub fn sediment_for_bulk_thickness(bulk: f64) -> f64 {
    invert(bulk, |entry| entry.0)
}

/// Pore volume per area (m) of a sediment column of rock-equivalent thickness `sediment`.
pub fn sediment_pores(sediment: f64) -> f64 {
    sediment_bulk_thickness(sediment) - sediment_grains(sediment)
}

/// Dry elevation (see [`to_dry`]) contributed by a water-saturated, compacted sediment column
/// of rock-equivalent thickness `sediment`.
pub fn sediment_dry_elevation(sediment: f64) -> f64 {
    interpolate(sediment, |entry| entry.1)
}

/// The rock-equivalent sediment thickness whose column contributes the dry elevation `dry`:
/// the inverse of [`sediment_dry_elevation`].
pub fn sediment_for_dry_elevation(dry: f64) -> f64 {
    invert(dry, |entry| entry.1)
}

/// The surface elevation after changing the rock-equivalent sediment thickness of a column
/// whose surface is at `elevation` from `from` to `to`.
pub fn change_sediment(elevation: f64, from: f64, to: f64) -> f64 {
    to_surface(to_dry(elevation) + sediment_dry_elevation(to) - sediment_dry_elevation(from))
}

/// The rock-equivalent sediment thickness to add to a column with `sediment` whose surface is
/// at `from` to raise it to `to` (negative to lower it).
pub fn sediment_for_rise(from: f64, to: f64, sediment: f64) -> f64 {
    sediment_for_dry_elevation(sediment_dry_elevation(sediment) + to_dry(to) - to_dry(from))
        - sediment
}

/// [`sea_level_for_volume_with_pores`] without pores.
#[cfg(test)]
pub fn sea_level_for_volume(dry_elevations: &[f64], cell_area: f64, volume: f64) -> f64 {
    sea_level_for_volume_with_pores(
        dry_elevations,
        &vec![0.0; dry_elevations.len()],
        cell_area,
        volume,
    )
}

/// [`sea_level_for_volume_with_passes`] where every cell joins the ocean once the sea level
/// rises above its own dry elevation (all water below sea level is ocean).
#[cfg(test)]
pub fn sea_level_for_volume_with_pores(
    dry_elevations: &[f64],
    pores: &[f64],
    cell_area: f64,
    volume: f64,
) -> f64 {
    sea_level_for_volume_with_passes(dry_elevations, dry_elevations, pores, cell_area, volume).level
}

/// A solved sea level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeaLevel {
    /// The sea level (an offset of dry elevations, m).
    pub level: f64,
    /// The part of the volume (m³) that the ocean does not hold at this level: non-zero only
    /// when the level stands at a pass (or at a cell whose pores fill) and the volume would
    /// spill over it into a basin it cannot fill (see [`crate::sea::sea_level`]).
    pub deficit: f64,
}

/// The sea level (as an offset of dry elevations, m) at which the ocean holds `volume` (m³) of
/// water, for cells of equal `cell_area` (m²) with the given dry elevations (m, relative to the
/// same datum), where each submerged cell also holds `pores` (m of water, e.g. sediment
/// thickness times porosity) in its sediment. A cell belongs to the ocean once the sea level
/// is above its pass level `passes` (m, dry, at least its dry elevation): the level at which
/// the ocean reaches it (see [`crate::sea`]); with the passes equal to the dry elevations all
/// water below sea level is ocean.
///
/// The water depth over an ocean cell with dry elevation `d` below sea level `S` is
/// `(S − d) · ρm / (ρm − ρw)` (the water load depresses the floor), so the volume is piecewise
/// linear and increasing in `S`, with a jump where a cell joins below its pass (a basin behind
/// a sill joins at once, filled to the sill) or its pores fill; it is solved exactly. If the
/// volume falls into such a jump, the sea level is that pass: the sea stands at the sill, and
/// the water that would spill into the basin is not represented (the returned deficit). A
/// volume of zero gives the lowest pass; a volume beyond all cells floods the planet.
/// [`crate::sea::sea_level`] solves the same while it floods the passes.
#[cfg(test)]
pub fn sea_level_for_volume_with_passes(
    dry_elevations: &[f64],
    passes: &[f64],
    pores: &[f64],
    cell_area: f64,
    volume: f64,
) -> SeaLevel {
    let mut cells: Vec<(f64, f64, f64)> = passes
        .iter()
        .zip(dry_elevations)
        .zip(pores)
        .map(|((&pass, &dry), &pore)| (pass, dry, pore))
        .collect();
    cells.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    let Some(&(lowest, ..)) = cells.first() else {
        return SeaLevel {
            level: 0.0,
            deficit: 0.0,
        };
    };
    // Σ over ocean cells of (S − dᵢ) + pᵢ / W that holds the volume.
    let target = volume / (cell_area * WATER_LOADING);
    if target <= 0.0 {
        return SeaLevel {
            level: lowest,
            deficit: 0.0,
        };
    }
    let (mut prefix, mut pore_prefix) = (0.0, 0.0);
    // The cells with a lower pass than the current one: their count and sums.
    let mut lower = (0, 0.0, 0.0);
    for k in 1..=cells.len() {
        let (pass, dry, pore) = cells[k - 1];
        if k == 1 || cells[k - 2].0 < pass {
            lower = (k - 1, prefix, pore_prefix);
        }
        prefix += dry;
        pore_prefix += pore / WATER_LOADING;
        // With the k lowest cells in the ocean: k S − Σ dᵢ + Σ pᵢ / W = target.
        let level = (target + prefix - pore_prefix) / k as f64;
        if level < pass {
            // The volume runs out while cell k joins (its basin below the pass, or its pores):
            // the cells with a lower pass hold their share, the rest is the deficit.
            let (count, dry_sum, pore_sum) = lower;
            let held = count as f64 * pass - dry_sum + pore_sum;
            return SeaLevel {
                level: pass,
                deficit: ((target - held) * cell_area * WATER_LOADING).max(0.0),
            };
        }
        if k == cells.len() || level <= cells[k].0 {
            return SeaLevel {
                level,
                deficit: 0.0,
            };
        }
    }
    unreachable!("the loop returns at the last cell")
}

/// Volume (m³) of the ocean over cells of equal `cell_area` with the given surface elevations
/// relative to sea level.
#[cfg(test)]
pub fn ocean_volume(surface_elevations: &[f64], cell_area: f64) -> f64 {
    surface_elevations
        .iter()
        .map(|&elevation| (-elevation).max(0.0))
        .sum::<f64>()
        * cell_area
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submerged_pores_hold_part_of_the_volume() {
        let dry = [-100.0, -50.0, 0.0];
        // Without pores, 2 m³ per m² fill the lowest cell up to −100 + 2 / W.
        let without = sea_level_for_volume_with_pores(&dry, &[0.0; 3], 1.0, 2.0 * WATER_LOADING);
        assert!((without + 98.0).abs() < 1e-9);
        // With 1 m of pores in the lowest cell, the level is lower by 1 / W.
        let with =
            sea_level_for_volume_with_pores(&dry, &[1.0, 0.0, 0.0], 1.0, 2.0 * WATER_LOADING);
        assert!((with + 98.0 + 1.0 / WATER_LOADING).abs() < 1e-9, "{with}");
        // A volume that runs out while the second cell's pores fill: the level stays at its
        // dry elevation.
        let volume = (50.0 + 5.0 / WATER_LOADING) * WATER_LOADING;
        let level = sea_level_for_volume_with_pores(&dry, &[0.0, 10.0, 0.0], 1.0, volume);
        assert_eq!(level, -50.0);
    }

    #[test]
    fn sea_level_holds_the_volume_exactly() {
        let dry: Vec<f64> = (0..1000)
            .map(|i| ((i * 7919) % 1000) as f64 * 7.3 - 4000.0)
            .collect();
        let area = 1e10;
        for volume in [1e12, 1e15, 2.5e16, 1e17] {
            let level = sea_level_for_volume(&dry, area, volume);
            let surface: Vec<f64> = dry.iter().map(|&d| to_surface(d - level)).collect();
            let held = ocean_volume(&surface, area);
            assert!((held / volume - 1.0).abs() < 1e-9, "{held} vs {volume}");
        }
        // More water, higher sea level.
        assert!(sea_level_for_volume(&dry, area, 2e16) > sea_level_for_volume(&dry, area, 1e16));
    }

    #[test]
    fn sea_level_edge_cases() {
        let dry = [-100.0, -100.0, 50.0, 200.0];
        assert_eq!(sea_level_for_volume(&dry, 1.0, 0.0), -100.0);
        // Ties: two cells at the same depth share the water.
        let level = sea_level_for_volume(&dry, 1.0, 2.0 * 10.0 * WATER_LOADING);
        assert!((level + 90.0).abs() < 1e-9, "{level}");
        // Beyond all cells the whole planet is flooded.
        let level = sea_level_for_volume(&dry, 1.0, 4.0 * 1000.0 * WATER_LOADING);
        assert!((level - (1000.0 + (-100.0 - 100.0 + 50.0 + 200.0) / 4.0)).abs() < 1e-9);
        assert_eq!(sea_level_for_volume(&[], 1.0, 1.0), 0.0);
    }

    #[test]
    fn a_uniform_shift_moves_sea_level_and_keeps_surfaces() {
        let dry = [-3000.0, -1000.0, 200.0, 800.0];
        let level = sea_level_for_volume(&dry, 1e10, 3e13);
        let shifted: Vec<f64> = dry.iter().map(|d| d - 250.0).collect();
        let shifted_level = sea_level_for_volume(&shifted, 1e10, 3e13);
        assert!((shifted_level - (level - 250.0)).abs() < 1e-9);
        for (d, s) in dry.iter().zip(&shifted) {
            assert!((to_surface(d - level) - to_surface(s - shifted_level)).abs() < 1e-9);
        }
    }

    #[test]
    fn layers_are_consistent() {
        for (from, to) in [(-4000.0, -120.0), (-50.0, 30.0), (100.0, 900.0)] {
            let thickness = layer_for_rise(from, to, SEDIMENT_DENSITY);
            assert!(
                thickness > to - from,
                "isostasy makes rising more expensive"
            );
            assert!((add_layer(from, thickness, SEDIMENT_DENSITY) - to).abs() < 1e-9);
        }
        // Under water, sediment raises the sea floor by (ρm − ρs) / (ρm − ρw) ≈ 0.44 of its
        // thickness.
        let rise = add_layer(-4000.0, 1000.0, SEDIMENT_DENSITY) + 4000.0;
        assert!((rise - 440.5).abs() < 1.0, "{rise}");
    }

    #[test]
    fn compaction_is_consistent() {
        for sediment in [0.0, 10.0, 1000.0, 5000.0, 20_000.0, 70_000.0] {
            let bulk = sediment_bulk_thickness(sediment);
            assert!((sediment_for_bulk_thickness(bulk) - sediment).abs() < 1e-6);
            let dry = sediment_dry_elevation(sediment);
            assert!((sediment_for_dry_elevation(dry) - sediment).abs() < 1e-6);
            let pores = sediment_pores(sediment);
            assert!(pores >= 0.0 && pores <= bulk * SURFACE_POROSITY + 1e-9);
        }
        // The table matches the exact columns.
        for sediment in [3.0, 777.0, 12_345.0] {
            let exact = bulk_for_grains(sediment_grains(sediment));
            assert!((sediment_bulk_thickness(sediment) - exact).abs() < 1e-2);
            let dry = sediment_dry_elevation_of_bulk(exact);
            assert!((sediment_dry_elevation(sediment) - dry).abs() < 1e-2);
        }
        assert!(sediment_bulk_thickness(80_000.0) > sediment_bulk_thickness(60_000.0));
        // Thin sediment is porous, thick sediment is compacted: the mean porosity of a 1 km
        // column is near the surface value, that of a 10 km column well below the reference.
        let porosity = |s: f64| sediment_pores(s) / sediment_bulk_thickness(s);
        assert!(porosity(10.0) > 0.59);
        assert!(porosity(15_000.0) < SEDIMENT_POROSITY);
        // At the reference porosity, the column has the reference bulk density.
        let reference = sediment_for_bulk_thickness(4000.0);
        let reference_porosity = porosity(reference);
        let bulk_density = SEDIMENT_GRAIN_DENSITY * (1.0 - reference_porosity)
            + WATER_DENSITY * reference_porosity;
        let response = sediment_dry_elevation(reference) / 4000.0;
        assert!((response - dry_response(bulk_density)).abs() < 1e-9);
        // Rising by a given amount takes the sediment the inverse says.
        for (from, to, sediment) in [
            (-4000.0, -2000.0, 0.0),
            (-3000.0, -120.0, 6000.0),
            (-50.0, 2.0, 100.0),
        ] {
            let added = sediment_for_rise(from, to, sediment);
            assert!(added > 0.0);
            assert!((change_sediment(from, sediment, sediment + added) - to).abs() < 1e-6);
        }
        // A thick column takes much more sediment for the same rise than the porous surface.
        let thin = sediment_for_rise(-5000.0, -4000.0, 0.0);
        let thick = sediment_for_rise(-5000.0, -4000.0, 10_000.0);
        assert!(thick > 1.5 * thin, "{thin} {thick}");
    }
}
