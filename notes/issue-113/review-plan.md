# #113 plan review (independent)

Reviewed: `plan.md`, `../issue-57/results.md`, `crustal_flow_inner` and the subduction-erosion
loop in `resolve_ownership` (boundaries.rs), `apply_surface_changes` (mod.rs),
`show_distinct_samples` (matching.rs). Overall: the crustal-flow change is sound; the
subduction-erosion change has one real design problem (2) and one rate question (3).

## Crustal flow

1. **1/max(k_s, k_r) per cell-pair transfer is sound and conservative.** (OK.) Each
   transfer still debits the source and credits the receiver by the same `scaled` amount, so mass
   is conserved locally without residuals. Twin cells of one sample are already skipped
   (`neighbor_owner == owner`), so the factor only applies across real exchanges. For a
   duplicated sample next to singles, its total equals the mean over its cells (rule 1 of #104).
   Where both sides are duplicated, the side with the smaller k gets less than its own mean,
   which is the price of a symmetric factor, and the plan says so. Cross-plate edges: k is per
   sample, independent of plate, so no special case. The caps (`available/outflow`,
   `room/inflow`) act after the factor and only bind less often. Hidden samples (k = 0) never
   appear as owners, so there is no division by zero. Stability only improves (smaller
   effective diffusivity). Recommendation: apply the factor to `amount` after the
   `min(0.5 * equalising)` clamp (as written), and say in the doc comment that the factor is the
   per-edge area share of the duplicated sample.

2. **Multiplicity type: `u8` can wrap.** (Low–medium.) Gap filling shows the parent sample
   along consumed slivers (`new_owners[neighbor] = Some(parent)`, BFS), and holes show a template
   sample. A pathological sliver could give k > 255. In release builds, `u8` wraps to 0, which
   gives 1/0 = inf and then NaN crust, silently. Recommendation: use `u16`/`u32` (or
   `saturating_add`) and `debug_assert!(k >= 1)` for every owner.

## Subduction erosion

3. **k is taken from the wrong owner map.** (Medium; design.) Counting k in `new_owners`
   before gap filling and `show_distinct_samples` divides by the multiplicity of raw
   nearest-vertex matching. That is about 16% duplicated cells, and more at converging boundaries.
   `show_distinct_samples` then gives most twin cells a hidden sample of the same plate, and it
   never changes the plate. The cell where an event happened then shows a sample that was not
   eroded. Meanwhile the winner was eroded only 1/k although it ends up shown at one cell. The
   erosion is under-applied twice over, and the effective rate drop follows the
   resampling statistics, not the physics.
   Recommendation: record the events as `(contact cell, winner plate)`. After
   `show_distinct_samples` (end of `resolve_ownership`), erode `owners[cell]` (the same plate is
   guaranteed). Aggregate per sample n_s and divide by k_s counted in the final `owners`. This
   is the same map that `crustal_flow` and `apply_surface_changes` use. Most events then land on
   distinct samples at the full rate, and only true duplicates are averaged. Check one side
   effect: the steps in between (rift test `parent_crust.thickness > breakup_thickness`,
   `heal_gap` templates, the distinct-resampling elevation tolerance) would then see uneroded
   crust. This is a small change of 300 m once per step. If it is not acceptable, keep the
   erosion in place but compute k from a provisional pass after gap filling.

4. **"No residual because it is a sink" changes the calibrated flux.** (Medium.) Mass
   conservation is fine either way, because the ledger books what is removed. But n counts
   consumed loser samples (one cell of slab each, deduplicated by `state`), which is the right
   measure of subducted area. Today the total flux n × 300 m × A is correct; only its spatial
   distribution is wrong (all of it lands on one sample, and the `OCEANIC_THICKNESS` floor may
   even truncate it). n/k without a residual lowers the flux per subducted area by
   Σ(1 − 1/k). This is a grid-statistics-dependent cut of a parameter that was derived from
   "recycling ≈ arc production". Arc production per event is kept in full mass by
   `apply_additions`, so the two would drift apart.
   Recommendation: with (3), the gap shrinks to the true post-distinct duplicates. Either
   (a) accept it and report the rate factor from the new log line as a known effective cut,
   with no retuning, or (b) give the residual (k−1)/k × n × 300 m to the winner's plate-local
   mesh neighbours. That means the adjacent forearc/arc samples, ordered by local index, not
   hidden samples far away. It keeps the flux and stays local. Present this choice to
   Sebastian if the measured factor is more than a few percent.

5. **Edge cases otherwise fine.** (OK.) A winner shows at its contact cell, so k ≥ 1 in either
   owner map. With (3), the cell's final owner always exists after gap filling. A winner is never
   consumed in the same pass (state 1), and `if let Some(crust)` stays as a guard. Several losers
   at one cell simply add to n. One thinning by Σ instead of n sequential ones is identical:
   sediment comes first, and the floor `max(thickness.min(OCEANIC))` is monotone. With non-integer
   n/k this still holds.
   Recommendation: use a dense `Vec<u32>` per sample index plus a touched list sorted by index
   instead of a `HashMap` with an insertion `Vec`. That is simpler and has no hash at all.

## Tests

6. **Crustal-flow equality test must pin cycles = 1 and keep the caps slack.** (Low.) With
   sub-cycles the state evolves between cycles, and "loss = mean of the two separate losses" only
   holds approximately. Recommendation: choose `dt` so that `cycles == 1` (assert it) and keep
   the thicknesses away from the floor and cap. Add these cases:
   (a) both sides duplicated (k = 2/2) gives the expected 1/2 per edge;
   (b) a cross-plate (non-converging) edge with a duplicate;
   (c) a duplicated source feeding a one-cell hole;
   (d) a global check on a rotated plate with many duplicates (distinct resampling off): Σ mass
   over all samples, hidden included, changes by the ledger value, ≤ 1e-12 relative, and no
   sample's outflow exceeds the maximum over its cells of the single-cell outflow.

7. **Erosion tests should exercise the owner map, not only the helper.** (Low–medium.) If the
   helper takes `(events, owners)`, add these:
   (a) an event at a cell that `show_distinct_samples` reassigns erodes the newly shown sample;
   (b) sediment-first and the oceanic floor with a non-integer n/k;
   (c) the ledger and `cover_budget` equal the removed mass.
   An integration test through `resolve_ownership` with an artificially duplicated winner
   would catch ordering mistakes like (3).

## Determinism

8. **No risk found.** (OK.) Integer counts, cell-order loops, and no transcendental math. The
   ledger summation order changes (per sample instead of per event), so results differ
   bit-wise from main but stay deterministic. Avoiding the HashMap (5) removes the last
   ordering hazard.

## Other paths

9. **No further mass path acts per cell on duplicated samples.** (OK.) I checked these:
   `apply_additions` (once per sample since #104) and `apply_surface_changes` (#104). Arc,
   accretion and collision additions are per loser and go through `apply_additions`. Deformation
   (`sheet.rs`) works on the plate-local mesh. Stretching/`heal_gap` also work on the local mesh.
   `reheat_above_plumes` is per cell but is an age reset, idempotent and without mass. Splits
   count cells, which is not a mass change.
   The **statistics line** reports *requested* flow with and without 1/k, not applied flow.
   Recommendation: also report applied flow per sample class, because the caps make requested
   flow a poor proxy near orogens.
