# Batch 1 results: sediment routing artefacts (#47, #46)

_2026-11-20. Plan: [`plan.md`](plan.md), plan review: [`review-plan.md`](review-plan.md).
`main` at 3527833 against the branch, 8 seeds (0–7) × 1000 Myr, default parameters,
`--headless --material-budget --cell-dump`. Values are the mean over seeds of the mean over the
snapshots of 500–1000 Myr, with the seed range in brackets; t is the Welch t against `main`
(n = 8; |t| ≳ 2.4 ≈ p < 0.05). Attribution on seeds 0–3._

## What changed

1. **Full deep-sea fans aggrade to a graded surface** (`fill_fan`, `erosion.graded_fans`,
   `erosion.fan_relief`). After the capped lobes, the cells on the same turbidity paths fill up
   to a concave surface falling from the slope depth, without a thickness cap. Slope and rise
   cells take part.
2. **Sediment the sea refuses builds a terminal fan or prograding delta**
   (`build_terminal_fan`, `erosion.terminal_fans`, `erosion.alluvial_slope`): a cone around the
   river mouth over land and shelf, with a surface slope of 2 × 10⁻⁴. Nothing is piled on one
   cell any more.
3. New log field "terminal fans and deltas" next to the overfill.

**Dropped: the depth threshold for basins (#46).** The plan made depressions shallower than
~20 m spill. Run on seeds 0–3 to 400 Myr (with change 1 and 2): endorheic land fell from 11.0%
to 3.0% (t −2.5), lakes from 2.8% to 0.9% of land, lake evaporation from 3200 to 700 km³/yr.
That is further from an Earth-like planet (endorheic ~18%, lakes ~2–4% of unglaciated land),
not closer. The premise was wrong: a shallow pit in an arid region is a real terminal basin (a
playa), whatever made the low; whether it spills is decided by its water balance, which #44
already computes. So single-cell pits are how the mesh resolves arid interior drainage, and
their share (5.5% of land) is not an artefact. The code was removed again.

## Targets

| Metric | main | branch | target | |
|---|---|---|---|---|
| Overfill (km·cells per 100 Myr) | 4373 (3168…6007) | **0** in every seed | ≤ 100 | ✓ |
| Spikes > 2 km above all neighbours on sediment, in 48 snapshots | 4 (all on the 12.9 km column, up to 5.2 km) | **0** | 0 | ✓ |
| Land cells > 500 m above all neighbours (% of land) | 0.14 | 0.09 | | |
| Land draining into single-cell pits (% of land) | 5.5 | 5.5 | ≤ 1.5 | dropped, see above |
| Runtime (s, 4 runs at once) | 592 | 529 | ≤ +10% | ✓ |

Cells more than 2 km above all neighbours still occur without sediment: 8 in 48 snapshots on
the branch, all coastal cells of 43–67 km thick crystalline crust with the sea around them
(single-cell mountain islands). They are tectonic, not sediment, and are not #47.

## Attribution (seeds 0–3)

`nograded`: `erosion.graded_fans=0`; `noterminal`: `erosion.terminal_fans=0`.

window 500-1000 Myr; seeds: main: 4, b1: 4, nograded: 4, noterminal: 4
| metric | main | b1 | t | nograded | t | noterminal | t |
|---|---|---|---|---|---|---|---|
| land (% of surface) | 29.45 (28.10…30.84) | 26.57 (23.48…29.38) | -2.1 | 27.53 (25.48…29.52) | -1.6 | 28.73 (28.30…29.74) | -1.0 |
| mean land (m) | 609 (552…713) | 517 (341…615) | -1.2 | 540 (451…638) | -1.2 | 621 (501…733) | +0.2 |
| highest point (m) | 4647 (4566…4699) | 4550 (3717…5055) | -0.3 | 4590 (4226…5059) | -0.3 | 4904 (4501…5490) | +1.2 |
| flooded continental crust (%) | 31.65 (27.00…35.60) | 37.90 (30.60…46.80) | +1.6 | 36.20 (31.40…41.80) | +1.4 | 32.55 (28.20…35.40) | +0.4 |
| sediment part of sea level (m) | 271 (252…292) | 256 (208…292) | -0.8 | 260 (244…283) | -0.9 | 253 (230…275) | -1.4 |
| land lower than 10 m (% of land) | 2.32 (1.92…2.62) | 3.51 (2.60…4.62) | +2.7 | 3.35 (2.10…4.16) | +2.1 | 2.31 (1.84…2.82) | -0.1 |
| shelf area (% of surface) | 6.36 (5.54…7.22) | 7.79 (6.86…8.82) | +2.5 | 7.37 (5.52…8.30) | +1.4 | 6.34 (5.80…7.50) | -0.0 |
| trench sediment (Gt/yr) | 3.83 (3.36…4.84) | 3.88 (2.00…5.04) | +0.1 | 3.98 (2.98…4.96) | +0.3 | 4.42 (3.54…5.26) | +1.2 |
| burial metamorphism on continental crust (Gt/yr) | 0.19 (0.14…0.26) | 0.12 (0.08…0.20) | -2.0 | 0.13 (0.08…0.16) | -2.1 | 0.20 (0.10…0.29) | +0.2 |
| burial metamorphism on other crust (Gt/yr) | 0.95 (0.87…1.03) | 0.69 (0.34…1.08) | -1.6 | 0.81 (0.53…1.33) | -0.7 | 0.79 (0.64…0.90) | -2.6 |
| sediment trapped in lakes (km3/yr) | 0.20 (0.15…0.22) | 0.13 (0.05…0.23) | -1.5 | 0.16 (0.08…0.26) | -0.9 | 0.23 (0.17…0.27) | +1.3 |
| sediment to ocean (km3/yr) | 4.02 (3.68…4.66) | 3.46 (2.02…4.32) | -1.0 | 3.90 (2.96…5.10) | -0.2 | 4.01 (3.38…4.92) | -0.0 |
| endorheic (% of land) | 10.93 (8.28…14.94) | 8.71 (7.12…11.10) | -1.3 | 9.74 (7.00…11.28) | -0.7 | 12.80 (10.96…16.06) | +1.0 |
| lakes (% of land) | 2.83 (2.42…3.44) | 1.90 (1.40…2.60) | -2.6 | 2.54 (1.90…3.48) | -0.7 | 3.17 (2.10…4.12) | +0.6 |
| terminal fans and deltas (km cells /100 Myr) | — | 3562 (1304…6174) | — | 5697 (3467…9585) | — | 0.00 (0.00…0.00) | — |
| overfill (km cells /100 Myr) | 4775 (3568…6007) | 0.00 (0.00…0.00) | -9.2 | 0.00 (0.00…0.00) | -9.2 | 3906 (3219…5272) | -1.2 |
| sediment-part slope (m/100 Myr) | 10.02 (2.16…15.82) | 5.72 (-1.53…17.96) | -0.8 | 11.07 (0.37…19.19) | +0.2 | 10.29 (-2.97…21.63) | +0.0 |
| sediment part at 1000 Myr (m) | 293 (271…324) | 264 (210…296) | -1.2 | 287 (234…344) | -0.2 | 264 (220…305) | -1.2 |
| runtime (s) | 577 (560…590) | 522 (470…552) | -2.7 | 554 (535…573) | -1.8 | 557 (546…568) | -2.4 |

- **The graded fans take the smaller share.** With them alone the overfill falls by 18%
  (4775 → 3906); switching them off raises the terminal fans from 3562 to 5697 km·cells per
  100 Myr. Most of the refused sediment is at mouths without a way into deep water: 39–47% on
  water bodies of 1–3 cells (depressions below sea level inside continents, which the model
  treats as sea), most of the rest on full shelf seas.
- **The side effects come with the terminal fans** (present in `nograded`, absent in
  `noterminal`): less land, more flooded continental crust, more land below 10 m.

## Side effects and guard rails

- **Land 29.2 → 27.5% of the surface (t −1.8), flooded continental crust 32.3 → 36.2%
  (t +1.6), mean land 573 → 509 m (t −1.2).** Not significant one by one, but consistent. On
  `main` the piles became crust (burial metamorphism on continental crust 0.20 → 0.13 Gt/yr,
  t −4.0) and stood high. Now the same sediment lies as low cones and deltas.
- **Land below 10 m: 2.3 → 3.3% of land (t +3.3); shelf area 6.3 → 7.5% of the surface
  (t +2.9).** Deltas and the feet of the cones. Earth's land below 10 m is ~2%; #22's target
  was ≲ 3%.
- **Lakes 2.9 → 2.0% of land (t −3.7)**, lake evaporation 2900 → 2030 km³/yr: cones fill
  depressions next to the mouths. Endorheic land unchanged (11.3 → 10.2%).
- **Sediment part of the sea level: unchanged.** Mean 269 → 269 m; at 1000 Myr 284 → 291 m
  (t +0.4); slope over 500–1000 Myr 8.9 → 10.0 m/100 Myr (t +0.3; seed range −1.5…22.6). The
  expected rise of 10–20% did not happen: the sediment stays on land and shelf.
- **Ancient ocean basins (#14):** share of the oceanic sediment on crust ≥ 200 Myr 44.6 → 49.9%,
  mean thickness there 8.5 → 9.2 km. A small shift towards the trapped basins, as the review
  warned (finding 7). The thickest oceanic column is at the 12.9 km cap on both.
- Tectonics: no metric differs significantly except rifts per 100 Myr (4.5 → 3.3, t −2.5) and
  the mantle sea-level term (−74 → −55 m/100 Myr, t +3.2). With ~60 metrics compared, one or
  two at this level are expected by chance; the erosion change has no direct path to rifting.

**For #45:** the slope target (≤ 10 m/100 Myr) is met on `main` (8.9) and at the limit on the
branch (10.0), both with a seed scatter of ±7. The level at 1000 Myr did not change. The
"regression" of #44 was a slower fill to the same level. Recommendation: close #45.

**For #46:** recommend closing. Endorheic 10–11% and lakes 2–3% of land are plausible for an
Earth-like planet, and the single-cell pits are not an artefact (see above).

## Full comparison (8 seeds)

window 500-1000 Myr; seeds: main: 8, b1: 8
| metric | main | b1 | t |
|---|---|---|---|
| plates | 25.23 (18.80…29.60) | 22.60 (14.80…29.00) | -1.2 |
| plates > 1% | 14.90 (10.20…18.60) | 13.05 (9.40…16.00) | -1.5 |
| mean speed (cm/yr) | 3.86 (3.62…4.04) | 3.85 (3.52…4.20) | -0.2 |
| speed 99% (cm/yr) | 9.47 (8.90…10.18) | 9.32 (8.74…10.30) | -0.6 |
| trenches (10^3 km) | 125 (100…140) | 122 (102…139) | -0.5 |
| ridges (10^3 km) | 99.15 (79.41…109.38) | 90.83 (72.62…105.30) | -1.5 |
| collisions (10^3 km) | 14.70 (7.46…20.20) | 12.02 (5.65…18.46) | -1.2 |
| slab pull (1e21 N) | 1.39 (1.27…1.51) | 1.40 (1.29…1.51) | +0.4 |
| relative speed median (cm/yr) | 6.77 (6.38…7.22) | 6.90 (6.62…7.26) | +0.9 |
| boundary slower than 2 cm/yr (%) | 6.08 (4.40…8.60) | 5.75 (4.00…8.40) | -0.4 |
| opening/closing (%) | 61.35 (58.40…70.60) | 61.95 (58.20…65.20) | +0.4 |
| strike-slip (%) | 13.88 (8.60…15.60) | 13.53 (11.40…15.80) | -0.4 |
| sea-floor production kinematic (km2/yr) | 5.97 (5.18…6.51) | 5.80 (5.13…6.42) | -0.8 |
| sea-floor production created (km2/yr) | 6.69 (5.69…7.25) | 6.34 (5.52…7.17) | -1.3 |
| mean sea-floor age (Myr) | 52.31 (47.58…55.58) | 53.87 (49.82…58.14) | +1.2 |
| ocean age median (Myr) | 32.10 (28.80…34.80) | 33.05 (30.00…35.00) | +1.0 |
| sea floor >= 200 Myr (%) | 3.32 (2.26…4.46) | 3.59 (2.48…4.96) | +0.8 |
| sea floor < 20 Myr (% of ocean) | 35.55 (31.60…38.20) | 34.12 (31.00…38.00) | -1.3 |
| sea floor 20-40 Myr (%) | 22.62 (21.40…24.20) | 23.05 (21.00…24.60) | +0.8 |
| ocean deeper than 6 km (%) | 3.92 (2.86…6.42) | 3.92 (2.42…4.86) | -0.0 |
| continental (% of surface) | 35.27 (33.76…38.18) | 35.63 (33.42…38.76) | +0.5 |
| land (% of surface) | 29.22 (27.76…30.84) | 27.48 (23.48…30.82) | -1.8 |
| mean land (m) | 573 (397…713) | 509 (341…657) | -1.2 |
| highest point (m) | 4605 (3514…5288) | 4461 (3717…5090) | -0.5 |
| flooded continental crust (%) | 32.30 (27.00…35.60) | 36.20 (27.80…46.80) | +1.6 |
| crust > 50 km (% of continental) | 1.65 (0.40…3.20) | 1.57 (0.20…3.00) | -0.1 |
| sediment part of sea level (m) | 269 (252…292) | 269 (208…309) | -0.0 |
| land lower than 10 m (% of land) | 2.31 (1.92…2.74) | 3.29 (2.22…4.62) | +3.3 |
| shelf area (% of surface) | 6.28 (5.54…7.22) | 7.52 (6.00…8.82) | +2.9 |
| shelf mean sediment (km) | 3.61 (3.14…4.06) | 3.51 (2.72…3.99) | -0.6 |
| dynamic topography min (m) | -1928 (-2475…-1590) | -1906 (-2142…-1701) | +0.2 |
| dynamic topography max (m) | 1117 (971…1287) | 1130 (1054…1315) | +0.3 |
| sea-level budget mantle (m/100 Myr) | -73.61 (-84.48…-58.50) | -54.52 (-77.44…-34.72) | +3.2 |
| felsic+sediment net (Gt/yr) | -0.36 (-1.36…0.56) | -0.18 (-1.62…1.02) | +0.5 |
| trench sediment (Gt/yr) | 3.67 (2.66…4.84) | 3.67 (2.00…5.04) | +0.0 |
| collision stacking (Gt/yr) | 13.61 (5.48…17.42) | 12.40 (4.32…18.58) | -0.5 |
| burial metamorphism on continental crust (Gt/yr) | 0.20 (0.14…0.26) | 0.13 (0.08…0.20) | -4.0 |
| burial metamorphism on other crust (Gt/yr) | 0.84 (0.60…1.03) | 0.73 (0.34…1.08) | -1.0 |
| sediment trapped in lakes (km3/yr) | 0.18 (0.11…0.25) | 0.12 (0.05…0.23) | -2.2 |
| depressions (lakes) | 243 (196…285) | 220 (139…285) | -1.1 |
| land precipitation (m/yr) | 0.75 (0.66…0.80) | 0.77 (0.59…0.89) | +0.5 |
| runoff (km3/yr) | 37658 (32653…45060) | 35594 (28518…39367) | -1.1 |
| largest river (m3/s) | 79344 (52712…132353) | 76487 (58772…97993) | -0.3 |
| sediment to ocean (km3/yr) | 3.75 (2.56…4.66) | 3.44 (2.02…4.32) | -1.0 |
| endorheic (% of land) | 11.29 (8.28…15.82) | 10.24 (7.12…18.38) | -0.7 |
| endorheic into single-cell pits (% of land) | 5.51 (4.20…7.50) | 5.53 (3.90…9.32) | +0.0 |
| lakes (% of land) | 2.91 (2.42…4.00) | 2.02 (1.40…2.60) | -3.7 |
| lake evaporation (km3/yr) | 2904 (1850…3619) | 2028 (1258…2736) | -2.9 |
| global temperature (C) | 14.21 (13.82…14.60) | 14.75 (13.26…15.60) | +1.8 |
| equator (C) | 25.94 (25.42…26.36) | 26.34 (24.26…27.18) | +1.1 |
| poles (C) | -13.42 (-14.42…-12.46) | -12.89 (-13.24…-12.62) | +2.0 |
| permanently frozen (%) | 11.18 (9.58…11.96) | 10.60 (8.98…12.24) | -1.2 |
| ocean heat transport 20N (PW) | 1.21 (0.98…1.32) | 1.24 (1.02…1.34) | +0.4 |
| gyres (Sv) | 12.65 (10.80…14.40) | 12.40 (10.60…14.80) | -0.3 |
| rifts /100 Myr | 4.45 (3.00…5.40) | 3.25 (1.60…5.40) | -2.5 |
| initiations /100 Myr | 8.57 (5.20…12.00) | 7.22 (4.60…10.20) | -1.3 |
| sutures /100 Myr | 3.30 (1.40…4.20) | 2.65 (1.20…3.60) | -1.4 |
| plates born /100 Myr | 14.07 (8.40…17.40) | 11.22 (7.00…14.20) | -2.0 |
| segments created /100 Myr | 515 (412…575) | 472 (375…544) | -1.4 |
| terminal fans and deltas (km cells /100 Myr) | — | 3981 (1304…6174) | — |
| overfill (km cells /100 Myr) | 4373 (3168…6007) | 0.00 (0.00…0.00) | -13.5 |
| sediment-part slope (m/100 Myr) | 8.85 (2.16…15.82) | 10.02 (-1.53…22.55) | +0.3 |
| sediment part at 1000 Myr (m) | 284 (262…324) | 291 (210…364) | +0.4 |
| highest point, max over all snapshots (m) | 5614 (5210…5960) | 5692 (5330…5942) | +0.6 |
| runtime (s) | 592 (560…633) | 529 (470…570) | -4.4 |

## Known limits

- A cone's sediment is eroded like any sediment. In a filled inland basin the river carries it
  on once the drainage finds the next sea; meanwhile the same sediment can pass a mouth more
  than once, so "terminal fans and deltas" (3981 km·cells per 100 Myr) counts some of it
  repeatedly. Sediment to the ocean did not rise (3.75 → 3.44 km³/yr, t −1.0).
- Depressions below sea level inside continents are still sea in the model, whatever their
  connection to the ocean. They should be lakes with a water balance (Caspian or Dead Sea
  type). That would remove most of the need for terminal fans at 1–3-cell seas.
- A crust sample shown by two cells receives both cells' deposits (review finding 4). No spike
  on the branch sits on such a sample.
