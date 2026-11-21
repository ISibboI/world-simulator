# #57: crust samples shown by two cells get both cells' erosion and deposition — plan

## Problem

Each world cell shows one crust sample (`Simulation::owners`); after resampling a sample can be
shown by k ≥ 2 cells ("duplicated"), and about as many samples are shown by no cell ("hidden").
Erosion computes a bedrock and a sediment change per **cell**. `apply_surface_changes` adds the
changes of all k cells to the one sample, so every one of the k cells shows the **sum**: k times
the erosion or deposition of a cell. This conserves mass (each cell's change lands on exactly
one sample), but distorts the surface: 57–59% of the single-cell land pits of #46 sit on
duplicated samples against 15–16% of land cells (realism-batch-1 evaluation).

## Why the sum was chosen, and what must still hold

Masses are kept per sample (areal mass × one cell area, `ledger::plates_mass` sums all samples,
hidden ones too). A duplicated sample stands for k cells of world area but holds the mass of
one; a hidden sample holds the mass of one cell but no world area. Erosion is mass-conserving on
cells (what one cell loses, others gain, all on cells). Applying only the mean change to the
duplicated sample would lose (k−1)·mean of the moved mass. So the rest has to go to samples
without world area: the hidden ones. That is what the comment "about as many samples are hidden
by resampling as are shown twice" assumed, but the code never did it.

Where do duplicates come from? Nearest-vertex resampling of a rotated plate: where one local
vertex is nearest to two world cells, a neighbouring local vertex is usually nearest to none
(aliasing). So the hidden partner is typically a **local mesh neighbour on the same plate**,
lying under one of the k cells.

## Diagnostic before coding: duplicates and hidden samples are not local pairs

The plan review (`review-plan.md`, F2–F4) asked to check the "hidden partner is a local
neighbour" assumption. A diagnostic (default parameters, 6 subdivisions, 20 plates, seed 1,
60 Myr of tectonics) says it is wrong:

| step (Myr) | duplicated samples | extra cells | hidden samples (not under another plate) | partners within ring 1 / 2 / 3 (rounds, each hidden sample once) | none within 11 rings |
|---|---|---|---|---|---|
| 10 | 659 | 676 | 498 | 134 / 57 / 24 | 312 |
| 30 | 1550 | 1598 | 1235 | 337 / 139 / 98 | 703 |
| 60 | 2189 | 2238 | 1842 | 526 / 234 / 157 | 902 |

- ~93% of the duplicated cells are true nearest-vertex duplicates (both cells' nearest local
  vertex is the sample); ~7% are gap fills next to consumed vertices.
- Only ~15% of the hidden samples lie nearest to a cell that shows a duplicated sample (rule F4).
- Duplicates grow from 0 at the start to ~5% of cells after 60 Myr. They are mostly not
  aliasing between neighbours but most likely a regional density mismatch: the icosphere's cells differ in
  size, and a rotated plate lays a region of its (fixed, local) mesh with large cells over world
  cells that are small (duplicates there) and elsewhere the opposite (hidden samples there).
  The hidden counterpart of a duplicate is often hundreds or thousands of km away.

With true sample areas this would close by itself (a sample covering two cells has twice the
area, so the mean change over its cells is exact). The books treat every sample as one cell of
area, so the mass of the k−1 extra cells has no home near the duplicate.

## Rule (revised after the diagnostic)

Per call of `apply_surface_changes`:

1. Count per sample the cells showing it (`shown[sample]`).
2. Every sample gets the **mean** of its cells' changes (k = 1: its cell's change, as now).
   Each cell then shows (about) its own change, never a multiple.
3. The **residual** of a duplicated group, (k−1) × mean, is kept as **mass** (review F1):
   bedrock mass = bedrock metres × the shown sample's density (what the erosion used),
   sediment as rock-equivalent metres. Per plate and per class (column of continental origin
   or not, review F6) the residuals are summed.
4. The residual of a plate and class goes to that plate's **hidden samples of the same
   class** whose position is not under another plate (nearest world cell owned by the plate;
   the search starts from the cell of a shown local neighbour), in three parts with their own
   weights (second look S2/S3): removed bedrock mass in proportion to the bedrock removal at
   the cell the hidden sample lies under, deposited sediment in proportion to the deposition
   there, removed sediment in proportion to the sediment removal there; equal shares where no
   such cell has that kind of change. So a hidden sample takes about the change of its own
   place, not the plate's mean (a hidden lowland sample does not take mountain incision).
   Bedrock mass is turned into metres with each recipient's density.
5. Fallback, counted: no hidden sample of that class on the plate → the residual stays on the
   duplicated samples (the old sum for them). No transfer to the other class (second look S4:
   thinning mafic crust for felsic erosion distorts the felsic budget).
6. Sediment removal beyond the available sediment comes from the bedrock at the same mass,
   per recipient, as now. Thinning clamps at zero thickness (pre-existing); clamped mass is
   counted. Cumulative totals (gross change, part to hidden samples, part kept, clamped) are
   logged with every snapshot.

Mass is conserved exactly: the applied mass equals the per-cell sum of the erosion result.
Not local in general: the residual moves within the plate, to hidden samples weighted by
their own place's change. That is the price of equal-area books; the
alternative with locality (matching duplicates to hidden samples nearest-first) leaves ~40%
unmatched within 11 rings and would put full shares on single hidden samples.

Considered and rejected: area-weighted books (true Voronoi areas per sample) would remove the
mismatch at its source, but change every mass, ledger and sea-level computation; one-to-one
resampling (review F11) touches matching and consumption. Both are follow-ups.

### Bookkeeping

- Mass ledger (`Term::SurfaceProcesses`): unchanged, the sum of per-sample mass changes, now
  equal to Σ cells (bedrock × ρ_shown + sediment × ρ_sed) up to clamping.
- Material budget (felsic, sediment): recomputed before/after as now; the per-class pools keep
  felsic bedrock erosion on felsic hidden samples.
- Determinism: samples and plates are visited in index order; no hash maps.
- Isostasy: per column, nothing lateral.
- Sea level: `SeaLevelBudget::sharing` (cell prediction vs. sample solve) should shrink;
  changes of hidden samples appear later as resampling effects.

## Other per-cell → per-sample write paths (checked)

- Climate, temperature, ice: write no crust (only `apply_surface_changes` from `planet.rs`).
- Initial sediment (`surface_crust_mut`): runs while every sample is at its own cell (identity
  `owners`), and sets rather than adds. No duplication.
- Metamorphism, healing, trench sediment: per sample, no cells involved.
- Plume reheating (`reheat_above_plumes`): per cell but idempotent (sets a thermal age), so a
  sample at two cells is reheated once in effect. No bug.
- Crust additions (`apply_additions`): collision bands already count a sample once (test
  `samples_shown_at_several_cells_take_their_band_share_once`), but the **non-band profile
  path** (arcs, rifts/accretion using `profile`, and collisions without band) gives a duplicated
  sample one profile weight per cell → each of its cells shows double thickening. Same bug
  class. Fix: use the `first_seen` de-duplication for every addition (a sample takes part once,
  at its nearest ring). Mass stays conserved (the weights are normalised).
- Subduction erosion (review F9): a winner sample shown at k trench cells is thinned about k
  times. Listed, not fixed (needs a calibration look).
- Crustal flow (`crustal_flow_inner`): a duplicated sample exchanges crust across the edges of
  all its k cells while holding the mass of one, so it diffuses up to k× faster. Mass is
  conserved and diffusion is self-limiting (each pair moves at most half of what equalises
  it), so this smooths rather than making pits or spikes. **Listed, not fixed** here (a fix
  changes the effective diffusivity of the orogenic collapse and needs its own calibration
  check); mentioned in the PR.

## Tests

- Replace `shared_samples_get_the_sum_of_their_changes`: a sample shown by two cells gets the
  mean (−10 m and −20 m → −15 m), the residual goes to the plate's hidden samples of the same
  class, total mass change equals the per-cell sum (with recipients of a different density).
- No hidden sample on the plate → the group keeps the sum; mass conserved.
- A hidden sample under another plate gets nothing; consumed/`None` vertices stay `None`.
- Sediment removal larger than the available sediment on a group: bedrock removed at the same
  mass, ledger closes (extend `crust_mass_ledger_closes`, which already has shared samples).
- Additions: an arc/profile addition with a duplicated sample gives it the share of one cell.
- Determinism test stays green.

## Measurement

8 seeds × 1000 Myr, `main` vs branch, default args plus `--material-budget --cell-dump
--snapshot-interval 100`, Welch t over 500–1000 Myr (log metrics with the realism-batch-1
`compare.py`; pits from the cell dumps, 600–1000 Myr like the batch-1 evaluation):
- single-cell land pits (% of land, a land cell > 1 m below all neighbours) and their share on
  duplicated samples (`dup` > 1) vs. that share of all land cells; target: the pit share on
  duplicated samples falls towards the land share;
- endorheic share, endorheic into single-cell pits, lakes (% of land); mean land, land %,
  highest point, relief (land p90/p99), flooded continental crust;
- sediment to ocean, sediment part of the sea level, sea-level budget `sharing`, felsic +
  sediment net, ledger residual;
- residual statistics (duplicated samples, residual mass, fallbacks, clamped mass);
- single-cell land peaks (> 1 m above all neighbours), review F7;
- runtime (paired, local placement);
- guard rails: plates, speeds, sea-floor age, temperature, precipitation, runoff.
The arc-addition de-duplication is a tectonic change in its own commit; a run of the surface
rule alone (8 seeds) attributes the effects (review F9). A go/no-go of 2 seeds × 200 Myr checks
runtime and fallbacks before the sweep. Pass criterion for #57: the share of single-cell pits on
duplicated samples falls to within a few points of the share of duplicated land cells, with a
significant drop (Welch t < −3) of single-cell pits.

## Interaction

#58 (inland depressions below sea level → lakes, worked on in parallel on agent/issue-58)
changes the same pit/lake/endorheic metrics through the drainage code. Whichever lands second
rebases and re-measures briefly. For #46: if single-cell pits drop as expected, the remaining
endorheic share is the physical part.
