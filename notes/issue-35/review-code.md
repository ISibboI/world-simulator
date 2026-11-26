# Code review: issue #35 (cover export split, youngest-first tracer, cover budget)

Diff: `review-diff.patch` (branch `agent/issue-35`, commit 232f9ed). The cover tests pass
(`cargo test --release cover`: 13 passed).

## Summary

- Simulation state is not changed. Every replaced write gives the same float as before:
  `remove_sediment_to(x)` stores `x` exactly where the old code stored `x`
  (`crust.sediment - from_sediment`, `crust.sediment - mass / ρ`, `new_sediment.max(0.0)`), and
  `add_sediment` computes the same `total`. `start_cover_cohort` now uses
  `crust.is_continental_origin()`, which is the same predicate as the old
  `crust.basement().is_continental_origin()` (`is_continental_origin` =
  `basement().is_continental_origin_column()`). Nothing outside the diagnostics reads
  `Crust::cohort`; it is not in the state hash or the history codec.
- Tracer semantics are consistent at every write site I found: `add_sediment` (surface deposits
  with NONE, CrustAddition shortening, topology merge), Mixture, `scaled`, heal_gap and
  stretch_rift (lateral shares, so both fractions stay the same in donors), and the three
  partial-removal sites (surface, metamorphism, subduction erosion) now call
  `remove_sediment_to`. Whole-sample removals at trenches remove both tracers completely.
  `lifo >= mixed` holds per sample: mixing is linear, and on removal from the top
  `min(l·S, S') >= m·S'` when `l >= m`. So the youngest-first fraction is never below the
  well-mixed one. No removal site still treats it in proportion.
- Performance: no new full pass per step. There is one pass over cells per erosion step
  (`add_step`, cheap next to `erode`) and a `*crust` copy per touched sample. The full passes
  (`cover_masses`, `cover_cohort`) run only in `statistics()` and once when the cohort starts.

## Findings

### 1. HIGH: `start_cover_cohort` resets the cover's own budget and throws away its "other" term
`src/tectonics/cover.rs:189` (`start_cover_budget`), called from `start_cover_cohort` (cover.rs ~238).

`start_cover_budget` sets `cover_start[i] = now[i] - recorded[i]` for **all three** masses. For
index 0 (the cover) that makes `CoverTerm::Other[0]` zero at the moment the cohort starts. All
of the cover's "other tectonics" change from `Simulation::new` up to that point is lost.
Consequences:
- The cumulative `cover_budget.total()[0]` no longer equals `cover_masses[0] - masses at start`.
- In `main.rs` `log_cover_export`, the snapshot interval that contains the cohort start
  (default `--cover-cohort 500`, so the Other of 500 Myr) logs an "other tectonics" rate that is
  off by minus the whole Other accumulated so far. The printed terms then do not add up to the
  printed stock change.

A probe on a copy (icosphere 4, 8 plates, seed 5, 5 tectonic steps, then `start_cover_cohort`):
stock change −1.7919e6 Gt, budget total before the start −1.7919e6 (correct), after the start
−1.8744e6. Other dropped from +8.26e4 Gt to 4.7e-8.

The test `planet::tests::cover_export_and_budget_close` does not catch this. It starts the cohort
after the first step, when Other is still about 0. Starting it after a few steps fails the
assertion `masses[0] - start[0] == total[0]`.

Fix: in `start_cover_cohort` reset only the cohort indices, e.g.
`let (now, rec) = (self.cover_masses(), self.cover_budget.total()); for i in 1..3 { self.cover_start[i] = now[i] - rec[i]; }`.
Keep the full reset for `Simulation::new`. Add a test that starts the cohort after several
steps and checks `masses[0] - start[0] == total[0]`.

### 2. MEDIUM (docs/claims): the well-mixed tracer is not strictly a lower bound once burial metamorphism removes sediment
`src/tectonics/crust.rs` (the `Cohort` doc, ~lines 103–120) and the log text in `main.rs` `log_cover_cohort` ("well mixed, lower bound").

The lower-bound argument holds for removal from the top. Burial metamorphism physically converts
the *bottom* of the column, which is the cohort (the oldest sediment). An oldest-first loss
there takes the cohort *faster* than taking it in proportion. So on columns deep enough to
metamorphose, the well-mixed half-life can overestimate the true one. The youngest-first tracer
is still a valid upper bound, because it deliberately takes metamorphism from the top.

Fix: state the caveat in the doc and the log/notes. Better: make the well-mixed tracer take
metamorphism oldest-first, i.e. from the bottom, using `slice_cohort` with the bottom slice and
adjusting `mixed`. Or report how much of the cohort's loss metamorphism causes
(`Metamorphism[1]`) so the reader can judge.

### 3. LOW: closed depressions are classified at the end of the step
`src/planet.rs:689` (`result.water.depression[cell]`).

A depression that the step's own deposits fill (or a lake that spills) counts as "continental
land", although the sediment was trapped in a closed depression. `SurfaceWater` has
`depression_at_start` ("before its deposition, for analysis"), which fits a destination split
better. Suggest `result.water.depression_at_start[cell]`, or document the choice. The
`ocean[cell]` flag (end of step) is a similar, smaller effect, because sea level moves little
per step.

### 4. LOW (tests): the planet closure test is mostly true by construction
`src/planet.rs:1173` (`cover_export_and_budget_close`).

- `masses[0] - start[0] == total[0]` and `anywhere == initial + total[1+t]` hold by definition,
  because Other is the remainder. They only catch resets of `cover_start`, and even that one is
  missed here (finding 1) because the cohort starts after step 1.
- `sum(cohort_to) == -Surface[cohort]` is exact by construction (`add_cohort` splits exactly the
  delta).
- The non-trivial parts are `sum(to) == denudation` (mass conservation of `erode` in kg) and the
  `tectonics` test `cover_budget_records_surface_changes_and_metamorphism`, which checks recorded
  terms against the actual mass change for `apply_surface_changes`.
- The trench sites (Collision, Accreted, Subducted, SubductionErosion) get only a sign
  (monotonicity) check. Nothing checks that they explain the change they cause.

Suggestions:
- Start the cohort after several steps (catches finding 1).
- Assert `lifo >= mixed` with erosion enabled. In my probe without erosion both tracers were
  equal to 7 digits, so the bound is only tested where surface removal happens.
- Optionally check that `Other` for a single `update_boundaries` call equals the change minus
  the recorded sinks, with transport switched off, or at least that the cover's Other stays
  small relative to the recorded sinks.

### 5. LOW (perf, statistics only)
`src/tectonics/cover.rs` `cover_masses` collects into a `Vec` before summing (an allocation for no
reason). `PlanetStatistics` calls `cover_budget()` and `cover_masses_gt()`, two full passes that
compute the same `cover_masses()`, plus `cover_cohort()` (a third pass). This is fine at snapshot
frequency. One pass would do if `cover_budget` took the masses as an argument.

### 6. INFO: export vs. budget use different areas and granularity
The export uses the erosion's requested per-cell deltas, `Planet::cell_area` (the `planet_radius`
passed to `Planet::new`) and the surface sample's class at the start of the step. The cohort
amounts use the applied per-sample changes and `Simulation::cell_area`
(`params.planet_radius`). They agree when the two radii match, as in `main`. The export split
only uses `to` as weights for the cohort, so the closure is unaffected. If the radii could
differ, it is worth a note.

## Checked and found correct
- Signs: `record_removal` books negative amounts. `record_change` books after minus before, and
  `Other = now - start - recorded`. `since()` differences are right. In `cohort_fate`, the
  removed terms are negated, and the fate sums to `initial` because `anywhere = initial + Σterms`
  and `Σcohort_to = -Surface`.
- Accreted/Subducted split: the décollement takes the top `[0, accreted_sediment]` and the rest
  is subducted. This matches the trench_sediment books. Collision removes the whole column,
  which goes into metasediment crust. Diffuse shortening moves sediment through a CrustAddition
  and correctly ends up in Other.
- Units: sediment rock-equivalent m × `SEDIMENT_ROCK_DENSITY`, and bedrock Δ ×
  `bedrock_density[cell]`, the same density array that `erode` received (now computed once and
  reused). Gt = kg/m² × area / 1e12.
- `add_step` is a correct partition. Continental cells add gross loss to denudation and gross
  gain to destinations 0–2 by ocean, then depression, then land. Other cells add their net
  change to destination 3. Σto = Σ net change of continental cells + denudation = denudation
  when `erode` conserves mass.
- Logging indices: `cover_masses[0]` for the stock, `term()[1 + tracer]` for the cohort, and
  destination names in the order of `cohort_to`. `left.on_continents`/`anywhere` per tracer are
  right.
- The cohort start snapshot (`cover_budget()`, `cover_export`) is taken after the reset, so the
  cohort's own budget since the start is consistent (indices 1, 2). Only index 0 is affected by
  finding 1.
