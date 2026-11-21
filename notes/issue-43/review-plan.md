# Review of `notes/issue-43/plan.md` (before implementation)

_2026-11-21, independent reviewer. Read: plan, investigation, `src/tectonics/{dynamics,mod,events,polarity,boundaries,statistics,topology}.rs`
at agent/issue-43 (= main 288f9cf), the #42 diff (`~/worktrees/issue-42`, e5a6a4d) and its plan.
No builds or runs. Checks on existing data (main, s0–s7, 500–1000 Myr) with single-threaded python:
`~/tasks/issue-43/tmp/planreview/{check,coll,normals}.py`._

Framing: an Earth-like planet, not Earth; grid inaccuracies (~110 km cells) are accepted. The
plan's direction is sound: all three items fix real implausibilities without tuning targets.
Below, findings ranked by how much they would change the outcome.

---

## Ranked findings

### 1. (High) Converging collisions end up in limbo: no suture, and no terrane accretion
`suture_colliding_plates` puts every pair whose `collision_time` grows into `suturing`, and
`suturing` pairs never accrete terranes (events.rs ~L1650–1712). `collision_time` grows whenever
the pair has ≥ 10 non-diverging collision cells and at least as many as trench cells, **whatever
the convergence**. The plan keeps `collision_time` "as the measure that blocks terrane accretion"
and adds a stall rule for suturing. A pair that keeps converging above 0.5 cm/yr then **never
sutures (stall_time does not grow) and never accretes (it is in `suturing`)**. The plan's
"can accrete terranes (unchanged rule)" is false under the current code. Example: the existing
test `suturing_pairs_do_not_accrete_terranes` (2.5 cm/yr, trench ratio 0) would end with no
suture and no accretion.

**Fix:** build the blocking set from the stall state. A pair is "on its way to a suture" only
while `stall_time > 0` (or is growing). Converging pairs go through the normal terrane rule.
Rewrite `suturing_pairs_do_not_accrete_terranes` around a stalled pair, and add
`converging_microcontinent_pairs_still_accrete`. Alternatively drop `collision_time` as the gate
altogether and keep it only as a diagnostic.

### 2. (High) A2 gates slab *growth*, but "being a trench" stays per cell and ungated, so the expected drop of oblique trenches will not follow
After A2, a segment that fails the gate loses its slab. Its cells are still `Trench` wherever the
smoothed convergence is > 0.1 cm/yr, because `subducts_here` (slab 0) only needs
`convergence > min`. At 10° from strike and 5 cm/yr that is 0.87 cm/yr. Such cells:
- consume crust and build arcs in `resolve_ownership`;
- feed `subduction_rates`/`sinking_slabs`, which drive mantle downwelling and dynamic topography;
- carry the full initiation strength (Trench with slab 0);
- count as `t` in `sim_classes.py`.

So "trench length within 20° of strike: 15% → a few %" is not what A2 does on its own. The unit
test `strike_slip_slabs_decay … and the cells stop being trenches` passes only because pure
strike-slip gives convergence ≈ 0.

**Fix:** store the result of the A2 gate in the segment (e.g. `subducting: bool`, or the
per-segment `rate`/`strike_rate`, see 4). In `boundary_cells`, require it for `Trench` when the
slab is 0: `segment.slab_length > 0 && conv > −min` **or** `segment passes gate && conv > min`.
Cells of a failing segment without a slab become `Other` (transform). Test it with a 10–15°
oblique contact, not with pure strike-slip.

### 3. (High) Closed loop: the decaying slab's own pull may keep the gate open; the tests must run with solved velocities
In `main`, the convergence that keeps oblique slabs alive is largely produced by those slabs:
pull plus slab resistance drive the plate along the normal, towards slab_sinking_speed. Under A2
the hysteresis keeps a decaying slab pulling normal to the trench for its e-folding time (10 Myr;
break-off from 600 km below 20 km takes 10·ln 30 ≈ 34 Myr). That pull raises c̄, which may pass
c̄ ≥ tan 20° · s̄ again and regrow the slab. Whether the gate bites is an emergent property. The
plan's static numbers cannot show it: my preview on the main dumps puts 16% (gate on `t` cells)
to 18% (gate on all lower-plate cells) of trench rows in failing segments, and only 7–10% fail
on c̄ ≤ 0.1 alone.

**Fix:**
- `strike_slip_slabs_decay` and `oblique_convergence_…` must call `update_plate_velocities`
  (solved velocities, slab pull included, e.g. a far-field drag driving strike-slip), not
  prescribed angular velocities only.
- Validation: log per step the number of segments failing the gate, how many of them regrow
  within 20 Myr ("gate flicker"), and the time series of the oblique-trench share.

### 4. (Medium-high) A2's cell set is biased; align it with #42's segment rate
"Over all its trench cells (Trench kind)" only sees cells with convergence > −0.1 cm/yr (with a
slab) or > +0.1 (without). Clearly diverging cells are never `Trench`, so c̄ is biased upwards
exactly on rotating and oblique segments. A segment whose Euler pole lies near it, converging at
one end and diverging at the other, averages only the converging half.

#42's `update_segment_rates` already computes `rate` over **all lower-plate boundary cells of
the segment whose crust can subduct**, with smoothed normals, right after the velocity solve.
The plan's claim that `segment_motion` gives "the same per-segment rate #42 computes" is not
true as written: the cell set differs (Trench only vs all lower-plate cells), and so does the
time (A2 uses the post-motion boundaries in `update_slab_lengths`, #42 the solve's boundaries).

**Fix:** define c̄ and s̄ over the same cell set as #42 (lower plate, i.e.
`boundary.other == segment.overriding`, not `resists_subduction`; Collision cells excluded),
computed once after the solve. Store `segment.rate` and `segment.strike_rate`, and have both A2
and #42 read them. One implementation, no duplication, no bias. Length-weighting is implicit
(one row per cell).

### 5. (Medium-high) A1 is a hidden ~+15% recalibration of every normal force (slab pull, ridge push, slab resistance)
Raw normals scatter: median 16.5° from the smoothed normal, 8.4% of rows more than 45°, mean cos
0.90 (`normals.py`, 3 seeds × 3 snapshots, 42k rows). So their sum partly cancels. Per plate-pair
side, |Σ n_smoothed| / |Σ n_raw| = **1.16** (10–90%: 1.06–1.27). `boundary_force` and
`slab_resistance` use `R · mean_edge_angle` per cell, **not** `boundary_length_per_cell()`
(÷ `BOUNDARY_ZIGZAG` = 1.1). The raw normals' cancellation happened to roughly undo the zig-zag
over-count. With smoothed normals the net normal force per true boundary length rises by ~10–16%.

Slab pull and slab resistance scale together, so the terminal speed of a slab-driven plate barely
changes. Ridge push against basal drag, and slab forces against coupling, collision resistance and
initiation strength, do change.

A1 also removes the along-strike component of slab resistance (raw m mᵀ had ~E[sin²θ] ≈ 10–15%
of it along strike). Trench-parallel motion becomes easier, which feeds finding 3: more oblique
trench motion.

**Fix:** in the same commit, switch `boundary_force` and `slab_resistance` (and the
`initiation_strength` length, for consistency) to `boundary_length_per_cell()`, so the force per
true length stays within ~5% of today. Or keep them and declare and measure the recalibration.
Add a test: the net slab-pull force on a straight trench equals `slab_pull × true length`
within a few per cent at subdivisions 4 and 5. Log the total slab pull and ridge push per km.

### 6. (Medium) The suture measurement does not reproduce; the stall rule will cut sutures much more than the plan implies
The plan cites 22 pair-snapshots with percentiles −0.4/0.0/0.55/1.05/2.1 cm/yr and 45% below
0.5. On the same dumps (s0–s7, 500–1000 Myr; collision rows `c`, conv = smoothed-normal
convergence), pairs eligible under today's rule (≥ 10 non-diverging collision rows and ≥ trench
rows) give **210** pair-snapshots:
- mean over all collision rows: −0.17/0.17/**0.85**/1.74/2.8 cm/yr, **37%** below 0.5;
- mean over non-diverging rows only (the code's "colliding cells"): 0.32/0.61/1.27/1.98/3.02,
  **20%** below 0.5;
- ≥ 10 rows, any trench ratio: 355 pair-snapshots, 32% below 0.5.

Three consequences:
- **"Mean over its colliding cells" is ambiguous, and the choice halves or doubles the stalled
  share.** State it: signed mean over all Collision rows of the pair, both sides.
- **Most eligible pairs (63–80%) would not suture.** Sutures are a main path of plate death and
  of continent assembly, so plate count, continental plate count, `max_plates` = 40 saturation
  (it blocks rifts) and the supercontinent cycle may shift. These must be guard rails.
- Commit the measurement script in `notes/issue-43/`; the 22-sample number cannot be checked.

### 7. (Medium) The stall threshold is really a force threshold set by `collision_resistance`
Collision resistance is linear viscous (k = 50 basal-drag cells per boundary cell, active only
while converging), so a pair converges at (net push)/k. "Below 0.5 cm/yr" therefore means "net
push below 0.5 cm/yr × k". `collision_resistance` was calibrated (2026-11-17) on stacking and
land; now it also decides suturing. That is a hidden coupling, not tuning, but it must be stated.
The sensitivity runs should vary `collision_resistance` (30/100, the range its doc calls
equivalent) as well as `suture_convergence`.

The 20 Myr is barely argued ("pauses between pulses"). It also re-defines `suture_time`: a
collision locked from the start now sutures after 20 instead of 30 Myr.

**Fix:** add `suture_time` = 30 to the sensitivity set, or argue 20 Myr from the model's own
time scales, e.g. slab decay plus `polarity_memory` = 20.

Optional, and more robust against rotating pairs (one end converging, one diverging, mean ≈ 0):
call a pair stalled when fewer than `suture_min_cells` of its collision cells converge faster than
`suture_convergence`. This reads directly as "no longer a convergent boundary anywhere
significant".

### 8. (Medium) Every consumer of `BoundaryCell.normal`/`.convergence` and what A1 changes there
| consumer | effect of A1 |
|---|---|
| `boundary_cells`: trench test (incl. hysteresis), ridge test | intended. Fewer flickering trench/ridge cells. ~7.5% of today's trench cells diverge along the smoothed normal and drop out |
| `boundary_force`: slab pull, ridge push direction | +~16% net normal force, strike components gone (finding 5) |
| `slab_resistance` (m = r × n) in `update_plate_velocities` | normal drag up, along-strike drag at trenches gone (finding 5) |
| shear coupling, collision resistance, nascent strength | already smoothed: no change (use `boundary.normal`) |
| `update_slab_lengths` | replaced by A2 |
| `subduction_rates`, `sinking_slabs` (mod.rs step → `mantle.step`) | the downwelling flux is summed over Trench cells with relative > 0. Spurious strike-slip flux goes; slab sink positions use a coherent normal. This changes the **mantle flow, dynamic topography and sea level**: add them to the guard rails |
| mod.rs `opened_area` / `continental_opened_area` | kinematic production: measurement change (acknowledged) |
| mod.rs `collision_inflow` (ledger diagnostic) | changes by measurement: say so |
| `initiation_candidates` (`kind == Other && convergence > −min`) | different candidate cells for induced boundary initiation, and so the hazard |
| `near_own_trench` (invasion reach) | follows the Trench cell count |
| `suture_colliding_plates`, `accrete_terranes`' `converging` set | already smoothed: no change |
| statistics: incipient/mature trench convergence, trench length, slab-pull sum, collision converging/diverging length | log values shift by measurement. `boundary_motion` and `boundary_records` (dump conv/strike) were already smoothed, so the dump stays comparable |
| `crustal_flow` (boundaries.rs ~L86) | uses its own per-edge raw normal, **not** BoundaryCell. Unchanged, but now inconsistent with the classification. Out of scope; note it |
| tests: dynamics `mean_trench_convergence`, `slab_pull_moves_…`, `collisions_stop_convergence`, `set_relative_motion` (axis from `b.normal`), `diverging_boundaries_are_not_trenches…`, `creeping_contacts_build_no_slab`, `slabs_grow_with_convergence_and_decay_without`; events L2815 (`convergence < 0`); boundaries.rs L1286/L1369; polarity L957 | re-check. `set_relative_motion` silently changes its set-up |

Edge cases of the smoothed normal:
- **Sign flips:** 0.4% of rows have raw · smoothed < 0 (slivers, peninsulas, one-cell
  necks), and 8% differ by > 45°. A flipped normal turns a trench into a ridge (or back) and
  reverses slab pull at that cell. **Fix:** fall back to the raw normal (or drop the cell's
  force) when smoothed · raw < 0. Add a sliver test.
- **Fallback:** `try_normalize().unwrap_or(boundary.normal)` falls back to the raw normal. This
  stays correct only if the raw normal is passed in before it is overwritten (two-pass build). It
  never triggered in my sample.
- **Two sides:** each side is computed around its own cell, so the two normals are antiparallel
  only approximately. Fine for forces. A test that the A-side and B-side smoothed normals of
  adjacent cells are within ~30° of antiparallel would catch regressions.
- **Triple junctions:** only the cells of the two plates count, so the centroids skew near the
  third plate. It never failed in the sample; no action beyond the sign guard.

### 9. (Medium) A2 details to specify
- **Brand-new segments** (slab 0): their Trench cells have conv > min by construction, so c̄ >
  min is automatic and only the obliquity test decides. Fine once finding 4 is applied.
- **`last_contact` and `accretionary`** are refreshed only in the growth branch. A failing
  strike-slip segment without overlap expires after `polarity_memory` (20 Myr) with up to
  ~600·e⁻² ≈ 80 km of slab left. That slab vanishes at once, and the polarity is lost. Decide
  explicitly (it is probably fine) and mention it.
- **Mean |s| vs signed c̄:** acceptable. A curved segment (radius 500 km, so ~1000 km long) can
  pass with a strike-slip end; the per-cell `t` metric will keep some oblique cells. Expected:
  "fewer", not "a few %".
- The 20° gate equals the metric's 20°, so the headline metric drops partly by construction.
  Validate with independent shapes as well (finding 10).
- **Physics citation:** the gate is defensible, but the best analogue is the **western
  Aleutians**: obliquity rises westward, subduction and arc volcanism die out where motion is
  within ~20° of strike. Puysegur/Alpine and Queen Charlotte fit as well. Keep Bird's 20° as the
  consistency argument, not as the justification.

### 10. (Medium) Missing tests and validation metrics
Tests (in addition to those above):
- the A1 sign-guard/sliver test; slab pull per true length independent of resolution;
- a rotating segment (Euler pole inside it) under A2;
- `stall_time` carried by `Polarities::merge` (max, like `collision_time`), zero for new split
  pairs, included in `snapshot()`;
- a converging pair keeps its boundary for > 2 × `suture_time` **and** a microcontinent on it
  still accretes (finding 1).

Validation guard rails to add:
- sutures/Myr, terrane accretions/Myr, plate count and time at `max_plates`, continental plate
  count, largest continent's share (assembly);
- durations of active collisions (Earth: Himalaya 50+ Myr);
- the **full subduction angle histogram** vs Earth's head-on skew (1 3 3 6 11 12 16 23 25),
  which is independent of the 20° gate;
- gate flicker (finding 3); dynamic topography range and sea level (mantle forcing changes);
- net slab pull and ridge push per km; the share of smoothed · raw < 0 rows.

Keep `min_trench_obliquity` = 0 in the sensitivity set; it separates A1 plus the net-motion rule
from the gate.

### 11. (Low-medium) Conflicts with #42
- Both PRs rewrite `boundary_cells` (#42: `localised` gate, the `Diffuse` kind on `segment.rate`)
  and `update_slab_lengths` (#42: `displacement`, localisation, slab growth still on the raw
  per-cell map).
- Semantics: after A1, #42's Trench test (raw) and Diffuse test (smoothed rate) become
  consistent. Good.
- The slab-growth rule must combine: grow only if `localised && c̄ > min && gate`. #42's
  `displacement` should use the same c̄.
- **#42 localises a diffuse segment when `rate` > 1 cm/yr, whatever the obliquity.** A
  near-strike-slip contact with c̄ > 1 cm/yr would localise into a thrust that A2 then refuses
  to feed. Apply the obliquity gate to localisation too.
- Suture ratio: #42 counts `Diffuse` as trench cells. That is compatible with the stall rule.
- Practical: land A1 as its own first commit, so #42 can rebase onto `boundary.normal` = smoothed
  and drop its `normals` argument.

### 12. (Low) Recommendation 4: "no change" is supported, with two caveats
I reproduced the table exactly from the dumps (`check.py`):
- plates per snapshot in the bins < 2% / 2–10% / ≥ 10% trench share: 0.76/2.52/10.18;
- area-weighted speeds 1.37/1.93/5.52 cm/yr;
- low–low boundary 6.3%, median relative speed 2.15 cm/yr, 47% slower than 2 cm/yr.

**Frame:** the model's net lithospheric rotation is small (mean 0.56, max 1.3 cm/yr). In the NNR
frame the speeds are 1.59/2.03/5.45 cm/yr, so the conclusion does not depend on the frame (state
this instead of the "order of magnitude" caveat). Relative speeds are frame-free anyway.

Caveats:
- (a) Small sample: < 1 slab-free plate per snapshot.
- (b) The row "boundary between two plates with trench share < 10%: 6.4% vs 48%" compares
  different definitions. The Earth figure is PR #63's slab ≥ 150 km count. Recompute Earth from
  PB2002 with the same trench-share rule, or label the row as indicative.

`cls` in the cells csv: 0 = continental origin (felsic > 0.5 and resists subduction), 2 =
oceanic, 1 = neither (thick arcs/plateaus). The table does not depend on it (trench share comes
from the boundary dump). If a version of the script used `cls == 0` for "continental plate", note
that this is continental *origin*.

Pre-register the revisit criterion, e.g. "median relative speed between slab-free plates
> 2.5 cm/yr after A2 + #42".

---

## Smaller points
- `smoothed_normals` inside `boundary_cells` allocates a `stamp` of mesh size per call and walks
  ~37 cells per boundary row. Negligible; reuse the existing statistics/events calls as planned.
- The plan says the slab detaches "within ~10 Myr" after subduction stops. The model's pull
  e-folds in 10 Myr but break-off (< 20 km) comes after ~34 Myr from a full slab. Say "pull
  e-folds in 10 Myr".
- `stall_time` decaying at 1:1 like `collision_time` is fine. Document that alternating
  50/50 stall/convergence never sutures.
- Persistent collisions stack crust up to the 75 km cap and then spill outwards (topology
  `spread_crust`), so elevation is bounded but plateaus widen. `crustal_flow` does not cross
  converging boundaries. Watch the share of crust > 50 km (calibrated at 2–6%) and the highest
  cell.
