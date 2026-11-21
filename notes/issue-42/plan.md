# #42 follow-up plan: diffuse convergent zones and resisted initiation events

_2026-11-21. Issue #42; request on PR #63 ([comment](https://github.com/ISibboI/world-simulator/pull/63#issuecomment-7057768507)),
gaps listed in [the bot's reply](https://github.com/ISibboI/world-simulator/pull/63#issuecomment-7057735389).
Builds on batch 2 (#63, merged): [`../realism-batch-2/results.md`](../realism-batch-2/results.md).
Goal of the project: an Earth-like planet, not Earth. Earth numbers are plausibility checks._

## What is wrong (in the code on `main`)

Every overlap of two plates at a cell goes through `Polarities::contact`, which creates a segment
(polarity, slab 0) at once, and `resolve_ownership` then **subducts** the lower plate's crust and
builds arc crust, whatever the state of the boundary. #63 added a plastic strength
(`initiation_resistance`, 3 × 10¹² N/m, fading as the slab grows to 150 km) to the force balance,
but nothing in the crust's treatment depends on it. Three gaps:

1. **Initiation events skip the resistance.** Passive-margin and fracture-zone splits
   (`seed_margin_subduction`) and induced boundary initiations (`induce_subduction`) start a
   segment with a 150 km slab. The resistance fades with the slab length, so it is zero there from
   the first step.
2. **Driven contacts yield in a few Myr, and yield straight into a trench.** A contact converging
   against the strength either is locked (creep < `locked_convergence` = 0.1 cm/yr, a numerical
   cap) or yields; nothing in between. Physically, forced (induced) initiation is real
   (Puysegur, Hjort; Gurnis et al. 2004; Hall et al. 2003), so a strongly forced contact *should*
   localise. What is missing is the regime below the strength.
3. **No state between "no contact" and "trench".** A slab-free contact that converges consumes
   crust like a trench and builds an arc.

Earth for orientation (PB2002, Bird 2003; own count from `PB2002_steps.dat`): of the convergent
boundary length in oceanic lithosphere, 17,400 km are **oceanic convergent boundaries without
subduction (OCB)** against 51,300 km of subduction zones, i.e. **~25% of it is not a trench**.
Their convergence is slow: median 0.67 cm/yr (quartiles 0.38–1.4 cm/yr), against 5.4 cm/yr at
subduction zones. The best-studied example, the Central Indian Basin, has shortened at
0.25–0.6 cm/yr for 8–15 Myr by distributed reverse faulting and folding over ~1000 km without
forming a trench (Bull & Scrutton 1992; Gordon, DeMets & Royer 1998; DeMets et al. 2005).
Strain rates ~10⁻¹⁶ s⁻¹. The model (#63 measurement): 12.9% of converging ocean–ocean contact
rows are not part of a trench, and the batch-2 review traced most of that to the zig-zag raw
normals, i.e. it is not a physical state.

## Physics used

- A convergent zone in oceanic lithosphere without a through-going thrust deforms by distributed
  shortening: the crust and lithosphere thicken; nothing goes into the mantle and there is no arc.
  It creeps viscously below the lithosphere's strength: the Central Indian Basin converges at
  0.25–0.6 cm/yr at stresses near the strength; OCB on Earth converge at < 1.4 cm/yr on 75% of
  their length.
- Subduction initiates where the driving force (forcing from the plates, or the negative buoyancy
  of a foundering slab) exceeds the resisting strength, ~10¹² – 10¹³ N/m (McKenzie 1977; Mueller &
  Phillips 1991; Toth & Gurnis 1998; Gurnis et al. 2004; Stern 2004). Then the zone localises on
  one thrust (the weak zone), and the lower plate underthrusts. It becomes self-sustaining after
  ~100–150 km of convergence on the thrust (Hall et al. 2003; Gurnis et al. 2004), which #63
  already models (`self_sustaining_slab_length` = 150 km).
- Spontaneous initiation (passive margin, fracture zone) supplies a weak zone and the negative
  buoyancy of old foundering lithosphere, but still has to overcome the strength (Stern 2004;
  Stern & Gerya 2018); induced initiation at an existing boundary is forced by convergence.

## Change

### A. Segment state: localised or diffuse (`polarity.rs`)

New segment fields:
- `localised: bool`: whether the convergent zone has a through-going thrust (it is a trench where
  it converges). Segments created by a contact start **diffuse** (`false`). Segments set up by
  the initiation events and by `seed_subduction` (initial configuration, tests) start localised.
  A polarity flip (`end_pass`) keeps it localised (the thrust exists; only its vergence changes).
  Merging: localised if either was.
- `displacement: f64` (m): convergence taken up on the thrust since it localised. It grows with
  the slab at the same rate (in `update_slab_lengths`), and is reset to 0 by a flip and by the
  events. Merging takes the larger.
- `shortening: f64` (m): convergence taken up by distributed shortening while diffuse
  (diagnostic, and for the log).
- `rate: f64` (rad/Myr): mean convergence of the segment's lower-plate boundary cells from the
  last velocity solve, with the smoothed normals (see C).

A diffuse segment that stops converging stays diffuse; a localised one whose slab broke off and
decayed to 0 goes back to diffuse after `polarity_memory` without contact like today (it
expires), or immediately when its slab is gone and its `displacement` is below the
self-sustaining length (a failed initiation leaves no lasting thrust). _Decision for the
reviewer: the simpler alternative is to keep a localised segment localised until it expires._

### B. Classification (`dynamics.rs::boundary_cells`)

- `Trench` requires a **localised** segment (plus today's conditions).
- New `BoundaryKind::Diffuse`: the plate is the lower plate of a diffuse segment, its crust does
  not resist subduction, and it converges (raw convergence > `min_trench_convergence`, as for
  trenches). No force of its own (like `Other`); it carries the initiation strength (D).
- The dump writes `d` for it; the log reports the diffuse length and its convergence.

### C. Localisation rule (`update_plate_velocities`, `update_slab_lengths`)

After the velocity solve, each segment gets `rate` = mean smoothed-normal convergence over its
lower-plate boundary cells (non-buoyant crust). In `update_slab_lengths` (after the step's
motion):
- A diffuse segment with `rate` > `diffuse_convergence` **localises**: the contact has reached the
  strength and yields (see D: below the strength, a diffuse zone creeps at most at
  `diffuse_convergence`). Otherwise `shortening += rate · dt` where positive.
- Slabs grow only on localised segments (today: all segments with trench cells).

### D. Force balance (`initiation_strength`, the Picard loop)

- The strength fades with **`displacement`** instead of the slab length: `S · (1 − D / 150 km)`.
  For contact-born segments D equals the slab length, so nothing changes there; event-born
  segments start with a 150 km foundering slab (its pull acts) but D = 0, so they feel the full
  strength (gap 1). A 150 km slab of 80–100 Myr old lithosphere pulls with 3.1–3.7 × 10¹² N/m
  (`slab_buoyancy`), about the strength: whether an event succeeds is decided by the forcing of
  the plates around it, which is what spontaneous-initiation studies conclude.
- **Diffuse creep**: at diffuse contacts (diffuse segment, or no segment yet) the viscous lock
  below the strength uses the new `diffuse_convergence` instead of `locked_convergence`: the
  coupling is `S / max(v, v_d)`, so a contact forced below the strength creeps at
  `v = v_d · F / S`, and at the strength it yields. Localised trenches keep `locked_convergence`.
- `diffuse_convergence` = **1 cm/yr**: Earth's diffuse oceanic zones converge at 0.25–0.6 cm/yr
  (Central Indian) and OCB at a median 0.67, upper quartile 1.4 cm/yr; across a zone of
  ~300–1000 km this is a strain rate of ~10⁻¹⁶ – 10⁻¹⁵ s⁻¹. Sensitivity: 0.5 cm/yr on 3 seeds.

### E. Crust at diffuse contacts (`resolve_ownership`)

Where the overlapping crust belongs to the lower plate of a **diffuse** segment (and does not
resist subduction anyway), it is **stacked** onto the upper plate (a new `AdditionKind::Shortening`
with the collision profile, sediment folded in as metasediment like in collisions), not subducted:
no arc, no sediment subduction, no subduction erosion, no slab. New ledger terms
`shortening_consumed` / `shortening_stacked`, logged in the material budget. Thickened oceanic
crust is still oceanic below 12 km; the change is mass-conserving.

### F. Events (`events.rs`)

- `seed_margin_subduction` and `induce_subduction` set up localised segments with the 150 km
  foundering slab and D = 0 (see D). No change to their hazards.
- Diffuse ocean–ocean contacts with old lithosphere on one side are already candidates for
  induced boundary initiation (`BoundaryKind::Other` today → also `Diffuse`).

## Gap 2, explicitly

Strong forcing still makes a contact yield: that is induced initiation and happens on Earth. What
changes: (1) below the strength a contact now creeps as a diffuse zone instead of being either
locked or a trench; (2) yielding first has to localise and then take up 150 km of displacement
against the strength before the resistance is gone, also for events. Not done (would be tuning):
making the strength grow with age or scaling it to make more contacts lock.

## Expected direction

- Non-trench share of converging (> 0.1 cm/yr, smoothed) ocean–ocean contact rows: 12.9% → up,
  plausibly 15–25%, mostly slow (< 1 cm/yr).
- Locked share: down (creep replaces lock); trench length: down a little; incipient trench length:
  down (slow incipient trenches become diffuse).
- Successful event initiations (slab to 300 km) / initiation segments: down; slab-free plates and
  boundary between slab-free plates: up a little.
- Sea-floor production: down a little (less trench consumption at slow contacts and fewer
  successful events); sea-floor age up; young-floor subduction share: down a little.
- Relative plate speed: about unchanged (slow contacts are a small fraction).
- New shortening flux: small compared with subduction (< 5%).
- Runtime: about unchanged (one more pass over the boundary cells per step).

## Tests (unit)

- `contacts_start_diffuse_and_stack_crust`: a new oceanic overlap without a localised segment
  conserves crust (ledger: shortening consumed = stacked, no subduction, no arc).
- `a_diffuse_zone_creeps_below_its_strength`: two plates pushed together below the strength
  converge at ~`v_d · F / S`; above it, the contact yields and localises at the next slab update.
- `localised_segments_subduct_and_grow_slabs`; `diffuse_segments_build_no_slab`.
- `event_slabs_feel_the_initiation_strength`: a seeded 150 km slab with D = 0 is resisted with
  the full strength, and not at all once D ≥ 150 km.
- Existing tests that rely on immediate subduction at a fresh contact are moved to localised
  segments (seeded), not weakened.

## Validation

8 seeds (0–7) × 1000 Myr, `main` (= #63) against the branch, same flags
(`--headless --cell-dump --width 256 --snapshot-interval 50`), Welch t over 500–1000 Myr.
Metrics: contact classification of converging ocean–ocean and ocean–continent rows (trench,
overriding side, diffuse, other; locked share), slab-free plates and boundary between them
(plate-structure script of batch 2), sea-floor production (kinematic), trench and incipient
trench length, young-floor (< 20 Myr) share of the subduction flux, relative and mean plate speed,
initiation success. Guard rails (batch 2's list): sea-floor age and ≥ 200 Myr share, sediment-part
slope, hypsometry (highest cell, mean land), flooded continental crust, collision stacking,
dynamic topography minimum, plate count and births, mass ledger residual, runtime (paired, ≤ +10%).
Pre-#63 `main` numbers are quoted from batch 2 (those run outputs are gone).

## Revisions after the plan review ([`review-plan.md`](review-plan.md))

What changed against the sections above (the review's finding numbers in brackets):

1. **Events stay completed initiations by default** [1]. Their hazards are calibrated to
   initiations that succeeded (83% of initiation segments reach 300 km on `main`), and a failed
   passive-margin event would leave a split plate that nothing merges back. New parameter
   `events.initiation_displacement` (default 150 km = the seeded slab: no strength left). Gap 1
   then is closed by definition: the resistance is inside the calibrated hazard. The alternative
   (displacement 0: the foundering slab works against the full strength, hazards become attempt
   rates) is run as a 3-seed sensitivity with its success rate, and goes to Sebastian as a
   decision.
2. **New segments inherit the thrust along strike** [2]: in `contact()`, a new segment next to a
   localised one (within 2 × radius, where it also inherits the polarity) is localised with that
   displacement. Only contacts away from any localised segment of the pair start diffuse.
3. **Everything overlapped at a diffuse segment is stacked**, from either side [3].
4. **Sediment stays sediment** [4]: the crystalline crust is stacked with its own composition,
   the sediment is moved onto the same cells as sediment (`add_sediment`, same profile). (The
   plan's "like in collisions" was wrong: the collision path folds sediment into the basement.)
5. **Displacement is not slab length** [5]: it never decays, so after a slab breaks off or decays
   a mature thrust stays weak (`main` restores the strength there). Listed as a change; the
   log's "incipient trench" keeps the slab-based definition for comparison with batch 2.
6. **Displacement grows from the smoothed segment rate** of the solve, only above
   `min_trench_convergence` [7, 8]; slab growth stays on the raw normals as on `main` (not
   touched here). `Diffuse` is classified by segment state plus the smoothed segment rate.
7. **Localised stays localised until the segment expires** [9]; no revert rule. Localisation is
   decided from a per-segment yield flag (most lower-plate cells converge faster than
   `diffuse_convergence` in the solve), and transient stacking (mass stacked by segments that
   localise within 2 Myr of creation) is logged.
8. **Ocean–continent contacts without a segment** carry the full strength on the oceanic side
   [9]: only that side can subduct.
9. **Why the localised thrust is stiffer below the strength than the diffuse zone** [7]: a fault
   below its frictional strength is locked; distributed ductile shortening creeps. Gurnis et al.
   (2004) find the incipient force balance controlled by cumulative convergence, not rate.
10. **Suturing** counts diffuse cells like trench cells [13], so former incipient trenches still
    hold pairs apart as on `main`.
11. **Metric by segment state, locked rows included** [6]: share of the converging (smoothed)
    rows where oceanic crust is the lower plate that are diffuse vs localised, ocean–ocean and
    ocean–ocean + ocean–continent (like PB2002's OCB/(OCB+SUB), of which 25% lies in Bird's
    orogens). The speed distribution of diffuse rows is bounded by `diffuse_convergence` by
    construction and validates nothing above it.
12. **Expected direction, revised** [6]: the diffuse state takes over the population that #63
    locks below 0.1 cm/yr plus slow fresh contacts; expect a **few per cent**, not Earth's 25%.
    Earth's diffuse zones deform intact lithosphere, which does not localise (Gurnis et al.
    2004), while in the model every contact is a plate boundary; a higher strength for intact
    lithosphere, or a strength growing with plate age (physics, not tuning), is a separate
    decision for Sebastian, not done here.
13. **Guard rails added** [10]: shortening per diffuse segment (mean, max), oceanic crust thicker
    than 12 / 17 km, mafic mass stacked onto continental crust (felsic + sediment net,
    continental area). If long-lived zones thicken too much, strain-weakening localisation is
    the physical option; named, not tuned in.
14. Sensitivity: `diffuse_convergence` 0.5 cm/yr (the Central Indian Basin rate) on 3 seeds.

## Relation to the #43 investigation

The independent #43 investigation ([`notes/issue-43/investigation.md` on `agent/issue-43`](https://github.com/ISibboI/world-simulator/blob/agent/issue-43/notes/issue-43/investigation.md))
finished while this was being implemented. Where this change stands to its points:

- **Its mechanism 1 / recommendation 2 (slab-less convergent class for slow ocean–ocean
  contacts)** is what this change adds, with one difference: here a diffuse zone localises when
  it *yields* (converges faster than it can creep at its strength), not after a fixed
  ~100–150 km of shortening. The 100–150 km enters as the displacement over which the strength
  of a localised thrust fades (Gurnis et al. 2004 find cumulative convergence controls the force
  balance of an incipient thrust; Central Indian–type zones take up tens of km without
  localising). A shortening threshold for localisation is the strain-weakening option named in
  revision 13.
- **Its mechanism 2 / recommendation 1 (raw normals; slabs that never decay on strike-slip
  contacts)** is left as it is: slab growth, slab decay and the trench test of localised segments
  still use the raw per-cell normal and the converging cells only. This change uses the smoothed
  normals only where it adds state: the displacement on a thrust, the yield test and the
  `Diffuse` classification. Not taken on, as Sebastian has not decided on it.
- Its recommendations 3–5 (suture timing, driving of slab-free plates, ridge orientation): not
  touched. Suturing counts diffuse cells like trench cells so that its behaviour stays as on
  `main`.
