# #91: results

Design: [plan.md](plan.md). Screenshots in this folder.

## What it looks like
- Top-left corner: a drop-down of the render modes and a **Legend** button; the legend (short
  description, colour bar with tick labels and unit, swatches, arrow scale at the arrows'
  on-screen size) opens below. Starts open on wide screens, closed below 600 pt (phones), like the
  parameter panel and the cell widget. Same widgets on desktop and web; on touch screens they get
  the 44 pt targets of the shared touch style; text follows the root font size.
- Modes: **Topographic** (default, unchanged colours) and **Plate motion** (example: plate
  velocities as yellow arrows with dark borders over the topographic colours; length ∝ √speed up to
  10 cm/yr, 642 arrows ≈ 8° apart; legend shows 1, 3 and 10 cm/yr arrows). #93 can extend it with
  plate colours and boundaries.
- The parameter panel's hidden-state button (and the step-time hint under it) now keeps clear of
  the switcher: beside it if ≥ 160 pt of room, else below it (`web-narrow-window.png`).
  - Found while testing: the button's area used a right-aligned layout over the full width, so its
    (invisible) layer covered the whole top strip and swallowed taps on the switcher on phones
    (mouse clicks worked because on wide screens the panel is open). It is now at most as wide as
    the room beside the switcher.

## Data path and cost
- Scalar modes: no extra transfer; they colour from the `CellInfo`s every update already carries.
- Extra fields (`Layer`s) only while the active mode needs them. Plate velocity: 12 B/cell =
  0.49 MB per update at 6 subdivisions (0.12 MB at 5), extraction 0.16–0.24 ms (vertices take
  5.6–8.9 ms, `CellInfo`s 2.4–4.2 ms in the same updates). Nothing while the mode is not active.
- View side, per update: plate-motion display rebuild 0.35–0.41 ms at 6 subdivisions (642 samples,
  ~11k overlay vertices = 0.3 MB uploaded). A recolouring scalar mode adds one pass over the cells
  and a copy of the vertices (1.6 MB at 6 subdivisions); not measured, as no such mode exists yet.
- A mode switch that needs new fields gets them at once: the web worker now runs one step per
  `setTimeout(0)` task and re-sends the current state between steps; the desktop thread sends them
  with the next step. After the run is complete both re-send the final state on request (checked
  on desktop and web: switching after "t = 10 of 10 Myr" shows arrows).
- Worker scheduling overhead: one `setTimeout(0)` per step (≤ 4 ms clamp vs. 0.2–1 s steps).

## Remembering the mode
- Desktop: `--render-mode topographic|plate-motion`.
- Web: `?render-mode=…` > localStorage `world-simulator.render-mode` > default. Checked in headless
  Chromium: stored `plate-motion` without URL parameter → plate motion; URL `topographic` with
  stored `plate-motion` → topographic; picking a mode → URL gets `render-mode=plate-motion` at once
  and the storage is set; restarts keep the mode (desktop and web). With remembered subdivisions
  (localStorage) and no `subdivisions` in the URL, picking a mode leaves the URL without them.
- Phones: the open parameter panel goes below the switcher (`phone-panel-open.png`), the hidden
  panel's button beside it.

## Tests
- Unit tests (new): mode names vs clap, precedence, store/forget, argument rewriting and parsing,
  colour-scale interpolation/clamping/NaN, topographic scale = map colours, a scalar test mode
  recolouring vertices, layer bits/sets, field insertion and validation, plate-velocity extraction
  (tangent, < 50 cm/yr, moving), arrow length/width scaling (linear and √), tangent and lift,
  dropped tiny/radial vectors, geometry (tip, widths, head share, CCW orientation, border), border
  draw order, sample spacing, base on the highest neighbour, panel button placement.
- CI-clean locally: fmt, clippy native and wasm32, tests, doc, warnings denied.
- Headless `--subdivisions 4 --duration 20 --snapshot-interval 10`: PNGs, state hashes and logs
  byte-identical to main.
- Visual: vscreen desktop viewer (4–6 subdivisions; switching during and after a run, restart
  keeps the mode, legend toggle), vscreen Chromium (SwiftShader) wide and narrow window, headless
  Chromium desktop and phone emulation (tap the drop-down, pick a mode, open the legend).
- Not tested: real phones, a real GPU, Firefox/Safari, phone widths below 500 CSS px in emulation
  (headless Chromium's emulated small viewports mis-size eframe's canvas, see #72); the 340 px
  case was checked in a real narrow Chromium window on vscreen instead.

## Notes for later modes
- A scalar mode: `colors()` = `ColorScale::colors(data.cells.iter().map(|c| c.temperature),
  missing)`, `legend()` with the same scale. Seasonal variants can use the Jan/Jul fields; a month
  slider would need a layer.
- A vector mode on the coarse ocean/climate grid (#96, #97): add a 3-component `Layer` (tangent
  vectors per fine cell, or interpolate), then `data.vector_samples(layer)` + an `ArrowStyle`.
- #92 (photorealistic) needs its own pipeline/shader; `RenderMode` would then grow a hook for
  that. The crust-anchored detail noise needs per-cell plate-frame positions — another layer.

## Independent review
One reviewer subagent read the diff (findings in the task folder, summarised here). No blocking
findings; fixed: requests lost while the worker module loads (buffered in `worker.js`), step-time
measurements keyed with the render mode, picking a mode wrote remembered subdivisions into the URL
(now only the URL's own `render-mode` is replaced), arrows diving into mountains (base lifted over
everything within an arrow's length), a stale vertex id on an error path, the open parameter panel
overlapping the switcher (placed beside or below it like the button), legend texts now dynamic and
built from the data, a lit/unlit shading hook, `--render-mode NAME` spelling, and small nits.
Left as documented limits for later modes: categorical colours interpolate across cells, no line
hook yet, stateless modes, `setTimeout` 4 ms clamp per step.

## Unrelated fix
- `src/viewer/pace.rs` (#78) used std's `HashMap`, which #87's Clippy rule rejects (the two PRs
  were merged around the same time, so main fails `cargo clippy` with warnings denied); it now
  uses `crate::hash::HashMap`.
