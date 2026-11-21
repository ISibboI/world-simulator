# #33 results: low-latitude highlands follow the free air; the spin-up starts without ice

_2026-11-21. Plan: [`plan.md`](plan.md), plan review: [`review-plan.md`](review-plan.md).
`main` at 288f9cf against the branch, 8 seeds (0–7) × 1000 Myr, default parameters. `main` here
is the first build of the branch with `--set temperature.free_air_height=0`, which the plan
review confirmed to be bit-identical to `main`. Mean over seeds (seed range) of the snapshots
of 500–1000 Myr; Welch t, n = 8 (|t| ≳ 2.4 ≈ p < 0.05)._

## What changed (`src/temperature.rs`)

1. The spin-up of the temperature model starts without ice sheets instead of with a land cold
   enough to glaciate every plateau in its first year.
2. Land in low latitudes is at least as warm as the free air at its height, which the sea
   surface of its latitude band sets: the annual mean is raised towards it with elevation
   (full at 3 km), fully up to 30° of latitude and not at all beyond 45°. It is never lowered.

## The issue's metrics

| | main | branch | t |
|---|---|---|---|
| Tropical land (|lat| < 25°) with an annual mean below −10 °C (% of tropical land) | 1.21 (0.51…3.53) | **0.00** in every seed | −3.4 |
| … below −5 °C, where ice begins | 1.83 | 0.02 | −3.7 |
| Tropical land above 3 km, reduced to sea level (°C) | 16.7 (12.6…20.0) | 29.7 (28.3…31.1) | +13.1 |
| Land at 15–35° above 3.5 km: annual mean (°C) | −13.4 (−20.2…−6.3) | +2.0 (−0.3…6.5) | +9.0 |
| … share of it below −5 °C | 59% | 7% | −5.5 |

- The tropical highland ice is gone.
- **The plan's target "highlands within ±2 K of the lowland, at sea level" was wrong, and is
  missed in the other direction:** the highlands are now 7 K warmer than the tropical lowland
  on average when both are reduced to sea level (−5.0 → +7.0 K). That is what real plateaus
  show: the Altiplano (3.8 km, ~8 °C) reduces to ~33 °C against ~26 °C in the Amazon lowland,
  Tibet (4.5 km, ~−2 °C) to ~27 °C against ~17 °C at its latitude (values from memory, not
  looked up). The lowland mean here also includes cooler coasts and the land at 20–25°.

## Guard rails

High latitudes, which the change must leave alone:

| | main | branch | t |
|---|---|---|---|
| Land poleward of 60° below −10 °C (% of that land) | 51.9 | 51.5 | −0.1 |
| … and with ≥ 0.3 m/yr of precipitation (ice sheets) | 24.1 | 21.8 | −0.7 |
| Land at 45–60° below −10 °C (%) | 11.7 | 8.7 | −1.0 |
| Land poleward of 70°: annual mean (°C) | −14.9 | −14.2 | +0.7 |
| Land poleward of 50° above 2 km (°C) | −24.1 | −24.0 | +0.1 |
| Seasonal range of land at 55–70° (K) | 42.4 | 42.2 | −0.1 |

Planet:

| | main | branch | t |
|---|---|---|---|
| Global mean temperature (°C) | 14.5 | 15.2 | +1.8 |
| Permanently frozen (% of the surface) | 11.4 | 10.3 | −2.1 |
| Land precipitation (m/yr) / arid land (%) | 0.78 / 25.0 | 0.83 / 21.4 | +1.0 / −1.4 |
| **Land (% of the surface)** | 28.0 | **26.4** (22.9…30.4) | −1.6 |
| **Mean land elevation (m)** | 552 | **486** (402…666) | −1.5 |
| **Flooded continental crust (%)** | 34.0 (28.4…42.2) | **38.5** (30.0…49.0) | +1.6 |
| **Continental crust thicker than 50 km (%)** | 2.05 | **1.00** | **−2.4** |
| Highest cell (m) | 4534 | 4429 | −0.5 |
| Sediment to the ocean (km³/yr) | 3.82 | 3.40 | −1.6 |
| Runoff (km³/yr) | 37,100 | 35,700 | −0.9 |
| Sediment part of the sea level (m) | 268 | 252 | −1.9 |
| Runtime (s; machine shared, load varying) | 734 | 762 | +0.8 |

No run came near a snowball.

## The side effect: lower land

The highlands were protected by their cold. On seeds 0–3 (500–1000 Myr; main → branch):

| | main | branch |
|---|---|---|
| Precipitation on land above 2 km (m/yr) | 0.51 (0.34…0.63) | 0.79 (0.61…0.95) |
| Precipitation on land below 1 km (m/yr) | 0.80 | 0.85 |
| Annual mean of land above 2 km at |lat| < 45° (°C) | −5.8 | +6.0 |
| Land above 2 km / above 1 km (% of land) | 7.3 / 15.3 | 4.0 / 10.7 |
| Mean thickness of continental crust (km) | 29.6 | 28.3 |
| Mean elevation of continental crust (m) | +188 | −46 |

Warm highlands get half as much rain again as cold ones (cold ground hardly evaporates, so
little of the rain is recycled), rivers cut them down faster, and the continents end lower and
more flooded. Each of the planet-wide differences is below significance on 8 seeds, but they
all follow from this one chain and point the same way, so I take the effect as real: about
−65 m of mean land, −1.6 points of land area, +4.5 points of flooded continental crust.

This moves the hypsometry away from Earth's (land above 2 km: Earth ~8% without the ice
sheets, model 7.3 → 4.0%; mean land Earth ~700 m, model 552 → 486 m). The cold highlands were
compensating for erosion that is too strong for the relief the tectonics builds, or for
mountain building that is too weak; this change removes the compensation.

## Attribution and the initial state

ATTRIBUTION

## Full tables (8 seeds)

Highland and latitude metrics (`highland.py`):

window 500-1000 Myr; seeds: main: 8, final: 8
| metric | main | final | t |
|---|---|---|---|
| tropical land > 3 km (% of tropical land) | 2.39 (0.55…7.87) | 1.76 (0.56…3.58) | -0.7 |
| tropical land > 3 km: T reduced to sea level (C) | 16.72 (12.61…19.97) | 29.67 (28.34…31.14) | +13.1 |
| tropical lowland < 500 m: T (C) | 21.83 (20.01…23.35) | 22.76 (22.21…23.22) | +2.2 |
| tropical land > 3 km minus lowland, at sea level (K) | -5.05 (-9.98…-0.61) | 6.96 (5.29…9.08) | +9.5 |
| tropical land with T < -10 C (% of tropical land) | 1.21 (0.51…3.53) | 0.00 (0.00…0.00) | -3.4 |
| tropical land with T < -5 C (% of tropical land) | 1.83 (0.85…5.05) | 0.02 (0.00…0.09) | -3.7 |
| land at |lat| < 45 above 3 km: T reduced to sea level minus lowland there (K) | -6.19 (-12.64…-3.35) | 6.82 (1.93…10.71) | +8.2 |
| land at |lat| < 45 with T < -10 C (% of that land) | 2.35 (0.37…4.43) | 0.28 (0.02…1.12) | -4.4 |
| land at 15-35 above 3.5 km: T (C) | -13.43 (-20.16…-6.28) | 1.95 (-0.32…6.48) | +9.0 |
| land at 15-35 above 3.5 km with T < -5 C (% of it) | 58.55 (16.67…98.22) | 6.58 (0.00…15.92) | -5.5 |
| land at 45-60 with T < -10 C (% of that land) | 11.68 (3.09…18.07) | 8.72 (1.44…19.69) | -1.0 |
| land poleward of 60 with T < -10 C (% of that land) | 51.92 (16.50…66.33) | 51.50 (31.19…77.04) | -0.1 |
| land poleward of 60 with T < -10 C and P >= 0.3 m/yr (% of that land) | 24.10 (11.04…33.63) | 21.79 (16.31…33.57) | -0.7 |
| land poleward of 70: T (C) | -14.94 (-16.69…-9.50) | -14.18 (-18.29…-12.45) | +0.7 |
| land poleward of 50 above 2 km: T (C) | -24.08 (-27.87…-18.59) | -23.99 (-28.71…-19.94) | +0.1 |
| all land with T < -10 C (% of land) | 9.51 (2.07…13.77) | 8.78 (5.18…11.92) | -0.5 |
| land T (C) | 9.78 (7.33…11.67) | 10.98 (9.62…12.80) | +1.7 |
| global T (C) | 14.47 (13.81…15.45) | 15.19 (13.99…16.42) | +1.9 |
| land 55-70 seasonal range (K) | 42.44 (38.65…47.99) | 42.16 (34.78…47.37) | -0.1 |
| land precipitation (m/yr) | 0.77 (0.68…0.88) | 0.82 (0.69…0.95) | +1.1 |
| arid land < 0.25 m/yr (% of land) | 24.98 (18.47…30.62) | 21.37 (11.73…29.34) | -1.4 |
| highest land (m) | 4473.66 (3044.62…4938.93) | 4377.58 (4100.85…4693.33) | -0.4 |

Log metrics (`compare.py`):

window 500-1000 Myr; seeds: main: 8, final: 8
| metric | main | final | t |
|---|---|---|---|
| plates | 24.07 (17.80…28.20) | 26.12 (17.00…32.60) | +1.0 |
| plates > 1% | 14.12 (9.80…17.40) | 14.45 (10.40…17.60) | +0.3 |
| mean speed (cm/yr) | 3.84 (3.56…4.20) | 3.71 (3.36…4.38) | -1.0 |
| speed 99% (cm/yr) | 9.38 (8.62…10.86) | 8.82 (7.92…9.82) | -1.7 |
| trenches (10^3 km) | 122 (107…135) | 127 (102…148) | +0.8 |
| ridges (10^3 km) | 95.84 (77.28…106.38) | 98.85 (74.23…118.62) | +0.5 |
| collisions (10^3 km) | 13.37 (1.26…20.79) | 11.41 (5.57…14.53) | -0.8 |
| incipient trenches (10^3 km) | 10.52 (5.34…14.31) | 9.86 (6.17…13.32) | -0.5 |
| convergence at incipient trenches (cm/yr) | 1.90 (1.68…2.26) | 1.95 (1.68…2.28) | +0.4 |
| convergence at other trenches (cm/yr) | 4.71 (4.52…4.88) | 4.55 (4.16…4.88) | -1.9 |
| slab pull (1e21 N) | 1.29 (1.20…1.38) | 1.32 (1.19…1.42) | +0.9 |
| relative speed median (cm/yr) | 6.58 (6.08…6.92) | 6.44 (5.96…7.00) | -1.0 |
| boundary slower than 2 cm/yr (%) | 7.67 (3.80…11.00) | 6.62 (4.20…9.00) | -1.0 |
| opening/closing (%) | 60.52 (56.80…66.00) | 62.10 (59.80…66.40) | +1.2 |
| strike-slip (%) | 14.75 (11.40…17.20) | 14.22 (12.40…15.20) | -0.7 |
| sea-floor production kinematic (km2/yr) | 5.68 (5.10…6.21) | 5.86 (5.15…6.54) | +0.9 |
| sea-floor production created (km2/yr) | 6.42 (5.84…6.88) | 6.58 (5.79…7.33) | +0.7 |
| mean sea-floor age (Myr) | 53.72 (46.26…58.44) | 53.04 (48.34…58.56) | -0.4 |
| ocean age median (Myr) | 32.88 (29.40…36.00) | 32.00 (28.40…36.20) | -0.7 |
| sea floor >= 200 Myr (%) | 3.46 (2.17…4.70) | 3.58 (2.94…4.47) | +0.3 |
| sea floor < 20 Myr (% of ocean) | 34.08 (31.00…36.80) | 35.27 (31.20…38.40) | +1.1 |
| sea floor 20-40 Myr (%) | 23.05 (21.60…25.20) | 23.48 (21.60…25.00) | +0.7 |
| ocean deeper than 6 km (%) | 3.86 (2.90…5.24) | 3.29 (1.34…4.34) | -1.3 |
| continental (% of surface) | 34.95 (34.10…36.80) | 35.80 (34.36…37.32) | +1.6 |
| land (% of surface) | 28.03 (25.30…29.72) | 26.41 (22.92…30.44) | -1.6 |
| mean land (m) | 552 (411…692) | 486 (402…666) | -1.5 |
| highest point (m) | 4534 (3098…5080) | 4429 (4230…4763) | -0.5 |
| flooded continental crust (%) | 33.95 (28.40…42.20) | 38.45 (30.00…49.00) | +1.6 |
| crust > 50 km (% of continental) | 2.05 (0.40…3.80) | 1.00 (0.60…1.80) | -2.4 |
| sediment part of sea level (m) | 268 (245…311) | 252 (233…271) | -1.9 |
| land lower than 10 m (% of land) | 3.42 (2.34…4.48) | 3.71 (2.42…5.40) | +0.7 |
| shelf area (% of surface) | 7.04 (5.28…9.22) | 7.54 (5.84…9.06) | +0.8 |
| shelf mean sediment (km) | 3.59 (3.17…3.97) | 3.48 (2.83…4.49) | -0.5 |
| dynamic topography min (m) | -2030 (-2521…-1666) | -1815 (-2170…-1484) | +1.6 |
| dynamic topography max (m) | 1161 (1058…1291) | 1103 (966…1326) | -1.1 |
| sea-level budget mantle (m/100 Myr) | -52.89 (-76.18…-32.76) | -52.17 (-73.06…-27.40) | +0.1 |
| felsic+sediment net (Gt/yr) | -0.89 (-1.74…0.64) | -0.49 (-0.86…0.22) | +1.3 |
| trench sediment (Gt/yr) | 4.22 (2.68…5.56) | 4.00 (2.98…4.62) | -0.6 |
| collision stacking (Gt/yr) | 14.99 (5.40…22.14) | 11.86 (9.04…14.96) | -1.4 |
| burial metamorphism on continental crust (Gt/yr) | 0.10 (0.06…0.18) | 0.09 (0.04…0.16) | -0.5 |
| burial metamorphism on other crust (Gt/yr) | 0.70 (0.47…0.96) | 0.62 (0.35…0.96) | -0.8 |
| sediment trapped in lakes (km3/yr) | 0.17 (0.08…0.30) | 0.12 (0.07…0.15) | -1.7 |
| depressions (lakes) | 229 (176…304) | 217 (169…273) | -0.5 |
| land precipitation (m/yr) | 0.78 (0.66…0.91) | 0.83 (0.68…0.96) | +1.0 |
| runoff (km3/yr) | 37131 (32737…44984) | 35670 (32085…41229) | -0.9 |
| largest river (m3/s) | 83802 (66022…119814) | 74853 (49050…85658) | -1.2 |
| sediment to ocean (km3/yr) | 3.82 (2.84…4.92) | 3.40 (3.06…3.66) | -1.6 |
| endorheic (% of land) | 11.18 (5.74…20.06) | 10.54 (5.94…16.00) | -0.3 |
| endorheic into single-cell pits (% of land) | 5.42 (3.32…9.34) | 5.79 (3.60…7.44) | +0.5 |
| lakes (% of land) | 2.45 (1.50…4.04) | 2.05 (1.62…2.44) | -1.4 |
| lake evaporation (km3/yr) | 2460 (1594…3970) | 2299 (1522…3034) | -0.5 |
| global temperature (C) | 14.53 (13.64…15.50) | 15.24 (14.10…16.46) | +1.8 |
| equator (C) | 26.44 (25.56…27.30) | 26.77 (25.82…27.94) | +0.9 |
| poles (C) | -13.03 (-13.96…-11.50) | -12.65 (-14.16…-11.48) | +0.9 |
| permanently frozen (%) | 11.36 (10.06…12.92) | 10.28 (8.76…11.82) | -2.1 |
| ocean heat transport 20N (PW) | 1.25 (1.04…1.36) | 1.29 (1.24…1.38) | +0.7 |
| gyres (Sv) | 11.53 (9.80…13.60) | 11.60 (10.60…14.60) | +0.1 |
| rifts /100 Myr | 4.50 (3.00…5.60) | 3.95 (2.00…4.80) | -1.2 |
| initiations /100 Myr | 8.15 (4.80…10.20) | 8.05 (4.40…11.20) | -0.1 |
| sutures /100 Myr | 3.25 (1.20…4.60) | 2.38 (1.40…3.20) | -1.9 |
| plates born /100 Myr | 13.30 (8.40…15.00) | 12.78 (7.20…17.00) | -0.4 |
| segments created /100 Myr | 489 (385…533) | 498 (419…566) | +0.3 |
| terminal fans and deltas (km cells /100 Myr) | 5194 (3177…8699) | 5219 (2052…8425) | +0.0 |
| overfill (km cells /100 Myr) | 0.00 (0.00…0.00) | 0.00 (0.00…0.00) | — |
| sediment-part slope (m/100 Myr) | 12.05 (4.05…20.88) | 10.18 (-0.12…16.91) | -0.6 |
| sediment part at 1000 Myr (m) | 286 (266…339) | 290 (271…315) | +0.3 |
| highest point, max over all snapshots (m) | 5834 (5508…6332) | 5666 (5106…5907) | -1.3 |
| runtime (s) | 734 (672…840) | 762 (667…864) | +0.8 |
