# Results: lakes as a moisture source (#126)

_2026-11-25. Stacked on #167 (agent/issue-127, 50a0b5a). Plan, plan review and the changes
after review and code review: [plan.md](plan.md). Full tables: [table-main.md](table-main.md)
(before / ice-only / after), [table-nosea.md](table-nosea.md) (without the 0.2% rule, before /
after), [table-before-rule.md](table-before-rule.md) (before, with / without the rule).
Scripts: [scripts/](scripts/) (`compare.py` from #127 with the #126 metrics, `lakes.py` from
#160). Goal: an Earth-like planet, not Earth; grid effects (~110 km cells) accepted (#64)._

## Validity on #167

Valid. Lake cells were land to the climate (`climate_elevations` raises every non-ocean cell
to ≥ 0 m) and evaporated only the land's Budyko evapotranspiration. The lake water balance
removed the lakes' extra evaporation (2–5 × 10³ km³/yr) from the runoff, and nothing gave it
to the air: the coupled water cycle lost it, and no lake moistened its surroundings.

## What changed

- **Lake evaporation returns to the air** (`SurfaceWater::lake_evaporation` → `Climate::update`):
  the water balance records the lake evaporation per cell (the excess over the land's
  evapotranspiration, as it removes it from the runoff; the per-cell field sums exactly to the
  balance's total, unit-tested). The climate adds it as a moisture source in the next step,
  spread over the solved months by `ice_free(T) · q_sat(T)` (temperatures only, so the months'
  weights average to exactly 1), and carries it with the winds like ocean evaporation.
- **Lake ice**: lakes evaporate at the potential rate only while free of ice
  (`WaterCycle::lake_potential_evaporation`: monthly land PET × a ramp from 0 at −2 °C to 1 at
  +2 °C monthly mean). The lake water balance uses it instead of the year-round land PET.
  The lake surface has the land's temperature (no lake thermal inertia).
- Diagnostics: the snapshot log shows "lake evaporation N km³/yr (into the air M)", the cell
  dump has `lake_evap` (balance, m/yr) and `lake_air` (climate, m/yr), a debug line per step
  compares both. "Lake potential evaporation" in the log now counts ice-free months only.

## Before/after (8 seeds × 1000 Myr, window 500–1000 Myr)

Before = agent/issue-127 tip (50a0b5a), ice-only = the ice change without the moisture source
(ablation, temporary commit 6c5b5e3), after = 7d30a5f. `--duration 1000 --snapshot-interval 50
--width 512 --cell-dump --lake-log-from 500`. Means over snapshots, then seeds (range), Welch t
against before. Excerpt (all metrics in [table-main.md](table-main.md)):

| metric | before | ice-only | t | after | t |
|---|---|---|---|---|---|
| lake cells (% of land) | 1.69 (0.70…2.3) | 1.42 (0.83…2.15) | -0.9 | 1.75 (0.46…3.67) | +0.1 |
| lake water (m of sea level) | 1.64 (0.39…2.8) | 1.9 (0.08…6.7) | +0.3 | 2.64 (0.76…6.9) | +1.2 |
| lake depth, median (m) | 73.9 | 80.8 | +0.3 | 108 | +1.2 |
| endorheic (% of land) | 10.2 (4.8…17.6) | 6.26 (3.2…12.4) | -1.9 | 6.17 (2.9…11.4) | -2.0 |
| playas (% of land) | 1.04 (0.26…2.35) | 0.49 (0.08…1.12) | -2.2 | 0.54 (0.05…0.84) | -2.1 |
| lake evaporation (km³/yr) | 2.97e3 | 2.10e3 | -1.7 | 2.35e3 | -1.0 |
| lake potential evaporation (m/yr) | 2.06 | 1.81 | -1.2 | 1.81 | -1.1 |
| precipitation on lake cells (m/yr) | 1.15 | 1.13 | -0.2 | 1.22 | +0.7 |
| precipitation 1 cell from lakes (m/yr) | 1.12 | 1.08 | -0.7 | 1.15 | +0.3 |
| precipitation 2–3 cells from lakes (m/yr) | 1.09 | 1.06 | -0.5 | 1.09 | -0.0 |
| precipitation, land farther from lakes (m/yr) | 1.18 | 1.23 | +0.8 | 1.16 | -0.3 |
| land precipitation (m/yr) | 1.17 | 1.21 | +0.7 | 1.16 | -0.1 |
| global precipitation = evaporation (m/yr) | 1.52 | 1.57 | +1.4 | 1.54 | +0.3 |
| arid land, P < 0.25 m/yr (% of land) | 21.5 | 18.2 | -1.2 | 20.3 | -0.4 |
| Köppen A / B / C / D / E (% of land) | 16.6 / 34.5 / 16.5 / 28.5 / 3.9 | 17.7 / 31.0 / 15.6 / 31.2 / 4.5 | | 17.1 / 33.7 / 14.6 / 29.5 / 5.1 | C −2.0 |
| clipped precipitation (%) | 0.008 | 0.020 | +1.1 | 0.008 | -0.1 |
| sea level (m) | -155 | -132 | +0.7 | -189 | -1.1 |

Lake persistence (per-step log, #160's script): lake cells per step 213 → 210, lakes per
step 44 → 38, area-weighted lake lifetime 2.80 → 2.87 Myr, lake cells in lakes older than
10 Myr 13.0 → 12.5%, lake cells appearing/vanishing per step 106 → 100.

- **Globally the effect is small**, as expected from its size: the lake source is ~2 × 10³
  km³/yr, 0.3% of the global evaporation (~8 × 10⁵ km³/yr). Precipitation on and next to lakes
  rises by ~0.05–0.07 m/yr (+3–6%), within the seed scatter (the planets diverge chaotically
  after the first changed step, so per-cell effects do not show as shifts of means). Global and
  land precipitation, aridity and Köppen shares are within scatter.
- **Lake effect, controlled**: in the unit test (`climate::lakes_are_a_moisture_source`), a
  lake of ~3 × 10⁵ km² evaporating 1 m/yr in the westerlies raises the precipitation on the
  land east of it by 8% (0.62 → 0.67 m/yr); most of the vapour travels farther (the column
  rains out in ~10 days, a few thousand km at westerly speeds). The precipitation near lakes
  in the runs is consistent with that.
- **Endorheic share 10.2 → 6.2% of land (t −2.0)**, playas halved: this comes from the **ice
  change** (the ablation shows the same, 6.3%): lakes no longer evaporate in frozen months, so
  cold and temperate lakes reach their spill level and drain. The moisture source on top
  brings the lake area back (1.42 → 1.75% of land) and deepens the lakes (lake water 1.9 →
  2.6 m of sea level). Earth: 18–20% of land drains internally, mostly arid; 6% is low.
  The difference to before is at the edge of significance (t −2) with a large seed scatter
  (before 4.8–17.6%).
- **No runaway**: lake area per snapshot fluctuates without a trend (seed means 1.0–2.3% of
  land from 300 Myr on); the total lake evaporation is bounded by the runoff of the
  catchments. The Newton solve is unaffected (the source does not depend on the vapour;
  clipped precipitation unchanged).

## Water conservation (P − E closure)

- The climate's budget closes per month as before (global P = E to 0.01%; the lake source is
  part of E).
- **Lake evaporation removed vs returned**, per-step debug log, 400 Myr, steps 100–400:
  returned/removed = 0.973 (seed 0) and 0.974 (seed 1 without the 0.2% rule). Snapshot values
  alone (11 per window) scatter by ±50% per seed, because each month keeps the lake source of
  the step that last solved it (one month per step, so the return lags 1–4 steps while lakes
  live ~2–3 steps).
- The remaining ~3% is not returned: the staggered refresh weights each step's lake evaporation
  with the seasonal weight of the month solved next (exact only on average), and lake
  evaporation correlates slightly with the month just refreshed. Before, 100% was lost; making
  it exact would need all months re-solved every step (4× the climate cost). Accepted.

## The 0.2% sea rule (`min_sea_fraction`, Sebastian's note on #126)

Re-tested as #58's `b2` (`--set min_sea_fraction=1`, closed ocean remnants become land with
lakes), before and after, same 8 seeds ([table-nosea.md](table-nosea.md)):

| metric | before, rule | before, no rule | after, rule | after, no rule |
|---|---|---|---|---|
| endorheic (% of land) | 10.2 (4.8…17.6) | 16.4 (5.3…23.6) | 6.2 (2.9…11.4) | 13.1 (7.4…31.2) |
| lake cells (% of land) | 1.69 | 2.19 | 1.75 | 2.70 |
| closed depressions below sea level (cells) | 48 | 467 | 46 | 418 |
| … of them under lake water (cells) | 29 | 201 | 31 | 265 |
| dry land below sea level (cells, dumps) | 22 | 231 | 19 | 108 |
| playas (% of land) | 1.04 | 2.30 | 0.54 | 1.38 |
| lake water (m of sea level) | 1.6 | 12 (0.4…66) | 2.6 | 18 (0.5…93) |
| arid land (% of land) | 21.5 | 21.0 | 20.3 | 21.2 |
| Köppen B (% of land) | 34.5 | 31.3 | 33.7 | 33.2 |

- **The moisture source helps the remnants**: without the rule, dry land below sea level
  halves (231 → 108 cells), the share of the remnants' cells under water rises from 43% to
  63%, playas 2.3 → 1.4%. Aridity and Köppen B are not raised by dropping the rule, before or
  after (the early #58 desert effect is not seen on the current chain).
- **But the rule still matters**: without it, endorheic land is about twice as large (13 vs 6%
  after; one seed 31%), and the lake water of a few seeds reaches tens of metres of sea level
  (seed 1: 93 m) — whole remnant seas held as lakes, which then evaporate inside the continent
  and are taken from the ocean.
- **Recommendation**: keep the 0.2% rule for now. Dropping it gives an endorheic share
  (13%) closer to Earth's 18–20% but with much more seed-to-seed scatter and very large
  inland seas; that is a decision for Sebastian. Letting the sea spill back over a sill into a
  basin below sea level that has dried out (his other suggestion) was not tried; it would turn
  most of the 108 dry cells into marine inlets.

## Runtime (6 subdivisions)

Paired local runs (seed 0, 200 Myr, one core each, at the same time): before 117.3 s, after
116.0 s (within noise). The change adds a few per-cell vectors per step and no solver work. The run
times of the run sets (827 → 868 s) come from a mix of local and GitHub runner machines and
are not comparable. The desktop still handles 6
subdivisions.

## Caveats

- The lake surface has the land's (air) temperature of its cell; no lake thermal inertia, no
  lake-effect snow specifics (precipitation does not fall as snow in the climate model).
- Ice is a monthly ramp over the 4 solved months (coarse); sublimation from ice is neglected.
- Where the land's Budyko evapotranspiration exceeds the ice-reduced lake PET (cold climates),
  a lake evaporates no more than land and adds nothing.
- Lake evaporation is from the lakes of the last steps (lakes move with their plates by up to
  half a cell per step; grid-scale).
- ~3% of the lake evaporation is not returned (staggered month refresh, see above).

## For #128

Nothing new: the moisture source does not change where lakes are drawn. Lakes are a little
deeper (median 74 → 108 m), so more lake floors drawn below sea level could show as sea in the
masks that test `elevation < 0` (the #127 caveat).
