# Batch 2 results: buoyant young lithosphere and resistance to subduction initiation

_2026-11-20. Measurement: [`measurement.md`](measurement.md), plan: [`plan.md`](plan.md), plan
review: [`review-plan.md`](review-plan.md). `main` at 11989cb against the branch, 8 seeds (0–7)
× 1000 Myr, default parameters. Mean over seeds (seed range); rates and levels over the
snapshots of 600–1000 Myr, counters and slopes over 500–1000 Myr; Welch t, n = 8
(|t| ≳ 2.4 ≈ p < 0.05)._

## Result in one sentence

The two physical gaps are closed and unit-tested, and **nothing at the scale of the planet
changes significantly**: no metric of the issues and no guard rail differs from `main` beyond
the seed scatter.

| | main | branch | t |
|---|---|---|---|
| Sea-floor production, kinematic (km²/yr) | 5.90 (5.29…6.49) | 5.75 (5.52…6.16) | −1.0 |
| Trench / ridge length (10³ km) | 123 / 95 | 125 / 97 | +0.5 / +0.4 |
| Median relative speed (cm/yr) | 6.90 | 6.71 | −1.1 |
| Boundary slower than 2 cm/yr (%) | 5.6 | 6.0 | +0.6 |
| Sea floor younger than 20 Myr (% of ocean) | 34.5 | 34.9 | +0.4 |
| Mean sea-floor age (Myr) | 55.1 | 51.7 | −1.4 |
| Sea floor ≥ 200 Myr (%) | 4.0 | 3.4 | −1.0 |
| Subduction flux carried by crust younger than 20 Myr (%) | 26.7 (23.1…32.3) | 23.0 (19.1…26.1) | |
| Trench length subducting crust younger than 10 Myr (%) | 17.0 | 15.7 | |
| Slab pull (10²¹ N) | 1.39 | 1.34 | −1.5 |
| Incipient trenches (slab < 150 km; 10³ km) | — | 9.8, converging at 1.9 cm/yr (others 4.7) | |
| Dynamic topography minimum (m; #37) | −1932 | −1976 | −0.4 |
| Runtime (s) | 689 (563…808) | 567 (523…595) | −3.9 |

The runtime difference is probably load on the machine during the `main` runs (another task
was using cores at the time; not verified). The branch adds solves, so it cannot be faster.

## Why so little

- **The first version did move the numbers** (seeds 0–2: production −11%, slab pull −25%,
  relative speed −0.8 cm/yr). The plan review showed why: it subtracted the crustal buoyancy
  along the whole slab, which cut the pull of every slab younger than 100 Myr (0.43× at
  20 Myr, 0.73× at 40 Myr). That was a recalibration of mature subduction, not the effect of
  buoyant young lithosphere. With the buoyancy confined to the shallow slab, where the crust
  has not yet turned to eclogite, the effect is gone.
- **Incipient trenches are few and yield.** Only 8% of the trench length has a slab shorter
  than 150 km. Those contacts converge at 1.9 cm/yr on average: the plates on both sides are
  driven by mature slabs elsewhere, with forces an order of magnitude above the resistance
  (slab pull 2 × 10¹³ N/m against 3 × 10¹² N/m), so the resistance slows them but does not
  stop them.
- **Buoyant slabs are probably rare** (an inference, not measured): the buoyancy only
  outweighs the pull while the slab is shorter than ~150 km, and only 8% of the trench length
  is in that state.

So #42's premise ("nothing resists subduction initiation") was true, and fixing it is right,
but it does not shape the planet. The cause of the trench length and the production is in the
organisation of the plates ([`measurement.md`](measurement.md)).

## Plate structure (main → branch)

| metric | main | branch |
|---|---|---|
| plates >= 1% of the surface | 13.3 (10.8…16.8) | 14.1 (9.5…16.3) |
| ... of them with own mature trenches on >= 10% of their boundary | 9.5 (7.2…11.2) | 10.1 (7.0…11.7) |
| surface on plates with own mature trench share < 2% (%) | 15.7 (4.7…27.3) | 19.0 (3.9…52.8) |
| surface on plates with own mature trench share 2-10% (%) | 35.2 (22.0…45.9) | 32.3 (2.2…49.3) |
| surface on plates with own mature trench share >= 10% (%) | 49.0 (38.4…58.1) | 48.7 (45.0…55.8) |
| mean speed of cells on slab-attached plates (cm/yr) | 5.9 (5.2…7.0) | 6.1 (5.2…7.2) |
| mean speed of cells on other plates (cm/yr) | 2.1 (1.3…2.6) | 1.9 (0.6…2.7) |
| continental crust on slab-attached plates (% of continental cells) | 17.1 (2.1…33.6) | 11.5 (0.1…27.3) |
| boundary between plates: neither plate slab-attached (% of boundary) | 9.1 (5.3…13.3) | 9.3 (4.8…12.3) |
| median relative speed there (cm/yr) | 6.2 (5.5…6.9) | 6.0 (5.6…6.7) |
| slower than 2 cm/yr there (%) | 11.9 (6.1…18.1) | 11.3 (5.7…13.7) |
| boundary between plates: one plate slab-attached (% of boundary) | 53.3 (44.3…66.0) | 52.0 (44.7…63.5) |
| boundary between plates: both plates slab-attached (% of boundary) | 37.7 (23.9…48.4) | 38.7 (31.8…45.9) |
| boundary slower than 2 cm/yr (%) | 5.5 (3.6…8.4) | 5.7 (3.2…7.4) |
| median relative speed (cm/yr) | 6.8 (6.3…7.3) | 6.7 (6.3…7.2) |
| trench length (10^3 km) | 112.7 (103.0…128.6) | 114.9 (109.7…128.3) |
| trench: slab < 150 km (% of trench) | 7.1 (4.6…9.5) | 7.6 (4.8…10.1) |
| trench: subducting crust younger than 20 Myr (%) | 33.3 (28.6…37.2) | 30.6 (27.3…33.3) |
| trench: subducting crust younger than 10 Myr (%) | 17.0 (14.7…19.2) | 15.7 (13.0…18.0) |
| trench: subducting crust older than 100 Myr (%) | 9.2 (5.4…11.8) | 9.4 (3.7…13.1) |
| trench: median age of subducting oceanic crust (Myr) | 32.4 (28.6…36.0) | 35.1 (32.1…41.7) |
| trench: convergence slower than 2 cm/yr (%) | 19.7 (15.7…23.5) | 20.7 (14.9…25.6) |
| trench: overriding plate is itself slab-attached (%) | 35.9 (15.5…49.3) | 35.8 (18.6…50.5) |
| trench: mean convergence (cm/yr) | 4.6 (4.2…4.9) | 4.5 (4.1…4.9) |
| subduction flux carried by crust younger than 20 Myr (%) | 26.7 (23.1…32.3) | 23.0 (19.1…26.1) |
| ridge flank rows: opening slower than 2 cm/yr (%) | 14.0 (11.3…16.3) | 14.8 (11.4…18.3) |
| ancient (>= 200 Myr) oceanic cells (% of oceanic) | 4.9 (3.6…7.9) | 4.6 (3.6…6.3) |
| ancient: on plates without own mature trench share >= 10% (%) | 76.1 (57.3…93.6) | 81.3 (64.6…95.8) |
| ancient: on plates without any own trench cell (%) | 0.4 (0.0…2.9) | 2.5 (0.0…17.8) |
| ancient: median distance to an own-plate trench (km; cells that have one) | 2968.1 (2294.0…3959.0) | 3321.2 (2275.5…6882.0) |
| ancient: farther than 1000 km from an own-plate trench or none (%) | 81.7 (75.6…89.8) | 83.4 (77.4…93.8) |
| ancient: within 300 km of continental crust of its plate (%) | 84.4 (74.6…90.7) | 83.1 (76.3…88.3) |
| ancient: shallower than 2000 m (filled; %) | 59.8 (49.7…72.6) | 60.2 (49.4…70.3) |
| ancient: mean sediment (km) | 8.9 (7.6…9.6) | 8.7 (8.0…9.1) |
| ancient: within 550 km of a trench of another plate (active margin of its own plate; %) | 11.2 (6.4…14.9) | 11.2 (6.9…19.2) |
| ancient: within 550 km of a trench of its own plate (%) | 9.6 (5.2…12.2) | 9.6 (3.7…14.7) |
| ancient: farther than 550 km from any trench (%) | 79.3 (74.3…83.5) | 79.2 (73.3…83.9) |
| ancient: age p50 (Myr) | 331.0 (290.6…400.3) | 333.0 (307.8…360.0) |
| ancient: age p90 (Myr) | 653.6 (527.0…758.9) | 704.0 (623.2…780.3) |
| ancient older than 400 Myr (% of oceanic) | 1.7 (1.0…2.4) | 1.7 (1.1…2.8) |
| ancient: at a plate boundary (%) | 3.0 (1.6…4.5) | 2.9 (1.8…5.1) |
| all oceanic: farther than 1000 km from an own-plate trench or none (%) | 53.3 (47.2…58.6) | 51.5 (44.2…58.7) |
| all oceanic: on plates without own mature trench share >= 10% (%) | 27.6 (18.3…34.1) | 24.0 (18.5…29.1) |

## Full comparison (8 seeds)

window 500-1000 Myr; seeds: main: 8, v2: 8
| metric | main | v2 | t |
|---|---|---|---|
| plates | 24.07 (19.40…29.60) | 24.45 (21.80…27.80) | +0.2 |
| plates > 1% | 13.43 (10.00…16.60) | 14.12 (9.00…16.60) | +0.6 |
| mean speed (cm/yr) | 3.75 (3.48…4.20) | 3.89 (3.44…4.80) | +0.8 |
| speed 99% (cm/yr) | 9.19 (8.36…10.00) | 9.09 (8.26…10.18) | -0.4 |
| trenches (10^3 km) | 123 (110…138) | 125 (116…139) | +0.5 |
| ridges (10^3 km) | 95.17 (81.14…107.13) | 97.25 (75.99…110.29) | +0.4 |
| collisions (10^3 km) | 11.43 (4.80…21.68) | 11.76 (5.28…16.71) | +0.1 |
| incipient trenches (10^3 km) | — | 9.79 (5.58…12.25) | — |
| convergence at incipient trenches (cm/yr) | — | 1.87 (1.74…1.98) | — |
| convergence at other trenches (cm/yr) | — | 4.72 (4.42…5.16) | — |
| slab pull (1e21 N) | 1.39 (1.29…1.48) | 1.34 (1.20…1.43) | -1.5 |
| relative speed median (cm/yr) | 6.90 (6.44…7.52) | 6.71 (6.32…7.28) | -1.1 |
| boundary slower than 2 cm/yr (%) | 5.58 (3.20…9.20) | 6.03 (3.40…7.60) | +0.6 |
| opening/closing (%) | 61.60 (59.60…64.60) | 60.88 (56.40…66.00) | -0.5 |
| strike-slip (%) | 13.95 (12.80…14.60) | 14.70 (11.60…17.60) | +0.9 |
| sea-floor production kinematic (km2/yr) | 5.90 (5.29…6.49) | 5.75 (5.52…6.16) | -1.0 |
| sea-floor production created (km2/yr) | 6.51 (5.69…7.12) | 6.44 (5.97…6.77) | -0.4 |
| mean sea-floor age (Myr) | 55.12 (47.04…66.54) | 51.73 (48.42…55.98) | -1.4 |
| ocean age median (Myr) | 33.85 (31.20…41.00) | 32.38 (28.00…38.20) | -0.9 |
| sea floor >= 200 Myr (%) | 4.04 (2.01…7.06) | 3.35 (2.23…5.12) | -1.0 |
| sea floor < 20 Myr (% of ocean) | 34.50 (31.80…36.40) | 34.90 (30.40…38.40) | +0.4 |
| sea floor 20-40 Myr (%) | 22.70 (21.00…23.80) | 23.30 (21.20…25.00) | +1.0 |
| ocean deeper than 6 km (%) | 3.54 (2.26…4.82) | 3.88 (2.76…6.68) | +0.7 |
| continental (% of surface) | 34.73 (31.38…36.82) | 35.31 (32.82…37.76) | +0.7 |
| land (% of surface) | 28.42 (26.38…32.70) | 27.88 (25.46…31.78) | -0.5 |
| mean land (m) | 527 (384…693) | 519 (318…724) | -0.1 |
| highest point (m) | 4458 (3968…5218) | 4515 (3490…5298) | +0.2 |
| flooded continental crust (%) | 32.17 (21.40…39.20) | 34.23 (18.60…41.40) | +0.6 |
| crust > 50 km (% of continental) | 1.57 (0.80…2.60) | 1.35 (0.00…2.60) | -0.6 |
| sediment part of sea level (m) | 277 (241…331) | 255 (233…279) | -1.8 |
| land lower than 10 m (% of land) | 3.42 (1.90…4.48) | 3.62 (2.22…5.28) | +0.5 |
| shelf area (% of surface) | 7.23 (4.10…8.48) | 7.16 (4.54…8.70) | -0.1 |
| shelf mean sediment (km) | 3.92 (3.32…4.60) | 3.59 (2.83…4.60) | -1.3 |
| dynamic topography min (m) | -1932 (-2247…-1704) | -1976 (-2355…-1744) | -0.4 |
| dynamic topography max (m) | 1116 (933…1243) | 1180 (1050…1328) | +1.1 |
| sea-level budget mantle (m/100 Myr) | -53.54 (-92.60…-29.00) | -52.91 (-83.02…-30.40) | +0.1 |
| felsic+sediment net (Gt/yr) | -0.88 (-2.30…0.88) | -0.62 (-2.58…1.22) | +0.5 |
| trench sediment (Gt/yr) | 4.51 (2.62…6.20) | 4.14 (2.42…6.34) | -0.6 |
| collision stacking (Gt/yr) | 13.31 (6.56…22.30) | 11.98 (3.50…18.78) | -0.5 |
| burial metamorphism on continental crust (Gt/yr) | 0.13 (0.08…0.19) | 0.09 (0.06…0.15) | -1.9 |
| burial metamorphism on other crust (Gt/yr) | 0.73 (0.42…1.11) | 0.61 (0.37…1.05) | -1.0 |
| sediment trapped in lakes (km3/yr) | 0.15 (0.09…0.21) | 0.15 (0.06…0.29) | +0.1 |
| depressions (lakes) | 240 (186…316) | 236 (146…301) | -0.2 |
| land precipitation (m/yr) | 0.78 (0.58…0.88) | 0.76 (0.60…0.86) | -0.4 |
| runoff (km3/yr) | 37846 (33008…44020) | 36020 (31575…40599) | -1.0 |
| largest river (m3/s) | 75791 (41592…115573) | 87294 (66435…120220) | +1.1 |
| sediment to ocean (km3/yr) | 3.71 (2.88…4.48) | 3.44 (2.30…4.34) | -0.9 |
| endorheic (% of land) | 11.17 (7.50…20.22) | 11.12 (4.22…19.18) | -0.0 |
| endorheic into single-cell pits (% of land) | 6.08 (4.08…10.86) | 6.07 (2.00…8.72) | -0.0 |
| lakes (% of land) | 2.29 (1.86…2.58) | 2.27 (1.66…3.00) | -0.1 |
| lake evaporation (km3/yr) | 2274 (1596…3192) | 2065 (1313…3166) | -0.7 |
| global temperature (C) | 14.73 (13.20…15.42) | 14.71 (13.04…16.00) | -0.0 |
| equator (C) | 26.61 (25.12…27.50) | 26.67 (24.84…27.96) | +0.1 |
| poles (C) | -13.07 (-14.10…-11.92) | -13.06 (-14.08…-11.62) | +0.0 |
| permanently frozen (%) | 10.61 (9.12…12.36) | 11.03 (9.22…13.38) | +0.7 |
| ocean heat transport 20N (PW) | 1.24 (1.04…1.36) | 1.23 (0.86…1.34) | -0.2 |
| gyres (Sv) | 11.65 (10.20…14.20) | 12.50 (9.60…14.40) | +1.1 |
| rifts /100 Myr | 3.75 (3.20…4.40) | 3.98 (1.80…5.20) | +0.5 |
| initiations /100 Myr | 7.78 (4.20…10.80) | 6.97 (4.20…9.40) | -0.7 |
| sutures /100 Myr | 2.70 (1.00…4.20) | 2.85 (1.20…4.40) | +0.3 |
| plates born /100 Myr | 12.30 (8.00…16.20) | 11.68 (6.40…14.60) | -0.5 |
| segments created /100 Myr | 503 (425…575) | 486 (391…535) | -0.6 |
| terminal fans and deltas (km cells /100 Myr) | 5747 (2250…10461) | 5381 (2138…11684) | -0.3 |
| overfill (km cells /100 Myr) | 0.00 (0.00…0.00) | 0.00 (0.00…0.00) | — |
| sediment-part slope (m/100 Myr) | 8.39 (-1.55…19.92) | 6.20 (-5.27…17.87) | -0.6 |
| sediment part at 1000 Myr (m) | 277 (258…326) | 274 (229…306) | -0.3 |
| highest point, max over all snapshots (m) | 5852 (5078…6239) | 5724 (5344…6167) | -0.8 |
| runtime (s) | 689 (563…808) | 567 (523…595) | -3.9 |

## For the issues

- **#42:** the missing resistance is implemented. Measured consequence: none significant.
  Recommend closing it with this PR, with #32/#43 carrying the open question.
- **#32, #43:** not solved. The measurement narrows them: twice as many plates carry slabs as
  on Earth, and plates without slabs do not border each other; continent arrangement,
  initiation at existing boundaries and plume rifts in oceanic plates are ruled out as causes.
  Untested candidates: the coupling of continental plates to the mantle flow, and how
  subduction initiation and its spreading divide plates.
- **#14:** the ancient sea floor is passive-margin basement under ~9 km of sediment, far from
  any trench. Whether that is a defect is a judgement: Earth has such crust, less of it and
  younger. No change in this batch.
- **#6:** created production exceeds the kinematic one by 12% on both; about 60% of that is
  real area, the rest boundary churn. No change in this batch.
- **#37:** unchanged (−1.9 km), as the subduction flux is unchanged.
