# Review of `plan.md` (#75 orogenic healing, #53 plume heating in the plate frame)

_2026-11-21. Independent review of the plan, checked against the code at the plan's base
(weak_zones.rs, crust.rs, boundaries.rs, topology.rs, events.rs, mantle.rs, mod.rs,
matching.rs). No simulations were run._

Overall: the mechanism is sound and a clear improvement. One memory field per process on the
sample, carried by the plate, linear decay that commutes with mass mixing, and a strength-only
age reset that leaves bathymetry alone. The time-scale arithmetic is right:
L²/(π²κ) = 50 Myr for 125 km and 200 Myr for 250 km, with κ = 1e-6 m²/s. The main gaps are
two. First, the calibration of the weakening amplitude is no longer consistent with its source
once it heals. Second, the measurements cannot answer the craton question that the decision
rule is about.

## Must-fix

1. **The amplitude `SUTURE_WEAKENING = 0.5` loses its justification once it heals.** Its doc
   comment derives 0.5 as the thickness ratio of *Phanerozoic* orogenic belts to cratons,
   ~100–150 vs 200–300 km (Artemieva & Mooney 2001). That population is 0–540 Myr old and is
   dominated by belts older than 200 Myr. With τ = 200 Myr the plan heals those belts to
   0.82–0.97, so the source that set the amplitude now contradicts the result. The plan's own
   new source (Hyndman et al. 2005) puts young mobile belts at 50–60 km against ~200–250 km,
   which is a t = 0 ratio of ~0.25–0.3, not 0.5. The plan should either:
   - (a) take the t = 0 amplitude from young or active belts (Hyndman) and use the
     Phanerozoic average only as a cross-check of amplitude × healing, or
   - (b) keep 0.5 and state that it is now a young-belt value with a different source.

   Either way, rewrite the `SUTURE_WEAKENING` doc ("for good", the A&M ratio) and the
   `Crust::orogenic` doc. This is a consistency fix, not tuning: pick the number from data on
   belts of the right age before the runs, not after.
2. **Healing to zero discards the evidence the weak-zone model was built on, and the plan
   should present that as a choice.** The classic Wilson-cycle case is the North Atlantic. It
   opened ~55–60 Ma along Caledonian/Iapetus sutures that closed ~400–420 Ma, ~350 Myr
   earlier. At that delay the plan gives a strength of 0.91–0.93, nearly healed. Tommasi &
   Vauchez 2001 attribute the reactivation to olivine fabric, which is permanent. Heron et al.
   2016 describe mantle scars that last for more than 1 Gyr. The plan names this as "not
   modelled", but the decision rule then offers only one alternative, a craton keel term. The
   "permanent residual fraction" option (heal towards 1 − (1−w)·f_perm instead of 1) is the
   other physically motivated alternative. Fix: list it next to the keel term as a decision
   for Sebastian if the measured contrast fades. Don't add it pre-emptively, because it is a
   new free parameter.
3. **The metrics cannot answer the craton decision.** The weighted rift contrast (mean
   `orogenic` on rift boundaries vs in the rifted plates) asks "does the cut find the currently
   weak cells?". It will stay above 1 almost by construction, because young belts keep being
   made and the cut minimises over `orogenic`. It does not ask whether old cores remain
   distinct. Initial continents start unmarked (strength 1), and healed belts return to
   strength 1. After ~1 Gyr a healed belt and an original core are identical, which is the
   failure the plan's own decision rule names. Add metrics that can show it:
   - the **never-marked continental area** (`orogeny == NO_OROGENY`) and the area last marked
     more than 500 Myr ago, over time to 3 Gyr, on main and the branch;
   - the share of rift-boundary continental cells in never-marked crust divided by its share of
     the rifted plates (do rifts avoid the original cores?), per 1 Gyr window.

   Base the craton decision on these, not on the weighted ratio alone.
4. **Make the rift statistics robust and count the sample size.** 8 runs × 1000 Myr gave ~220
   plume rifts. A 1 Gyr window on the 4 long seeds therefore has only ~100 rifts, and the
   per-seed values in batch 4 ranged from 22% to 73%. As `orogenic` decays, the denominator
   (mean `orogenic` in the rifted plates) becomes small and the ratio noisy. Fixes:
   - Log the raw sums (the plan does) plus the number of rifts per window.
   - Report a threshold version as well: the share of boundary cells with `orogenic ≥ 0.25`
     vs the share of that area in the plates.
   - Give per-seed ranges.
   - State the ≥ 1.5 rule relative to main's 0–1000 Myr value (1.6–2.0 in batch 4). A fixed
     1.5 sits inside main's current spread.
5. **Fix where the reheating runs in the step.** The plan places it "after the plate events,
   before the plume rifts of the next step". In `step` the order after the plate events is:
   `mantle.step` (plumes age, die, are born), then the next step's substeps (plates move),
   then `crustal_flow`, then the next `plate_events`. The claim "cells above the plume right
   now carry the same cap as before" holds only if the reheating runs **immediately before
   `plate_events`**: after the substeps and `crustal_flow`, with the plume list as it is when
   `rift_above_plumes` and the margin cuts in `initiate_subduction` read it. At dt = 1 Myr the
   offset is < 1 cell, but with `--dt` larger it is v·dt. Put it there and pin the order with
   a test: reheat, then cut, in the same step.

## Should-fix

6. **Heated trails can steer plume rifts away and make more of them fail.** Since #52,
   divisions whose boundary lies more than half a head radius from the plume are rejected,
   and a plume does not try again. A trail of heated lithosphere ~1000 km long behind the
   plume (20 Myr × 5 cm/yr) gives the cut a cheaper path along plate motion, so:
   - rift orientation becomes biased along the absolute plate motion;
   - more splits may be rejected.

   Add counters for **plume rifts that failed for good**. Batch 4 counted these from logs:
   35 → 93. Also add the angle between the rift boundary near the plume and the plate's
   velocity at the plume, so a systematic alignment would show.
7. **Tails now write long weak tracks, also in old ocean.** A tail heats a 500 km radius to
   the full 25 Myr-equivalent cap for 60–180 Myr, and each heated cell stays weak for 75 Myr.
   That gives a strip ~1000 km wide and up to ~3700 km long behind every live plume. The
   margin and fracture-zone initiation cuts (`initiate_subduction`) use the same strength, so
   oceanic hotspot tracks can attract initiations. Do two things:
   - Name it under Risks.
   - Check initiation counts and geometry in the guard rails (already listed). Optionally
     look at how many initiation boundary cells have `reheated` set.

   Physically, a tail's buoyancy flux is much smaller than a head's, and Von Herzen et al.
   1989 (cited in the plan) argue for little reheating under oceanic swells. Reusing the
   head's centre cap for the tail is the generous end. Either justify it or scale the tail's
   heating by `flux / peak_flux`: that profile already exists and includes the end-of-life
   decay, so it adds no new parameter.
8. **`PLUME_HEAD_STAGE = 20 Myr` equals `rift_delay = 20 Myr`.** That is chosen so the rift
   sees the head heating. It is plausible on its own but couples two parameters: raising
   `rift_delay` later would silently change plume rifts. Note the coupling in the constant's
   doc, or define the head stage independently and make sure the result doesn't hinge on the
   equality (head-heated cells persist via max anyway; only cooling since matters).
9. **Plume-heated continents recover in 75 Myr, orogens take τ = 200 Myr.** The plan's own
   table says rebuilding thick continental lithosphere takes ~200 Myr, yet plume-heated (and,
   in the existing code, stretched) continental lithosphere regains full strength in
   75–100 Myr on the plate-model law. This may be acceptable, since a plume only partly
   thins the base, but it should be stated, not left implicit.
10. **Places in the code the plan doesn't list:**
    - `matching.rs::assert_same_state` compares crust field bits. Add `reheated`, or the
      incremental-vs-full determinism tests won't cover it.
    - `topology.rs::add_crystalline` (plate merges, spill). The plan says "merges keep the
      receiver's", so name the function so nobody mixes there by accident.
    - `stretch_rift`: the donors' `birth` is reset by stretching from their thermal fraction,
      which ignores `reheated` because the thermal state ≠ the strength age. Fine, but
      document that the plume heating doesn't feed the stretching heat budget.
    - Healing: apply it once per step with the step dt, not per substep, to all `Some` samples
      of all plates. Precompute `exp(−dt/τ)`.
    - The main.rs cell dump header and the log line (main.rs ~l.400/434) for the new
      statistics.
11. **Tests.**
    - Add a test that a cut near a plume with *no* reheating applied is no longer weakened.
      This documents the removal of the mantle-frame field and catches a forgotten
      reheating call in `step`.
    - Add an end-to-end `step` test: a plume of age ≥ 10, a few steps, `reheated` set on the
      cells above it and on cells that have moved off.
    - The three plume-rift tests set plumes directly. Calling the reheating once before the
      split reproduces the old field only if the plume's `age ≥ 10` (head_growth = 1).
      Check their ages; several use `lifetime: 200.0` with explicit ages.
12. **Steady-state estimate.** The batch-4 rate of 3–6 points per 100 Myr is the *net* growth
    of marked area: re-marking cells that were already 1 added nothing on main. With healing,
    re-marking partly healed belts counts again, so the gross rate is closer to the early 5–6
    points. The expected bound is therefore ~10–15%, not 6–12%. Minor, but don't call 12–15%
    a deviation.

## Optional

13. Citations: Stein & Stein 1992 (GDH1) uses a 95 km plate, not ~125 km; ~125 km is Parsons
    & Sclater 1977. Cite them separately. Hyndman, Currie & Mazzotti 2005, GSA Today 15(2),
    is the right paper. Their later work, Currie & Hyndman 2006 (JGR), gives a thermal
    relaxation time of ~300 Myr for backarc/mobile-belt lithosphere, so 200 Myr sits at the
    fast end. Quote it as within the range rather than as derived. Check the quoted
    sentences verbatim before putting them in the notes. Crough 1983, Wessel 1993, Courtillot
    & Renne 2003, Campbell & Griffiths 1990, Tommasi & Vauchez 2001, Heron et al. 2016 and
    Artemieva & Mooney 2001 fit the uses made of them.
14. The "Atlantic ~200 Myr after closure" example is right for the central Atlantic (Acadian
    and Alleghanian ~380–300 Ma, rifting ~230–200 Ma), not for the North Atlantic (see 2).
    Say which one is meant.
15. The 3000 Myr main runs are what show whether #75's premise ("contrast vanishes in
    2–3 Gyr") is real. If main's contrast doesn't fade either, say so. The bounded weak area
    alone still justifies the change.
16. Metric 1, "strength < 0.75 from all causes": restrict it to continental cells or report
    continental and oceanic separately. Otherwise young sea floor dominates it.

## Not tuning, as far as I can see

τ follows from the thickness, and the plume profile is today's. Only `PLUME_TAIL_RADIUS` and
`PLUME_HEAD_STAGE` are new free numbers; both are sourced, and they should be fixed before the
runs (see 7, 8). Choosing the amplitude after seeing the contrast numbers would be tuning
(see 1).
