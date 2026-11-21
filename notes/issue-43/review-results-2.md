# Review of issue #43 results and diff (7229f1b against 288f9cf)

_Independent reviewer, 2026-11-21. Read: results.md, plan.md (with revisions), review-plan.md,
investigation.md, `git diff 288f9cf HEAD -- src`. Data: v1/s0–7, main/s0–7 (issue-42), oldsuture
and noobl s0–2. My scripts: `~/tasks/issue-43/tmp/rev2/ext.py` (log metrics per seed),
`rev2/coll.py` (collision pairs from the boundary dumps). Nothing was built or run._

Ranking: **A** = must fix before the PR (wrong or misleading statement, or a decision built on
it). **B** = should fix, or declare. **C** = minor.

## Summary

The code does what the plan revisions say. I found no unit errors and no crash-type bugs. Three
things matter for the PR:

1. The headline "subduction 47 → 39%" mostly comes from a larger denominator. In absolute terms,
   trench length barely changed over 8 seeds.
2. The hypsometry shift comes from the suture rule, as results.md says. The specific driver is
   the absolute 10-cell stall count, which keeps long, creeping belts alive: belts whose mean
   convergence is *below* the rule's own 0.5 cm/yr threshold.
3. Several smaller claims in results.md do not follow from the data: "speeds match Earth", "rec 1
   lowers mean land (fewer collisions survive)", the >50 km row of the sensitivity table, and the
   flicker attribution.

## A. Must fix

### A1. Subduction "47 → 39%" is mostly a denominator effect (results.md:46, 118)
- Absolute subduction over 8 seeds is 121 → 116 thousand km: −4%, Welch t = −1.5, not
  significant (`cmp_v1.md`). From the log, trenches go 122 → 115, t = −1.9.
- The share falls because total boundary length rose 260 → 299 thousand km (+15%, t = +4.2).
  More plates and long, unsutured collision and transform belts add length.
- Earth's 51 thousand km is absolute. The model is still at 2.3× Earth, essentially as on `main`.
- Intra-oceanic subduction does fall in absolute terms: 78.7 → 67.7 thousand km, −14%, t = −2.3.
  That is the real effect of recommendation 1.
- "Recommendation 1: trenches −13%" (results.md:105) comes from seeds 0–2. Those happen to be
  `main`'s three longest-trench seeds (123.5k, 130.3k and 130.5k, against an 8-seed mean of 122k).
  Over 8 seeds the reduction is about a third of that.
- **Fix:** lead with absolute lengths. Say: subduction 121 → 116 thousand km (n.s.),
  intra-oceanic 79 → 68 (significant), share 47 → 39% mainly because the total boundary grew.
  Qualify the seeds 0–2 "−13%".

### A2. Wrong numbers in the sensitivity row "continental crust > 50 km" (results.md:102)
- At 1000 Myr the logs give:
  - `main` s0–2: **1, 5, 1** (results.md says 1, 1, 3)
  - branch s0–2: **12, 5, 6** (results.md says 6, 9, 15)
  - old suture rule: 0, 2, 0 (correct)
- The 8-seed row (results.md:78) is correct: 2 (0–5) and 9.4 (5–15).
- More robust than end values are the 500–1000 Myr means: 1.9% on `main` against 8.1% on the
  branch. On the branch the value is not stationary (s0 still rises to 12% at 1000 Myr).
- **Fix:** correct the row, and report the window means next to the end values.

### A3. "Speeds of the slow classes now match Earth's" (results.md:61) does not hold for collisions
- Continental convergent boundaries have a median of 0.9 cm/yr, against Earth's 2.1. 87% of their
  length is slower than 2 cm/yr (`v1_classes.txt`). That is half of Earth's speed, and mostly
  creeping, near-locked belts.
- Transforms (1.3 against 1.6) and rifts (1.1 against 1.1) do match.
- The "slower than 2 cm/yr" gain of 8 → 18% is about 30 thousand km. Of that, ~13 thousand km are
  continental convergent, ~6 thousand km continental transform and ~5.5 thousand km continental
  rift. So it is mostly the collision belts that no longer suture.
- **Fix:** "transforms and rifts match. Collision belts are slower than Earth's (0.9 against 2.1
  cm/yr), mostly creeping belts that no longer suture (see A4)."

### A4. The cause of the hypsometry shift is the absolute stall count, not just "converging collisions"
- `events.rs:1676` counts a pair as stalled only while `fast < suture_min_cells`. That means fewer
  than 10 colliding cells (both sides counted) converge faster than 0.5 cm/yr.
- The count is absolute, so for a long belt the rule is far stricter than its own physical
  statement: "plates merge once convergence across the belt has ceased; slowest active belts
  0.2–0.5 cm/yr" (events.rs:83–88).
- From the boundary dumps, 500–1000 Myr, collision pairs with ≥ 10 `c` cells:

| | main | branch |
|---|---|---|
| pair-snapshots | 388 | 613 |
| stalled by the count rule | 94 (24%) | 138 (23%) |
| belt mean convergence < 0.5 cm/yr | 150 (39%) | 301 (49%) |
| belts ≥ 40 cells: median mean convergence | 0.55 cm/yr | **0.32 cm/yr** |
| belts ≥ 40 cells: count-stalled / mean < 0.5 | 35 / 92 of 191 | **73 / 243 of 380** |
| share of positive convergence (stacking proxy) in belts with mean < 0.5 cm/yr | 12% | **33%** |
| … of which the count rule keeps from suturing | 10% | **29%** |

- So a third of the collision inflow on the branch is in belts that creep below the threshold on
  average but never stall, because 10+ of their many cells exceed 0.5 cm/yr.
- Stacked crust equals the crust carried in by convergence (25.6 against 26.3 Gt/yr; `main` 14.5
  against 15.2). All the extra stacking is convergence across these belts.
- The belts sit inside a still-assembled supercontinent:
  - the largest continental crust mass is 86% of all continental crust on the branch (82% on
    `main`)
  - the largest land mass is 85% (72% on `main`)
  - but the largest plate fell from 34% to 19%.
- Basal drag (×4 under continents) keeps driving these plates against each other at a few mm/yr
  indefinitely.
- **Fix:** see the recommendation below. In results.md, replace "with rigid plates a converging
  collision can only stack" with this mechanism.

## B. Should fix or declare

### B1. Undeclared behaviour changes in `update_slab_lengths` (dynamics.rs:619–662)
- **`last_contact` refresh.** It is now refreshed for every segment with *any* lower-plate
  boundary cell (`in_contact`): resisting crust, diverging cells, strike-slip. Before, only trench
  cells refreshed it.
  - Segments no longer expire along ridges and transforms, so polarity memory persists
    indefinitely while plates touch.
  - Segments: 487 → 570 (8-seed window means), with slabs 383 → 285. Half the segments are now
    slab-free.
  - Polarity on later convergence is inherited instead of chosen by buoyancy.
  - Declare it, or restrict `in_contact` to cells that converge (> −min).
- **Initiation success** drops from 81% to 67% (8 seeds; "initiation segments … grew to 300 km":
  main 77–85%, branch 59–73%). Initiations per 100 Myr: 8.2 → 6.6 (t = −2.0).
  - Likely cause: the gate, and the segment-mean rate (which includes non-converging cells)
    replacing the mean over converging trench cells, so seeded slabs grow more slowly or decay.
  - Not mentioned in results.md. Add it as a guard rail.
- **Slower slab growth.** Growth now uses the mean over all non-resisting lower-plate cells,
  diverging ones included (dynamics.rs:181–188, 663–666), instead of the mean over converging
  trench cells. Segments that converge at one end and open at the other grow slower or not at all.
  This is in plan revision 2, but results.md item 2 says "at that mean rate" without saying that
  diverging cells lower it.

### B2. "Otherwise the slab decays (10 Myr) and breaks off" understates the hysteresis
- In `boundary_cells` (dynamics.rs:144–148), a segment that is not subducting but still has a slab
  keeps all cells with conv > −0.1 cm/yr as trenches, pulling with the decaying slab.
- 600 km decays to 20 km in 10·ln 30 ≈ 34 Myr. The strike-slip test needs 60 Myr.
- **Fix:** say so in results.md item 2: "a strike-slip segment keeps pulling for up to ~35 Myr
  while its slab decays."

### B3. The gate-flicker attribution is wrong (results.md:129–131)
- Resumed subductions over 500–1000 Myr:

| | resumed per Myr |
|---|---|
| branch, 8 seeds | 6.7 (results.md says 6.9) |
| branch, s0–2 | 6.9 |
| without the obliquity gate, s0–2 | **6.0** |
| old suture rule, s0–2 | 6.0 |

- The 20° gate adds ~15%. Most of the flicker is at the 0.1 cm/yr threshold, and in the sign of
  the segment mean, not "near the 20° … thresholds".
- `update_segment_motion` runs twice per step: after the velocity solve (dynamics.rs:447) and
  again in `update_slab_lengths` after the substeps. Toggles between the two evaluations of the
  same step, caused by geometry change alone, are counted too.
- Relative to ~285 slab segments, the rate is about 2.4% per segment per Myr.
- **Fix:** reword. Optionally count only in `update_slab_lengths`, or add hysteresis, for example
  start at > min and stop at < 0.

### B4. "Rec 1 … lowers mean land (fewer collisions survive)" (results.md:107) is not supported
- Old-rule runs against `main`, s0–2:
  - mean land: t = −3.0 with 3 against 3 seeds (df ≈ 3, t_crit ≈ 3.2, so not significant at 5%);
    against all 8 `main` seeds, t = −2.07
  - sutures: 2.67 against 2.73 per 100 Myr (no change)
  - collision length: t = −1.3; stacking: t = −1.8
- The "|t| > 2.1" rule of results.md:35 holds for 8 against 8 seeds, not for the 3-seed table.
- About 50 metrics are tested, so ~2–3 false positives are expected at 5%.
- **Fix:** "possibly lowers mean land (3 seeds, not significant); no change in sutures." State
  that the 3-seed comparisons are indicative.

### B5. Guard rails from plan revision 8 are missing in results.md
- Not reported: time at `max_plates`, continental plates, largest continent's share, and sea
  level. The largest continent's share shows the supercontinent persists with internal plate
  boundaries (A4).
- Max plates per run is 30–39 on the branch, under the cap of 40, but close in s5 (39).
- The full subduction-angle histogram (plan revision 6, the independent check of the gate) is only
  in `v1_classes.txt`. Branch bins: 4 5 8 10 12 14 15 16 17 (main: 7 8 9 10 11 12 13 14 15). Put
  it in results.md.
- "More, smaller plates" (results.md:113): plates 24.4 → 26.9, t = +1.6, not significant. Only
  the largest plate's share is.

### B6. Tests (would they fail on `main`?)
- Meaningful:
  - `boundary_normals_follow_the_boundary_not_the_mesh`: raw normals give mean |conv| well above
    0.1 v.
  - `contacts_close_to_strike_do_not_subduct`: on `main` the slab grows at 12°.
  - `strike_slip_slabs_decay_*`
  - `converging_collisions_do_not_suture`: sutures at step 3 on `main`.
  - `converging_collisions_do_not_suture_but_accrete_terranes`
  - `merged_pairs_keep_the_longer_stall`, and the split test with `stall_time`.
- `slab_pull_per_boundary_length_does_not_depend_on_the_resolution` (dynamics.rs:1222) would also
  pass on `main`. Both length definitions scale with `mean_edge_angle`, so resolution independence
  was already there. The ÷1.1 change is untested.
  - **Fix:** assert Σ|F| / (2πR) ≈ `slab_pull` × length factor in absolute terms.
- Missing against plan revision 7:
  - the sliver fallback (`smoothed.dot(raw) ≤ 0`)
  - a segment rotating about a pole inside it
  - 10–15° from strike with *solved* velocities (the 12° case uses prescribed velocities)
  - `stall_time` in `snapshot()`
- Worth adding: a long creeping belt (mean < 0.5 cm/yr, more than 10 cells > 0.5) to pin down the
  stall rule's behaviour, whichever variant is chosen.

## C. Minor

- **C1.** `PairState::collision_time` is now dead state (events.rs:1673/1682, snapshot,
  merge). Nothing reads it. Remove it, or keep it as a diagnostic.
- **C2.** `Segment::rate`, `strike_rate` and `subducting` are not in `Polarities::snapshot()`, so
  the matching verification (matching.rs:331) cannot catch divergence in them. They are
  recomputed every step, so the risk is low.
- **C3.** `kept.subducting |= removed.subducting` (polarity.rs:386) is overwritten at the next
  evaluation, which happens right after `end_step` in `step`. It is harmless, but `rate` and
  `strike_rate` are not merged, so the merged segment can briefly carry inconsistent fields.
- **C4.** `suturing` is now `stall_time > 0`. A pair that has stopped colliding but still has
  decaying `stall_time` (up to 30 Myr) cannot accrete terranes, even when trenches dominate. On
  `main` such a pair was eligible. This is rare, but undeclared.
- **C5.** `smoothed_normals` allocates `vec![u32::MAX; mesh.len()]` on every `boundary_cells`
  call (2–4 per step). It is cheap at 41k cells, and the runtime gain of +3% is fine.
- **C6.** The units check out:
  - min convergence: cm/yr → rad/Myr via `v·1e4/R`
  - `rate·R·1e-4`: rad/Myr → cm/yr
  - slab growth `rate·R·dt`: rad/Myr × m × Myr = m
  - gate: `rate ≥ tan20°·strike_rate` with both in rad/Myr
  - collision resistance still uses `cell_area` (unchanged, already on smoothed normals on `main`,
    apart from the sliver guard)

## Question 3: the hypsometry shift

**Is the explanation right?** Mostly yes. The old-rule runs (recommendation 1 alone) show
stacking of 9.7 Gt/yr against 17.2 on `main`, and mean land of 471 m against 625 m. So the
smoothed normals, the gate and the per-length force change do not cause it. Stacked crust equals
the crust carried in by convergence. I checked for other code-level causes:

- **Collision resistance on smoothed normals:** unchanged from `main`, apart from the sliver
  guard.
- **Length-per-cell change:** it does not touch collision resistance or basal drag, and it is
  included in the old-rule runs, which stack less.
- **Terrane accretion doubling (5.9 → 12.9 per 100 Myr):** accretion moves cells and does not
  stack by itself. It does relocate collision contacts and keep pairs converging. It belongs to
  the same rule change and cannot be separated with the existing runs.

The precise cause is A4. The absolute 10-cell stall count, plus continuous basal-drag driving of
continental plates inside an assembled supercontinent, keeps long belts creeping at ~0.3–0.5
cm/yr on average indefinitely. Converging collision length goes 8.3 → 22.0 thousand km in the log.

**Plausibility.** The branch values are not implausible for an Earth-like planet:

- mean land 1.04 km (Earth ~0.84)
- highest cell 5.7 km at 110 km resolution (Tibet averages ~5 km)
- 24% of continental crust flooded
- continent-continent boundary 18.8 thousand km (PB2002 CCB ~23 thousand km; `main`'s 6.9 was
  short)

Crust > 50 km at 8–9%, and a 90th percentile of 48 km, should be checked against CRUST1.0 before
being called too thick. The issue-1r band of 3–6% was calibrated under a suture rule that merged
converging continents, so falling outside it is not by itself a defect.

**Recommendation for the default:**

1. **Keep the stalled-collision rule on.** Merging continents that still converge at 1–4 cm/yr
   (India-like) is the less physical choice, and the rule produces most of the slow, slab-free
   boundaries.
2. **Make the stall criterion consistent with its own physical statement.** "Convergence across
   the belt has ceased" should be the belt's convergence, not an absolute count of fast cells.
   Use the mean convergence over the non-diverging colliding cells, or require fewer than
   max(10, ~25%) of them to be fast. This is a correction, not tuning, and keeps 0.5 cm/yr.
   - The static estimate from the dumps: up to ~30% of the collision inflow sits in belts that
     would then stall and suture.
   - Rerun with that, and report the hypsometry.
   - It needs care with pairs that converge at one end and open at the other (the plan-review
     concern). Averaging only non-diverging cells handles most of it.
3. **Physically motivated companion: the buoyancy (gravitational potential energy) force of
   thickened crust in the collision force balance.**
   - Thickened crust pushes outward on both plates. Per metre of belt, the force is
     ∫(σ_zz excess) dz ≈ g ρ_c (1 − ρ_c/ρ_m) (h² − h₀²)/2, about 4–7 × 10¹² N/m for a Tibet-like
     70 km crust. That is comparable to ridge push and to a fraction of slab pull.
   - This is why collisions decelerate and plateaus stop growing (England & Houseman 1986/1989;
     Molnar & Lyon-Caen 1988; Copley et al. 2010 for India's slowdown).
   - In the model it would slow converging collisions as they thicken, so they stall and suture
     on their own instead of stacking indefinitely.
   - It uses only known densities, and the existing isostasy gives h.
   - Second candidate: lower-crust / continental subduction at collisions (the open C2 item),
     which removes part of the incoming crust instead of stacking it.
4. If Sebastian wants to merge now, the rule-on state is defensible, with the hypsometry caveat
   stated plainly. Rerun recommendation 2 before calling the issue-1r band violated.
