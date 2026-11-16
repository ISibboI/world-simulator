//! Crust samples: thickness, density, time of formation and sediment, and the elevation that
//! follows from isostasy and thermal subsidence.

use crate::isostasy::{
    self, CONTINENTAL_DENSITY, MANTLE_DENSITY, OCEANIC_DENSITY, SEDIMENT_DENSITY,
    SEDIMENT_ROCK_DENSITY, WATER_LOADING,
};

/// Oceanic crust is thinner than this (m); thicker crust is transitional or continental.
const MAX_OCEANIC_THICKNESS: f64 = 12_000.0;

/// Oceanic crust is denser than this (kg/m³), between continental and oceanic density.
const MIN_OCEANIC_DENSITY: f64 = 2850.0;

/// Thickness of new oceanic crust in metres.
pub const OCEANIC_THICKNESS: f64 = 7000.0;

/// Crust whose crystalline layer alone would stand at least this high (dry, in metres) above its
/// compensation depth counts as continental: it is too buoyant to subduct. Normal oceanic crust
/// reaches ~850 m, continental crust ~5800 m, and arc crust becomes continental at ~22 km.
pub const CONTINENTAL_BUOYANCY: f64 = 3000.0;

/// Crust deeper than this (m) densifies in thick orogens (granulite, partial eclogitisation).
const DENSE_LOWER_CRUST_DEPTH: f64 = 50_000.0;

/// Effective density (kg/m³) of densified lower crust, between granulite and eclogite. At the
/// mantle density or above, thickening would lower the surface, which would need delamination
/// dynamics; this value gives ~4.85 km for 70 km crust (Tibet ~5 km, Altiplano ~4 km).
const DENSE_LOWER_CRUST_DENSITY: f64 = 2950.0;

/// Crustal buoyancy (dry, m) of normal 35 km continental crust: the thermal uplift of the
/// lithosphere fades out between [`CONTINENTAL_BUOYANCY`] and this value.
const NORMAL_CONTINENTAL_BUOYANCY: f64 =
    35_000.0 * (MANTLE_DENSITY - CONTINENTAL_DENSITY) / MANTLE_DENSITY;

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
    /// Simulated time in Myr at which the lithosphere formed at a ridge (negative for
    /// lithosphere older than the simulation). Its age is the time since then (see
    /// [`Crust::age`]); continental lithosphere is old and fully cooled.
    pub birth: f64,
    /// Thickness of loose sediment on top of the bedrock in metres.
    pub sediment: f64,
}

impl Crust {
    /// Age of the lithosphere in Myr at the simulated time `time` (Myr).
    pub fn age(&self, time: f64) -> f64 {
        time - self.birth
    }

    /// Elevation of the surface relative to sea level in metres at the simulated time `time`
    /// (Myr) and the sea level `sea_level` (an offset of dry elevations, see
    /// [`crate::isostasy::sea_level_for_volume`]), from Airy isostasy of crust and sediment plus
    /// the thermal uplift of young lithosphere.
    pub fn surface_elevation(&self, time: f64, sea_level: f64) -> f64 {
        isostasy::to_surface(self.dry_elevation(time) - sea_level)
    }

    /// Surface elevation at time `time` without the water load (see [`crate::isostasy`]).
    pub(super) fn dry_elevation(&self, time: f64) -> f64 {
        self.crustal_buoyancy()
            + self.sediment * isostasy::dry_response(SEDIMENT_DENSITY)
            + self.thermal_weight() * thermal_uplift(self.age(time))
            - COMPENSATION
    }

    /// Dry elevation contributed by the crystalline crust. In thick crust, the part deeper than
    /// [`DENSE_LOWER_CRUST_DEPTH`] densifies at constant mass: its volume shrinks by `ρ / ρd`
    /// and it is less buoyant.
    pub(super) fn crustal_buoyancy(&self) -> f64 {
        let normal = self.thickness.min(DENSE_LOWER_CRUST_DEPTH);
        let excess = self.thickness - normal;
        let dense_density = DENSE_LOWER_CRUST_DENSITY.max(self.density);
        normal * isostasy::dry_response(self.density)
            + excess * self.density / dense_density * isostasy::dry_response(dense_density)
    }

    /// How much of the oceanic thermal uplift applies: fully to oceanic and arc lithosphere
    /// (arcs and back-arcs are hot), fading out between the continental threshold and normal
    /// continental crust, whose lithosphere has its own, cold geotherm.
    fn thermal_weight(&self) -> f64 {
        ((NORMAL_CONTINENTAL_BUOYANCY - self.crustal_buoyancy())
            / (NORMAL_CONTINENTAL_BUOYANCY - CONTINENTAL_BUOYANCY))
            .clamp(0.0, 1.0)
    }

    /// The density with which [`isostasy::add_layer`] gives the same change of elevation as
    /// thinning this crust from the top: at the base of thick crust, the densified layer shrinks.
    pub fn removal_isostatic_density(&self) -> f64 {
        if self.thickness > DENSE_LOWER_CRUST_DEPTH {
            let dense_density = DENSE_LOWER_CRUST_DENSITY.max(self.density);
            let slope = self.density / dense_density * isostasy::dry_response(dense_density);
            MANTLE_DENSITY * (1.0 - slope)
        } else {
            self.density
        }
    }

    /// Areal mass (kg/m²) of the crust: crystalline crust plus the rock mass of its sediment.
    pub fn mass(&self) -> f64 {
        self.thickness * self.density + self.sediment * SEDIMENT_ROCK_DENSITY
    }

    /// Whether the crust is thick and light enough to resist subduction.
    pub fn is_continental(&self) -> bool {
        self.crustal_buoyancy() >= CONTINENTAL_BUOYANCY
    }

    /// Whether this is normal oceanic crust: thin and dense. Crust that is neither oceanic nor
    /// continental is transitional (thickened arc crust, or thinned continental crust, whose age
    /// is not the age of a sea floor).
    pub fn is_oceanic(&self) -> bool {
        self.thickness < MAX_OCEANIC_THICKNESS && self.density > MIN_OCEANIC_DENSITY
    }

    /// Crust with higher buoyancy overrides crust with lower buoyancy at convergent boundaries:
    /// continental before oceanic, young (hot) oceanic before old oceanic.
    pub(super) fn buoyancy(&self, time: f64) -> f64 {
        self.dry_elevation(time)
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

/// New oceanic crust formed at the simulated time `time` (Myr).
pub(super) fn new_oceanic_crust(time: f64) -> Crust {
    Crust {
        thickness: OCEANIC_THICKNESS,
        density: OCEANIC_DENSITY,
        birth: time,
        sediment: 0.0,
    }
}
