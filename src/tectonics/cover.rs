//! Diagnostics of the continental sedimentary cover (#34, #35): the budget of the cover and of
//! the cover cohort's tracers by process. Nothing in the simulation reads them.
//!
//! The budget follows three masses (rock-equivalent metres of sediment summed over all crust
//! samples, hidden ones included; times the cell area and
//! [`SEDIMENT_ROCK_DENSITY`] for kilograms), see [`CoverMasses`]:
//! 0. the **cover**: all sediment on crust whose basement is of continental origin;
//! 1. the cohort under the well-mixed tracer ([`Cohort::mixed`]), anywhere;
//! 2. the cohort under the youngest-first tracer ([`Cohort::lifo`]), anywhere.
//!
//! The processes that remove sediment from a column or change it at the surface record the
//! change of the three masses they caused, sample by sample, where they happen (no extra passes
//! over the crust per step). Everything else (transport and deformation, distributed shortening,
//! gap healing, crustal flow, plate events, columns changing their class) is the rest of the
//! change since the start ([`CoverTerm::Other`]), so the terms sum to the change exactly.

use super::*;

/// `[cover, cohort (well mixed), cohort (youngest first)]` (see the module documentation).
pub type CoverMasses = [f64; 3];

/// The processes that change the cover and the cohort.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverTerm {
    /// Erosion and deposition (net per sample and step), including columns whose bedrock
    /// erosion changes their class.
    Surface,
    /// Burial metamorphism (sediment turned into metasediment crust).
    Metamorphism,
    /// Sediment subducted with the lower plate at trenches (below the décollement).
    Subducted,
    /// Sediment scraped off at accretionary trenches into wedges (metasediment crust).
    Accreted,
    /// Sediment of buoyant crust stacked into orogens in collisions and terrane accretion
    /// (crust).
    Collision,
    /// Sediment of the overriding plate removed by subduction erosion (and columns whose
    /// thinning by it changes their class).
    SubductionErosion,
    /// The rest: transport and deformation of the plates, distributed shortening (the sediment
    /// moves onto another column), gap healing, crustal flow, plate events (merges lose hidden
    /// crust), columns changing their class.
    Other,
}

impl CoverTerm {
    pub const ALL: [CoverTerm; 7] = [
        CoverTerm::Surface,
        CoverTerm::Metamorphism,
        CoverTerm::Subducted,
        CoverTerm::Accreted,
        CoverTerm::Collision,
        CoverTerm::SubductionErosion,
        CoverTerm::Other,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CoverTerm::Surface => "surface",
            CoverTerm::Metamorphism => "metamorphism",
            CoverTerm::Subducted => "subducted",
            CoverTerm::Accreted => "accreted",
            CoverTerm::Collision => "collision",
            CoverTerm::SubductionErosion => "subduction erosion",
            CoverTerm::Other => "other tectonics",
        }
    }
}

/// Changes of the [`CoverMasses`] per [`CoverTerm`] so far (negative: removed).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CoverBudget {
    terms: [CoverMasses; 7],
}

impl CoverBudget {
    pub fn term(&self, term: CoverTerm) -> CoverMasses {
        self.terms[term as usize]
    }

    /// The sum of all terms.
    pub fn total(&self) -> CoverMasses {
        sum(self.terms.iter())
    }

    /// The budget with every mass multiplied by `factor` (e.g. to Gt).
    pub fn scaled(&self, factor: f64) -> Self {
        Self {
            terms: self.terms.map(|masses| masses.map(|mass| mass * factor)),
        }
    }

    /// The change of every term from `earlier` to this budget.
    pub fn since(&self, earlier: &Self) -> Self {
        let mut terms = self.terms;
        for (term, before) in terms.iter_mut().zip(&earlier.terms) {
            for (mass, before) in term.iter_mut().zip(before) {
                *mass -= before;
            }
        }
        Self { terms }
    }

    fn add(&mut self, term: CoverTerm, change: CoverMasses) {
        for (mass, change) in self.terms[term as usize].iter_mut().zip(change) {
            *mass += change;
        }
    }

    /// Records the change of a column from `before` to `after` by `term`.
    pub(super) fn record_change(&mut self, term: CoverTerm, before: &Crust, after: &Crust) {
        let [before, after] = [before, after].map(column_masses);
        self.add(
            term,
            [
                after[0] - before[0],
                after[1] - before[1],
                after[2] - before[2],
            ],
        );
    }

    /// Records the removal of the part of the sediment of `crust` (as it was before the removal)
    /// between `from` and `to` metres below the top of its column (rock-equivalent) by `term`.
    pub(super) fn record_removal(&mut self, term: CoverTerm, crust: &Crust, from: f64, to: f64) {
        let cohort = slice_cohort(crust, from, to);
        let cover = if crust.is_continental_origin() {
            to - from
        } else {
            0.0
        };
        self.add(term, [-cover, -cohort[0], -cohort[1]]);
    }
}

/// The [`CoverMasses`] of one column.
fn column_masses(crust: &Crust) -> CoverMasses {
    let [mixed, lifo] = crust.cohort.amounts(crust.sediment);
    let cover = if crust.is_continental_origin() {
        crust.sediment
    } else {
        0.0
    };
    [cover, mixed, lifo]
}

fn sum<'a>(masses: impl Iterator<Item = &'a CoverMasses>) -> CoverMasses {
    masses.fold([0.0; 3], |sum, masses| {
        [sum[0] + masses[0], sum[1] + masses[1], sum[2] + masses[2]]
    })
}

/// The cohort amounts (rock-equivalent m, `[mixed, lifo]`) in the part of the sediment of
/// `crust` between `from` and `to` metres below its top: the well-mixed cohort in proportion,
/// the youngest-first cohort where the part reaches into the bottom of the column, which it
/// fills (see [`Cohort`]).
pub(super) fn slice_cohort(crust: &Crust, from: f64, to: f64) -> [f64; 2] {
    let sediment = crust.sediment.max(0.0);
    let (from, to) = (from.clamp(0.0, sediment), to.clamp(0.0, sediment));
    let [mixed, lifo] = crust.cohort.amounts(sediment);
    // The youngest-first cohort is the bottom `lifo` metres of the column.
    let overlap = (to - (sediment - lifo).max(from)).max(0.0);
    [mixed * (to - from) / sediment.max(1e-300), overlap]
}

/// The masses of the cover cohort (Gt) per tracer (`[mixed, lifo]`).
#[derive(Clone, Copy, Debug, Default)]
pub struct CohortMasses {
    /// Still sediment on crust whose basement is of continental origin.
    pub on_continents: [f64; 2],
    /// Still sediment anywhere.
    pub anywhere: [f64; 2],
}

impl Simulation {
    /// The [`CoverMasses`] now (rock-equivalent m summed over all samples).
    pub(super) fn cover_masses(&self) -> CoverMasses {
        let mut masses = [0.0; 3];
        for crust in self.plates.iter().flat_map(|p| p.crust.iter().flatten()) {
            for (sum, mass) in masses.iter_mut().zip(column_masses(crust)) {
                *sum += mass;
            }
        }
        masses
    }

    /// Sets the masses the budget starts from to the current ones, less what was recorded so
    /// far: all three (`masses` 0..3) at the start of the simulation, only the cohort's
    /// (1..3) when the cohort is tagged.
    pub(super) fn start_cover_budget(&mut self, masses: std::ops::Range<usize>) {
        let (now, recorded) = (self.cover_masses(), self.cover_budget.total());
        for i in masses {
            self.cover_start[i] = now[i] - recorded[i];
        }
    }

    /// Factor from rock-equivalent metres of sediment summed over samples to Gt.
    fn sediment_to_gt(&self) -> f64 {
        self.cell_area() * SEDIMENT_ROCK_DENSITY / 1e12
    }

    /// The budget of the cover and of the cohort so far, in Gt (see [`CoverBudget`]), with
    /// [`CoverTerm::Other`] as the rest of the change since the start. Sums over all samples.
    pub fn cover_budget(&self) -> CoverBudget {
        let mut budget = self.cover_budget;
        let (now, recorded) = (self.cover_masses(), budget.total());
        budget.add(
            CoverTerm::Other,
            [0, 1, 2].map(|i| now[i] - self.cover_start[i] - recorded[i]),
        );
        budget.scaled(self.sediment_to_gt())
    }

    /// The cohort removed by surface processes so far, in Gt per tracer (`[mixed, lifo]`;
    /// cheap, unlike [`Simulation::cover_budget`]).
    pub fn cover_cohort_eroded(&self) -> [f64; 2] {
        let surface = self.cover_budget.term(CoverTerm::Surface);
        [-surface[1], -surface[2]].map(|mass| mass * self.sediment_to_gt())
    }

    /// The cover masses now in Gt (see [`CoverMasses`]).
    pub fn cover_masses_gt(&self) -> CoverMasses {
        self.cover_masses().map(|mass| mass * self.sediment_to_gt())
    }

    /// Starts tracking the continental sedimentary cover as a cohort (see [`Crust::cohort`]):
    /// all sediment now on crust whose basement is of continental origin belongs to it, nothing
    /// else does. Returns the cohort's mass in Gt.
    pub fn start_cover_cohort(&mut self) -> f64 {
        for crust in self
            .plates
            .iter_mut()
            .flat_map(|plate| plate.crust.iter_mut().flatten())
        {
            // The carbonate stays (it is not a cohort tracer).
            crust.cohort = Cohort {
                carbonate: crust.cohort.carbonate,
                ..if crust.is_continental_origin() {
                    Cohort::ALL
                } else {
                    Cohort::NONE
                }
            };
        }
        self.start_cover_budget(1..3);
        self.cover_cohort().on_continents[0]
    }

    /// Masses (Gt, all samples including hidden ones) of the cover cohort (see
    /// [`Simulation::start_cover_cohort`]) still on crust whose basement is of continental
    /// origin, and still sediment anywhere, per tracer.
    pub fn cover_cohort(&self) -> CohortMasses {
        let to_gt = self.sediment_to_gt();
        let mut masses = CohortMasses::default();
        for crust in self.plates.iter().flat_map(|p| p.crust.iter().flatten()) {
            let amounts = crust.cohort.amounts(crust.sediment);
            let continental = crust.is_continental_origin();
            for (tracer, amount) in amounts.into_iter().enumerate() {
                if continental {
                    masses.on_continents[tracer] += amount * to_gt;
                }
                masses.anywhere[tracer] += amount * to_gt;
            }
        }
        masses
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(sediment: f64, mixed: f64, lifo: f64) -> Crust {
        Crust {
            sediment,
            cohort: Cohort {
                mixed,
                lifo,
                carbonate: 0.0,
            },
            ..super::super::crust::new_oceanic_crust(0.0)
        }
    }

    #[test]
    fn slices_take_the_youngest_first_cohort_from_the_bottom() {
        // 1000 m of sediment, the bottom 400 m of it the cohort (youngest first), 0.4 mixed.
        let crust = column(1000.0, 0.4, 0.4);
        let close = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).abs() + (a[1] - b[1]).abs() < 1e-9;
        assert!(close(slice_cohort(&crust, 0.0, 600.0), [240.0, 0.0]));
        assert!(close(slice_cohort(&crust, 0.0, 700.0), [280.0, 100.0]));
        assert!(close(slice_cohort(&crust, 700.0, 1000.0), [120.0, 300.0]));
        assert!(close(slice_cohort(&crust, 0.0, 2000.0), [400.0, 400.0]));
    }

    #[test]
    fn removal_from_the_top_spares_the_youngest_first_cohort() {
        let mut crust = column(1000.0, 0.4, 0.4);
        // New deposits go on top: neither tracer counts them.
        crust.add_sediment(1000.0, Cohort::NONE);
        assert!((crust.cohort.mixed - 0.2).abs() < 1e-12);
        assert!((crust.cohort.lifo - 0.2).abs() < 1e-12);
        // Eroding the new deposits again takes the well-mixed cohort in proportion, but none of
        // the youngest-first one.
        let removed = crust.remove_sediment_to(1000.0);
        assert!((removed[0] - 200.0).abs() < 1e-9 && removed[1].abs() < 1e-9);
        assert!((crust.cohort.amounts(crust.sediment)[1] - 400.0).abs() < 1e-9);
        // Below the cohort's top it goes too.
        let removed = crust.remove_sediment_to(100.0);
        assert!((removed[1] - 300.0).abs() < 1e-9);
        assert!((crust.cohort.lifo - 1.0).abs() < 1e-12);
        crust.remove_sediment_to(0.0);
        assert_eq!(crust.cohort.amounts(crust.sediment), [0.0, 0.0]);
    }
}
