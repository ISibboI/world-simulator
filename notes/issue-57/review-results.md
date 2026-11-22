# Independent evaluation of the #57 results (branch agent/issue-57, 848ae93 on main 67ca0a2)

Evaluator: independent subagent, 2026-11-21. Read: results.md, plan.md, review-plan.md, `git diff 67ca0a2..HEAD -- src`,
`write_cell_dump` (src/main.rs), `nearest_from` (src/sphere.rs). Own scripts, all in
`/home/agent/tasks/issue-57/tmp/eval/`: `pits_eval.py`, `spill.py`, `anom.py`, `logs.py` (own parsers, csv + numpy;
not derived from notes/issue-57/scripts). Own diagnostic: an instrumented copy of the branch
(`tmp/eval/copy`, untracked, not committed) run for seed 0, 300 Myr (`tmp/eval/diagrun/diag*.txt`).
Nothing tracked was edited.

Judged by the project goal: an Earth-like planet, physical plausibility and conservation, not Earth's exact numbers.

## Summary

The bookkeeping fix is correct in its main path, conserves mass, is deterministic, and its
headline numbers reproduce exactly. The central new claim (the pits on duplicated samples come from
the shared column, not from the doubling) **holds up**. My own analysis gives a sharper and slightly
different mechanism than the "terrace on a slope" wording: the pair's elevation difference is locked
to the dynamic-topography difference, so the low twin can never fill. One real weakness remains in the
new code: the residual is concentrated on very few hidden samples when a plate/class has few
recipients. In practice this puts kilometre-scale sediment and bedrock changes into a handful of
hidden samples per step (F5). It is small in mass terms (0.4% of the gross), and the guard rails show
no significant effect, but it is a latent source of spikes and pits when those samples resurface.
Recommendation: merge as a bookkeeping fix after capping the per-recipient share (or after recording
the issue as a known limitation), and with results.md toned down in a few places (F8–F10).

## Findings

### F1 — Info (confirmed): the key numbers reproduce exactly
*Evidence.* My own parsers, run independently of `pits.py`/`compare.py`:
- Seed 2: main 1.39% pits, 53.9% on duplicated samples, 17.1% land dup; full 1.34%, 64.7%, 15.8%.
  Seed 5: main 1.20%, 49.1%, 16.4%; full 1.16%, 49.3%, 15.2%. These match the per-seed ranges in
  pits.txt (64.72 is the full maximum, 49.13 the main minimum).
- Log metrics over 8 seeds (snapshots 600–1000 Myr): sharing main +29.41 → surface −9.12 (t −22.5) →
  full −8.13 (t −21.9), per-seed main 23.8…38.0, full −7.0…−9.8. Endorheic 15.89 / 15.15 / 16.55;
  endorheic into single-cell pits 7.16 / 7.12 / 7.90; to hidden 8.49% / 8.44%, kept 0.10% / 0.12%.
  All identical to the results.md tables.
- No "surface processes" entry ever appears in the crust mass budget line of any of the 24 runs, so
  it stayed below the print threshold, as claimed.
- Determinism: my instrumented copy (same seed, different `--width`, no dumps) gives a bit-identical
  `t = 300 Myr` status line to `runs/full/seed-0`, run twice.
*Recommendation.* None.

### F2 — Info (claim supported, mechanism sharpened): duplicated-sample pits are locked twin pairs
*Evidence* (seeds 0, 2, 5; main and full; 600–1000 Myr; twin = neighbour with identical
thick, sed, dens, felsic, meta, plate and age):
- 100% of duplicated-sample pits have an adjacent twin (460/460, 365/365, 488/488, 311/311 …).
- For every one of them, elev(twin) − elev(pit) equals dyn(twin) − dyn(pit) to within 0.1 m, which is
  the dump's rounding. So the two cells show the same column, as claimed. The `dup` column is what it
  claims to be: the count of cells whose `Ownership` matches (main.rs 1161–1164).
- Spill depth (how far the pit must fill before it drains): median 5–6 m, q90 18–21 m, and **never
  more than the dyn difference to the twin** (median 16–18 m). In 27–36% of cases the twin itself is
  the spill. In the rest a non-twin neighbour is lower than the twin, but always by less than Δdyn.
- The pair is almost never a closed two-cell depression (the twin is below all its own other
  neighbours in only 2–7% of cases), and the twin drains into the pit in only ~1/3 of cases.
- The low member of a twin pair (Δelev > 1 m) is a pit 8–12% of the time, against 0.6–0.8% for
  non-duplicated land cells, so the rate is 12–20× higher. The rate and depth are the same on main and full.

So the mechanism is: the column's level is set by where the higher twin settles, and the low twin
sits exactly Δdyn below it forever. When it receives deposition (it is a pit), the deposit is
averaged with the twin's change, so both rise together and the offset never closes. That happens
under the sum rule and under the mean rule alike: the pair's change is (c_i + c_j) or (c_i + c_j)/2,
which has the same sign and the same stationary point. The fix could therefore not have removed
these pits even in principle, and the doubling hypothesis predicted a fall in count and/or depth,
which did not happen (dup-pit share +0.9/+1.0 t, depth unchanged). results.md's "two-cell terrace …
the low end … a few metres below its other neighbours" is right in substance. But "on a slope" is
misleading: these pits are on near-flat land where local relief is smaller than Δdyn ≈ 15–25 m. The
pit is bounded by the dyn offset of the twins, not by the land slope.

Alternatives I checked and ruled out as the main cause:
- `dup` meaning something else: ruled out, see above.
- Per-cell erosion artefacts unrelated to sharing: these would not give the exact Δdyn bound.
- Resurfacing hidden samples: these would sit on dup = 1 cells, and they do not explain the
  dup-pit share.
*Recommendation.* Reword the mechanism in results.md and the PR: "the elevation difference of the
two cells is locked to their dynamic-topography difference (median ~17 m). The low twin can't fill,
because any deposit is shared with the twin. Pits up to Δdyn deep form where the local relief is
smaller than that." Quote the "spill depth ≤ Δdyn in 100% of cases" figure. It is the strongest
evidence in the analysis.

### F3 — Info: power of the pit comparison
*Evidence.* Pit % spread across seeds is ~0.13 (sd). With 8 vs 8 seeds, a 20% reduction (−0.28
points) would give t ≈ −4, and the observed t is −0.8. A large effect of the doubling on pits is
excluded, but an effect below ~10% is not.
*Recommendation.* Phrase it as "no detectable effect; a reduction above ~15–20% is excluded",
not as "not caused at all".

### F4 — Low (latent conservation bug): fallback re-reads the class after the mean was applied
*Evidence.* In the first loop the residual goes to `residuals[plate][class]`, with `class` taken
**before** `apply_surface_change`. The fallback loop (`targets.is_empty()`) selects samples with
`is_continental_origin_column()` **after** the mean change was applied. If a duplicated sample
flips class (erosion thins it below `crust_resists_subduction`, or deposition lifts it over), then:
- if its original class has no recipients, its residual is never applied (mass lost);
- if the other class also falls back, the residual is applied there instead (in the wrong
  class);
- if the original class had recipients and the new class falls back, the residual is applied
  **twice**: once via the pool and once kept (mass created).

The ledger still "closes", because it records the applied change, so no test or ledger line would
catch it. In my seed-0 diagnostic (300 calls) no duplicated sample flipped, so this is rare in
practice.
*Recommendation.* Store the class in `SampleChange` (computed once, before the change) and use it in
the fallback. Add a test where the mean change flips the class.

### F5 — Medium: residuals concentrate on few hidden samples (equal-share fallback, small recipient sets)
*Evidence* (instrumented seed 0, 50–300 Myr, 251 calls, ~2600 recipients per call, ~30
plate/class groups per call, of which 6.6 per call have < 10 recipients):
- The pool / Σ(recipient-cell change) ratio of the worst group per call has median 112 for bedrock.
  For deposits and sediment removal it is usually infinite: no recipient cell has that kind of
  change, so the 1/n equal-share fallback applies. The groups with n = 1 recipient take the plate's
  entire class residual.
- The largest single-recipient change per step has median 760 m (bedrock, max 6.2 km) and
  2.8 km (sediment, max 12 km).
- Per step, ~4.8 recipients get > 1 km of sediment and ~0.4 get > 1 km of bedrock removal.
  ~5.5 recipients per step get > 100 m and > 10× their own cell's change. That is 0.4% of the gross
  mass. Over all recipient-steps: 87% get < 2× their cell's change or < 1 m, 11.7% get 2–10×,
  0.8% get > 10×.
- These samples are hidden, so they do not show until they resurface. The guard rails show a
  consistent but not significant drift that would fit resurfacing spikes (8 seeds, per 10⁴ land
  cells, main → surface / full):
  - cells with sediment > 2 km above every neighbour: 90.8 → 103.1 / 100.9 (t +1.5 / +1.3);
  - thickness > 3 km below every neighbour: 78 → 89 / 87 (t +1.2 / +1.1);
  - land peaks > 100 m: 94 → 109 / 105 (t +2.9 / +1.4; results.md reports this, but does not explain it).

The plan rejected local matching because it "would put full shares on single hidden samples". The
implemented rule does the same for small plate/class groups.
*Recommendation.* Cap what one recipient can take per call. For example: at most c × max(own-cell
change, the mean |change| of the plate's cells), with c ≈ 2–3. Leave the excess on the duplicated
samples (the existing "kept" path, counted), or spread it equally over the class's recipients
before capping. Also send the equal-share part only when n is large (e.g. n ≥ the number of
duplicated samples of that group / 2). Log max per-recipient metres per snapshot. Re-run 8 seeds and
look at the three anomaly counts above. If this is deferred, list it in the PR as a known
limitation with these numbers.

### F6 — Low: hidden-sample selection is correct; the hint is cheap except for deep hidden samples
*Evidence.*
- Greedy `nearest_from` on the icosphere converges to the true nearest vertex (near-Delaunay
  mesh), so the result does not depend on the start.
- The start hint (the cell of a shown local neighbour) is good for resampling-hidden samples.
- Samples overridden deep under another plate have no shown neighbour and start at cell 0. Their walk
  is O(√N) ≈ 100–200 steps each, every call, for every plate with a residual. They are then
  correctly discarded ("under another plate").
- The test "not under another plate" (owner of the nearest cell ≠ plate → skip) matches the intent.
  Consumed / `None` vertices are skipped by `let Some(crust) … else continue`.
- The cost is real but bounded: a scan of plates × cells, plus walks.
*Recommendation.* Fill in the pending runtime section before the PR (the time-main/time-full runs
were still running while I evaluated). If the overhead is noticeable, cache the last nearest cell per
plate vertex (like `lookup_cache`), or skip vertices whose local neighbours are all hidden and none
is shown (they are almost surely under another plate).

### F7 — Low: weighting mixes units, and its fallbacks are not tested
*Evidence.*
- The bedrock weights are metres of each cell's own shown bedrock (densities vary by up to ~10%).
  This is harmless.
- The bedrock part assumes `thickness_delta ≤ 0`. A positive net bedrock residual (if any process
  ever produces it) would be spread by removal weights.
- The three-part split is mass-exact: each part's shares sum to 1, and bedrock is converted with
  each recipient's own density, so mass is conserved.
- The clamping (bedrock runs out) is counted. Its sign is negative in the logs ("−2.9e-6 Gt found
  no bedrock"), because it is expected − applied for a negative request. That is cosmetic.
*Recommendation.* Log `clamped` as a positive magnitude. Optionally assert or debug-assert
`thickness_delta ≤ 0`.

### F8 — Low/Medium: the tests test the main path; one passes vacuously, another closes by construction
*Evidence.*
- `shared_samples_get_the_mean_and_hidden_samples_the_rest`: on main it fails (the sum gives −30
  m, not −15). It covers the mean, the density conversion of the residual and conservation against
  the requested mass. This test is good.
- `samples_shown_at_several_cells_take_their_profile_share_once`: on main it fails (ratio 2). Good.
- `shared_samples_without_hidden_samples_keep_the_sum`: asserts the sum, so it would pass on main
  too (apart from `surface_sharing()`), which is fine for a fallback test.
- `hidden_samples_under_another_plate_get_nothing`: the only assertion that the recipient
  *is* selected is behind `if same class { … }`. If the classes differ, the test checks only that
  nothing happened, so it passes vacuously. **I checked: in this fixture the condition is false**
  (probe in the scratch copy prints `CONDITION false`; all 6 related tests pass), so the test never
  asserts that a valid hidden recipient receives anything — it would also pass if recipient
  selection were broken entirely.
- `crust_mass_ledger_closes` closes by construction: the ledger records the applied change. It
  cannot detect created or lost mass (see F4).
- The plan's promised "sediment removal larger than the available sediment on a group" test
  was not added.
- `simulation_is_deterministic` steps tectonics only and compares `owners`, so it does not
  exercise `apply_surface_changes`. Determinism holds anyway (F1).
- There is no test for the class flip (F4), the equal-share fallback, or the concentration (F5).
*Recommendation.* Make the class condition a precondition of the fixture (set the recipient's crust
to the shown sample's class, as the first test does) and assert unconditionally. Add the
sediment-overdraw and class-flip tests. In the ledger test, also compare the applied change with
`requested()`.

### F9 — Low: the additions dedup is correct
*Evidence.* `first_seen` is now built for every addition. The profile weights of repeated owners
are zeroed, the total weight is renormalised and mass is conserved (the test checks it). Collision
bands were already de-duplicated. "Nearest ring" holds because `reached` is in ring order. This is a
tectonic change, and the `full` vs `surface` comparison attributes its effect. The only notable
shift is the dynamic-topography maximum (t −4.2), which results.md honestly calls unexplained.
*Recommendation.* None for the code. In the PR, don't present the −4.2 as chance: with 76 metrics
|t| > 4 is unlikely by chance. Say "unexplained, one seed set". Or re-check it with a second set of
seeds, or look at whether the additions change thickens the arc crust less (crust > 50 km went
5.05 → 3.78, t −1.3, which points the same way).

### F10 — Low: bookkeeping and guard-rail statements, partly overstated
*Evidence.*
- "Cells now show the changes the erosion computed": true. Σ over the k cells of the mean = Σ of
  the cell changes.
- The sharing term going from +29 to −9 is real, but it measures only what the cells see. 8.5% of
  the gross change now goes into hidden storage that is invisible to the sea-level solve and the
  climate, and it reappears as a resampling effect when those samples resurface (inside the
  "tectonics" term: −250 → −218 / −234 m/100 Myr, t +1.4 / +0.8). The −9 m residual is attributed
  to "averaging across a coastline", but this was not examined. Nonlinear sediment porosity
  (`sediment_bulk_thickness`) and the overdraw conversion are equally plausible.
- "Shows no shift outside what 8 chaotic runs give, apart from those listed": fair for |t| ≤ 3.
  But the peaks-on-dup drop (−3.1 / −2.7) and land peaks > 100 m (+2.9 surface) are significant
  and only listed in a table, not discussed. The first is a real effect of removing the doubling
  (the sum over-raised and over-lowered single columns). The second may be F5.
- "Mass is conserved exactly": true in the main path. Not strictly true in the class-flip corner
  case (F4).
- "The doubling was real, but it was not what makes the pits": supported (F2, F3).
*Recommendation.* Add one sentence each on: the hidden storage and that the sharing term doesn't
see it; −9 m as unexplained (hypothesis: coastline/porosity); the peaks-on-dup drop as a positive
effect of the fix; and the >100 m peaks as a possible effect of F5.

### F11 — Info: recommendation and the #46 follow-up
- **Merge?** Yes, as a bookkeeping fix. A sample no longer gets k cells' change. The sea-level
  sharing residual falls by about two thirds, single-cell peaks on duplicated samples drop, mass is
  conserved, and nothing degrades significantly. Fix F4 (a two-line change) first. Deal with F5
  first or list it as a limitation. The pits are not a reason to hold the merge, and they are not a
  reason to merge either: be explicit that #57's original motivation (#46 pits) is not addressed.
- **#46 follow-up.** Option (b), the drainage treating the pit and its same-sample twin as one node,
  is the cheapest and targets exactly the measured mechanism (F2). The twin is always adjacent, and
  the pit is always ≤ Δdyn deep. It should cut the "endorheic into single-cell pits" share (7–8% of
  land) roughly in half, which is a testable prediction. Option (a), one-to-one resampling, removes
  the cause (duplicates, the terraces, the doubled crustal flow and subduction erosion, and also the
  need for F5's hidden-storage scheme). But it is a large tectonic change and should be its own
  issue. I'd do (b) now and open (a) as an issue.
- A third, cheaper idea to consider for (b): when filling single-cell pits in the drainage, ignore
  pits whose depth is ≤ the Δdyn to an adjacent same-sample twin. This uses the exact bound found
  above.
