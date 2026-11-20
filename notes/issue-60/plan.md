# Issue #60: web version: plan

Goal (#60): a static website on GitHub Pages that runs the simulator in the browser and shows
the same 3D view and UI as the desktop window, built from **one code base**, so that changes
to the desktop version automatically show up on the web and the other way round.

This is a plan only. Nothing is implemented yet. The decisions for Sebastian are listed in
[§9](#9-decisions-for-sebastian).

## 1. Where we stand (measured 2026-11-20 on `main` @ bbb849f)

- **UI stack:** `eframe 0.37` (egui) with the **wgpu** renderer. The planet is drawn by our own
  wgpu pipeline (`src/viewer/renderer.rs`, `planet.wgsl`) inside an egui paint callback.
  eframe, egui-wgpu and wgpu 30 all support `wasm32-unknown-unknown`: eframe has a
  `WebRunner`, and egui-wgpu enables both the `webgpu` and `webgl` (WebGL2 fallback) backends
  by default. The shader only uses a uniform buffer, a vec3/vec3/vec4 vertex layout and a
  diffuse light, so it is safe for WebGL2.
- **`cargo check --target wasm32-unknown-unknown`** (experiments in `agent-scratch/wasm-exp/`):
  1. With the current `Cargo.toml` it fails in **`getrandom 0.4`**, which `rand 0.10` pulls in
     through its default features (`thread_rng`/`sys_rng`). The simulation only uses seeded
     `ChaCha8Rng` (`tectonics/mod.rs`, `generation.rs`, `initial_sediment.rs`, …), so
     `rand = { version = "0.10.3", default-features = false, features = ["chacha", "std"] }`
     fixes this. No OS randomness is needed.
  2. After that, the **only** compile errors are `eframe::NativeOptions` / `eframe::run_native`
     in `viewer::run` (`src/viewer/mod.rs:98,108`), which don't exist on wasm.
  3. With `viewer::run` gated off (stub), **the whole crate compiles for wasm32**: simulation,
     `image`/PNG, clap, tracing-subscriber and eframe/wgpu with default features.
- **Runtime traps** (compile fine on wasm, but panic or do nothing there):
  - `std::time::Instant::now()` panics on wasm32-unknown-unknown: `Planet::step`
    (`src/planet.rs:228,232`) and `simulate` (`src/main.rs:250`).
  - `std::thread::spawn` panics: the viewer's simulation thread (`src/main.rs:225`).
  - `std::fs` returns "unsupported": `create_dir_all` in `main`, PNG snapshots in `snapshot`
    (`main.rs:810`), `write_cell_dump`, `write_mesh_dump`.
  - `tracing_subscriber::fmt().with_writer(stderr)` prints nowhere, so the browser console needs
    its own subscriber.
  - eframe's web painter always renders with `sample_count: 1` (`eframe/src/web/
    web_painter_wgpu.rs:65`), so our pipeline must use 1 MSAA sample on the web instead of 4,
    or wgpu validation fails. `renderer::initialize` already takes `msaa_samples` as a
    parameter. `Depth32Float` (32 depth bits) works on WebGPU and WebGL2.
- **Speed** (native release build, tktie, 20 Myr from the default start):

  | subdivisions | cells  | native ms/step (first 20 Myr) | peak RSS |
  |-------------:|-------:|------------------------------:|---------:|
  | 5            | 10 242 | ~175                          | –        |
  | 6 (default)  | 40 962 | ~355 (~550 in 1000 Myr runs with `--material-budget`) | ~156 MB |

  Expect wasm to be about **1.3–2× slower** than native: no native SIMD autovectorisation unless
  we enable `+simd128`, and a different `libm`. That gives roughly **0.25–0.35 s/step at
  subdivisions 5** and **0.5–1.1 s/step at 6**, i.e. 500 Myr takes about 2–3 min at
  subdivisions 5 or about 5–9 min at 6. Memory is far below wasm32's 4 GB limit.
- **CI** (`.github/workflows/ci.yml`) checks Linux/Windows/macOS but not wasm.
- **GitHub Pages** is not enabled for the repository yet (API: 404). The repository is
  public, so free Pages works.

## 2. Architecture: one crate, shared core, two thin entry points

Keep a single crate (no workspace split needed), but turn it into **library + binaries**:

```
src/lib.rs            all current modules (planet, tectonics, erosion, …, render, viewer)
                      + new `app` module (shared setup and loop)
src/main.rs           native entry: clap CLI, logging, headless/viewer (as now, minus the moved code)
src/web/mod.rs        #[cfg(target_arch = "wasm32")]: browser entry (WebRunner) + worker entry
index.html            trunk entry page (canvas + loader)
```

What moves where:

- **`app::Config`** = today's `Cli` struct, still `#[derive(Parser)]`. On the web it is filled
  by **parsing the URL query string through the same clap definition**
  (`?seed=3&subdivisions=5` → `Cli::try_parse_from(["world-simulator", "--seed", "3", …])`),
  so every new CLI option exists on the web automatically. Options that make no sense there
  (`--output`, `--cell-dump`, `--width`, `--headless`) are ignored or rejected on wasm.
- **`app::build_planet(&Config) -> Planet`**: the setup part of today's `main()`
  (`main.rs:159-216`: presets, overrides, `Simulation::new`, `Planet::new`), shared unchanged.
- **`app::simulate` / `snapshot`**: the step loop and statistics logging stay shared. Only the
  file writes (PNG, CSV dumps, `create_dir_all`) are `#[cfg(not(target_arch = "wasm32"))]` or
  behind an `Option<&Path>` output directory that is `None` on the web.
- **`viewer`**: `Viewer`, `surface_vertices`, `renderer` and the shader stay **100 % shared**.
  This is where "same UI" comes from: every egui panel or control added to `Viewer::ui` later
  appears on both targets with no extra work. Only `viewer::run` gets two small
  implementations:
  - native: `eframe::run_native` as now (MSAA 4);
  - web: `eframe::WebRunner::new().start(canvas, WebOptions { depth_buffer: 32, .. }, …)`,
    with the same creation closure (`renderer::initialize(…, msaa = 1)`).
- **`Instant`**: replace `std::time::Instant` with **`web_time::Instant`** (same API; on native
  it is just `std::time::Instant`). This is a one-line change in `planet.rs` and `main.rs`.
- **Simulation transport**: `Viewer` already consumes a `std::sync::mpsc::Receiver<SurfaceUpdate>`
  plus a wake-up callback (`request_repaint`). Keep that interface and make only its producer
  target-specific:
  - native: `std::thread::spawn` running `app::simulate` (as now);
  - web: a Web Worker (see §3) whose `onmessage` handler, running on the main thread, pushes
    into the same `Sender` and calls `request_repaint()`. `Viewer` stays unchanged.
- **Logging**: native keeps `tracing_subscriber::fmt` to stderr. Web uses `tracing-web` (or
  `tracing-wasm`) to the browser console, plus `console_error_panic_hook`.
- **Window title**: on the web `ViewportCommand::Title` has no effect. Show the simulated time
  as a small egui overlay label in `Viewer::ui` on **both** targets, which keeps them identical,
  and on the web also set `document.title`.

New dependencies, all under `[target.'cfg(target_arch = "wasm32")'.dependencies]` except
`web-time`: `wasm-bindgen`, `wasm-bindgen-futures`, `web-sys` (features `Window`, `Document`,
`HtmlCanvasElement`, `Worker`, `WorkerOptions`, `MessageEvent`, `DedicatedWorkerGlobalScope`),
`js-sys`, `tracing-web`, `console_error_panic_hook`; `web-time` for both targets.

## 3. Threading in the browser: one Web Worker for the simulation, no wasm threads

The simulation stays **single-threaded** ("efficiency before parallelism"). In the browser there
are two ways to run it next to the UI:

- **Step on the main thread between frames (time slicing).** This is the simplest option, but
  `Planet::step` can't be interrupted and takes ~0.25–0.35 s (subdivisions 5) to ~0.5–1.1 s
  (subdivisions 6) in wasm, far longer than a 16 ms frame. While a step runs, the page freezes:
  no rotation and no egui input. Splitting `Planet::step` into resumable phases would touch the
  physics code everywhere and is not worth it.
- **Recommended: one dedicated Web Worker that owns the `Planet`.** The worker loads the same
  `.wasm` (a second entry point), builds the planet from the same `Config`, and steps in a loop.
  After each step it calls the shared `viewer::surface_vertices` and posts the vertex array
  (40 962 × 40 B ≈ 1.6 MB at subdivisions 6) as a **transferable** `ArrayBuffer`, so it is
  moved, not copied, together with the time. The main thread only renders and handles input, so
  rotation stays at full frame rate even while a step takes a second. The worker yields to its
  event loop between steps (`await` a resolved promise or `setTimeout(0)`), so it can receive
  control messages later (pause, restart with new parameters).
  - This uses plain `postMessage` without `SharedArrayBuffer`, so it needs **no COOP/COEP
    headers** and works on GitHub Pages as is.
  - The worker runs the same `app::build_planet` and `app::simulate` code with a different
    `on_step` callback, so the simulation logic is not duplicated. It is the web counterpart
    of today's `std::thread::spawn` in `main.rs:225`, which also only separates the simulation
    from the window.
  - The mesh faces (needed once for the index buffer) are rebuilt on the main thread with
    `SphereMesh::icosphere(subdivisions)`, which is cheap and deterministic, so they don't
    need to be sent.
- *Rejected:* wasm threads with `SharedArrayBuffer` (needs nightly `build-std`, COOP/COEP
  headers that GitHub Pages can't set, so a `coi-serviceworker` hack, and the simulation is
  single-threaded anyway).

Milestone 1 (§7) may first step on the main thread (one step per frame at subdivisions 4–5)
to get pixels on screen quickly. The worker is the target design.

## 4. Performance and defaults on the web

- Default **subdivisions 5** on the web (10 242 cells, ~0.3 s/step): one 500 Myr run takes a
  few minutes, which suits a demo page. 6 stays selectable via `?subdivisions=6`. Native
  keeps 6. (Decision D2.)
- Build the wasm with `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`, and try
  `-C target-feature=+simd128` (supported by all current browsers). Run `wasm-opt -O3` (trunk
  does this with `data-wasm-opt="3"`). Measure the speedup in M2 and keep it only if it helps.
- Expected download: a typical eframe+wgpu app is ~4–8 MB of wasm, ~1.5–3 MB gzipped (Pages
  serves gzip). This needs to be measured in M1.
- Results will **not be bit-identical** to native for the same seed: wasm uses Rust's
  `libm` for `sin`/`exp`/…, native uses glibc, and the simulation is chaotic. Both are valid
  Earth-like planets, but a given seed looks different on the web. (If this ever matters, the
  `libm` crate could be used on both targets.)
- WebGPU where available (Chrome/Edge, Firefox 141+ on Windows, Safari 26), otherwise WebGL2.
  Both are built in by egui-wgpu's defaults.

## 5. Build tooling: trunk

- **trunk** (`cargo install trunk --locked`, or the `jetli/trunk-action` in CI) drives cargo,
  `wasm-bindgen` and `wasm-opt`, and writes a static `dist/`. It is the standard tool for
  eframe web apps (the eframe template uses it).
- `index.html` contains a full-window `<canvas id="world_simulator">`, a loading text and two
  trunk links:
  `<link data-trunk rel="rust" data-bin="world-simulator" data-wasm-opt="3">` and
  `<link data-trunk rel="rust" data-bin="sim-worker" data-type="worker">`.
  Alternatively there is one bin, and the worker re-instantiates the same module with a small
  JS shim. Pick whichever is simpler in M2.
- `Trunk.toml`: `public_url = "/world-simulator/"` (Pages project URL). Local test:
  `trunk serve`.
- Alternatives considered: plain `wasm-bindgen-cli` + hand-written HTML/JS works but means
  writing worker glue ourselves; `wasm-pack` targets npm packages and is unmaintained-ish.

## 6. CI and deployment (GitHub Actions)

Following the existing "Tests and Lints" template, with actions pinned to major version tags:

1. **Keep desktop and web in sync (`ci.yml`, every push/PR)**: add a job
   `rust_check_wasm`: `setup-rust-toolchain@v2` with `target: wasm32-unknown-unknown`, then
   `cargo check --target wasm32-unknown-unknown` and
   `cargo clippy --target wasm32-unknown-unknown`, both with warnings denied. Because the
   viewer and simulation code are shared, any change that breaks the web build (a new
   `std::fs` or `std::thread` use, a native-only dependency) fails CI on the PR that
   introduces it. Optional extra: a `trunk build --release` job so that the HTML/worker
   wiring is also tested (~3–5 min).
2. **Deploy (`.github/workflows/pages.yml`)**: on push to `main` (+ `workflow_dispatch`):
   checkout → toolchain with wasm target → install trunk (`jetli/trunk-action` or
   `cargo binstall trunk`) → `trunk build --release --public-url /world-simulator/` →
   `actions/upload-pages-artifact` (dist/) → `actions/deploy-pages`, with
   `permissions: pages: write, id-token: write` and `concurrency: pages`. Cache with
   `Swatinem/rust-cache` (setup-rust-toolchain already does this).
3. **What Sebastian must do once**: Settings → Pages → *Build and deployment* → Source:
   **GitHub Actions**. Nothing else (no gh-pages branch, no tokens). The site will be at
   `https://isibboi.github.io/world-simulator/`.
4. Note for tests: `cargo test` stays native-only. wasm tests (`wasm-bindgen-test` in a headless
   browser) are not worth it at first.

## 7. Milestones and rough effort

Each milestone is one PR, CI-clean, desktop behaviour unchanged.

| # | Step | Effort |
|---|------|--------|
| M0 | Prep refactor: `lib.rs` + `main.rs` split, `app::build_planet`/`simulate`, `web_time::Instant`, `rand` without default features, file output behind `cfg`/`Option`. Native behaviour identical (check headless output matches for a seed). | ~0.5 day |
| M1 | Web entry: `viewer::run` for wasm via `WebRunner`, MSAA 1, console logging, panic hook, `index.html` + `Trunk.toml`; simulation stepped on the main thread (temporary). CI job `rust_check_wasm`. First manual test in Chrome + Firefox; measure wasm size and ms/step. | ~1 day |
| M2 | Web Worker for the simulation (transferable vertex buffers), time overlay label shared by both targets, URL-parameter config, `+simd128`/LTO measurements, defaults (subdivisions 5). | ~1–1.5 days |
| M3 | Pages deploy workflow, README link/badge, smoke test of the deployed site. | ~0.5 day |
| M4 (optional) | Web niceties: pause/restart and seed/subdivision controls in egui (they also benefit desktop, since they are shared), a loading/progress indicator for the planet setup (~4 s native at subdivisions 6), touch rotation. | ~1 day |

In total about **3–4 agent-days for M0–M3**, plus review rounds. Per the repo conventions,
an independent agent reviews this plan before M0 starts.

## 8. Risks

- **Browser GPU differences** (WebGL2 limits, Safari quirks): low risk for our simple pipeline.
  egui-wgpu picks WebGPU or WebGL2 automatically. Test in Chrome, Firefox and Safari if
  available.
- **Speed**: if wasm turns out much slower than estimated, lower the default resolution or
  add a "steps per update" option. The UI stays responsive either way because of the worker.
- **Trunk worker support** with two bins of one crate: trunk supports `data-type="worker"`.
  Fallback is a small JS shim that loads the same wasm in the worker.
- **CI time**: the wasm check adds one job (~2–4 min). The deploy runs only on `main`.
- **Drift**: the main risk of "two versions" is native-only code paths growing in the viewer.
  The wasm CI job catches compile-level drift. Keeping `cfg` gates confined to `viewer::run`,
  `app` file output and the two entry points keeps UI drift low by construction.
- **Dependency upgrades** (eframe/wgpu) may change the web runner API. This is the same risk
  as on desktop and is caught by the wasm CI job.

## 9. Decisions for Sebastian

- **D1 Simulation in a Web Worker** (recommended) vs. stepping on the main thread with a page
  that freezes during each step. Neither uses wasm threads.
- **D2 Web defaults:** subdivisions 5 on the web (recommended; ~0.3 s/step) or the desktop
  default 6 (~0.5–1 s/step, ~1.6 MB per update).
- **D3 Parameters on the web:** URL query parameters only (cheap, shares the clap
  definition), or also egui controls (seed, plates, subdivisions, restart). The egui controls
  would appear on desktop too.
- **D4 Deploy trigger:** every push to `main` (recommended) or only tagged releases or a
  manual trigger.
- **D5 Enable Pages:** set Settings → Pages → Source to "GitHub Actions" when M3 lands.
- **D6 Determinism:** is it acceptable that a seed gives a different (but equally valid)
  planet on the web than natively? (Recommended: yes. Otherwise use `libm` on both targets,
  which may slow down native.)
