# #94 Temperature render mode: plan

Issue: https://github.com/ISibboI/world-simulator/issues/94 — surface temperature as colours of a
perceptually uniform diverging scale centred near 0 °C, legend in °C; annual mean by default,
optionally a month/season (January/July or a month slider); optionally the −10 °C permanent-ice
threshold as a contour. Builds on the mode registry (#91/#105), frame history (#109/#115) and the
overlay/legend pieces of the plates mode (#93/#118).

## Data

The frame already keeps annual, January and July temperatures per cell at 0.1 °C (`CellInfo`,
`history::codec`, 3 × 2 bytes). The mode uses only these, so **no frame-size change** and past
frames work as they are. A month slider would need the other ten months: +10 bytes/cell at a
1-byte offset from the annual mean (0.5 °C steps), i.e. 25 → 35 bytes/cell raw (+40 %), or +20
bytes at 0.1 °C. Not done; offered to Sebastian as a decision.

## Mode (`src/viewer/modes/temperature.rs`)

(Line colours and texts changed after the review; see results.md.)

- Colours: ColorBrewer **RdBu** (11 classes; colour-blind safe, diverging, monotone lightness on
  each arm), white at 0 °C; 10 °C per class on the cold arm (−50 … 0) and 8 °C on the warm arm
  (0 … +40), so that both ends are used (simulated ranges: annual −30…+34, Jan/Jul −50…+36 °C on a
  fresh planet). Values beyond the ends clamp. One fixed scale for all periods, so January and
  July are comparable.
- Unlit (colours match the legend), with **coastlines** drawn as thin dark lines (generic helper
  `modes::coastlines`, for all scalar modes to come) so land and sea stay readable.
- Options (new generic mechanism, below): *Mean of* Year / January / July; *0 °C isotherm*
  (white line, of the shown period); *Ice-sheet threshold* (cyan line on land only, annual mean
  −10 °C = `land_ice_temperature`, around which the model grows ice sheets given enough snowfall;
  over the ocean the threshold does not apply, sea ice has its own model, so the line ends at
  coasts).
- Legend: scale titled by period, swatches for the lines that are on, a description.

## Generic infrastructure

- `RenderMode::options() -> &'static [ModeOption]` (label, hover text, `OptionKind::Choice(&[..])`
  or `OptionKind::Switch { default }`), values as `OptionValues` (one `u8` per option),
  `ModeData::options`, and `legend(options, data)`. The switcher keeps the values of every mode
  (kept while another mode is shown) and shows the controls of the active one above the legend
  (selectable buttons / checkbox; touch sizes come from the global touch style). A change marks
  the display outdated (colours, overlay and legend are rebuilt from the displayed frame; no
  simulation request).
- `overlay::isolines(faces, value, level, position)`: marching triangles with linear
  interpolation along edges; NaN corners leave the triangle out (to restrict a line to land,
  etc.). Reusable for any scalar mode (precipitation, crust age, …).

Not done: remembering option values in the URL/local storage (only the mode is remembered).

## Checks

Unit tests: scale monotone and diverging; options → period/legend; a planet in the mode (colours
match the scale, overlay contains coastlines, isotherms add lines); isolines on a sphere follow
z = const with the right length and stop at NaN. GUI: desktop and web (vscreen), past frames,
narrow/phone layout.
