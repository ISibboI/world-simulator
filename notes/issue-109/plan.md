# #109 Simulation time controls: plan

Goal (issue #109 and the agreed design in its comments): pause/resume, step one frame back and
forth, a timeline slider over all computed frames, a "Live" button, and the age (Myr) of the
shown frame. Past frames are cached compactly: quantised per-cell fields, delta to the previous
frame, lz4, a keyframe every 25 frames; on disk on the desktop, in browser storage on the web.

## Frame contents

A frame is what the view needs to rebuild everything it shows:

- the [`CellInfo`] of every cell, quantised (25 bytes per cell instead of 56, see the table
  below), stored column by column (one channel per field, u16 channels split into a low- and a
  high-byte plane, so that lz4 finds the runs);
- the angular velocity of every plate (12 bytes per plate, a few dozen plates), so that the
  plate-motion layer (and future layers derived from plates) can be rebuilt for any past frame,
  whatever mode was active when it was computed;
- the simulated time.

Vertices are not stored: positions and topographic colours follow from the elevation, ice and
discharge of the cells (`render::surface_color` gets a variant on these values), with the run's
`--exaggeration` and `--river-threshold`. Every render mode, the legend and the cell widget then
work on past frames as on the live one.

| Field | Encoding | Resolution / range |
| --- | --- | --- |
| elevation | i16 | 1 m, ±32 km |
| crust thickness | u16 | 10 m, 0–655 km |
| lithosphere age | u16 | 0.25 Myr, 0–16 Gyr |
| sediment | u16 | 1 m, 0–65 km |
| orogenic share | u8 | 1/254 |
| time since orogeny | u16 | 0.25 Myr, NaN reserved |
| temperature (annual, Jan, Jul) | i16 ×3 | 0.1 °C, NaN reserved |
| precipitation | u16 | 1 mm/yr, 0–65 m/yr, NaN reserved |
| ice | u8 | 1/254, NaN reserved |
| discharge | u16 log | ln(1+Q)·170: 0.6 % relative |
| plate | u16 | id |
| flags | u8 | bits |

The resolutions are the cell widget's display precision or finer (chosen after measuring, see
`results.md`). Values outside the range
are clamped.

## Codec

- Delta frames: each channel minus the previous recorded frame's channel (wrapping), so unchanged
  values are 0. Keyframes (every 25th recorded frame): the values
  themselves. Then `lz4_flex` (pure Rust, no unsafe, works on wasm).
- Decoding frame i: from the nearest keyframe k ≤ i, or from the last decoded frame j if
  k ≤ j < i (stepping forward costs one delta). Frames are appended in order, so k..=i is one
  contiguous read.

## Storage (`FrameStore`)

Shared code (`viewer::history`) does all encoding, indexing and decoding; a store only keeps the
compressed blobs, appended in order and read as a range, asynchronously (a callback wakes the
view):

- Desktop: an anonymous temporary file (`tempfile::tempfile()`: unlinked at once on Unix,
  delete-on-close on Windows, so it disappears with the run, a restart or a crash), with an
  offset index in RAM. Reads complete at once.
- Web: IndexedDB on the page (one database per run, `world-simulator-frames-<random>`, deleted
  when the run ends; databases of earlier page loads are deleted at start). IndexedDB rather than
  OPFS in the worker: the view needs past frames while the worker is busy with a step (seconds
  at 6 subdivisions), and the worker can only answer between steps; OPFS from the page has no
  cheap append (a writable stream commits by copying the file). The browser may evict the data;
  a frame that cannot be read is reported in the time bar.
- Fallback (and tests): in memory.
- Cap: when the stored bytes exceed the cap (desktop 4 GB, web 1 GB or a write fails with a
  quota error), only every 2nd (then 4th, …) frame from then on is recorded. The slider covers
  recorded frames; their times are shown.

## Simulation control

`SimulationControl` gets `set_paused(bool)` and `step()` (one step, then paused). Desktop
thread: requests go through the existing channel as an enum; while paused, the thread blocks on
it after publishing. Web worker: `{ paused }` and `{ step }` messages; the worker stops
scheduling steps while paused.

## UI (time bar)

A strip along the bottom edge (the cell widget moves up above it; the parameter panel stays
above both): ⏸/▶ (pause, resume), ⏴ / ⏵ (one frame back/forward; ⏵ on the newest frame while
paused computes one more step), a slider over the recorded frames, "Live" (back to the newest
frame, following it), and the time of the shown frame ("123 Myr", or "123 Myr · live 300").
Viewing a past frame does not pause the simulation: the slider keeps growing, the view stays on
the chosen frame until "Live" or the end of the slider. On narrow screens the slider gets its
own row. On touch screens the buttons and the slider handle are touch-sized (44 pt, from the
existing `UiOptions`). Keyboard (desktop and web): space pauses/resumes, ←/→ step, End = live,
unless a text field has focus.

## Measurements

Raw and compressed bytes per frame at subdivisions 5 and 6 (keyframes and deltas) over a
run, logged by a test binary or debug log line; results in `results.md`.

## Tests

Unit tests: quantisation round trip within resolution (incl. NaN, clamping), codec round trip
for key and delta frames over a sequence, decoding any frame from any decoded state, index /
stride after the cap, memory and file store.

## Plan review (independent agent) and decisions

The review found, and the implementation follows:

- **Repeated states** (sent again with new fields after a mode change) carry the same time: a
  frame is recorded only if its time is later than the last recorded one.
- **One history per run**: a restart drops it (the desktop temp file disappears with its
  handle; the web database of the run is deleted) and starts a new one; frames of another cell
  count are refused when decoding.
- **Other tabs**: each tab holds a Web Lock named after its run's database; the clean-up at start
  deletes only databases whose lock nobody holds.
- **The cap is a cap**: thinning to every n-th frame from then on would still grow without bound,
  and after a quota error further writes fail as well. At the cap (or the first failed write) the
  history stops recording and the time bar says so ("history full"); frames whose write failed,
  and the delta frames after them, are marked unreadable. At the measured sizes the cap is far
  beyond a default run.
- **Live and past frames look alike**: the view builds the vertices from the cell attributes for
  every frame, live included; the simulation no longer sends vertices (less to transfer, one code
  path for colours and relief).
- Plate rotations in every frame make the plate-motion layer derivable for past frames
  (ω[plate] × r). The layer mechanism for the live frame stays as it is.
- Pause/step: the desktop thread blocks after publishing while paused, still answering layer
  requests, and exits when the view drops its control (restart). The web worker stops scheduling
  steps; pause and step take effect between steps. A step after a pause is not counted for the
  step-time hint. ⏵ is disabled while a requested step is pending.
- Measured before deciding the resolutions; see `results.md`.
- Keys are consumed before the widgets see them (space would otherwise also press a focused
  button).
- Phones: the time bar is two rows (buttons and time; slider) at the bottom, the cell widget's
  button above it.
