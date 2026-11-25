# Plan: lakes flicker (#160)

_2026-11-25. Stacked on PR #165 (→ #163). Goal: an Earth-like planet, not Earth; effects of the
~110 km grid are accepted (#64)._

## What the code did

`erosion::erode` finds the closed depressions (≥ 1 m below their spill level, `LAKE_MIN_DEPTH`)
by a priority flood **at the start of every step**, and every depression gets a water balance
(`lake_balance`, since #58/#114): the lake rises until its excess evaporation over the open water
balances the inflow, or spills. So lakes are not pure geometry: arid basins are partly dry and
endorheic. But the lakes shown (and their water in the sea-level budget) were those of the
**start of the step**, before the step's sediment went into them.

## Measurement (before)

Per-step depression log (`--lake-log-from`, now with the depth below the spill level, the
depressions at the start of the step and those filled by sediment), script
[`scripts/lakes.py`](scripts/lakes.py). Lakes are tracked as connected groups of lake cells from
step to step (overlap, else adjacency, because lakes move with their plates).

8 seeds × 1000 Myr, 6 subdivisions, steps 500–1000 (`agent/issue-112` tip):

| | |
|---|---|
| lake cells still lake 1 / 10 Myr later | 36% / 14% |
| lakes per step / single-cell lakes | 248 / 74% |
| lake lifetime (mean, area-weighted) | 1.9 / 2.2 Myr |
| lake cells lost because the depression itself vanished | 92% (dried in a remaining depression: 8%) |
| depth below the spill of lake cells / of vanishing ones (median) | 27 m / 16 m |

A diagnostic build (seed 0, 5 subdivisions, steps 220–320) located where depressions appear and
vanish:

- **The step's sediment fills them.** About half of the depression cells at the start of a step
  are no depressions any more after the step's deposition in lakes (90 → 44 cells per step;
  52 at the end of the step after the coastal processes and diffusion): a one-cell basin 15 m
  deep holds ~200 km³, and a few eroding cells deliver that in a fraction of a Myr.
- **Each step makes new ones.** ~60% of the depression cells at the start of a step are new.
  52% of the new ones show a different crust sample than at the end of the previous step
  (17% of all land cells do), 82% are next to such a cell. A land cell that shows another sample
  jumps by 220 m on average, others by 20 m. The plates move ~0.3–0.5 cell per step and
  nearest-vertex resampling hops cells to the next sample at different times, which distorts the
  relief by up to half a cell each step: on low relief this opens and closes depressions of
  tens of metres. That is a grid effect (#64).

## Fix

1. **Lakes at the end of the step.** The routing of water and sediment during the step still
   uses the depressions at its start (that is where the water went). The lakes shown, their
   water volume (sea level), the receivers and discharge for the view and the hydrology
   statistics come from a second water balance on the topography **after** the step's erosion
   and deposition. A basin the step's sediment filled holds no lake.
2. **Onset depth with continuation (hysteresis).** A depression that held no lake in the
   previous step holds water only if its deepest floor lies at least `lake_onset_depth` = 30 m
   below its spill (about the step's elevation noise away from resampling hops); shallower new
   depressions pass their water like a filled basin (their sediment is still trapped, so they
   fill). A depression on or next to last step's lake cells continues the lake down to 1 m.
   The rendering rule (lake ≥ 1 m, #128) is unchanged.
3. **Playas:** endorheic depressions whose water covers less than a tenth of their area are
   counted as salt flats/playas (log). Arid basins were already partly dry by the balance.

Not done: lake water as a moisture source (#126); water load on dry depressions (#127); the
drawing of dry depressions (#128). Notes on what they need are in [results.md](results.md).

## Evaluation

Before = `agent/issue-112` tip with the new logging (state identical), after = branch; 8 seeds ×
1000 Myr (`--snapshot-interval 50 --width 512 --cell-dump --lake-log-from 500`), window
500–1000. Plus 4 seeds at 5 subdivisions (the web default) and onset depths 15 and 60 m.
Metrics: lake persistence (1/10 Myr), lifetimes, turnover (lake cells appearing/vanishing per
step), lake count/area/size, endorheic share, playas, sea level, climate, guard rails, runtime
at 6 subdivisions.
