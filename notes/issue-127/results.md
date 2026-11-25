# Results: water load only where water stands (#127)

_2026-11-25. Plan with its review and revision: [plan.md](plan.md). Scripts: [scripts/](scripts/)
(`compare.py` snapshot/log metrics, `lakes.py` lake persistence from #160)._

## Validity on #166

Still valid, but small in area: per snapshot (500–1000 Myr, 8 seeds) 40 cells of closed
depressions below sea level outside the ocean (0.1% of the surface), 18 of them dry; they were
drawn with the ocean's load, 45% too deep (mean −116 m). The larger part of the error was the
other side: the ~250 lake cells (1.9% of land, mostly above sea level) had floors without the
load of their own water, so lakes were 31% too shallow and their water volume too small.

## What changed

- **Lake depth and volume with the water's own load** (`erosion::lake_water_depth`): a lake
  whose surface stands at `L` over the unloaded floor `x` is `1.45 · (L − x)` deep (Airy). It
  covers the same cells at the same level, so the water balance is unchanged. The lake volume,
  which is taken from the ocean, is the loaded one.
- **The water balance stands on unloaded floors**: closed depressions below sea level that the
  ocean does not reach are converted from the processes' (ocean-loaded) elevations to their
  unloaded ones before the balance (found by the code review: their lake levels were otherwise
  computed on floors 45% too deep, and a lake could have a level but no depth). The conversion
  keeps the order, so drainage and depressions are unchanged; spill and water levels go back
  to the processes' frame for the routing and deposition.
- **Displayed relief** (`Planet::surface_elevation`, used by the maps, the viewer and the
  cell dump `elev`): lake floors pressed down by their water, dry closed depressions below sea
  level unloaded (`SurfaceWater::load_correction`, from the end of the last erosion step).
- **Unchanged**: the processes (`Simulation::surface_elevation`: routing, sediment fills, ocean,
  sea-level solve), the hypsometry in the snapshot log, `spill_depth` (unloaded).

## Before/after (8 seeds × 1000 Myr, window 500–1000 Myr)

Before = `agent/issue-160` tip (10df505), after = cb3a0b9; `--snapshot-interval 50 --width 512
--cell-dump --lake-log-from 500`; means over snapshots, then seeds (range), Welch t. The
elevation columns of "after" are the displayed ones.

| metric | before | after | t |
|---|---|---|---|
| cells on duplicated samples (%) | 12 (11.5…12.6) | 11.7 (10.9…12.3) | -1.7 |
| single-cell land pits (% of land) | 1.26 (1…1.77) | 1.29 (1.07…1.52) | +0.3 |
| pits with a same-sample twin (% of pits) | 46.1 (33.8…52.9) | 47.5 (40.3…54.6) | +0.5 |
| pit depth median (m) | 9.6 (6.73…14.2) | 9.95 (7.15…12.8) | +0.3 |
| pits > 20 m (% of land) | 0.369 (0.217…0.724) | 0.381 (0.234…0.55) | +0.2 |
| pits > 100 m (% of land) | 0.0764 (0.037…0.195) | 0.0798 (0.038…0.139) | +0.2 |
| pits > 500 m (per snapshot) | 0.534 (0.273…1.36) | 0.58 (0.0909…0.909) | +0.3 |
| single-cell land peaks > 100 m (% of land) | 1.1 (0.993…1.16) | 1.13 (0.97…1.29) | +0.7 |
| land neighbour |dE| (m) | 160 (139…199) | 149 (102…185) | -0.8 |
| land (% of surface) | 29.8 (28.3…31.1) | 29 (25.7…30.6) | -1.1 |
| mean land (m) | 872 (561…1.56e+03) | 747 (411…1.24e+03) | -0.9 |
| land p50 (m) | 618 (276…1.44e+03) | 485 (241…909) | -0.9 |
| land p90 (m) | 2.11e+03 (1.62e+03…3.03e+03) | 1.83e+03 (998…2.92e+03) | -1.1 |
| land p99 (m) | 3.18e+03 (2.96e+03…3.79e+03) | 3.02e+03 (2.64e+03…3.77e+03) | -0.9 |
| ocean p50 (m) | -3.68e+03 (-3.76e+03…-3.61e+03) | -3.67e+03 (-3.78e+03…-3.48e+03) | +0.1 |
| lake cells (% of land) | 1.89 (1.03…4.17) | 1.69 (0.703…2.3) | -0.5 |
| lakes on single-cell pits (% of lake cells) | 26.2 (20.7…31.5) | 27.8 (21…38.5) | +0.6 |
| dry land below sea level (cells) | 18.4 (12…24.5) | 21.9 (13.5…31.7) | +1.4 |
| dry land below sea level, mean elevation (m) | -116 (-169…-54.9) | -109 (-181…-26.4) | +nan |
| lake depth, median (m) | 67.4 (26.2…135) | 73.9 (37…135) | +0.3 |
| lake floor, median (m) | 702 (234…1.28e+03) | 446 (265…1.19e+03) | -1.7 |
| flooded continental crust (%) | 22.5 (10.5…31) | 26.3 (19.6…35.9) | +1.2 |
| highest point (m) | 4.16e+03 (3.82e+03…4.54e+03) | 4.11e+03 (3.82e+03…4.6e+03) | -0.4 |
| mean speed (cm/yr) | 4.03 (3.42…4.64) | 4.16 (3.81…4.68) | +0.9 |
| sea level (m) | -185 (-307…-92.2) | -155 (-272…-93.4) | +0.9 |
| duplicated cells (%, log) | 12 (11.5…12.6) | 11.7 (10.8…12.3) | -1.7 |
| land precipitation (m/yr) | 1.14 (0.9…1.38) | 1.17 (0.992…1.36) | +0.4 |
| endorheic (% of land) | 9.9 (4.06…22.7) | 10.2 (4.84…17.6) | +0.1 |
| endorheic into single-cell pits (% of land) | 0.574 (0.191…1.22) | 0.586 (0.318…1.05) | +0.1 |
| lakes (% of land, log) | 1.81 (0.973…3.94) | 1.62 (0.682…2.37) | -0.5 |
| depressions | 58.1 (36.5…117) | 54.1 (25.5…75) | -0.4 |
| endorheic depressions | 32 (16…65.4) | 30.8 (16.3…47.5) | -0.2 |
| all closed depressions (% of land) | 4.3 (2.35…9.2) | 4.22 (2.62…6.22) | -0.1 |
| playas (% of land) | 1.16 (0.273…3.69) | 1.04 (0.255…2.35) | -0.3 |
| lake evaporation (km³/yr) | 2.6e+03 (1.49e+03…3.69e+03) | 2.97e+03 (1.1e+03…4.99e+03) | +0.7 |
| runoff to the ocean (km³/yr) | 5.83e+04 (4.86e+04…6.52e+04) | 5.56e+04 (4.54e+04…6.07e+04) | -1.0 |
| lake water (m of sea level) | 1.47 (0.495…4.13) | 1.64 (0.387…2.8) | +0.3 |
| closed depressions below sea level (cells) | 40.5 (26.1…56.5) | 47.8 (31.6…72.6) | +1.1 |
| … of them under lake water (cells) | 22.8 (13.9…27.2) | 28.7 (12.2…44.4) | +1.2 |
| global mean temperature (°C) | 22.4 (20.8…23.5) | 22.8 (21.7…23.6) | +1.0 |
| run time (s) | 631 (583…689) | 813 (679…944) | +4.4 |

Lake persistence (per-step log, #160's script): lake cells still lake 1 Myr later 48.4 → 49.2%,
10 Myr 13.7 → 14.0%; lake lifetime (area-weighted) 2.79 → 2.80 Myr; lakes per step 48 → 44;
lake cells appearing/vanishing per step 123 → 106.

- **Per cell, by construction:** a dry depression below sea level is drawn 31% shallower than
  before (its unloaded floor); a lake is 45% deeper (its floor lower by 0.45 × its depth on the
  unloaded floor), with 45% more water taken from the ocean.
- **Over the runs:** everything is within the seed scatter (|t| ≤ 1.7). Lake water 1.47 → 1.64 m
  of sea level, median lake floor 702 → 446 m, lake area 1.9 → 1.7% of land; the planets
  diverge chaotically after the first changed step, so per-cell effects are not visible as
  shifts of the means. No knock-on on persistence, endorheic share, sea level or climate.
- **Water and sea level:** the ocean's volume is the planet's water minus the (loaded) lake
  volume, as before; the sea-level solve is unchanged.

## Runtime (6 subdivisions)

Paired local runs (seed 0, 200 Myr, one core each): before 108.8 s, after 109.7 s (+0.8%, within
noise): the change adds two per-cell vectors per step. The GitHub-hosted run times (+29%) come
from different runner machines (seeds 4–7 of "after" ran ~40% slower than seeds 0–3).

## Caveats and notes for #128

- A deep lake on low land can now have its floor drawn below sea level (when its depth on the
  unloaded floor exceeds ~2.2× the floor's height). Masks that decide "ocean" by
  `elevation < 0` in the view (realistic shading, `surface_color`, some mode masks, the
  aridity index) then treat it as sea; closed depressions below sea level already had this.
  Using the ocean mask instead of the sign is #128's change.
- The correction is from the end of the last erosion step; it is applied at snapshots and
  frames after the same step.
- A partly covered top cell of a lake gets the full lake depth (grid-scale).
- The erosion's deposition still uses the processes' frame (a lake fills to its spill in dry
  terms, which is right: sediment replaces the water and a full basin carries no water load).
