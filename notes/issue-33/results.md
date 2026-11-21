# #33 results: low-latitude highlands are at least as warm as the free air around them

_2026-11-21. Plan: [`plan.md`](plan.md), plan review: [`review-plan.md`](review-plan.md), results
evaluation: [`review-results.md`](review-results.md). `main` at 288f9cf against the branch,
8 seeds (0–7) × 1000 Myr, default parameters. `main` here is the first build of the branch
with `--set temperature.free_air_height=0`, which the plan review confirmed to be bit-identical
to `main`. Mean over seeds (seed range) of the snapshots of 600–1000 Myr; Welch t, n = 8
(|t| ≳ 2.4 ≈ p < 0.05)._

Three versions were run: the first (free air at all latitudes, in both directions; stopped
after the plan review), the second (low latitudes, warming only, weight from sea level; the one
the results evaluation examined, 8 seeds) and the final one (weight from 1 km; 8 seeds). The
tables are the final version. **Its numbers were not evaluated independently again**; they
come from the same scripts whose output for the second version the evaluator reproduced.

## What changed (`src/temperature.rs`)

1. Land in low latitudes is at least as warm as the free air at its height, which the sea
   surface of its latitude band sets. The annual mean is raised towards it with elevation
   (from 1 km, full at 3 km), fully up to 30° of latitude and not at all beyond 45°. It is
   never lowered.
2. The spin-up of the temperature model starts without ice sheets. **This has no effect in the
   simulation as it is run today** (see "Attribution"); it removes a latent artefact.

## The issue's metrics

| | main | branch | t |
|---|---|---|---|
| Tropical land (|lat| < 25°) with an annual mean below −10 °C (% of tropical land) | 1.16 (0.61…2.64) | **0.01** (0.00…0.05) | −4.8 |
| … below −5 °C, where ice begins | 1.75 | 0.14 | −4.6 |
| Tropical land above 3 km, reduced to sea level with the lapse rate (°C) | 16.6 (11.9…20.9) | 29.2 (27.8…30.3) | +10.5 |
| Land at 15–35° above 3.5 km: annual mean (°C) | −14.0 (−22.2…−6.3) | −1.6 (−6.5…0.7) | +6.4 |
| … share of it below −5 °C | 59% | 18% | −4.0 |
| Abiotic preset at t = 0 against modern (K; seeds 0–3) | −6.4 | −4.1 | |

- The ice on tropical highlands is gone. Between 15° and 35° a fifth of the land above 3.5 km
  is still colder than −5 °C: by the lapse rate alone, land above ~4 km at 30° is, and beyond
  30° the free air fades out by design.
- The abiotic start (#29: "6.4 K colder at t = 0") reproduces on `main` (5.8 against 12.2 °C,
  seeds 0–3) and is 8.3 against 12.4 °C on the branch. What the remaining 4 K consist of was
  not examined (#29 attributed the whole offset to the brighter bare land of the preset).
- Reduced to sea level, tropical highlands are now 7.6 K warmer than the mean tropical
  lowland (before: 5.0 K colder). That is not a warm plateau: the evaluator found the
  highlands at the sea-surface temperature of their latitude band (+0.5 K, second version), as
  designed, while
  the model's tropical lowland is 4.6–5.3 K **colder** than the sea of its band, on `main` and
  on the branch alike. That lowland bias is a separate finding (follow-up issue).

## Guard rails

None of these differs significantly.

| | main | branch | t |
|---|---|---|---|
| Land poleward of 60° below −10 °C (% of that land) | 52.2 | 57.8 | +0.8 |
| … and with ≥ 0.3 m/yr of precipitation (ice sheets) | 24.4 | 23.2 | −0.4 |
| Land at 45–60° below −10 °C (%) | 12.6 | 15.9 | +0.8 |
| Land poleward of 70°: annual mean (°C) | −15.0 | −15.9 | −0.9 |
| Land poleward of 50° above 2 km (°C) | −24.4 | −26.1 | −1.1 |
| Seasonal range of land at 55–70° (K) | 42.7 | 40.6 | −1.2 |
| Global mean temperature (°C) | 14.5 | 14.2 | −1.2 |
| Permanently frozen (% of the surface) | 11.4 | 11.6 | +0.5 |
| Land precipitation (m/yr) / arid land (%) | 0.78 / 24.0 | 0.72 / 29.0 | −1.4 / +1.6 |
| Precipitation on land above 2 km (m/yr) | 0.55 | 0.62 | +0.9 |
| Land (% of the surface) | 28.0 | 28.8 | +1.1 |
| Mean land elevation (m) | 552 | 612 | +1.1 |
| Land above 2 km / above 1 km (% of land) | 7.0 / 14.6 | 8.3 / 17.4 | +1.0 / +1.4 |
| Flooded continental crust (%) | 33.9 | 32.0 | −0.9 |
| Sediment to the ocean (km³/yr) | 3.82 | 4.09 | +0.9 |
| Runoff (km³/yr) | 37,100 | 36,500 | −0.3 |
| Sediment part of the sea level (m) | 268 | 281 | +1.6 |
| Runtime (s; shared machine, load varying) | 734 | 683 | −1.1 |

No run came near a snowball. Among the ~60 log metrics, three unrelated ones pass |t| = 2.4
(share of opening/closing boundary −2.5, mean shelf sediment +2.4, terminal fans +2.5), about
what chance gives.

## What the second version showed, and why the final one differs

In the second version the weight grew from sea level. Its 8 seeds had lower and more flooded
continents (mean land 552 → 486 m, flooded continental crust 34 → 38%, each below
significance), and I first read that as "warm highlands get more rain and erode faster". The
results evaluation did not support that chain: sediment to the ocean was lower, not higher;
the lowering appeared late and unsteadily; and most of it went with fewer collisions in those
runs. It also found that the version warmed ordinary tropical uplands by 2–5 K (300 m to
2 km), because the lowland is colder than the sea of its latitude and the weight pulled it up.
The final version leaves ground below 1 km alone. Its continents are, if anything, slightly
higher than on `main` (not significant). So no side effect on the relief is established.

## Attribution

- **Ice-free start alone** (final build with `temperature.free_air_height=0`): seeds 0 and 1
  are byte-identical to `main` in the cell dump of 1000 Myr (checked here) and in every dump
  up to 500 Myr (evaluator), and the abiotic t = 0 state
  is identical on seeds 0–3. The initial topography is at most ~1.8 km high, so neither start
  puts ice on low-latitude ground in the first year. The diagnosis in the first plan ("the
  spin-up starts in the glaciated state") holds in the unit tests, not in the runs.
- So everything measured above is the free air.

## Results evaluation ([`review-results.md`](review-results.md), on the second version)

1. All temperature numbers reproduced. The planet-wide tables used two different windows
   (500–1000 and 600–1000 Myr) under one label → both scripts now use 600–1000.
2. The side-effect chain was not supported → withdrawn (section above).
3. The explanation of the warm highlands by real plateaus was not supported → replaced by the
   lowland bias the evaluator measured.
4. The weight from sea level warmed ordinary uplands → the weight now starts at 1 km; all
   8 seeds were run again.
5. The ice-free start has no effect in the runs → stated; it stays as a safeguard for other
   initial conditions.
6. Code: correct. Minor, not changed: the latitude fade uses the coarse cell's latitude, so
   between 30° and 45° the shift steps by up to ~25% at coarse-cell edges.
7. Tests the evaluator missed: ground at 0.5–1.5 km (added: unchanged below 1 km); a plateau
   at 45° and the continuity of the sea/air blend are still not tested.

## Full tables (8 seeds, final version)

Highland and latitude metrics (`highland.py`):

window 500-1000 Myr; seeds: main: 8, final: 8
| metric | main | final | t |
|---|---|---|---|
| tropical land > 3 km (% of tropical land) | 2.34 (0.66…7.03) | 2.74 (0.57…8.13) | +0.4 |
| tropical land > 3 km: T reduced to sea level (C) | 16.61 (11.85…20.93) | 29.19 (27.81…30.33) | +10.5 |
| tropical lowland < 500 m: T (C) | 21.73 (20.01…23.32) | 21.66 (20.96…23.21) | -0.2 |
| tropical land > 3 km minus lowland, at sea level (K) | -5.04 (-10.82…-0.36) | 7.62 (5.31…8.61) | +8.6 |
| tropical land with T < -10 C (% of tropical land) | 1.16 (0.61…2.64) | 0.01 (0.00…0.05) | -4.8 |
| tropical land with T < -5 C (% of tropical land) | 1.75 (1.01…3.98) | 0.14 (0.00…0.43) | -4.6 |
| land at |lat| < 45 above 3 km: T reduced to sea level minus lowland there (K) | -6.66 (-14.85…-2.69) | 4.60 (-0.02…9.97) | +6.0 |
| land at |lat| < 45 with T < -10 C (% of that land) | 2.59 (0.44…4.21) | 1.12 (0.06…3.57) | -2.5 |
| land at 15-35 above 3.5 km with T < -5 C (% of it) | 59.25 (20.00…97.86) | 17.82 (5.33…57.24) | -4.0 |
| land at 45-60 with T < -10 C (% of that land) | 12.57 (2.29…19.61) | 15.87 (5.37…32.49) | +0.8 |
| land poleward of 60 with T < -10 C (% of that land) | 52.15 (17.61…70.10) | 57.76 (46.90…66.91) | +0.8 |
| land poleward of 60 with T < -10 C and P >= 0.3 m/yr (% of that land) | 24.44 (11.46…33.69) | 23.16 (18.82…29.23) | -0.4 |
| land poleward of 70: T (C) | -14.95 (-17.38…-9.61) | -15.91 (-18.11…-13.11) | -0.9 |
| land poleward of 50 above 2 km: T (C) | -24.41 (-27.86…-18.59) | -26.13 (-31.41…-22.47) | -1.1 |
| all land with T < -10 C (% of land) | 10.31 (2.33…14.98) | 10.13 (5.52…17.13) | -0.1 |
| land T (C) | 9.47 (6.73…11.57) | 10.05 (6.55…13.31) | +0.6 |
| global T (C) | 14.53 (13.66…15.49) | 14.18 (13.64…14.78) | -1.2 |
| land 55-70 seasonal range (K) | 42.67 (39.12…47.97) | 40.62 (33.80…44.54) | -1.2 |
| land precipitation (m/yr) | 0.78 (0.66…0.91) | 0.72 (0.63…0.85) | -1.4 |
| arid land < 0.25 m/yr (% of land) | 24.04 (15.36…31.84) | 28.98 (21.57…39.39) | +1.6 |
| highest land (m) | 4534.34 (3097.70…5079.54) | 4760.84 (4075.86…5811.70) | +0.7 |
| land above 2 km (% of land) | 7.01 (2.80…10.52) | 8.31 (4.25…11.86) | +1.0 |
| land above 1 km (% of land) | 14.63 (9.70…19.26) | 17.43 (10.97…22.34) | +1.4 |
| precipitation on land above 2 km (m/yr) | 0.55 (0.37…0.70) | 0.62 (0.46…0.85) | +0.9 |
| precipitation on land 2-3 km at 15-30 (m/yr) | 0.44 (0.26…0.63) | 0.53 (0.29…0.77) | +1.1 |
| land at |lat| < 30, 300-1000 m: T reduced to sea level (C) | 20.49 (19.13…21.53) | 20.99 (19.77…21.72) | +1.3 |
| land at |lat| < 30, 1-2 km: T reduced to sea level (C) | 19.82 (17.72…21.59) | 22.54 (21.13…23.47) | +4.5 |
| flooded continental crust (%) | 33.88 (28.40…42.06) | 31.97 (25.68…36.27) | -0.9 |
| mean land (m) | 552.29 (410.65…692.22) | 611.92 (446.72…767.20) | +1.1 |
| land (% of surface) | 28.04 (25.29…29.72) | 28.78 (27.68…30.30) | +1.1 |
| continental crust > 50 km (%) | 2.10 (0.49…3.97) | 2.66 (1.03…5.69) | +0.8 |
| land at 15-35 above 3.5 km: T (C) | -14.00 (-22.22…-6.28) | -1.61 (-6.50…0.72) | +6.4 |

Log metrics (`compare.py`):

window 500-1000 Myr; seeds: main: 8, final: 8
| metric | main | final | t |
|---|---|---|---|
| plates | 24.07 (17.80…28.20) | 24.43 (19.60…30.80) | +0.2 |
| plates > 1% | 14.12 (9.80…17.40) | 14.40 (11.80…17.60) | +0.2 |
| mean speed (cm/yr) | 3.84 (3.56…4.20) | 3.82 (3.56…4.06) | -0.2 |
| speed 99% (cm/yr) | 9.38 (8.62…10.86) | 9.26 (8.80…10.18) | -0.4 |
| trenches (10^3 km) | 122 (107…135) | 121 (111…128) | -0.3 |
| ridges (10^3 km) | 95.84 (77.28…106.38) | 95.63 (83.27…105.07) | -0.1 |
| collisions (10^3 km) | 13.37 (1.26…20.79) | 15.90 (9.91…27.18) | +0.8 |
| incipient trenches (10^3 km) | 10.52 (5.34…14.31) | 10.87 (6.37…16.06) | +0.2 |
| convergence at incipient trenches (cm/yr) | 1.90 (1.68…2.26) | 1.89 (1.50…2.34) | -0.1 |
| convergence at other trenches (cm/yr) | 4.71 (4.52…4.88) | 4.59 (4.38…4.82) | -1.7 |
| slab pull (1e21 N) | 1.29 (1.20…1.38) | 1.26 (1.21…1.31) | -1.2 |
| relative speed median (cm/yr) | 6.58 (6.08…6.92) | 6.52 (6.12…6.96) | -0.4 |
| boundary slower than 2 cm/yr (%) | 7.67 (3.80…11.00) | 9.55 (5.40…12.20) | +1.4 |
| opening/closing (%) | 60.52 (56.80…66.00) | 57.88 (56.80…60.20) | -2.5 |
| strike-slip (%) | 14.75 (11.40…17.20) | 16.02 (14.00…17.00) | +1.7 |
| sea-floor production kinematic (km2/yr) | 5.68 (5.10…6.21) | 5.64 (5.26…6.06) | -0.2 |
| sea-floor production created (km2/yr) | 6.42 (5.84…6.88) | 6.43 (5.86…6.93) | +0.1 |
| mean sea-floor age (Myr) | 53.72 (46.26…58.44) | 55.51 (50.94…60.24) | +0.9 |
| ocean age median (Myr) | 32.88 (29.40…36.00) | 34.60 (31.60…36.40) | +2.0 |
| sea floor >= 200 Myr (%) | 3.46 (2.17…4.70) | 3.78 (2.57…5.46) | +0.7 |
| sea floor < 20 Myr (% of ocean) | 34.08 (31.00…36.80) | 32.95 (31.00…35.20) | -1.4 |
| sea floor 20-40 Myr (%) | 23.05 (21.60…25.20) | 22.38 (21.40…23.40) | -1.3 |
| ocean deeper than 6 km (%) | 3.86 (2.90…5.24) | 3.71 (3.38…4.34) | -0.5 |
| continental (% of surface) | 34.95 (34.10…36.80) | 34.30 (33.32…35.26) | -1.7 |
| land (% of surface) | 28.03 (25.30…29.72) | 28.78 (27.66…30.28) | +1.1 |
| mean land (m) | 552 (411…692) | 612 (447…767) | +1.1 |
| highest point (m) | 4534 (3098…5080) | 4761 (4076…5812) | +0.7 |
| flooded continental crust (%) | 33.95 (28.40…42.20) | 32.02 (25.60…36.60) | -0.9 |
| crust > 50 km (% of continental) | 2.05 (0.40…3.80) | 2.55 (0.80…5.80) | +0.7 |
| sediment part of sea level (m) | 268 (245…311) | 281 (265…300) | +1.6 |
| land lower than 10 m (% of land) | 3.42 (2.34…4.48) | 3.42 (2.40…4.92) | +0.0 |
| shelf area (% of surface) | 7.04 (5.28…9.22) | 7.08 (6.06…8.04) | +0.1 |
| shelf mean sediment (km) | 3.59 (3.17…3.97) | 3.94 (3.37…4.25) | +2.4 |
| dynamic topography min (m) | -2030 (-2521…-1666) | -1948 (-2259…-1805) | +0.7 |
| dynamic topography max (m) | 1161 (1058…1291) | 1083 (932…1170) | -1.9 |
| sea-level budget mantle (m/100 Myr) | -52.89 (-76.18…-32.76) | -55.65 (-73.14…-44.64) | -0.5 |
| felsic+sediment net (Gt/yr) | -0.89 (-1.74…0.64) | -1.30 (-2.20…-0.28) | -1.2 |
| trench sediment (Gt/yr) | 4.22 (2.68…5.56) | 4.70 (3.74…6.06) | +1.1 |
| collision stacking (Gt/yr) | 14.99 (5.40…22.14) | 18.11 (13.40…27.84) | +1.1 |
| burial metamorphism on continental crust (Gt/yr) | 0.10 (0.06…0.18) | 0.11 (0.07…0.19) | +0.7 |
| burial metamorphism on other crust (Gt/yr) | 0.70 (0.47…0.96) | 0.84 (0.59…1.05) | +1.6 |
| sediment trapped in lakes (km3/yr) | 0.17 (0.08…0.30) | 0.20 (0.11…0.35) | +0.8 |
| depressions (lakes) | 229 (176…304) | 264 (219…302) | +1.6 |
| land precipitation (m/yr) | 0.78 (0.66…0.91) | 0.72 (0.63…0.84) | -1.4 |
| runoff (km3/yr) | 37131 (32737…44984) | 36548 (32291…41072) | -0.3 |
| largest river (m3/s) | 83802 (66022…119814) | 81423 (50704…111138) | -0.3 |
| sediment to ocean (km3/yr) | 3.82 (2.84…4.92) | 4.09 (3.44…4.84) | +0.9 |
| endorheic (% of land) | 11.18 (5.74…20.06) | 12.23 (8.98…18.98) | +0.5 |
| endorheic into single-cell pits (% of land) | 5.42 (3.32…9.34) | 6.64 (5.12…9.16) | +1.5 |
| lakes (% of land) | 2.45 (1.50…4.04) | 2.42 (1.88…3.38) | -0.1 |
| lake evaporation (km3/yr) | 2460 (1594…3970) | 2365 (1895…3083) | -0.3 |
| global temperature (C) | 14.53 (13.64…15.50) | 14.17 (13.62…14.76) | -1.2 |
| equator (C) | 26.44 (25.56…27.30) | 25.86 (24.44…26.98) | -1.5 |
| poles (C) | -13.03 (-13.96…-11.50) | -13.38 (-14.40…-12.82) | -0.9 |
| permanently frozen (%) | 11.36 (10.06…12.92) | 11.58 (10.72…12.58) | +0.5 |
| ocean heat transport 20N (PW) | 1.25 (1.04…1.36) | 1.21 (1.04…1.32) | -0.7 |
| gyres (Sv) | 11.53 (9.80…13.60) | 11.95 (9.80…15.00) | +0.5 |
| rifts /100 Myr | 4.50 (3.00…5.60) | 3.98 (2.80…5.20) | -1.3 |
| initiations /100 Myr | 8.15 (4.80…10.20) | 8.12 (6.60…9.40) | -0.0 |
| sutures /100 Myr | 3.25 (1.20…4.60) | 3.12 (2.00…4.00) | -0.3 |
| plates born /100 Myr | 13.30 (8.40…15.00) | 13.22 (10.40…15.20) | -0.1 |
| segments created /100 Myr | 489 (385…533) | 501 (433…568) | +0.5 |
| terminal fans and deltas (km cells /100 Myr) | 5194 (3177…8699) | 7654 (5175…10822) | +2.5 |
| overfill (km cells /100 Myr) | 0.00 (0.00…0.00) | 0.00 (0.00…0.00) | — |
| sediment-part slope (m/100 Myr) | 12.05 (4.05…20.88) | 12.85 (4.20…25.37) | +0.2 |
| sediment part at 1000 Myr (m) | 286 (266…339) | 284 (266…335) | -0.2 |
| highest point, max over all snapshots (m) | 5834 (5508…6332) | 5859 (5554…6651) | +0.2 |
| runtime (s) | 734 (672…840) | 683 (518…799) | -1.1 |
