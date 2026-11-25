# Plan: lakes as a moisture source (#126)

_2026-11-25. Stacked on #167 (agent/issue-127, 50a0b5a). Goal: an Earth-like planet, not
Earth; grid effects (~110 km cells) accepted (#64)._

## Validity on #167

Still valid. `planet::climate_elevations` raises every non-ocean cell to ≥ 0 m, so lake cells
are land to `Climate`: their evaporation is the Budyko evapotranspiration of the land (≤ P),
nothing more. The lake water balance (`erosion::water_state` → `lake_balance`) removes the
lake's *excess* evaporation, `cover · (PET − ET)` per cell, from the runoff (`Hydrology::
lake_evaporation`, ~3 × 10³ km³/yr in #127's runs, 1.8% of land under lakes). That water
leaves the land and never reaches the atmosphere: the coupled water cycle loses it
(~0.3% of global evaporation, but concentrated on the lakes and their lee).

## Change

1. **Per-cell lake evaporation from the balance** (`SurfaceWater::lake_evaporation`, m/yr per
   cell, cell-averaged): for every lake member, `clamp(covered − i, 0, 1) · excess(c) /
   cell_area`, computed in `water_state` exactly as the balance sums it, so the per-cell field
   sums to `Hydrology::lake_evaporation` (endorheic: = inflow; spilling: Σ excess). Taken from
   the end-of-step water state (#160), stored in `Planet::water` like `lake_depth`.
2. **Climate source, one step lag**: `Climate::update` takes `lake_evaporation: &[f64]`
   (m/yr). In each solved month, cells that are land for the climate get
   `E_lake(c) · w_m(c)` added to their evaporation (after the Budyko ET), with the monthly
   weights `w_m = PET_lake,m / mean_m(PET_lake,m)` (as the ET is spread by the land PET), so
   the annual mean of the source equals the balance's lake evaporation. Cells that became
   ocean since (lag) drop their lake source (logged as a mismatch) — *revised, see below*. The moisture budget then
   carries it downwind like ocean evaporation (fetch, rain-out, lake effect).
3. **Open-water potential evaporation with ice** (`WaterCycle::lake_potential_evaporation`):
   per month `PET_lake,m = PET_m · ice_free(T_m)`, with the land PET (bulk formula at the
   surface temperature, the same as today's lake rate) and `ice_free` ramping linearly from 0
   at −2 °C to 1 at +2 °C monthly mean temperature (lake ice: little evaporation, sublimation
   neglected). The lake balance uses the annual mean of this instead of the land PET, so frozen
   months no longer evaporate lake water. The lake surface temperature is the land
   temperature of the cell (no lake thermal inertia; grid-scale).
4. **Diagnostics**: snapshot log adds "lake evaporation into the air" (climate side, km³/yr)
   next to the balance's lake evaporation (closure check); cell dump adds `lake_evap`
   (m/yr). Unit tests: per-cell lake evaporation sums to the balance; the climate with a lake
   source conserves water (ΣP = ΣE incl. lake) and raises precipitation downwind of a lake
   (westerlies) vs without; ice-free factor zero in frozen months.

Water conservation: the land loses `lake_evaporation` from its runoff (balance, step n); the
atmosphere gains the same amount as evaporation (climate, step n+1), and the climate's
budget closes ΣP = ΣE per month. Everything else unchanged (ocean evaporation, ET, runoff).

Determinism: no new transcendental calls (Magnus/exp already through `math`), no hash-map
iteration; order of sums fixed.

## Feedback

Lake evaporation partly rains back on the lake's own catchment → more runoff → larger lake.
The recycled share is < 1 (wind carries most moisture out of a ~110 km cell), so the
response is a bounded geometric series, not a runaway; check lake area/evaporation per step
for growth over time and the largest lakes in the runs.

## Evaluation

8 seeds × 1000 Myr, `--duration 1000 --snapshot-interval 50 --width 512 --cell-dump
--lake-log-from 500`, window 500–1000 Myr (as #160/#127):
- **before** = agent/issue-127 tip (50a0b5a; its notes-only commit after cb3a0b9, rerun so all
  sets use the same runner mix), **after** = this branch.
- **0.2% rule** (Sebastian on #126): both with `--set min_sea_fraction=1` (as #58's `b2`):
  closed ocean remnants become lakes. Compare endorheic share, aridity and the fate of the
  remnants (lake vs desert) with the rule on/off, before/after.
Metrics: lake area and persistence (lakes.py), endorheic share, precipitation within 1–3 cells
of lakes and downwind, global and land precipitation, P − E closure (climate lake source vs
balance), arid land, Köppen shares, sea level, runtime (paired local runs seed 0, 200 Myr at
6 subdivisions, before/after).

## Plan review (independent agent) and revision

Verdict: sound; validity analysis correct; per-cell field sums exactly to the balance (checked
the `fraction` and `loss == 0` branches); taken from the end-of-step pass that also gives
`Hydrology`, so the two `water_state` passes create no mismatch; no solver risk (the source
does not depend on the vapour, the Jacobian is unchanged); total lake evaporation is bounded
by the catchment's runoff, so even remnant seas (`min_sea_fraction=1`) only return runoff;
determinism fine. Findings and what was done:

1. *Cells that became ocean should not drop the source*: adding it to that cell's
   evaporation is exact and harmless. **Done** (evenly over the year; no "dropped" bookkeeping).
2. *Conservation closes only as a long-run average*: each solved month keeps the lake source
   of the step it was solved in (one of four months per step), with monthly weights normalised
   by the other months' (older) lake potential evaporation. A lake living one step returns
   `E · w_m` instead of `E`. **Accepted and stated**; the closure is evaluated as window sums
   (climate source vs balance), plus the per-snapshot mismatch. Exact per step would need all
   months re-solved every step (4× the climate cost).
3. *Separate the two effects*: **done** with an ablation run set (ice-reduced lake PET only,
   no moisture source; a temporary commit).
4. *The logged "lake potential evaporation" now counts ice-free months only*: noted in the
   doc comment and the results.
5. Optional, done: unit tests for the exact annual return with uniform months and for the
   frozen-all-year fallback (weight 1); evaluation adds Newton iterations/clipped share where
   logged and lake/endorheic shares by latitude band where the dumps allow.

## Changes after the plan review (during implementation and code review)

- **Monthly weights from temperature alone**: `w_m = s(T_m) / mean_m s(T_m)` with
  `s(T) = ice_free(T) · q_sat(T)` (`climate::lake_season`), instead of the months' lake PET.
  The PET depends on the vapour of the month being solved, so a PET-based normalisation was not
  exact even within one update; with temperature-only weights the months average to exactly 1
  in every update, and the only remaining mismatch is the lag (each month keeps the lake source
  of the update that last solved it: up to 4 steps with the defaults).
- **Areas** (code review): the water balance works with the mean cell area `4πR²/N`, the
  climate with Voronoi areas; the lake source is converted (`mean_area / area[cell]`), so the
  volumes match exactly.
- **Cold lakes** (code review, no change): the excess is the ice-reduced lake PET minus the
  Budyko ET (with the land's unreduced PET); where the ET exceeds it, the lake evaporates no
  more than land would and adds nothing — plausible, no water lost.
