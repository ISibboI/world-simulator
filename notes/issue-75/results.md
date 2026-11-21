# Results: orogenic weakness heals (#75); plume heating moves with the plate (#53)

_2026-11-21. Plan: [`plan.md`](plan.md), its review: [`review-plan.md`](review-plan.md).
Goal: an Earth-like planet, not Earth; effects of the ~110 km grid are accepted (#64)._

## What changed

One mechanism for both issues: the lithosphere carries a memory of thermotectonic events on
its crust samples (plate frame), and the memory fades by conductive cooling.

1. **#75.** `Crust::orogenic` is now the *remaining* orogenic weakening: 1 after an orogeny,
   mixed by mass as before, and decaying with `exp(−t / 200 Myr)` (first conductive mode of
   a 250 km lithosphere, `L²/(π²κ)`; Currie & Hyndman 2006: former back-arcs/mobile belts
   become cool and strong over ~300 Myr). Freshly deformed belts are weaker than before:
   strength 0.25 instead of 0.5 (young mobile belts 50–60 km thick vs 150–300 km; Hyndman,
   Currie & Mazzotti 2005); a belt is at 0.72 after 200 Myr, 0.83 after 300 Myr, 0.94 after
   500 Myr.
2. **#53.** A plume resets the thermal age that sets the strength (`Crust::reheated`) of the
   crust above it, every step, just before the plate events: over the head (1000 km radius)
   for the plume's first 20 Myr, then over the tail's swell (500 km), in proportion to the
   plume's flux. The reset moves with the plate and cools on the existing thermal-age law
   (full strength 75 Myr after a 25 Myr-equivalent reset). It changes only the strength, not
   bathymetry or sea-floor age. The cut no longer uses a field in the mantle frame.
3. `weak_zones::lithosphere_strength(crust, time)` is the single function combining thermal
   age, plume reset, thinning and orogenic weakening (interface for #69, see the end).
4. Diagnostics: a log line with raw rift-belt sums (ever marked, weakening, cells ≥ 0.25),
   failed plume rifts, and the orientation of plume rifts relative to plate motion; dump
   columns `strength_age`, `strength`.

## Runs

| set | code | runs |
|---|---|---|
| `main` | main 536cecb + only the new rift log line and counters of this branch ([`scripts/baseline-diagnostics.patch`](scripts/baseline-diagnostics.patch), commit 481b577; no physics change: its logs of seeds 0 and 3 equal a plain-main run apart from the new line and wall time) | 8 seeds × 1000 Myr; seeds 0–3 × 3000 Myr |
| `branch` | this branch, 7a86510 | same |

Default parameters, `--material-budget --cell-dump`, snapshots every 100 (250) Myr. Runs
diverge after the first changed event, so sets are compared as distributions: mean over
seeds (min–max), Welch t. Scripts: [`scripts/`](scripts/) (`weak_area.py` on the cell dumps,
`rifts.py` on the rift log line, `compare.py` from batch 4 for the guard rails).

## #75: the orogenic weak area is bounded

% of continental cells, mean of the seeds (1000 Myr: 8 seeds; later: 4 seeds):

| Myr | 500 | 1000 | 1500 | 2000 | 2500 | 3000 |
|---|---|---|---|---|---|---|
| mean orogenic weakening, main | 27.5 | 47.4 | 57.7 | 61.6 | 64.1 | 66.4 |
| mean orogenic weakening, branch | 14.0 | 16.5 | 13.5 | 12.6 | 14.5 | 19.6 |
| orogenic ≥ 0.25 (strength ≤ 0.81), main | 28.4 | 49.0 | 59.7 | 63.9 | 66.7 | 69.1 |
| orogenic ≥ 0.25, branch | 19.9 | 22.5 | 18.1 | 17.6 | 19.9 | 28.5 |
| ever marked, main | 29.9 | 51.4 | 62.5 | 66.8 | 70.0 | 72.7 |
| ever marked, branch | 28.4 | 47.9 | 57.9 | 63.7 | 69.0 | 75.7 |
| never marked, main | 70.1 | 48.6 | 37.5 | 33.2 | 30.0 | 27.3 |
| never marked, branch | 71.6 | 52.1 | 42.1 | 36.3 | 31.0 | 24.3 |
| last marked > 500 Myr ago, branch | 0 | 15.0 | 29.6 | 37.6 | 37.1 | 38.1 |

(Full tables with seed ranges: `weak_area.py` output, reproduced in
[`tables.md`](tables.md).)

- On main the weakened area keeps growing (66% of the continents after 3 Gyr, still +1 point
  per 250 Myr). On the branch the mean weakening levels off at **12–20%** after ~500 Myr,
  as expected from gross marking rate × τ (plan: 10–15%; the last window is higher in 3 of 4
  seeds, which had large collisions then: the seed range at 3000 Myr is 9–27%).
- The marking itself is unchanged (ever-marked area about the same on both): collisions
  rework most of the continental crust within 3 Gyr. On Earth, too, most continental crust
  outside the Archean cores has been reworked; what changes is that old belts are strong
  again.
- Lithosphere weaker than 0.75 from **all** causes still grows on the branch: 16% of the
  continental cells at the start, 40% at 500 Myr, 45% at 1000–2000 Myr, 54% at 3000 Myr
  (branch only; main's dump has no strength column). Broken down (review): continental
  crust thinner than ~26 km (thinning factor < 0.75) grows from 16% to 31% over 3 Gyr and
  drives it; the orogenic factor < 0.75 stays at 14–24%, thermal < 0.75 at ~5%. The growth
  of thinned continental crust (median thickness 30.8 → 28.9 km) is older than this branch
  and not addressed here.

## Rifts keep following belts over 3 Gyr

Plume rifts; continental cells on the new boundaries vs in the two rifted plates, pooled over
seeds per window. "Weak" = orogenic weakening ≥ 0.25 (strength ≤ 0.81). The share ratio
(share on the boundaries / share in the plates) is capped at 1/p, where p is the weak share
of the plates, so it is not comparable when p differs much (main's p reaches 55–65%, capping
its ratio at ~1.5–1.8). The **odds ratio** has no such cap and is the measure used here
(both from `rifts.py`; > 1: rifts follow belts):

| window | set | rifts | weak cells on boundaries / in plates | share ratio | odds ratio | odds ratio, ever marked | never-marked share ratio |
|---|---|---|---|---|---|---|---|
| 0–1000 (8 seeds) | main | 221 | 49% / 25% | 1.96 | 2.88 | 2.88 | 0.67 |
| | branch | 197 | 28% / 14% | 1.98 | 2.36 | 1.97 | 0.80 |
| 1000–2000 (4) | main | 69 | 83% / 55% | 1.53 | 4.17 | 4.68 | 0.32 |
| | branch | 85 | 30% / 17% | 1.73 | 2.04 | 1.57 | 0.75 |
| 2000–3000 (4) | main | 93 | 87% / 65% | 1.34 | 3.60 | 3.85 | 0.34 |
| | branch | 95 | 32% / 18% | 1.84 | 2.23 | 1.25 | 0.85 |

- **Branch:** rifts select weak belts with an odds ratio of ~2.0–2.4 in every window, with no
  trend over 3 Gyr: they follow the *young* belts (weak cells ~30% of the boundaries,
  ~15–18% of the plates). Healed belts are treated almost like crust never deformed
  (ever-marked odds ratio 2.0 → 1.25).
- **Main:** selectivity is not lower, by the odds ratio even higher (2.9–4.2) — but on main
  83–87% of the rifted continental cells are weak after 1 Gyr: rifts there run almost
  anywhere *except* through the last never-marked crust (never-marked share ratio 0.3).
  The "fading contrast" claimed in #75 is, in the rift statistics, mostly the shrinking
  share of strong crust, not lower selectivity.
- Per seed the branch and main do not differ significantly in any window (Welch t of the
  per-seed share ratio −0.9, −0.2, +0.6); per-seed values range widely (branch share ratio
  0.2–2.4 at 0–1000 Myr; one seed with few belts gives the low end), and the same window
  gives main 2.4 with 4 seeds and 2.0 with 8. A branch-vs-main comparison of rift selectivity
  is also confounded by the stronger fresh weakening (0.25 instead of 0.5).

What the data support: on the branch the orogenic weak area stays bounded at 12–20% and the
belts that are weak keep steering rifts with a stable selectivity over 3 Gyr; on main the
weak area grows to two thirds of the continents, so "following belts" becomes "avoiding the
last strong crust".

## Do cratons need their own strength term? Not for #75

Decision rule of the plan: healing suffices if the weak area is bounded and rifts keep
following belts. Both hold on the branch (bounded at 12–20%; odds ratio ~2 in every window
up to 3 Gyr). **No craton term was added.**

What healing does *not* give: original cores staying distinct from healed belts. On the
branch, rifts through never-marked crust occur at 0.75–0.85 of its share in the plates, the
small remainder of avoidance coming from belts being weaker nearby. Once a belt has healed
(> ~500 Myr), it is as strong as crust that was never deformed. On Earth, Archean and
Paleoproterozoic cratons stay distinct because of their thick, depleted keels (Artemieva &
Mooney 2001), and old belts keep some permanent weakness (mantle fabric, scars: Tommasi &
Vauchez 2001; Heron et al. 2016; the North Atlantic opened ~350 Myr after the Caledonian
closure). Two options if this matters (decision for Sebastian, neither is needed for #75):
- a **craton keel term**: lithosphere never deformed in the simulation (initial continental
  cores) stronger than any healed belt, lost on deformation;
- a **permanent residual** of the orogenic weakening (heal towards `1 − (1−0.25)·f`
  instead of 1).
Both add a free parameter, which is why neither was added in advance.

## #53: plume heating carried by the plate

- Lithosphere still weakened by a plume reset (strength age < 100 Myr and younger than its
  formation): **1.4–2.3% of the planet's surface** at any time (1000 and 3000 Myr runs), of
  the continental cells 1–3%. The trail behind the plumes and the fading after a plume dies
  are as designed; there is no growing weak area.
- Plume rifts (0–1000 Myr, 8 seeds): 27.6 per run on main, 24.6 on the branch (t = −0.9);
  all rifts pass within half a head radius of the plume on both; plume rifts that found no
  division: 54 vs 56. The rift sees the head heating where the plate has carried it (up to
  ~1000 km at 5 cm/yr in 20 Myr), and #52's anchor keeps the break at the plume.
- Orientation of the break near the plume against the plate's motion, mean |cos|: main 0.61,
  branch 0.63 (random: 0.64; along the motion: 1). The heated trail does not draw breaks
  along the plate's motion.

## Guard rails

`main` vs `branch`, 500–1000 Myr, 8 seeds, 69 log metrics
([`compare-500-1000.md`](compare-500-1000.md)): no metric at |t| ≥ 2.5. At |t| ≈ 2: mean
sea-floor age 48.1 → 52.1 Myr (t = +2.0), sea floor ≥ 200 Myr 2.2 → 2.9% (+1.9), endorheic
land 13.6 → 16.3% (+1.9). |t| 1.2–1.6, not significant: plates > 1% 16.5 → 14.6, slab pull
−5%, highest point 5384 → 4969 m, crust > 50 km 7.0 → 4.6%, runoff −10%, sea-level budget
mantle. Plate speeds (4.11 vs 4.10 cm/yr), sea-floor production (6.35 vs 6.12 km²/yr),
continental area, land, temperature, sediment budget unchanged.

With 69 metrics, ~3–4 at |t| ≥ 2 are expected by chance, so these are not evidence of an
effect. If the older sea floor were real, healed belts (fewer weak paths) and the twice
weaker fresh belts (more) would act in opposite directions; the sign flips later (−6 Myr,
t = −1.5, at 2000–3000 Myr). Either way it stays plausible for an Earth-like planet (mean
52 Myr, median 35 Myr).

On the 3000 Myr runs (4 seeds; [`compare-long-1000-2000.md`](compare-long-1000-2000.md),
[`compare-long-2000-3000.md`](compare-long-2000-3000.md)) the signals at |t| ≥ 2 differ in
sign between the windows (1000–2000: more flooded continental crust and shelf on the branch;
2000–3000: more collision stacking, sutures, thick crust, runoff, younger sea floor on the
branch), i.e. the supercontinent cycle of 4 seeds is out of phase, not a trend. The sea-floor
age that was +4 Myr at 500–1000 is −6 Myr at 2000–3000.

Runtime: paired local runs (seeds 0–1, 1000 Myr, no cell dumps) took 861 / 588 s on main
and 878 / 861 s on the branch. Seed 0 differs by 2%; seed 1's 46% is not a cost of the code
(plate count and trench length are the same; local runs share the machine with other jobs at
low priority, so wall times scatter). By construction the added work per 1 Myr step is one
dot product per cell and live plume (~10 plumes × 41k cells) plus one multiplication per crust
sample, well under 1% of a step (~0.6 s). The 8-seed sets on mixed machines: 594 vs 535 s.

## Caveats

- `SUTURE_WEAKENING` changed 0.5 → 0.25 (young-belt value, chosen before the runs). Its
  cross-check: belts of all Phanerozoic ages average 0.74 here, against ~0.5 from
  lithosphere thicknesses (Artemieva & Mooney 2001); the rest is compositional (younger
  lithospheric mantle less depleted), not modelled.
- Plume tails reheat to the same 25 Myr-equivalent as heads (the generous end; Von Herzen et
  al. 1989 argue for less under oceanic swells).
- `reheated` is mixed with the max rule (youngest contributor); it fades within 75 Myr, so
  the over-spreading cannot accumulate.
- Grid: belts are a few 110 km cells wide; a cell's weakening is a mean over the cell.
- 4 seeds for the long runs: a window's ratio changes by up to ~0.5 between 4 and 8 seeds;
  only patterns that hold in every window are claimed.

## Interaction with #69 (deformable margins, branch `agent/issue-69`)

The strength history sits behind `weak_zones::lithosphere_strength(crust, time)` (and
`Simulation::lithosphere_strength(cell)`). #69's band deformation should call it instead of
reading `orogenic`/`birth`. Expected conflicts on rebase: the new `Crust::reheated` field in
struct literals and the dropped `plume_head` argument.

## Review of these results ([`review-results.md`](review-results.md))

Code found correct, numbers reproduced. Worked in: the rift comparison uses odds ratios and
no longer claims that main's selectivity fades while the branch's holds; the craton decision
rests on the bounded weak area and the stable selectivity on the branch, with the
`SUTURE_WEAKENING` confound named (1); paired local runtime added (2); "orogenic weak area"
instead of "weak area", thinning named as the driver of the all-causes weak area (3); the
guard-rail sea-floor age is not interpreted as an effect (4); tests: the step test now kills
the plume and checks that the carried resets remain and fade, the healing test checks the
orogenic mass of a whole plate, and the max rule for `reheated` is tested in `stretch_rift`
and `heal_gap` (5); the baseline is reproducible from
[`scripts/baseline-diagnostics.patch`](scripts/baseline-diagnostics.patch) (6).
