# #94 Temperature render mode: results

Plan: [plan.md](plan.md). An independent review of the change was done before the PR; its points
and what was done about them are at the end.

## What changed

- **New render mode "Temperature"** (`--render-mode temperature`, `?render-mode=temperature`,
  the switcher): surface temperature (land surface or sea surface) per cell, unlit so the colours
  match the legend, with the **coastlines** drawn as thin dark lines.
  - Colour scale: ColorBrewer **RdBu** (diverging, colour-blind safe, lightness falls steadily on
    each arm), white at 0 °C; −50 … 0 °C in 10 °C classes, 0 … +40 °C in 8 °C classes (the arms
    are asymmetric so that both ends are used: annual means reach about −30 °C, January/July
    −50 … +36 °C; so −10 °C has the lightness of about +8 °C). Values beyond the ends are
    clamped. One fixed scale for all periods, so January and July can be compared.
  - Options: **Mean of Year / January / July** (January = the month starting at perihelion,
    northern winter); **0 °C isotherm** (of the shown period, dark line with a white border);
    **Ice-sheet threshold**: on land, where the annual mean is −10 °C
    (`land_ice_temperature`; with ≥ 0.3 m/yr of precipitation land there is half covered by ice
    sheets, partly from −5 °C, fully below −15 °C), purple line with a white border. Both off by
    default.
  - Legend: the scale titled by period, the coastline and the lines that are on, "No data" grey
    if a cell has no temperature.
- **Generic pieces for the modes to come** (moisture, wind, currents, crust age, …):
  - **Mode options**: `RenderMode::options()` lists `ModeOption`s (a choice shown as a row of
    buttons, or a switch shown as a checkbox; label and hover text); values (`OptionValues`, one
    byte each) reach the mode in `ModeData::options` and `legend(options, data)`. The switcher
    keeps the values per mode (kept while another mode is shown) and shows the active mode's
    controls at the top of the legend panel. Changing one rebuilds colours, overlay and legend
    from the displayed frame (no simulation request), so it works on past frames too.
  - **`overlay::isolines`**: contour lines of any per-cell value (marching triangles, linear
    interpolation along the mesh edges; NaN corners leave a triangle out, e.g. to restrict a line
    to land).
  - **`modes::coastlines`**: coastlines for every unlit scalar mode.
  - Legend line swatches in a dark colour get a light border (visible on the dark panel).

## Frame size

None. The frame already keeps annual, January and July temperatures at 0.1 °C (3 × 2 bytes per
cell); the mode reads only those, so past frames show everything the live view does.

A **month slider** (the issue's alternative) would need the ten other months: as a 1-byte offset
from the annual mean (0.5 °C steps) +10 bytes per cell (25 → 35 raw, +40 %; delta frames
compress well since temperatures change slowly), or +20 bytes at 0.1 °C. Not done — a decision
for Sebastian.

## Checks

- Unit tests: the scale is diverging around 0 °C with monotone lightness on both arms; options
  choose the period and the legend; a planet in the mode (colours equal the scale's colour of
  the chosen period for every cell; the overlay has coastlines; isotherms add lines);
  `isolines` on an icosphere for z = 0.3 lie exactly on z = 0.3, total length within 5 % of the
  circle, and stop at NaN cells.
- CI-equivalent locally with `CARGO_BUILD_WARNINGS=deny`: fmt, clippy (native and wasm32,
  all targets and features), all 333 tests, doc.
- GUI (vscreen; the desktop screenshots predate the review changes: white 0 °C and cyan ice lines, thinner coastline): desktop at 5 subdivisions ([annual](desktop-annual.png),
  [January with both lines](desktop-january-lines.png),
  [a past frame (29 of 30 Myr) in July](desktop-past-frame-july.png)); web build at 4
  subdivisions ([January with both lines](web-january-lines.png),
  [phone width with the legend panel open](phone-legend-options.png)).

## Caveats

- **Coarse grid**: coastlines step along the cells (~110 km at 6 subdivisions, ~220 km at 5 on
  the web). The land and ocean temperatures come from separate fields of the energy-balance model,
  so winter 0 °C isotherms often run right along a coast. The ice-sheet line uses only triangles
  whose three cells are all land, so it ends up to one cell short of the coast and skips land one
  cell wide.
- The ice-sheet line uses the default threshold (−10 °C); `--set temperature.land_ice_temperature=…`
  does not move it.
- On phones the legend panel starts closed, as before; the mode's options are inside it (the
  "Legend" button opens both).
- Option values are not remembered in the URL or local storage (only the mode is).
- An option cannot yet change which layers a mode requests from the simulation (`layers()` takes
  no options); this will matter for e.g. a wind mode with a level option. It is easy to add then.

## Review

An independent review found no correctness bugs (option values/mode matching, isoline maths, NaN
handling, other modes unchanged). Its points and what was done about them:
- The −10 °C text said "ice sheets grow below": changed to the 50 % point with the −5/−15 °C
  ramp; the hard-coded parameter is documented.
- The 0 °C line was white on the white of 0 °C, and the ice line cyan on pale blue: both are now
  dark lines with a white border.
- The coastline could be under a pixel wide on small screens: widened (0.018 → 0.03 of the arrow
  spacing, about a third narrower than the plate boundaries).
- "No data" swatch added.
- Options hidden while the legend is closed (phones), options addressed by position (a stable
  key will be needed to remember them), options cannot change layers: left as caveats above.
