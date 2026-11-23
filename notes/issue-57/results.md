# #57: duplicated crust samples — results

Plan: [plan.md](plan.md) (with the diagnostic that changed the rule); plan review:
[review-plan.md](review-plan.md); independent results review:
[review-results.md](review-results.md) (of the intermediate `full` state; its findings F4, F5,
F8 and F10 were acted on, see "After the review").

## What changed

1. `Simulation::apply_surface_changes`: every crust sample gets the **mean** of the
   erosion/deposition of the cells showing it, so each cell shows its own change (summed over the
   k cells: exactly their changes) instead of k times it. The residual of a sample shown at k
   cells, (k−1) × mean, is moved **as mass** to the plate's **hidden** samples of the same class
   (column of continental origin or not) that lie under the plate's own cells. It goes in three
   parts (bedrock removal, deposition, sediment removal), each weighted by the same kind of
   change at the cell the hidden sample lies under. A hidden sample takes at most two extra
   cells' worth of a part per step, and the excess is handed on to the others. What none can take
   stays on the duplicated samples (0.4% of the gross change). Mass is conserved exactly. The
   cumulative totals are logged with every snapshot ("surface changes of duplicated samples").
2. `apply_additions`: arcs, accretion and collisions without a band gave a duplicated sample
   one profile weight per cell. Collision bands already avoided this, and now every addition does
   the same: a sample takes part once, at its nearest ring.
3. `src/viewer/pace.rs`: uses `crate::hash::HashMap` instead of std's. This is unrelated: Clippy
   on `main` fails on it since #78 and #87 crossed, and the fix is needed for a clean CI.

Other per-cell → per-sample paths I checked:
- Climate, temperature and ice write no crust.
- Initial sediment runs before any resampling.
- Metamorphism and healing work per sample.
- Plume reheating is idempotent.

**Listed, not fixed:**
- Crustal flow: a duplicated sample exchanges crust over the edges of all its cells, so it
  diffuses up to k× faster. The effect is self-limiting.
- Subduction erosion: a winner sample at k trench cells is thinned about k times.

Fixing either would change effective rates and needs a calibration look.

## Why duplicates exist (diagnostic, plan section "Diagnostic before coding")

About 16% of the cells show a duplicated sample after the first ~100 Myr, with at most 3 cells per
sample. These are not local aliasing pairs. The icosphere's cells range from 0.77× to 1.21× the
mean area (plain midpoint subdivision). A rotated plate therefore lays parts of its fixed local
mesh with large cells over small world cells, which gives duplicates, and elsewhere the reverse,
which gives hidden samples. The hidden counterpart is usually far away. That is why the residual
goes to hidden samples across the whole plate instead of to a neighbour.

## Measurement

8 seeds × 1000 Myr with default parameters, against `main` (67ca0a2), with Welch t vs `main`:
- `final`: the branch head, c581d87;
- `surface`: change 1 without the cap, alone;
- `full`: changes 1 and 2, without the cap; this is what the results review evaluated.

An intermediate run with a plain cap (38ab4ee, no hand-on) kept two thirds of the residual on
the duplicates: 2.9% to hidden, 5.4% kept, and the sharing term back at +13.5 m/100 Myr. Its
data was discarded.

Log metrics are means over the snapshots in the 500–1000 Myr window. Pits come from the cell
dumps at 600…1000 Myr, as in the realism-batch-1 evaluation: a pit is a land cell more than 1 m
below all its neighbours. Scripts are in [scripts/](scripts/): the realism-batch-1 `compare.py`
with new metrics, plus `pits.py` and `twins.py`. Full tables are in [compare.txt](compare.txt)
and [pits.txt](pits.txt).

### Pits and the surface

| metric | main | final | t | surface | t | full | t |
|---|---|---|---|---|---|---|---|
| single-cell land pits (% of land) | 1.38 (1.20…1.63) | 1.21 (1.09…1.32) | −2.9 | 1.33 | −0.8 | 1.32 | −0.8 |
| pits on duplicated samples (%) | 54.9 (49.1…61.4) | 60.1 (50.8…70.4) | +1.9 | 56.6 | +0.9 | 57.2 | +1.0 |
| land cells on duplicated samples (%) | 16.7 | 16.2 | −1.3 | 16.2 | −1.6 | 16.1 | −1.6 |
| single-cell land peaks > 1 m (% of land) | 4.72 | 4.63 | −0.5 | 4.82 | +0.7 | 4.69 | −0.2 |
| peaks on duplicated samples (%) | 26.4 (21.5…29.6) | 23.9 (21.9…25.8) | −2.3 | 23.0 | −3.1 | 22.8 | −2.7 |
| land peaks > 100 m above all neighbours (% of land) | 0.94 | 1.00 | +1.2 | 1.09 | +2.9 | 1.05 | +1.4 |
| land cells, sediment > 2 km above every neighbour (per 10⁴) | 90.8 | 85.2 | −1.1 | 103.1 | +1.5 | 100.9 | +1.3 |
| land cells, crust > 3 km thinner than every neighbour (per 10⁴) | 78.4 | 71.3 | −1.3 | 88.6 | +1.2 | 87.0 | +1.1 |
| land neighbour \|dE\| (m) | 185 | 183 | −0.2 | 187 | +0.2 | 191 | +0.6 |
| land p90 / p99 (m) | 2453 / 4035 | 2312 / 3922 | −0.6 / −0.6 | 2244 / 3884 | | 2221 / 3951 | |
| endorheic (% of land) | 15.9 (13.4…21.2) | 12.9 (9.2…17.4) | −2.3 | 15.1 | −0.4 | 16.5 | +0.3 |
| endorheic into single-cell pits (% of land) | 7.16 (5.80…10.30) | 6.58 (4.82…9.54) | −0.7 | 7.12 | −0.1 | 7.90 | +0.9 |
| lakes (% of land) | 2.92 | 2.63 | −1.2 | 2.67 | −1.0 | 2.94 | +0.0 |
| depressions (lakes) | 285 | 250 | −2.3 | 262 | −1.4 | 271 | −0.9 |

**The pits on duplicated samples did not go away.** The doubled change was real, but it is not
what makes these pits. Two analyses support this: `twins.py`, and the results review's own
analysis (F2), which found the same.
- Every pit on a duplicated sample has its twin, the other cell showing the same sample, as a
  neighbour. The twin is higher by exactly the difference in dynamic topography between the two
  cells (to within 0.1 m; median 15–25 m), because both cells show the identical crust column.
- The depth to the spill point is median 5–6 m and is **never larger than that
  dynamic-topography difference**.
- The low member of a twin pair is a pit 8–12% of the time, against 0.6–0.8% for other land
  cells.

So the two cells' elevation difference is **locked** to their dynamic-topography difference.
Any deposit in the low cell is shared with its twin, and the pair rises together, so the low cell
can never fill. This happens wherever the local relief is smaller than that offset. It applies
under the sum and under the mean alike, because both give the pair one common change. It is a
resolution limit of nearest-vertex resampling.

The seed spread allows a firm but bounded statement: a pit reduction of more than ~15–20% from
the doubling is excluded. Single-cell pits overall fall by 12% in `final` (t −2.9), but not on
the duplicated samples, so not through this mechanism. The endorheic share falls 15.9 → 12.9%
(t −2.3) in `final` only; it is not robust across the three variants.

What the fix does remove: peaks on duplicated samples drop from 26% to 23–24% (t −2.3 to −3.1).
The sum over-raised and over-lowered single shared columns.

### Bookkeeping and guard rails

| metric | main | final | t | surface | t | full | t |
|---|---|---|---|---|---|---|---|
| sea-level budget "sharing" (m/100 Myr) | +29.4 (23.8…38.0) | −6.6 (−8.1…−4.0) | −20.7 | −9.1 | −22.5 | −8.1 | −21.9 |
| surface change moved to hidden samples (% of gross) | — | 7.95 (7.3…8.5) | | 8.49 | | 8.44 | |
| surface change kept on duplicates (% of gross) | — | 0.41 | | 0.10 | | 0.12 | |
| surface processes felsic / sediment (Gt/yr) | −8.77 / +9.46 | −8.48 / +9.21 | +0.5 / −0.4 | −8.74 / +9.45 | | −8.88 / +9.69 | |
| felsic + sediment net (Gt/yr) | −1.93 | −1.64 | +0.6 | −1.70 | +0.5 | −1.93 | −0.0 |
| sediment part of the sea level (m) | 230 | 237 | +1.1 | 231 | +0.2 | 224 | −0.7 |
| sediment to ocean (km³/yr) | 4.67 | 4.73 | +0.2 | 4.75 | +0.2 | 4.88 | +0.7 |
| land (% of surface) | 29.7 | 28.3 | −2.1 | 28.7 | −1.8 | 29.1 | −0.8 |
| mean land (m) | 824 | 764 | −0.9 | 784 | −0.5 | 778 | −0.7 |
| highest point (m) | 5027 | 4891 | −1.2 | 4839 | −1.4 | 4952 | −0.5 |
| flooded continental crust (%) | 22.5 | 28.9 | +2.9 | 26.8 | +1.9 | 25.5 | +1.0 |
| shelf area (% of surface) | 5.12 | 6.10 | +2.3 | 5.56 | +1.0 | 5.34 | +0.4 |
| crust > 50 km (% of continental) | 5.05 | 3.80 | −1.4 | 3.85 | −1.2 | 3.78 | −1.3 |
| burial metamorphism on continental crust (Gt/yr) | 0.14 | 0.11 | −1.6 | 0.11 | −1.3 | 0.09 | −2.6 |
| dynamic topography max (m) | 1195 | 1111 | −2.2 | 1173 | −0.4 | 1090 | −4.2 |
| plates > 1% | 15.4 | 17.3 | +1.7 | 17.3 | +1.9 | 17.7 | +2.1 |
| rifts /100 Myr | 1.88 | 2.12 | +0.7 | 2.52 | +2.6 | 2.40 | +2.1 |
| duplicated cells (%) | 15.95 | 15.65 | −1.6 | 15.86 | −0.7 | 15.87 | −0.6 |
| temperature / land precipitation / runoff | 13.8 °C / 0.66 m/yr / 37572 km³/yr | 14.1 / 0.71 / 38085 | +1.1 / +1.1 / +0.3 | | | | |

- **Mass.** The ledger's surface-processes term stays below the 0.005 Gt/yr print threshold in
  every run, as on `main`. The ledger records what was applied, so that alone cannot prove
  conservation. The unit tests therefore compare the applied change with the change the cells
  asked for: with recipients of another density, at the cap, with the hidden sample under another
  plate, with no hidden samples, and with sediment removal beyond the available sediment. The
  erosion that found no bedrock comes to 1.5e-3 Gt over 1000 Myr.
- **Sea level.** The "sharing" term is the difference between the sea-level change the erosion
  predicts on the cells and the solve on the samples. It falls from +29 to −7 m per 100 Myr,
  because the cells now show the changes the erosion computed. The term does not see the ~8% of
  the gross change now stored in hidden samples; that shows up later as a resampling effect,
  inside the "tectonics" term. The remaining −7 m is not examined. Plausible causes are the
  averaging of two cells' changes across a coastline and nonlinear sediment porosity.
- **Guard rails.** The full table has 76 metrics ([compare.txt](compare.txt)). Plate tectonics
  diverges chaotically after the first changed step, so |t| ≈ 2–3 on a few metrics is expected.
  In `final` the largest shifts are:
  - flooded continental crust +6 points (t +2.9);
  - land −1.4 points of the surface (t −2.1);
  - shelf area +1 point (t +2.3).

  All three point the same way: slightly lower continents. Mean land (−60 m, t −0.9) and crust
  > 50 km (t −1.4) agree but are not significant. The dynamic-topography maximum is lower in every
  variant that contains the additions change: t −4.2 in `full`, −2.2 in `final`. With fewer
  thick arcs and orogens that would be plausible, but it is unexplained.
- **Runtime:** +2%, see below.

## After the review

The results review (of `full`) asked for four things, all done in 38ab4ee and c581d87:
- **F5:** cap the residual one hidden sample takes. It had found single hidden samples taking up
  to 12 km of sediment in one step when a plate had few of them. Now the cap is two extra cells'
  worth, and the excess goes to the others. With it, the anomaly counts above (sediment spikes,
  thin-crust pits, peaks > 100 m) are back at or below `main`, where in `surface` and `full` they
  had drifted up.
- **F4:** fix the class of a duplicated sample before its change, so that a class flip cannot
  lose or double its residual.
- **F8:** make the overridden-sample test assert unconditionally (it passed vacuously before),
  and add tests for the cap and for removing more sediment than there is.
- **F10:** tone down the wording here.

## Runtime

Paired local runs of `full`: seeds 0–3, 1000 Myr, `--material-budget`, no cell dumps, run side
by side on tktie.

| seed | main (s) | full (s) | change |
|---|---|---|---|
| 0 | 632 | 633 | +0.1% |
| 1 | 624 | 637 | +2.1% |
| 2 | 726 | 775 | +6.8% |
| 3 | 770 | 773 | +0.4% |

The mean change is +2.3%. The runs diverge chaotically after the first changed step, so part of
this difference is a different planet, not the code. The work added per step is small: one pass
over the samples and a nearest-cell search per hidden sample of plates with duplicates. The
hand-on pass in `final` adds at most 8 passes over a plate's hidden samples. The GitHub-hosted
8-seed runs are not comparable, because they ran on a mix of runner machines.

## Conclusion

- **Bookkeeping fixed.** #57's bookkeeping bug is fixed: a sample shown by k cells no longer gets
  k cells' change. Mass stays conserved, the sea-level "sharing" residual drops from +29 to −7 m
  per 100 Myr, and peaks on duplicated samples become rarer. Profile additions are
  de-duplicated in the same way.
- **Pits not caused by the doubling.** The single-cell pits on duplicated samples have another
  cause: twin cells showing one crust column, locked to their dynamic-topography offset, median
  ~6 m deep. They stay at 55–60% of the pits.
- **For #46.** Endorheic drainage into single-cell pits (6.6–7.9% of land) remains an artefact
  of resampling, now with the right cause. Two options:
  - **(a) One-to-one resampling.** A cell whose nearest local vertex is already taken would show
    the nearest free hidden vertex of the plate. This removes most duplicates and with them the
    locked twins, the faster crustal flow, the repeated subduction erosion and the need to store
    residuals in hidden samples. It changes matching and consumption, so it should be its own
    issue.
  - **(b) Drainage treats a pit and its twin as one node.** A pit whose adjacent twin shows the
    same sample would drain where the pair drains; equivalently, pits no deeper than the
    dynamic-topography offset to such a twin would be ignored. This is cheap and targets exactly
    the measured mechanism. The testable prediction is that the single-cell-pit share roughly
    halves. It touches the drainage code, which #58 is changing in parallel.
