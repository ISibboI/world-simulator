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
/// Density of (compacted) sediment in kg/m³.
pub const SEDIMENT_DENSITY: f64 = 2300.0;
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

#[cfg(test)]
mod tests {
    use super::*;

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
