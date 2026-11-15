# Plan: crust mass conservation (issue 3)

_2026-11-15. Issue 3 in `../ISSUES.md`: continental crust is not conserved when erosion is on
(−36% / −60% over 500 Myr in two seeds), from several bookkeeping leaks. The research
(`../research/feasibility-2026-11-15.md`) recommends conserving mass, not volume, and a global
mass ledger with asserts; target: arc addition ≈ subduction erosion + sediment subduction
≈ 3 km³/yr, continental volume ~constant._

## 1. A crust mass ledger (first, to measure)

Track, per step, the total crust mass (crystalline crust `thickness · density` plus sediment
`sediment · ρs`, per cell area, over all samples of all plates, including hidden ones) and
every flux that changes it, in `Simulation` (tectonics) and the erosion result:

| Source (+) | Sink (−) | Neutral (moves mass) |
|---|---|---|
| new oceanic crust at ridges | subducted oceanic crust and its sediment | collision stacking |
| arc crust | subduction erosion | crustal flow |
| gap copies (resampling) | delamination above the thickness cap | erosion → sediment → deposition |
| | hidden samples dropped in merges | plate split |
| | sediment lost beyond the shelf | |

Log the ledger per snapshot (km³/yr per term, continental crust volume and mass) and check in
a debug assertion / test that `Δmass = Σ sources − Σ sinks` within round-off per step. The
ledger shows which leaks matter before fixing them.

## 2. Fixes

1. **Erosion conserves mass:** eroded bedrock of density ρb becomes `from_bedrock · ρb / ρs`
   of sediment (`erosion.rs`), so mass, not volume, is conserved.
2. **Shared samples get the mean change:** `apply_surface_changes` currently applies the change
   of the first cell showing a sample and drops the others. A sample shown at several cells
   represents one cell's worth of crust; apply the **mean** of their changes (summing would
   erode it twice).
3. **No deleted sediment:** sediment that does not fit on the shelf within reach is spread as a
   deep-sea fan over the ocean cells reached by continuing the breadth-first search (up to a
   larger limit, e.g. 2048 cells), thinly, instead of being deleted. Only if no ocean is
   reachable at all is it recorded as lost.
4. **Sediment accretion at trenches:** when a sample subducts, a fraction (default 50%, Earth
   ~50–70% of incoming sediment is accreted or recycled into the arc; Scholl & von Huene 2009)
   of its sediment is added to the overriding plate at the trench cell (accretionary wedge, as
   sediment); the rest subducts (recorded).
5. **Thickness cap:** keep the spill-over; whatever still does not fit is recorded as
   delamination (a physical sink; issue 4 will add eclogite densification).
6. **Gap copies** (issue 5) are recorded as a source; their size decides whether a further fix
   is needed.

## 3. Verification

- Unit tests: erosion of an island conserves mass (eroded mass = deposited + delivered); the
  ledger closes over a few hundred steps of a full simulation; shared samples get the mean
  change; deep-sea fan instead of deletion; sediment accretion moves the right amount.
- Runs (seed 0 at subdivisions 6, 500 Myr): ledger terms over time vs Earth (arcs ~2.5–3
  km³/yr, subduction erosion ~1.3–1.4, sediment subduction ~1–1.6, sediment flux ~6–8 km³/yr);
  continental crust volume and area trend; compare with the run before the fix.

## Not in scope

Changing the arc / erosion balance parameters (calibration after the ledger shows the terms),
eclogite densification and thermal age (issue 4), continental margins (issue 7).

## Plan review (independent agent, 2026-11-15): sound with changes

Required changes, adopted:

1. **Shared samples:** erosion runs per cell, so a sample shown at k cells produces k cells'
   worth of sediment; applying the mean (or the first change) still creates (k−1)/k of it.
   Apply the **sum** of the changes (roughly as many samples are hidden as are shown twice, so
   on average this is right), taking any excess sediment removal from bedrock by mass, or book
   the difference as an explicit resampling term.
2. **Complete sink list:** `remove_dead_plates` drops plates with hidden samples; merges
   overwrite hidden crust of `into`, collide absorbed samples, stack sediment at crust density,
   and hit the cap; the thickness cap also acts in crustal flow (and its `max(0)` clamp) and in
   `apply_additions` (leftover after spilling, whole addition when `total_weight <= 0`);
   subduction erosion removes less than nominal (clamp at 7 km, sediment first); subducted crust
   includes transitional and sometimes continental crust (book by class, crust and sediment
   separately); gap/hole filling can reveal hidden samples (no mass created although counted),
   the interior "consumed → new ocean" branch overwrites, the degenerate restart is a source.
   **Rule: book every term as the actual Δ(Σ ρ·h) of the samples it touches, and log a residual.**
3. **One sediment density convention** for rock (grain) mass everywhere (erosion conversion,
   collision stacking, merges, accretion, ledger); 2300 kg/m³ as bulk density means ~2050 kg/m³
   of rock if pores are water-filled — state the choice.
4. **Accretion vs. subduction erosion vs. arcs:** subduction erosion runs after consumption and
   removes sediment first, so accreted sediment would be eroded again → define a net rule or
   accrete after erosion. The arc term already contains sediment recycled through arc magma, so
   accrete only the **frontally accreted** fraction (globally well below 50%: erosive margins
   accrete almost nothing — check Clift & Vannucchi 2004, Scholl & von Huene 2007/2009). Spread
   it with the arc profile or convert to crust of ~2600–2700 kg/m³ at constant mass, not as a
   sediment pile at one cell (issue 7).
5. **Where fluxes are measured:** tectonic terms summed over substeps into a per-step ledger in
   `Simulation`; erosion terms measured in `apply_surface_changes` as actual changes;
   `ErosionResult` reports its own budget (eroded, deposited, fan, lost). Closure: Δ(sample mass
   incl. hidden) = Σ tectonic terms + Σ applied erosion terms.

Technical notes: cost ~1 ms per step; tolerance ~1e-9 relative; accumulate in index order
(not `HashMap` order) for determinism; report hidden-sample mass (phantom crust riding along
on subducting plates) separately.

Verification additions: per-operation closure tests (split, merge late in a run with hidden
samples, remove, crustal flow near the cap, additions with spill/delamination, gap/hole
filling, shared-sample erosion); erosion test with fan overflow; accretion + subduction erosion
on one contact; ledger closure `debug_assert` each step in existing simulation tests; runs with
erosion on and off; terms in Gt/yr and km³/yr; mass by class (continental, transitional,
oceanic, sediment) and felsic mass (density < 2850), since `is_continental` volume can fall
while mass is conserved.

Deferred: lithification of buried sediment, dissolved load, eclogite (issue 4), calibration.
Note: mass conversion gives 20–34% more sediment volume (slightly worse shelves, issue 7).
