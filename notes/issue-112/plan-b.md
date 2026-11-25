# Plan: option (b), the drainage sees each crust sample once (#112)

_2026-11-25. On top of option (a) (one-to-one resampling within 30 m, PR #165). Requested by
Sebastian on [#165](https://github.com/ISibboI/world-simulator/pull/165#issuecomment-7169052117).
Goal: an Earth-like planet, not Earth; effects of the ~110 km grid are accepted (#64)._

## The artefact (recap)

After (a), 11% of cells still show a crust sample that another cell also shows (twins). A cell's
surface is `to_surface(crust.dry_elevation + dynamic_topography[cell] − sea_level)`, so twins
differ only by the dynamic topography of their cells (typically metres to a few tens of metres
per cell spacing). Erosion (`erosion::erode`) works on cells; `apply_surface_changes` gives the
sample the **mean** of its cells' changes. So when the drainage sees the lower twin as a pit
(lake, endorheic sink), the sediment it deposits there is averaged with its twin's change: both
cells rise by the same amount and the pit never fills. Twins are 48% of the remaining
single-cell land pits after (a).

## Definition of (b)

**The surface processes see every crust sample at one elevation: all cells showing one sample
get the surface elevation of the sample at the mean dynamic topography of those cells.**

`drainage_elevation[cell] = to_surface(crust.dry_elevation(t) + mean_{c ∈ cells(s)} dyn[c] − sea_level)`
for the sample `s` that `cell` shows. For a sample shown at one cell this is the cell's own
surface elevation, bit for bit. Only `erode`'s `elevation` input changes; the climate, the
view, the snapshots and the cell dump keep the per-cell surface.

Why the mean in dry space: the twins are one column; its surface differs between the cells only
by the dynamic topography under it. Averaging the dry elevation keeps the water loading of
submerged cells exact (averaging surface elevations across the coast would mix loaded and
unloaded values).

Consequences for each part of `erode`:
- **Routing / priority flood.** Twins are a flat at one level. The flood resolves flats with its
  ε rise (1 mm per cell), so the lower twin is no longer a depression of its own: a pair of twins
  is a pit only if the sample's mean surface lies below all the pair's other neighbours, i.e. a
  real depression of the column. That is "a pit and its twin are one cell".
- **Lakes and the lake balance.** A real depression of a shared sample is a lake of k cells with
  one floor level; evaporation counts all its cells (its area is k cells: the sample is shown
  there). `filled − elevation ≤ LAKE_MIN_DEPTH` handles the ε flats.
- **Incision.** No incision between twins (equal elevation = flat); incision towards other
  neighbours uses the sample's mean surface.
- **Sediment.** `fill_lake` raises all cells of a merged lake to one level, i.e. every twin gets
  the same deposit, so the sample's mean change equals each cell's change: the lake can fill and
  stays filled (today the low twin's deposit is halved by averaging). Diffusion and waves see no
  gradient between twins (today they move sediment between twins that the averaging undoes).
- **Mass.** `erode` conserves mass on cells and `apply_surface_changes` conserves it on samples,
  both unchanged. The input elevation only changes where the processes act, not what is
  conserved. The sea-level solves inside `erode` (fluvial and coastal sea-level change) run on the
  sample surface; the difference to the solve on the samples is booked as
  `SeaLevelBudget::sharing`, as before.
- **Ocean.** `ocean_from` runs on the sample surface from the seeds of the per-cell surface; a
  twin pair across the coast becomes either both sea or both land (the sample decides). Rare
  (twins straddling the coast) and consistent.
- **Outputs.** `SurfaceWater` (receivers, depressions, lake depth) and `Hydrology` describe the
  sample surface. The lake depth of a twin is relative to the sample surface: drawn on the
  per-cell surface, a lake on twins sits up to half the twin offset (a few metres) off its level.
  Not visible at the view's exaggeration; documented.

## Interaction with the lake work stacked above (#166 → #170)

(b) touches only what `Planet::step` passes as `elevation` to `erode` (plus diagnostics), not
`erosion.rs`'s algorithms, so the merges into #166–#170 should be mechanical:
- #166 passes `previous_lakes` (last step's lake depth > 0) for onset hysteresis: it is on the same
  (sample) surface in both steps, consistent.
- #127 computes a load correction for the view from lake depths and the elevations given to
  `erode`; with (b) those are the sample surface. The correction is per cell from the lake
  depth, so it stays meaningful; check at the merge.
- #126 returns lake evaporation to the climate per cell: lakes on twins cover all twins, as above.
- #128 draws lakes where ≥ 1 m of water stands: on the per-cell surface (see Outputs).

## Implementation

1. `Simulation::sample_surface_elevations()` (tectonics/mod.rs): one pass summing the dynamic
   topography per shown sample (dense index `sample_index`, as `apply_surface_changes`), then the
   surface of every cell from its sample's mean. Samples at one cell take the per-cell path, so
   the result is bit-identical there.
2. `ErosionParams::sample_surface` (default on; `--set erosion.sample_surface=0` restores the
   per-cell surface bit for bit). `Planet::step` passes the sample surface to `erode` when on.
3. Diagnostics: the snapshot log's hydrology line also reports endorheic land draining into
   **single-sample** depressions (all cells of the terminal depression show one sample), computed
   from `SurfaceWater` at snapshot time (with (b) a twin lake is a 2-cell lake, so the existing
   "single-cell pits" count changes by definition). Cell dump: columns `sample` (plate-local
   index) and `drain` (the sample surface).
4. Tests: (i) twins of one sample get one elevation, single-shown samples their per-cell
   surface exactly; (ii) an erosion test on a small mesh where a pit exists only by a twin
   offset: with the merged surface it is no lake and its water reaches the ocean; with a real
   depression of the pair it is one 2-cell lake that fills evenly.

## Evaluation

8 seeds × 1000 Myr through `runs`, same commit, `--set erosion.sample_surface=0` (base = branch
head behaviour, checked bit-identical by state hash on a short run) vs default, both
`--duration 1000 --snapshot-interval 50 --width 512 --cell-dump --lake-log-from 900`. Window
500–1000 Myr. Metrics as in (a) (`scripts/pits.py`, `scripts/compare.py`) plus, on the drainage
surface: pits (cells of a sample > 1 m below all neighbours of other samples), twins' share of
pits, endorheic into single-cell and single-sample depressions, lake area and persistence,
hypsometry, sea-level budget `sharing`. Runtime: paired 200 Myr runs at 6 subdivisions.

**Expectation.** Twin pits vanish from the drainage (48% of the remaining pits by the twin
criterion); lakes on single-cell pits drop; endorheic drainage into single-cell pits may not
drop much (the water moves to the next closed depression, as with (a); #172). If (b) harms (e.g.
more endorheic land, worse hypsometry), it stays behind the switch, default off, and the PR says
so.

## Review (independent agent, 2026-11-25) and revisions

Verdict: **sound with changes**. Points and what was done:

1. **Twins drift apart within the step.** Incision and routing change each cell separately, so
   after the step's first stage the twins' working elevations differ again; diffusion, waves and
   the delta fills see small gradients between them (harmless here: the transfers cancel in
   `apply_surface_changes`). **On #160** (stacked above) the lakes are recomputed at the end of the
   step on `seabed.elevation`, and that state feeds the lake water, the view and the next step's
   onset hysteresis (`previous_lakes`): twin pits would come back there. → **Required at the
   merge into #166:** average the end-of-step elevations per sample (dry space) before the
   end-of-step water state. On #165 nothing reads the in-step elevations after the step.
2. **`coastal` sea-level term carried the sea level before the step** (zero only when the
   erosion sees the same surface as the tectonics' solve), which the `sharing` term then absorbed.
   → Fixed: `coastal` is now relative to the sea level before the step. Budget digits only, no
   state change.
3. **Ocean seeds come from the per-cell surface;** a seed or strait on a twin pair could stop
   being ocean on the sample surface. Rare, water-conserving. → Measured: the snapshot log reports
   the cells whose ocean differs between the two surfaces (`ocean differs … at N cells`).
4. **Coast-straddling twins:** the sample decides whether both are land or sea; the climate's
   per-cell runoff of a twin that is sea for the climate but land for the erosion is ~0. Negligible.
5. **Samples shown at ≥ 3 or non-adjacent cells:** the mean is right for one column, but it can
   make a pit where the dynamic topography has a gradient. → The analysis splits drainage-surface
   pits by k and adjacency (cell dump: `sample`, `drain`).
6. **The visible relief does not change:** the view, snapshots and the per-cell pit scripts keep
   the per-cell surface, where the lower twin is still a few-metre pit, just no longer a lake or
   sink. (b) fixes the hydrology, not the relief; the results and the PR say so.

"Single-sample depression": all cells of the depression show one sample (the sample may also be
shown at cells outside it). Added metrics: lake persistence on twin pairs, lake volume, the
ocean mismatch, `sharing` before/after. Switch bit-identity: checked by state hash on a short run.
