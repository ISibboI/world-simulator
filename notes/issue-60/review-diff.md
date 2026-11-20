# Review of the issue-60 diff (web version)

Scope: `git diff origin/main..HEAD -- . ':!notes'` at 27f740d. Read-only review; nothing was built.

Overall: the design fits the requirements. It uses one code base, keeps cfg gates at the entry
points and file output, runs one blocking Web Worker with no wasm threads, parses parameters
through the same clap `Cli`, and has least-privilege Pages permissions. I found no cfg mistakes
that would change desktop behaviour: `web_time::Instant` is `std::time::Instant` on native,
MSAA stays 4, and `build_planet` is the old code moved, not changed. `rand` without default
features doesn't change the algorithm. The page/worker split, the transfer of the vertex
buffer, and `copy_from`/`copy_to` on same-type typed arrays (bit-exact) are correct. eframe's
web painter uses `sample_count: 1` and `depth_format_from_bits(depth_buffer, 0)`, so passing
`msaa_samples = 1` and the same `DEPTH_BUFFER_BITS` matches it. The trunk tarball has `trunk`
at its root, so the `tar -xz … trunk` install works.

## Must-fix

None that break the build or desktop behaviour. The first two should-fix items are the closest:
with them missing, a broken deploy or a crashed worker leaves the page stuck on "Starting the
simulation…".

## Should-fix

1. **Worker crashes and load failures never reach the page.** `web/worker.js:6`,
   `src/web.rs:700-747`.
   - `main` runs inside `init()`, which is async. A Rust panic in the worker (the trap after
     `console_error_panic_hook`) is therefore an unhandled promise rejection inside the
     worker. Rejections don't fire the `Worker` `error` event on the page.
   - A 404, import error or wasm instantiation failure of `worker.js` / `world-simulator.js`
     fires `error` on the `Worker` object, but no handler is installed.
   - In both cases the page keeps showing "Starting the simulation…" forever.
   - Fix in `worker.js`:
     `init().catch((e) => postMessage({ error: String(e && e.message || e) }));`
     In `start_worker`, also set `worker.set_onerror(...)` with a closure that calls
     `show_status(&format!("Simulation failed: {}", event.message()))`. This needs the web-sys
     features `ErrorEvent` (and `EventTarget`), and the closure must be `forget()`-ed like
     `on_message`.

2. **A worker start error is overwritten.** `src/web.rs:686-694`.
   - `start_worker` runs synchronously inside `app_creator`, before `run_web(...).await`
     returns. If it fails (for example a CSP or module-worker error), it shows the error, and
     then line 694 replaces it with `show_status("Starting the simulation…")`.
   - Fix: call `show_status("Starting the simulation…")` before `start_worker` inside the
     closure, and drop the call after the `await`. Alternatively, have the closure record the
     failure and show the status only on success.

3. **A malformed or mismatched vertex message can panic or overflow the GPU buffer.**
   `src/web.rs:724-728`.
   - `Float32Array::copy_to` panics when the lengths differ (js-sys documents this). `dst` has
     `floor(len / 10) * 10` floats, so any length that isn't a multiple of 10 panics the page.
   - A length larger than `vertex_count` would make `queue.write_buffer` overrun
     `vertex_buffer` (`renderer.rs:176`), which is a wgpu validation panic.
   - Today page and worker parse the same query, so this cannot happen. It is still the only
     panic path in the message handler, and #61 (restarts with new parameters) makes a
     mismatch plausible.
   - Fix: move `vertex_count` into the closure and check
     `floats.length() as usize == vertex_count * floats_per_vertex` before allocating. On a
     mismatch, log `error!` and return.

4. **Option values that start with `-` are parsed as flags.** `src/web.rs:619-622`.
   - `?epoch=-1` becomes `["--epoch", "-1"]`. Without `allow_negative_numbers`, clap treats
     `-1` as an option and fails ("a value is required").
   - `?no-erosion=true` becomes `--no-erosion true`, which fails with a confusing "unexpected
     argument 'true'".
   - Fix: push one token, `format!("--{name}={value}")`, when a value is present. This is how
     you would write it on the command line. Clap then accepts leading `-` values and gives a
     clear "unexpected value for flag" error.
   - `set=basal_drag=8e14` still works, because clap splits on the first `=`.

5. **Pages cache vs. `filehash = false`.** `Trunk.toml:8`, `web/worker.js:4`.
   - GitHub Pages serves with `Cache-Control: max-age=600`. For up to 10 minutes after a
     deploy, a browser can combine a cached `world-simulator.js` with a new
     `world-simulator_bg.wasm` or the reverse. The result is a wasm-bindgen import/LinkError,
     and with item 1 unfixed, a silent hang.
   - Cheap fix: keep `filehash = false`, but have the page add a version to the worker URL
     (for example `?v=<env!("CARGO_PKG_VERSION")>` or a build id) and pass the wasm URL to
     `init({ module_or_path: … })` in the worker.
   - Alternatively, accept and document it. Low impact (a reload fixes it) but worth one line.

## Nits

- `src/main.rs:238-239`: the doc comment says the web is "about 1.5× slower", but
  results.md measured about 2.4× (0.43 s/step vs 0.175). Update the number.
- `.github/workflows/pages.yml:8-10`: `push` (all branches) plus `pull_request` builds every
  PR commit twice, the same as ci.yml presumably. Consider `push: branches: [main]`, or
  `cancel-in-progress: ${{ github.event_name == 'pull_request' }}` so superseded PR builds are
  cancelled. Deploys on main stay uncancelled.
- `pages.yml`: add a `timeout-minutes` to the build job (trunk downloads wasm-bindgen/wasm-opt
  at build time and can hang). Optionally add `github.repository == 'ISibboI/world-simulator'`
  to the deploy condition so forks don't try to deploy.
- `pages.yml:87-91`: the condition is duplicated on the upload step and the deploy job.
  That's fine, but a job-level `env`/output or a comment saying they must stay in sync would
  help.
- `src/web.rs:729-732`: `show_status("")` and `set_title` run on every message. The viewer
  already sends `ViewportCommand::Title`, which the web runner ignores, so this is needed, but
  hiding the status once (with a `bool`) is enough.
- `src/web.rs:745-746`: `std::mem::forget(worker)` is fine for now. For #61 (restart = new
  worker) the `Worker` handle will need to be kept (and `terminate()`d), so a comment pointing
  there would help.
- `src/main.rs:1-3`: crate-wide `allow(dead_code)` on wasm also hides real dead code in
  wasm-only paths. It's acceptable because native still checks, but
  `#[cfg_attr(target_arch = "wasm32", allow(dead_code))]` on the few file-output items would
  be tighter.
- `src/web.rs:612-618`: the rejected-options list duplicates knowledge of `Cli`. Consider a
  comment in `Cli` next to `headless`/`output`/`width`/`cell_dump` saying "rejected on the web
  in `web::parse_query`".
- Not tested (as noted in results.md): a real GPU, WebGPU, Firefox and Safari. Cross-browser
  reproducibility (D6) rests on Rust's own libm in wasm and no relaxed SIMD. That is sound,
  but one run in Firefox comparing the logged statistics would confirm it cheaply.
