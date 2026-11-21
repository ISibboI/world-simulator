# Issue #72: cell information widget and outline: results

Design: [plan.md](plan.md).

## What it shows

Bottom-left widget "Cell" (the parameter panel stays top right): position (lat/lon), elevation
or depth relative to sea level, land/ocean (with ice sheet / sea ice), plate, crust type
(continental/oceanic) and thickness of the crystalline crust, ocean floor age (lithosphere age
on continental crust; "old (≥ 1000 Myr)" for the nominal age of old lithosphere), sediment
(rock-equivalent thickness), orogeny (deformed share of the lithosphere and time since the
last orogeny, where there was one), annual mean temperature, January/July temperature,
precipitation, permanent ice (where present), river discharge in m³/s (land cells with flow),
and the cell index. A frame (light line with a dark border) outlines the cell on the planet.

Use: desktop and web with a mouse: point at the planet; click to keep a cell (shown while the
pointer is elsewhere, e.g. on the widget); click beside the planet to clear. Touch: tap a cell;
drag rotates; tap beside the planet clears. The widget closes to a button that shows position
and elevation of the shown cell (the default on screens narrower than 600 pt, like the panel).

## Cost

Measured in the desktop viewer (release, llvmpipe rendering competing for the CPU):

| subdivisions | cells | `surface_vertices` | `cell_infos` | update size (vertices + cells) |
| --- | --- | --- | --- | --- |
| 5 (web default) | 10,242 | 1.2–1.3 ms | 0.46 ms | 0.41 + 0.57 MB |
| 6 (desktop default) | 40,962 | 5.5–8.6 ms | 2.3–3.9 ms | 1.64 + 2.29 MB |

A simulation step takes ~0.2 s (5 subdivisions) to seconds (6), so the extra time is < 1–2% of
a step. The view keeps the latest update only. Per frame, picking (when the pointer moved over
the planet) is a march of up to ~100 short walks, and the outline is ≤ 24 instanced segments.

## Tests

- Unit tests: ray–sphere, unprojection of the pointer ray, picking on a smooth sphere against
  the nearest cell of the ray–sphere hit (500 rays, with and without a far start hint), a miss
  beside the planet, a grazing ray hitting a raised cell before the ocean, the outline polygon
  (count, position, counter-clockwise order), lat/lon, the formatting of all rows for land and
  ocean cells, the button summary, `CellInfo` size (whole u32 words for the web) and
  `cell_infos` of a small planet against the simulation's accessors.
- Headless output byte-identical to main: `--headless --subdivisions 4 --duration 20`, all 12
  PNG snapshots identical and the log identical except for the run time.
- Visual, virtual screen (Xvfb, Mesa llvmpipe):
  - Desktop viewer, 5 and 6 subdivisions: hover ([desktop-hover.png](desktop-hover.png)), click
    to keep and pointer moved off the planet, cell at the limb on land
    ([desktop-selected.png](desktop-selected.png)), widget hidden with the summary button
    ([desktop-hidden.png](desktop-hidden.png)), default resolution
    ([desktop-subdivisions-6.png](desktop-subdivisions-6.png)), drag still rotates.
  - Web in Chromium (SwiftShader WebGL2) at 1280 × 800 with the mouse: hover
    ([web-desktop-hover.png](web-desktop-hover.png)), a cell at the limb on land.
- Headless Chromium phone emulation (390 × 844 CSS px, device scale 3, coarse pointer, touch
  events via CDP): tap selects, drag rotates (the selected cell moves with the planet), the
  summary button opens the widget, tap beside the planet clears
  ([phone-tap-drag-open-clear.png](phone-tap-drag-open-clear.png)); with both the widget and the
  parameter panel open, the panel ends above the widget
  ([phone-both-open.png](phone-both-open.png)).
- Independent review of the diff: no blocking bugs; fixed its findings (selection kept by
  place across a restart with another resolution, no dead zone at the start of a drag, the
  parameter panel no longer resizes with the widget's rows, warm-started picking walk, label
  of old thinned continental crust, tests of the wire size and of `cell_infos`, docs).

Not tested: keeping the selected place across a restart with another `--subdivisions` (text
input through xdotool does not reach egui on the virtual screen; the code maps the cell to the
nearest one of the new mesh), a real phone or tablet, a real GPU, Firefox and Safari, Windows and macOS (CI
compiles them only), high-DPI desktop screens.

## Caveats

- At the default 6 subdivisions on desktop a cell is only ~5–6 px wide on screen (no zoom in
  the viewer yet), so the frame is a small ring; on the web (5 subdivisions) ~11 px.
- The cell is the Voronoi cell of a mesh vertex (the colours are interpolated between vertices,
  so the drawn frame does not match a hard colour edge). The picking treats every cell as a
  terrace at its vertex's height, so on steep exaggerated flanks the pick can be one cell off.
- egui decides between a click and a drag after 6 pt of movement; the rotation then catches
  up with the movement since the press, so it is not lost.

## Follow-up: stable rows (PR #83 comment)

Sebastian: the rows jumped around when moving over the planet, because some cells lack some
values. Now:

- The widget always shows the same 13 rows in the same order (`ROW_LABELS`): Position,
  Elevation, Surface, Plate, Crust, Lithosphere age, Sediment, Orogeny, Temperature, Jan / Jul,
  Precipitation, Permanent ice, River discharge. A value that does not apply (no orogeny, no
  permanent ice, no river or an ocean cell, unknown) is shown as "—" with the row greyed out
  (egui's weak text colour). Without a cell, all rows are greyed out.
- Rows that changed their label are fixed: "Depth 4259 m" is now "Elevation -4259 m", and
  "Ocean floor age 45 Myr" is "Lithosphere age 45 Myr (ocean floor)".
- The widget has a fixed width, so values of other lengths do not resize it; the label column
  is set by the longest label, which is always present.
- The summary button of the hidden widget (phones) is padded and in a monospace font, so it
  keeps its width.
- Unit test: the same rows for land, ocean, sea-ice, ice-sheet and bare-land cells and for no
  cell; constant summary length.

Screenshots: [rows-before-after.png](rows-before-after.png) (desktop viewer, virtual screen:
before/after for an ocean and a land cell; before, the widget changes height, width and
labels), [rows-phone.png](rows-phone.png) (headless Chromium phone emulation: the summary
button for two cells, then the open widget for two cells; constant sizes).
