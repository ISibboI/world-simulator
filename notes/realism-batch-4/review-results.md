# Review of the batch 4 results (#40, #52)

_2026-11-21, independent review of `results.md`, the code diff `74e80e3...HEAD -- src` and the run
data in `agent-scratch/runs/` (sets `main`, `v1`, `L/v2`, `L/m68`, `L/m68v1`, `L/m68v2`, 8 seeds
each). Not built, not run; scripts re-run and own checks written in Python. Goal: an Earth-like
planet, not Earth; grid effects (~110 km) accepted but named._

**Verdict: approve with changes.** The code changes are sound. I found no case in which the
#52 anchor produces a wrong split; every failure mode I found ends in "no rift". Almost every
number in `results.md` reproduces. The changes needed are in the text. Three explanations are
contradicted by the data or are weaker than they could be (findings 1, 2, 4). One caveat is too
weak and should become an issue (finding 6). The remaining 4% of far rifts are still accepted
(finding 3). Finding 12 names two tests to add.

## Numbers re-derived

| claim in `results.md` | re-derived | |
|---|---|---|
| `m68` marked 11.9 / 23.9 / 32.9 / 40.8 / 47.2 | same | ✓ |
| `m68v2` any / mean / ≥ 0.5 / ≥ 0.9 at 200–1000 Myr | same (all 20 values) | ✓ |
| old base: `main` 47.5; `v1` 48.5 / 42.4 / 42.8 / 37.3 | same | ✓ |
| 92% (87%) of the orogenic total in cells ≥ 0.9; 1.7% (2.7%) in cells < 0.5 | 92.4 (87.5); 1.7 (2.7) | ✓ |
| over-spread 3.7 of 44 (8%); old base 6 of 42 (14%) | 47.8 − 44.1 = 3.7; 48.5 − 42.4 = 6.1 | ✓ |
| marked area grows ~5–6% per 100 Myr in every set | 5.8–6.1 per 100 Myr up to 400 Myr, then 3–4 (`m68` 28.8 → 47.2 over 500–1000 Myr = 3.7) | ✗ (finding 5) |
| rift/suture table, `v2`: 52% / 26% / 2.0 | 52.1% / **26.75%** / 1.95 | small mismatch |
| other rows of that table, range 25–69% | same | ✓ |
| plume rifts 363 / 296 / 235 / 188; far 28 / 4 / 29 / 4%; median 111 / 0 / 111 / 0 km | same | ✓ |
| causes 97 / 0 / 3% (`v1`), 96 / 0 / 4% (`m68v1`) | same | ✓ |
| median 41 cells moved | 41 (`v1`), 24 (`m68v1`) | ✓ |
| remaining far rifts: plume ≤ 150 km from the edge, two at 150–500 km | `v2` 12/12 ≤ 150 km; `m68v2` 5 ≤ 150, 2 at 150–500 | ✓ |
| −18 to −20% debug-line rifts; log 2.66 → 2.35, t = −1.5, range 1.6–3.3 | 363 → 296 (−18%), 235 → 188 (−20%); 2.66 → 2.35, t = −1.5 **in the 0–1000 Myr window** (in 500–1000: 2.70 → 2.50, t = −0.7) | ✓, window unstated |
| guard rails `m68` vs `m68v2` 500–1000 | `compare.py` output identical to `compare-main-branch.md`; no |t| ≥ 2 except runtime | ✓ |
| old base: four metrics at |t| 2.0–2.9 | five at |t| ≥ 1.98: trenches +1.98, **sea-floor production (kinematic) +1.99**, slow boundaries −2.24, median age −2.68, sea floor < 20 Myr +2.95 (ridges +1.94) | ✗ (finding 10) |
| runtime local `main` 742 s vs `v1` 725 s | same (t = −0.4) | ✓ |

The scripts are correct for what they claim, with a few caveats:
- `rifts.py` pairs each "Plume rifted a plate" with the last "Rift break stages" before it.
  That pairing is right: the last stage line belongs to the accepted division, and rejected
  attempts are overwritten.
- "Distance" is the angle from the plume's cell to the nearest boundary cell. For the final
  value it is one-sided: cells of the new plate next to the old one (`record_split_geometry`).
  For `cut_distance` and `rigid_distance` it is two-sided. The difference is at most one cell,
  and I checked that the stage line's two-sided final distance gives the same > 500 km counts
  (101 / 12 / 69 / 7).
- 500 km is 0.5 × `PLUME_HEAD_RADIUS`, the same threshold as the log counter
  `head_rifts`. 250 km is about 2.3 cells.
- The cause classes are logically exhaustive. Given cut ≤ 250 and rigid ≤ 250, a final
  distance > 500 can come only from the flipping step, and the "other" class is empty.
- `marks.py` restricts to `cls == 0` (continental-origin columns). "Share of the orogenic
  total" is a per-cell (area) sum of the share, not a mass. "Orogenic lithosphere" in the
  text means this area sum.

Independent check of the issue's 30% on the sets without debug lines: from the log counter
`head_rifts`, the far share is `main` 35% (116/336) and `m68` 28% (59/213). With the fix it is
4% (`v2` 12/296, `m68v2` 7/188).

## Findings

1. **(moderate) The described cause of #52 is partly contradicted by the data.** The
   write-up says that a small piece at the plume and a larger block elsewhere are "typical for
   plumes near a plate edge or under a continent held together as rigid crust". The far
   rifts (`v1` / `m68v1`) are, however, *less* often under rigid crust than the near ones:
   40% vs 59% and 26% vs 55%. Edge proximity raises the risk: 69% / 78% of the rifts with the
   plume ≤ 150 km from the edge ended far, against 18% / 13% of those > 500 km from the edge.
   But only 38% / 58% of the far rifts had the plume within 300 km of the edge, and 45% / 28%
   had it more than 500 km inside the plate. *Action:* drop "under a continent held together
   as rigid crust" and say that a nearby edge raises the risk but most far rifts are not at an
   edge.

2. **(moderate) The fall in rift count can be counted. It need not be inferred.** The logs
   contain "Split leaves a part too small … anchor=Rift { nucleus … }", and a plume never
   retries: `mark_rifted` runs also after a failure. Per set, counting successes and rifts
   that failed after all re-cuts:

   | | successful | failed (part too small) | successful only after re-cuts | visible attempts |
   |---|---|---|---|---|
   | `v1` | 363 | 35 | 41 | 398 |
   | `v2` | 296 | 93 | 97 | 389 |
   | `m68v1` | 235 | 22 | 26 | 257 |
   | `m68v2` | 188 | 55 | 49 | 243 |

   The new too-small failures, +58 and +33, explain about 85% and 70% of the drops of 67 and
   47 rifts. Most of these failures (60 of 93, 36 of 55) leave the new side too small.
   The remainder is a 2–5% drop in visible attempts. That drop is either chance or splits
   for which `local_split_piece` returned `None` without a log line (e.g. 8 flip iterations
   without convergence, see finding 13). The cause given in `results.md` is right. *Action:*
   replace "inferred, not counted" with these numbers. State the window of 2.66 → 2.35
   (0–1000 Myr). Also note that #40 alone *raised* the rift count, by +8–10% (`m68` → `m68v1`
   2.66 → 2.94, t = +1.3; `main` → `v1` 4.20 → 4.54, t = +1.4), so the net change against
   main is −12%. Optionally add a debug line for `None` returns.

3. **(moderate) The remaining 4% are still accepted far rifts.** All 19 remaining far rifts
   are in the class where the cut itself missed the plume. In 9 of the 12 in `v2` the break
   lies more than 1000 km from the plume. `results.md` argues that "a plume at a plate
   boundary does not split its plate there on a real planet either". That argues for
   *rejecting* such splits (no rift when the final break is more than half a head radius
   from the plume), not for keeping a break 1000 km away. *Action:* reject them (a few lines;
   the plume is then marked rifted as after any failure), or say plainly that 4% of plume
   rifts still break the plate far from the plume and why that is accepted. Also fix the
   wording: "all … ≤ 150 km; two cases at 150–500 km" contradicts itself; "17 of 19 within
   150 km, 2 at 150–500 km" does not. The explanation that one spine has no cells inside the
   plate is plausible from the code but was not measured; mark it as inferred.

4. **(moderate) The #40 conclusion is right, but its evidence is the weaker one, and "direct
   marking" needs a note.**
   - (a) "92% of the orogenic total sits in cells ≥ 0.9, i.e. cells marked directly" is an
     identification, not a measurement. A heal or stretch from all-orogenic donors also gives
     1.0. At 1000 Myr, 38.0% of continental cells have exactly 1.000 and 2.9% have 0.9–1.
     The stronger argument is conservation. Share mixing conserves Σ share × mass in each
     mixing, so the mean-share curve (44.1%) can grow only by direct marks. Additions on top
     and differences in crust thickness add second-order terms. The max rule's surplus is
     then the 3.7-point halo. That comparison is *within one run* (`m68v2` keeps `orogeny`
     with the max rule: 47.9% marked vs 47.8% with any share), so it is free of the
     run-to-run chaos. The text should say so; the cross-run `m68` vs `m68v2` row is only a
     consistency check.
   - (b) The other reading: "direct" marking is itself broad. A collision addition marks
     every reached cell of the winner within the 4-ring `collision_profile` (~0–330 km) whose
     thickness exceeds 35 km after the addition. That covers almost any normal continental
     crust, and it repeats every substep while the collision lasts. The growth of the
     orogenic area therefore scales with collision length × duration (collisions 20–22 ×
     10³ km in these runs). That is a real orogen width (200–1000 km on Earth). But the
     plausibility statement depends on the collision length, which is a known open problem.
     I found no path by which the share or the max rule feeds back into direct marking, and
     the marked area is the same with both rules (47.2 vs 47.8), so there is no sign of a
     feedback through rift positions.
   - (c) Snapshot artefacts: the dump shows the surface sample per cell, and shares below
     0.0005 print as 0.000 (0.13% of continental cells carry an `orogeny` time but share
     0.000). Both are negligible.

   *Action:* use the conservation argument and the within-run comparison; name the
   collision-profile marking as the driver of the growth.

5. **(minor) The growth rate is misquoted.** "~5–6% per 100 Myr in every set" holds only up
   to 400 Myr. After 500 Myr the rate is 3–4 per 100 Myr (`m68v2` 900 → 1000 Myr: +2.7). The
   plan quoted ~4. *Action:* fix the number.

6. **(moderate) The saturation caveat is too weak; open an issue.** At 1000 Myr, 47.8% of the
   continental cells carry some share and 38% are fully orogenic. The marked area still
   grows ~3 points per 100 Myr, and nothing removes the mark. In 2–3 Gyr runs the "belt vs
   craton" contrast disappears, which is exactly what #40 was meant to protect. That
   contrast disappears through direct marking under "weak for good", not through the mixing
   rule. The Phanerozoic comparison (~29% in 500 Myr vs "roughly a third" on Earth, not
   re-derived) is fair as an order of magnitude, and the text is honest that the source was
   not checked. On Earth, however, cratons keep their strength through thick, depleted,
   cold keels, not because they were never deformed. *Action:* state the numbers above in
   `results.md`, and propose a new issue to Sebastian: the orogenic weakness never heals,
   so the weak area rises without limit; options are healing with thermal age, or a
   strength term for craton keels. Low to medium priority, his decision.

7. **(minor) "Mass-weighted share and majority rule give the same area (44.1 vs 44.3%)".**
   The ≥ 0.5 count in a share run is not what the majority rule would give. The majority rule
   rounds at every mixing, and the rounding compounds. *Action:* say "cells with a majority
   share" instead of "majority rule", or drop the claim.

8. **(minor) The median distance 111 → 0 km is a grid quantisation.** Among the near rifts the
   split between 0 and 1 cell is the same before and after (52–54% at 0 km in all four sets).
   The median flips only because the far tail is gone. *Action:* report "break within one
   cell of the plume: 71% → 95% (old base), 70% → 94% (new base)" instead.

9. **(minor) Rift/suture table, `v2` row:** 52% / 27% / 1.95 (26.75% in the mean over seeds),
   not 26% / 2.0. The conclusion is unchanged. The table still counts the max-rule mark
   (`orogeny > NO_OROGENY`). In the branch this matches the any-share area, not the share
   that weakens. Say so in one clause, since the plan announced a share-based count.

10. **(minor) Guard rails: no shift is hidden, but two items are misdescribed.**
    - New base: the claim "no |t| ≥ 2 except runtime" is correct. A coherent set of
      hypsometry shifts at |t| 1.2–1.7 goes unnamed: flooded continental crust 27.7 → 23.4%
      (t = −1.4), shelf area 5.57 → 4.77% (−1.2), land +1.6. Also unnamed is the largest
      non-runtime shift, sea floor ≥ 200 Myr 3.21 → 2.52% (t = −1.7). Name them as
      not significant.
    - Old base: there are five metrics at |t| ≈ 2, not four. Add sea-floor production
      +1.99; ridges are at +1.94. Together they form *one* correlated signal: more
      boundaries and spreading, younger sea floor, fewer slow boundaries. They are not four
      independent chance hits. That signal is partly present with #40 alone (`main` → `v1`:
      production +1.8, median age −1.6). It does not repeat on the new base (production
      t = 0.0, median age with the opposite sign). Whether it shows in `m68v1` vs `m68v2` is
      also mixed. "Read as chance" is therefore defensible. Better: "a coherent signal on
      the old base that does not reproduce on the new base; any real effect is below ~5%".
    - Isolating #52 on the new base needs `m68v1` vs `m68v2`, not `m68` vs `m68v2`; both
      show the same picture for the metrics above.

11. **(minor) Runtime.** On the runners: `m68` 576 s, `m68v1` 718 s (t = +3.3), `m68v2` 671 s
    (t = +2.2). The table says all three ran on GitHub runners, so "different machines"
    means runner heterogeneity (`m68` ranges 442–696 s). That is plausible, but it should be
    said that way. The only clean timing is the local old-base pair, which has the
    diagnostics but not the #52 fix. The added code (one O(cells) nearest search per split
    attempt, two `break_distance` passes) is negligible next to a 600 s run.

12. **(moderate) Tests.**
    - #40 is well covered: mixing and the ocean reset, conservation in `stretch_rift`,
      `heal_gap` and `spread_crust`, proportional strength, belt vs diluted cut cost, and
      the determinism check now including `orogenic`. Missing is the test the plan review
      asked for (point 7): a collision addition resets a diluted sample to share 1, the
      path that drives the growth (finding 4b).
    - #52: `plume_rifts_near_a_plate_edge_stay_at_the_plume` accepts a distance < 0.13 rad
      (≈ 830 km, 7.5 cells), weaker than the 500 km criterion of the issue. It needs only
      one of 18 set-ups to rift, and nothing shows that it fails on the old code.
    - *Action:* check that the test fails with the anchor hunk reverted. If it does not,
      tighten it to ≤ 0.08 rad (500 km) or choose a set-up that reproduces a flip. Add a
      test that a plume whose piece would be too small gives no rift instead of a far rift.

13. **(low, no action needed) Edge cases of the anchor.** Checked against the code:
    - The nucleus is always in the plate and anchors its own side at distance 0. The spines
      tie long lines of cells (≤ 0.4 rad) to both anchor pieces, so the anchor pieces are
      small only when a spine is short, i.e. at an edge.
    - A side without cells gives `anchors = None`. The code then falls back to the largest
      piece, and the area test fails anyway.
    - An anchor in a fragment already detached from the plate makes that fragment the main
      piece. The real block of that side then flips, and the split fails the area test.
    - With small anchor pieces a large block may flip back and forth. After 8 iterations
      `local_split_piece` returns `None` without a log line.
    - Flipping moves whole pieces, so rigid components stay whole and the caller's "new
      plate" is still the `true` side. That was already true before the fix.
    - All of these end in no rift, never in a wrong one.

14. **(process, minor)** Commit 1e22f88 carries `Co-Authored-By: Claude Fable 5.2`; the
    project rule is Opus 5.5. Fix only if the history is rewritten anyway.

## What needs action before the PR

Text in `results.md`: findings 1, 2, 3 (or code), 4, 5, 6, 8, 9, 10. Tests: finding 12. To
propose to Sebastian: a new issue for the healing of the orogenic weakness (finding 6).
