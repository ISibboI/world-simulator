//! Crust samples: thickness, density, age and sediment, and the elevation that follows
//! from isostasy and thermal subsidence.

use crate::isostasy::{self, MANTLE_DENSITY, OCEANIC_DENSITY, SEDIMENT_DENSITY, WATER_LOADING};

/// Thickness of new oceanic crust in metres.
pub const OCEANIC_THICKNESS: f64 = 7000.0;

/// Crust whose crystalline layer alone would stand at least this high (dry, in metres) above its
/// compensation depth counts as continental: it is too buoyant to subduct. Normal oceanic crust
/// reaches ~850 m, continental crust ~5800 m, and arc crust becomes continental at ~22 km.
pub const CONTINENTAL_BUOYANCY: f64 = 3000.0;

/// Depth of fully cooled oceanic lithosphere in metres.
const OLD_OCEAN_DEPTH: f64 = 5651.0;

/// Dry elevation that columns are compensated against, chosen such that old oceanic crust of
/// normal thickness lies at [`OLD_OCEAN_DEPTH`] below sea level.
const COMPENSATION: f64 = OCEANIC_THICKNESS * (MANTLE_DENSITY - OCEANIC_DENSITY) / MANTLE_DENSITY
    + OLD_OCEAN_DEPTH / WATER_LOADING;

/// A sample of crust carried by a plate.
#[derive(Clone, Copy, Debug)]
pub struct Crust {
    /// Thickness of the crystalline crust (bedrock) in metres.
    pub thickness: f64,
    /// Mean density of the crystalline crust in kg/m³.
    pub density: f64,
    /// Age of the lithosphere in Myr: time since it formed at a ridge. Continental lithosphere
    /// is old and fully cooled.
    pub age: f64,
    /// Thickness of loose sediment on top of the bedrock in metres.
    pub sediment: f64,
}

impl Crust {
    /// Elevation of the surface relative to sea level in metres, from Airy isostasy of crust and
    /// sediment plus the thermal uplift of young lithosphere.
    pub fn surface_elevation(&self) -> f64 {
        isostasy::to_surface(self.dry_elevation())
    }

    /// Surface elevation without the water load (see [`crate::isostasy`]).
    pub(super) fn dry_elevation(&self) -> f64 {
        self.crustal_buoyancy()
            + self.sediment * isostasy::dry_response(SEDIMENT_DENSITY)
            + thermal_uplift(self.age)
            - COMPENSATION
    }

    /// Dry elevation contributed by the crystalline crust.
    pub(super) fn crustal_buoyancy(&self) -> f64 {
        self.thickness * isostasy::dry_response(self.density)
    }

    /// Whether the crust is thick and light enough to resist subduction.
    pub fn is_continental(&self) -> bool {
        self.crustal_buoyancy() >= CONTINENTAL_BUOYANCY
    }

    /// Crust with higher buoyancy overrides crust with lower buoyancy at convergent boundaries:
    /// continental before oceanic, young (hot) oceanic before old oceanic.
    pub(super) fn buoyancy(&self) -> f64 {
        self.dry_elevation()
    }

    /// Adds crust of the given thickness and density, mixing the densities by mass. Crust beyond
    /// `max_thickness` is removed from below.
    pub(super) fn add_crust(&mut self, thickness: f64, density: f64, max_thickness: f64) {
        let total = self.thickness + thickness;
        if total > 0.0 {
            self.density = (self.density * self.thickness + density * thickness) / total;
        }
        self.thickness = total.min(max_thickness);
    }
}

/// Elevation of lithosphere of the given age above fully cooled lithosphere, without water load.
pub(super) fn thermal_uplift(age: f64) -> f64 {
    (oceanic_depth(age) + OLD_OCEAN_DEPTH) / WATER_LOADING
}

/// Depth of oceanic crust depending on its age, after the GDH1 model (Stein & Stein, 1992):
/// half-space cooling for young crust, flattening towards the depth of a cooled plate. Both
/// branches meet at 20 Myr.
pub fn oceanic_depth(age: f64) -> f64 {
    let age = age.max(0.0);
    if age < 20.0 {
        -(2600.0 + 365.0 * age.sqrt())
    } else {
        -(OLD_OCEAN_DEPTH - 2473.0 * (-0.0278 * age).exp())
    }
}

pub(super) fn new_oceanic_crust() -> Crust {
    Crust {
        thickness: OCEANIC_THICKNESS,
        density: OCEANIC_DENSITY,
        age: 0.0,
        sediment: 0.0,
    }
}
