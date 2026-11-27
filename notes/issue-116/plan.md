# #116 Web: pause/step take effect only between worker steps — plan

## Problem

The web worker runs one whole `Planet::step` per task (`setTimeout`), so a `{ paused }` or
`{ step }` message from the page waits until the step in flight is done: up to one step time,
seconds at 6–7 subdivisions on a slow device. The desktop thread also reads its requests only
between steps (it is just faster). The view draws the cog pressed at once, but the simulation
keeps going and one more state arrives, with no hint why.

## Options considered

1. **SharedArrayBuffer + `Atomics.wait`**: the worker could block mid-step at any point. Needs
   cross-origin isolation (COOP/COEP). GitHub Pages cannot send these headers; the only way is a
   service-worker shim (coi-serviceworker) that reloads the page once and rewrites responses. It
   does not work where service workers are unavailable (Firefox private windows, some embedded
   browsers, first load before the SW controls the page), adds a reload, and interferes with the
   cache handling of #122 and the IndexedDB history. Rejected: not robust.
2. **Terminate the worker on pause** and rebuild from a checkpoint: needs the full state
   serialised every step. Rejected: far too expensive.
3. **Resumable step, yielding to the worker's event loop between its parts** (chosen). Works in
   every browser without headers. The latency is bounded by the longest part instead of the
   whole step.

## Design

### Resumable step (shared, `src/planet.rs`, `src/tectonics/mod.rs`)

`Planet::step(dt)` becomes a sequence of **parts** with consistent state between them:

1. tectonics start (mantle velocities, plate velocities, deformation, substep count),
2. each tectonic substep (`Simulation::substep`, usually 1–3),
3. tectonics end (crustal flow, plate events, mantle, sea level),
4. temperature (ocean seeds, climate elevations, `TemperatureModel::update`),
5. water cycle (`Climate::update`),
6. erosion (`erode` and applying its result; skipped / ocean-only without erosion).

API: `Planet::begin_step(dt) -> StepInProgress` and `Planet::continue_step(&mut StepInProgress)
-> bool` (true when the step is complete). `StepInProgress` owns what the later parts need
(dt, substep count and index, the material-budget start values, elevation, ocean seeds, the
climate view, the part timings for the existing "Step finished" debug line). `Simulation::step`
gets the same split (`begin_step` / `substep` / `end_step`). **`Planet::step` is implemented
as the loop over the parts**, so headless runs, the desktop and the web all run the same code
in the same order: a step interrupted between parts gives bit-identical results, because
nothing but reads happens in between.

The parts' durations are measured (debug log) at 6 and 7 subdivisions; if one part dominates
(e.g. erosion), it is split further only where a natural, cheap boundary exists (no rewrite of
`erode` or the water-cycle solver in this issue).

### Run and control (shared, `src/main.rs`)

- `Run::advance_part(cli) -> Progress { Part, Step(Duration), Complete }` does one part,
  starting a new step if none is in progress; `Run::advance` loops over it (headless).
- A small shared state machine `Pausing` (paused flag, steps asked for, `after_pause`, whether a
  step is in progress) decides after each part whether to go on, and is used by both the
  desktop thread and the worker, so the semantics are the same:
  - Pause takes effect at the next part boundary, inside or between steps.
  - **Step** while paused between steps: one step. While paused inside a step (or still
    pausing): finishing the current step is the step. Then paused between steps again.
  - Resume: continue from where it stopped.
  - The step time of a step that spans a pause is not reported (as now).
- When the simulation stops for a pause, it reports it: new `SimulationEvent::Paused { in_step }`
  (web: message `{ halted: true, in_step }`).
- New fields (`request_layers`) while paused **inside** a step: the planet is between parts and
  must not be shown, so the last published state is sent again (its cells and plates are kept:
  an `Arc` clone on the desktop, the `Vec` the worker builds anyway) with the fields rebuilt as for
  past frames (`Fields::from_frame`: plate velocity without the deformation inside plates, #70);
  the exact fields come with the next step. Paused between steps: as now (current state again).
- Desktop thread: drives `Run` part by part, reads requests (and the cancel flag) after every
  part, blocks on the channel while paused. Restarts also take effect after a part, not a step.
  Snapshots are written as now.
- Web worker: one part per task. It yields through a `MessageChannel` (not `setTimeout`, which
  is clamped to ≥ 4 ms when nested, ~4 ms per part) so that page messages queued meanwhile are
  handled before the next part. While paused it stops scheduling parts (as now).

### UI feedback (shared egui, `src/viewer/timeline.rs`, `mod.rs`)

The view keeps `paused` (the user's wish, cog pressed at once as now) and learns from the
simulation whether it has actually stopped:

- paused, not yet confirmed: time text "… · pausing", tooltip "Pausing at the next consistent
  point of the step" (usually a fraction of a second; visible on slow devices);
- paused inside a step: time text "… · paused mid-step"; → tooltip "Finish the current step";
- step asked for and not arrived: time text "… · computing step" (→ is disabled as now).

All in the time label, which the narrow (phone) layout already gives a row of its own when it
does not fit; the collapsed strip shows the time label too. No new widgets, so touch sizes and
font scaling are unchanged. Desktop gets the same feedback (shared code).

## Tests

- `planet`: two planets of the same seed (4 subdivisions): one with `step()`, one through the
  parts with reads in between (state hash, cell infos) → identical `state_hash` after several
  steps.
- `Run`/`Pausing`: a scripted session (pause inside a step, new fields while paused, step to
  finish it, step again, resume) ends with the same state hash as an uninterrupted run, with
  the expected events (paused in step / between steps, republished state with the old time).
- Existing tests, clippy native + wasm32.

## Verification

Web build (`trunk build --release`) in chromium on a virtual screen at 7 subdivisions (steps of
seconds): time from pressing pause (P) to the simulation stopping, from the console log
timestamps, before (main) and after; screenshots of "pausing", "paused mid-step", "computing
step" at desktop size and at phone size 390×844; the desktop view as well. Numbers and
screenshots in `notes/issue-116/results.md`.
