# Plan: mass-conserving healing of resampling gaps (issue 1r, part of 3/5)

_2026-11-16. User decision: fix the gaps first, then the continental mass balance (issue 3);
collapse and rift stretching (`issue-1r-thickening.md`) afterwards only if still needed._

## Finding

A budget of continental crust (scratch instrumentation, seeds 0–1, 500 Myr, Gt/yr averaged
over the run): copies of continental crust made when healing resampling gaps +35.5 / +43.7 —
the largest single source, more than collision stacking onto continents (+16 / +18), arcs and
accretion on continents (+3), and larger than erosion (−19 / −21). Crust is created from
nothing in both healing paths of `resolve_ownership` (`boundaries.rs`): single-cell holes and
the layer-by-layer fill of resampling gaps copy the adjacent sample into an empty local vertex
of the plate. That pumps continental crust (and its thickness) at plate edges.

Experiment (gap cells show the adjacent sample instead, i.e. a shared sample as already done
for consumed slots; all crust types): mean continental thickness at 500 Myr 45.6–45.8 →
34.3–35.1 km, > 50 km 31–33% → 6–8% [Earth a few %], mean land 1.32–1.38 → 0.70–0.79 km
[0.84], ledger "gap filling" term gone. Side effects: continental area 40% → 27–30% in 500 Myr
(the copies had hidden the real continental mass loss through erosion and sediment
subduction — next: issue 3), plate speed 3.3–3.4 → 4.0–4.3 cm/yr, production 114–118 →
150–158 Gt/yr (less continental area and drag), runtime −15%.

## Change

- In both healing paths, when the plate's local vertex at the gap cell has no crust and was not
  consumed, the gap cell **shows the adjacent sample** (`new_owners = parent/template`,
  counted as `shared_cells`) instead of a copy. Where the local vertex holds a (hidden) sample,
  it is shown as before (no mass created). Spreading gaps (new sea floor) are unchanged.
- The ledger term `GapFilling` and `copied_cells`/`copied_volume` disappear (or stay as zero
  diagnostics); the docs of `GapStatistics`, `apply_surface_changes` (shared samples get the
  summed erosion of their cells) and the module docs are updated.
- Consequences to check: a plate's sample set no longer grows at its edges by resampling, so
  persistent gaps stay shared every substep (shown sample stretched over more cells); shared
  samples get summed erosion and additions per sample; statistics count visible cells
  (thickness distribution over cells, masses over samples).

## Verification

- Tests: the ledger closes with no gap-filling term; total crust mass does not change across a
  substep without subduction, spreading or additions (pure rigid rotation of plates); hole
  cells show a neighbouring sample; incremental vs full matching stay identical.
- Runs (seeds 0–3, 500 Myr): continental thickness distribution (p10/p50/p90, > 50 km), mean
  land, shared cell counts over time (no runaway sharing), continental mass and area, sea
  level, production, runtime.

## Plan review (independent agent, 2026-11-16): sound with changes — to adopt when implementing

Findings: 75% of the baseline's continental copies are at boundaries with another plate (the
pump that collisions then stack); sharing does not run away (duplicated cells 7.5–8.4% →
9–10.6%, max 5–6 cells per sample); but sharing everywhere leaves 220–450 permanently
**enclosed empty local vertices** (≈1% of cells, mostly under continents; from ridges never
sampled and cell-based merges), re-shared every substep, whose neighbour sample then gets the
summed erosion of 2+ cells for life. Variant "copy only enclosed holes, share the rest"
(`ENCL`, `agent-scratch/review-gaps/`): gap filling 92–94 → 2–5 Gt/yr, continental copies
35–44 → 3.3–3.6 Gt/yr, continental > 50 km 13–14%, mean 37–38 km. Interactions (consumption,
incremental matching, pieces, merges, additions) are fine.

Required:
1. Fill enclosed holes (empty, not consumed, all plate-frame neighbours with crust) with a real
   sample — mass-conserving for continental crust (fractions of the neighbours), oceanic as new
   sea floor (missed ridge sampling); share everywhere else (edge notches, boundary gaps). The
   hole test uses the plate's local frame.
2. The plan's baseline numbers predate 0a7d3b3 — rerun on current code (review: baseline mean
   land 1.56–1.90 km, continental 36–37%, sea level +620…+690 m at 500 Myr) and state that the
   continental area is still falling at 500 Myr with sharing (issue 3 loss, not steady).
3. Per-step diagnostics: shared cells per substep, duplicated-cell fraction and phantom
   continental volume, persistent enclosed empties, max cells per sample, continental area by
   cells and by distinct samples; say whether thickness statistics are over cells or samples.
4. Rename `copied_cells` (→ e.g. `revealed_cells`); remove `GapFilling` or book enclosed fills
   under it / `NewSeaFloor`.
5. Tests: transform and convergent pairs over N substeps conserve sample counts with bounded
   sharing; no crust created in a gap next to another plate unless it opens away; enclosed
   holes filled mass-conservingly, none persist after a merge; ledger closes; incremental vs
   full matching identical.
Optional: merges map samples instead of cells (removes the merge leak, −33…−40 Gt/yr baseline
— issue 3); split by cells assigns shared border samples by the matched cell; count persistent
continental shares at slow/oblique divergence (input for rift stretching); hole template =
nearest neighbour in the local frame.
