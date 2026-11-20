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
- **Same seed on web and desktop** (checked 2026-11-20, see D6):
  - The RNG is not the issue. All randomness is the seeded `ChaCha8Rng`, which gives the same
    sequence on every platform.
  - Natively the simulation is **deterministic**: two runs with `--seed 3`, 15 Myr, gave
    byte-identical outputs (all 29 PNG and cell-dump CSV files, and the logs).
  - The difference comes from floating-point math functions. The simulation calls
    `sin`/`cos`/`exp`/`ln`/`asin`/`acos`/`atan2`/`tan`/`powf` about 150 times in its source.
    The basic operations (`+ − × ÷ sqrt`) are exactly rounded by IEEE 754 and identical
    everywhere. The transcendental functions are not: native Linux uses glibc, while
    `wasm32-unknown-unknown` uses Rust's port of musl's libm (the `libm` crate). Comparing
    the two natively on 2 M random inputs, **3–14 % of the results differ in the last bit**
    (sin 3.0 %, exp 9.7 %, atan2 14 %, pow 9.7 %).
  - The simulation is chaotic (plate splits, thresholds), so last-bit differences grow into
    a different planet: equally valid, but not the same.
  - This is expected, not measured end-to-end (no wasm runtime on tktie). The same applies
    today between Linux, Windows and macOS builds, or even different glibc versions.
- **Decided (D6):** determinism is required **per platform**, not across platforms, so the
  `libm`-everywhere switch is not needed. The web version must give the same simulation for a
  seed on every run, in every browser and on every machine. Desktop must be deterministic as an
  algorithm, but its math-library differences don't matter. The web side gets this almost for
  free: the wasm binary carries its own `libm` (compiled into the module, not taken from the
  browser), and wasm floating point is IEEE-exact except for NaN bit patterns. Checked in the
  code for things that could break run-to-run determinism in the browser:
  - **Step size:** `dt` comes from the config (`--dt`/URL), never from wall-clock or frame time.
    The worker must keep it that way, so a slow machine takes longer but computes the same
    steps.
  - **Worker and UI timing** only decide *which* states get displayed (the viewer drains its
    channel and keeps the latest). They never feed back into the simulation. Control messages
    (later: pause/restart, #61) must act only between steps.
  - **`Instant`** (`planet.rs:228,232`, `main.rs:250`) is used only for log timings.
  - **Hash maps:** simulation maps use the fixed `FxHasher` (`tectonics/mod.rs:66`). The three
    std `RandomState` maps (`sphere.rs:69`, `ocean.rs:49`, `climate.rs:1085`) are only used
    for lookups and never iterated, so their random seed doesn't matter.
  - **SIMD:** `+simd128` is deterministic. Do **not** enable `relaxed-simd`, whose fused
    multiply-add and min/max results may differ between CPUs.
  - **Check:** M2 adds a reproducibility test that runs the same seed twice in the browser (or
    in Node/wasmtime in CI, if cheap) and compares a hash of the surface elevations after
    N steps.
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
- **D3 decided (Sebastian, 2026-11-20):** the web version reads the parameters from URL query
  parameters (`?seed=3&subdivisions=5`, same clap definition as the CLI). A GUI panel for
  choosing parameters and restarting goes to a separate issue, #61 (desktop and web, shared
  code). Its initial values still come from the CLI or URL. The desktop viewer's only input
  today is click-and-drag rotation.
- **D4 Deploy trigger:** every push to `main` (recommended) or only tagged releases or a
  manual trigger.
- **D5 Enable Pages:** set Settings → Pages → Source to "GitHub Actions" when M3 lands.
- **D6 decided (Sebastian, 2026-11-20):** seeds must reproduce the same simulation
  **on the web** (run to run, across browsers and machines). Desktop must be deterministic as
  an algorithm, but math-library differences don't matter, and web and desktop needn't
  match. So there is no `libm` switch; see §4 for what keeps the web build deterministic.

Still open before M0 can start: **D1, D2, D4, D5**.
