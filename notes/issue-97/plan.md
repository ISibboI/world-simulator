# #97 Wind render mode: plan

Stacked on #135 (#96 ocean currents) → #131 (#95 moisture); both change the frame format.

## What the model has (`src/climate.rs`)

- The water cycle's **low-level wind** per cell and per solved month (`ClimateParams::months`,
  default 4: January, April, July, October): `low_level_wind(position, local_equator)` plus
  `thermal_wind` (off by default: `thermal_wind_depth = 0`).
- `low_level_wind` is a fixed zonal-mean table by *circulation latitude* (latitude measured from
  the ITCZ, stretched so the poles stay at ±90°): trades (−4…−6 m/s eastward, 0.5–0.6 m/s
  towards the ITCZ), calm at 30°, westerlies up to 8 m/s with a 0.3 m/s poleward drift, polar
  easterlies −2 m/s.
- The ITCZ (`local_equator`) is, per month, the latitude of the moist-static-energy maximum in
  24 sectors of longitude (between 30° S and 30° N), interpolated linearly between sectors. So
  the wind belts shift with the seasons and bend poleward over summer continents.
- Used for: moisture advection, evaporation and PET (speed with 4.5 m/s gustiness in
  quadrature). The ocean gyres and the temperature model use a separate fixed annual analytic
  wind stress (`surface_wind_stress`, equator-centred).
- Not modelled: topographic steering/blocking, land–sea breezes, pressure-driven monsoon
  circulation (unless `thermal_wind_depth` is set), synoptic weather, wind speed changes with
  the temperature gradient (the profile is fixed).

## Data

- `WaterCycle` gains `wind` (annual mean of the solved months) and `monthly_wind`, as
  (east, north) m/s per fine cell. Not in the state hash; simulation unchanged.
- `CellInfo` +6 f32 (annual, January, July × east, north): 84 → 108 B per cell on the wire.
- Frame codec: new 1-byte `Signed8(step)` code, 6 channels at 0.25 m/s (±31.75 m/s; winds reach
  8 m/s, 18 with thermal winds): 39 → 45 B/cell raw, VERSION 5. Measured with `frame_sizes`.

## View: render mode `wind`

- Options: Season (Year / January / July, shared `Period` with the temperature and moisture
  modes), Colours (Speed / Precipitation / Temperature of that season), Convergence zone
  (switch, default on).
- Arrows at the view's evenly spread arrow cells (icosphere 3), length ∝ speed, full at 8 m/s;
  legend arrow scale 2/5/8 m/s. Reuses #135's building blocks; the east/north → world-vector
  conversion and per-cell arrow sampling move from the ocean-current mode into shared code
  (`arrows::tangent_vector`, `ModeData::tangent_samples`).
- Convergence zone line: isoline of the northward wind = 0 within the trade winds (eastward
  ≤ −1 m/s, |lat| < 35°), only in triangles where the air converges.
- Cell widget row "Wind": annual mean speed and the direction it blows from.
- No streamlines: #135's streamlines are isolines of the gyres' streamfunction; the wind is
  divergent, has none.
