# #113: crustal flow and subduction erosion act once per sample — plan

Follow-up of #57 / PR #104 (see [../issue-57/results.md](../issue-57/results.md)).

## Background: how #104 counts a duplicated sample once

A crust sample (one local vertex of a plate) can be shown by k > 1 world cells (11% of the
cells since #112's distinct resampling, 16% before). The books (mass ledger, sea level) count
every sample as one cell of area. #104 used two rules:

1. **Surface changes** (`Simulation::apply_surface_changes`): the sample gets the **mean** of its
   cells' changes, so summed over its k cells the shown change equals the cells' own changes.
   Erosion and deposition are a transfer (eroded mass is deposited elsewhere), so the residual
   (k − 1) × mean is moved as mass to the plate's hidden samples to conserve mass.
2. **Profile additions** (`apply_additions`): a sample takes part **once** (at its nearest ring).

The common rule: *a sample's change is that of one of its cells, not the sum over its cells.*

Two paths still sum over cells:

- **Crustal flow** (`crustal_flow_inner`, boundaries.rs): flows are computed per pair of adjacent
  cells and summed per sample. A sample at k cells has the edges of k cells, so it gives (or
  takes) up to k× what one cell would; the partner cell gets the full per-edge amount.
- **Subduction erosion** (in `resolve_ownership`): every consumed lower-plate sample pushes the
  overriding `winner` sample onto `eroded`, and each push thins it by `subduction_erosion`
  (300 m). A winner shown at k trench cells is consumed against k times and thinned k×.

## Change

### Crustal flow: scale each cell-pair transfer by 1 / max(k_source, k_receiver)

Count per sample the cells showing it (`self.owners`, once per call; dense `u8` per sample like
the existing `outflow`/`inflow` vectors). The requested amount of a transfer between two cells is
divided by the larger multiplicity of its two samples.

- For a duplicated sample next to non-duplicated ones, its total exchange becomes the **mean** of
  its cells' exchanges, which is rule 1 of #104 (and never more than that if both sides are
  duplicated).
- Mass stays conserved exactly and locally: a transfer gives the receiver exactly what it takes
  from the source, as now. No residual is left over, so nothing has to go to hidden samples.
- Why not the residual-to-hidden-samples route of rule 1: flow is a local, pairwise exchange.
  Hidden samples lie anywhere on the plate (#57 diagnostic: usually far away), and feeding or
  draining them would teleport orogen crust across the plate. The cost of the pairwise rule is
  that the non-duplicated partner exchanges 1/k of its per-edge amount over that edge. This is
  also what the geometry suggests: the duplicated sample stands for a larger area, its centre
  lies further from the partner, so the gradient (and the flux) towards it is lower.
- The existing caps (a sample gives at most its crust above the floor, takes at most its room
  below the cap; transfers scaled down together) stay and work on the scaled amounts.
- Hole filling (#129) is part of the same loop and gets the same factor; a hole stays a hole
  until filled, decided per step, so it only fills more slowly where a duplicated sample feeds it.

### Subduction erosion: the mean over the cells showing the winner

Collect the erosion events per winner sample (count n, a `crate::hash::HashMap` with an
insertion-ordered `Vec` of the samples for a defined iteration order). Count the multiplicity k
of each of these samples in `new_owners` as it stands at that point (the same nearest-vertex
assignment that produced the events, i.e. before `show_distinct_samples` and gap filling; k ≥ 1
since a winner shows at its contact cell). Thin each sample once by `n / k × subduction_erosion`
(sediment first, floor as now). Thinning with the sum in one go equals n sequential thinnings, so
for k = 1 the result is the same as now (up to rounding).

- n / k is the mean per-cell erosion over the cells showing the sample (rule 1). A sample shown at
  k trench cells with one event each is thinned once (300 m), not k×.
- Subduction erosion is a sink into the mantle; there is no transfer to conserve, the ledger
  books what is removed. So no residual is moved to hidden samples, and the effective rate drops.
- The loop moves into a function (`erode_overriding_samples`) so it can be unit-tested with a
  constructed duplicate.

### Statistics for the effective rates

A cumulative `SharedSampleRates` (like `SurfaceSharing`): crustal flow requested mass with and
without the 1/k factor, and subduction-erosion events n against the applied n/k. One log line per
snapshot next to the surface-sharing line, so the 8-seed runs report the rate change directly.

### Determinism

Only integer counts, divisions and sums in fixed order (cell order, insertion order); no
transcendental math, no std hash iteration. `--web-math` unaffected.

## Expected effect on rates

- Crustal flow: edges touching a duplicated sample ≈ 1 − 0.89² ≈ 20% of edges, halved (k = 2
  mostly), so roughly −10% of the requested flow; more where orogens are duplicated more often.
  Crustal flow is self-limiting (it equalises elevations), so thick crust stays a bit thicker:
  crust > 50 km and orogen heights may rise slightly.
- Subduction erosion: duplicated winners × (1 − 1/k), perhaps −5…−10% of the eroded mass; slightly
  more continental crust survives at margins: continental area and the continental balance may
  rise slightly.

## Tests

- Crustal flow: a thick source sample shown at two cells collapses onto its neighbours; its loss
  equals the mean of the losses its two cells would have as separate samples (compare against
  the same setup without the duplicate), mass conserved to 1e-12; and a receiver sample shown
  twice next to a source gets the share of one cell.
- Subduction erosion: a winner pushed at two cells that both show it is thinned once
  (`subduction_erosion`); a winner at one cell with two events twice; one event over two cells
  half; ledger equals the removed mass.
- Existing crustal-flow, hole and subduction tests stay green.

## Calibration check

8 seeds × 1000 Myr, default parameters, branch vs main (same main commit). Report crust > 50 km,
mean land, land fraction, continental area and balance, orogen heights (highest point, land
p99), and the rate factors from the new log line, with Welch t. If something leaves its
plausible range, look for the physical cause before touching `collapse_diffusivity` (1e4 km²/Myr,
physically derived from channel flow, Clark & Royden 2000) or `subduction_erosion` (300 m per
subducted cell, from Earth's recycling ≈ arc production); check in git history whether either
was fitted while the double counting was present.

## Revisions after the plan review

The independent review is in [review-plan.md](review-plan.md). Acted on:

- **F2:** multiplicities are `u32`, never 0 for an owner.
- **F3 (subduction erosion, owner map):** events are recorded as (contact cell, overriding
  plate). The erosion now runs after `show_distinct_samples` and gap filling, erodes the sample the
  contact cell finally shows (same plate; matching changes only which sample of the plate a cell
  shows), and counts k in the final `owners`, the map that crustal flow and
  `apply_surface_changes` use. Otherwise distinct resampling would hand most twin cells another
  sample and the winner would be thinned 1/k although it shows at one cell.
- **F4 (lost erosion flux):** the review is right that n counts subducted slab area, so the
  total erosion n × 300 m is physical and only its distribution on one column was wrong. Cutting
  it would make the erosion per subducted sample depend on resampling statistics and break its
  balance with arc growth (which `apply_additions` adds in full). So the rest
  `(1 − 1/k) n × amount` is taken in equal parts from the sample's neighbours with crust on its own
  plate (the adjacent forearc), its option (b). The sample itself is thinned once by n/k ×
  amount. Without neighbours the sample takes all of it (as before). The effective erosion rate
  is thereby unchanged by construction; its distribution changes.
- Every sample is thinned once with the sum of what it gets (sorted by (plate, local)), which
  equals sequential thinnings (sediment first, monotone floor).
- **F6/F7 (tests):** the crustal-flow equality test pins one sub-cycle and slack caps; extra
  cases for both sides duplicated and cross-plate edges; erosion tests go through the owner map,
  with a non-integer share and ledger/cover checks.
- **F9:** the log line reports requested flow per cell and per sample, and the erosion shares;
  the applied flow is the ledger-neutral exchange and is not logged separately (the requested
  ratio is the rate factor the change introduces; caps act on both the same way).

Not done: the dense `Vec<u32>` instead of a `crate::hash::HashMap` for the erosion events: the
events are few (trench cells per substep), so a map with a defined (sorted / insertion) order is
cheaper than a per-substep vector over all samples.
