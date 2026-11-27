# Review: plan for propagating plume rifts (#51, option 3)

_2026-11-27, independent review of [`plan-propagation.md`](plan-propagation.md) against
`breakup.rs` (`analyse_sections`, `most_overloaded_section`, `plume_sections`),
`weak_zones.rs::magma_weakening`, `mantle.rs::Plume::flux`, `topology.rs` (timer resets). No runs
or builds. Goal: an Earth-like planet, not Earth; ~110 km grid effects accepted (#64)._

**Verdict: approve with changes.** The model is a reasonable physical idea, but its strength
comes from one assumption it does not price (magma along the whole section, from any plume
stage), and the existing timer resets will likely stop long rifts from completing. Both are
fixable without tuning; a cheap diagnostic should come before the 8-seed evaluation.

## 1. Physical argument

* The criterion is sound as a **necessary** condition: a rigid-plastic limit analysis has no
  tip stress concentration, so `S_rift ≥ 1` is a global work balance (loads can pay for
  opening the whole section at the dyke-assisted strength). That is Buck's (2004, 2006) point:
  where dykes are fed, the force needed is the dyke-opening force, almost independent of rift
  length. It becomes **sufficient** only through the assumption that magma follows the tip.
  For a partially opened rift `S(x)` grows monotonically with the rifted length `x`, so without
  that assumption the binding condition is the start (`x` = footprint), which is the PR's
  ordinary plume section (≥ 1 in 0.9%). The plan should say plainly that the whole model rests
  on the magma assumption, not on the tip-concentration argument (which is elastic and does not
  enter the calculation).

* **[High] Magma along the whole section is effectively a second parameter set to ∞.** Sections
  cross 4000–10 000 km; the cited evidence (giant dyke swarms 1000–2500 km, Ernst & Buchan 1997;
  channelling > 1000 km in Afar, Ebinger & Sleep 1998) bounds the reach at ~2000–2500 km.
  Applying `m = 0.1` along a 10 000 km section is outside that evidence. Recommendation: either
  (a) a finite reach from the cited literature (opening strength × `m` within ~2000 km of the
  plume along the section, ramping to 1 beyond; the rest must yield amagmatically, which a rift
  linking up with other weak zones can do), or (b) keep the unbounded model but run a reach
  sensitivity arm (2000 km) as part of the evaluation, since this assumption, not `v_p`, decides
  how many plates rift. (a) is closer to "physics before tuning": the value is cited, not fitted.

* **[Medium] Tails rift as easily as heads.** `magma_weakening` gives `m = 0.1` at the centre
  whenever `flux = peak` (10 Myr to lifetime − 20 Myr), i.e. through the whole tail stage
  (~60–160 Myr), with only the footprint shrinking. Breakup on Earth goes with LIP heads
  (CAMP, Paraná–Etendeka, NAIP, Deccan, Afar); long-lived tails under moving plates (Hawaii,
  Réunion) feed no plate-crossing rifts, partly because tail melt supply is ~10× smaller and
  Buck's reduction depends on supply versus extension rate. Not necessarily a change to make,
  but measure and report the plume age at rift onset (head vs tail stage); if most rifts start
  in the tail stage, restrict the whole-section magma to the head stage (an existing constant).

* **Is the criterion too permissive?** Rough estimate from results.md: the PR's best plume
  sections are ≥ 0.25 in ~15% of analyses with magma only over the head chord (worth ~1.3–2×).
  With the whole section × 0.1 in opening (floored at 0.05), an opening-dominated section gains
  ~5–8× more, so `S_rift ≥ 1` roughly where today `S ≳ 0.15–0.2`: perhaps 20–30% of analyses,
  ~0.5–1 plate at a time with a rift-loaded section. With `t_cross` 20–60 Myr that suggests
  ~1–3 rifts per 100 Myr, so not obviously runaway, but very uncertain (opening fraction,
  shear-dominated sections). See the diagnostic pre-check below.

* **Plate drift vs. v_p.** Plates move over plumes at 1–10 cm/yr, the same order as `v_p`. In
  40 Myr the plume moves ~2000 km under the plate, so the section analysed at the break may be
  far from where the "rift" started (the timer carries over). Accepted as an approximation in
  the plan; at least record the plume's displacement (plate frame) between rift onset and break.

* Oceanic plates over plumes get the same `m`; their low loads should keep `S_rift` small, but
  report the continental share of rifted pieces (main's old rule required continental crust).

## 2. The parameter v_p = 5 cm/yr

* Order of magnitude and "middle on a log scale" are defensible: √(1.5 × 14) ≈ 4.6 cm/yr. Not
  tuned, sensitivity at the ends: good.
* Citations to check before they go into the code/PR (I cannot verify all from memory):
  * **Gulf of Aden: likely garbled.** The oldest oceanic crust (~17.6 Ma, d'Acremont et al.
    2010; Leroy et al. 2004) is in the **east** (Alula–Fartak area), not at Shukra el Sheik
    (~45–46°E, west). Propagation to Tadjoura over ~1000+ km in ~15 Myr gives ~5–9 cm/yr, not
    ~2. Manighetti et al. (1997, JGR) is the standard reference for Aden–Tadjoura propagation
    rates; please re-derive the number from distances and ages.
  * **Woodlark ~14 cm/yr (Taylor et al. 1995):** matches my recollection of the order (≥ 10
    cm/yr westward propagation); fine.
  * **Jourdon et al. 2020 ~1.5 cm/yr:** I cannot confirm the value; in such models propagation
    speed scales with the extension rate, so quote the model's conditions or drop it.
  * **South Atlantic:** breakup propagated northward from the south (Falkland/Cape segment) and
    reached Paraná–Etendeka; the Walvis/Rio Grande zone was rather a temporary barrier. It is
    not a rift "from the plume". The rate (~4000 km in ~30–40 Myr, ~10 cm/yr) is fine as a
    propagation example; reword the attribution.
  * Possibly add: Red Sea / Main Ethiopian Rift propagation (Wolfenden et al. 2004), oceanic
    propagating rifts (Hey 1977, Galápagos, a few cm/yr).
* Note in the parameter's doc that real rates scale with driving force/extension rate; a
  constant `v_p` ignores that a strongly overloaded plate rifts faster (acceptable).

## 3. Implementation pitfalls

* **[High] Timer resets kill long rifts.** `split_plate_cells` and `merge_plates_inner`
  (`topology.rs` ~300–311, 357–358) zero `breakup_timer`; the plan resets `rift_timer` the same
  way. With `t_cross` 20–100 Myr and ~10 splits + ~7 merges per 100 Myr (larger plates get most
  of the merges and initiations), many rifts will be reset before completing, biased against
  exactly the large plates that should rift. Suggest: on a split, keep `rift_timer` on the part
  that contains the rift's plume (the other part 0); on a merge, keep the receiving plate's. At
  minimum, count rifts lost to resets (timer > 5 Myr at reset) in the statistics.
* **[Medium] Timer carries across plumes.** If the best rift section switches to another plume,
  time accumulated by the first counts for the second. Plumes have no id (`Vec` with
  `retain`); add one (or store the plume position) and reset or decay the timer on a switch.
* **[Medium] Sheet split and #182 in the same step:** `split_deforming_plates` runs before
  `break_overloaded_plates` (events.rs 389–390) and clears `pending_breakup`; a pending rift
  break must be cleared too (stale cell lists after a split). Store it separately
  (`breakup_rift`) and make sure the reset paths cover it.
* **[Low] `propagation = 0`:** `L / 0` is `inf`, but `L = 0` (head covers the section) gives
  `0/0 = NaN`; handle "off" explicitly, not through NaN comparisons.
* **[Low] Footprint jump at 20 Myr:** the head→tail switch shrinks the footprint 1000 → 500 km
  and lengthens `L` by 500 km (`t_cross` +10 Myr) in one step; fine, but mention it.
* **[Low] Refinement:** with uniform `m` the min-cut follows `s` except where the floor binds;
  the band (±3 cells) can move the cut off the plume's cell. Harmless; check `through_plume`
  on the refined cut. `m` for the rift should be computed with `math`-based functions (it is,
  via `magma_weakening`) and evaluated at the plume's cell, not the plume position, to avoid a
  subtle frame mismatch with `mesh.nearest`.
* Cost and determinism as planned look fine (one extra edge list per wedge; ties in `max_by`
  are deterministic). The extra `section_edge` calls in the 42-wedge quick pass dominate; reuse
  one pass with both tension values in `SectionEdge::halves` rather than calling it twice.
* Tests: add one for the reset policy (split elsewhere keeps the rift timer) and one where the
  plume dies mid-propagation (timer runs back, no break).

## 4. Evaluation

Adequate in scope (three arms, 8 × 1000 Myr, supercontinent start, runtime), with additions:

* **[High] Diagnostic pre-check first (cheap):** compute `S_rift` and `t_cross` in diagnose mode
  with rift breaks off (state hash unchanged) on 2–4 seeds × 400 Myr; predict the rate from it.
  This catches a runaway or dead criterion before the expensive runs.
* **[High] Pre-register success criteria** as the previous plan did (e.g. plume rifts 1–3 per
  100 Myr; rift-type splits near plumes above the antipode null; largest plate back near main's
  ≤ ~36%; a supercontinent split within 500 Myr on ≥ half the seeds).
* Add: reach sensitivity (see §1), `propagation = 0` reproducing the PR head (same state hash),
  fraction of plumes whose plate rifts during their life and their age at onset (head/tail),
  rift length `L` and `t_cross` distributions, continental share of rifted pieces, rifts lost
  to timer resets, plume drift between onset and break, and the small-plate count (rifts near
  `min_area`).
