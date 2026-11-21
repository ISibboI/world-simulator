# Review of the #42 follow-up plan (diffuse convergent zones, resisted initiation events)

_2026-11-21, independent reviewer. Plan read at `notes/issue-42/plan.md`; code read at `main`
(288f9cf, = #63) and in the uncommitted start of the implementation in this worktree. Nothing was
built or run. PB2002 numbers recomputed from `PB2002_steps.dat` (copy in
`~/tasks/issue-43/tmp/pb2002/`). Event success on `main` from the issue-43 reference log
(`~/worktrees/issue-43/agent-scratch/runs/main/s0/log`)._

**Verdict: sound direction, revise before implementing.** The state machine (diffuse → localised
trench) and the displacement-based fading are good physics, and Gurnis et al. (2004) support both:
homogeneous plates do not localise, and the force balance depends on cumulative convergence, not on
the rate. But two parts change calibration without saying so. Resisting the events double-counts
failure against hazards calibrated to successful initiations (finding 1). Folding sediment into
stacked basement moves a guard rail (finding 4). New segments along existing trenches would start
diffuse and chop mature trenches (finding 2). And the expected rise of the non-trench share mostly
comes from the choice of `diffuse_convergence` and the metric threshold, not from a new physical
state (finding 6). The physics citations check out (finding 11).

## Findings, most important first

1. **Resisting the events is a hidden recalibration of the trench budget, and failed events leave
   split plates behind.** The hazards are calibrated to *observed outcomes*, i.e. successful
   initiations. `invasion_waiting_time` comes from "three Atlantic invasions within ~50 Myr" (Scotia,
   Lesser Antilles, Gibraltar, all successful). `initiation_waiting_time` comes from "the 180 Myr
   old Atlantic margins have not subducted" (events.rs:44-56). On `main`, 83% of the initiation
   segments already grow to 300 km (seed 0: 278 of 335). That is the calibrated success rate.
   If the strength now acts on them (D = 0), fewer succeed, and "successful event initiations:
   down" in *Expected direction* is the double counting itself, presented as a result.

   Two further effects:
   - Every passive-margin and fracture-zone event first **splits the plate** (`split_locally`).
     No rule merges the pieces back when the initiation fails (sutures need continental
     collision). Each failed event therefore leaves a new slab-free plate pair with a
     locked or diffuse boundary. That is the "slab-free plates up a little" the plan expects,
     but it is an artefact.
   - Event slabs are 150 km long and the events fire only at ages ≥ 80 Myr. Their pull is
     3.1 × 10¹² N/m at 80 Myr, 3.7 at 100 and 4.2 at 120 (`slab_pull_max_age`), against
     S = 3 × 10¹². So whether an event succeeds sits on a knife edge (P/S = 1.05 at the
     youngest allowed age).

   *Change (pick one, and put it in the PR as a decision for Sebastian):*
   - (a, recommended) Keep event segments as **completed** initiations: localised, with
     D = `initiation_slab_length`, so no strength. State in plan.md that gap 1 is closed by
     definition: the hazards stand for successful initiations, so the resistance is already
     inside them.
   - (b) Resist them, but then (i) reinterpret the hazards as attempt rates, (ii) merge a failed
     split back into one plate (event segment never reached the self-sustaining displacement and
     its slab decayed; the pair has no other boundary), and (iii) report the success fraction
     against `main`'s 83% and the successful initiations per 100 Myr. If those drop, raising the
     hazards back is a declared recalibration, not physics.

   Under (a), `displacement` is not needed for the events at all (see 12).

2. **New segments along an existing trench would start diffuse: mature trenches get chopped.**
   On `main` (seed 0, 1000 Myr) 5,756 segments were created, 3,504 merged and 5,407 expired,
   against 335 initiation segments. Most created segments are re-segmentations next to an
   existing segment of the same pair: they inherit its polarity in `contact()` (within 2·radius)
   and are later merged. Under the plan, every one of them starts diffuse with `rate` = 0. It
   stacks crust instead of subducting for at least the rest of its first step and the whole
   next step, because `rate` is first set at the next velocity solve and localisation happens
   in the next `update_slab_lengths`. At 5 cm/yr and dt = 1 Myr, that is 50–100 km of oceanic
   crust stacked onto the arc side inside a mature trench. A merge at the end of the step
   rescues only segments within 0.5·radius.

   *Change:* in `contact()`, a new segment inherits `localised` (and D) from the same
   along-strike neighbour it inherits the polarity from. Physically, a thrust propagates along
   strike from an existing trench tip. Start diffuse only where no localised segment of the pair
   is within 2·radius. Note that inheriting D also removes `main`'s full strength on these
   pieces (today they restart with slab 0 and feel the full S). That removes a batch-2 artefact.
   Report it as a change, with the incipient-trench share on `main` and on the branch. Add a
   unit test for it.

3. **At diffuse segments the overriding plate's oceanic crust still subducts.** In
   `resolve_ownership`, where continental crust of the lower plate stays on top (`contact()`
   returns `bottom`), the loser is the *upper* plate's crust. If that crust is oceanic, it goes
   down the subduction branch and builds an arc on the winner, at a contact that has no slab.
   The plan stacks only "crust that belongs to the lower plate of a diffuse segment".

   *Change:* at a diffuse segment, all overlapped crust that does not resist subduction is
   stacked onto the winner, whichever side it comes from. Never arc, erosion, fill or
   `SubductedSediment` there. A diffuse segment needs no flip either: polarity only says which
   side is stacked on. If you keep flips, say that a flip does not localise a diffuse segment.

4. **"Sediment folded in as metasediment like in collisions" is not what the collision branch
   does, and copying it moves guard rails.** The collision branch (boundaries.rs:301-322) adds
   sediment into the *basement*. It uses `felsic = (felsic + sediment) / mass` and
   `metasediment: 0.0`, so the sediment leaves the sediment stock. At diffuse oceanic contacts
   this would:
   - Lower the **sediment part of the sea level and its slope**, the long-running guard rail
     (target ≤ 10 m/100 Myr), by conversion rather than by physics.
   - Shift density. 7 km oceanic crust plus ≥ ~4.5 km of sediment (old ocean near margins has
     5–9 km) stacks to < 2850 kg/m³, so the column stops being oceanic (`is_oceanic_column`) and
     becomes "transitional" with felsic ≈ 0.25–0.35. It then subducts as
     `SubductedTransitional`, is excluded from the initiation candidates and from the ocean-age
     metrics, and so on.

   *Change:* stack the crystalline crust with its own composition. Move the loser's sediment onto
   the receiving cells **as sediment** (`add_sediment`, same profile), so compaction, burial
   metamorphism and the sediment budget handle it as everywhere else. Book it as a ledger/budget
   term of its own ("sediment moved at diffuse zones"). Correct the plan's description of the
   collision path.

5. **"For contact-born segments D equals the slab length, so nothing changes there" is false.**
   - D only grows; the slab decays. On `main`, a trench whose slab decays (stalled contact) or
     breaks off (`break_off`, used by terrane accretion and collisions) gets the full strength
     back. Under the plan, D ≥ 150 km keeps the strength at zero after break-off and decay.
   - Merging takes the larger slab and the larger D independently.
   - Flips reset both, which is consistent.

   Physically the plan is better: a mature thrust stays weak (fault healing is slow). But it
   removes resistance that `main` has in these cases, so it must be listed as a change and
   measured. Count segments with D ≥ 150 km and slab < 150 km, and how much convergence they
   take. Also redefine the log's and dump's "incipient trench" as D < 150 km, and keep the old
   slab-based number for comparison with batch 2.

6. **The headline metric would rise by construction of `diffuse_convergence`.** Under the new
   coupling `S / max(v, v_d)`, a contact yields (F > S) exactly when it converges faster than v_d.
   Contacts that do not yield are the ones `main` keeps *locked* below 0.1 cm/yr, and those are
   **excluded** from the metric "converging (> 0.1 cm/yr)". With v_d = 1 cm/yr they creep at
   0.1–1 cm/yr and enter the denominator as non-trench. The non-trench share therefore rises
   because v_d > 0.1 cm/yr, even if no contact changes its force state. Batch 2 measured that
   population as small: 1.4% of ocean–ocean contacts locked, and ~1.8% of the boundary carrying
   a strength outside trenches, of which 15–43% converge. The contacts that yield localise at the
   next step and behave as on `main` afterwards. The likely outcome is therefore a diffuse share
   of a few per cent, not 15–25%. The 12.9% "non-trench" of `main` is mostly raw-normal artefact
   (batch-2 review finding 3) and stays.

   *Change:*
   - Classify by **segment state**, not by speed: the length share of diffuse vs localised over
     all rows where oceanic crust is the lower plate and converges by the smoothed normal,
     **locked rows included**. Report the locked share separately.
   - Compare like with like. PB2002's 25% is OCB / (OCB + SUB), and SUB includes ocean–continent
     subduction, so use ocean–ocean + ocean–continent, not ocean–ocean alone.
   - Report the speed distribution of diffuse rows, but say that it is bounded by v_d by
     construction: it cannot validate anything above v_d.
   - Rewrite *Expected direction* honestly (small; the population batch 2 locked). If
     Sebastian wants Earth's ~25%, the physical lever is not v_d. It is that in the model every
     contact is an old plate boundary (a weak zone), whereas Earth's diffuse zones (Central
     Indian Basin) deform intact lithosphere, which Gurnis et al. 2004 find does not localise.
     A higher strength for contacts on freshly split, intact lithosphere is a separate,
     physically motivated decision. Name it in the PR; do not do it here.

7. **The localised thrust creeps 10× slower below the strength than the diffuse zone.**
   Localised segments keep `locked_convergence` (0.1 cm/yr), diffuse ones use v_d (1 cm/yr), so
   at the same sub-yield force the localised fault is stiffer than intact lithosphere. This is
   defensible: a fault below its frictional or plastic strength is locked, while distributed
   ductile shortening creeps, and Gurnis et al. 2004 find the incipient force balance
   rate-independent. *Change:* write this argument into plan.md, or the reviewer of the results
   will flag it. Either way, D must grow only on yield or on real slip above
   `min_trench_convergence` (see 8), never by creep below the strength. That is batch-2 finding
   3 in a new place.

8. **Raw vs smoothed normals: D inherits batch 2's known gap.** `update_slab_lengths` grows slabs
   (and now D) from the raw zig-zag normal of `boundary_cells`. Batch 2 measured that 35% of the
   incipient trench length is locked in the solver (smoothed) but still grows a slab (raw). Since
   D now carries the strength, that error removes the resistance at locked contacts.
   *Change:* compute `rate` (smoothed, from the solve) for **every** segment, localised ones too,
   and grow D by `max(rate, 0) · dt` only where `rate > min_trench_convergence`. Leave slab
   growth on the raw normal if you do not want to touch mature subduction in this change, and say
   so. Classify `Diffuse` (dump `d`, log) by segment state plus smoothed convergence, not raw.

9. **Ordering and state transitions need precise rules.**
   - The plan says a localised segment goes back to diffuse "immediately when its slab is gone
     and D < 150 km". Read literally, that reverts every freshly localised segment (slab 0,
     D 0), every flip (slab and D reset to 0) and every event under (b). *Change:* revert only
     on the transition in `update_slab_lengths` where a slab that existed falls below
     `min_slab_length`.
   - **Decision asked in section A:** take the simpler alternative and keep a localised segment
     localised until it expires (`polarity_memory`, 20 Myr without contact). The thrust is a
     lasting weak zone, which is physical, and there are fewer transitions to get wrong. The
     strength comes back anyway through D < 150 km. With 7 and 8 in place, a failed thrust does
     not creep its way to self-sustaining.
   - `rate` for merged segments: take the length-weighted mean or the maximum. Do not keep a
     stale 0 from the younger one.
   - Localisation in `update_slab_lengths` uses the `rate` of the solve at the start of the same
     step. That is consistent, but a segment created during the step only gets a rate at the next
     solve (see 2). Measure "stacked mass at segments that localise within 2 steps of creation"
     (transient stacking). If it is not negligible, decide localisation in `contact()` from the
     solve's yield state of the (Other, None) cells there. Those cells already carry
     0.5 S + 0.5 S in the solve, so the solve knows whether they yield.
   - The solver's yield test has a 5% tolerance. Store a per-segment `yielding` flag from the
     converged solve (coupling at target and v > v_d) and localise on that, rather than
     re-deriving yield from `rate > v_d`.
   - (Other, None) ocean–continent contacts carry only 0.5 S (batch 2 left this). With v_d they
     now yield and localise at half strength. Fix it here (the oceanic side carries the full S
     when the other side resists subduction), since it now decides localisation.

10. **Shortening can turn ocean into plateaus and continents: guard it.** Stacking all 7 km onto
    the upper plate with the collision profile (4 rings, ~450 km) thickens it by roughly
    7 km × (shortening / ~2.5 cells). A zone that creeps at 0.5–1 cm/yr for 50 Myr reaches about
    18 km. That passes `SUBDUCTION_RESISTANCE_BUOYANCY` (basalt ≳ 17 km), and at ≳ 25 km basalt
    the crust counts as continental by buoyancy. Then the zone becomes a "collision" or a
    terrane. Earth's Central Indian Basin took up only some tens of km over 8–15 Myr.
    *Change:* add guard rails: the distribution of shortening per diffuse segment and of diffuse
    lifetimes; oceanic-origin crust thickened past 12 / 17 km by shortening (cells, area); mafic
    mass stacked onto continental crust at ocean–continent diffuse contacts (it dilutes the
    felsic fraction; check felsic + sediment and continental area). If long-lived zones are
    common, the physical answer is strain-weakening localisation: a zone localises after a
    finite shortening. Name it as an option; do not tune it in.

11. **Physics and numbers: checked.**
    - PB2002 reproduces exactly from the steps file: OCB 17,447 km (13,181 outside and 4,267
      inside orogens), SUB 51,309 km; OCB convergence quartiles 0.38 / 0.67 / 1.38 cm/yr; SUB
      median 5.4 cm/yr. Say that 25% of the OCB lies in Bird's orogens. Other sources quote a
      Bird (2003) subduction length of ~61,900 km (EarthByte), so give the class definition.
    - Central Indian Basin: reverse faulting accelerated at ~8–7.5 Ma after slow motion before.
      Kinematic rates are a few mm/yr up to ~1 cm/yr along the zone. 0.25–0.6 cm/yr is fair;
      "8–15 Myr" is fair (onset debated). Gordon, DeMets & Royer 1998 is in *Nature* 395.
    - Gurnis, Hall & Lavier 2004 (G³ 5, Q07001): peak resisting force 2–3 × 10¹² N/m for cases
      that become self-sustaining, controlled by cumulative convergence, not by its rate, and no
      self-sustaining subduction from a homogeneous plate. That supports S = 3 × 10¹², fading
      with D, and a rate-independent localised law (7). The 100–150 km figure is cited from Hall
      et al. 2003 and Gurnis et al. 2004 by later work (e.g. Mao et al. 2017); fine.
    - Range ~10¹²–10¹³ N/m: fine; the upper end is intact lithosphere (Mueller & Phillips
      1991), which is the point of 6.
    - Strain rate: 1 cm/yr over 300–1000 km is 3 × 10⁻¹⁶ to 1 × 10⁻¹⁵ s⁻¹, as stated.
    - v_d = 1 cm/yr is at the upper end of the Central Indian Basin rates. A zone at the strength
      should creep at about the Central Indian Basin's rate, which suggests ~0.5 cm/yr. Keep 1 or
      0.5, but because of 6 report the sensitivity in the *state-based* metric.
    - Minor: "making the strength grow with age would be tuning". It is physics (the peak force
      depends on plate thickness; Gurnis et al. 2004). Out of scope, but call it that.

12. **Simpler alternatives.**
    - With 1(a), `displacement` is needed only for the fade-after-break-off behaviour (5). If you
      do not want that change, drop D and keep fading with the slab length as on `main`. The new
      state is then just `localised` (plus diagnostic `shortening` and `rate`/`yielding`).
    - Localise directly at the end of the Picard loop from the per-segment yield flag (9),
      instead of storing `rate` and testing it later.
    - Keep `BoundaryKind::Diffuse` (it is cheap and makes the dump readable), but drive all
      physics by segment state, not by the kind.

13. **Other interactions to handle or state.**
    - **Suturing:** `suture_colliding_plates` counts only `Trench` cells against collisions.
      Former incipient trenches that are now diffuse no longer hold pairs apart, so sutures can
      come sooner. Count diffuse cells with the trenches to keep `main`'s behaviour, or state the
      change.
    - **Invasion:** `near_own_trench` uses `Trench` only. Fine, but invasion is now slightly
      rarer near transiently diffuse segments.
    - **Induced boundary initiation:** with `Diffuse` as a candidate kind, a diffuse zone with
      ≥ 80 Myr crust gets the boundary hazard (150 Myr per 1000 km). Under 1(b) that event adds a
      150 km slab while D = 0, i.e. pull out of nowhere against an unchanged strength. Under 1(a)
      it is a completed initiation, as today.
    - **Ledger:** `MassLedger::terms()` returns a fixed-size array (17). Add
      `ShorteningConsumed` / `ShorteningStacked` (and sediment moved, 4), and check closure in the
      new test and in the 8-seed runs (residual guard rail already listed).
    - **Mantle and topography:** `subduction_rates` / `sinking_slabs` stay `Trench`-only. Good:
      no downwelling at diffuse zones.
    - **Tests:** keep one *integrated* multi-step test in which a fresh, strongly forced contact
      localises and then subducts with an arc (not only seeded segments). Add tests for: flip
      keeps localised and does not revert (9); inheritance along strike (2); D does not grow at a
      contact locked by the smoothed normal but converging by the raw one (8); upper-plate
      oceanic crust at a diffuse segment is stacked, not subducted (3); sediment stays sediment
      (4). The initial planet seeds its trenches via `seed_subduction` (localised, D = slab).
      Non-seeded initial contacts start diffuse, which is harmless after the 500 Myr window.
    - **Runtime:** negligible (one pass to average `rate`). The Picard loop starts from
      `S / v_d` for diffuse contacts, which should not raise the iteration count; keep the
      unsettled-solve counter in the comparison.

## Verdict

Implement the diffuse state and the stacking, with these changes:
- the events stay completed initiations (1a), or option 1(b) with its three conditions;
- new segments inherit localisation along strike (2);
- everything at a diffuse segment is stacked, and sediment stays sediment (3, 4);
- D grows from the smoothed rate and never by sub-yield creep (7, 8);
- localised segments stay localised until they expire (9).

Measure by segment state, locked rows included, like-for-like with PB2002 (6). Expect, and say in
advance, a small diffuse share: the diffuse state captures the contacts batch 2 locked. Earth's 25%
needs a different strength for intact lithosphere, which is a decision for Sebastian.
