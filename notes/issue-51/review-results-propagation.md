# Review: results of option 3 (propagating plume rifts, #51)

_2026-11-27, independent evaluation of [`results-propagation.md`](results-propagation.md) (draft),
code c320fea vs e844fdf, runs in `~/tasks/issue-51/runs/p3-*`, `pre-on`. Earth-like planet, not
Earth; ~110 km grid effects accepted (#64)._

## Verdict

The numbers are almost all right and the main conclusion (option 3 gives very few plume rifts
because whole-section `S_rift ≥ 1` is rare) holds. But there is one **model inconsistency that
biases the count low** (the rift's remaining distance is re-measured on whichever section is best
now, while the timer keeps the time already propagated: a moving target), the "speed is not the
limit" wording overstates, the `magma_weakening = 0.05` run is promised but missing, and a few
figures need fixing. None of this changes the decision qualitatively: even an upper bound stays
below the 1 per 100 Myr threshold.

## 1. Numbers (spot-checked)

Checked against `compare.py` output and the logs: the 500–1000 Myr table (all rows, all arms),
0–500 plates/largest plate/births, fabric numbers, rift counts per arm (now 2: seeds 0, 1;
v15 2; v140 8 = 3 heads + 5 tails; reach2000 2), "now = before on 6 of 8 seeds" (final snapshot
hashes identical on seeds 2–7) and "super-now = super-before on all 4" (identical), largest plate
at 500 Myr in super runs (main 23/17/18/14 %, now 52/27/56/22 %), pre-check longest rift (plate
315, seed 1: timer 38 Myr, needed 46 Myr for 2316 km). All correct. Fix:

* **0–500 Myr line: "plume rifts 0 → 0 → 0.03"** is wrong for main: main has 2.40 per 100 Myr
  there (by its rule). Write "2.4 (rule) → 0 → 0.03".
* **Supercontinent, main:** not "2 of 4 seeds down to 70–71 %". Largest continental mass in main:
  seed 1 at 70–71 % for 140–200 Myr, then 58 % at 210 Myr; seed 2 80→68 % at 210–240 Myr; seed 0
  a single 71 % snapshot at 150 Myr; all re-assemble to 95–99 % by 500 Myr. now/before: never
  below 91 %.
* **v = 140 "0.3 per 100 Myr"** is the 500–1000 Myr window; over the whole run it is 0.2, and 6 of
  the 8 rifts are on seed 1. Say so; with 2–8 events the arms differ by little more than noise.
* "~2–3× gain (unit test 2.3×)": the plate logs (pre-on, 1339 rift analyses) give
  `S_rift / S_plume-section` median **1.8** (p10 1.1, p90 2.2, max 3.6), consistent with an opening
  share of ~0.5 (median `opening` 0.46/0.55): `1/(0.5 + 0.05) ≈ 1.8`. Say "~2× (1.1–2.2)".
  The cited unit test `magma_weakens_sections_only_in_opening` gives 10× for pure tension; say
  which test gives 2.3×.
* "Offline replay: 1 more rift in 800 Myr" for stalled rifts: I could not reproduce it. From the
  plate logs no plate with a stalled timer gets a second `S_rift ≥ 1` episode, i.e. +0. Either
  way the claim (stalling does not help) holds; state how the replay was done.
* **p3-magma005** (`magma_weakening=0.05`) is still running (seeds at 720–820 Myr). Partial
  500–~770 Myr window: 3 plume rifts in total so far (0.08 per 100 Myr in the window), `S_rift ≥ 1`
  in 1.5 % of analyses (now: 4.5 %), plates 17.5, largest plate 34.8 %. The draft's Decision 2
  refers to it ("sensitivity run below") but has no numbers; add them when it ends. The lower
  `S_rift ≥ 1` share is surprising: `magma_weakening` also weakens the ordinary sections above
  plumes (`strength.tension = dyked(s, magma_weakening_at(r))`), so plumes break plates earlier by
  the ordinary mechanism; check before interpreting.

## 2. Are the conclusions supported?

**Possible bug / modelling flaw (most important): the reach is a moving target.** `rift_timer`
accumulates the time a rift has propagated regardless of which section is best, but the crossing
time is `rift.reach / propagation` with `reach` re-measured every analysis on the *current* best
section from the plume's *current* position (`rift_reach`, breakup.rs ~1188, used at ~671). As the
plate drifts over the fixed plume (at speeds comparable to 5 cm/yr), grows, and the search picks a
different wedge/refinement, the far end moves away. Plate logs:

| episode (pre-on) | `S_rift ≥ 1` | reach at onset → end | needed → timer reached | frozen reach? |
|---|---|---|---|---|
| seed 1, plate 315 | 253–289 Myr | 1320 → 2316 km | 26 → 46 Myr; 38 | splits at ~279 |
| seed 0, plate 2 | 73–87 Myr | 783 → 909 km | 15.7 → 18.2 Myr; 16 | splits at 87 |
| seed 0, plate 6 | 15–21 Myr | 437 → 324 km | split (the one rift) | same |

With the reach fixed at onset (or the rift tied to its own path in plate coordinates), 2 of the 3
near-misses become rifts: 3 instead of 1 in 800 seed-Myr. The timer and the reach should refer to
the same rift: store the rift's path (side/endpoints in plate-local frame) or the propagated
length, and measure the remaining distance along that path. This is a correctness fix, not tuning.

**"The limit is S_rift, not the speed": mostly right, overstated as worded.** From v = 50 to 140
km/Myr the count rises ~4–10× (2 → 8 events), so speed matters relatively. The absolute cap is the
frequency of long-enough `S_rift ≥ 1` episodes: in pre-on, 7 episodes in 800 seed-Myr, only 4
lasting ≥ 5 Myr (8, 16, 6, 38 Myr), one of which (plate 509) the ordinary breakup took first. So
even infinite speed (#182's 5 Myr minimum) gives ≤ ~0.5 per 100 Myr, below the threshold of 1.
Recommend stating it like that: "even at infinite speed ≤ 0.5 per 100 Myr".

**"Why so few": supported, with one addition.** S_rift is far from 1 typically (median 0.15/0.21,
p90 0.47/0.57). And in 5 of the 7 episodes the plate's *ordinary* overload was also near or above 1
(max 1.16–1.36; 23–664 on plate 438): the magma-fed rift is only viable on plates that are already
almost breaking anyway. The real limit is the plates' loads (option 1's ~20 plates and 35 %
largest plate), not the magma factor; that is what #182's strength or the driving forces set.

**Stalled rifts would not help:** supported (no plate with a stalled timer reaches `S_rift ≥ 1`
again in the logs). But see the reach issue: what helps is crediting the propagated length.

**Other code checks (no bug found):** plume identity via `cos(magma radius)` is fine (plumes are
fixed; tail radius ~500 km); split handover keeps the timer in the part above the plume (plate 315
kept its timer through a split at 261 Myr); `break_overloaded_plates` resets both parts' timers
after a stress breakup, so "the timer follows the rift through splits" holds only for non-breakup
splits — say so. Minor underestimate: `plume_sections` runs the full limit analysis only for the
best quick estimate per plume (the level search refines `REFINED` candidates); the rift
refinement is accepted only if it raises `S_rift` (fine). Timer decay at the same rate and the
`max(params.time)` floor behave as described.

## 3. Decision list and recommendation

The list is fair but incomplete. Add:

* **0. Fix the moving-target reach first** (cheap, physically required: a rift keeps its path).
  Expect ~2–3× more plume rifts (≈ 0.1 per 100 Myr), still far below the threshold, so it does not
  change the decision; it removes a reason to doubt the negative result.
* State **Decision 2's evidence** (magma005, partial: no gain) — currently it looks untested.
* Make explicit that **the real deficit (plates 20 vs 30, largest plate 35 % vs 17 %) is a load/
  strength question** that option 3 cannot fix by design; only option 4 (#182 strength) or larger
  driving loads address it, and with them option 3 would also rift more. Option 3 (rule back) would
  restore main's statistics but by a rule that ignores the loads, which #51/#195 set out to remove.

Recommendation, in order:
1. Correct the figures above, add the magma005 result when it finishes.
2. Fix the reach (tie timer and remaining distance to one rift path); rerun 8 × 1000 Myr `now`.
3. Recommend to Sebastian: keep option 3 (physical, harmless: identical to `before` on 6/8 seeds,
   +1 % runtime) as the plume-rift mechanism; do not reinstate the rule; open a separate issue for
   the plate count/largest plate (#182 strength or loads), which is the actual lever, and judge
   plume rifts again after it.
