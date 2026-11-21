//! Bookkeeping of the mass of the crust.
//!
//! Every operation that creates, destroys or moves crust records the change of mass it actually
//! caused (not its nominal size) in a term of the ledger. Comparing the total mass of all crust
//! samples, including samples hidden below other plates, with the recorded terms gives a
//! residual, which shows any change that is not accounted for. Summing all samples is costly, so
//! the comparison only runs when the ledger is reported (see [`MassLedger::checked`]); the
//! residual telescopes, so this gives the same result as checking after every operation.
//!
//! Masses are areal masses (kg/m²) summed over cells: crystalline crust counts as
//! `thickness · density`, sediment as `thickness · ρ_rock` (its rock mass, see
//! [`crate::isostasy::SEDIMENT_ROCK_DENSITY`]). Multiply by the cell area for kilograms.

use super::*;

/// Where crust mass came from or went to, accumulated over the simulation.
#[derive(Clone, Debug, Default)]
pub struct MassLedger {
    /// New oceanic crust at spreading ridges (+).
    pub new_sea_floor: f64,
    /// New arc crust above subducting slabs (+).
    pub arcs: f64,
    /// Subducted oceanic crust (−).
    pub subducted_oceanic: f64,
    /// Subducted transitional crust: thickened arc crust or thinned continental crust (−).
    pub subducted_transitional: f64,
    /// Subducted sediment, after frontal accretion (−).
    pub subducted_sediment: f64,
    /// Sediment scraped off at trenches and added to the overriding plate (+; also counted as
    /// subducted before, so net neutral).
    pub accretion: f64,
    /// Crust of the lower plate in continental collisions, removed there (−) …
    pub collision_consumed: f64,
    /// … and stacked onto the overriding plate (+).
    pub collision_stacked: f64,
    /// Crust of the lower plate at diffuse convergent zones, removed there (−) …
    pub shortening_consumed: f64,
    /// … and stacked onto the upper plate by distributed shortening (+).
    pub shortening_stacked: f64,
    /// Crust removed from the overriding plate at trenches (−).
    pub subduction_erosion: f64,
    /// Difference between the crust added (arcs, accretion, collisions) and what fitted below
    /// the maximum thickness (delamination, −).
    pub additions_lost: f64,
    /// Change caused by lateral crustal flow (should be ~0; the thickness cap removes crust).
    pub crustal_flow: f64,
    /// Change caused by merging plates (hidden crust of the absorbed plate is lost).
    pub merges: f64,
    /// Change caused by splitting plates (should be 0).
    pub splits: f64,
    /// Hidden crust of plates removed because they no longer cover any cell (−).
    pub removed_plates: f64,
    /// Net change caused by erosion and sedimentation (should be ~0: erosion conserves mass).
    pub surface_processes: f64,
    /// Total change not accounted for by the terms above.
    pub residual: f64,
    /// Total mass at the last check.
    last_total: f64,
    /// Sum of the terms recorded since the last check.
    recorded_since_check: f64,
}

/// The terms of the ledger.
#[derive(Clone, Copy, Debug)]
pub(super) enum Term {
    NewSeaFloor,
    Arcs,
    SubductedOceanic,
    SubductedTransitional,
    SubductedSediment,
    Accretion,
    CollisionConsumed,
    CollisionStacked,
    ShorteningConsumed,
    ShorteningStacked,
    SubductionErosion,
    AdditionsLost,
    CrustalFlow,
    Merges,
    Splits,
    RemovedPlates,
    SurfaceProcesses,
}

impl MassLedger {
    pub(super) fn new(total: f64) -> Self {
        Self {
            last_total: total,
            ..Self::default()
        }
    }

    /// Records a mass change.
    pub(super) fn record(&mut self, term: Term, mass: f64) {
        let field = match term {
            Term::NewSeaFloor => &mut self.new_sea_floor,
            Term::Arcs => &mut self.arcs,
            Term::SubductedOceanic => &mut self.subducted_oceanic,
            Term::SubductedTransitional => &mut self.subducted_transitional,
            Term::SubductedSediment => &mut self.subducted_sediment,
            Term::Accretion => &mut self.accretion,
            Term::CollisionConsumed => &mut self.collision_consumed,
            Term::CollisionStacked => &mut self.collision_stacked,
            Term::ShorteningConsumed => &mut self.shortening_consumed,
            Term::ShorteningStacked => &mut self.shortening_stacked,
            Term::SubductionErosion => &mut self.subduction_erosion,
            Term::AdditionsLost => &mut self.additions_lost,
            Term::CrustalFlow => &mut self.crustal_flow,
            Term::Merges => &mut self.merges,
            Term::Splits => &mut self.splits,
            Term::RemovedPlates => &mut self.removed_plates,
            Term::SurfaceProcesses => &mut self.surface_processes,
        };
        *field += mass;
        self.recorded_since_check += mass;
    }

    /// Compares the actual total mass with the recorded changes; the difference goes into the
    /// residual.
    pub(super) fn check(&mut self, total: f64) {
        self.residual += total - self.last_total - self.recorded_since_check;
        self.last_total = total;
        self.recorded_since_check = 0.0;
    }

    /// A copy of the ledger, checked against the actual total mass `total`.
    pub(super) fn checked(&self, total: f64) -> Self {
        let mut ledger = self.clone();
        ledger.check(total);
        ledger
    }

    /// The terms with their names, for reporting.
    pub fn terms(&self) -> [(&'static str, f64); 19] {
        [
            ("new sea floor", self.new_sea_floor),
            ("arcs", self.arcs),
            ("subducted oceanic", self.subducted_oceanic),
            ("subducted transitional", self.subducted_transitional),
            ("subducted sediment", self.subducted_sediment),
            ("accretion", self.accretion),
            ("collision consumed", self.collision_consumed),
            ("collision stacked", self.collision_stacked),
            ("shortening consumed", self.shortening_consumed),
            ("shortening stacked", self.shortening_stacked),
            ("subduction erosion", self.subduction_erosion),
            ("additions lost", self.additions_lost),
            ("crustal flow", self.crustal_flow),
            ("merges", self.merges),
            ("splits", self.splits),
            ("removed plates", self.removed_plates),
            ("surface processes", self.surface_processes),
            ("residual", self.residual),
            ("total", self.last_total),
        ]
    }
}

/// Areal mass (kg/m²) of all crust samples of the given plates, including hidden ones.
pub(super) fn plates_mass(plates: &[Plate]) -> f64 {
    plates
        .iter()
        .flat_map(|plate| plate.crust.iter().flatten())
        .map(Crust::mass)
        .sum()
}
