# Review of the results for #51 / #54

_2026-11-27, independent evaluator. Read: `plan.md` (with revisions), `review-plan.md`,
`results.md`, `git diff 57ae1ed..HEAD -- src`, the run logs in `~/tasks/issue-51/runs/*` and the
tables `cmp-*.md`. No builds or runs; checks by Python over the existing logs._

## Verdict

**The main conclusion holds, with corrections.** The code does what the plan says (swell-load sign
and magnitude right, plume sections evaluated, per-edge tension correct, no bug found that would
suppress plume rifts). The finding that plates don't rift at plumes holds up better than
`results.md` says, because the direct counts (opening breakups and plume-section breakups) back it
regardless of the chance null. But:
a few numbers are wrong; the plume-rift metric mixes in subduction initiations; the antipode null
is biased; the plume-section search covers only one geometry; and the #54 reactivation and
inversion claims are partly circular or misleadingly averaged. Fix the items marked **must**
before the PR. The decision options are fairly framed, but option 1's cost is understated.

## Findings

1. **(must, low) Step-0 numbers are wrong.** The logs give **2 448** analyses (not 2 600),
   split < 0.25 / 0.25–0.5 / 0.5–1 / ≥ 1 = **91.5 / 6.0 / 2.0 / 0.5 %**. The quoted "86%" does not
   come from step 0, and the four shares add up to only 94.5%. 13 analyses ≥ 1 is correct.
   For the `new` arm (1000 Myr): 11 544 analyses, 87.2 / 8.6 / 3.8 / 0.5 %, 52 ≥ 1.

2. **(must, low) Plume-section breakups do occur in `new`.** "No plate broke along one" is true
   for step 0, but in `new` two plates broke along a plume section (seed 4 at ~110 Myr, seed 6 at
   ~140 Myr; `1 through a plume` in the breakup line). The 500–1000 table shows 0, and the
   0–500 table shows 0.05/100 Myr. State it: 2 in 8 000 seed-Myr, and 52 overloads ≥ 1 that
   mostly did not break. A one-line diagnostic would explain why: overloaded but beaten by a
   level section, or not sustained for 5 Myr.

3. **(must, medium) The plume-rift metric is mostly subduction initiations.** Splits ≤ 500 km of
   a plume by cause (all 8 seeds, from the cumulative counts):

   | | new 500–1000 near / antipode | new 0–500 | base (non-rule) 500–1000 |
   |---|---|---|---|
   | #182 opening | 2 / 5 | 6 / 8 | 6 / 16 |
   | sheet | 6 / 8 | 0 / 0 | 5 / 7 |
   | initiation | **23 / 9** | 31 / 18 | 11 / 14 |

   The small excess over chance in `new` (0.78 vs 0.60) is entirely initiations, which are not
   rifts. Rift-type splits (opening + sheet) near plumes are *below* the antipode rate in every
   window and arm. So the conclusion is stronger than stated. Report the threshold on rift-type
   splits, and note the near-plume initiation excess in `new` (23 vs 9, base 11 vs 14). Possible
   causes: lithosphere weakened by heating or magma and pushed by the swell now attracts
   initiation cuts (`lithosphere_strength` is shared). Is this wanted?

4. **(should, medium) The antipode null is biased.** Plumes rise where no slabs sank
   (`mantle.rs`, the candidate with the least downwelling inflow). Their antipodes are therefore
   not random places relative to subduction and plate boundaries. On main, non-rule splits near
   plumes (23) are far *fewer* than near antipodes (40). Either plume surroundings avoid splits,
   or antipodes attract them. Use a null that keeps the plumes' relation to the slabs:
   (a) the plume positions under random rotations (as `review-plan.md` #10 suggested), or
   (b) time-shuffled plume positions from the same run. This does not change the verdict (see 3),
   but don't call 0.6/100 Myr "the chance rate" without a caveat.

5. **(should, medium–high) The plume-section search tests only one geometry.** `plume_sections`
   tries only full great circles through the plume, so the two arms are always 180° apart and
   cross the whole plate. It also picks the orientation by the *unrefined* quick bound. Refinement
   along weak lithosphere only happens if the best overall section is > 0.5, which plume sections
   almost never reach (≥ 0.5 in 2.5% in step 0). In nature, plume rifts are two arms at
   ~120° (RRR junctions) that run from the plume to the nearest weak zone or plate boundary, and
   a third arm fails. Such a section can be much shorter, and it is the geometry #54 is about.
   So "physics alone gives no plume rifts" is shown for straight sections only. Cheap, knob-free
   extension before deciding: candidate sections made of two radial arms from the plume, at
   k·30° pairs, each continued along the min-cut to the plate edge. Alternatively, refine the
   best plume section always, not only above 0.5. If that still gives < 1/100 Myr above chance,
   the conclusion is robust. Until then, say "with straight sections" in the summary.

6. **(should, low) "Strength drops by a factor of ~1.5" is the review's estimate, not a
   measurement.** `results.md` presents it as a result. Either label it as an estimate, or log
   the best plume section's strength with and without the magma factor. Spot-check:
   - With supply (1 − x²), the mean factor along a chord is 1 − 0.9·⅔ = 0.4.
   - The `MIN_STRENGTH` floor binds at the centre of heated lithosphere (s ≈ 0.5 → 0.05).
   - Full magma lasts only from 10 to 20 Myr: `head_growth` ramps the flux over the first 10 Myr,
     and the head-stage radius ends at 20 Myr. After that, the tail radius of 500 km gives a
     ~1 000 km chord.
   - A breakup needs > 1 for 5 Myr, with analyses every 2 Myr.

   These timings are part of the outcome and belong in the explanation. The head's 10 Myr window
   is short compared with Afar, where rifting continued ~30 Myr after the flood basalts.

7. **(ok) Swell load: sign and magnitude.** `plume_swell_gradient` points uphill (tested), and
   `force -= g·ρ_m g L A / R` pushes away from the crest. The magnitude is
   ρ_m g h L = 3300·9.81·650·10⁵ ≈ 2.1 × 10¹² N/m at full flux and L = 100 km, and less with
   L·thermal term (heated lithosphere) and with flux/`plume_flux.1`. That is about ridge push, as
   claimed. #182 gets the load on all plates (`cell_forces(&vec![true; …])` when an analysis is
   due), not only on deforming ones. Per-edge `a`, `b`: opening dissipation = tension, closing =
   compression·strength, sliding unchanged. That is correct and convex. The magma field and the
   mesh are both in the world frame; sheet samples are rotated to world with `plate.rotation`.
   Note: the conclusion depends on #182's `breakup.strength` (5 × 10¹³ N/m). Say so in the
   decision, since a different F changes all breakups, not specifically plume ones.

8. **(ok) Statistics.**
   - Rates from differences of cumulative counts at the window ends: correct (`compare.py`
     `rate`).
   - Welch t over per-seed values: correct.
   - I re-derived the 500–1000 table, the 0–500 numbers and the supercontinent table from the
     logs, and they match.
   - With ~40 metrics, treat |t| < 3 as suggestive. Most headline numbers clear that.
   - The "plates' share" chance level for #54 is computed per seed and then averaged. That is
     fine.

9. **(must, medium) The inversion share is misleading.** "55% [1–96%]" is the unweighted mean of
   per-seed ratios. The seeds are bimodal: 89–96% on 4 seeds, each with little total shortening
   (4 × 10⁴–9 × 10⁵), against 1–27% on the others. Shortening-weighted over all seeds it is
   **~20%** (seed 3 alone has 4.4 × 10⁶ at 1.3%). The ~90% seeds look like what `results.md`
   half-says: the sheet's deforming zones alternate between stretching (≥ 10⁻¹⁵ s⁻¹, so they
   mark fabric) and shortening on the same cells. That is a sheet oscillation, not geological
   rift inversion. `review-plan.md` expected inversion to be rare in the sheet. Report the weighted
   share, and count only shortening on fabric older than ~10–20 Myr (needs a marking time, or
   use the step of the last marking). Otherwise drop "rifts are inverted".

10. **(should, medium) Reactivation 2–3× is partly circular, and "interior = failed rifts" is
    not shown.** By cause, 500–1000, continental cells on fabric (boundary vs split plates):
    - opening: 37% vs 20%;
    - sheet: 40% vs 14%;
    - initiation: 14% vs 7% (interior 0%: margin fabric from `stretch_rift` is the margin
      itself).

    The fabric is counted at the moment of the split. Sheet splits happen where the sheet has
    been stretching, so their fabric comes largely from the *same* rifting episode. #182 opening
    breakups follow pre-breakup stretching too. Without the fabric's age, "reactivation of failed
    rifts/aulacogens" cannot be told apart from "the rift's own precursor". The plan review asked
    for this (#11: time since marking, median age of reactivated fabric); it was not done. Also:
    - Fabric correlates with orogenic belts (stretching behind collisions), and boundaries follow
      belts (51% vs 34%). Part of the 2.3× may be belts.
    - Suggestion: record the time of last marking per sample (or bin the fabric by age at the
      split). Report reactivation for fabric ≥ 20 Myr old. Report it inside belt-free cells
      only, or compare the boundary positions of `new` with `nofabric`.
    - Until then, word it as "new boundaries run through stretched lithosphere 2–3× more often
      than chance", not "failed arms are reactivated".

11. **(should, medium) Option 1's cost is understated.** Largest plate, 500–1000 Myr, maximum
    per seed:
    - `new`: up to **68%** of the surface (2 of 8 seeds), 31–68%;
    - base: 17–40%;
    - `nofabric`: 43–53%;
    - `new-super` seed 2: 50–61% for 300 Myr.

    One plate covering half or more of an Earth-like planet's surface is outside a plausible
    range, unlike base's ~20%. This is the main physical cost of option 1. Name it in the
    decision, together with the already weak supercontinent breakup on main. Main does not
    disperse either: 59–72% minimum on 2 of 4 seeds.

12. **(should, low) Option 2 is untested.** Reverting the removal commit gives "rule + swell +
    magma + plume sections + fabric", which was never run. Step 0 had the rule switched *off*.
    The fabric adds splits (`nofabric` has fewer plates than `new`), so option 2 could end up
    with more plates than main. Say that it needs a short run, or offer to run 4 seeds × 500 Myr.
    Option 3 is fairly framed as a new assumption with its own parameter. Consider adding the
    knob-free search extension (finding 5) as an option 0: "first make sure the search isn't the
    limit".

13. **(nice) Smaller points.**
    - `through_plume` stays true after refinement, even if the refined section leaves the plume.
    - Refinement uses the tension strength for every mode, sliding sections included.
    - The near-plume distance counts all plumes, including ones in their first ~2 Myr. That is
      harmless but worth a sentence.
    - Runtime +0.5%: fine.
    - The fabric as broad patches, not arms, is honestly reported (#64). It means #54's "failed
      arms" are not reproduced as arms, so mention that in the #54 status.

## Before the PR

Must:
- Fix the numbers (1, 2).
- Restate the threshold on rift-type splits (3).
- Report the inversion weighted (9).
- Soften the #54 wording (10).

Should:
- The antipode-null caveat or a rotated null (4).
- The section geometry caveat or the two-arm search (5).
- Plate-size realism in the decision (11).
- The option 2 caveat (12).

Also confirm CI with `CARGO_BUILD_WARNINGS=deny`; not checked here.

## Response (2026-11-27, implementer)

All "must" items adopted in `results.md`: step-0 counts and shares corrected; plume-section
breakups reported per arm (2 in the 8 × 1000 Myr runs); the plume-rift comparison is given per
cause (rift-type splits vs initiations); inversion weighted by shortening (20%); the #54
reactivation claim softened (same-episode stretching, no fabric age, overlap with belts; failed
arms not shown). "Should" items: the search was extended to two-arm sections (90–150°, final
code, 8 + 4 seeds rerun; same outcome); the ~1.3–2× strength drop is labelled an estimate; the
~10 Myr of full magma supply is in the explanation; option 1's cost (largest plate up to 68%) and
the untested option 2 are stated. Not done: a rotated/time-shuffled null (the antipode bias is
discussed instead) and fabric-age tracking.
