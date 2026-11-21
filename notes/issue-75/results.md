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
| `main` | main 536cecb + only the new rift log line and counters of this branch (local commit 481b577, not pushed; no physics change) | 8 seeds × 1000 Myr; seeds 0–3 × 3000 Myr |
| `branch` | this branch, 7a86510 | same |

Default parameters, `--material-budget --cell-dump`, snapshots every 100 (250) Myr. Runs
diverge after the first changed event, so sets are compared as distributions: mean over
seeds (min–max), Welch t. Scripts: [`scripts/`](scripts/) (`weak_area.py` on the cell dumps,
`rifts.py` on the rift log line, `compare.py` from batch 4 for the guard rails).

## #75: the weak area is bounded

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
- Lithosphere weaker than 0.75 from all causes (thin and young lithosphere included):
  16% of the continental cells at the start (rifted margins), ~40–45% from 500 Myr on,
  44–54% after 2–3 Gyr (branch only; main's dump has no strength column).

## Rifts still follow belts, also after 3 Gyr

Plume rifts; continental cells on the new boundaries vs in the two rifted plates, pooled over
seeds per window. "Weak" = orogenic weakening ≥ 0.25; the ratio is the share on the
boundaries over the share in the plates (> 1: rifts follow belts):

| window | set | rifts | weak cells on boundaries / in plates | ratio | weighted ratio | ever-marked ratio | never-marked ratio |
|---|---|---|---|---|---|---|---|
| 0–1000 (8 seeds) | main | 221 | 49% / 25% | 1.96 | 1.94 | 1.93 | 0.67 |
| | branch | 197 | 28% / 14% | 1.98 | 1.84 | 1.58 | 0.80 |
| 1000–2000 (4) | main | 69 | 83% / 55% | 1.53 | 1.52 | 1.51 | 0.32 |
| | branch | 85 | 30% / 17% | 1.73 | 1.66 | 1.17 | 0.75 |
| 2000–3000 (4) | main | 93 | 87% / 65% | 1.34 | 1.34 | 1.31 | 0.34 |
| | branch | 95 | 32% / 18% | 1.84 | 1.74 | 1.07 | 0.85 |

Per-seed ranges are wide (branch weak-cell ratio 0.2–2.4 at 0–1000, 0.7–2.7 at 2000–3000;
one seed with few belts drives the low end), so single windows are ±0.3.

- **Main:** the belt contrast fades, 2.0 → 1.5 → 1.3, because almost every rifted cell is
  weak (87% of the boundary cells in 2–3 Gyr). Rifts avoid the remaining never-marked crust
  strongly (0.3), but only because it is the last strong crust left.
- **Branch:** the contrast holds at ~1.7–2.0 in every window: rifts follow the *young* belts
  (weak cells are ~30% of the boundaries, ~15–18% of the plates). The ever-marked ratio
  drops to ~1.1, i.e. healed belts are treated like crust that was never deformed.

## Do cratons need their own strength term? Not to keep rifts on belts

Decision rule of the plan: healing suffices if the contrast stays clearly above 1 relative
to main in the same window and the weak area is bounded. Both hold (1.84 vs 1.34 in
2–3 Gyr; 12–20% vs 66%). **No craton term was added.**

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

A slightly older sea floor is the expected direction: old belts heal, so fewer weak
paths for breaks in later continents (plates > 1% −12%, t = −1.3). It stays plausible for an
Earth-like planet (mean 52 Myr, median 35 Myr).

On the 3000 Myr runs (4 seeds; [`compare-long-1000-2000.md`](compare-long-1000-2000.md),
[`compare-long-2000-3000.md`](compare-long-2000-3000.md)) the signals at |t| ≥ 2 differ in
sign between the windows (1000–2000: more flooded continental crust and shelf on the branch;
2000–3000: more collision stacking, sutures, thick crust, runoff, younger sea floor on the
branch), i.e. the supercontinent cycle of 4 seeds is out of phase, not a trend. The sea-floor
age that was +4 Myr at 500–1000 is −6 Myr at 2000–3000.

Runtime: RUNTIME_PLACEHOLDER

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
- 4 seeds for the long runs: windows are ±0.3 in the contrast ratios.

## Interaction with #69 (deformable margins, branch `agent/issue-69`)

The strength history sits behind `weak_zones::lithosphere_strength(crust, time)` (and
`Simulation::lithosphere_strength(cell)`). #69's band deformation should call it instead of
reading `orogenic`/`birth`. Expected conflicts on rebase: the new `Crust::reheated` field in
struct literals and the dropped `plume_head` argument.
