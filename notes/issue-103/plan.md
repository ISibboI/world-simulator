# #103 Climate-classification render mode (Köppen–Geiger): plan

## Data the model has
- Temperature: 12 monthly mean surface temperatures per fine cell (`TemperatureFields::monthly`,
  month 0 = January, land with lapse rate), annual mean, ice cover.
- Precipitation: the water cycle solves only `climate.months` months (default 4: Jan, Apr, Jul,
  Oct; `WaterCycle::monthly_precipitation`, rates in m/yr); the annual precipitation is their mean.

## Classification (simulation side, export only)
- New pure module `src/koppen.rs`: `KoppenClass` (the 30 classes of Beck et al. 2018, `u8` ids
  1..=30, code, name, main group) and `classify(temperature: [f64; 12], precipitation: [f64; 12])`
  with Beck et al. (2018, Table 1) thresholds (0 °C C/D boundary; E before B; B before A/C/D;
  s/w/f, a/b/c/d; Pthreshold = 2·MAT (+14, or +28 if ≥ 70 % of rain in summer, +0 if ≥ 70 % in
  winter); summer = the warmer of AMJJAS / ONDJFM, so it works on either hemisphere).
- Missing months of precipitation: periodic **linear interpolation** between the solved months
  (monthly amount = rate / 12). It keeps the annual total exactly (each solved month weighs the
  same) and never invents extremes: the driest/wettest month is always a solved month. With
  1 solved month the year is uniform. Caveat: with 4 solved months, the seasonality (driest
  month, s/w, Am) is smoothed, so Af/Am/Aw and s/w/f splits are approximate.
- Land under an ice sheet (ice ≥ 50 %, as in the moisture mode) is EF. Ocean is not classified.
- `cell_infos` (simulation thread / worker) adds `CellInfo::climate_class: u32` (0 = none:
  ocean or no data). The state hash is not touched (pure read); check against main.
- Frame history: one new `Integer8` channel (1 byte/cell), codec version bump.
- Snapshot log: one `info!` line with the land shares of the main groups (and top classes), for
  the plausibility check with `runs` over several seeds.

## Viewer
- Mode `climate` ("Climate classes"), unlit, coastlines (as moisture/temperature).
- Colours: Beck et al. (2018) standard table. Option "Detail": *Classes* / *Main groups*
  (A/B/C/D/E in Okabe–Ito-like colour-blind-safe colours) because the standard table is not
  colour-blind safe (Cfa/Cfb/Cwa greens, Dfb/Dwb blues, Csa/BSk yellows).
- Optional: class borders (category_boundaries) as thin lines (option "Class borders", off).
- Legend: one swatch per class present in the displayed frame ("Cfb — temperate oceanic"),
  sorted by id, plus ocean and coastline.
- Cell widget: row "Climate" with "Cfb — temperate oceanic" (land), — on the ocean.
- Works on past frames (the class is in the frame). No month slider.

## Tests
- Classifier unit tests against station climates (e.g. Singapore Af, Miami Am, Bamako/Darwin Aw,
  Cairo BWh, Riyadh/Ashgabat BWk, Niamey BSh, Denver BSk, Rome Csa, London Cfb, Reykjavík Cfc,
  Hong Kong Cwa, Moscow Dfb, Yakutsk Dfd, Beijing Dwa, Barrow ET, Vostok EF) and interpolation
  tests (annual total preserved, solved months kept).
- Codec round trip, mode legend/colours on a small planet.
