# #190 results: the barren default at epoch 0 against the old default

Commit `f5b0570` (the default change; the later commits only touch docs and tests), 4 seeds
(0–3) × 1000 Myr, 6 subdivisions, snapshots every 50 Myr, run on GitHub runners through `runs`.

| run | arguments |
|---|---|
| **new** (default) | `--duration 1000` (= `--biosphere abiotic --epoch 0`) |
| **old** (default before #190) | `--duration 1000 --biosphere modern --epoch 4` |
| abiotic today | `--duration 1000 --epoch 4` (separates biosphere and epoch) |

Values from the per-snapshot log lines. "Arid" is the share of land with less than 0.25 m/yr of
precipitation; "sea level" is the dry offset of the log line (the model fills a fixed ocean
volume, so it moves with the sediment and the topography). Ranges are per-seed means.

## Means over 200–1000 Myr

| | new | old | abiotic today |
|---|---|---|---|
| global mean temperature (°C) | **17.4** (17.3–17.6) | 20.9 (19.9–21.6) | 19.6 (18.9–20.3) |
| equator / poles (°C) | 26.0 / −2.5 | 32.8 / −7.3 | 31.4 / −8.5 |
| permanently frozen (% of surface) | **1.1** (0.8–1.4) | 4.9 (3.8–6.7) | 5.8 (4.8–6.9) |
| land precipitation (m/yr) | **0.52** (0.49–0.57) | 1.06 (0.95–1.22) | 0.73 (0.63–0.82) |
| global precipitation (m/yr) | 0.96 | 1.39 | 1.29 |
| runoff of land (m/yr) | 0.27 | 0.37 | 0.38 |
| arid land (< 0.25 m/yr) | 37% | 23% | 29% |
| Köppen A / B / C / D / E (% of land) | 8 / **46** / 15 / 26 / 5 | 16 / 34 / 15 / 30 / 6 | 10 / 37 / 13 / 33 / 7 |
| land (% of surface) | 28.2 | 30.1 | 28.3 |
| mean land elevation (m) | 525 | 808 | 465 |
| sea level, dry offset (m) | −172 | −186 | −169 |
| river sediment to the sea (Gt/yr) | 10.9 | 8.2 | 12.3 |

## Start and end (4-seed means)

| | new t=0 | new t=1000 | old t=0 | old t=1000 |
|---|---|---|---|---|
| global mean (°C) | 12.2 | 19.8 | 12.4 | 25.3 |
| equator / poles (°C) | 20.2 / −8.9 | 28.7 / −0.2 | 24.4 / −17.4 | 36.4 / −3.0 |
| permanently frozen | 6.1% | 0.2% | 14.0% | 1.3% |
| land precipitation (m/yr) | 0.49 | 0.55 | 0.68 | 1.37 |
| Köppen A / B / E | 2 / 33 / 21 | 6 / 48 / 3 | 6 / 31 / 21 | 22 / 34 / 3 |

No seed approaches a snowball: the most ice any new-default seed has at any snapshot is 6.3%
(t = 0); after 450 Myr it stays below 2.5% in every seed and ends at 0–1%.

## What this means

- **Climate of the new default.** At the start the global mean is the same as the old default
  (12.2 vs 12.4 °C), but the uniform greenhouse forcing (+61.8 W/m²) that offsets the faint sun
  (S/S₀ 0.741) flattens the gradient: the tropics are ~4 K cooler, the poles ~9 K warmer, and
  less than half as much is frozen. That is the faint-young-sun pattern already seen at epoch
  0.5 on #148/#153.
- **Warming during the run.** The forcing stays fixed while the sun brightens from 0.741 to
  0.792 S₀ (+12 W/m² absorbed), so the new default warms by ~7.6 K over 1000 Myr. The old default
  warms by ~13 K (1.0 → 1.096 S₀, +23 W/m²). The new default is thus the more stable of the two,
  but it still drifts towards an almost ice-free planet (0.2% frozen at 1000 Myr). The carbon
  cycle (#154) is meant to fix this drift. An ice-free early planet is plausible: Earth has no
  record of glaciation before ~2.9 Ga.
- **Dry.** Land gets half the rain of the old default (0.52 vs 1.06 m/yr). Two effects add up:
  the barren land recycles less (Budyko ω 1.6; abiotic today alone gives −31%) and the cooler
  tropics with a weaker temperature contrast evaporate less (global precipitation 0.96 vs 1.29
  m/yr in abiotic today). Arid climates (Köppen B) cover 46% of the land, against 34%. Tropical
  climates (A) halve, because the cooler tropics have more months below the A thresholds and
  less rain.
- **Lower, more eroded land.** The barren preset doubles erodibility and raises the runoff
  fraction, so the mean land is ~280 m lower (525 vs 808 m) and rivers carry 33% more sediment
  to the sea. The sea level (dry offset) and the land fraction change little (28 vs 30%).
- **Biosphere vs epoch.** At the same epoch (4), barren land alone makes the planet 1.3 K colder
  with a little more ice (bright land), 31% less land rain and ~340 m lower land. The epoch-0
  sun and forcing then add the flat gradient, the cooler tropics and the missing ice. The
  `Biosphere::Abiotic` docs are updated with these numbers (the old ones predate several climate
  changes, e.g. #7 and #33).

## Snapshots of the new default (seed 0)

`new-seed0-{elevation,temperature,precipitation}-{00000,01000}.png` in this folder: the start
and the end of 1000 Myr. At 1000 Myr the tropics are warm everywhere, the polar caps are nearly
free of ice, and the land interiors are dry.

## Grid notes

At 6 subdivisions (~110 km cells) the numbers above are model values. The Köppen shares on
coarser meshes (e.g. 3 subdivisions in a unit test) can lose the A class at the start, since
the few tropical land cells are cooler under the barren default.
