# #190: the default run starts at epoch 0 with a barren planet

Issue #190 (Sebastian): "After switching the epoch counter to start at zero at the earliest
possible simulation start, the default for a simulation should be to start at zero with a barren
planet." Stacked on #150 (PR #192), which makes `--epoch` count forwards: 0 = 4.0 Ga (sun age
0.57 Gyr, S/S₀ 0.741), 4 = today (the default so far).

## 1. What the code assumes about life (inventory)

The parameter structs (`TectonicParams`, `ErosionParams`, `ClimateParams`,
`TemperatureParams`) describe **present-day Earth**. Issue #28 / #29 added presets
(`src/presets.rs`) that move groups of parameters to another planet; `--biosphere abiotic`
already exists and is the barren planet. Everything the model does that depends on life:

| Process | Where | Life-dependent part | `--biosphere abiotic` today |
|---|---|---|---|
| Land albedo | `temperature.land_albedo` 0.32 | vegetated land (0.15–0.2 at the surface) | 0.37 (bare rock, regolith, desert) |
| Evapotranspiration / moisture recycling | Budyko–Fu curve, `climate.budyko_omega` 2.6 | transpiration (~60% of land ET) and root-zone storage | ω = 1.6 (sparse cover); potential evaporation unchanged |
| Hillslope/channel erosion | `erosion.erodibility` | root cohesion, plant cover | ×2 |
| River style | `erosion.sediment_diffusivity` | root/mud-bound banks → single-thread | 1e4 m²/yr (braided) |
| Pelagic sediment | `initial_sediment.biogenic_pelagic` | carbonate/siliceous ooze | off (red clay only) |
| Vegetation colour (Realistic view, #92) | `viewer/modes/realistic.rs` | steppe/grass/forest from moisture index + warmest month | none: bare ground, no tundra greening |
| Greenhouse forcing | `presets::Epoch::greenhouse_forcing` | none directly: prescribed, offsets the faint sun (weathering-feedback assumption) | same |

So the barren treatment is complete for every process the model has, and switchable. Nothing
life-driven is "deeply built in" without a switch. Things the model does not represent at all
(so neither barren nor living): soils and chemical weathering, a carbon cycle (#154), biogenic
gases (O₂/ozone, methane; the stratosphere and the Archean methane greenhouse), biogenic cloud
condensation nuclei (DMS) in `ocean_albedo`/`radiation_a`, dynamic vegetation. These are
listed in the PR as not covered.

The Köppen classes (render mode and statistics) are a *climate* classification; names like
"tropical rainforest" describe the climate, not vegetation. They stay.

## 2. Change

- CLI defaults: `--epoch` 4 → **0**, `--biosphere` modern → **abiotic**. The web URL parameters
  and the parameter panel take their defaults from clap, so they follow; links that leave out
  `epoch=`/`biosphere=` now mean a barren planet at 4.0 Ga.
- `Biosphere`'s `#[default]` moves to `Abiotic` to match the CLI.
- The parameter structs keep their present-day-Earth defaults; `Parameters::new(.., Modern,
  Epoch::PRESENT)` stays bit-identical to them (existing test). Only the *default run* changes.
- The old default stays available as `--biosphere modern --epoch 4` (URL
  `biosphere=modern&epoch=4`); documented in the CLI help, the presets module and the README.
- Tests that assumed the old default (panel round trip with `--biosphere=abiotic`, tests whose
  default planet has vegetation) are updated.

## 3. Climate of the new default

At epoch 0 the default greenhouse forcing is +61.8 W/m², which offsets the faint sun at the start
(`(S₀ − S)(1 − 0.3)/4`), and then stays fixed. Over 1000 Myr the sun goes from 0.741 to 0.792 S₀:
+12 W/m² absorbed, against +23 W/m² for the old default (1.0 → 1.096 S₀). Both warm until the
carbon cycle (#154); the new default warms about half as much. The uniform forcing flattens the
gradient (warm poles, cooler tropics; #148), and bright bare land cools.

## 4. Runs

4 seeds × 1000 Myr through `runs`, same commit:

- **new**: defaults (`--duration 1000`)
- **old**: `--duration 1000 --biosphere modern --epoch 4`
- **abiotic today**: `--duration 1000 --epoch 4` (separates the effect of the biosphere from
  that of the epoch)

Compared: global mean temperature, permanent ice, land precipitation, Köppen and aridity
shares, sea level / land fraction, mean land elevation, sediment flux, over 200–1000 Myr and at
the start/end. Snapshots of the new default go to this folder.
