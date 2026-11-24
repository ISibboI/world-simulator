# #97 Wind render mode: results

Plan: [plan.md](plan.md). An independent review of the diff found no bugs. Its wording
remarks are applied: the speed scale is "speed of the mean wind", and the cell row is
"Wind (year)".

## What the mode shows

**Render mode `wind`** (switcher label "Wind", `--render-mode wind`, `?render-mode=wind`):

- **Arrows**:
  - Placement: at the view's evenly spread arrow cells (icosphere 3, about 760 km apart).
  - Direction: they point where the air goes.
  - Length: proportional to the speed, full length at 8 m/s.
  - Legend: arrows for 2, 5 and 8 m/s.
- **Season**: Year, January or July. The arrows and the colours both follow it.
- **Colours**:
  - **Speed**: speed of the mean wind of the season, ColorBrewer YlOrBr, 0–10 m/s.
  - **Precipitation**: the moisture mode's scale.
  - **Temperature**: the temperature mode's scale.
  - All are unlit, with the coastlines drawn as lines.
- **Convergence zone** (switch, on by default): a line where the northward wind changes to
  southward within the trade winds (easterlies of at least 1 m/s, within 35° of the equator).
  This is the ITCZ, where the air rises and it rains.
- **Cell widget**: a new row "Wind (year)", e.g. "5.0 m/s from E". Like weather reports, it gives
  the direction the wind blows from.
- **Past frames**: everything comes from the cells every frame keeps, so past frames show alike.
  - Screenshot: [past frame, July](desktop-past-frame-july.png).
  - At 0.25 m/s quantisation, the convergence line of a past frame is a little more jagged.

## What the model has, and what it does not

The wind is the low-level wind of the water cycle (`climate::low_level_wind`). It is the wind that
carries the moisture and sets the evaporation (the evaporation also adds 4.5 m/s of gusts in
quadrature).

**The profile is a fixed zonal-mean table by "circulation latitude"**, i.e. the latitude measured
from the ITCZ:
- trade winds of −4 to −6 m/s with 0.5–0.6 m/s towards the ITCZ;
- calm at 30°;
- westerlies up to 8 m/s with a 0.3 m/s poleward drift;
- polar easterlies of −2 m/s.

**The ITCZ moves.** In each solved month (January, April, July, October), the ITCZ sits at the
moist-static-energy maximum in each of 24 longitude sectors. So it shifts with the seasons and
with longitude.
- Seed 0 at 20 Myr (subdivisions 5): the monthly mean ITCZ is at −13°, −9°, +14° and +10°.
- In July, the thermal equator of the temperature model, its first guess, reaches 25–30° N in some sectors.

**Not modelled:**
- winds steered or blocked by mountains;
- sea and land breezes;
- storms and day-to-day weather;
- wind speeds that respond to temperature gradients (the profile is fixed);
- cross-equatorial monsoon flow.

The pressure-driven monsoon winds (`thermal_wind_depth`, Lindzen–Nigam) exist but are **off by
default**: they made the climate worse at the model's coarse resolution (see the comment on
that parameter). With them on, the stored winds include them.

**Weaknesses seen.** They are added to #132:
- The wind belts are uniform bands. The westerlies are equally strong over land and sea and
  in summer and winter.
- The annual mean of a seasonally migrating ITCZ gives weak, partly cancelling trades near the
  equator. The "Year" view can show two convergence lines; see
  [desktop-year-double-convergence.png](desktop-year-double-convergence.png). Earth's annual mean
  shows the same averaging, but here it comes from the hard switch of the trades' meridional
  component at the ITCZ.
- The ocean gyres and the temperature model use a separate fixed equator-centred annual wind
  stress (`surface_wind_stress`), not these winds. The two can disagree where the ITCZ is far
  off the equator.

## Data and frame size

- **`WaterCycle`** gains:
  - `wind`: the annual mean of the solved months;
  - `monthly_wind`;
  - both as east/north in m/s per fine cell. They are not in the state hash; the simulation is
    unchanged.
- **`CellInfo`** gains 6 f32 (year, January, July × east, north): 84 → 108 B per cell on the wire.
- **Frame codec**:
  - a new 1-byte code `Signed8`: 0.25 m/s, ±31.75 m/s, `i8::MIN` is NaN;
  - 6 channels, 39 → 45 B/cell raw;
  - VERSION 5.

`frame_sizes` test, 5 subdivisions, 100 steps, second half:

| | before (#96) | after | change |
|---|---|---|---|
| quantised frame (raw) | 400 kB | 461 kB | +15 % |
| keyframe (lz4) | 225 kB | 265 kB | +18 % |
| delta frame | 106.6 kB | 109.5 kB | +2.7 % |
| **stored per frame** (1 key in 25) | **111.3 kB** | **115.7 kB** | **+4.0 %** |

- **Per-channel deltas** (east/north): year 1.3 / 0.2 kB, January 0.75 / 0.2 kB, July
  0.6 / 0.15 kB. Each update refreshes one month of four, and the winds change only where the
  ITCZ moves.

## Code reuse

- **Shared with the ocean-current mode:**
  - the conversion from east/north to world vectors (`arrows::tangent_vector`, `east_north`);
  - per-cell arrow sampling (`ModeData::tangent_samples`);
  - `ArrowStyle`, `ArrowScale`, `isolines` and `coastlines`.
- **Shared `Period`** (Year/January/July) for the temperature, moisture and wind modes. The
  wind mode reuses their colour scales.
- **No streamlines.** #135's streamlines are isolines of the gyres' streamfunction. The wind is
  divergent and has no streamfunction.

## Checks

- With `CARGO_BUILD_WARNINGS=deny`:
  - fmt;
  - clippy (all targets) native and wasm32;
  - doc with `-D warnings`;
  - tests: 358 pass. New tests cover the wind components, the mode, the cell row and the codec.
- **Desktop** (vscreen):
  - [year, speed](desktop-year-speed.png);
  - [July, precipitation](desktop-july-precipitation.png);
  - [July, temperature with the cell row](desktop-july-temperature-cell-row.png);
  - [past frame](desktop-past-frame-july.png).
- **Web at phone width** (390 px), trunk release build:
  - [planet](web-phone.png): panels collapsed;
  - [legend](web-phone-legend.png): touch-sized buttons from the shared UI.
