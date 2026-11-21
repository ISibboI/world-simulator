# #91: render-mode selector and shared data path — plan and design

Scope: the foundation for the render modes of #92–#103, with the existing topographic view as
the first (default) mode, and one small example mode (plate motion arrows) that exercises every
building block end to end. The other modes are not implemented here.

## Design

### Modes (`src/viewer/modes/`)
- `RenderModeId` (clap `ValueEnum`): the names on the command line (`--render-mode`), in the URL
  (`?render-mode=…`) and in the local storage.
- `RenderMode` trait, one type per mode in its own file:
  - `label()` — name in the switcher;
  - `layers()` — extra fields it needs beyond `CellInfo` (default: none);
  - `colors(&ModeData)` — per-cell colours, or `None` to keep the topographic vertex colours;
  - `arrows(&ModeData)` — vector samples + `ArrowStyle`, or `None`;
  - `shading()` — lit (relief visible, default) or unlit (colours exactly as in the legend);
  - `legend(Option<&ModeData>)` — description, colour bar (`ColorScale` with title/unit and tick
    labels), swatches, arrow scale; built with every update, so ranges/counts can follow the data
    (texts are `Cow<'static, str>`).
- `ModeData` gives a mode the mesh, the displayed vertices, the `CellInfo`s, the received fields
  and evenly spread arrow cells (icosphere-3 grid, 642 arrows ~8° apart).
- Adding a mode = enum variant + its name + a file with the impl + one line in
  `RenderModeId::mode`. A scalar mode is ~30 lines: `ColorScale::colors(cells.iter().map(|c|
  c.temperature), missing)` + a legend. A vector mode: a `Layer` + `ModeData::vector_samples`.
- Known limits for later modes (from the review), deliberately left for the modes that need them:
  - Colours are per vertex and interpolated across triangles, so categorical modes (#93 plates,
    #99 crust type, #103 Köppen) get soft boundaries one cell wide; crisp ones need flat per-cell
    shading (a dual mesh or a per-cell colour texture) — #93 can add it.
  - Boundary lines (#93): the overlay already takes any triangles (a line is a thin quad);
    a mode hook for lines/polygons besides arrows is a small extension of `RenderMode::arrows`.
  - Modes are stateless `&'static` values; settings (e.g. #94's month) or caches would move into
    a per-mode state owned by the viewer.
  - #92 (photorealistic) needs its own shader/pipeline; `shading()` is the first such hook.
- Topographic keeps the colours the simulation computes (`render::surface_color`), so it is
  byte-for-byte the old view (and the headless PNGs are untouched).

### Data path (`src/viewer/fields.rs`)
- Every update still carries the vertices (40 B/cell) and the `CellInfo`s (56 B/cell). Those cover
  all scalar modes planned so far (elevation, temperature incl. Jan/Jul, precipitation, ice,
  discharge, crust type/thickness/age, sediment, orogeny, plate id) — they cost nothing extra; the
  view recolours on its side.
- Data beyond that (vectors for plates/ocean/wind, later basins etc.) are `Layer`s: flat `f32`
  arrays, `components()` per cell, extracted by the simulation **only while the active mode
  requests them** (`Layers` bit set). On the web each layer is one more transferred buffer.
- Requests: `StartSimulation` now returns a `SimulationControl` (`request_layers`). The initial
  layers come from `cli.render_mode` (the view puts the active mode into the launch arguments).
  - Desktop: an mpsc channel to the simulation thread, drained before each update; after the run
    the thread waits on the channel and re-sends the final state with new layers.
  - Web: the worker used to block in the simulation loop and could not receive messages. It now
    runs one step per `setTimeout(0)` task (`Run`, factored out of `simulate`), so `{ layers }`
    messages arrive between steps; it then re-sends its current state with the new fields at
    once (also after the run is complete). `worker.js` keeps messages that arrive while the module
    is still loading and hands them over once the simulation's handler is installed.

### Vector overlay (`src/viewer/arrows.rs`, `overlay.wgsl`)
- `Sample { base, vector }` → `arrows()` with an `ArrowStyle` (length and width grow linearly
  with magnitude up to a reference value; tiny ones dropped; radial part removed) →
  `triangles()`: shaft + head in the tangent plane at the base, lifted above the highest
  neighbouring vertex, with a dark border (borders first, then fills).
- Renderer: a third pipeline, unlit triangles, depth-tested against the planet, no depth write
  (draw order = borders under fills), uploaded only when the overlay changes.

### UI (`src/viewer/modes/switcher.rs`)
- Top-left corner (free: parameters top-right, cell widget bottom-left, banner across the
  middle, speed hint under the ⚙ button): a drop-down of the modes and a "Legend" toggle; the
  legend (description, colour bar with tick labels, swatches, arrow scale drawn at the arrows'
  on-screen size) below. Starts collapsed below 600 pt like the other panels; same widgets on
  desktop and web; touch sizes come from the shared `UiOptions` style; text follows the root
  font size.
- The parameter panel hides `render-mode` (no restart needed to change it).

### Remembering the mode
- Desktop: `--render-mode` (default `topographic`).
- Web: URL `render-mode` > local storage `world-simulator.render-mode` > default
  (`modes::initial`). Picking a mode replaces the page URL at once (`history.replaceState`, the
  running options + mode) and stores it (the default is removed instead of stored). Restarts keep
  the mode in the URL.

## Tests
Unit tests: mode names vs clap, precedence, store/forget, argument rewriting, colour scale
interpolation, topographic scale = map colours, a scalar test mode recolouring vertices, layer
sets and field insertion/validation, plate-velocity extraction (tangent, plausible), arrow
length/width scaling, tangent/lift, geometry (tip, widths, head share, orientation, border),
draw order, sample spacing, base on the highest neighbour. No egui UI smoke tests.
Visual: vscreen desktop + web, headless-Chromium phone emulation. Headless PNGs byte-identical
to main.
