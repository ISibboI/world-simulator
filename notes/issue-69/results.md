# Results: deformable plate margins (#69)

_2026-11-21. Plan: [`plan.md`](plan.md) (with its revisions after the [plan review](review-plan.md)).
Goal: an Earth-like planet, not Earth; PB2002 and hypsometry numbers are plausibility checks.
Plates stay rigid (#70)._

## What changed

1. **Collision band** (`deformation.rs`, `boundaries.rs::apply_additions_inner`). Crust of the
   lower plate overridden in a continental collision is no longer stacked with the fixed profile
   [1, 0.75, 0.5, 0.25] over 4 rings, but distributed over a band of the overriding plate,
   `collision_band_width` = 500 km (5 rings), by weight = linear taper × softness s⁻³ (thin
   viscous sheet, n = 3; s = lithosphere strength, floored at 0.3) × room
   1 − ΔGPE(h)/ΔGPE(70 km) (plateau force balance). Three weighted passes (water-filling), then
   the rest spills nearest first as before. Only crust of continental origin takes part (else
   the old profile). A collision marks a cell as orogenic only if it thickens it by ≥ 1%.
2. **Belt resistance** (`Simulation::belt_resistance`, used in `dynamics.rs`). Each collision
   boundary cell resists convergence with the excess gravitational potential energy of the
   thickest continental crust at it and its neighbours, ½ g ρc (1 − ρc/ρm)(h² − h₀²)
   (2.6·10¹² N/m at 50 km, 7.6·10¹² N/m at 70 km), as a plastic force like the initiation
   resistance (released when the contact opens, linear below `locked_convergence`). Factor
   `belt_gpe_resistance` = 1 (physical).
3. **Rift band**: stretching donors are weighted (h/35 km)²/ring instead of 1/ring: thickened
   crust stretches first (extensional collapse). Identical for crust ≤ 35 km.
4. **`events.suture_convergence` 0.5 → 0.2 cm/yr**, by the decision rule written before the runs
   (plan, revision 8): see "The suture threshold".

Interface for #75: the band reads strength only through `deformation::band_strength`, which calls
`weak_zones::lithosphere_strength(crust, time, 1.0)`.

## Runs

`main` = 536cecb; the branch at e834b96 (before the rebase onto #77, which does not change
simulation output). 1000 Myr, `--snapshot-interval 50 --width 256 --cell-dump`, scheduler runs
(GitHub runners). Means over seeds of the 500–1000 Myr snapshot means, seed range, Welch t.

| set | code | suture threshold | seeds |
|---|---|---|---|
| `main` | main | 0.5 | 0–7 |
| `main-s02` | main | 0.2 | 0–15 |
| `full` | branch (1–3) | 0.5 | 0–7 |
| **`full-s02`** | **branch (1–3) = the PR's defaults** | 0.2 | 0–15 |
| `band-only` | branch, `belt_gpe_resistance=0` | 0.5 | 0–3 |
| `gpe-only` | branch, `collision_band_width=0` | 0.5 | 0–3 |

Scripts: [`scripts/analyse.sh`](scripts/analyse.sh) (runs notes/issue-43/compare.py with
sim_classes.py, and notes/realism-batch-4/scripts/compare.py), [`scripts/belts.py`](scripts/belts.py)
(thickness and orogenic share from the cell dumps).

## Primary metrics

| | main | main-s02 | full | **full-s02 (PR)** | t (PR vs main) | Earth / target |
|---|---|---|---|---|---|---|
| boundary slower than 2 cm/yr (%) | 13.1 | 18.3 | 12.1 | **16.9** | +2.4 | 39 |
| continental boundaries (rift + transform + convergent, %) | 14.3 | 18.8 | 13.7 | **16.5** | +1.5 | ~30 |
| continental crust > 50 km (%, cell dumps) | 6.8 | 10.3 | 2.1 | **3.0** | −2.6 | ~3–6 |
| mean land (m) | 923 | 1128 | 598 | **679** | −2.0 | ~840 |
| collision stacking (Gt/yr) | 24.3 | 33.7 | 17.1 | **18.9** | −1.4 | 13–23 band |

(main-s02 and full-s02: 16 seeds; t of full-s02 vs main-s02 on crust > 50 km −2.4, mean land
−4.7, stacking −4.6, slow share −1.4, continental share −2.3.)

- **Thickness is fixed**: `main` is above the calibration band (6.8%, 1.5–13.9% by seed), and
  keeping creeping belts made it worse (10.3%). With the band and the belt resistance it is
  2–3% (1.0–6.6% by seed); the 99th percentile of continental thickness falls from 62 to 55 km,
  the area > 60 km from 2.2% to 0.6%. Highest cell 5.4 → 4.7 km (cell means of ~110 km; Tibet
  averages ~5 km). Seed scatter of the hypsometry shrinks (mean land 469–1397 → 438–904 m).
- **Slow and continental boundaries move only through the suture threshold**, as the plan review
  predicted: at 0.5 the branch has the same shares as `main` (12.1 vs 13.1%, 13.7 vs 14.3%);
  with 0.2 they rise to 16.9% and 16.5% (vs 13.1% and 14.3% on `main`). That is a third of the
  way to Earth for the slow share and a seventh for the continental share. The belt resistance
  works against persistence: thick belts slow down and then stall and suture (sutures per 100 Myr
  0.88 → 1.32 at 0.5; 0.35 → 0.99 at 0.2). So `main-s02` keeps more slow boundaries than
  `full-s02` (18.3 vs 16.9%), but only by thickening crust far beyond the band.
- **Mean land falls to 0.68 km** (Earth ~0.84; the calibration band of `collision_resistance`
  was 0.6–0.8 km): less crust is stacked (18.9 vs 24.3 Gt/yr, inside its band), the same
  continental volume covers more area (continental area 32.8 → 34.8% of the surface) at lower
  elevation; flooded continental crust 25 → 29%.

## Which change does what (4 seeds each for the ablations, against `main`)

| | main | band-only | gpe-only | full |
|---|---|---|---|---|
| crust > 50 km (%) | 6.8 | 3.8 (t −1.9) | 1.4 (t −3.4) | 2.1 (t −2.9) |
| thickness p99 (km) | 62 | 60 | 51 | 54 |
| mean land (m) | 923 | 697 | 487 | 598 |
| flooded continental crust (%) | 25 | 26 | 40 | 33 |
| collision stacking (Gt/yr) | 24.3 | 15.7 | 12.2 | 17.1 |
| slower than 2 cm/yr (%) | 13.1 | 12.1 | 11.4 | 12.1 |
| orogenic share ≥ 0.5 (% of continental) | 39 | 28 | 27 | 27 |

- The belt resistance does most of the thinning (less convergence at thick belts: plate speed
  4.11 → 3.69 cm/yr on its own), but alone it flattens the continents too much (mean land
  0.49 km, 40% flooded). The band spreads the stacked crust (deterministic test below) and,
  combined with it, gives the middle (0.60–0.68 km).
- Band-only also lowers the stacking rate (15.7 Gt/yr) and the collision length (19 vs
  27·10³ km); with 4 seeds this is within scatter (t −1.8 / −1.5), and band-only has fewer
  rifts (1.5 vs 2.6 per 100 Myr, t −2.2), probably via the smaller orogenic (weak) area.

Deterministic check (test `a_collision_band_spreads_the_belt_and_conserves_mass`, two continental
hemispheres converging at 1 cm/yr for 40 Myr, ~220 km mesh, crustal flow on): with a 5-ring band
the belt > 37 km is wider, the area > 50 km smaller, the peak lower (53 vs 58 km in the
section), nothing lost at the cap. The difference is modest because crustal flow above 40 km
already spreads thick crust and because 400 km of shortening of 35 km crust has to go
somewhere: spread over ~500 km it still makes ~60 km crust. The band is a shape change; the
amount of thickening is set by how much the belts converge, which the belt resistance limits.

## Guard rails (`main` vs `full-s02`, 69 metrics of batch 4)

Full tables: [`guard-full-s02.md`](guard-full-s02.md) (and [`guard-full.md`](guard-full.md) for
the 0.5 threshold). Notable (|t| ≥ 2 or relevant):

| | main | full-s02 | t |
|---|---|---|---|
| mean plate speed (cm/yr) | 4.11 | 3.95 | −1.4 |
| sea-floor production created (km²/yr) | 6.86 | 6.50 | −1.8 |
| mean ocean age (Myr) / sea floor ≥ 200 Myr (%) | 52.3 / 2.1 | 57.3 / 3.1 | +2.7 / +2.8 |
| slab pull (10²¹ N) | 1.15 | 1.10 | −2.1 |
| subduction / ridge / transform (10³ km) | 118 / 101 / 33 | 113 / 97 / 33 | −1.1 / −1.1 / +0.3 |
| continental rift (10³ km) | 10.4 | 12.6 | +1.9 |
| continent–continent convergent (10³ km, sim_classes) | 15.1 | 15.4 | | 
| continental (% of surface) | 32.8 | 34.8 | +1.9 |
| shelf mean sediment (km) | 4.18 | 3.60 | −2.0 |
| highest point over all snapshots (m) | 6067 | 5678 | −3.5 |
| sutures / rifts / plates born per 100 Myr | 0.88 / 2.60 / 9.3 | 0.99 / 2.69 / 10.0 | +0.6 / +0.3 / +0.8 |
| crust lost at the cap (Gt/yr) | 0.08 | 0.05 | |

Slower plates and less sea-floor production go towards Earth (production ~1.9× Earth's
3.4 km²/yr), at the price of slightly older sea floor (still within the range of earlier
batches). With ~70 metrics, a few |t| > 2 are expected by chance.

Runtime, paired on this machine (seed 0, 1000 Myr, same flags, run at the same time): `main` 498 s, branch (`full`) 486 s: no measurable cost (one extra BFS ring and a strength call per reached cell per collision addition; one more plastic term per collision cell in the velocity solve).

## The suture threshold

Rule fixed before the runs (plan, revision 8): propose 0.2 only if (a) `full-s02` is in band on
crust > 50 km (≤ 6%: 3.0 ✓), mean land (≤ ~1.0 km: 0.68 ✓) and stacking (≤ 23 Gt/yr: 18.9 ✓);
(b) `full-s02` beats `main-s02` on crust > 50 km with |t| > 2 (t −2.4 ✓); (c) the deterministic
test shows the band widening the belt (✓). All three hold, so the PR sets 0.2. The physics
behind it: the slowest active collision belts converge at ~0.2–0.5 cm/yr and Gordon's diffuse
boundaries at 0.2–1.5 cm/yr; #68 chose 0.5 only to stop creeping belts piling crust onto their
edges, which the belt resistance now prevents. `--set events.suture_convergence=0.5` restores
#68's rule.

## Assessment

- Physically better: shortening goes into a wide band preferentially where the lithosphere is
  weak, thick belts resist their own collision with their potential energy (Tibet's force at
  70 km), thick crust in rifts collapses first. Crust thickness moves from above the band
  (6.8%) into it (3.0%), stacking into its band, plates and sea-floor production get slower.
- Not reached: Earth's slow (39%) and continental (~30%) boundary shares; the model goes from
  13 to 17% and from 14 to 16.5%. With rigid plates a slow continental boundary is a creeping
  collision or a slow rift between two plates; the remaining gap needs intraplate diffuse zones
  (deforming plates, #70) and more slab-free plates (#42 territory).
- Mean land drops below Earth's (0.68 vs ~0.84 km). Not recalibrated: `collision_resistance`
  (50) was tuned on main's thickness with no GPE term; with the physical GPE resistance part of
  its role is now taken by a derived force. Lowering it towards the GPE-free share would raise
  mean land again; that is a decision (see below), not done here.

## Caveats

- `main`, `full` and the ablations have 8 resp. 4 seeds; ablation statements are indicative.
- The band is on the overriding plate only (the lower plate's side is consumed every substep).
- The belt resistance uses the thickest crust at the boundary cell and its neighbours, not a
  belt-integrated GPE; the lithospheric mantle's thickening (which lowers ΔGPE) is neglected.
- #75 (healing of orogenic weakness, plume reheating) changes the strengths the band reads;
  measured here without it.
- Runtimes of the scheduler runs are unpaired (GitHub runners).
