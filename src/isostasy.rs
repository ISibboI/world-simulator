//! Airy isostasy: columns of crust and sediment float on the mantle.
//!
//! Adding a layer of thickness `h` and density `ρ` to a column raises its surface by
//! `h (ρ_m − ρ) / ρ_m` above sea level. Below sea level the column is additionally loaded by the
//! water that fills the space above it, which amplifies vertical motions by
//! `ρ_m / (ρ_m − ρ_w)`.
//!
//! To handle columns that cross sea level, elevations are converted to *dry* elevations (the
//! elevation the column would have without water loading), where contributions simply add up.

/// Density of the upper mantle in kg/m³.
pub const MANTLE_DENSITY: f64 = 3300.0;
/// Density of sea water in kg/m³.
pub const WATER_DENSITY: f64 = 1030.0;
/// Bulk density of (compacted, water-saturated) sediment in kg/m³, used for isostasy.
pub const SEDIMENT_DENSITY: f64 = 2300.0;
/// Rock (grain) mass per volume of sediment in kg/m³: 25% pores in 2700 kg/m³ grains. With
/// water in the pores this gives the bulk density above. Mass conservation uses this density.
pub const SEDIMENT_ROCK_DENSITY: f64 = 2050.0;
/// Density of sediment grains in kg/m³.
pub const SEDIMENT_GRAIN_DENSITY: f64 = 2700.0;
/// Pore fraction of sediment (filled with water below sea level).
pub const SEDIMENT_POROSITY: f64 = 1.0 - SEDIMENT_ROCK_DENSITY / SEDIMENT_GRAIN_DENSITY;
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
pub fn layer_for_rise(from: f64, to: f64, density: f64) -> f64 {
    (to_dry(to) - to_dry(from)) / dry_response(density)
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

/// The sea level (as an offset of dry elevations, m) at which the ocean holds `volume` (m³) of
/// water, for cells of equal `cell_area` (m²) with the given dry elevations (m, relative to the
/// same datum), where each submerged cell also holds `pores` (m of water, e.g. sediment
/// thickness times porosity) in its sediment. The water depth over a cell with dry elevation `d`
/// below sea level `S` is `(S − d) · ρm / (ρm − ρw)` (the water load depresses the floor), so the
/// volume is piecewise linear and increasing in `S`, with a jump where a cell's pores fill; it
/// is solved exactly. If the volume falls into such a jump, the sea level is that cell's dry
/// elevation (its pores partly filled). A volume of zero gives the lowest dry elevation; a
/// volume beyond all cells floods the planet.
pub fn sea_level_for_volume_with_pores(
    dry_elevations: &[f64],
    pores: &[f64],
    cell_area: f64,
    volume: f64,
) -> f64 {
    let mut cells: Vec<(f64, f64)> = dry_elevations
        .iter()
        .copied()
        .zip(pores.iter().copied())
        .collect();
    cells.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    let Some(&(lowest, _)) = cells.first() else {
        return 0.0;
    };
    // Σ over submerged cells of (S − dᵢ) + pᵢ / W that holds the volume.
    let target = volume / (cell_area * WATER_LOADING);
    if target <= 0.0 {
        return lowest;
    }
    let (mut prefix, mut pore_prefix) = (0.0, 0.0);
    for k in 1..=cells.len() {
        let (dry, pore) = cells[k - 1];
        prefix += dry;
        pore_prefix += pore / WATER_LOADING;
        // With the k lowest cells below sea level: k S − Σ dᵢ + Σ pᵢ / W = target.
        let level = (target + prefix - pore_prefix) / k as f64;
        if level < dry {
            // The volume runs out while the pores of cell k fill.
            return dry;
        }
        if k == cells.len() || level <= cells[k].0 {
            return level;
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
}
