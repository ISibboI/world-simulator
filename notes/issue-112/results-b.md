# Results: option (b), the surface processes see every crust sample once (#112)

_2026-11-25. Plan and reviews: [plan-b.md](plan-b.md). On top of option (a) ([results.md](results.md)).
Script: [scripts/compare_b.py](scripts/compare_b.py). Goal: an Earth-like planet, not Earth;
effects of the ~110 km grid are accepted (#64)._

## What changed

- `Simulation::sample_surface_elevations`: cells showing one crust sample get the sample's
  surface at the mean dynamic topography of those cells (mean in dry elevation). This is the
  elevation `erode` works on; the climate, view, maps and cell dump keep the per-cell surface.
  Samples shown at one cell: bit-identical. `--set erosion.sample_surface=0` gives the state of
  (a) bit for bit (state hash checked, seed 3, 40 Myr; the 8-seed "off" runs below reproduce the
  (a) results exactly).
- `coastal` term of the sea-level budget relative to the sea level before the step (diagnostic).
- Diagnostics: endorheic land into single-sample depressions, cells whose ocean differs between
  the sample and per-cell surface, cell-dump columns `sample` and `drain`.
- On the stacked lake branch (#166, merged there): the lakes at the end of the step also see the
  cells of one sample at their mean level, so per-cell changes within the step do not bring the
  twin pits back.

## Before/after (8 seeds × 1000 Myr, window 500–1000 Myr)

Same commit 8ceea08, `--duration 1000 --snapshot-interval 50 --width 512 --cell-dump
--lake-log-from 900`, off = `--set erosion.sample_surface=0` (= (a), PR #165 before (b)).
Means over snapshots, then seeds (range), Welch t. "Drainage pits" are pits on the surface the
erosion used (with (b): a sample whose cells lie > 1 m below all neighbours of other samples,
counted once per sample).

| metric | off (a) | on (b) | t |
|---|---|---|---|
| **per-cell twin pits that are lakes (% of land)** | 0.425 (0.392…0.487) | 0.127 (0.103…0.161) | −23.3 |
| **lakes on single-cell pits (% of lake cells)** | 20.8 (18.7…22.8) | 14.6 (13.1…17.3) | −8.5 |
| **lake cells on shared samples (% of lake cells)** | 19 (17.8…19.9) | 11.6 (9.95…14.1) | −13.3 |
| drainage pits (per 100 land cells) | 1.19 (0.968…1.42) | 1.11 (0.948…1.39) | −0.9 |
| drainage pits on shared samples (% of pits) | 48.7 (41.1…54.2) | 34.3 (31.5…37.2) | −7.0 |
| drainage pits, k = 2 adjacent (per 100 land cells) | 0.53 | 0.34 | −11.9 |
| drainage pits, k ≥ 3 or non-adjacent (per 100 land cells) | 0.032 | 0.036 | +0.9 |
| drainage pit depth median (m) | 9.45 | 22.6 | +6.8 |
| **endorheic into single-cell pits (% of land)** | 8.18 (6…11.8) | 6.2 (4.44…7.97) | −2.5 |
| endorheic into single-sample depressions (% of land) | 8.32 (6.05…12.2) | 6.43 (4.6…8.17) | −2.3 |
| endorheic (% of land) | 20.2 (12.4…29.6) | 20.8 (15.8…31.2) | +0.2 |
| depressions / endorheic depressions | 300 / 141 | 273 / 131 | −2.1 / −1.1 |
| lake cells (% of land, dump) | 4.32 | 4.62 | +1.1 |
| lakes (% of land, log) | 3.71 | 4.09 | +1.4 |
| lake evaporation (km³/yr) | 5850 | 6700 | +2.1 |
| lake cells still lake after 1 / 10 Myr (%) | 40.3 / 15.2 | 42.6 / 18.6 | +0.5 / +1.1 |
| lake-cell run length (Myr) | 1.71 | 1.79 | +0.6 |
| cells where the erosion's ocean differs from the per-cell one | 0 | 21.5 (of 41k) | |
| per-cell single-cell land pits (% of land) | 1.19 | 1.33 | +1.9 |
| per-cell pits with a same-sample twin (% of pits) | 48.4 | 45.2 | −1.1 |
| cells on duplicated samples (%) | 11.4 | 11.9 | +2.2 |
| land (% of surface) | 29.5 | 29.8 | +0.7 |
| mean land / p50 / p90 / p99 (m) | 883 / 626 / 2130 / 3120 | 957 / 650 / 2310 / 3320 | +0.6 / +0.2 / +0.8 / +1.1 |
| land neighbour \|dE\| (m) | 146 | 162 | +1.3 |
| ocean p50 (m) | −3660 | −3670 | −0.3 |
| highest point (m) | 4170 | 4260 | +0.6 |
| flooded continental crust (%) | 22.4 | 21.6 | −0.3 |
| mean plate speed (cm/yr) | 4.17 | 4.31 | +0.8 |
| sea-level `sharing` (m per 100 Myr) | −15 | −14.3 | +0.2 |

Full table: `compare_b.py off=… on=…` output (same numbers, more digits).

**Reading**
- **Twin pits stop being sinks.** Lakes on per-cell twin pits −70% (t −23), lakes on
  single-cell pits 21 → 15% of lake cells, lake cells on shared samples 19 → 12%. The drainage
  pits left on shared samples (k = 2 adjacent: −36%) are real depressions of the column (the
  sample's mean surface below all its surroundings), so they are deeper (median 23 m instead
  of 9 m) and can fill: their deposit raises all its cells alike.
- **Endorheic drainage into single-cell pits drops** 8.2 → 6.2% of land (t −2.5), but the total
  endorheic share stays at 20–21% (Earth ~18–20%): as with (a), the water goes to the next
  closed depression. That is the open question of #172.
- **Lakes:** area +7–10% and lake evaporation +15% (t 1.1–2.1, at the edge of the scatter);
  persistence slightly better (not significant). Lakes on a sample shown at two cells now cover
  both cells.
- **Relief unchanged within scatter.** The per-cell surface (view, maps, pit scripts on `elev`)
  is not changed by (b): the lower twin still shows as a few-metre dip, just no longer as a lake
  or sink. (b) fixes the hydrology, not the relief. Per-cell pits and duplicates +12% / +4%
  (t 1.9 / 2.2) are within the chaotic scatter of ~40 metrics.
- **Ocean mismatch:** ~21 cells per step (0.05%) are sea on one surface and land on the other
  (twins straddling a coast or a strait); the sample decides for the erosion.

## Runtime

Paired local 200 Myr runs, 6 subdivisions, seed 0, one core each (run scheduler, tktie):
on 122 s, off 118 s (+3%). Per-phase timing over 100 Myr (debug log, run together): tectonics
15.8/15.2 s, climate 15.1/15.2 s, erosion 14.1/13.8 s (on/off): the extra pass is ~1–2% of a
step. The 8-seed runs ran on GitHub runners in two separate batches (on: 902 s, off: 667 s
mean); batches land on different runner hardware, so those times are not comparable.

## Verdict

(b) does what it should for the hydrology: the lower of two twins is no longer a lake or a
sink, and the remaining pits of shared samples are real and can fill. It is on by default.
It does not lower the total endorheic share (#172) and does not change the visible relief.
