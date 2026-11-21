# Review of `origin/main..HEAD` (issue #61, commit 18fdc0e)

Read-only review; nothing built or run. Line numbers refer to HEAD of `agent/issue-61`.

Overall the design matches the description: new channel per simulation, join-before-run on
desktop, terminate + new worker on the web, `onmessage`/`onerror` cleared before the closures
are dropped, query <-> args round-trips (`-` values, repeated `--set`, `=` inside values) look
correct, and headless only changes by `ControlFlow::Continue`. Two real bugs below.

## Must fix

1. **The mesh triangles can be lost, and then the view freezes until the next restart.**
   `src/viewer/mod.rs:308-346`, `src/viewer/renderer.rs:185`.
   `ui()` drains the channel into locals and hands `faces` to one paint callback. egui 0.37
   runs several passes per frame when a pass asks to be discarded, and only paints the last
   pass. `egui::Grid` calls `ui.request_discard("new Grid")` the first time it is shown
   (`egui-0.37.0/src/grid.rs:468`), and so does a first `Resize`. The panel has two Grids: one
   appears when the panel is first opened, the other when "More options" is first expanded.
   If the first `SurfaceUpdate` of a simulation lands in such a frame, pass 1 drains it,
   pass 1's callback is thrown away, and pass 2 finds `vertices`/`faces` = `None`. The same
   happens if the callback rect has an empty clip rect (`tessellator.rs`: `if
   !clip_rect.is_positive() return`), e.g. a window with zero height.
   What happens next: if the subdivisions changed, `resources.mesh` keeps the old size and
   every later update is rejected as "Surface update of the wrong size". The planet stops
   updating until the next restart. If it was the very first simulation (`mesh: None`),
   nothing is ever drawn. Before this change the index buffer was built at start-up, so a lost
   frame only cost one vertex update.
   Fix: keep the latest `faces` (and vertices) in `Viewer` with a generation counter, pass them
   to every callback, and let `prepare` rebuild or upload only when the generation differs from
   what `PlanetResources` holds. The other option is to drain events in `App::logic` and clear
   the pending data only when the pass is not discarded, but the generation approach is
   simpler and robust.
   Confidence: high that it can happen (checked in the egui source), low that it happens often.
   The effect lasts until the next restart, and the fix is cheap.

2. **Validation gap: more plates than cells panics instead of being rejected, and on desktop
   the panic is not reported.** `src/main.rs:271-281` (`Cli::check`) vs
   `src/tectonics/mod.rs:619` (`assert!(plate_count <= mesh.len(), "more plates than cells")`).
   The panel accepts e.g. `subdivisions=0, plates=13` (12 cells) or `subdivisions=1,
   plates=50` (42 cells). `build_planet` then panics in `Simulation::new`. On desktop that kills
   the simulation thread without a `SimulationEvent::Failed`: the panel shows "Starting the
   simulation…" forever, and the panic is only logged when the *next* restart joins the
   thread (`main.rs:219`). On the web the worker reports the panic as `{error}`, so it shows up
   there. Fix: add `ensure!(self.plates <= 10 * 4usize.pow(self.subdivisions) + 2, "more
   plates than cells …")` to `Cli::check` (after the subdivisions check, so `pow` cannot
   overflow). See also Should fix 1. Confidence: high.

## Should fix

1. **Panics in the desktop simulation thread are invisible in the view.** `src/main.rs:213-260`.
   Any panic in `build_planet`/`simulate` (an `assert!`, an odd `--set` value) drops the sender
   during unwinding. `Viewer::ui` ignores `TryRecvError::Disconnected`, so the status stays
   "Starting the simulation…" or a stale "t = …". The panel makes odd parameter combinations
   much easier to reach than the CLI did. Wrap the body in `std::panic::catch_unwind`
   (`AssertUnwindSafe`) and send `Failed("panicked: …")`. Or treat "disconnected before
   `time >= duration` and no `Failed`" as a failure in the viewer. Confidence: high.

2. **Fast repeated restarts on desktop do a full build plus snapshot for every intermediate
   launch.** `src/main.rs:213-250`, `simulate` at `main.rs:364`. Each click spawns a thread that
   waits for the previous one. A thread whose channel is already dropped still runs
   `build_planet` (slow at subdivisions 7–8), then `simulate` writes the initial snapshot PNG
   (and the CSV with `--cell-dump`). Only then does `on_step` notice the dropped channel. Five
   quick clicks mean five builds in a row before the wanted simulation starts. Fix: share an
   `Arc<AtomicBool>` "superseded" flag (or a generation counter) between launches, and check
   it before `build_planet` and in a cheap pre-check before `simulate`. Confidence: high on the
   behaviour, medium on how much it matters.

3. **Restarted desktop simulations write into the same output directory.** `src/main.rs:244-247`.
   With the default `--output=output`, a restart with other parameters overwrites the
   previous run's snapshots one by one. Higher-numbered files from a longer earlier run stay,
   so the directory mixes runs with different parameters and nothing says so. At least log a
   warning. Better, give restarts a fresh subdirectory, or document it in the panel tooltip.
   Confidence: high on the behaviour. Whether it matters is for Sebastian to decide.

4. **Web: if a worker cannot be created on restart, the panel waits forever.** `src/web.rs:170-173`.
   On `start_worker` error the closure only calls `show_status` (DOM). `events` is dropped
   without a `SimulationEvent::Failed`, so the panel keeps "Starting the simulation…". Send
   `Failed(format!("{error:#}"))` through `events` (clone it before the call) and call
   `wake_up`. Confidence: high, but the case is rare.

## Nits

- `src/viewer/parameters.rs:18-21`: a `cfg(target_arch = "wasm32")` gate inside the shared
  viewer (the `HIDDEN` list). This contradicts "keep wasm gates confined to entry points".
  Pass the hidden option names in from the entry point, e.g. as a field of `UiOptions` or
  `Launch`. Then `mod query` would not need wasm-only use from viewer code either.
- `src/query.rs:83`: `u8::from_str_radix` accepts a leading `+`, so `%+f` decodes to 0x0F
  instead of being rejected. Check `hex.bytes().all(|b| b.is_ascii_hexdigit())` first.
  Harmless.
- `src/query.rs:76`: `+` decodes to a space (as before with `decode_uri_component`), so a
  hand-typed `?set=x=1e+3` becomes `x=1e 3` and fails. Links generated by `args_to_query` are
  fine (`%2B`). Pre-existing; maybe mention it in the module doc.
- `src/main.rs:320-331`: `build_planet` now runs all `--set` overrides before the CO₂ warning,
  so the stderr log order changes ("Overrode parameter" now comes after the CO₂ warning). Snapshot
  outputs are unaffected. This is only a remark next to the "byte-identical headless" claim, which
  presumably covers files only.
- `src/web.rs:142`: the page still checks only `subdivisions <= 8` before starting. It could call
  `launch.cli.validate()` and show the error at once instead of waiting for the worker. An
  invalid initial URL (unknown option) still keeps the page from starting at all, so the panel
  cannot be used to fix it. Pre-existing.
- `src/main.rs:271-281`: `width` has no upper bound (desktop panel shows it; `--width=1000000`
  → huge allocation in `snapshot`). Pre-existing CLI gap, more reachable now.
- `src/viewer/parameters.rs:306-313`: Restart uses `self.checked` (the args at the start of the
  frame) rather than `self.args()`. `launch_from` re-validates, so this is not a bug, but
  `self.args()` would be clearer.
- Simplification: `validate` (`parameters.rs:386`) only wraps `launch_from` and could be
  dropped (`launch_from(args).err()`). In `web.rs` the `on_error` closure repeats the `send`
  helper that `on_message` uses.
