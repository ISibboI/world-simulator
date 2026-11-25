# Results: one-to-one resampling (#112) and deep single-cell pits (#129)

_2026-11-25. Stacked on PR #163 (`agent/issue-70`). Plan and its revisions: [plan.md](plan.md);
screening variants: [screening.md](screening.md); scripts: [scripts/](scripts/). Goal: an
Earth-like planet, not Earth; effects of the ~110 km grid are accepted (#64)._

## What changed

1. **Distinct samples (#112, option a)** — `matching::show_distinct_samples`, run at the end of
   every ownership resolution. Of the cells showing one crust sample, the nearest keeps it;
   each other cell gets a sample of its plate that no cell shows, within 1.6 cell spacings,
   through a short chain of cells passing their samples on (≤ 6 steps). Every cell on a chain
   must show a sample whose dry elevation is within 30 m of its nearest sample's. Only which
   sample of its plate a cell shows changes; no crust is created or moved. The sheet's moving
   plate edges (#70) act before the resolution, which rematches everything, so they stay in
   sync. `--set matching.distinct=0` gives the old resampling; `matching.distinct_radius`,
   `matching.distinct_depth`, `matching.distinct_tolerance`.
2. **Deep holes (#129)** — (a) a gap at a consumed slot away from other plates is healed from
   its neighbours like any resampling gap instead of becoming new 7 km sea floor; (b) **hole
   flow** in the crustal flow: a cell more than `hole_flow_depth` = 300 m below all its
   neighbours and with thinner crust than all of them takes crust from the continental
   neighbours of its plate at full mobility (lower-crustal flow; down to half a source's
   thickness per step), until it is no longer such a hole. Mass-conserving.
3. Analysis: a `lake` column (lake depth) in the cell dump and `--lake-log-from T` (the lake
   cells of every step from T on, in `lakes.csv`), for lake persistence (#160).

## Validity on `agent/issue-70` (before)

Duplicated samples on 15.6% of cells; 57% of single-cell land pits have a same-sample twin;
8.8 pits deeper than 500 m per snapshot, **none of them on a duplicated sample**: they are single
samples with crust > 3 km thinner than every neighbour. Both issues valid, with different
causes (details in [plan.md](plan.md)).

## Why the one-to-one resampling cannot be complete

The plate's local mesh and the world mesh differ in cell density by up to ±20%. Where the plate's
samples are sparser than the cells, there are not enough samples nearby: a full matching would
show samples displaced by many cells. Screening (v1, no elevation limit) showed that even
displacements of 1–1.6 spacings make **new, deeper pits on slopes** (depressions 309 → 411,
endorheic land 19 → 22%, pits > 100 m doubled) while removing the twins. With the 30 m
tolerance, a quarter of the shared cells get their own sample (25% of the cells showing a
shared sample per substep), mostly in flat terrain, where twins make pits.

## Before/after (8 seeds × 1000 Myr, window 500–1000 Myr)

Base = `agent/issue-70` tip (a7523d5; state hash identical at 1000 Myr), branch = 5332751, both
`--duration 1000 --snapshot-interval 50 --width 512 --cell-dump --lake-log-from 900`. Means over
snapshots, then seeds (seed range in brackets), Welch t. Pits: non-ocean cells > 1 m below all
neighbours. Lake persistence: from the per-step lake log, 900–1000 Myr.

| metric | base | final | t |
|---|---|---|---|
| cells on duplicated samples (%) | 15.6 (15…16.1) | 11.4 (10.7…12.1) | -17.7 |
| single-cell land pits (% of land) | 1.53 (1.43…1.76) | 1.19 (0.968…1.42) | -4.6 |
| pits with a same-sample twin (% of pits) | 56.5 (41…66) | 48.4 (41…53.8) | -2.4 |
| pit depth median (m) | 9.48 (7.5…13.2) | 9.45 (6.25…11.4) | -0.0 |
| pits > 20 m (% of land) | 0.459 (0.316…0.728) | 0.347 (0.197…0.508) | -1.8 |
| pits > 100 m (% of land) | 0.191 (0.11…0.375) | 0.0706 (0.0296…0.136) | -3.7 |
| pits > 500 m (per snapshot) | 8.76 (4.27…14.9) | 0.409 (0.0909…1) | -6.9 |
| single-cell land peaks > 100 m (% of land) | 1.08 (0.948…1.28) | 0.982 (0.848…1.1) | -1.7 |
| land neighbour |dE| (m) | 142 (112…190) | 146 (110…184) | +0.2 |
| land (% of surface) | 30.3 (29…31.5) | 29.5 (27.8…31.2) | -1.6 |
| mean land (m) | 762 (483…1.21e+03) | 883 (470…1.32e+03) | +0.9 |
| land p50 (m) | 488 (294…962) | 626 (260…1.15e+03) | +1.1 |
| land p90 (m) | 1.91e+03 (1.22e+03…2.6e+03) | 2.13e+03 (1.27e+03…2.79e+03) | +0.8 |
| land p99 (m) | 3.01e+03 (2.29e+03…3.53e+03) | 3.12e+03 (2.45e+03…3.82e+03) | +0.5 |
| ocean p50 (m) | -3.73e+03 (-3.82e+03…-3.61e+03) | -3.66e+03 (-3.73e+03…-3.58e+03) | +2.1 |
| lake cells (% of land) | 4.46 (3.73…5.22) | 4.32 (3.46…4.89) | -0.6 |
| lakes on single-cell pits (% of lake cells) | 27.9 (24.3…33.8) | 20.8 (18.7…22.8) | -5.8 |
| flooded continental crust (%) | 22.9 (18.2…28.8) | 22.4 (12…33.5) | -0.2 |
| highest point (m) | 4.01e+03 (3.47e+03…4.34e+03) | 4.17e+03 (3.78e+03…4.69e+03) | +1.1 |
| mean speed (cm/yr) | 4.06 (3.83…4.24) | 4.17 (3.39…4.72) | +0.7 |
| duplicated cells (%, log) | 15.6 (15…16.1) | 11.4 (10.6…12.1) | -17.7 |
| endorheic (% of land) | 19 (14.9…23.9) | 20.2 (12.4…29.6) | +0.5 |
| endorheic into single-cell pits (% of land) | 7.38 (6.24…9.1) | 8.18 (6…11.8) | +1.1 |
| lakes (% of land, log) | 3.85 (3.08…4.72) | 3.71 (2.9…4.28) | -0.5 |
| depressions | 309 (280…338) | 300 (254…338) | -0.7 |
| endorheic depressions | 147 (127…165) | 141 (111…166) | -0.8 |
| lake cells still lake after 1 Myr (%) | 40.8 (27.4…57.2) | 40.3 (20.2…49.6) | -0.1 |
| lake cells still lake after 10 Myr (%) | 14.7 (9.74…19.3) | 15.2 (7.78…25.3) | +0.2 |
| lake-cell run length (Myr, mean) | 1.73 (1.38…2.34) | 1.71 (1.24…2) | -0.2 |
| lake cells per step | 553 (403…670) | 569 (414…731) | +0.3 |
| run time (s) | 775 (703…869) | 814 (794…843) | +1.5 |

**Reading:**
- **#129 solved:** pits deeper than 500 m 8.8 → 0.4 per snapshot (−95%), > 100 m −63%. The
  screening variant with only the #129 changes (fixonly) gives the same reduction, so it does not
  depend on the resampling change.
- **#112 partly:** duplicated cells 15.6 → 11.4%, single-cell land pits −22% (t −4.6), lakes on
  single-cell pits 28 → 21% of lake cells. Twins still make 48% of the remaining pits: they sit
  where the plate has too few samples nearby or where taking another sample would change the
  elevation by more than 30 m.
- **Endorheic drainage into single-cell pits is unchanged** (7.4 → 8.2% of land, t +1.1, within
  the seed scatter), and so are the endorheic share (19 → 20%) and the lake area. In arid land,
  removing a pit sends the water to the next closed depression; the share depends on the
  climate and on all closed depressions, not on this one pit type.
- **Lakes flicker as before (#160):** only ~40% of lake cells are still lake one step (1 Myr)
  later, ~15% after 10 Myr, a lake cell lasts 1.7 steps on average — in both runs. The
  resampling is not what makes them flicker.
- **Guard rails:** land area, mean land, hypsometry, flooded continental crust, plate speed,
  highest point: all within |t| < 2.2 (ocean median +70 m, t +2.1, within the chaotic scatter
  expected over ~30 metrics).

## Sources of the deep holes (diagnostic)

A scratch build tagged every sample with how its crust was created ([scripts/origins.py](scripts/origins.py);
seed 0, 300 Myr, with distinct samples). Deep pits came from: reopened consumed slots inside a
plate (new 7 km sea floor, ~27%), the sheet's advancing fronts (the outer sample gets about a
third of the column, ~33%), merges (absorbed samples placed at the nearest free vertex, ~33%),
rift stretching and gap heals. Only the first is a wrong rule (fixed); the others are
legitimate mass-conserving operations that leave one thin column, which Airy isostasy per cell
then lets sink. On Earth the lithosphere's flexural strength and lower-crustal flow would not
let one ~110 km wide column sit 500+ m below all its surroundings: a grid effect, handled by
the hole flow.

## Runtime (6 subdivisions)

Paired 200 Myr runs, seed 0, one core each, started together on tktie (loaded, load average ~7):
base 143.4 s, branch 142.6 s (0.71 s/Myr under that load; #70 measured 0.47 s/Myr on a quiet
machine). The distinct-samples pass took 6.4 s of the branch run (~4.5%); the trajectories
differ, so the totals also differ by the planet. The hole detection is one pass over the cells
per step. The desktop still handles 6 subdivisions.

## Caveats

- One-to-one is partial (11% of cells still share a sample) by construction: a complete one
  would need samples that are not tied to the plate's mesh vertices.
- Cells may show a sample up to 1.6 spacings away (within 30 m of their nearest sample's
  elevation).
- The hole-flow depth (300 m) is a choice, not calibrated; deeper single-cell depressions can
  still exist briefly (until the next step's crustal flow).
- Hole flow can move a source below the 40 km threshold of orogenic collapse (it gives down to
  half its thickness into a hole).
