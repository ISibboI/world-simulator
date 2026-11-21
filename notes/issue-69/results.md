# Results: deformable plate margins (#69)

_2026-11-21. Plan: [`plan.md`](plan.md) (with its revisions after the [plan review](review-plan.md)).
Results review: [`review-results.md`](review-results.md) (of the first version of this file; its
findings are worked in, see the end). Goal: an Earth-like planet, not Earth; PB2002 and
hypsometry numbers are plausibility checks. Plates stay rigid (#70)._

## Current state (after #82 merged and `collision_resistance` recalibrated)

Requested on PR #81: recalibrate `collision_resistance`. #82 (healing orogenic weakness, plume
reheating) merged meanwhile; the branch is rebased on it, the band reads strength through
`lithosphere_strength(crust, time)`, and the band's strength floor is 0.5 (with #82 fresh belts
have strength 0.25; with the old floor of 0.3 the band localised in its own fresh marks and made
the belt narrower than no band, see `MIN_BAND_STRENGTH`). Calibration plan, sweep and choice:
[`calibration.md`](calibration.md). **`collision_resistance` 50 → 0**: the old value stood in for
the potential energy of thick belts, which the belt resistance now models.

16 seeds × 1000 Myr each, 500–1000 Myr means, `main82` = main at 1a4c25b (with #82):

| | main82 | PR at 50 (8 seeds) | **PR, recalibrated (0)** | t (vs main82) | Earth / target |
|---|---|---|---|---|---|
| mean land (m) | 876 | 685 | **801** | −1.1 | ~840 |
| crust > 50 km (% of continental) | 5.6 | 2.4 | **4.5** | −1.1 | ~3–6 |
| boundary slower than 2 cm/yr (%) | 11.3 | 16.7 | **15.6** | +3.6 | 39 |
| continental boundaries (%) | 13.0 | 16.5 | **15.9** | +2.5 | ~30 |
| collision stacking (Gt/yr) | 20.5 | 19.1 | **21.9** | +0.4 | 13–23 |
| flooded continental crust (%) | 24.9 | 31.3 | **24.9** | 0.0 | |

The sections below describe the measurement on the old base (main 536cecb, `collision_resistance`
50); their mechanisms and attributions still hold, their absolute numbers are superseded by this
table.

## What changed

1. **Collision band** (`deformation.rs`, `boundaries.rs::apply_additions_inner`). Crust of the
   lower plate overridden in a continental collision is no longer stacked with the fixed profile
   [1, 0.75, 0.5, 0.25] over 4 rings, but distributed over a band of the overriding plate,
   `collision_band_width` = 500 km (5 rings), by weight = linear taper × softness s⁻³ (thin
   viscous sheet, n = 3; s = lithosphere strength, floored at 0.3) × room
   1 − ΔGPE(h)/ΔGPE(70 km) (plateau force balance). Each sample counts once (at its nearest
   ring), three weighted passes (water-filling), then the rest spills nearest first as before.
   Only crust of continental origin takes a share (none in reach: the old profile). A collision
   marks a cell as orogenic only if it thickens it by ≥ 1%.
2. **Belt resistance** (`Simulation::belt_resistance`, used in `dynamics.rs`). Each collision
   boundary cell resists convergence with the excess gravitational potential energy of the
   thickest continental crust at it and its neighbours, ½ g ρc (1 − ρc/ρm)(h² − h₀²)
   (2.6·10¹² N/m at 50 km, 7.6·10¹² N/m at 70 km), × ½ cell length (both sides have collision
   cells), as a plastic force like the initiation resistance (released when the contact opens,
   linear below `locked_convergence`). Factor `belt_gpe_resistance` = 1 (physical).
3. **Rift band**: stretching donors are weighted (h/35 km)²/ring instead of 1/ring: thickened
   crust stretches first (extensional collapse; wide rifts). Identical for crust ≤ 35 km.
4. **`events.suture_convergence` 0.5 → 0.2 cm/yr** (see "The suture threshold").

Interface for #75: the band reads strength only through `deformation::band_strength`, which calls
`weak_zones::lithosphere_strength(crust, time, 1.0)`.

## Runs

`main` = 536cecb; the branch at 37f64ca (after the review fix; the rebase onto #77 does not
touch the simulation). 1000 Myr, `--snapshot-interval 50 --width 256 --cell-dump`, scheduler runs.
Means over seeds of the 500–1000 Myr snapshot means, seed range, Welch t.

| set | code | suture threshold | seeds |
|---|---|---|---|
| `main` | main | 0.5 | 0–7 |
| `main-s02` | main, `--set events.suture_convergence=0.2` | 0.2 | 0–15 |
| **`pr`** | **branch, defaults** | 0.2 | 0–15 |
| `pr-s05` | branch, `--set events.suture_convergence=0.5` | 0.5 | 0–7 |
| `gpe05` | branch, `--set belt_gpe_resistance=0.5` | 0.2 | 0–7 |
| `band-only` | branch, `belt_gpe_resistance=0`, threshold 0.5 | 0.5 | 0–3 |
| `gpe-only` | branch before the fix, `collision_band_width=0` (band path unused, so unaffected by the fix) | 0.5 | 0–3 |

Scripts: [`scripts/analyse.sh`](scripts/analyse.sh) (notes/issue-43/compare.py with
sim_classes.py, and notes/realism-batch-4/scripts/compare.py), [`scripts/belts.py`](scripts/belts.py)
(thickness and orogenic share from the cell dumps). Tables: [`guard-pr.md`](guard-pr.md) (69
metrics, `main` vs `pr`), [`guard-s02-pr.md`](guard-s02-pr.md) (`main-s02` vs `pr`),
[`belts.md`](belts.md).

## Primary metrics

| | main | main-s02 | pr-s05 | **pr** | t (pr vs main) | t (pr vs main-s02) | Earth / target |
|---|---|---|---|---|---|---|---|
| boundary slower than 2 cm/yr (%) | 13.1 | 18.3 | 13.0 | **18.6** | +3.2 | +0.2 | 39 |
| continental boundaries (rift + transform + convergent, %) | 14.3 | 18.8 | 13.6 | **17.6** | +2.1 | −1.0 | ~30 |
| continental crust > 50 km (%, log) | 6.9 | 10.3 | 2.4 | **3.1** | −2.2 | −4.6 | ~3–6 |
| mean land (m) | 923 | 1128 | 591 | **718** | −1.6 | −4.1 | ~840 |
| collision stacking (Gt/yr) | 24.3 | 33.7 | 16.5 | **20.4** | −1.0 | −3.9 | 13–23 band |

- **Crust thickness back in its band, while slow belts stay boundaries.** Lowering the suture
  threshold on `main` alone gives the same slow share (18.3%) but pushes thickness, mean land and
  stacking far out of their bands (10.3%, 1.13 km, 33.7 Gt/yr). With the band and the belt
  resistance the same slow share comes with 3.1% crust > 50 km (1.0–7.2% by seed), 0.72 km mean
  land and 20.4 Gt/yr stacking. The 99th percentile of continental thickness falls from 62 to
  55 km, the area > 60 km from 2.2% to 0.6%; highest cell 5.4 → 4.8 km (cell means of ~110 km).
- **The boundary shares move through the threshold; the band and the belt resistance make that
  affordable.** At 0.5, the branch has `main`'s shares (13.0 vs 13.1%, 13.6 vs 14.3%); 0.2 adds
  +5.6 points slow (t +3.3 vs `pr-s05`) and +4.0 points continental (t +2.7). Of the slow gain
  over `main`, 2.0 points are collision boundaries and 3.6 points others; continental rifts grow
  most (10.4 → 15.2·10³ km, t +3.8; Earth 28), continent–continent convergence 15.1 → 16.4,
  continental transforms 11.2 → 13.8·10³ km. Median speeds of these classes: 0.8 / 1.3 /
  1.3 cm/yr (Earth CCB 2.1, CRB 1.1, CTF 1.6). Collision cells converging slower than 0.2 cm/yr:
  0.8 → 1.8% of the boundary cells.
- Earth's shares (39%, ~30%) are not reached; the model goes a seventh to a quarter of the way.

## Which change does what

Paired against `main` on seeds 0–3 (crust > 50 km, mean land; t of the paired differences, n = 4):

| | main | band-only | gpe-only | pr-s05 | pr |
|---|---|---|---|---|---|
| crust > 50 km (%) | 7.0 | 4.4 (t −1.5) | 1.4 (t −2.1) | 1.5 (t −2.1) | 2.3 (t −2.3) |
| mean land (m) | 900 | 785 (t −1.0) | 487 (t −2.1) | 517 (t −2.1) | 631 (t −2.5) |

- **The belt resistance does most of the thinning**: thick belts converge less (plate speed
  4.11 → 3.97 cm/yr). Alone it flattens the continents (gpe-only: 0.49 km mean land, 40%
  of continental crust flooded).
- **The band reshapes rather than reduces**: it lowers the thick outlier seeds and evens out the
  belts (in seeds that were thin on `main` it raised the area > 50 km a little, per the results
  review); not significant on its own with 4 seeds.
- The factor of the belt resistance is not a sensitive knob: at 0.5 (`gpe05`, 8 seeds) crust
  > 50 km 3.4%, mean land 0.70 km, slow share 16.3%, all within the scatter of `pr`.

Deterministic check (test `a_collision_band_spreads_the_belt_and_conserves_mass`, two continental
hemispheres converging at 1 cm/yr for 40 Myr, ~220 km mesh, crustal flow on): with a 5-ring band
the belt > 37 km is wider, the area > 50 km smaller, the peak lower (53 vs 58 km in the
section), nothing lost at the cap. The difference is modest: crustal flow above 40 km already
spreads thick crust, and 400 km of shortening of 35 km crust spread over ~500 km still makes
~60 km crust. How much crust thickens is set by how much belts converge, which the belt
resistance limits.

## Guard rails (`main` vs `pr`, 69 metrics)

All with |t| ≥ 2 (plus a few relevant ones):

| | main | pr | t |
|---|---|---|---|
| relative speed median (cm/yr) | 6.23 | 5.92 | −2.0 |
| convergence at incipient trenches (cm/yr) | 2.42 | 2.14 | −2.7 |
| mean plate speed (cm/yr) | 4.11 | 3.97 | −1.1 |
| sea-floor production created (km²/yr) | 6.86 | 6.60 | −1.4 |
| mean ocean age (Myr) / sea floor ≥ 200 Myr (%) | 52.3 / 2.1 | 54.7 / 2.7 | +1.6 / +2.0 |
| highest point (m), max over snapshots (m) | 5408, 6067 | 4750, 5673 | −3.5, −3.6 |
| shelf mean sediment (km) | 4.18 | 3.60 | −2.0 |
| burial metamorphism on continental crust (Gt/yr) | 0.16 | 0.10 | −2.1 |
| largest river (m³/s) | 115 000 | 85 000 | −3.0 |
| terminal fans and deltas (km cells/100 Myr) | 15 300 | 9 500 | −2.2 |
| flooded continental crust (%) | 25.4 | 29.5 | +1.5 |
| subduction / ridge / transform (10³ km) | 118 / 101 / 33 | 117 / 99 / 35 | −0.1 / −0.5 / +0.9 |
| sutures / rifts / plates born per 100 Myr | 0.88 / 2.60 / 9.3 | 0.78 / 2.34 / 10.0 | −0.5 / −0.8 / +0.7 |
| stacked mass per collision length (Gt/yr per 10³ km) | 0.91 | 0.64 | |
| crust lost at the thickness cap (Gt/yr) | 0.08 | ~0.05 | |

Slower boundaries and less sea-floor production go towards Earth (production is still ~1.9×
Earth's 3.4 km²/yr). Lower relief gives smaller rivers and fewer terminal fans; sea floor ≥ 200 Myr
rises from 2.1 to 2.7% (earlier batches: 2–3.4%). With ~70 metrics a few |t| > 2 occur by chance.

Runtime, paired on this machine (seed 0, 1000 Myr, run at the same time; branch before the
review fix): `main` 498 s, branch 486 s. No measurable cost.

## The suture threshold

The rule written before the runs (plan, revision 8) compared the band code with `main` (both at
0.2) and holds for `pr`: (a) crust > 50 km 3.1% ≤ 6%, mean land 0.72 ≤ ~1.0 km, stacking 20.4 ≤ 23
Gt/yr; (b) `pr` vs `main-s02` on crust > 50 km t −4.6; (c) the deterministic test shows a wider
belt. As the results review noted, (b) tests the band, not the threshold. The evidence for the
threshold itself is `pr` vs `pr-s05` (same code): slow share 13.0 → 18.6% (t +3.3), continental
boundaries 13.6 → 17.6% (t +2.7), mean land 0.59 → 0.72 km (towards Earth), flooded continental
crust 32 → 29%, crust > 50 km 2.4 → 3.1% (in band), rifts per 100 Myr 3.2 → 2.3 (t −3.7).

It is a calibration choice within the physical range, not a consequence of the band: the slowest
active collision belts converge at ~0.2–0.5 cm/yr and Gordon's diffuse boundaries at
0.2–1.5 cm/yr. #68 chose 0.5 only to stop creeping belts from piling crust onto their edges,
which the belt resistance now prevents. Part of what 0.2 does is offset the extra sutures the belt
resistance causes (sutures per 100 Myr: `main` 0.88, `pr-s05` 1.15, `pr` 0.78): a thick belt
whose driving force is used up slows below 0.1 cm/yr and sutures. `--set
events.suture_convergence=0.5` restores #68's rule.

## Assessment

- Physically better: shortening goes into a wide band, preferentially where the lithosphere is
  weak; thick belts resist their own collision with their potential energy (Tibet's force at
  70 km); thick crust in rifts collapses first. Crust thickness moves from above its band (6.9%)
  into it (3.1%), stacking stays in its band, and slow belts can stay plate boundaries without
  piling crust: slow boundaries 13 → 19%, continental boundaries 14 → 18%.
- Not reached: Earth's 39% / ~30%. With rigid plates a slow continental boundary is a creeping
  collision or a slow rift between two plates; the rest needs intraplate diffuse zones (#70) and
  more slab-free plates (#42 territory).
- Mean land 0.72 km is below Earth's ~0.84 km (main: 0.92). Not recalibrated:
  `collision_resistance` (50) was tuned on main's thickness without a GPE term, and the belt
  resistance now takes part of its role. Lowering it would raise mean land; a decision, not done.

## Caveats

- Ablations have 4 seeds and are indicative; `gpe-only` ran on the code before the review fix
  (its band path is not used, so the fix does not affect it).
- The band is a disc on the overriding plate only (the lower plate's side is consumed every
  substep); it reaches across oceanic cells of that plate, which take no share.
- The belt resistance uses the thickest crust at the boundary cell and its neighbours, not a
  belt-integrated GPE, and neglects the thickening of the lithospheric mantle (an upper bound).
- A stalled thick belt locks (below 0.1 cm/yr) and sutures; it does not collapse into extension.
  A GPE push (collapse when the driving force goes away) is the natural follow-up.
- #75 (healing of orogenic weakness, plume reheating) changes the strengths the band reads;
  measured here without it.
- Runtimes of the scheduler runs are unpaired (GitHub runners).

## Results review: what was done

1. Wrong t values: corrected (now from the final runs). 2. Decision rule tests the band:
   `pr` vs `pr-s05` added, the threshold is presented as a calibration choice. 3. Belt
   resistance on top of a calibrated resistance: stated; sensitivity `gpe05` run. 4. Unpaired
   ablations: paired on seeds 0–3, claims revised. 5. Duplicated samples weighted several times:
   fixed (each sample once, at its nearest ring; test), `pr` and `pr-s05` re-run. 6. Band reach
   across oceanic cells: named. 7. Doc points: done. 8. Lock/collapse: caveat. 9. Missing guard
   rails: listed. 10. Diagnostics: stacked mass per collision length, runtime added; the
   largest-receiver diagnostic and a "no softness" ablation were not run (the softness is bounded
   at 37× and the deterministic and unit tests show the spread). 11. Housekeeping: done.
