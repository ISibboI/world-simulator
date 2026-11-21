# Review of results and code: #75 / #53

_2026-11-21, independent review of branch `agent/issue-75-53` (src diff vs 536cecb, results.md,
tables.md, compare-*.md, scripts/, run data in `agent-scratch/runs/{diag,heal,diag-long,heal-long}`).
Yardstick: an Earth-like planet, not Earth; ~110 km grid effects accepted._

## Summary

The code does what the plan says and I found no bug. Most of the numbers reproduce exactly
from the data with the scripts. The main conclusion holds: the weak area is bounded on the
branch and grows without limit on main, and the plume heating moves with the plate and fades.
**One conclusion is over-claimed:** "main's belt contrast fades while the branch's holds",
which is the comparison the craton decision rests on. That comes from the metric (a share
ratio that saturates) and from pooling, not from the runs. Per-seed differences are not
significant in any window, and by an odds ratio main's selectivity is higher and does not
fade. This is a wording and metric fix. It needs no new runs, and the no-craton-term decision
can stay, justified on the branch's own stability.

## What I verified

- `weak_area.py` on all four sets reproduces every number in the results' area table and in
  tables.md. The 500/1000 columns come from the 8-seed sets and the later ones from the 4-seed
  sets, as labelled.
- `rifts.py` (pooled lines) reproduces every row of the rift table: rifts 221/197/69/85/93/95,
  weak shares and ratios 1.96/1.98/1.53/1.73/1.34/1.84, weighted, ever-marked and never-marked
  ratios, failed plume rifts 54/56, |cos| 0.61/0.63. Window indexing is right: the log has one
  record per snapshot from t = 0 (11 records for 1000 Myr, 13 for 3000 Myr at 250), and the
  sums are cumulative, so b − a is correct. The `rifts` counter counts plume rifts only (the
  only increment is in `rift_above_plumes`), as the table says.
- `compare.py 500 1000` reproduces `compare-500-1000.md` byte for byte. Plume rifts per run
  are 27.6 vs 24.6, Welch t = −0.93.
- The "main" set really is main's physics. `diag` (481b577) differs from 536cecb only in the
  log line and counters, and the `main` and `diag` logs of seeds 0 and 3 are identical except
  for the new line and the wall time.
- Arithmetic in the text: 0.72/0.83/0.94 after 200/300/500 Myr; the Phanerozoic mean of 0.74;
  "orogenic ≥ 0.25 ⇔ strength ≤ 0.81".

## Must-fix

### 1. The claim "main fades, branch holds" in the rift contrast is an artifact of the metric and of pooling

The ratio "share on boundaries / share in plates" is bounded by 1/p, where p is the weak share
of the plates. On main p reaches 55–65%, so main's ratio *cannot* exceed 1.8 and then 1.54,
whatever the rifts do. Its fall 2.0 → 1.5 → 1.3 is mostly this cap. An odds ratio, which has
no cap, tells the opposite story (pooled, recomputed from the log sums;
`~/tasks/issue-75-53/tmp/odds.py`):

| window | main OR (weak ≥ 0.25) | branch OR | main share ratio, per-seed mean | branch, per-seed mean | Welch t (per-seed ratio, branch − main) |
|---|---|---|---|---|---|
| 0–1000 (8 seeds) | 2.88 | 2.36 | 2.09 | 1.82 | −0.9 |
| 1000–2000 (4) | 4.17 | 2.04 | 1.53 | 1.49 | −0.2 |
| 2000–3000 (4) | 3.60 | 2.23 | 1.33 | 1.58 | +0.6 |

- By odds, main's rifts are *more* selective for weak cells than the branch's in every window,
  and main's selectivity does not fade.
- By the paper's own share ratio, per-seed means show no significant branch–main difference in
  any window (|t| ≤ 0.9). The pooled 1.73/1.84 vs 1.53/1.34 is driven by the seeds with many
  boundary cells. The branch's per-seed values span 0.18–2.38 (0–1000) and 0.69–2.73
  (2000–3000).
- The "±0.3" uncertainty is optimistic. The same 0–1000 window pooled over the 4 long seeds
  gives main 2.41 against 1.96 over 8 seeds (Δ 0.45).

What the data do support, and what the results should say instead:
- On the branch, rifts prefer young belts with a **stable** selectivity over 3 Gyr: OR ≈ 2.0–2.4,
  pooled share ratio 1.7–2.0, > 1 in 3 of 4 seeds per window, while the weak area stays
  bounded.
- On main, "rifts follow belts" loses its meaning rather than its strength: 83–87% of the
  boundary cells are "weak" because most of the continent is.

The decision "no craton term" can stand on the first point, which is the plan's purpose:
the contrast does not fade and the weak area is bounded. Drop the comparison "1.84 vs 1.34 in
2–3 Gyr" as the evidence, or present it together with the odds ratio and the per-seed t.
Also say that the branch's young-belt contrast is partly set by `SUTURE_WEAKENING` 0.5 → 0.25
(fresh belts twice as weak). That change confounds any branch-vs-main comparison of rift
selectivity.

### 2. `Runtime: RUNTIME_PLACEHOLDER` in results.md

Fill it in or remove it. `time-diag/` and `time-heal/` exist but contain no `seconds` yet. The
only wall times available come from mixed GitHub and local machines, so they are not a paired
comparison. Sum of the 3000 Myr runs on GitHub: diag 2084–2195 s, heal 1386–1656 s; machine
noise dominates.

## Should-fix

### 3. The all-causes weak area keeps growing on the branch, from thinning; say so

Results quote "strength < 0.75: ~40–45% from 500 Myr on, 44–54% after 2–3 Gyr" without saying
where it comes from. On the long runs it rises steadily: 39 → 43 → 45 → 50 → 54% at
500/1500/2000/2500/3000 Myr. A rough breakdown of the continental cells with strength < 0.75
(`~/tasks/issue-75-53/tmp/decomp.py`; the thinning factor is inferred as
strength / (thermal · orogenic factor)) gives:

| Myr | weak < 0.75 | thermal factor < 0.75 | orogenic factor < 0.75 | thinning < 0.75 |
|---|---|---|---|---|
| 500 | 39 | 5 | 18 | 22 |
| 3000 | 54 | 6 | 24 | 31 |

Thinned continental crust (basement < ~26 km) grows from 16% at t = 0 to 31%. That driver
predates this branch and has no healing. It does not undermine #75 (the *orogenic* weak area
is bounded), but "the weak area is bounded" should read "the orogenic weak area". One sentence
naming the thinning growth as a separate, existing driver would let Sebastian decide whether
it needs an issue. Main's dump has no strength column, so it cannot be compared directly.

### 4. Guard-rail interpretation of the older sea floor at 500–1000 Myr is post hoc

- With 69 metrics and about 14 degrees of freedom, ~4 metrics at |t| ≥ 2 are expected by
  chance. Three metrics sit at t = 1.9–2.0. That is consistent with noise, as the text itself
  shows a few lines later: the sea-floor age is −6 Myr at 2000–3000 (t = −1.5).
- The mechanism offered ("old belts heal, so fewer weak paths") ignores the opposite effect of
  the same change, young belts twice as weak (0.25).
- Plume rifts are 24.6 vs 27.6 per run (t = −0.9); breakups/"plates > 1%" are −12% (t = −1.3).
  Neither is significant.

Suggest: "no metric at |t| ≥ 2.5; the three at |t| ≈ 2 are within what 69 comparisons give by
chance, and the sea-floor age changes sign later; if real, the direction would fit stronger old
belts". The plausibility statement (mean 52 Myr, median 35 Myr) is fine.

### 5. Two tests do not test what their names say

- `steps_carry_plume_heating_with_the_plate` only checks that the sample above the plume and
  ≥ 10 samples have `reheated` set. A mantle-frame field rewritten every step would pass it
  too: icosphere(4) cells are ~480 km apart, and the head radius (1000 km) covers well over 10
  cells by itself. The test should pick a sample heated in step 1 and assert, after a few
  steps, that it still carries the reset at a position now *outside* the heating radius, or
  that a cell outside the radius is weakened. Use a fast plate or more steps if needed.
- In `orogenic_weakening_heals_and_commutes_with_mixing`, the commutation part is scalar
  arithmetic on local variables: it is true by construction and touches no code. Either drop
  it or exercise a real mixing path (`Crust::mix_orogeny`, or `heal_gap`/`stretch_rift` on two
  samples, then `heal_orogenic_weakening`, against the reverse order).
- Missing: a test that `heal_gap`/`stretch_rift` take the *latest* `reheated` of the donors.
  Small, and it pins the mixing rule.

### 6. The baseline commit is local only

481b577 ("Diagnostics only", the `main` set) is not pushed, so the comparison cannot be
reproduced from GitHub. Either commit its 3-file diff as a patch under
`notes/issue-75/scripts/` or state that the branch's own diagnostics commit reproduces it on
top of 536cecb.

## Optional

7. `rifts.py`: print the per-seed Welch t (main vs branch) next to the pooled line, and an odds
   ratio. Assert that the record count matches T/INTERVAL + 1, so a wrong `INTERVAL` fails
   loudly instead of silently indexing the wrong snapshot. The `TIME` regex and the unused `n`
   can go.
8. `weak_area.py` "plume-heated" counts any cell with strength age < 100 Myr, including tail-edge
   resets that weaken by < 1% (strength age 98 → 0.99). A threshold such as strength age
   < 81 Myr (thermal factor < 0.9) would better match "still weakened". The 1.4–2.3% would
   shrink, which strengthens the conclusion.
9. Alignment statistic (`rift_motion_alignment`): only boundary cells on plate b's side are
   used, and the cells next to the plume are dominated by the half-cell offset. The random
   baseline 2/π still holds, but sensitivity is reduced. Fine as a sanity check; say "no
   detectable alignment" rather than implying a sharp test.
10. Doc of `SUTURE_WEAKENING` says the central Atlantic opened "~100 Myr after" the Alleghanian
    closure, while plan item 2 says "~200 Myr after closure". The dates given (~300 Ma, rifting
    230–200 Ma) mean 70–100 Myr. Make the two consistent.
11. The results' area table mixes the 8-seed (500, 1000) and 4-seed (later) sets in one row.
    That is labelled, but main's row gives 47.4 (8 seeds) at 1000 and the long set 47.6, and
    the branch's 16.5 vs 15.2. A footnote with the 4-seed value at 1000 Myr would make the
    series continuous.

## Code review: correctness (no bugs found)

- **Healing:** `heal_orogenic_weakening(dt)` runs once per step with the step's dt, on every
  `Some` sample of every plate. That includes hidden samples, and there is no other crust store
  (the topology/boundary structs are transient). The factor is precomputed. Exponential
  compounding is exact for variable dt.
- **Order:** heal and reheat run after the substeps, `crustal_flow` and the slab update, and
  immediately before `plate_events`. Within it, `rift_above_plumes` runs after sutures and
  captures. `self.time` is not yet advanced and the plumes are not yet aged (the mantle steps
  later), so the plume age and time match the old evaluation of `plume_head_strength`. The
  first rift-eligible step (age ≥ 20 = `PLUME_HEAD_STAGE`) heats only the tail radius, while
  the head heating from the preceding steps (≤ 1 step old) is kept. That is as designed.
- **Profile:** `plume_heating_age` gives exactly the old cap at the centre (25 Myr ⇒ 0.5), the
  `1 − x²` taper and None outside the radius. The intensity is `flux/peak_flux`, which gives
  head growth and the end-of-life fade; the guard `peak_flux <= 0` is there. `reheat` applies
  the max rule, so no cooling.
- **Mixing:** `stretch_rift` and `heal_gap` take the max over donors and keep `reheated` out of
  the stretching heat budget. `add_crystalline` keeps the receiver's value (documented). Struct
  update syntax elsewhere copies the field. `assert_same_state` includes it.
- **Strength:** `lithosphere_strength` uses `strength_age`; the `.max(0.0)` handles any
  `reheated > time` (cannot happen). Initiation age contrasts still use `birth`, as intended.
- **Statistics:** `record_split_geometry` adds the sums and weak counts over the same
  continental cells as the existing counts. `failed_plume_rifts` counts only plumes that
  passed the continent/large-plate filter (correct reading of "tried"). The dump column
  `strength_age.min(1e4)` cannot create false "plume-heated" cells, because the metric also
  requires < 100.

## Tuning check

No sign of tuning to fake an effect:
- τ = 200 Myr follows from L²/(π²κ) for 250 km and is checked against Currie & Hyndman 2006.
- `SUTURE_WEAKENING` 0.25, the tail radius and the head stage were fixed in the plan review
  before any run, with physical sources.
- The decision rule was set before the runs.

The only caution is finding 1: the rule's "relative to main" form, applied with the share
ratio, favours the branch by construction. That is a metric problem, not tuning.
