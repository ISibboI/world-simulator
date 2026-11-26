//! Crust samples: thickness, density, time of formation and sediment, and the elevation that
//! follows from isostasy and thermal subsidence.

use crate::isostasy::{
    self, CONTINENTAL_DENSITY, MANTLE_DENSITY, OCEANIC_DENSITY, SEDIMENT_ROCK_DENSITY,
    WATER_LOADING,
};
use crate::math;

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
    /// Mass fraction of the crystalline crust that is felsic material (continental, mature arc
    /// and metasedimentary crust) rather than mafic (oceanic crust, partly young intra-oceanic
    /// arcs); a tracer for mass budgets.
    pub felsic: f64,
    /// Mass fraction of the crystalline crust that formed by burial metamorphism of sediment
    /// during the simulation (see `Simulation::metamorphose_sediment`); a tracer like
    /// [`Crust::felsic`], mixed by mass. Metasediment is felsic, so it never exceeds
    /// [`Crust::felsic`].
    pub metasediment: f64,
    /// Simulated time in Myr of the last orogeny that deformed this crust (continental
    /// collision, suturing, terrane accretion), or [`NO_OROGENY`]. Orogenic belts and sutures
    /// stay weak zones of the lithosphere long after the orogeny (see `weak_zones`). It is set
    /// and passed on in mixtures only as continental lithosphere (see [`Crust::mix_orogeny`]);
    /// the lithosphere keeps it when it is thinned later.
    pub orogeny: f64,
    /// Remaining orogenic weakening of the sample's lithosphere (0 to 1): 1 where a collision,
    /// suture or accretion marks the sample ([`Crust::orogeny`]); the crust-mass-weighted mean
    /// of the contributors where samples are merged or split (merging plates, stretching
    /// rifts, healing gaps), so that a mixing does not turn a little orogenic lithosphere into
    /// a whole sample of it; and decaying as the deformed lithosphere cools and re-thickens
    /// (see [`Crust::heal_orogenic`]). The decay is linear, so it commutes with the mixing by
    /// mass. Crust added on top or eroded off (arcs, stacked crust, sediment turned to rock)
    /// leaves it unchanged: the weakness is in the lithosphere. The lithosphere is weakened in
    /// proportion (see `weak_zones`).
    pub orogenic: f64,
    /// Simulated time in Myr to which a plume last reset the thermal age of the lithosphere,
    /// or [`NO_REHEATING`]: a plume heats and thins the lithosphere above it as if it had
    /// formed at this time (thermal rejuvenation), and it cools again from there. Carried with
    /// the plate. It only affects the strength of the lithosphere (see
    /// [`Crust::strength_age`]), not the thermal subsidence or the sea-floor age, which follow
    /// [`Crust::birth`]. Where samples are mixed it takes the latest of the contributors (like
    /// `birth` in healed gaps); a sample receiving crust from a merging plate keeps its own.
    pub reheated: f64,
    /// Diagnostic tracers of the tracked cohort of continental cover (see
    /// `Simulation::start_cover_cohort` and [`Cohort`]). Nothing in the simulation reads them.
    pub cohort: Cohort,
}

/// Diagnostic tracers of the cover cohort (#34, #35): the mass fraction of a sample's sediment
/// that belongs to the tracked cohort of continental cover, under two assumptions about the order
/// in which a column loses its sediment. Both mix by mass where sediment is added or moved
/// sideways (a lateral share of a column is a vertical slice of it, so both keep their fraction),
/// and new deposits are new rock, which carries neither. They differ only where sediment is
/// removed from a column (erosion, subduction erosion, burial metamorphism, scraping at
/// trenches):
/// - [`Cohort::mixed`]: removal takes the cohort in proportion (a well-mixed column). The cohort
///   goes faster than in any layered column, so its preservation half-life is a **lower
///   bound**.
/// - [`Cohort::lifo`]: removal takes everything deposited after the cohort first (youngest
///   first, last in first out), and the cohort only once the column is stripped below it. The
///   cohort goes as late as the removed masses allow, so its half-life is an **upper bound**.
///   Burial metamorphism, which physically converts the bottom of a column, is youngest first
///   here as well, to keep it a bound.
///
/// Caveat of the lower bound: burial metamorphism converts the bottom of a column, which in a
/// layered column is the cohort, faster than in proportion. Where metamorphism removes much of
/// the cohort, the well-mixed half-life is therefore not strictly a lower bound (the cohort's
/// fate in the log shows the metamorphosed share).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cohort {
    pub mixed: f64,
    pub lifo: f64,
}

impl Cohort {
    /// Sediment outside the cohort (all new deposits).
    pub const NONE: Self = Self {
        mixed: 0.0,
        lifo: 0.0,
    };

    /// Sediment that all belongs to the cohort.
    pub const ALL: Self = Self {
        mixed: 1.0,
        lifo: 1.0,
    };

    /// Both fractions times `sediment`: the cohort's rock-equivalent thickness in a column of
    /// `sediment` metres, as `[mixed, lifo]`.
    pub fn amounts(self, sediment: f64) -> [f64; 2] {
        [sediment * self.mixed, sediment * self.lifo]
    }
}

/// [`Crust::orogeny`] of crust that never went through an orogeny in the simulation.
pub const NO_OROGENY: f64 = f64::NEG_INFINITY;

/// [`Crust::reheated`] of lithosphere that no plume has heated in the simulation.
pub const NO_REHEATING: f64 = f64::NEG_INFINITY;

impl Crust {
    /// Age of the lithosphere in Myr at the simulated time `time` (Myr).
    pub fn age(&self, time: f64) -> f64 {
        time - self.birth
    }

    /// Thermal age in Myr that sets the strength of the lithosphere at the simulated time
    /// `time`: the age since its formation ([`Crust::age`]) or since a plume last reset it
    /// ([`Crust::reheated`]), whichever is younger.
    pub fn strength_age(&self, time: f64) -> f64 {
        time - self.birth.max(self.reheated)
    }

    /// Resets the thermal age that sets the strength (see [`Crust::reheated`]) to `age` Myr at
    /// the simulated time `time`, unless the lithosphere is already younger.
    pub(super) fn reheat(&mut self, time: f64, age: f64) {
        self.reheated = self.reheated.max(time - age);
    }

    /// Lets the orogenic weakening decay by `factor` (`exp(−dt / τ)`, see
    /// `weak_zones::OROGENIC_HEALING_TIME`).
    pub(super) fn heal_orogenic(&mut self, factor: f64) {
        self.orogenic *= factor;
    }

    /// Elevation of the surface relative to sea level in metres at the simulated time `time`
    /// (Myr) and the sea level `sea_level` (an offset of dry elevations, see
    /// [`crate::sea::sea_level`]), from Airy isostasy of crust and
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
    /// continental crust, whose lithosphere has its own, cold geotherm. The lithosphere is that
    /// of the basement: a filled oceanic basin subsides like the sea floor of its age.
    fn thermal_weight(&self) -> f64 {
        ((NORMAL_CONTINENTAL_BUOYANCY - self.basement().crustal_buoyancy())
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

    /// Thickness (m) of the densified lower crust: the part of the crust deeper than
    /// [`DENSE_LOWER_CRUST_DEPTH`]. Removing crust from the top shrinks this part first (with
    /// [`Crust::removal_isostatic_density`]), then the normal crust (with [`Crust::density`]).
    pub fn dense_crust_thickness(&self) -> f64 {
        (self.thickness - DENSE_LOWER_CRUST_DEPTH).max(0.0)
    }

    /// Areal mass (kg/m²) of the crust: crystalline crust plus the rock mass of its sediment.
    pub fn mass(&self) -> f64 {
        self.thickness * self.density + self.sediment * SEDIMENT_ROCK_DENSITY
    }

    /// Whether the crust is continental: thick and light (see [`CONTINENTAL_BUOYANCY`]).
    ///
    /// Like all classifications of a sample, this judges its basement (see
    /// [`Crust::basement`]): metasediment formed on oceanic, arc or thinned crust keeps the
    /// fate of the lithosphere it formed on (thick fills of oceanic basins still subduct below
    /// their accretionary wedge: Makran, Andaman, Cascadia). Crust becomes continental from
    /// sediment only by accretion onto an overriding plate and by collisions.
    pub fn is_continental(&self) -> bool {
        self.basement().crustal_buoyancy() >= CONTINENTAL_BUOYANCY
    }

    /// Whether the crust is too buoyant to subduct (see [`SUBDUCTION_RESISTANCE_BUOYANCY`]):
    /// continental crust, thick arcs and plateaus, and stretched continental margins whose
    /// sediment wedge makes up for the thinning. A margin arriving at a trench with a thick
    /// wedge jams it and is stacked into the orogen (Banda/Timor, Taiwan). Sediment counts only
    /// on continental (felsic, not oceanic) crust: thick fill on oceanic crust subducts below
    /// its accretionary wedge, pulled by the old mantle lithosphere (Makran, Cascadia), which
    /// this crustal criterion does not see. The sediment counts at its compacted grain density
    /// (metamorphosed at depth), not as porous bulk, and so does metasediment on such margins.
    /// Judged on the basement (see [`Crust::is_continental`]).
    pub fn resists_subduction(&self) -> bool {
        let basement = self.basement();
        basement.crust_resists_subduction()
            || (basement.felsic > 0.5
                && !basement.is_oceanic_column()
                && basement.crustal_buoyancy()
                    + self.sediment
                        * (1.0 - isostasy::SEDIMENT_POROSITY)
                        * isostasy::dry_response(isostasy::SEDIMENT_GRAIN_DENSITY)
                    + self.metasediment_thickness() * isostasy::dry_response(METASEDIMENT_DENSITY)
                    >= SUBDUCTION_RESISTANCE_BUOYANCY)
    }

    /// Whether the crystalline crust alone (as it is, metasediment included) is too buoyant to
    /// subduct.
    fn crust_resists_subduction(&self) -> bool {
        self.crustal_buoyancy() >= SUBDUCTION_RESISTANCE_BUOYANCY
    }

    /// Whether the crust is of continental origin: mostly felsic (see [`Crust::felsic`]) and its
    /// crystalline crust too buoyant to subduct. Unlike [`Crust::is_continental`] this includes
    /// stretched continental margins, and it excludes thin intra-oceanic arcs. Judged on the
    /// basement (see [`Crust::is_continental`]).
    pub fn is_continental_origin(&self) -> bool {
        self.basement().is_continental_origin_column()
    }

    /// [`Crust::is_continental_origin`] for the whole crystalline column, metasediment
    /// included: for processes that depend on the rock that is there rather than on the
    /// lithosphere (crustal flow, rifting).
    pub fn is_continental_origin_column(&self) -> bool {
        self.felsic > 0.5 && self.crust_resists_subduction()
    }

    /// Takes over the orogeny mark `orogeny` of crust mixed into this sample (see
    /// [`Crust::orogeny`]): the latest of the two if the mixed sample is still continental
    /// lithosphere ([`Crust::is_continental_origin`]), and none otherwise. The weakness of
    /// orogenic belts lies in their deformed continental lithosphere (mantle fabric, faults); it
    /// is carried with that lithosphere, but oceanic or transitional crust made from a mixture
    /// starts unweakened.
    ///
    /// `orogenic` is the mass fraction of the mixture that stands on orogenic lithosphere (see
    /// [`Crust::orogenic`]).
    pub(super) fn mix_orogeny(&mut self, orogeny: f64, orogenic: f64) {
        if self.is_continental_origin() {
            self.orogeny = self.orogeny.max(orogeny);
            self.orogenic = orogenic.clamp(0.0, 1.0);
        } else {
            self.orogeny = NO_OROGENY;
            self.orogenic = 0.0;
        }
    }

    /// Marks the sample as deformed in an orogeny at `time` (Myr).
    pub(super) fn mark_orogeny(&mut self, time: f64) {
        self.orogeny = time;
        self.orogenic = 1.0;
    }

    /// Whether this is normal oceanic crust: thin and dense. Crust that is neither oceanic nor
    /// continental is transitional (thickened arc crust, or thinned continental crust, whose age
    /// is not the age of a sea floor). Judged on the basement (see [`Crust::is_continental`]).
    pub fn is_oceanic(&self) -> bool {
        self.basement().is_oceanic_column()
    }

    /// [`Crust::is_oceanic`] for the whole crystalline column, metasediment included.
    pub fn is_oceanic_column(&self) -> bool {
        self.thickness < MAX_OCEANIC_THICKNESS && self.density > MIN_OCEANIC_DENSITY
    }

    /// Crust with higher buoyancy overrides crust with lower buoyancy at convergent boundaries:
    /// continental before oceanic, young (hot) oceanic before old oceanic.
    pub(super) fn buoyancy(&self, time: f64) -> f64 {
        self.dry_elevation(time)
    }

    /// Adds crust of the given thickness, density, felsic and metasediment mass fractions,
    /// mixing densities and tracers by mass. Crust beyond `max_thickness` is removed from below
    /// (with the mean composition).
    pub(super) fn add_crust(
        &mut self,
        thickness: f64,
        density: f64,
        (felsic, metasediment): (f64, f64),
        max_thickness: f64,
    ) {
        let total = self.thickness + thickness;
        let (mass, added) = (self.thickness * self.density, thickness * density);
        if mass + added > 0.0 {
            self.felsic = (mass * self.felsic + added * felsic) / (mass + added);
            self.metasediment = (mass * self.metasediment + added * metasediment) / (mass + added);
        }
        if total > 0.0 {
            self.density = (mass + added) / total;
        }
        self.thickness = total;
        self.thin_from_below(total.min(max_thickness));
    }

    /// Thins the crystalline crust to `thickness` (not thicker than now) from the top, as
    /// erosion does: metasediment, which lies on its basement, goes first.
    pub(super) fn thin_from_top(&mut self, thickness: f64) {
        let thickness = thickness.clamp(0.0, self.thickness);
        let removed = (self.thickness - thickness) * self.density;
        let meta = (self.metasediment_mass() - removed).max(0.0);
        self.thickness = thickness;
        self.set_metasediment_mass(meta);
    }

    /// Thins the crystalline crust to `thickness` (not thicker than now) from below (thickness
    /// caps, subduction erosion of the underside): the basement goes first.
    pub(super) fn thin_from_below(&mut self, thickness: f64) {
        let thickness = thickness.clamp(0.0, self.thickness);
        let meta = self.metasediment_mass();
        self.thickness = thickness;
        self.set_metasediment_mass(meta);
    }

    fn set_metasediment_mass(&mut self, meta: f64) {
        let mass = self.thickness * self.density;
        self.metasediment = if mass > 0.0 {
            (meta / mass).min(self.felsic).clamp(0.0, 1.0)
        } else {
            0.0
        };
    }

    /// Adds `sediment` (rock-equivalent m) carrying the cohort fractions `cohort` (see
    /// [`Crust::cohort`]).
    pub(crate) fn add_sediment(&mut self, sediment: f64, cohort: Cohort) {
        let total = self.sediment + sediment;
        if total > 0.0 {
            self.cohort = Cohort {
                mixed: (self.sediment * self.cohort.mixed + sediment * cohort.mixed) / total,
                lifo: (self.sediment * self.cohort.lifo + sediment * cohort.lifo) / total,
            };
        }
        self.sediment = total;
    }

    /// Lowers the sediment to `sediment` (rock-equivalent m, not more than now) by removing it
    /// from the column, and updates the cohort tracers (see [`Cohort`]): the well-mixed one keeps
    /// its fraction, the youngest-first one loses the cohort only below the removed part.
    /// Returns the cohort amounts removed (rock-equivalent m, `[mixed, lifo]`).
    pub(crate) fn remove_sediment_to(&mut self, sediment: f64) -> [f64; 2] {
        let before = self.cohort.amounts(self.sediment);
        let tagged = before[1].min(sediment.max(0.0));
        self.sediment = sediment;
        self.cohort.lifo = if sediment > 0.0 {
            (tagged / sediment).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let after = self.cohort.amounts(self.sediment.max(0.0));
        [before[0] - after[0], before[1] - after[1]]
    }

    /// Areal mass (kg/m²) of the felsic part of the crystalline crust.
    pub fn felsic_mass(&self) -> f64 {
        self.thickness * self.density * self.felsic
    }

    /// Areal mass (kg/m²) of the metasediment in the crystalline crust (see
    /// [`Crust::metasediment`]).
    pub fn metasediment_mass(&self) -> f64 {
        self.thickness * self.density * self.metasediment
    }

    /// Thickness (m) of the metasediment in the crystalline crust.
    pub fn metasediment_thickness(&self) -> f64 {
        self.metasediment_mass() / METASEDIMENT_DENSITY
    }

    /// The crust without its metasediment: the basement it formed on, with the same sediment
    /// and age. Metasediment is felsic, so it is taken from the felsic part.
    pub fn basement(&self) -> Crust {
        if self.metasediment <= 0.0 {
            return *self;
        }
        let mass = self.thickness * self.density;
        let meta = self.metasediment_mass().min(mass);
        let thickness = (self.thickness - meta / METASEDIMENT_DENSITY).max(0.0);
        let rest = mass - meta;
        if thickness <= 0.0 || rest <= 0.0 {
            // All metasediment: it formed on oceanic lithosphere, whose crust is gone.
            return Crust {
                thickness: 0.0,
                density: isostasy::OCEANIC_DENSITY,
                felsic: 0.0,
                metasediment: 0.0,
                ..*self
            };
        }
        Crust {
            thickness,
            density: if thickness > 0.0 && rest > 0.0 {
                rest / thickness
            } else {
                self.density
            },
            felsic: if rest > 0.0 {
                ((self.felsic_mass() - meta) / rest).clamp(0.0, 1.0)
            } else {
                0.0
            },
            metasediment: 0.0,
            ..*self
        }
    }
}

/// Density of metasediment (kg/m³): metamorphosed sediment, accreted wedges and stacked
/// sediment (greenschist to amphibolite facies pelites and greywackes).
pub(super) const METASEDIMENT_DENSITY: f64 = 2750.0;

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
    if anomaly >= 2473.0 * math::exp(-0.0278f64 * 20.0) {
        Some(((ridge - anomaly) / 365.0).powi(2))
    } else if anomaly > 1e-3 {
        Some(-math::ln(anomaly / 2473.0) / 0.0278)
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
        -(OLD_OCEAN_DEPTH - 2473.0 * math::exp(-0.0278 * age))
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
        metasediment: 0.0,
        orogeny: NO_OROGENY,
        orogenic: 0.0,
        reheated: NO_REHEATING,
        cohort: Cohort::NONE,
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
    fn a_new_orogeny_deforms_the_whole_sample() {
        let mut crust = Crust {
            thickness: 35_000.0,
            density: CONTINENTAL_DENSITY,
            birth: -1000.0,
            sediment: 0.0,
            felsic: 1.0,
            metasediment: 0.0,
            orogeny: NO_OROGENY,
            orogenic: 0.0,
            reheated: NO_REHEATING,
            cohort: Cohort::NONE,
        };
        // A share of an old belt mixed in: the latest orogeny, a part of the lithosphere.
        crust.mix_orogeny(-500.0, 0.2);
        assert_eq!((crust.orogeny, crust.orogenic), (-500.0, 0.2));
        // An older mark does not replace the time.
        crust.mix_orogeny(-800.0, 0.3);
        assert_eq!((crust.orogeny, crust.orogenic), (-500.0, 0.3));
        // A collision at the sample deforms all of it.
        crust.mark_orogeny(-100.0);
        assert_eq!((crust.orogeny, crust.orogenic), (-100.0, 1.0));
        // Oceanic lithosphere carries no mark.
        let mut ocean = new_oceanic_crust(-50.0);
        ocean.mix_orogeny(-100.0, 0.5);
        assert_eq!((ocean.orogeny, ocean.orogenic), (NO_OROGENY, 0.0));
    }

    #[test]
    fn felsic_mass_is_conserved_when_mixing() {
        let mut crust = new_oceanic_crust(0.0);
        let before = crust.felsic_mass();
        crust.add_crust(2000.0, CONTINENTAL_DENSITY, (1.0, 0.0), 75_000.0);
        let added = 2000.0 * CONTINENTAL_DENSITY;
        assert!((crust.felsic_mass() - (before + added)).abs() < 1e-6 * added);
        assert!((0.0..1.0).contains(&crust.felsic));
    }

    #[test]
    fn metasediment_keeps_the_fate_of_its_lithosphere() {
        // 15 km of burial metasediment on 7 km oceanic crust: felsic and buoyant as a whole,
        // but classified, and cooling, by its oceanic basement.
        let mut filled = new_oceanic_crust(-100.0);
        filled.add_crust(15_000.0, METASEDIMENT_DENSITY, (1.0, 1.0), 75_000.0);
        assert!(filled.felsic > 0.5 && filled.is_continental_origin_column());
        assert!(filled.is_oceanic() && !filled.is_continental_origin());
        assert!(!filled.is_continental() && !filled.resists_subduction());
        let basement = filled.basement();
        assert!((basement.thickness - OCEANIC_THICKNESS).abs() < 1e-6);
        assert!(basement.felsic.abs() < 1e-9);
        // It subsides with age like the sea floor (and stands higher by its metasediment).
        let young = filled.dry_elevation(-90.0);
        let old = filled.dry_elevation(0.0);
        let ocean = new_oceanic_crust(-100.0);
        let drop = young - old;
        assert!((drop - (ocean.dry_elevation(-90.0) - ocean.dry_elevation(0.0))).abs() < 1e-6);
        // Erosion takes the metasediment first (the eroded mass at the mean density, as the
        // erosion's mass budget counts it).
        let mut eroded = filled;
        eroded.thin_from_top(filled.thickness - 5000.0);
        let expected = filled.metasediment_mass() - 5000.0 * filled.density;
        assert!((eroded.metasediment_mass() - expected).abs() < 1e-6 * expected);
        assert!((eroded.basement().thickness - OCEANIC_THICKNESS).abs() < 200.0);
        // A thinned continental margin with a metasediment wedge still jams a trench.
        let mut margin = Crust {
            thickness: 9000.0,
            density: CONTINENTAL_DENSITY,
            birth: -1000.0,
            sediment: 0.0,
            felsic: 1.0,
            metasediment: 0.0,
            orogeny: NO_OROGENY,
            orogenic: 0.0,
            reheated: NO_REHEATING,
            cohort: Cohort::NONE,
        };
        assert!(!margin.resists_subduction());
        margin.add_crust(8000.0, METASEDIMENT_DENSITY, (1.0, 1.0), 75_000.0);
        assert!(!margin.is_continental_origin() && margin.resists_subduction());
        // Crust that is all metasediment is oceanic lithosphere.
        let pure = Crust {
            thickness: 5000.0,
            density: METASEDIMENT_DENSITY,
            felsic: 1.0,
            metasediment: 1.0,
            ..new_oceanic_crust(0.0)
        };
        assert!(pure.is_oceanic());
    }

    #[test]
    fn only_continental_lithosphere_keeps_the_orogeny_mark() {
        let belt = Crust {
            thickness: 35_000.0,
            density: CONTINENTAL_DENSITY,
            birth: -1000.0,
            sediment: 0.0,
            felsic: 1.0,
            metasediment: 0.0,
            orogeny: 200.0,
            orogenic: 1.0,
            reheated: NO_REHEATING,
            cohort: Cohort::NONE,
        };
        let mut mixed = belt;
        mixed.mix_orogeny(400.0, 0.3);
        assert_eq!(mixed.orogeny, 400.0);
        assert_eq!(mixed.orogenic, 0.3);
        // A metamorphosed wedge on oceanic basement is felsic and buoyant as a whole, but its
        // lithosphere is oceanic: it does not carry the mark of a belt mixed into it.
        let mut wedge = new_oceanic_crust(-100.0);
        wedge.add_crust(15_000.0, METASEDIMENT_DENSITY, (1.0, 1.0), 75_000.0);
        assert!(wedge.is_continental_origin_column() && wedge.is_oceanic());
        wedge.mix_orogeny(400.0, 0.3);
        assert_eq!(wedge.orogeny, NO_OROGENY);
        assert_eq!(wedge.orogenic, 0.0);
    }

    #[test]
    fn sediment_wedges_on_stretched_margins_resist_subduction() {
        let margin = Crust {
            thickness: 11_000.0,
            density: CONTINENTAL_DENSITY,
            birth: -1000.0,
            sediment: 0.0,
            felsic: 1.0,
            metasediment: 0.0,
            orogeny: NO_OROGENY,
            orogenic: 0.0,
            reheated: NO_REHEATING,
            cohort: Cohort::NONE,
        };
        assert!(!margin.resists_subduction() && !margin.is_continental_origin());
        let wedged = Crust {
            sediment: 8000.0,
            ..margin
        };
        assert!(wedged.resists_subduction());
        // Its crust stays what it is: not of continental origin.
        assert!(!wedged.is_continental_origin());
        // Thick fill on oceanic crust still subducts.
        let ocean = Crust {
            sediment: 8000.0,
            ..new_oceanic_crust(-100.0)
        };
        assert!(!ocean.resists_subduction());
    }
}
