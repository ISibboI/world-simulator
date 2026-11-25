# Results: lakes flicker (#160)

_2026-11-25. Plan and the measurement of the causes: [plan.md](plan.md). Scripts:
[scripts/lakes.py](scripts/lakes.py) (persistence from the per-step depression log),
[scripts/compare.py](scripts/compare.py) (snapshot and log metrics). Goal: an Earth-like
planet, not Earth; grid effects accepted (#64)._

## What changed

1. **Lakes at the end of the step.** `erode` computes the water balance twice
   (`water_state`): at the start of the step for routing water and sediment (unchanged), and
   on the topography after the step's erosion and deposition for the lakes shown, their water
   volume (sea level), the receivers and discharge of the view and the hydrology statistics.
   A basin the step's sediment filled holds no lake.
2. **Onset depth with continuation** (`erosion.lake_onset_depth` = 30 m): a new depression
   holds water only if it is at least 30 m deep; one on or next to last step's lake continues
   the lake down to 1 m. Shallow new depressions pass their water (their sediment is still
   trapped). The rendering rule (lake ≥ 1 m) is unchanged.
3. **Playas** (endorheic basins less than a tenth covered) and all closed depressions are
   logged; the depression log (`--lake-log-from`) has the depth below the spill level, the
   depressions at the start of the step and the depressions filled by sediment.

## Why lakes flickered

See [plan.md](plan.md): before the change, about half of the depression cells at the start of a
step were filled to their spill by the step's own sediment, yet they were shown (and counted)
as lakes;
and ~60% of the depressions at the start of a step were new, mostly where the plates' motion
had made cells show the next crust sample (a resampled land cell jumps by ~220 m, others by
~20 m). The lakes shown were the shallow depressions that the latest step's tectonics opened,
not the basins that hold water over the step.

The water balance was already there (#58/#114): basins are partly dry where evaporation
balances the inflow. It does not need a climate coupling for this issue.

## Lake persistence (steps 500–1000, per-step log)

Lakes are connected groups of lake cells, followed from step to step by overlap (else
adjacency: lakes move with their plates). "Lost per step": lake cells that are no lake one
step later (what flickers on screen).

| | 6 sub: before | 6 sub: after (30 m) | 6 sub: onset 15 m (4 seeds) | 6 sub: onset 60 m (4 seeds) | 5 sub: before (4 seeds) | 5 sub: after (4 seeds) |
|---|---|---|---|---|---|---|
| lake cells per step | 530 | 250 | 269 | 141 | 152 | 63.7 |
| lakes per step | 248 | 47.5 | 56.9 | 22.8 | 82 | 23.3 |
| single-cell lakes (% of lakes) | 73.9 | 51.2 | 53.6 | 51.4 | 71.5 | 61.7 |
| lake size, cells (area-weighted mean) | 14.3 | 32.7 | 41 | 32.8 | 5.18 | 10.7 |
| lake cells lost per step | 335 | 123 | 142 | 61.2 | 79.9 | 30 |
| lake cells still lake after 1 Myr (%) | 35.6 | 48.4 | 47.1 | 53.2 | 47 | 52.1 |
| lake cells still lake after 10 Myr (%) | 13.8 | 13.7 | 13.4 | 12.8 | 14.1 | 11.6 |
| lake lifetime, mean (Myr) | 1.94 | 2.32 | 2.26 | 2.6 | 2.51 | 2.93 |
| lake lifetime, area-weighted mean (Myr) | 2.17 | 2.79 | 2.77 | 2.93 | 2.74 | 3.36 |
| lake cells in lakes older than 10 Myr (%) | 8.04 | 12.6 | 11.6 | 12.6 | 9.46 | 9.35 |
| depression cells per step | 772 | 551 | 466 | 431 | 228 | 135 |
| lost lake cells: depression vanished (%) | 91.7 | 83.1 | 87.3 | 83.9 | 85.9 | 79.6 |
| spill depth of lake cells, median (m) | 26.9 | 106 | 119 | 98.6 | 47.2 | 75.3 |

- **Flicker:** lake cells appearing and vanishing per step −63% (335 → 123 at 6 subdivisions,
  80 → 30 at 5); lakes per step 248 → 48; single-cell lakes 74 → 51%; a lake cell is still lake
  1 Myr later 36 → 48%.
- **Lifetimes** grow only a little: 2.2 → 2.8 Myr (area-weighted), 13% of lake cells in lakes
  older than 10 Myr (8% before). The basins that hold water still come and go with the
  resampled relief and sediment fill; deep, long-lived structural lakes (Baikal-like) exist but
  are few.
- **Onset depth:** 15 m and 60 m bracket 30 m; 60 m halves the flicker again but leaves 0.6%
  of land as lake. 30 m is the step's typical elevation change away from resampling hops (not
  calibrated).

## Before/after (8 seeds × 1000 Myr, window 500–1000 Myr)

Before = `agent/issue-112` tip with the new logging (8191d5e, same state), after = 0ed0d1e;
`--snapshot-interval 50 --width 512 --cell-dump --lake-log-from 500`. Means over snapshots,
then seeds (seed range), Welch t. Run times are GitHub runners (not comparable, see below).

| metric | before | after | t |
|---|---|---|---|
| cells on duplicated samples (%) | 11.4 (10.7…12.1) | 12 (11.5…12.6) | +2.9 |
| single-cell land pits (% of land) | 1.19 (0.968…1.42) | 1.26 (1…1.77) | +0.7 |
| pits with a same-sample twin (% of pits) | 48.4 (41…53.8) | 46.1 (33.8…52.9) | -0.8 |
| pit depth median (m) | 9.45 (6.25…11.4) | 9.6 (6.73…14.2) | +0.1 |
| pits > 20 m (% of land) | 0.347 (0.197…0.508) | 0.369 (0.217…0.724) | +0.3 |
| pits > 100 m (% of land) | 0.0706 (0.0296…0.136) | 0.0764 (0.037…0.195) | +0.3 |
| pits > 500 m (per snapshot) | 0.409 (0.0909…1) | 0.534 (0.273…1.36) | +0.7 |
| single-cell land peaks > 100 m (% of land) | 0.982 (0.848…1.1) | 1.1 (0.993…1.16) | +2.7 |
| land neighbour |dE| (m) | 146 (110…184) | 160 (139…199) | +1.1 |
| land (% of surface) | 29.5 (27.8…31.2) | 29.8 (28.3…31.1) | +0.5 |
| mean land (m) | 883 (470…1.32e+03) | 872 (561…1.56e+03) | -0.1 |
| land p50 (m) | 626 (260…1.15e+03) | 618 (276…1.44e+03) | -0.0 |
| land p90 (m) | 2.13e+03 (1.27e+03…2.79e+03) | 2.11e+03 (1.62e+03…3.03e+03) | -0.1 |
| land p99 (m) | 3.12e+03 (2.45e+03…3.82e+03) | 3.18e+03 (2.96e+03…3.79e+03) | +0.3 |
| ocean p50 (m) | -3.66e+03 (-3.73e+03…-3.58e+03) | -3.68e+03 (-3.76e+03…-3.61e+03) | -0.5 |
| lake cells (% of land) | 4.32 (3.46…4.89) | 1.89 (1.03…4.17) | -6.1 |
| lakes on single-cell pits (% of lake cells) | 20.8 (18.7…22.8) | 26.2 (20.7…31.5) | +3.7 |
| flooded continental crust (%) | 22.4 (12…33.5) | 22.5 (10.5…31) | +0.0 |
| highest point (m) | 4.17e+03 (3.78e+03…4.69e+03) | 4.16e+03 (3.82e+03…4.54e+03) | -0.1 |
| mean speed (cm/yr) | 4.17 (3.39…4.72) | 4.03 (3.42…4.64) | -0.8 |
| sea level (m) | -191 (-263…-70) | -185 (-307…-92.2) | +0.2 |
| duplicated cells (%, log) | 11.4 (10.6…12.1) | 12 (11.5…12.6) | +2.9 |
| land precipitation (m/yr) | 1.16 (0.978…1.39) | 1.14 (0.9…1.38) | -0.3 |
| endorheic (% of land) | 20.2 (12.4…29.6) | 9.9 (4.06…22.7) | -3.4 |
| endorheic into single-cell pits (% of land) | 8.18 (6…11.8) | 0.574 (0.191…1.22) | -11.3 |
| lakes (% of land, log) | 3.71 (2.9…4.28) | 1.81 (0.973…3.94) | -5.1 |
| depressions | 300 (254…338) | 58.1 (36.5…117) | -17.6 |
| endorheic depressions | 141 (111…166) | 32 (16…65.4) | -12.2 |
| lake evaporation (km³/yr) | 5.85e+03 (4.82e+03…6.44e+03) | 2.6e+03 (1.49e+03…3.69e+03) | -9.3 |
| runoff to the ocean (km³/yr) | 5.41e+04 (4.66e+04…5.97e+04) | 5.83e+04 (4.86e+04…6.52e+04) | +1.8 |
| global mean temperature (°C) | 22.7 (21.7…23.7) | 22.4 (20.8…23.5) | -0.6 |
| run time (s) | 689 (640…730) | 860 (784…943) | +6.8 |

After only: all closed depressions 4.3% of land (with or without water), playas 1.2% of land.
5 subdivisions (4 seeds): lake cells 4.8 → 2.0% of land, endorheic 20 → 10%, into single-cell
pits 6.8 → 1.1%, depressions with water 95 → 26.

- **Lakes 4.3 → 1.9% of land** (Earth: ~2% of land under lakes, most of it in lakes far below
  the cell size; lakes larger than a cell, > 10⁴ km², cover ~0.6%).
- **Endorheic land 20 → 10%**, and the land draining into single-cell pits **8.2 → 0.6%**:
  most of #129's endorheic pit drainage was water ending in new shallow depressions that the
  step's sediment fills anyway. Earth: ~18–20% of land drains internally, mostly arid; 10% is on
  the low side but plausible for these continents (arid fraction about Earth-like).
- **Knock-ons:** lake evaporation halves (2.6 vs 5.9 × 10³ km³/yr), runoff to the ocean +8%;
  sea level, hypsometry, climate (temperature, land precipitation) within scatter.
  Duplicated cells +0.6 points (t +2.9) and single-cell peaks +0.1 point (t +2.7) are not
  touched by this change (chaotic divergence among ~40 metrics).

## Runtime (6 subdivisions)

Paired local runs (seed 0, 200 Myr, one core each, tktie loaded): before 150.9 s, after 149.3 s.
Per-step phase timings over 60 Myr (`RUST_LOG=debug`): erosion 141.6 → 148.6 ms per step
(+7 ms for the second water balance, ~1.6% of a step of ~430 ms). The GitHub-hosted run times
(+25%) come from different runner machines. The desktop still handles 6 subdivisions.

## For the related issues

- **#126 (lakes as a moisture source):** needs the lake area per cell and its evaporation:
  `SurfaceWater::lake_depth` (> 0 = lake) and the excess evaporation of the balance, both now
  from the end of the step and persistent enough for a lagged (previous step) coupling. The
  balance itself does not need it.
- **#127 (water load on dry depressions):** the water balance gives each depression's water
  volume (`lake_volume`, per cell `lake_depth`), so loading lakes with their water is direct.
  Dry depressions (2.4% of land, 1.2% playas) have no water by the balance; whether they should
  be loaded is #127's question, not made trivial by this change.
- **#128 (draw depressions as lake ≥ 1 m or land):** unchanged premise. `depression` now also
  contains shallow new depressions without water and playas; `spill_depth` is available.

## Caveats

- Flicker is reduced, not gone: the relief itself is resampled every step (grid effect), and
  basins that survive the sediment fill still move and vanish with it. Lifetimes of 2–3 Myr per
  lake are a consequence.
- The onset depth is a resolution argument, not a process; a real 20 m-deep basin of 10⁴ km²
  that the sediment does not fill would hold a lake.
- Lake water is not carried from step to step (lakes equilibrate within years; a Myr step sees
  only the balance).
