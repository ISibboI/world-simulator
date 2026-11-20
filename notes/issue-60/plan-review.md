# Review of notes/issue-60/plan.md (web version) — independent plan review, 2026-11-20

Verdict: **sound plan, go ahead with M0**, after the must-fix items below are folded in. The
architecture (lib + thin entries, shared `Viewer`, one worker, no wasm threads, trunk, Pages via
Actions) is the right one. Checked against the code and the vendored sources (eframe 0.37,
egui-wgpu 0.37, wgpu 30); no builds run.

## Claims verified
- eframe web API: `WebRunner::new()` + `async fn start(&self, HtmlCanvasElement, WebOptions, AppCreator)`
  exists; `WebOptions` has `depth_buffer`, `renderer`, `wgpu_options`, no multisampling field. OK.
- MSAA: `eframe/src/web/web_painter_wgpu.rs:65` creates the depth texture with `sample_count: 1`
  and `RendererOptions` keeps the default msaa 1, so the planet pipeline must use 1 on wasm.
  Correct. (Consequence: no anti-aliasing on the web; acceptable, mention it as a known
  difference.)
- `Depth32Float` via `depth_format_from_bits(32, 0)`: fine on WebGPU and WebGL2.
- Colour target: pipeline uses `render_state.target_format` on both targets, so no sRGB
  mismatch between native and web. OK.
- getrandom: `cargo tree --target wasm32-unknown-unknown -i getrandom@0.4.3` shows only
  `rand 0.10` → world-simulator; getrandom 0.3 (ahash/tempfile) is not in the wasm graph. The
  `default-features = false, features = ["chacha","std"]` fix is right; code only uses seeded
  `ChaCha8Rng` + `Rng/RngExt` (no `rand::rng()`/`from_os_rng`). Does not change the ChaCha stream.
- `Instant::now()` panics on wasm32-unknown-unknown; `std::thread::spawn` panics; `std::fs` errs:
  all correct. `std::env::var` (EnvFilter, temperature.rs tests) just returns `Err`: harmless.
- RandomState maps (sphere.rs, ocean.rs, climate.rs) are only used via entry/get: no
  iteration-order dependence. OK.

## Must-fix (plan changes before M0/M1)
1. **Two bins break `cargo run`.** Adding `src/bin/sim-worker.rs` makes `cargo run --release -- …`
   (used everywhere in the repo's docs/agent workflow) fail with "could not determine which
   binary to run". Add `default-run = "world-simulator"` to `[package]`, and give the worker bin a
   native stub (`#[cfg(not(target_arch="wasm32"))] fn main() {}`) so `cargo build/test/clippy
   --all-targets` stay green natively. Alternatively use the one-bin + JS-shim worker (see S1),
   which avoids the issue entirely.
2. **`Cli::parse()` must not be used on the web.** clap's `parse()` calls `process::exit` (panics/
   aborts on wasm), and `--help`/errors go to stderr (nowhere). Use `Cli::try_parse_from(...)` on
   wasm and render the clap error/help text in the page (or an egui label). Define the query→argv
   mapping explicitly: iterate `web_sys::UrlSearchParams` (it percent-decodes); `key=value` →
   `--key value`; empty value / `true` → bare `--flag`; repeated keys (`set=a=1&set=b=2`) → repeated
   `--set`. Reject `output`, `cell-dump`, `headless`, `width` with a clear message rather than
   silently ignoring them.
3. **Logging timestamps panic on wasm.** `tracing_subscriber::fmt()` with the default timer calls
   `SystemTime::now()`, which panics on wasm32-unknown-unknown. With `tracing-web` use
   `.without_time()` (or `tracing-wasm`), and install the subscriber **and**
   `console_error_panic_hook` in **both** wasm instances (page and worker are separate modules).
4. **PR build is required, not optional (D4).** §6.1 calls `trunk build --release` an "optional
   extra", but D4 says PRs build the site. Make it a real job (build only, no deploy) so the
   HTML/worker wiring and wasm-bindgen/wasm-opt steps are checked on every PR. Simplest: one
   `pages.yml` with `on: push (main), pull_request, workflow_dispatch`, a `build` job that always
   runs and uploads the artifact, and a `deploy` job with
   `if: github.event_name != 'pull_request' && github.ref == 'refs/heads/main'`.
5. **`simulate` is a blocking loop; the worker design needs a stepper.** The plan says the worker
   "yields to its event loop between steps", but `simulate(planet, cli, on_step)` is a synchronous
   `for` loop. In M0 refactor it into a resumable struct (`SimulationRun { planet, step, steps,
   snapshot_every, ledger }` with `fn advance(&mut self) -> bool`); native keeps a `while
   run.advance()` loop (byte-identical), the worker drives `advance()` from a `setTimeout(0)`/
   `spawn_local` loop. Without this, #61's pause/restart messages can never be received. (Posting
   *out* of a blocking worker loop does work, so M2 alone would survive, but don't build on it.)

## Should-fix
1. **Worker wiring: pick one approach now, not "in M2".** Both are feasible:
   - (a) trunk `data-type="worker"` + `data-loader-shim` for a `sim-worker` bin: documented trunk
     pattern (its `webworker` example); the worker wasm contains only the simulation (smaller,
     faster to load). Cost: second bin (see M1), worker URL must be the loader shim's fixed name
     resolved relative to the page (`new Worker("./sim-worker_loader.js")` — works with
     `public_url` because Pages redirects `/world-simulator` → `/world-simulator/`).
   - (b) one module: a hand-written `worker.js` copied by trunk (`rel="copy-file"`) that
     `importScripts`/`import`s the same wasm-bindgen glue and calls an exported
     `#[wasm_bindgen] fn worker_main()`. Needs stable output names (`filehash = false`, or pass the
     hashed URL to the worker in its first message) and instantiates the full eframe wasm twice.
   Recommendation: (a), with `default-run` (M1). Spike it at the start of M1 rather than M2, since
   it determines the crate layout.
2. **Transfer = still two copies; fine, but say so.** Worker: wasm memory → fresh `ArrayBuffer`
   (`Uint8Array::from(bytemuck::cast_slice(..))`, then transfer); main: `Uint8Array::copy_to` into a
   `Vec<Vertex>` → `Arc`. ~1.6 MB/step is negligible; don't over-engineer. Define a tiny message
   format (`{kind, time, vertices}`) so pause/restart/error fit in later.
3. **Worker failures must be visible.** Register `worker.onerror`/`onmessageerror` on the main
   thread and post `{kind:"error", message}` from Rust on `Err`/panic (panic hook in the worker),
   and show it in the egui UI ("simulation stopped: …"). Otherwise the planet just freezes silently.
4. **Pin the toolchain pieces in CI.** Pin the trunk version (download the release binary or
   `cargo binstall trunk@<x.y.z>`; `cargo install` takes minutes, `jetli/trunk-action` is
   unmaintained/old Node). In `Trunk.toml` pin `[tools] wasm_opt` to a recent binaryen version:
   current Rust emits bulk-memory/nontrapping-fptoint/reference-types etc. by default and older
   wasm-opt rejects them. wasm-bindgen CLI: trunk auto-matches the `wasm-bindgen` version in
   Cargo.lock — keep `wasm-bindgen` as a direct dep so that works, and note that a `cargo update`
   bumping it is fine under trunk (but would break a hand-installed `wasm-bindgen-cli`).
5. **Don't put LTO/cgu=1 into the shared release profile.** Native release builds are what all
   simulation runs use; `lto` + `codegen-units = 1` slows every native build. Add
   `[profile.web] inherits = "release"` with the wasm settings and use
   `data-cargo-profile="web"` in `index.html`. Put `+simd128` in `.cargo/config.toml`
   `[target.wasm32-unknown-unknown] rustflags`, not `RUSTFLAGS` (cache invalidation, local/CI
   divergence).
6. **wasm CI job flags:** use the repo's convention `cargo clippy --target wasm32-unknown-unknown
   --all-targets --all-features` (with warnings denied). `--all-targets` compiles the test
   modules and the native-stub bins for wasm too; that is wanted (catches drift) and should pass
   since tests only use `std::env`/`std::fs` at runtime. If something test-only cannot compile for
   wasm, gate that test, not the CI flag.
7. **Pages workflow details:** top-level `permissions: contents: read`; `deploy` job only:
   `pages: write`, `id-token: write`, `environment: { name: github-pages, url: ${{
   steps.deployment.outputs.page_url }} }` (deploy-pages requires the environment);
   `concurrency: { group: pages, cancel-in-progress: false }`. `actions/configure-pages` is
   optional; use it only if you want `base_path` instead of hard-coding `/world-simulator/`.
   Pin actions to major tags like the rest of ci.yml.
8. **Make the web determinism check concrete and cheap.** Instead of "browser or Node/wasmtime if
   cheap": add a lib function `fingerprint(seed, subdivisions, steps) -> u64` (hash of surface
   elevations bits), a native test calling it twice, and a `wasm-bindgen-test` run in Node
   (`cargo test --target wasm32-unknown-unknown` with `wasm-bindgen-test-runner`, subdivisions 3,
   a few steps). This actually exercises the wasm libm path and catches accidental wall-clock or
   iteration-order dependence. Also keep `relaxed-simd` off explicitly (already noted).
9. **M0 identity check:** specify it as: same seed, headless, e.g. 15 Myr at subdivisions 5 with
   `--cell-dump`, before/after the refactor, `cmp` all PNG/CSV files and diff the logs minus
   timings. (The plan's 2026-11-20 determinism run already shows this is a valid oracle.)

## Nits
- Add `dist/` to `.gitignore`; document `trunk serve` (needs `rustup target add
  wasm32-unknown-unknown` — or add `targets = ["wasm32-unknown-unknown"]` to rust-toolchain.toml so
  CI and local get it automatically; costs one std download per toolchain install).
- Window title: `ViewportCommand::Title` is ignored on web; the shared overlay label is the right
  fix. Setting `document.title` too is fine.
- Canvas: give it CSS `width:100%;height:100%` and `touch-action:none`, otherwise drag-rotation
  on touch devices scrolls the page.
- `--subdivisions` default differs per target (6 native, 5 web): implement as a wasm-only
  default injected into argv before parsing (prepend `--subdivisions 5`, later args win since it is
  a single-value option — verify clap's "last wins" for that arg, or use `#[arg(default_value_t =
  DEFAULT_SUBDIVISIONS)]` with a cfg'd const). Keep the `--help` text truthful.
- The web snapshot statistics (`snapshot()` logs) still run in the worker — fine, but they go to
  the console; consider lowering their level on wasm if they cost noticeable time.
- The `agent-scratch/wasm-exp/` experiments cited in §1 are no longer in the worktree; drop the
  path or say they were deleted.
