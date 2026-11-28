# #207 results: seasonal snow on land

Snow-on runs from commit `7cc54f1`, snow-off runs from `ed32175` (the same code with snow off; branch `agent/issue-207`), 6 subdivisions, 1000 Myr, snapshots every 50 Myr,
run through `runs` (GitHub runners, `e4-off` locally). "Off" is the same commit with
`--set temperature.seasonal_snow=0`, which is **bit-identical to main** (`e55bf1d`): same state
hashes on a 20 Myr check (seed 0, 5 subdivisions), so the off runs are the main baseline with
the new log line. Values are from the per-snapshot log lines; "late" = mean over 200–1000 Myr
and seeds, `t0` = after the spin-up at the start (the generated planet with its ~1 km
highlands, the coldest state of every run).

Model and parameters: see [plan.md](plan.md) (with the changes after the
[plan review](review-plan.md)). In short: snow water equivalent per fine land cell, updated
every season (24/yr) from the season mean temperature; snowfall from a smooth rain/snow split
(normal daily temperatures, σ 5 K, 50% at 1 °C); melt by positive degree days (4 mm/K/day);
cover `tanh(S/0.01 m)`; surface albedo 0.80 (cold) → 0.60 (melting); half of the surface
albedo change reaches the planetary albedo (snowy bare land 0.52–0.62, the same as an ice
sheet at most); forest masking (snowy forest 0.25, up to 70% cover where the warmest season
reaches ~10 °C) only with `--biosphere modern`; latent heat of freezing/melting exchanged
with the land; snow above 1 m w.e. flows off as glacier ice; snow that lasts the whole year
with a non-negative balance counts as permanent ice.

## Runs

| run | arguments | seeds |
|---|---|---|
| barren | `--duration 1000` (default: abiotic, epoch 0, carbon cycle) | 8 on, 8 off |
| modern | `--biosphere modern --epoch 4` | 4 on, 4 off |
| barren today | `--epoch 4` | 4 on, 4 off |
| cold | `--epoch 4 --co2 0.25 --no-carbon-cycle` (barren, −7.4 W/m², fixed) | 4 on, 4 off |
| sensitivity | cold with `surface_albedo_share=0.35`; cold with `snow_latent_heat=0`; barren with `perennial_snow_ice=0` | 4 each |

## Climate (late = 200–1000 Myr)

| | barren off | barren on | modern off | modern on | barren today off | on | cold off | cold on |
|---|---|---|---|---|---|---|---|---|
| global mean (°C), late | 17.0 | **15.5** | 22.2 | 21.6 | 25.2 | 24.4 | 14.8 | 11.5 |
| per-seed range (°C) | 15.7–18.6 | 14.0–17.6 | 21.2–22.8 | 20.4–22.5 | 24.8–25.4 | 23.3–25.9 | 14.2–15.6 | 10.2–13.0 |
| poles (°C), late | −2.7 | −5.1 | −5.5 | −6.7 | −2.1 | −4.1 | −14.3 | −17.9 |
| permanently frozen, late | 1.1% | 3.0% | 3.5% | 4.4% | 0.6% | 1.7% | 11.6% | 19.1% |
| global mean at t0 (°C) | 12.3 | **8.1** | 12.4 | 8.6 | 8.3 | 0.5 | −1.6 | −6.8 |
| frozen at t0 | 5.9% | 22.7% | 14.0% | 23.2% | 21.2% | 41.6% | 40.6% | 53.0% |
| coldest snapshot (°C) | 11.2 | 5.7 | 11.8 | 6.5 | 7.0 | −3.0 | −5.4 | −10.1 |
| most frozen snapshot | 7.3% | 29.3% | 14.8% | 28.5% | 22.8% | 48.1% | 48.0% | 58.3% |
| CO₂ (bar), late | 0.475 | 0.423 | 5.0e-4 | 6e-4 | 1.8e-3 | 2.0e-3 | 1e-4 (fixed) | 1e-4 (fixed) |
| forcing (W/m²), late | 61.0 | 59.6 | 2.3 | 3.3 | 10.3 | 11.3 | −7.4 | −7.4 |
| φ | 1.18 | 1.40 | 3.00 (clamp) | 3.00 (clamp) | 3.00 (clamp) | 3.00 (clamp) | – | – |

- **Barren default:** 1.5 K colder late, 4.2 K colder at the start; permanent ice 3.0% instead
  of 1.1% of the surface late, 22.7% instead of 5.9% at the start.
- **Modern, epoch 4:** 0.6 K colder late, within the seed spread. Forest masking and the carbon
  cycle (φ at its clamp, CO₂ +20%) absorb most of the snow. At the start it is 3.8 K colder.
- **Barren today, epoch 4:** the coldest start with a carbon cycle, 0.5 °C global with 42%
  frozen (coldest snapshot −3.0 °C, 48%); the carbon cycle recovers it within 200 Myr.
- **CO₂ and φ in the barren runs:** φ is calibrated at the end of the ramp-up phase at the
  start's CO₂ and climate. With snow that climate is colder, so φ comes out higher (1.40 vs 1.18,
  inside the 0.3–3 band). Later, as the planet warms, the higher φ draws CO₂ ~11% lower (lower in
  7 of the 8 seeds). This comes from the calibration against a colder start, not from more
  weathering under snow.

## Snow

| | barren on | modern on | barren today on | cold on |
|---|---|---|---|---|
| snow cover of NH land, Jan / Jul (late) | 25% / 3% | 24% / 2% | 16% / 0.5% | 52% / 29% |
| snow cover of SH land, Jan / Jul (late) | 2% / 17% | 3% / 27% | 2% / 25% | 19% / 39% |
| same at t0, NH Jan / Jul | 62% / 53% | 65% / 49% | 83% / 80% | 93% / 92% |
| perennial snow (% of land), late / t0 | 2.0 / 47.2 | 2.8 / 37.3 | 0.9 / 77.6 | 20.4 / 89.8 |
| sunlight reflected by snow (global, W/m²), late | **1.45** | 0.94 | 1.80 | 4.10 |
| snow stored, late (km³ / m of sea level) | 3700 / 0.010 | 5600 / 0.016 | 2400 / 0.007 | 33 000 / 0.093 |
| snow stored at t0 | 79 000 / 0.22 | 61 000 / 0.17 | 124 000 / 0.35 | 143 000 / 0.41 |

- **Seasonality.** Late in the barren run, a fifth of the winter hemisphere's land is under
  snow (NH January 25%) and a few percent of the summer hemisphere's. The planet has less land at high
  latitudes than Earth and is warmer (15–17 °C), so this is less than Earth's January
  (~45–50% of NH land). The cold starts are glacial states: the snow lasts through the summer on
  a third (barren) to almost all (cold case) of the land.
- **Albedo effect.** Snow reflects 0.9 (modern, masked by forest) to 1.5 W/m² (barren) of
  sunlight globally, and 4.1 W/m² in the cold case. For orientation, Earth's land snow is of the
  order of 1 W/m² globally. That figure is derived from Flanner et al. (2011): −2.2 to −4.6 W/m²
  over the NH for snow and sea ice together, of which land snow is a part. The barren effect
  is ~1.5× the modern one at a similar snow cover. The difference is bare land: no forest
  masking, and a brighter snow-free surface that leaves less contrast but more snow-covered area.
- **Water budget.** Unit test: `start + snowfall − melt − overflow − dropped = end` to 1e-9.
  Storage is 0.01–0.4 m of sea level and is not taken from the ocean. Rivers use the annual
  `P − ET`, which already contains the melt and the glacier overflow.

## #107: land whose warmest month is below 0 °C

| | barren off | barren on | barren on, perennial→ice off | modern off | modern on |
|---|---|---|---|---|---|
| warmest month < 0 °C, % of land (late) | 1.1 | 3.3 | 2.6 | 1.6 | 3.1 |
| … of it with ice < 0.5 (late) | **47%** | **30%** | 43% | **14%** | **9%** |
| … of it with ice < 0.5 (t0) | 16% | 3% | 16% | 0% | 1% |

- **Partly fixed by #207.** The off numbers reproduce the #55 triage (44% default, 15% modern).
  With snow on, perennial snow on frozen-summer land counts as ice: the share below 0.5 ice
  falls to 30% (barren) and 9% (modern) late, and to 1–3% in the cold starts, where that land is
  large. The late numbers concern only 1–3% of the land, so they are noisy.
- **What remains.** The rest is land where too little snow falls to outlast the few melt days
  of a summer with a monthly mean just below 0 °C (σ = 5 K puts some days above 0 °C). Without
  sublimation that is the only way snow is lost; such land is a polar desert (like the Dry
  Valleys). A second contribution is the half-month mean of the snow model being slightly
  warmer than the monthly mean of the statistic.
- **Without the perennial→ice coupling,** the share stays near main (43% late, 16% at the start):
  the fix comes from the coupling, not from the albedo.
- **The two rules disagree.** The annual-mean ice-sheet rule and the snow rule cover different
  land: late in the barren run, 1.9% of land is under an ice sheet without perennial snow (an
  annual mean below about −10 °C with summers that melt snow), and 0.6% has perennial snow but no
  ice sheet. In the cold case the figures are 12% and 8%. The ice-sheet rule is older and still
  sets the ice albedo. Deciding which rule should own permanent land ice is left open (see
  Decisions).

## Snowball risk

- **No run fell into a snowball.** The coldest state of any run is the start of the cold case:
  −6.8 °C global, 53% of the surface frozen, equator +4.2 °C (coldest snapshot −10.1 °C, 58%),
  against −1.6 °C and 41% without snow. All runs recover. The epoch-0 default is never colder
  than 5.7 °C or more than 29% frozen (off: 11.2 °C, 7%).
- **Sensitivity on the cold case** (start / late; the base case is −6.8 °C, 53% / 11.5 °C, 19%):
  - `surface_albedo_share` 0.35: −5.9 °C, 51% / 13.5 °C, 15%.
  - Latent heat off: −6.8 °C, 53% / 10.4 °C, 21%. The latent heat changes nothing in the
    glacial start, where the snow hardly melts, and warms the later partly glaciated climate by
    1.1 K: it releases the snowfall's heat in winter and takes it back with the melt in spring
    and summer.
  - Even the most snowball-prone variant stays partial.
  - A first version gave the glacier overflow's latent heat to the land and never took it back,
    ~0.8 W/m² globally at the glacial starts. It was fixed (`7cc54f1`) and the snow-on runs were
    redone; all numbers here are from the fixed version.
- **Offline solar sweep** (temperature model alone on the 6-subdivision cell dumps of two main
  runs: seed 0 at t0 and seed 1 at 500 Myr, with their January/July precipitation). The
  greenhouse forcing is 0 and the spin-up 40 years per step. The sweep goes 100 → 90 → 100% S₀.
  Cells show the global mean °C and the frozen surface:

  | S/S₀ | 100 | 98 | 96 | 94 | 92 | 90 | back to 100 |
  |---|---|---|---|---|---|---|---|
  | modern, off | 13.4 / 12% | 10.0 / 17% | 6.8 / 20% | 2.4 / 28% | −2.6 / 36% | −8.5 / 48% | 13.3 / 12% |
  | modern, on | 11.0 / 17% | 4.3 / 30% | −4.7 / 50% | −9.0 / 56% | −11.7 / 60% | −14.1 / 63% | **3.6 / 35%** |
  | barren, off | 10.3 / 16% | 7.4 / 20% | 3.1 / 27% | −2.2 / 38% | −6.6 / 45% | −10.0 / 50% | 10.3 / 16% |
  | barren, on | 5.4 / 29% | −1.7 / 45% | −6.2 / 53% | −9.1 / 57% | −11.8 / 60% | −14.2 / 63% | **2.0 / 39%** |
  | barren, on, share 0.35 | 7.4 / 23% | 3.5 / 31% | −3.1 / 46% | −7.8 / 55% | −10.6 / 59% | −13.3 / 62% | 6.5 / 27% |

  (seed 0 at t0; seed 1 at 500 Myr is similar: modern on 9.8 → −16.0 → 6.0 °C, barren on
  5.2 → −16.0 → 2.4 °C.)

  - **No full snowball, even at 90%.** Snow strengthens the ice–albedo feedback: at 94–96% the
    snow-on planet is 7–12 K colder and has 17–30 points more ice. But it stops at 62–64% frozen
    with open tropical ocean. Without snow the planet stops at 50%. Voigt & Marotzke (2010)
    freeze an Earth with modern continents completely at ~91% S₀. So the model is, if anything,
    *less* snowball-prone than that GCM, both with and without snow.
  - **A second stable state.** Back at 100% S₀ the snow-on runs do not return to their warm
    state within 40 years: 3.6 instead of 11.0 °C, 35% instead of 17% frozen. Perennial snow
    keeps the high-latitude land bright, which keeps its summers cold. This is the bistability
    of a land snow/ice cover that glacial-inception studies find for the Northern Hemisphere
    (Calov & Ganopolski, 2005). It is physical in kind: perennial snow fields are incipient ice
    sheets. Its strength depends on the snow albedo and on the share of it that reaches the top
    of the atmosphere (0.5, perhaps high for cloudy polar regions). With 0.35, the barren sweep
    returns to within 1 K of its start.
  - **In the full runs** the carbon cycle (weathering slows under ice and cold) and the
    brightening Sun pull every cold start out within ~200 Myr. In the cold case without a carbon
    cycle, only the brightening Sun and the wearing-down of the highlands do it.

## Runtime

- **Local, same machine:** seed 0, 40 Myr, 6 subdivisions, both runs concurrently. Snow on took
  64.0 s against 59.2 s off (+8%; measured on `ed32175`, the fix does not change the cost). The snow update itself is small; the rest is the per-season
  albedo sums and the last-year diagnostics.
- **GitHub runners are too noisy to compare:** the same configuration took 635–1180 s depending
  on the runner batch.

## Caveats

- Without sublimation, perennial snow in cold deserts is an upper bound.
- The snow updates once per season (~15 days): an albedo lag of at most half a season, and the
  spring albedo is that of the season that just ended.
- The EBM constants (`land_albedo` 0.32, the radiation law) were calibrated in 2026-11 against
  Earth-like temperatures without seasonal snow. Part of the cooling is snow that the
  calibration already absorbed, but nothing was retuned.
- The latent heat of the glacier overflow is taken back on the cell it leaves, not where the ice
  would melt (at the coast or in the sea).
- The coarse temperature cells (~440 km) give one land temperature per cell. Snow lines follow
  elevation through the lapse rate on the fine cells, but not sub-grid slopes or aspect (#64).
- Overlap with #204 (cloud albedo): this change adds a surface term on top of `land_albedo`. If
  #204 changes the cloud masking, `surface_albedo_share` is the parameter where the two meet.

## Decisions for Sebastian

1. **Perennial snow as permanent ice** (on by default, `temperature.perennial_snow_ice`): it
   largely resolves #107 and stops weathering under the snow. Keep it on, or treat #107
   separately?
2. **Which rule owns permanent land ice.** The annual-mean ice-sheet rule (−10 °C, scaled by
   snowfall) and the snow mass balance now disagree on ~3–4% of land late, and on ~20% in cold
   states. A follow-up could let the snow mass balance (overflow above the cap) decide ice
   sheets and drop the −10 °C rule.
