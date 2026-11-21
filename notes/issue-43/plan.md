# Issue #43: plan for recommendations 1, 3 and 4

_2026-11-21. Requested on #43 ([comment](https://github.com/ISibboI/world-simulator/issues/43#issuecomment-7058488260)):
"self-orienting ridges are out of scope for this project. Implement your other recommendations."
Investigation: [`investigation.md`](investigation.md). Goal of the project: an Earth-like
planet, not Earth; Earth numbers (PB2002) are plausibility checks, not targets._

**Out of scope for the project (Sebastian, 2026-11-21): self-orienting ridges** (ridge segments
perpendicular to spreading plus transform offsets). At ~110 km cells most real offsets are
sub-grid.

**Recommendation 2** (a slow, slab-less convergent class for ocean–ocean contacts) is implemented
by the #42 task (diffuse convergent zones). Not touched here; see "Interaction with #42".

## 1. Trenches on smoothed normals; slabs decay under strike-slip motion (recommendation 1)

### What is wrong (on `main`, 288f9cf)
- `Simulation::boundary_cells` (`dynamics.rs`) uses the raw per-cell normal (sum of directions to
  the neighbours of the other plate), which scatters by ~20° and more on the zig-zag boundary.
  This normal sets the trench test (`convergence > min_trench_convergence`, or slab and
  `> −min_trench_convergence`), the direction and existence of slab pull, slab resistance and
  ridge push, and the ridge test (`convergence < 0`). Only shear coupling, collision resistance
  and initiation resistance use `smoothed_normals` (3-ring centroids).
- `update_slab_lengths` grows each segment's slab by the **mean over its converging trench cells
  only** (cells with raw convergence > 0.1 cm/yr), and the slab decays only when **no** trench
  cell converges. On a strike-slip contact some cells always see spurious convergence, so the
  slab never decays.
- Measured (investigation §3): 15% of trench length moves within 20° of strike (Earth 4%,
  5–8% with 20° of noise added to Earth's azimuths); 7.5% of trench cells diverge along the
  smoothed normal.

### Physics
Subduction consumes lithosphere at the rate of convergence normal to the trench; a slab is fed
only by that normal component, integrated along the trench. Where motion becomes nearly parallel
to the boundary, the normal component vanishes; strongly oblique convergence is partitioned
(Sumatra: trench-normal subduction plus a strike-slip fault in the upper plate, McCaffrey 1992;
Fitch 1972), and at very low obliquity subduction stops and the boundary becomes a transform
(e.g. the Queen Charlotte margin, the Alpine fault where the Puysegur subduction ends). After
subduction stops, the slab detaches within ~10 Myr (slab break-off, Davies & von Blanckenburg
1995), which the model already represents by `slab_decay_time` = 10 Myr and break-off below
`min_slab_length` = 20 km.

### Change
**A1. One boundary normal.** `boundary_cells` computes the raw normals as now, then the smoothed
normals (existing `smoothed_normals`, 3 rings), and stores the smoothed normal in
`BoundaryCell::normal` and the convergence along it in `BoundaryCell::convergence`; the
classification (trench test, ridge test) uses them. So slab pull, slab resistance, ridge push and
everything else downstream use the smoothed normal. The callers that recompute
`smoothed_normals(&boundaries)` (dynamics, events, statistics) use `boundary.normal` instead
(same result, saves the second computation).

**A2. Slab growth from the segment's net motion.** In `update_slab_lengths`, per segment over
all its trench cells (`Trench` kind, the subducting side): mean normal convergence c̄ (signed;
diverging cells count) and mean |strike-slip| s̄ (from the plates' angular velocities and the
strike r × n). The slab grows by c̄ · dt only if
- c̄ > `min_trench_convergence` (0.1 cm/yr, as now), and
- the motion is not nearly parallel to the trench: c̄ ≥ tan(`min_trench_obliquity`) · s̄, with
  `min_trench_obliquity` = **20°** (angle between the mean relative motion and the strike; the
  same 20° as Bird's transform criterion, i.e. a contact that PB2002 would class as a transform
  builds no slab).

Otherwise it decays as now (10 Myr, break-off below 20 km). The accretionary/erosive switch uses
c̄. The new segment helper is a separate function (`segment_motion`), so the #42 change can call it.

Per-cell hysteresis in `boundary_cells` stays (a cell with a slab stays a trench while the
smoothed normal does not clearly diverge), so a decaying slab keeps pulling until it breaks off,
as on Earth.

### Expected
- Trench length within 20° of strike: 15% → a few per cent; those contacts become transforms or
  convergent-without-slab (the #42 class, once merged) or keep only a decaying slab.
- Trench length down by up to ~15% (~18 thousand km), transforms up by up to that (upper
  estimate, no feedback).
- Slab pull on the plates concerned down → relative speeds and sea-floor production down a
  little; mean sea-floor age up a little.
- Kinematic production measure changes partly by measurement (spurious opening of raw normals
  disappears; #32 put it at a few per cent): the "created" production (cells actually filled)
  and the young-floor age estimate are the unbiased checks.
- Ridge test on the smoothed normal: fewer flickering ridge cells on oblique boundaries.

## 2. Suture when the collision has stalled (recommendation 3)

### What is wrong
`suture_colliding_plates` (`events.rs`) merges a colliding pair (≥ 10 colliding cells, at least as
many colliding as trench cells) after `suture_time` = 30 Myr of collision, whatever the plates
do. On Earth a collision stays a plate boundary as long as the plates converge across it (India–
Asia ~4 cm/yr after 50 Myr, Arabia–Eurasia ~2 cm/yr after ~25–35 Myr); the plates merge only when
the convergence has ceased (Urals, Appalachians).

Measured on `main` (8 seeds, 500–1000 Myr, colliding pairs with ≥ 10 cells in a snapshot, 22
pair-snapshots): mean convergence across the collision −0.4 / 0.0 / 0.55 / 1.05 / 2.1 cm/yr
(10/25/50/75/90%); 45% of the pairs converge slower than 0.5 cm/yr (collision resistance locks
them). So a stall criterion will **lengthen some collisions and shorten others**; the net effect
on the length of continental convergent boundaries is uncertain and is measured, not assumed.

### Change
`PairState` gets `stall_time`: it grows while the pair collides (same colliding-cell conditions)
**and** the mean convergence over its colliding cells (smoothed normals) is below
`events.suture_convergence` = **0.5 cm/yr**, and decays otherwise (like `collision_time`). The pair
sutures when `stall_time` ≥ `events.suture_time`, re-defined as the time the collision must have
been stalled: **20 Myr** (enough to rule out pauses between pulses; break-off of the last slab
~10 Myr after collision onset, plus the time the collision resistance needs to lock). A collision
that keeps converging faster than 0.5 cm/yr does not suture; it stays a collision boundary, can
accrete terranes (unchanged rule, `terrane_accretion_time` = 12 Myr: docking of small pieces is
quick on Earth) and can be cut later. `collision_time` stays as the measure that blocks terrane
accretion while a pair is on its way to a suture.

Threshold: Earth's continental convergent boundaries (PB2002 CCB) move at a median 2.1 cm/yr, 49%
below 2 cm/yr; the slowest active collision belts (Alps, ~0.2–0.5 cm/yr) are still boundaries in
PB2002, so 0.5 cm/yr is at the slow end. Sensitivity: 0.25 cm/yr on 3 seeds.

### Expected
- Pairs that keep converging persist as collision boundaries; stalled pairs suture after 20 instead
  of 30 Myr. Continental convergent length: uncertain sign (measured). Collision stacking and
  maximum elevation can rise where collisions persist (guard rails).

## 3. Driving of slab-free plates (recommendation 4): measured, no change planned

Measured on `main` (8 seeds, 500–1000 Myr; trench share = the plate's subducting cells / its
boundary cells):

| | model | Earth |
|---|---|---|
| plates ≥ 1% with trench share < 2% / 2–10% / ≥ 10% (per snapshot) | 0.8 / 2.5 / 10.2 | |
| area-weighted mean speed of those plates (cm/yr) | 1.4 / 1.9 / 5.5 | slab-free plates ~1–2 (Africa, Eurasia, Antarctica, Americas, NNR frame); Pacific, Nazca, Australia ~6–8 |
| boundary between two plates with trench share < 10% | 6.4% | 48% (PR #63's count, slab ≥ 150 km) |
| … median relative speed; share slower than 2 cm/yr | 2.15 cm/yr; 47% | 1.5 cm/yr; 59% |

Slab-free plates move at Earth-like speeds, and the boundaries between them are slow. The
continental drag factor and the mantle flow do not make them fast; the deficit is how few such
plates and such boundaries there are, which A2 and #42 address. **No change to the driving.** If
the results of A2 and #42 together leave the relative speeds between slab-free plates
clearly above Earth's, revisit.

(The speeds are in the model's frame, the mantle reference frame of the flow field; Earth's are
in the no-net-rotation or hotspot frame. The comparison is order of magnitude.)

## Tests (unit)
- `boundary_normals_follow_the_boundary_not_the_mesh`: two hemispheres in pure strike-slip on a
  zig-zag (icosphere) boundary: |convergence| at every boundary cell well below the relative
  speed (raw normals give up to ~half of it); no trench, no ridge cell.
- `strike_slip_slabs_decay`: a seeded segment with a 600 km slab between plates in strike-slip
  motion loses its slab within a few `slab_decay_time` and the cells stop being trenches.
- `oblique_convergence_beyond_the_threshold_grows_a_slab`: motion 45° from strike grows the slab
  at the normal rate; 10° from strike does not.
- `stalled_collisions_suture_and_converging_ones_do_not`: two continental plates locked by
  collision resistance suture after `suture_time` of stall; the same pair pushed together faster
  than `suture_convergence` does not suture after 2 × `suture_time`.
- Existing tests that rely on raw-normal behaviour are checked; adapted only where the old
  expectation was the noise.

## Validation
- 8 seeds (0–7) × 1000 Myr, `main` (288f9cf) vs branch, `--headless --cell-dump --width 256
  --snapshot-interval 50`, Welch t over 500–1000 Myr (8 seed means each).
- Boundary classification (`sim_classes.py`): subduction share and length, intra-oceanic
  subduction, trench length within 20° of strike, transform and ridge shares, convergent without
  slab, continental shares, slower than 2 cm/yr, median relative speed.
- Guard rails (from the log): sea-floor production (kinematic and created), mean sea-floor age and
  ≥ 200 Myr share, mean plate speed, plate count, slab pull, collision stacking, mean land and
  highest cell (hypsometry), flooded continental crust, sediment-part slope, mass ledger, runtime
  (paired timing, seeds 0–1).
- Sensitivity (3 seeds): `min_trench_obliquity` = 0 (only A1 and the net-motion rule), and
  `events.suture_convergence` = 0.25 cm/yr.
- One independent review of results and diff; fix and re-run what changes.

## Interaction with #42 (diffuse convergent zones, in progress, not yet pushed)
- #42 changes `boundary_cells` (a `Trench` needs a localised segment; new `Diffuse` kind) and
  `update_slab_lengths` (displacement from a smoothed segment rate), and leaves slab growth on the
  raw normals. Both changes touch the same two functions: whichever PR lands second rebases.
  A1 makes #42's separate smoothed-normal pass redundant (`boundary.normal` is smoothed); A2's
  `segment_motion` gives the same per-segment rate #42 computes in `update_segment_rates`.
- Suturing: #42 counts diffuse cells like trench cells; compatible with the stall rule.

## Runtime
One more `smoothed_normals` pass inside `boundary_cells` (called 2–4 times per step), minus the
passes it replaces in the velocity solve, events and statistics: expected within a few per cent.

## Revisions after the plan review ([`review-plan.md`](review-plan.md))

Finding numbers of the review in brackets.

1. **Suture rule** [1, 6, 7]: a pair counts as **stalled** while fewer than `suture_min_cells`
   (10) of its collision cells converge faster than `events.suture_convergence` = 0.5 cm/yr
   (smoothed normals); this also handles pairs converging at one end and opening at the other.
   `stall_time` grows while the pair collides (today's conditions) **and** is stalled, decays
   otherwise; the pair sutures at `stall_time` ≥ `suture_time`, which **stays 30 Myr**, so a
   collision locked from the start sutures exactly as today and only converging collisions change.
   "On its way to a suture" (no terrane accretion) is `stall_time > 0`, so converging pairs keep
   accreting terranes. `stall_time` merges by max, is zero for new pairs, and is in `snapshot()`.
   The reviewer counted 210 eligible pair-snapshots (median 0.85 cm/yr), of which 63–80% would
   no longer suture under a mean-convergence rule: plate count, `max_plates` saturation and
   continent assembly become guard rails. Sensitivity: the old rule
   (`events.suture_convergence` = 10⁹, i.e. always stalled; 3 seeds) separates the effect of
   the suture change from A1/A2.
2. **The obliquity gate also decides whether slab-free cells are trenches** [2]. Per segment, after
   each velocity solve, `rate` (mean normal convergence) and `strike_rate` (mean |strike-slip|)
   over **all lower-plate boundary cells of the segment whose crust does not resist subduction**,
   smoothed normals [4] (the same definition as #42's `update_segment_rates`, so both can share
   it). `subducting` = rate > `min_trench_convergence` and rate ≥ tan(20°) · strike_rate.
   `boundary_cells`: a cell is a trench if its segment has a slab and the cell does not clearly
   diverge (hysteresis, as now), or if the segment is `subducting` and the cell converges. A
   slab-free contact moving within 20° of strike is no trench (it does not consume as a trench in
   the force balance, feeds no downwelling, counts as `o`). New segments start not subducting until
   the next solve (one step).
3. **Slab growth** uses `rate` if `subducting`, else decays. A segment with lower-plate cells keeps
   `last_contact` refreshed and its accretionary flag (from `rate`) also while failing [9]. The
   decaying slab's own pull can reopen the gate [3]: that is self-consistent (the pull drives
   normal convergence), but it is counted (log: segments whose gate reopened while their slab
   decayed) and the unit tests use solved velocities.
4. **Force magnitudes per true boundary length** [5]: smoothed normals no longer cancel partly
   along the zig-zag, which raises normal forces per plate pair by ~16% (10–90%: 6–27%). The
   force terms used the edge length without the zig-zag factor; slab pull, ridge push, slab
   resistance, initiation strength and the subduction fluxes to the mantle now use
   `boundary_length_per_cell()` (÷ 1.1, as the statistics and shear coupling already do). Net:
   ~+6% normal force per true length against `main`, declared and measured (slab pull, ridge push
   per km, mantle flux, dynamic topography as guard rails).
5. **Sliver guard** [8]: where the smoothed normal points against the raw one (0.4% of rows:
   slivers, one-cell necks), the raw normal is used.
6. **Physical analogue of the 20° gate** [9]: the western Aleutians (motion nearly parallel to
   the arc west of ~180°: no active slab feeding, a strike-slip boundary) and the Queen Charlotte
   margin. Bird's 20° is the consistency argument; the headline "trench within 20° of strike"
   drops partly by construction, so the **full subduction-angle histogram** against Earth's
   head-on skew is the independent check.
7. **Tests added** [10]: a contact 10–15° from strike builds no trench and no slab (solved
   velocities); a segment rotating about a pole inside it; a converging microcontinent still
   accretes; `stall_time` through merge/split/snapshot; slab pull per true length independent of
   resolution; sliver fallback.
8. **Guard rails added** [8, 10]: sutures and terrane accretions per Myr, plate count, time at
   `max_plates`, continental plates, largest continent's share, dynamic topography minimum, sea
   level, gate reopenings.
9. **Rec 4** [12]: in the no-net-rotation frame the speeds are 1.6 / 2.0 / 5.5 cm/yr (net rotation
   0.56 cm/yr), so the frame does not matter. Revisit criterion, fixed now: median relative speed
   between slab-free plates above 2.5 cm/yr after A1/A2 and #42.
10. **#42** [11]: A1 and the segment rates go into their own first commit so #42 can rebase onto
    it; the combined rule would be "slab grows if localised and subducting"; #42's displacement
    should use the same `rate`.
