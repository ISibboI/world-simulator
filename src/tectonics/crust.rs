//! Crust samples: thickness, density, time of formation and sediment, and the elevation that
//! follows from isostasy and thermal subsidence.

use crate::isostasy::{
    self, CONTINENTAL_DENSITY, MANTLE_DENSITY, OCEANIC_DENSITY, SEDIMENT_ROCK_DENSITY,
    WATER_LOADING,
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

/// Crust whose crystalline layer alone would stand at least this high (dry, in metres) resists
/// subduction: arc and continental crust thicker than ~13–15 km, basaltic plateaus thicker than
/// ~17 km (Cloos 1993). At trenches it is accreted to the overriding plate instead.
pub const SUBDUCTION_RESISTANCE_BUOYANCY: f64 = 2000.0;

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
    /// Sediment on top of the bedrock as a rock-equivalent thickness in metres (mass /
    /// [`SEDIMENT_ROCK_DENSITY`]); its actual thickness depends on compaction (see
    /// [`isostasy::sediment_bulk_thickness`]).
    pub sediment: f64,
    /// Mass fraction of the crystalline crust that is felsic material (continental, arc and
    /// metasedimentary crust) rather than mafic (oceanic) crust; a tracer for mass budgets.
    pub felsic: f64,
}

impl Crust {
    /// Age of the lithosphere in Myr at the simulated time `time` (Myr).
    pub fn age(&self, time: f64) -> f64 {
        time - self.birth
    }

    /// Elevation of the surface relative to sea level in metres at the simulated time `time`
    /// (Myr) and the sea level `sea_level` (an offset of dry elevations, see
    /// [`crate::isostasy::sea_level_for_volume_with_pores`]), from Airy isostasy of crust and
    /// sediment plus the thermal uplift of young lithosphere.
    pub fn surface_elevation(&self, time: f64, sea_level: f64) -> f64 {
        isostasy::to_surface(self.dry_elevation(time) - sea_level)
    }

    /// Surface elevation at time `time` without the water load (see [`crate::isostasy`]).
    pub(super) fn dry_elevation(&self, time: f64) -> f64 {
        self.crustal_buoyancy()
            + isostasy::sediment_dry_elevation(self.sediment)
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

    /// Whether the crust is continental: thick and light (see [`CONTINENTAL_BUOYANCY`]).
    pub fn is_continental(&self) -> bool {
        self.crustal_buoyancy() >= CONTINENTAL_BUOYANCY
    }

    /// Whether the crust is too buoyant to subduct (see [`SUBDUCTION_RESISTANCE_BUOYANCY`]):
    /// continental crust, thick arcs and plateaus.
    pub fn resists_subduction(&self) -> bool {
        self.crustal_buoyancy() >= SUBDUCTION_RESISTANCE_BUOYANCY
    }

    /// Whether the crust is of continental origin: mostly felsic (see [`Crust::felsic`]) and too
    /// buoyant to subduct. Unlike [`Crust::is_continental`] this includes stretched continental
    /// margins, and it excludes thin intra-oceanic arcs.
    pub fn is_continental_origin(&self) -> bool {
        self.felsic > 0.5 && self.resists_subduction()
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

    /// Adds crust of the given thickness, density and felsic mass fraction, mixing densities
    /// and felsic fractions by mass. Crust beyond `max_thickness` is removed from below.
    pub(super) fn add_crust(
        &mut self,
        thickness: f64,
        density: f64,
        felsic: f64,
        max_thickness: f64,
    ) {
        let total = self.thickness + thickness;
        let (mass, added) = (self.thickness * self.density, thickness * density);
        if mass + added > 0.0 {
            self.felsic = (mass * self.felsic + added * felsic) / (mass + added);
        }
        if total > 0.0 {
            self.density = (mass + added) / total;
        }
        self.thickness = total.min(max_thickness);
    }

    /// Areal mass (kg/m²) of the felsic part of the crystalline crust.
    pub fn felsic_mass(&self) -> f64 {
        self.thickness * self.density * self.felsic
    }
}

/// Elevation of lithosphere of the given age above fully cooled lithosphere, without water load.
pub(super) fn thermal_uplift(age: f64) -> f64 {
    (oceanic_depth(age) + OLD_OCEAN_DEPTH) / WATER_LOADING
}

/// Thermal uplift of lithosphere of the given age as a fraction of that of new lithosphere at a
/// ridge (1 at age 0, falling to 0 as it cools).
pub(super) fn thermal_fraction(age: f64) -> f64 {
    thermal_uplift(age) / thermal_uplift(0.0)
}

/// The age (Myr) of lithosphere whose thermal uplift is the fraction `fraction` of that at a
/// ridge: the inverse of [`thermal_fraction`], in closed form for both GDH1 branches. Returns
/// `None` for fractions too close to 0 (fully cooled).
pub(super) fn age_for_thermal_fraction(fraction: f64) -> Option<f64> {
    // Depth below the fully cooled depth (m): 3051 at a ridge.
    let ridge = OLD_OCEAN_DEPTH - 2600.0;
    let anomaly = fraction.min(1.0) * ridge;
    // The branches meet at 20 Myr, where the anomaly is 2473 e^(−0.0278 · 20).
    if anomaly >= 2473.0 * (-0.0278f64 * 20.0).exp() {
        Some(((ridge - anomaly) / 365.0).powi(2))
    } else if anomaly > 1e-3 {
        Some(-(anomaly / 2473.0).ln() / 0.0278)
    } else {
        None
    }
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
        felsic: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thermal_fraction_inverts() {
        for age in [0.0, 1.0, 10.0, 19.9, 20.0, 20.1, 50.0, 150.0] {
            let back = age_for_thermal_fraction(thermal_fraction(age)).unwrap();
            assert!((back - age).abs() < 1e-6 * (1.0 + age), "{age} → {back}");
        }
        assert!(age_for_thermal_fraction(0.0).is_none());
    }

    #[test]
    fn felsic_mass_is_conserved_when_mixing() {
        let mut crust = new_oceanic_crust(0.0);
        let before = crust.felsic_mass();
        crust.add_crust(2000.0, CONTINENTAL_DENSITY, 1.0, 75_000.0);
        let added = 2000.0 * CONTINENTAL_DENSITY;
        assert!((crust.felsic_mass() - (before + added)).abs() < 1e-6 * added);
        assert!((0.0..1.0).contains(&crust.felsic));
    }
}
