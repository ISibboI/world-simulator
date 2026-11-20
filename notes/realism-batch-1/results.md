# Batch 1 results: sediment routing artefacts (#47, #46)

_2026-11-20. Plan: [`plan.md`](plan.md), plan review: [`review-plan.md`](review-plan.md),
results evaluation: [`review-results.md`](review-results.md). `main` at 3527833 against the
branch, 8 seeds (0–7) × 1000 Myr, default parameters, `--headless --material-budget
--cell-dump`. Values are the mean over seeds, with the seed range in brackets; rates and levels
are means over the snapshots of 600–1000 Myr, counters and slopes are over 500–1000 Myr. t is
the Welch t against `main` (n = 8; |t| ≳ 2.4 ≈ p < 0.05)._

Three run sets appear below: `main`, `b1` (the first version of the branch, which the results
evaluation examined) and `final` (the branch as it is, after the evaluation's findings).

## What changed

1. **Full deep-sea fans aggrade to a graded surface** (`fill_fan`, `erosion.graded_fans`,
   `erosion.fan_relief`). After the capped lobes, the cells on the same turbidity paths fill up
   to a concave surface falling from the slope depth, without a thickness cap. Slope and rise
   cells take part.
2. **Sediment the sea refuses builds a terminal fan or prograding delta**
   (`build_terminal_fan`, `erosion.terminal_fans`, `erosion.alluvial_slope`): a cone around the
   river mouth with a surface slope of 2 × 10⁻⁴. It spreads only through cells it covers. Where
   it meets the sea, the sea becomes land. Nothing is piled on one cell any more.
3. New log field "terminal fans and deltas" next to the overfill.

Changes after the results evaluation (`b1` → `final`):
- In `b1` the cone covered every cell below its surface within reach, also behind higher
  ground, and it did not enter deep water. A mouth with only deep water around then got the
  whole load on its own cell: `b1` had 7 single-cell islands more than 2 km high in 88
  snapshots (`main` 0), with 43–67 km of crust made from the sediment. Now the cone spreads
  cell by cell through what it covers, into deep water too.
- The bisection leaves no remainder; rounding dust stays at the apex.

**Dropped: the depth threshold for basins (#46).** The plan made depressions shallower than
~20 m spill. Run on seeds 0–3 to 400 Myr: endorheic land fell from 11.0% to 3.0% (t −2.5),
lakes from 2.8% to 0.9% of land, lake evaporation from 3200 to 700 km³/yr, because the
threshold removes every shallow depression, not only single-cell pits. That is further from an
Earth-like planet (endorheic ~18%), so the code was removed again.
**What the pits are** (results evaluation): 57–59% of the single-cell pits sit on crust samples
shown by two cells, against 15–16% of all land cells, and pits are not drier than other land.
`apply_surface_changes` gives such a sample both cells' erosion. So most pits are an artefact
of the tectonic bookkeeping, not of the erosion: new issue #57. #46 stays open on that.

## Targets

| Metric | main | final | target | |
|---|---|---|---|---|
| Overfill (km·cells per 100 Myr) | 4373 (3168…6007) | **0** in every seed | ≤ 100 | ✓ |
| Cells > 2 km above all neighbours on > 10 km of sediment, in 88 snapshots (0–1000 Myr) | 8 | **0** | 0 | ✓ |
| Other cells > 2 km above all neighbours, in 88 snapshots | 2 | 6 (2 of them single-cell islands) | | see below |
| Land cells > 500 m above all neighbours (% of land) | 0.14 | 0.10 | | |
| Land draining into single-cell pits (% of land) | 5.5 | 5.5 | ≤ 1.5 | not met: #57 |
| Runtime (s, 4 runs at once) | 592 | 576 (t −1.2) | ≤ +10% | ✓ |

The remaining cells more than 2 km above their neighbours carry no sediment: coastal cells of
45–51 km thick crust with the sea next to them. They are not #47. `final` has 6 against 2 on
`main`; with these counts that is not a significant difference, and the cause was not examined.

## Attribution (seeds 0–3, on `b1`)

`nograded`: `erosion.graded_fans=0`; `noterminal`: `erosion.terminal_fans=0`.

window 500-1000 Myr; seeds: main: 4, b1: 4, nograded: 4, noterminal: 4
| metric | main | b1 | t | nograded | t | noterminal | t |
|---|---|---|---|---|---|---|---|
| land (% of surface) | 29.45 (28.10…30.84) | 26.57 (23.48…29.38) | -2.1 | 27.53 (25.48…29.52) | -1.6 | 28.73 (28.30…29.74) | -1.0 |
| mean land (m) | 609 (552…713) | 517 (341…615) | -1.2 | 540 (451…638) | -1.2 | 621 (501…733) | +0.2 |
| flooded continental crust (%) | 31.65 (27.00…35.60) | 37.90 (30.60…46.80) | +1.6 | 36.20 (31.40…41.80) | +1.4 | 32.55 (28.20…35.40) | +0.4 |
| sediment part of sea level (m) | 271 (252…292) | 256 (208…292) | -0.8 | 260 (244…283) | -0.9 | 253 (230…275) | -1.4 |
| land lower than 10 m (% of land) | 2.32 (1.92…2.62) | 3.51 (2.60…4.62) | +2.7 | 3.35 (2.10…4.16) | +2.1 | 2.31 (1.84…2.82) | -0.1 |
| shelf area (% of surface) | 6.36 (5.54…7.22) | 7.79 (6.86…8.82) | +2.5 | 7.37 (5.52…8.30) | +1.4 | 6.34 (5.80…7.50) | -0.0 |
| burial metamorphism on continental crust (Gt/yr) | 0.19 (0.14…0.26) | 0.12 (0.08…0.20) | -2.0 | 0.13 (0.08…0.16) | -2.1 | 0.20 (0.10…0.29) | +0.2 |
| sediment trapped in lakes (km3/yr) | 0.20 (0.15…0.22) | 0.13 (0.05…0.23) | -1.5 | 0.16 (0.08…0.26) | -0.9 | 0.23 (0.17…0.27) | +1.3 |
| sediment to ocean (km3/yr) | 4.02 (3.68…4.66) | 3.46 (2.02…4.32) | -1.0 | 3.90 (2.96…5.10) | -0.2 | 4.01 (3.38…4.92) | -0.0 |
| endorheic (% of land) | 10.93 (8.28…14.94) | 8.71 (7.12…11.10) | -1.3 | 9.74 (7.00…11.28) | -0.7 | 12.80 (10.96…16.06) | +1.0 |
| lakes (% of land) | 2.83 (2.42…3.44) | 1.90 (1.40…2.60) | -2.6 | 2.54 (1.90…3.48) | -0.7 | 3.17 (2.10…4.12) | +0.6 |
| terminal fans and deltas (km cells /100 Myr) | — | 3562 (1304…6174) | — | 5697 (3467…9585) | — | 0.00 (0.00…0.00) | — |
| overfill (km cells /100 Myr) | 4775 (3568…6007) | 0.00 (0.00…0.00) | -9.2 | 0.00 (0.00…0.00) | -9.2 | 3906 (3219…5272) | -1.2 |
| sediment-part slope (m/100 Myr) | 10.02 (2.16…15.82) | 5.72 (-1.53…17.96) | -0.8 | 11.07 (0.37…19.19) | +0.2 | 10.29 (-2.97…21.63) | +0.0 |
| sediment part at 1000 Myr (m) | 293 (271…324) | 264 (210…296) | -1.2 | 287 (234…344) | -0.2 | 264 (220…305) | -1.2 |

- **The graded fans take the smaller share.** With them alone the overfill falls by 18%
  (4775 → 3906). Most of the refused sediment is at mouths without a way into deep water:
  39–47% on water bodies of 1–3 cells (depressions below sea level inside continents, which the
  model treats as sea; new issue #58), most of the rest on full shelf seas.
- **The side effects come with the terminal fans** (present in `nograded`, absent in
  `noterminal`).

## Side effects and guard rails (`main` → `final`)

Significant:
- **Land below 10 m: 2.3 → 3.4% of land (t +3.6).** Deltas and the feet of the cones. Earth's
  is ~2%; #22's target was ≲ 3%.
- **Lakes 2.9 → 2.3% of land (t −3.0).** Cones fill depressions next to the mouths.
  Endorheic land is unchanged (11.3 → 11.2%).
- **Burial metamorphism on continental crust 0.20 → 0.13 Gt/yr (t −3.9).** The piles used to
  be buried under themselves and turned into crust.
- **Rifts per 100 Myr 4.5 → 3.8 (t −2.4; `b1` 3.3, t −2.5).** Both versions show it, so it is
  probably real. No mechanism is known; the erosion has no direct path to rifting.
- **Mantle term of the sea-level budget −74 → −54 m per 100 Myr (t +2.7).** The term is the
  bookkeeping counterpart of the coastal term (see the triage on #36), so it moves with the
  routing.

Not significant, but worth knowing:
- **Shelf cells on oceanic crust: 2.0 → 4.9% of the shelf cells (seed range 2.6…11.2).** The
  cones prograde into deep water. Real deltas do that (Niger, Mississippi, Amazon), but this is
  the metric of the closed #8 (target 0).
- Land 29.2 → 28.4% of the surface (t −1.0), mean land 573 → 527 m (t −1.0), flooded
  continental crust 32.3 → 32.2%. In `b1` the shifts were larger (land 27.5%, flooded 36.2%)
  but not robust per seed (paired t −1.4, half of it from seed 0).
- Trench sediment 3.7 → 4.5 Gt/yr (t +1.8); felsic + sediment net −0.36 → −0.88 Gt/yr (t −1.3;
  inside 0…−1.5).
- **Sediment part of the sea level: unchanged.** Mean 269 → 277 m (t +0.7); at 1000 Myr
  284 → 277 m; slope over 500–1000 Myr 8.9 → 8.4 m/100 Myr (seed range −1.6…19.9). The rise of
  10–20% expected in the plan did not happen.
- Ancient ocean basins (#14): share of the oceanic sediment on crust ≥ 200 Myr 44.6 → 45.5%,
  mean thickness there 8.5 → 8.9 km; sea floor ≥ 200 Myr 3.3 → 4.0% (t +1.2).
- Hypsometry: highest cell 4.6 → 4.5 km, land p99 3.3 → 3.4 km, p90 1.47 → 1.51 km.

**For #45:** the level at 1000 Myr is unchanged from before #44, and the slope is 8–9 m per
100 Myr on `main` and on the branch, with a seed scatter of ±7 that is too wide to call the
≤ 10 target met or missed. It is an Earth-exact target on a quantity whose level is fine.
Recommendation: close #45.

**For #46:** keep open, on the cause in #57. The shares themselves (endorheic 11%, lakes 2–3%
of land) are plausible for an Earth-like planet.

## Results evaluation

An independent agent re-derived the numbers of the `b1` table with its own parser (all
reproduced), reviewed the code (mass conserved; marks reset; statistics correct) and compared
the cell dumps (no new sediment artefacts: thickness distribution on land unchanged, no step at
the cone's fall in neighbour height differences). Its findings and what was done:
1. The cone crossed divides → fixed (`final`).
2. Single-cell mountain islands were more frequent than on `main`, not "tectonic at the same
   rate" as the first draft of this file said → cause found (mouths surrounded by deep water),
   fixed; 7 → 2 in 88 snapshots.
3. #46 must not be closed as "pits are legitimate basins": most pits sit on shared samples →
   issue #57, #46 stays open.
4. "The piles became high-standing crust" explains the drop in metamorphism, but the land
   change does not follow it per seed → land and flooded crust are reported as not significant.
5. #45: close as Earth-exact, not as "target met" → adopted.
The `final` runs were not evaluated independently again; their numbers come from the same
scripts whose `b1` output the evaluator reproduced.

## Full comparison (8 seeds)

window 500-1000 Myr; seeds: main: 8, b1: 8, final: 8
| metric | main | b1 | t | final | t |
|---|---|---|---|---|---|
| plates | 25.23 (18.80…29.60) | 22.60 (14.80…29.00) | -1.2 | 24.07 (19.40…29.60) | -0.6 |
| plates > 1% | 14.90 (10.20…18.60) | 13.05 (9.40…16.00) | -1.5 | 13.43 (10.00…16.60) | -1.3 |
| mean speed (cm/yr) | 3.86 (3.62…4.04) | 3.85 (3.52…4.20) | -0.2 | 3.75 (3.48…4.20) | -1.2 |
| speed 99% (cm/yr) | 9.47 (8.90…10.18) | 9.32 (8.74…10.30) | -0.6 | 9.19 (8.36…10.00) | -1.2 |
| trenches (10^3 km) | 125 (100…140) | 122 (102…139) | -0.5 | 123 (110…138) | -0.4 |
| ridges (10^3 km) | 99.15 (79.41…109.38) | 90.83 (72.62…105.30) | -1.5 | 95.17 (81.14…107.13) | -0.8 |
| collisions (10^3 km) | 14.70 (7.46…20.20) | 12.02 (5.65…18.46) | -1.2 | 11.43 (4.80…21.68) | -1.4 |
| slab pull (1e21 N) | 1.39 (1.27…1.51) | 1.40 (1.29…1.51) | +0.4 | 1.39 (1.29…1.48) | +0.1 |
| relative speed median (cm/yr) | 6.77 (6.38…7.22) | 6.90 (6.62…7.26) | +0.9 | 6.90 (6.44…7.52) | +0.7 |
| boundary slower than 2 cm/yr (%) | 6.08 (4.40…8.60) | 5.75 (4.00…8.40) | -0.4 | 5.58 (3.20…9.20) | -0.6 |
| opening/closing (%) | 61.35 (58.40…70.60) | 61.95 (58.20…65.20) | +0.4 | 61.60 (59.60…64.60) | +0.2 |
| strike-slip (%) | 13.88 (8.60…15.60) | 13.53 (11.40…15.80) | -0.4 | 13.95 (12.80…14.60) | +0.1 |
| sea-floor production kinematic (km2/yr) | 5.97 (5.18…6.51) | 5.80 (5.13…6.42) | -0.8 | 5.90 (5.29…6.49) | -0.3 |
| sea-floor production created (km2/yr) | 6.69 (5.69…7.25) | 6.34 (5.52…7.17) | -1.3 | 6.51 (5.69…7.12) | -0.7 |
| mean sea-floor age (Myr) | 52.31 (47.58…55.58) | 53.87 (49.82…58.14) | +1.2 | 55.12 (47.04…66.54) | +1.2 |
| ocean age median (Myr) | 32.10 (28.80…34.80) | 33.05 (30.00…35.00) | +1.0 | 33.85 (31.20…41.00) | +1.3 |
| sea floor >= 200 Myr (%) | 3.32 (2.26…4.46) | 3.59 (2.48…4.96) | +0.8 | 4.04 (2.01…7.06) | +1.2 |
| sea floor < 20 Myr (% of ocean) | 35.55 (31.60…38.20) | 34.12 (31.00…38.00) | -1.3 | 34.50 (31.80…36.40) | -1.2 |
| sea floor 20-40 Myr (%) | 22.62 (21.40…24.20) | 23.05 (21.00…24.60) | +0.8 | 22.70 (21.00…23.80) | +0.1 |
| ocean deeper than 6 km (%) | 3.92 (2.86…6.42) | 3.92 (2.42…4.86) | -0.0 | 3.54 (2.26…4.82) | -0.8 |
| continental (% of surface) | 35.27 (33.76…38.18) | 35.63 (33.42…38.76) | +0.5 | 34.73 (31.38…36.82) | -0.7 |
| land (% of surface) | 29.22 (27.76…30.84) | 27.48 (23.48…30.82) | -1.8 | 28.42 (26.38…32.70) | -1.0 |
| mean land (m) | 573 (397…713) | 509 (341…657) | -1.2 | 527 (384…693) | -1.0 |
| highest point (m) | 4605 (3514…5288) | 4461 (3717…5090) | -0.5 | 4458 (3968…5218) | -0.6 |
| flooded continental crust (%) | 32.30 (27.00…35.60) | 36.20 (27.80…46.80) | +1.6 | 32.17 (21.40…39.20) | -0.1 |
| crust > 50 km (% of continental) | 1.65 (0.40…3.20) | 1.57 (0.20…3.00) | -0.1 | 1.57 (0.80…2.60) | -0.2 |
| sediment part of sea level (m) | 269 (252…292) | 269 (208…309) | -0.0 | 277 (241…331) | +0.7 |
| land lower than 10 m (% of land) | 2.31 (1.92…2.74) | 3.29 (2.22…4.62) | +3.3 | 3.42 (1.90…4.48) | +3.6 |
| shelf area (% of surface) | 6.28 (5.54…7.22) | 7.52 (6.00…8.82) | +2.9 | 7.23 (4.10…8.48) | +1.8 |
| shelf mean sediment (km) | 3.61 (3.14…4.06) | 3.51 (2.72…3.99) | -0.6 | 3.92 (3.32…4.60) | +1.5 |
| dynamic topography min (m) | -1928 (-2475…-1590) | -1906 (-2142…-1701) | +0.2 | -1932 (-2247…-1704) | -0.0 |
| dynamic topography max (m) | 1117 (971…1287) | 1130 (1054…1315) | +0.3 | 1116 (933…1243) | -0.0 |
| sea-level budget mantle (m/100 Myr) | -73.61 (-84.48…-58.50) | -54.52 (-77.44…-34.72) | +3.2 | -53.54 (-92.60…-29.00) | +2.7 |
| felsic+sediment net (Gt/yr) | -0.36 (-1.36…0.56) | -0.18 (-1.62…1.02) | +0.5 | -0.88 (-2.30…0.88) | -1.3 |
| trench sediment (Gt/yr) | 3.67 (2.66…4.84) | 3.67 (2.00…5.04) | +0.0 | 4.51 (2.62…6.20) | +1.8 |
| collision stacking (Gt/yr) | 13.61 (5.48…17.42) | 12.40 (4.32…18.58) | -0.5 | 13.31 (6.56…22.30) | -0.1 |
| burial metamorphism on continental crust (Gt/yr) | 0.20 (0.14…0.26) | 0.13 (0.08…0.20) | -4.0 | 0.13 (0.08…0.19) | -3.9 |
| burial metamorphism on other crust (Gt/yr) | 0.84 (0.60…1.03) | 0.73 (0.34…1.08) | -1.0 | 0.73 (0.42…1.11) | -0.9 |
| sediment trapped in lakes (km3/yr) | 0.18 (0.11…0.25) | 0.12 (0.05…0.23) | -2.2 | 0.15 (0.09…0.21) | -1.7 |
| depressions (lakes) | 243 (196…285) | 220 (139…285) | -1.1 | 240 (186…316) | -0.2 |
| land precipitation (m/yr) | 0.75 (0.66…0.80) | 0.77 (0.59…0.89) | +0.5 | 0.78 (0.58…0.88) | +0.7 |
| runoff (km3/yr) | 37658 (32653…45060) | 35594 (28518…39367) | -1.1 | 37846 (33008…44020) | +0.1 |
| largest river (m3/s) | 79344 (52712…132353) | 76487 (58772…97993) | -0.3 | 75791 (41592…115573) | -0.3 |
| sediment to ocean (km3/yr) | 3.75 (2.56…4.66) | 3.44 (2.02…4.32) | -1.0 | 3.71 (2.88…4.48) | -0.2 |
| endorheic (% of land) | 11.29 (8.28…15.82) | 10.24 (7.12…18.38) | -0.7 | 11.17 (7.50…20.22) | -0.1 |
| endorheic into single-cell pits (% of land) | 5.51 (4.20…7.50) | 5.53 (3.90…9.32) | +0.0 | 6.08 (4.08…10.86) | +0.7 |
| lakes (% of land) | 2.91 (2.42…4.00) | 2.02 (1.40…2.60) | -3.7 | 2.29 (1.86…2.58) | -3.0 |
| lake evaporation (km3/yr) | 2904 (1850…3619) | 2028 (1258…2736) | -2.9 | 2274 (1596…3192) | -2.0 |
| global temperature (C) | 14.21 (13.82…14.60) | 14.75 (13.26…15.60) | +1.8 | 14.73 (13.20…15.42) | +1.7 |
| equator (C) | 25.94 (25.42…26.36) | 26.34 (24.26…27.18) | +1.1 | 26.61 (25.12…27.50) | +2.1 |
| poles (C) | -13.42 (-14.42…-12.46) | -12.89 (-13.24…-12.62) | +2.0 | -13.07 (-14.10…-11.92) | +0.9 |
| permanently frozen (%) | 11.18 (9.58…11.96) | 10.60 (8.98…12.24) | -1.2 | 10.61 (9.12…12.36) | -1.1 |
| ocean heat transport 20N (PW) | 1.21 (0.98…1.32) | 1.24 (1.02…1.34) | +0.4 | 1.24 (1.04…1.36) | +0.5 |
| gyres (Sv) | 12.65 (10.80…14.40) | 12.40 (10.60…14.80) | -0.3 | 11.65 (10.20…14.20) | -1.4 |
| rifts /100 Myr | 4.45 (3.00…5.40) | 3.25 (1.60…5.40) | -2.5 | 3.75 (3.20…4.40) | -2.4 |
| initiations /100 Myr | 8.57 (5.20…12.00) | 7.22 (4.60…10.20) | -1.3 | 7.78 (4.20…10.80) | -0.7 |
| sutures /100 Myr | 3.30 (1.40…4.20) | 2.65 (1.20…3.60) | -1.4 | 2.70 (1.00…4.20) | -1.3 |
| plates born /100 Myr | 14.07 (8.40…17.40) | 11.22 (7.00…14.20) | -2.0 | 12.30 (8.00…16.20) | -1.3 |
| segments created /100 Myr | 515 (412…575) | 472 (375…544) | -1.4 | 503 (425…575) | -0.4 |
| terminal fans and deltas (km cells /100 Myr) | — | 3981 (1304…6174) | — | 5747 (2250…10461) | — |
| overfill (km cells /100 Myr) | 4373 (3168…6007) | 0.00 (0.00…0.00) | -13.5 | 0.00 (0.00…0.00) | -13.5 |
| sediment-part slope (m/100 Myr) | 8.85 (2.16…15.82) | 10.02 (-1.53…22.55) | +0.3 | 8.39 (-1.55…19.92) | -0.1 |
| sediment part at 1000 Myr (m) | 284 (262…324) | 291 (210…364) | +0.4 | 277 (258…326) | -0.6 |
| highest point, max over all snapshots (m) | 5614 (5210…5960) | 5692 (5330…5942) | +0.6 | 5852 (5078…6239) | +1.4 |
| runtime (s) | 592 (560…633) | 529 (470…570) | -4.4 | 576 (524…616) | -1.2 |

## Known limits

- A cone's sediment is eroded like any sediment. In a filled inland basin the river carries it
  on once the drainage finds the next sea; meanwhile the same sediment can pass a mouth more
  than once, so "terminal fans and deltas" (5747 km·cells per 100 Myr) counts some of it
  repeatedly. Sediment to the ocean did not rise (3.75 → 3.71 km³/yr).
- Depressions below sea level inside continents are still sea in the model (#58).
- A crust sample shown by two cells receives both cells' deposits and erosion (#57).
