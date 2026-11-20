# Batch 2 results: buoyant young lithosphere and resistance to subduction initiation

_2026-11-20. Measurement: [`measurement.md`](measurement.md), plan: [`plan.md`](plan.md), plan
review: [`review-plan.md`](review-plan.md), results evaluation:
[`review-results.md`](review-results.md). `main` at 11989cb against the branch, 8 seeds (0–7)
× 1000 Myr, default parameters. Mean over seeds (seed range); rates and levels over the
snapshots of 600–1000 Myr, counters and slopes over 500–1000 Myr; Welch t, n = 8
(|t| ≳ 2.4 ≈ p < 0.05)._

Three versions of the branch were run on 8 seeds: the first (flawed, see the plan), the second
(after the plan review; the one the results evaluation examined) and the final one, which
differs from the second only in letting the force iteration settle (see "Results evaluation").
The tables are the final version.

## Result in one sentence

The two physical gaps are closed and unit-tested; the planet changes a little in the expected
direction and **nothing changes much**: slab pull −7% (t −3.1) is the only significant
difference, and no guard rail moves.

| | main | branch | t |
|---|---|---|---|
| Sea-floor production, kinematic (km²/yr) | 5.90 (5.29…6.49) | 5.68 (5.10…6.21) | −1.3 |
| Trench length (10³ km) | 123 | 122 | −0.2 |
| Slab pull (10²¹ N) | 1.39 | **1.29** | **−3.1** |
| Median relative speed (cm/yr) | 6.90 | 6.58 | −1.9 |
| Boundary slower than 2 cm/yr (%) | 5.6 | 7.7 | +1.9 |
| Subduction flux carried by crust younger than 20 Myr (%) | 26.7 (23.1…32.3) | 23.7 (19.0…27.7) | −1.9 |
| Median age of the subducting sea floor (Myr) | 32.4 | 35.1 | +1.9 |
| Sea floor younger than 20 Myr (% of ocean) | 34.5 | 34.1 | −0.5 |
| Mean sea-floor age (Myr) | 55.1 | 53.7 | −0.5 |
| Sea floor ≥ 200 Myr (%) | 4.0 | 3.5 | −0.9 |
| Trench length with a slab shorter than 150 km (%) | 7.1 | 8.5 | +1.7 |
| Dynamic topography minimum (m; #37) | −1932 | −2030 | −0.8 |
| Unsettled velocity solves per run | 0 (seed 0 × 300 Myr) | 0 in all 8 runs | |
| Runtime, paired (seed 0 × 300 Myr, same load; s) | 121 (second version) | 121 | |

- Less young sea floor is subducted, the subducting sea floor is 3 Myr older, plates pull 7%
  less and move a little more slowly against each other. Each of these is what the buoyancy
  should do; none but the slab pull is significant on 8 seeds. In the second version the drop
  of the young-crust flux was 26.7 → 23.0% with t −2.7.
- Earth's share of the subduction flux carried by crust younger than 20 Myr is ~11%. The
  change closes a fifth of that gap.
- The 8-seed runtimes (689 against 570 s) differ because the machine was busier during the
  `main` runs. Paired timings show no difference (second version against `main`, seeds 0–1 ×
  300 Myr side by side: 162/160 against 162/159 s).
- The log's "incipient trenches … converging at 1.9 cm/yr" uses each cell's raw zig-zag normal
  and overstates it; with the smoothed normals of the force balance it is ~0.5 cm/yr (results
  evaluation, second version).

## Why so little

- **The first version did move the numbers** (seeds 0–2: production −11%, slab pull −25%,
  relative speed −0.8 cm/yr). The plan review showed why: it subtracted the crustal buoyancy
  along the whole slab, which cut the pull of every slab younger than 100 Myr (0.43× at
  20 Myr, 0.73× at 40 Myr). That was a recalibration of mature subduction, not the effect of
  buoyant young lithosphere. With the buoyancy confined to the shallow slab, where the crust
  has not yet turned to eclogite, most of the effect is gone.
- **The resistance has almost nothing to act on** (results evaluation, second version).
  Incipient trenches are 7–8% of the trench length on `main` and on the branch alike, and they
  already converge slowly (0.5–0.6 cm/yr; 40% of their length below 0.1 cm/yr). Contacts that
  carry the resistance outside trenches are ~1.8% of the boundary. Trenches started by the
  initiation events begin with a 150 km slab and are exempt by design.
- **Buoyant slabs are rare:** 4.5% of the trench length, on `main` and on the branch.

So #42's premise ("nothing resists subduction initiation") was true, and fixing it is right,
but it does not shape the planet. The cause of the trench length and the production is in the
organisation of the plates ([`measurement.md`](measurement.md)).

## Plate structure (main → branch)

| metric | main | branch | t |
|---|---|---|---|
| plates >= 1% of the surface | 13.3 (10.8…16.8) | 14.1 (10.0…16.5) | +0.8 |
| ... of them with own mature trenches on >= 10% of their boundary | 9.5 (7.2…11.2) | 9.8 (8.0…11.2) | +0.5 |
| surface on plates with own mature trench share < 2% (%) | 15.7 (4.7…27.3) | 15.3 (5.9…25.9) | -0.1 |
| surface on plates with own mature trench share 2-10% (%) | 35.2 (22.0…45.9) | 35.9 (27.1…45.9) | +0.2 |
| surface on plates with own mature trench share >= 10% (%) | 49.0 (38.4…58.1) | 48.8 (43.3…57.2) | -0.1 |
| mean speed of cells on slab-attached plates (cm/yr) | 5.9 (5.2…7.0) | 5.9 (5.0…7.1) | +0.0 |
| mean speed of cells on other plates (cm/yr) | 2.1 (1.3…2.6) | 2.1 (0.8…2.4) | +0.1 |
| continental crust on slab-attached plates (% of continental cells) | 17.1 (2.1…33.6) | 15.5 (0.0…35.1) | -0.3 |
| boundary between plates: neither plate slab-attached (% of boundary) | 9.1 (5.3…13.3) | 11.0 (6.9…14.1) | +1.5 |
|    median relative speed, neither plate slab-attached (cm/yr) | 3.5 (1.9…5.3) | 2.7 (1.8…3.4) | -2.0 |
|    slower than 2 cm/yr, neither plate slab-attached (%) | 34.0 (15.4…55.9) | 38.3 (18.1…60.2) | +0.6 |
| boundary between plates: one plate slab-attached (% of boundary) | 53.3 (44.3…66.0) | 50.1 (41.4…65.8) | -0.9 |
|    median relative speed, one plate slab-attached (cm/yr) | 6.6 (6.1…7.4) | 6.3 (5.8…6.8) | -1.4 |
|    slower than 2 cm/yr, one plate slab-attached (%) | 2.9 (1.4…5.8) | 4.1 (0.3…7.5) | +1.2 |
| boundary between plates: both plates slab-attached (% of boundary) | 37.7 (23.9…48.4) | 38.8 (27.3…48.0) | +0.3 |
|    median relative speed, both plates slab-attached (cm/yr) | 8.3 (7.7…9.5) | 8.1 (7.4…8.8) | -0.9 |
|    slower than 2 cm/yr, both plates slab-attached (%) | 1.5 (0.8…2.9) | 2.5 (1.4…3.4) | +2.7 |
| boundary slower than 2 cm/yr (%) | 5.5 (3.6…8.4) | 7.3 (3.7…9.6) | +1.8 |
| median relative speed (cm/yr) | 6.8 (6.3…7.3) | 6.6 (6.1…6.9) | -1.6 |
| trench length (10^3 km) | 112.7 (103.0…128.6) | 112.4 (98.4…121.9) | -0.1 |
| trench: slab < 150 km (% of trench) | 7.1 (4.6…9.5) | 8.5 (4.9…10.2) | +1.7 |
| trench: subducting crust younger than 20 Myr (%) | 33.3 (28.6…37.2) | 31.2 (27.7…35.2) | -1.6 |
| trench: subducting crust younger than 10 Myr (%) | 17.0 (14.7…19.2) | 16.1 (13.7…18.6) | -1.0 |
| trench: subducting crust older than 100 Myr (%) | 9.2 (5.4…11.8) | 10.7 (5.6…14.2) | +1.2 |
| trench: median age of subducting oceanic crust (Myr) | 32.4 (28.6…36.0) | 35.1 (30.7…38.9) | +1.9 |
| trench: convergence slower than 2 cm/yr (%) | 19.7 (15.7…23.5) | 22.1 (15.9…25.0) | +1.8 |
| trench: overriding plate is itself slab-attached (%) | 35.9 (15.5…49.3) | 38.6 (11.1…49.9) | +0.5 |
| trench: mean convergence (cm/yr) | 4.6 (4.2…4.9) | 4.5 (4.2…4.8) | -0.9 |
| subduction flux carried by crust younger than 20 Myr (%) | 26.7 (23.1…32.3) | 23.7 (19.0…27.7) | -1.9 |
| ridge flank rows: opening slower than 2 cm/yr (%) | 14.0 (11.3…16.3) | 15.1 (12.5…18.0) | +1.2 |
| ancient (>= 200 Myr) oceanic cells (% of oceanic) | 4.9 (3.6…7.9) | 4.7 (3.1…6.7) | -0.4 |
| ancient: on plates without own mature trench share >= 10% (%) | 76.1 (57.3…93.6) | 80.5 (62.4…99.7) | +0.7 |
| ancient: on plates without any own trench cell (%) | 0.4 (0.0…2.9) | 0.7 (0.0…2.1) | +0.7 |
| ancient: median distance to an own-plate trench (km; cells that have one) | 2968.1 (2294.0…3959.0) | 2961.2 (1905.5…6012.5) | -0.0 |
| ancient: farther than 1000 km from an own-plate trench or none (%) | 81.7 (75.6…89.8) | 80.6 (71.3…93.3) | -0.4 |
| ancient: within 300 km of continental crust of its plate (%) | 84.4 (74.6…90.7) | 86.9 (82.0…91.6) | +1.0 |
| ancient: shallower than 2000 m (filled; %) | 59.8 (49.7…72.6) | 63.7 (48.7…71.7) | +1.1 |
| ancient: mean sediment (km) | 8.9 (7.6…9.6) | 9.2 (8.5…9.8) | +1.3 |
| ancient: within 550 km of a trench of another plate (active margin of its own plate; %) | 11.2 (6.4…14.9) | 10.9 (7.4…19.2) | -0.2 |
| ancient: within 550 km of a trench of its own plate (%) | 9.6 (5.2…12.2) | 10.9 (2.9…16.7) | +0.8 |
| ancient: farther than 550 km from any trench (%) | 79.3 (74.3…83.5) | 78.2 (73.2…81.1) | -0.8 |
| ancient: age p50 (Myr) | 331.0 (290.6…400.3) | 359.3 (328.2…383.4) | +2.1 |
| ancient: age p90 (Myr) | 653.6 (527.0…758.9) | 700.6 (628.1…785.5) | +1.4 |
| ancient older than 400 Myr (% of oceanic) | 1.7 (1.0…2.4) | 1.8 (1.5…2.5) | +0.9 |
| ancient: at a plate boundary (%) | 3.0 (1.6…4.5) | 3.7 (1.4…6.1) | +1.1 |
| all oceanic: farther than 1000 km from an own-plate trench or none (%) | 53.3 (47.2…58.6) | 51.8 (47.9…60.8) | -0.8 |
| all oceanic: on plates without own mature trench share >= 10% (%) | 27.6 (18.3…34.1) | 26.7 (22.6…33.4) | -0.4 |

## Results evaluation ([`review-results.md`](review-results.md))

An independent agent reproduced the headline numbers of the second version with its own
parsers and checked the code against the plan review. What it found and what was done:
1. The speed between slab-free plates in the measurement was wrong (a bug in the analysis
   script) → corrected there; the per-class numbers are in the table above.
2. The first draft of this file left out the drop in young-crust subduction → added.
3. The explanation "the resistance slows incipient trenches but does not stop them" was not
   supported → replaced by what the data show.
4. **Known gap, not fixed:** slab growth and the trench classification use each cell's raw
   normal, the force balance the smoothed one. 35% of the incipient trench length is locked in
   the force balance and still grows a slab. `main` has the same inconsistency. Fixing it
   changes the dynamics and needs its own validation.
5. Non-convergence of the resistance iteration was only a debug message → the log now counts
   "unsettled velocity solves". With the 20 iterations of the second version a fifth of the
   solves did not settle (61 in 300 Myr on seed 0; none on `main`). The limit is now 100, and
   no solve of the 8 final runs is unsettled. **This changed the dynamics slightly, so all 8
   seeds were run again; the final numbers were not evaluated independently.**
6. The buoyancy test could have passed without the buoyant term → it now compares against a
   slab that is exactly neutral.
7. An ocean–continent contact without a segment carries half the resistance (0.7% of the
   boundary). Left as it is.
8. The runtime guard rail was not measured → paired timings above.

## For the issues

- **#42:** the missing resistance is implemented. Measured consequence: small. Recommend
  closing it with this PR, with #32/#43 carrying the open question.
- **#32, #43:** not solved. The measurement narrows them: plates without slabs rarely border
  each other (9% of the boundary against Earth's 48%), and more plates carry slabs; continent
  arrangement, initiation at existing boundaries and plume rifts in oceanic plates are ruled
  out as causes (3 seeds each). Untested candidates: the coupling of continental plates to the
  mantle flow, and how subduction initiation and its spreading divide plates.
- **#14:** the ancient sea floor is passive-margin basement under ~9 km of sediment, far from
  any trench. Whether that is a defect is a judgement: Earth has such crust, less of it and
  younger. No change in this batch.
- **#6:** created production exceeds the kinematic one by 12% on both; about 60% of that is
  real area, the rest boundary churn. No change in this batch.
- **#37:** unchanged (−1.9 to −2.0 km), as the subduction flux is unchanged.

## Full comparison (8 seeds)

window 500-1000 Myr; seeds: main: 8, final: 8
| metric | main | final | t |
|---|---|---|---|
| plates | 24.07 (19.40…29.60) | 24.07 (17.80…28.20) | +0.0 |
| plates > 1% | 13.43 (10.00…16.60) | 14.12 (9.80…17.40) | +0.6 |
| mean speed (cm/yr) | 3.75 (3.48…4.20) | 3.84 (3.56…4.20) | +0.8 |
| speed 99% (cm/yr) | 9.19 (8.36…10.00) | 9.38 (8.62…10.86) | +0.7 |
| trenches (10^3 km) | 123 (110…138) | 122 (107…135) | -0.2 |
| ridges (10^3 km) | 95.17 (81.14…107.13) | 95.84 (77.28…106.38) | +0.2 |
| collisions (10^3 km) | 11.43 (4.80…21.68) | 13.37 (1.26…20.79) | +0.6 |
| incipient trenches (10^3 km) | — | 10.52 (5.34…14.31) | — |
| convergence at incipient trenches (cm/yr) | — | 1.90 (1.68…2.26) | — |
| convergence at other trenches (cm/yr) | — | 4.71 (4.52…4.88) | — |
| slab pull (1e21 N) | 1.39 (1.29…1.48) | 1.29 (1.20…1.38) | -3.1 |
| relative speed median (cm/yr) | 6.90 (6.44…7.52) | 6.58 (6.08…6.92) | -1.9 |
| boundary slower than 2 cm/yr (%) | 5.58 (3.20…9.20) | 7.67 (3.80…11.00) | +1.9 |
| opening/closing (%) | 61.60 (59.60…64.60) | 60.52 (56.80…66.00) | -1.0 |
| strike-slip (%) | 13.95 (12.80…14.60) | 14.75 (11.40…17.20) | +1.2 |
| sea-floor production kinematic (km2/yr) | 5.90 (5.29…6.49) | 5.68 (5.10…6.21) | -1.3 |
| sea-floor production created (km2/yr) | 6.51 (5.69…7.12) | 6.42 (5.84…6.88) | -0.5 |
| mean sea-floor age (Myr) | 55.12 (47.04…66.54) | 53.72 (46.26…58.44) | -0.5 |
| ocean age median (Myr) | 33.85 (31.20…41.00) | 32.88 (29.40…36.00) | -0.7 |
| sea floor >= 200 Myr (%) | 4.04 (2.01…7.06) | 3.46 (2.17…4.70) | -0.9 |
| sea floor < 20 Myr (% of ocean) | 34.50 (31.80…36.40) | 34.08 (31.00…36.80) | -0.5 |
| sea floor 20-40 Myr (%) | 22.70 (21.00…23.80) | 23.05 (21.60…25.20) | +0.6 |
| ocean deeper than 6 km (%) | 3.54 (2.26…4.82) | 3.86 (2.90…5.24) | +0.8 |
| continental (% of surface) | 34.73 (31.38…36.82) | 34.95 (34.10…36.80) | +0.3 |
| land (% of surface) | 28.42 (26.38…32.70) | 28.03 (25.30…29.72) | -0.4 |
| mean land (m) | 527 (384…693) | 552 (411…692) | +0.5 |
| highest point (m) | 4458 (3968…5218) | 4534 (3098…5080) | +0.3 |
| flooded continental crust (%) | 32.17 (21.40…39.20) | 33.95 (28.40…42.20) | +0.7 |
| crust > 50 km (% of continental) | 1.57 (0.80…2.60) | 2.05 (0.40…3.80) | +1.0 |
| sediment part of sea level (m) | 277 (241…331) | 268 (245…311) | -0.8 |
| land lower than 10 m (% of land) | 3.42 (1.90…4.48) | 3.42 (2.34…4.48) | -0.0 |
| shelf area (% of surface) | 7.23 (4.10…8.48) | 7.04 (5.28…9.22) | -0.3 |
| shelf mean sediment (km) | 3.92 (3.32…4.60) | 3.59 (3.17…3.97) | -1.7 |
| dynamic topography min (m) | -1932 (-2247…-1704) | -2030 (-2521…-1666) | -0.8 |
| dynamic topography max (m) | 1116 (933…1243) | 1161 (1058…1291) | +0.9 |
| sea-level budget mantle (m/100 Myr) | -53.54 (-92.60…-29.00) | -52.89 (-76.18…-32.76) | +0.1 |
| felsic+sediment net (Gt/yr) | -0.88 (-2.30…0.88) | -0.89 (-1.74…0.64) | -0.0 |
| trench sediment (Gt/yr) | 4.51 (2.62…6.20) | 4.22 (2.68…5.56) | -0.5 |
| collision stacking (Gt/yr) | 13.31 (6.56…22.30) | 14.99 (5.40…22.14) | +0.6 |
| burial metamorphism on continental crust (Gt/yr) | 0.13 (0.08…0.19) | 0.10 (0.06…0.18) | -1.4 |
| burial metamorphism on other crust (Gt/yr) | 0.73 (0.42…1.11) | 0.70 (0.47…0.96) | -0.3 |
| sediment trapped in lakes (km3/yr) | 0.15 (0.09…0.21) | 0.17 (0.08…0.30) | +0.7 |
| depressions (lakes) | 240 (186…316) | 229 (176…304) | -0.5 |
| land precipitation (m/yr) | 0.78 (0.58…0.88) | 0.78 (0.66…0.91) | +0.1 |
| runoff (km3/yr) | 37846 (33008…44020) | 37131 (32737…44984) | -0.3 |
| largest river (m3/s) | 75791 (41592…115573) | 83802 (66022…119814) | +0.8 |
| sediment to ocean (km3/yr) | 3.71 (2.88…4.48) | 3.82 (2.84…4.92) | +0.3 |
| endorheic (% of land) | 11.17 (7.50…20.22) | 11.18 (5.74…20.06) | +0.0 |
| endorheic into single-cell pits (% of land) | 6.08 (4.08…10.86) | 5.42 (3.32…9.34) | -0.7 |
| lakes (% of land) | 2.29 (1.86…2.58) | 2.45 (1.50…4.04) | +0.6 |
| lake evaporation (km3/yr) | 2274 (1596…3192) | 2460 (1594…3970) | +0.5 |
| global temperature (C) | 14.73 (13.20…15.42) | 14.53 (13.64…15.50) | -0.5 |
| equator (C) | 26.61 (25.12…27.50) | 26.44 (25.56…27.30) | -0.5 |
| poles (C) | -13.07 (-14.10…-11.92) | -13.03 (-13.96…-11.50) | +0.1 |
| permanently frozen (%) | 10.61 (9.12…12.36) | 11.36 (10.06…12.92) | +1.3 |
| ocean heat transport 20N (PW) | 1.24 (1.04…1.36) | 1.25 (1.04…1.36) | +0.3 |
| gyres (Sv) | 11.65 (10.20…14.20) | 11.53 (9.80…13.60) | -0.2 |
| rifts /100 Myr | 3.75 (3.20…4.40) | 4.50 (3.00…5.60) | +2.1 |
| initiations /100 Myr | 7.78 (4.20…10.80) | 8.15 (4.80…10.20) | +0.3 |
| sutures /100 Myr | 2.70 (1.00…4.20) | 3.25 (1.20…4.60) | +1.0 |
| plates born /100 Myr | 12.30 (8.00…16.20) | 13.30 (8.40…15.00) | +0.8 |
| segments created /100 Myr | 503 (425…575) | 489 (385…533) | -0.6 |
| terminal fans and deltas (km cells /100 Myr) | 5747 (2250…10461) | 5194 (3177…8699) | -0.5 |
| overfill (km cells /100 Myr) | 0.00 (0.00…0.00) | 0.00 (0.00…0.00) | — |
| sediment-part slope (m/100 Myr) | 8.39 (-1.55…19.92) | 12.05 (4.05…20.88) | +0.9 |
| sediment part at 1000 Myr (m) | 277 (258…326) | 286 (266…339) | +0.7 |
| highest point, max over all snapshots (m) | 5852 (5078…6239) | 5834 (5508…6332) | -0.1 |
| runtime (s) | 689 (563…808) | 570 (506…588) | -3.7 |
