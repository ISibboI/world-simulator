# #95 Moisture render mode: plan

Issue: https://github.com/ISibboI/world-simulator/issues/95 — precipitation as colours of a
sequential scale with a legend; optionally an aridity sub-mode (P − E or an aridity index) so that
deserts and wet regions read clearly; optionally seasonal precipitation. Builds on the mode
registry (#105), frame history (#115), overlay/legend pieces (#118) and the mode options,
`overlay::isolines` and `modes::coastlines` of the temperature mode (#130, stacked on it).

## What the model provides (src/climate.rs, `WaterCycle`)

- Annual precipitation (m/yr), already in every frame (`CellInfo::precipitation`, 1 mm/yr).
- Precipitation of every solved month (`monthly_precipitation`; by default 4 months: January,
  April, July, October; `climate.months` may be 1, 2, 3, 4, 6 or 12). January is always solved,
  July whenever the count is even.
- Annual potential evaporation of land (`potential_evaporation`; the bulk formula at the land
  temperature with the humidity of the air above; zero over the ocean), the same PET the
  Budyko-curve evapotranspiration uses. Actual evaporation also exists (ocean + land).

So the UNEP aridity index **AI = P / PET** can be computed on land from data the model already
has; it is consistent with the model's own runoff (Budyko: AI < 1 is water-limited, most rain
evaporates; AI > 1 energy-limited). Over the ocean PET is not computed, so the aridity view
shows the ocean in one neutral colour.

## Data (frames)

New `CellInfo` attributes (f32): `precipitation_january`, `precipitation_july` (NaN if the month
is not solved), `potential_evaporation` (m/yr, 0 over the ocean). Wire size 56 → 68 bytes/cell.
History codec: three `Unsigned(0.001)` channels (1 mm/yr, as the annual precipitation),
25 → 31 bytes/cell raw (+24 %); codec version 2 → 3. The compressed per-frame size before/after
is measured with the existing `frame_sizes` test and reported in results.md. (Alternative: store
only P/PET in 1 byte; rejected: PET is the physical quantity, also useful in the cell widget,
and AI derives from it.)

Cell widget: precipitation row gains January/July ("850 mm/yr (Jan 40 / Jul 120)"? or a row of
its own) and a new row "Aridity" (P/PET with its UNEP class) on land.

## Mode (`src/viewer/modes/moisture.rs`, `--render-mode moisture`)

- Options: **Show** Precipitation / Aridity; **Precipitation of** Year / January / July (the
  aridity index is always annual); switch **Desert edge**: isoline on land at AI = 0.2 (UNEP
  arid / semi-arid boundary), drawn in both views.
- Precipitation colours: ColorBrewer **YlGnBu** (sequential, colour-blind safe, monotone
  lightness), on a **logarithmic** axis (log10 of mm/yr; precipitation spans 3 orders of
  magnitude, and equal ratios matter more than equal differences): about 30 mm/yr (pale yellow)
  to 5000 mm/yr (dark blue), values beyond clamp. Ticks 50, 100, 200, 500, 1000, 2000, 5000 —
  exact range after measuring the simulated distribution (annual and Jan/Jul, land and ocean).
  One fixed scale for all periods, so months compare. Land and ocean alike.
- Aridity colours: ColorBrewer **BrBG** (diverging, colour-blind safe), log10(AI), neutral at
  AI = 1 (P = PET), brown = dry, teal = wet; about 0.02 … 5 (to be checked against the data);
  ticks at the UNEP class limits 0.05 (hyper-arid), 0.2 (arid), 0.5 (semi-arid), 1 and wetter.
  Ocean in a neutral grey with a legend swatch.
- Unlit (colours match the legend) with `modes::coastlines`, as the temperature mode.
- Legend: scale with option-dependent title, swatches (ocean in aridity view, no data, coast,
  desert edge), description (what P/PET means, that polar land with little rain still counts as
  humid by this index because PET is tiny there).

## Checks

Unit tests: scale monotone in lightness; options → legend; a planet in the mode (colours match
the scale, overlay, desert line adds lines); codec round trip of the new channels; cell rows.
Measure the value distributions (a test printing quantiles) to set ranges. GUI: desktop and web
(vscreen), past frames, phone width.
