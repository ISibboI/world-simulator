# Issue #60: web version: implementation and results

Implements [plan.md](plan.md) (M0–M3) with Sebastian's decisions D1–D6 (§9 there). An
independent review of the plan is in [plan-review.md](plan-review.md). How it was handled is
below.

## What changed

- **`src/web.rs`** (new, wasm only): the entry point of the web version. One wasm module is
  both the page and the simulation worker; `web::start` tells them apart by the global scope.
  - **Page:** parses the URL query into the CLI options (same clap `Cli`), builds the mesh for
    the triangles and starts the shared viewer in the canvas. It starts the worker with the
    same query string and forwards each `{ time, vertices }` message as a `SurfaceUpdate` into
    the viewer's channel. It sets `document.title` to the simulated time, because the window
    title does not exist on the web. Errors (invalid parameters, a failed simulation) are
    shown on the page.
  - **Worker:** `build_planet` + `simulate`, exactly as the desktop's simulation thread.
    After every step it sends the vertices as a transferred `ArrayBuffer`.
  - **Logging:** `tracing` to the browser console (`tracing-web`, without timestamps) and
    `console_error_panic_hook`, in both instances.
- **`src/main.rs`:** the planet setup moved from `main` into `build_planet` (unchanged code).
  `main` is native only; on wasm it calls `web::start`. Writing snapshot files (PNG, CSV) moved
  into `write_snapshot_files`, which is native only; the statistics logging stays shared. The
  default `--subdivisions` is 6 on desktop and 5 on the web (D2). `Instant` comes from
  `web-time` (it is `std::time::Instant` on native). On wasm, dead code is allowed (the file
  output is unused there); the native build still checks it.
- **`src/viewer/mod.rs`:** the eframe app creator (`app_creator`) is shared. `run` (native,
  `run_native`, 4× MSAA) and `run_web` (`WebRunner`, no MSAA because eframe's web painter
  does not multisample) are the only target-specific parts. `Viewer`, `surface_vertices`, the
  renderer and the shader are unchanged.
- **`Cargo.toml`:** `rand` without default features (only the seeded ChaCha8 is used; the
  defaults pull in `getrandom`, which does not build for wasm). `web-time` is added, and the
  web-only crates sit under `[target.'cfg(target_arch = "wasm32")'.dependencies]`.
- **`web/index.html`, `web/worker.js`, `Trunk.toml`:** the trunk site. `filehash = false`,
  because the worker shim imports the module by name, and `public_url = "./"`, so it works
  under `/world-simulator/` and locally.
- **CI:** `ci.yml` gets `rust_check_wasm` (`cargo check` + `cargo clippy --all-targets
  --all-features --target wasm32-unknown-unknown`, warnings denied). The new `pages.yml`
  builds the site with trunk (pinned v0.22.2) on every push and PR, and deploys it on pushes
  to `main` (D4) with `actions/upload-pages-artifact@v5` and `actions/deploy-pages@v5`.
  Permissions are `contents: read` at the top level, and `pages: write` + `id-token: write`
  only in the deploy job (environment `github-pages`).

### Deviations from the plan, and how the plan review was handled

- **No lib/bin split and no second binary:** the crate stays a single binary. `main.rs`
  gets two cfg'd `main` functions, which keeps the diff small and `cargo run` unchanged
  (review must-fix 1 is moot).
- **Worker = the same module via a 3-line JS shim** (`web/worker.js`), not a trunk
  `data-type="worker"` binary. One build, one download: the browser caches the wasm, so the
  worker doesn't fetch it again. The worker also gets its parameters from its own URL, so it
  needs no messages from the page.
- **The worker loop blocks** (review must-fix 5): the worker never has to receive messages.
  Parameters come with its URL, and a restart for #61 can terminate the worker and start a
  new one. A stepper refactor of `simulate` is therefore not needed now; #61 can add one if
  it wants pause/resume.
- Applied from the review:
  - `try_parse_from`, with errors shown on the page;
  - query decoding: percent-decoding, bare flags, repeated `set=`, `_` → `-` in names;
    desktop-only options are rejected with a message;
  - `.without_time()` logging and the panic hook in both instances;
  - the PR build is a real job;
  - least-privilege Pages permissions with `environment: github-pages` and
    `cancel-in-progress: false`;
  - the wasm lint job uses `--all-targets --all-features`;
  - `touch-action: none` on the canvas;
  - `dist/` in `.gitignore`;
  - worker errors are forwarded to the page.
- Not done:
  - a separate `[profile.web]` with LTO and `+simd128`, which would be an optimisation
    without a measured need;
  - a `wasm-bindgen-test` under Node, because there is no Node in CI. Reproducibility was
    checked in a headless browser instead (below).

### Diff review

An independent review of the diff ([review-diff.md](review-diff.md)) found no must-fix items.
Its should-fix items were handled as follows:

- **Fixed:**
  - worker failures reach the page (`init().catch` → `{ error }` in `web/worker.js`, plus a
    `Worker.onerror` handler);
  - a worker start error is no longer overwritten by the "Starting…" status;
  - surface updates of the wrong size are rejected instead of panicking;
  - query values are passed as `--name=value`, so `?epoch=-1` works.
  - From the nits: the speed comment is corrected, and the build job has a timeout.
- **Documented instead of fixed:** with fixed file names, a browser can briefly (Pages caches
  for 10 min) combine an old JS file with a new wasm file right after a deploy. It now shows
  an error, and a reload fixes it. Cache-busting would mean hashed names passed on to the
  worker; that's not worth it for now.

## Results

All on tktie, 2026-11-20.

- **Desktop unchanged:** headless `--seed 3 --duration 15 --snapshot-interval 5 --width 256
  --cell-dump`, old `main` (11989cb) vs. this branch: **all 29 output files (PNG and CSV)
  byte-identical, and the logs identical** apart from the run time.
- **Web build:** `trunk build --release` works (trunk 0.22.2 fetches wasm-bindgen 0.2.130 to
  match `Cargo.lock`, and wasm-opt version_131). Output: `world-simulator_bg.wasm` is
  **7.4 MB (2.8 MB gzipped)**, plus a 159 kB JS file.
- **Headless browser test** (Playwright, Chromium headless shell, WebGL2 on SwiftShader, page
  served by a local static server):
  - the page loads without errors, the worker starts, the planet renders, and the title
    counts up the simulated time;
  - screenshot: the planet with continents, ice cap and ocean, as on desktop;
  - **speed:** 10 steps at subdivisions 5 took 4.3 s in the worker, **~0.43 s/step** (native
    ~0.175 s/step, so ~2.4× slower);
  - **reproducibility (D6):** two runs of `?duration=10&snapshot-interval=5` logged
    **identical statistics** (74 lines);
  - **errors:** `?seed=abc` shows the clap error on the page, and
    `?set=nonexistent=1` shows "Simulation failed: unknown or untunable tectonic parameter …".
  - After the review fixes, the same checks were rerun. Two runs are again identical, and
    negative values such as `?epoch=-0.0` parse.
  - Not tested: a real GPU, WebGPU, Firefox and Safari. SwiftShader only exercises the WebGL2
    path.

## Using it

- Site (after merge): https://isibboi.github.io/world-simulator/
- Parameters as URL query parameters, using the CLI names:
  `?seed=3&subdivisions=6&plates=8&no-erosion&set=basal_drag=8e14`. `headless`, `output`,
  `width` and `cell-dump` are rejected on the web.
- Local: `trunk serve` (or `trunk build --release` and any static file server for `dist/`).
