# Issue #174: the web view lags whenever the planet updates

## Answer

**Not the atmosphere.** The stall on every new state is the realistic mode rebuilding its
sub-cell detail mesh (`modes::realistic::detail_vertices`, from #158 on 2026-11-24) on the page's
main thread: 85% of that time was `powf` in the sRGB encoding of the colours, done six times per
cell (once for every corner of every triangle) and four colours each. The simulation itself runs in
the worker and does not block the page.

The per-update cost did not change with the merges since yesterday morning (#163–#170): about
125 ms of main-thread work per update here, before and after. What changed:

- **More updates per second.** #163 made a step ~19% faster (native, single-threaded: 100 Myr at
  5 subdivisions in 19.4 s before, 15.8 s with #163; in the browser worker 306 → 248 ms per step
  in the topographic mode). The same stall now comes more often, and a slow phone that kept up
  before can run out of main-thread time (at a 4× CPU throttle the main thread was saturated both
  before and after: ~99% of the time in long tasks).
- **The atmosphere (#170)** makes every drawn frame more expensive on the GPU (+25–33% in
  SwiftShader, see `notes/issue-92/atmosphere.md`) and costs ~1 ms of CPU per frame
  (`atmosphere::Model::brightest_limb` in the exposure metering, ~6 ms at 4× throttle). That is per
  frame, not per update, and the same while dragging the planet without updates.

The detail level (Low/Medium/High) only changes the shader's noise, not this CPU work, so the
stall was the same on Low.

## Fix

`detail_vertices` computes the attributes of a cell (colours, coast height) once per cell and
copies them to its corners, and encodes the colours with `srgb8`: a table of the 255 thresholds of
`unorm(srgb(x))`, built once with the same `srgb`, and a binary search. The output is identical:
a test compares `srgb8` with `unorm(srgb(x))` on every 1021st float in 0..=1, ±200 floats around
every code boundary and special values (NaN, ±∞, negatives, >1); an exhaustive check over all
1.07·10⁹ floats in 0..=1 (native, release) found no difference. The existing test of the detail
mesh still passes.

## Measurements

Headless chromium (SwiftShader WebGL2), 390×844, `?subdivisions=5&render-mode=realistic`
(default Medium detail), 3 cores, after 20–40 s of warm-up, 30–40 s measured. CPU profiles with
Chrome's sampling profiler on builds with wasm names (`data-wasm-opt="0"`); long tasks (> 50 ms)
from a `PerformanceObserver` on the deployed configuration (`trunk build --release`).

Main-thread work per update (unthrottled):

| build | updates/s | `detail_vertices` | all page work per update (egui frames) | long tasks in 30 s |
|---|---|---|---|---|
| before #163 (ece532d) | 1.85 | 122 ms | 139 ms | 55, median 181 ms |
| main (3124aa0) | 2.25 | 127 ms | 148 ms | 65, median 186 ms |
| fix | 2.36 | 6.5 ms | 23 ms | 5, median 56 ms |

With a 4× CPU throttle (`Emulation.setCPUThrottlingRate`, a slow phone):

| build | updates/s | main thread in long tasks | long-task median |
|---|---|---|---|
| before #163 | 2.72 | 38.8 s of 39.3 s (saturated) | 377 ms |
| main | 3.13 | 38.7 s of 39.3 s (saturated) | 423 ms |
| fix | 2.11 | 12.5 s of 39.8 s (31%) | 143 ms |

(The worker is not throttled; with the fix it competes with more frames of SwiftShader for the
three cores, hence fewer updates per second there.)

What remains per update with the fix, 4× throttle: the frame history's encoding
(`History::record`, ~65 ms: quantisation into planes, lz4, IndexedDB put), `detail_vertices`
(~40 ms), the surface mesh and drainage (`Viewer::show`, ~14 ms), plus the exposure metering
(~8 ms per frame, ~6 ms of it the atmosphere's limb). The topographic mode costs ~12 ms per update
unthrottled (mostly the history) and showed no stalls. A channel-major rewrite of the
quantisation (`Planes::new`) was tried and was slower; it is not part of the change.

## Possible follow-ups (not in this change)

- Encode the frame history in the worker (it has the previous state for the delta) and send the
  encoded frame along: takes the largest remaining per-update cost off the page.
- Build the detail mesh's per-cell attributes in the worker too, or move the per-corner expansion
  to the GPU (an indexed draw with the plate triple per face from a storage/texture lookup).
- Cache the atmosphere's brightest-limb metering while the eye and the sun do not move.

## Method

`cdp.py` (in `~/tasks/issue-174/`, not in the repo) starts headless chromium with
`--use-angle=swiftshader --enable-unsafe-swiftshader`, injects a script that wraps `Worker` to
time every message, records `requestAnimationFrame` times, long tasks and WebGL draw calls,
optionally throttles the CPU and records a CPU profile over the DevTools protocol. Native step
times: `--headless --subdivisions 5 --duration 100` with `RAYON_NUM_THREADS=1` at ece532d,
05c13b7 (#163), 97b0bef (#165), fe442d9 (#168) and main: 19.4, 15.8, 16.1, 16.5, 16.5 s.

## Who does what per frame (PR #176 review question)

Measured after merging main (with #175's water/ice detail), headless chromium/SwiftShader,
390×844, 5 subdivisions, realistic mode; ms of main-thread time per update (unthrottled / 4×
CPU throttle), from CPU profiles.

**Worker (web) / simulation thread (desktop)**, per step: the simulation step; `cell_infos` (the
attributes of every cell, full precision) and `plate_infos`; the extra fields the mode asked for
(`Fields::extract`); the mesh with the first update only; posting it all with the buffers
transferred (web) or sent through a channel (desktop). With this change also the frame of the
history (`FrameEncoder`: quantisation into planes, difference to the previous frame, lz4).

**Page main thread** per arriving update, before this change:

| step | unthrottled | 4× |
|---|---|---|
| message handler: copy the transferred buffers into `Vec`s (cells, plates, fields) | 0.3 | 4.7 |
| history: quantise (`Planes::new`) | 4.5 | 38 |
| history: delta + lz4 (`codec::encode`) | 1.7 | 13 |
| history: IndexedDB put (copy into JS, transaction) | 2.6 | 12 |
| `show`: surface mesh vertices, drainage, plate colours | 1.7 | 15 |
| `refresh_display`: the mode's colours/overlay; realistic: detail mesh + uniforms | 9.1 | 62 |
| per drawn frame (~1.7 per update): exposure metering incl. the atmosphere's limb, egui, GPU uploads/submission | ~5.5 | ~35 |
| **all page work per update** | **27** | **189** |

The page does not decode anything for the latest state: it gets the cells at full precision and
builds the meshes from them. Decoding happens only when the time bar shows a past frame (read
from IndexedDB, delta chain from the keyframe, dequantise). The desktop does the same in its
view thread.

## Frame encoding in the worker

The simulation side now encodes the history frame (`viewer::FrameEncoder`, the former
`History::record` logic, byte-identical frames, format unchanged at version 11) and sends it with
the update (web: one more transferred `ArrayBuffer`, ~120–140 KB; `None` for a state sent again
with other fields). The page stores it as it is (`History::record(frame)`, reading kind, time and
size from the header). It still also gets the full cells for the latest state: decoding the
frame instead would cost more than the copy (native: decode 0.6 ms + dequantise 1.5 ms vs. a
0.05 ms copy of the cells; on the page the copy is 0.3 ms) and would show the latest state
quantised. If a message cannot be posted, the next frame is a keyframe.

| | page: history per update | page: all work per update | long tasks, 40 s (4×) | worker step (median) |
|---|---|---|---|---|
| before (merged main + #174 fix) | 8.8 / 63 ms | 27 / 189 ms | 82, 14.5 s | 450 / 478 ms |
| frames encoded in the worker | 1.8 / 11 ms | 20 / 155 ms | 73, 8.8 s | 463 / 501 ms |

(unthrottled / 4× throttle; the worker is not throttled. The worker's step grows by the encoding,
~2–3%.) What remains on the page per update is the IndexedDB put, the meshes of the shown
state, and the per-frame work. The detail mesh (~9 / 62 ms) is now the largest part; it could move
to the worker only for the latest frame, and the view would then need it again for past frames.
