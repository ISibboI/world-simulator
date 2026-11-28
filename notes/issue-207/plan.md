# #207 plan: seasonal snow on land

## Goal

Winter snow on land that melts in spring has no albedo effect today. Land is either snow-free
(`land_albedo`, planetary 0.37 barren / 0.32 modern) or under an ice sheet (`ice_sheet_cover`:
annual mean below −10 °C, scaled by snowfall / 0.3 m/yr). Add a seasonal snow pack on land that
accumulates, melts, and brightens the land in the energy balance season by season, keeps the
water budget closed, and check the snowball risk across seeds and epochs.

Project goal: an Earth-like planet, not Earth. Physical plausibility, conservation and plausible
ranges; Earth numbers are orientation. Coarse-grid artefacts (~110 km cells, temperature model on
~440 km coarse cells) are accepted (#64).

## How the model handles snow and ice now (`src/temperature.rs`)

- Energy balance on a coarse icosphere (4 subdivisions, 2562 cells), 24 seasons × 8 substeps per
  year, 20 spin-up years, 2 years per update. Every coarse cell has a land surface (one sea-level
  temperature), an ocean column and a lake column.
- The land albedo of a coarse cell is **constant for the whole year**: computed once per year from
  last year's annual mean land temperature, per fine land cell (lapse rate, free-air shift) through
  `ice_sheet_cover`, averaged over the coarse cell's fine land cells, blended between
  `land_albedo` and `ice_albedo` (0.62). All albedos are **planetary** (top of atmosphere,
  including clouds).
- Sea and lake ice: Wagner & Eisenman columns with their own seasonal albedo.
- Precipitation: only the annual field (m/yr, per fine cell, from the last water-cycle update)
  reaches the temperature model; the climate also has 4 solved months (Jan, Apr, Jul, Oct).
- `fields.ice` (permanent ice, 0–1) feeds weathering (`1 − ice`), Köppen EF, the frozen fraction
  statistic and the view (ice where > 0.5).

## Model

A snow water equivalent `S` (m) per **fine land cell** (not lakes, not ocean), state of the
temperature model, updated **once per season** (24 per year, ~15 days) at the end of the season
from that season's mean land temperature of the fine cell,
`T = interpolate(season mean of coarse land T) + free-air shift − lapse · elevation` (the same
formula as the monthly output temperatures).

1. **Snowfall** (smooth rain/snow split). Daily temperatures scatter around the season mean with
   a normal distribution of σ = 5 K (as in positive-degree-day models; Reeh 1991, Calov & Greve
   2005). Precipitation falls as snow on days below the 50% rain/snow threshold `T_s` = 1 °C
   (Jennings et al. 2018: global median ~1 °C). Snow share `= ½ erfc((T − T_s)/(√2 σ))`.
   Season precipitation = annual precipitation × seasonal weight, the weight from a periodic
   linear interpolation of the climate's solved months, normalised so the 24 seasons sum exactly
   to the annual precipitation (uniform when the months are missing; 1 m/yr when the model gets
   no precipitation at all, as the ice sheets assume).
2. **Melt** by positive degree days with the same σ (Calov & Greve 2005):
   `PDD = days · (σ/√(2π) e^(−T²/2σ²) + T/2 · erfc(−T/(√2 σ)))`, melt = `DDF · PDD` with
   DDF = 4 mm w.e./K/day (snow: Braithwaite 1995 ~3, Hock 2003 review 2.5–11.6, mostly 3–6),
   at most the snow present plus the season's snowfall.
3. **Cap** at 1 m w.e.: snow beyond it is firn that turns into glacier ice and flows off, counted
   as runoff (overflow). Without sublimation (not modelled), snow on land whose warmest season stays
   below freezing accumulates to the cap: perennial snow.
4. **Cover fraction** `c = tanh(S / 0.01 m)` (ECHAM-type snow cover for flat land; Roesch et al.
   2001). Grid-scale sub-grid topography is ignored (#64).
5. **Surface albedo of snow** ages with temperature (Robock 1980; Roesch et al. 2001): fresh
   0.85 at season temperatures ≤ −10 °C, aged/melting 0.60 at ≥ 0 °C, linear between; the
   temperature of the season that just ended sets the next season's value.
6. **Planetary albedo.** The existing presets turn surface albedo into planetary albedo with
   `∂α_p/∂α_s ≈ 0.5` (Donohoe & Battisti 2011; Qu & Hall 2005 find 0.3–0.6 over snow regions).
   Snow on a fine cell raises its planetary albedo by `c · 0.5 · (α_snow,eff − α_land,surface)`,
   with the snow-free surface albedo of land `α_land,surface` = 0.20 modern / 0.30 barren (the same
   values the abiotic preset already argues with). Barren: fresh snow → planetary ~0.64, aged ~0.52
   (ice sheets 0.62).
7. **Forest masking, `--biosphere modern` only** (Betts & Ball 1997: snowy boreal forest
   0.15–0.3 vs > 0.7 for snowy open land). The model has no vegetation map, so a forest share
   comes from the climate: trees where the warmest season of the previous year reaches ~10 °C
   (treeline; ramp 8–12 °C) and precipitation is ≥ ~0.3 m/yr (ramp 0.1–0.3), times a maximum
   forest cover of 0.7 (`snow_forest_cover`, 0 for barren). Under forest the effective snow albedo
   is 0.25. Barren land has no masking.
8. **Combination with ice sheets.** Per fine cell, the old ice-sheet blend stays as it is; snow
   acts on the ice-free part: coarse land albedo = old value + mean over fine land cells of
   `(1 − ice) · c · Δα`. With snow off the old value is used unchanged (bit-identical to main).
9. **Temporal resolution.** The land albedo becomes seasonal (24 per year): the albedo of season
   `s` uses the snow at the end of season `s − 1` (half a season of lag). Ice sheets stay annual.
10. **Not included (caveats):** latent heat of melt is not taken from the land energy balance
    (degree-day model; melt does not delay spring warming); no sublimation; no snow on lake ice
    or sea ice beyond their existing albedos; no snow-depth effect on soil/runoff timing (the
    hydrology is annual).

## Water budget

Snow is stored precipitation. The climate's runoff is annual `P − ET` (Budyko), so seasonal
storage that melts within the year does not change it; overflow above the cap is glacier
discharge that the annual runoff already contains. The temperature model keeps a snow budget per
update (snowfall, melt, overflow, change of storage, snow dropped from cells that stop being land)
and a unit test checks `snowfall − melt − overflow − dropped = Δ storage` to rounding. The stored
volume is reported (km³, sea-level equivalent); it is far below the ocean volume and is not taken
out of the sea.

## Permanent ice and #107

Snow that stays all year (the minimum cover over the last year's seasons) is permanent snow,
which over a 1 Myr step is firn and glacier ice. `fields.ice` on land becomes
`max(ice_sheet_cover, perennial snow cover)`, so cold dry land whose warmest month stays below
0 °C counts as ice-covered once any snow falls (#107's comment: any positive mass balance builds
ice). This changes weathering (`1 − ice`), the frozen fraction and the view. The albedo itself is
not double counted: the ice-sheet part keeps `ice_albedo`, perennial snow outside it has the snow
albedo. A log line reports #107's number (share of land with warmest month < 0 °C whose ice < 0.5)
with snow on and off.

## Parameters (`TemperatureParams`, `--set temperature.NAME=VALUE`)

`seasonal_snow` (1/0), `snow_fresh_albedo` 0.85, `snow_aged_albedo` 0.60,
`snow_cold_temperature` −10, `land_surface_albedo` 0.20 (barren preset 0.30),
`surface_albedo_share` 0.5, `snow_forest_albedo` 0.25, `snow_forest_cover` 0.7 (barren preset 0),
`rain_snow_temperature` 1.0, `daily_temperature_sd` 5.0, `degree_day_factor` 0.004 m/K/day,
`snow_cover_depth` 0.01 m, `max_snow` 1.0 m. `erfc` goes into `src/math.rs` as a composite of
`math::exp` (Numerical Recipes `erfcc`, relative error < 1.2e-7), deterministic with `--web-math`.

## Interface

`TemperatureModel::update` gets the climate's monthly precipitation (`&[Vec<f64>]`, empty for
none). New fields: `monthly_snow` (12 × fine, mean cover per month, 0 off land), a `SnowBudget`
(snowfall, melt, overflow, dropped, storage; global mean of the last year) and
`snow_radiative_effect` (global mean W/m² of sunlight reflected by snow in the last year).
`PlanetStatistics` gets snow cover of NH/SH land in Jan/Apr/Jul/Oct, perennial snow share, the
radiative effect and the #107 numbers; one new log line. The viewer is not changed (the view of
`ice` shows perennial snow); no new UI.

## Cost

Per season ~12k fine land cells × (one 7-point interpolation + 2 exp) ≈ 0.3 M operations per
year: small against the substep loops. Expected runtime change < 3%.

## Evaluation

- Unit tests: water budget closes; snow accumulates in a cold winter and is gone in a warm summer
  on a continent; snow brightens high-latitude land in winter and cools it; snow off is
  bit-identical to the old model; `erfc` against reference values.
- Runs from the branch commit (snow on, default) and with `--set temperature.seasonal_snow=0`
  (= main physics, checked bit-identical by state hash on a short run): default barren 8 seeds ×
  1000 Myr; `--biosphere modern --epoch 4` 4 seeds; snowball sweep: `--epoch 4` (barren today)
  4 seeds, and a cold case `--epoch 4 --co2 0.5 --no-carbon-cycle` 4 seeds (snow on/off).
- Offline solar sweep (`solar_sweep` test on the 6-subdivision cell dumps of main, issue-65
  runs) with snow on and off: the snowball threshold and hysteresis against literature
  (Voigt & Marotzke 2010: modern continents freeze at ~91% S₀; EBMs ~2–10% below S₀).
- Report: snow cover by season, the snow albedo effect (W/m²), global T and ice, CO₂ and φ,
  #107 numbers, snowballs, runtime. Results in `notes/issue-207/results.md`, an independent
  evaluation in `notes/issue-207/review-results.md`.

## Overlap

#204 (abiotic cloud nuclei → cloud albedo) changes the planetary albedo of clouds. This plan only
adds a land-surface snow term on top of `land_albedo`; if #204 changes `land_albedo` or the
cloud contribution, `surface_albedo_share` (the cloud masking of surface changes) is the place
where the two meet.

## Risk from the history

The first temperature model (2026-11-15, `notes/sessions/2026-11-15.md`) fell into a snowball
(equator 4 → −32 °C) when the land albedo reacted directly to the seasonal temperature, and was
switched to the annual-mean ice criterion. That model had no ocean columns, sea ice physics or
gyres and switched albedo instantly with temperature. The snow pack here is mass-based (needs
snowfall, melts by degree days), so snow only covers land that gets snow, and ages to 0.6 when
melting; still, the runs must show whether the feedback runs away. If it does, the report
compares it with literature thresholds before anything is changed (no tuning away).

## Changes after the plan review (`review-plan.md`)

1. **Latent heat (must-fix 1): implemented, not a caveat.** Freezing the season's snowfall
   releases its heat of fusion to the land (the air column, where the snow forms), melting takes
   it back, each spread over the following season as a flux on the coarse cell's land
   (`snow_latent_heat`, on). Over a year of seasonal snow it moves heat from winter to spring
   and conserves energy; melt now delays the spring warming. Glacier overflow takes its latent
   heat away unreleased (small). `--set temperature.snow_latent_heat=0` for the sensitivity run.
2. **Fresh snow vs ice sheets (must-fix 2):** fresh snow 0.80 surface, which with the same
   conversion (share 0.5) is exactly `ice_albedo` 0.62 for both presets; aged 0.60.
3. **Origin of `land_albedo` 0.32 (must-fix 3):** set in the first temperature model
   (commit 87f3337) with the other constants of the EBM, which was calibrated against Earth's
   temperatures without seasonal snow. So a snow-on cooling partly double counts snow that the
   calibration absorbed; this is reported, nothing is retuned.
4. **Perennial snow → ice (must-fix 4):** only where the snow survives every season of the last
   year *and* the year's balance is not negative (end ≥ start; at the cap the excess is glacier
   flow); behind its own switch `perennial_snow_ice` (on). New fields `ice_sheet` and
   `perennial_snow`; the log reports the land under one rule but not the other.
5. **Evaluation (must-fix 5):** solar sweep with the seasonal precipitation of the cell dumps
   (January and July), snow on/off, threshold compared to ~91% S₀ (Voigt & Marotzke 2010;
   > 95% would point to an artefact); a sensitivity set on the coldest case
   (`surface_albedo_share` 0.35, latent heat off, perennial ice off); snapshots and log lines
   every 50 Myr track the onset (perennial share, frozen fraction, snow cover by month) of any
   run that freezes; Earth orientation for the modern case (NH January snow cover ~45–50% of
   NH land, August a few %).
   A per-substep snow update (192/yr) would cost ~0.5 s per step; not done. The once-per-season
   update keeps at most half a season of lag.
6. Cover `tanh(S/0.01)` without ECHAM's 0.95 factor, which stands for vegetation and roughness
   elements sticking out; on bare land the full cover is reachable, and perennial snow can then
   count as full ice.
