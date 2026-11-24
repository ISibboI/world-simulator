# #136 Moisture mode: P − E — plan

Issue: https://github.com/ISibboI/world-simulator/issues/136 (asked for on #131). Add **P − E**
(precipitation minus actual evaporation, mm/yr) as a third "Show" choice of the moisture mode
(#95, `src/viewer/modes/moisture.rs`).

## What evaporation the model has

- `climate::WaterCycle::evaporation` (annual, m/yr): over the ocean the bulk aerodynamic formula
  at the SST; on land the Budyko (Fu) evapotranspiration from the annual P and PET, spread over
  the months in proportion to each month's PET. It returns to the air column, so the water
  cycle's finite-volume budget gives Σ P = Σ E over the planet (each month).
- Each solved month keeps its own evaporation (`SolvedMonth::evaporation`, kg/m²/s), so
  January/July P − E exist (export them as `WaterCycle::monthly_evaporation`; export only, the
  simulation does not change).
- Lakes: the river model (`erosion.rs`) evaporates lake water at the PET in excess of the land's
  ET and takes it from the runoff; it is a basin total only (no per-cell field) and is not given
  back to the air. So it is not in the shown E; the legend says so.

## Data

`CellInfo` gains `evaporation`, `evaporation_january`, `evaporation_july` (f32; NaN where the
month is not solved): 84 → 96 bytes. History codec: one byte each on a square-root axis up to
8 m/yr (new `Code::Root`; first 5 m/yr, raised after monthly land rates of ~6 m/yr showed up
after 50 Myr), fine near zero where land P − E is small; codec version 4 → 5.
Compare against 2-byte 1 mm/yr channels with the `frame_sizes` test.

## Mode

- Show: Precipitation / Aridity / **P − E**; the Season option (Year/January/July) applies.
- Colours: ColorBrewer BrBG (diverging, colour-blind safe; same meaning as the aridity view:
  brown loses water, blue-green gains), white at 0, on a signed logarithmic axis
  `sign(x)·log10(1 + |x|/10 mm/yr)`, ±3000 mm/yr, few ticks (phone width).
- Legend text per season; "No data" swatch where a month is missing.
- Cell widget row "P − E": annual value with the evaporation, e.g. "+312 mm/yr (evap. 500)".

## Checks

Unit tests (scale symmetric, monotone, lightness; P − E of cells; codec round trip/clamping;
rows). Distribution and global sum (area-weighted, from the frame values) via the ignored
`moisture_distribution` test; frame sizes; GUI on desktop and web at phone width, past frames.
