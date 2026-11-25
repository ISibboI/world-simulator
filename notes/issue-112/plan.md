# Plan: one-to-one resampling (#112) and deep single-cell pits (#129)

_2026-11-25. Stacked on PR #163 (non-rigid plates, branch `agent/issue-70`). Goal: an Earth-like
planet, not Earth; effects of the ~110 km grid are accepted (#64)._

## Validity on top of `agent/issue-70` (a7523d5)

Measured on the 8-seed × 1000 Myr runs of #70 (`final-branch2`, branch 6adca63, identical
`src/` to a7523d5), snapshots 500–1000 Myr, script [`scripts/pits.py`](scripts/pits.py). Land =
cells not connected to the ocean (`ocean` = 0 in the cell dump); a pit is a land cell more than
1 m below all its neighbours.

| | mean (seed range) |
|---|---|
| cells showing a duplicated sample | 15.6% (15.0–16.1); up to 4 cells per sample |
| single-cell land pits | 1.53% of land (1.43–1.76) |
| … with an adjacent same-sample twin | 57% (41–66) |
| median pit depth | 9.5 m |
| pits deeper than 500 m | 8.8 per snapshot (0.07% of land), 63% of them below sea level |
| … on duplicated samples | 0% |
| … next to another plate | 46% |

**#112 is valid**: duplicates and twin pits are as on `main` before #70 (#57: 16%, 55–60%).
**#129 is valid**, and the deep pits are a different mechanism: none sits on a duplicated
sample. 97% of them show crust more than 3 km thinner than every neighbour
([`scripts/deepclass.py`](scripts/deepclass.py)): a single thin sample (oceanic 7–12 km, or
thinned continental 15–25 km) inside thick crust, often with kilometres of sediment that cannot
fill it. So (a) cannot remove them; their sources must be found (below).

## Why duplicates exist (recap of #57)

A plate's crust samples sit at the vertices of its own copy of the icosphere, rotated with the
plate. The icosphere's cells range from 0.77× to 1.21× the mean area, and the two meshes are
offset, so nearest-vertex resampling shows some samples at 2–4 cells and leaves others hidden.
A rotated sphere in Python ([`../../agent-scratch` experiment, summarised here]): 13–16% of
cells show a duplicated sample, 7–8% of the samples are hidden, and a duplicate cell is a median
2.4 cell spacings (p90 7) from the nearest hidden sample. A maximum bipartite matching between a
plate's cells and its samples, with edges up to 1.6 spacings, leaves only the plate-level
deficit unmatched (cells − samples, 0–2% for caps of 15–60°).

## (a) One-to-one resampling: design

After ownership resolution (matching, consumption, gap filling) a new pass
`matching::show_distinct_samples` changes which sample of its plate a cell shows, never which
plate owns it:

1. Per plate, of the cells showing one sample, the cell nearest to the sample keeps it; the
   others are orphans.
2. Each orphan (in cell order) searches breadth-first for an augmenting chain: a sample of the
   plate within `distinct_radius` = 1.6 spacings of the cell that no cell shows ends the chain;
   a sample shown by another cell of the plate within that radius lets that cell continue the
   search. At most `distinct_depth` = 12 steps; nearest samples first, ties by index
   (deterministic, no hash maps). The chain is flipped: every cell on it takes the next one's
   sample, the last takes the hidden sample.
3. An orphan without a chain (the plate covers more cells than it has samples nearby) keeps
   showing the shared sample, as before. Expected: ~1–2% of cells.

So every cell shows a sample at most ~1.6 cell spacings from it (most within one), and hidden
samples surface where they are near. Consequences:

- **Pits.** Twins disappear except for the residual; each cell's column gets its own erosion and
  deposition, so a low cell can fill. The #57 residual mechanism (mean change for shared
  samples, rest to hidden samples) stays for the residual duplicates.
- **Mass.** No crust is created or destroyed: the pass only reassigns cells to existing samples.
  Crust that was hidden becomes visible, so the visible continental area changes slightly; the
  ledger is unaffected.
- **Moving plate edges (#70).** Edge samples created and removed by the sheet's fronts and the
  owner redirects of vacated samples happen before the ownership resolution of the substep,
  which rematches everything and runs the pass at its end, so they stay in sync. Plate events
  after the substeps (splits, merges) keep each cell's sample (a merge's spread case can share
  one); they are not deduplicated until the next substep.
- **Matching.** The incremental matching and the hidden-sample tracking use the nearest-vertex
  matches (`found`), which the pass does not change, so they stay exact. `lookup_cache` keeps the
  nearest vertex.
- **Parameters:** `--set matching.distinct=0` restores the old resampling bit for bit;
  `matching.distinct_radius`, `matching.distinct_depth`.
- **Runtime:** one pass over the cells and a short search per orphan (~8% of cells) per
  substep.

## #129: deep single-cell pits

Find their sources by tagging, in a diagnostic build, each sample with how and when its crust
was created (initial, new sea floor, reopened consumed slot, rift stretching, gap heal, sheet
front, merge) and logging every new pit deeper than 500 m with the tag of the sample it shows
and of its neighbours. Then fix at the source where a process makes a one-cell hole that no
physical process would make, or, if the sources are many, add a physically motivated smoothing
(e.g. lateral flow of the lower crust towards a sample much thinner than all its neighbours in
the same plate). Decide after the diagnostic.

## Evaluation

Base: `final-branch2` (#70's 8 seeds × 1000 Myr, `--snapshot-interval 50 --width 512
--cell-dump`). Branch: same arguments. Metrics: duplicates, pits (count, depth, twins, deep),
closed-basin and lake area (log), endorheic share and share into single-cell pits, relief and
hypsometry, the #70 guard rails (`notes/realism-batch-4/scripts/compare.py`), and lake-cell
persistence between snapshots (needs a lake column in the cell dump and short snapshot
intervals over a window; relevant to #160). Runtime at 6 subdivisions: paired 200 Myr
single-core runs, base vs branch.
