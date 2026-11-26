# #116 Pause and step inside a step: results

Plan: [plan.md](plan.md) (with the plan review below). Code: `src/pausing.rs` (the shared pause
state machine), `Planet::begin_step`/`continue_step` (`src/planet.rs`),
`Simulation::begin_step`/`next_substep`/`end_step` (`src/tectonics/mod.rs`), `Run::advance_part`
and the desktop thread (`src/main.rs`), the worker loop (`src/web.rs`), the time bar
(`src/viewer/timeline.rs`, `src/viewer/mod.rs`).

## What changed

- A step runs in **parts**: tectonics start, each tectonic substep, tectonics end, temperature,
  water cycle, erosion. `Planet::step` is the loop over the parts, so every run (headless, desktop,
  web) takes the same code in the same order.
- The desktop thread and the web worker read the view's requests **after every part**. A pause
  stops at the next part boundary, inside a step or between steps. Nothing but reads happens
  between parts, so a paused run ends bit-identically.
- The simulation reports where it stopped (`{ halted, request, in_step }` on the web,
  `SimulationEvent::Halted`). Requests are numbered, so a report that a later request overtook is
  ignored.
- **Step** while paused between steps takes one step. While paused inside a step, or still
  pausing, it finishes that step. Either way the simulation then stops between steps again.
- The **time bar** says what the paused simulation does:
  - "pausing…": it has not stopped yet. This shows only after 300 ms, because it usually stops at
    once.
  - "mid-step": it stopped inside a step. The view shows the state before that step, and → finishes
    the step.
  - "stepping…": it is computing the step that was asked for.

  The wide bar puts the status after the time and reserves its width, so the slider does not jump.
  On phones it moves to its own row when it does not fit. The collapsed strip shows the status on a
  line below the time. Tooltips (desktop) explain it.
- A new render mode's fields (plate velocity) are **built by the view** from the shown state, as for
  past frames, until the simulation sends them. A simulation paused inside a step sends none, and
  the mode still works (screenshot below).
- Desktop:
  - Snapshots are written right after the step's update, instead of on resume.
  - A restart stops the previous simulation after its current part, not its step.
- Web worker:
  - It runs parts in tasks of at least 30 ms (`YIELD_AFTER`) and then yields with `setTimeout`.
  - It does not yield after every part, because nested timeouts are clamped to ≥ 4 ms.
  - It does not yield through a `MessageChannel`, whose ordering relative to the page's messages is
    not specified.

## Determinism

- **Same results as main.** A headless run (`--web-math --subdivisions 5 --seed 3`, 8 steps) gives
  the same state hashes as `main`: 19c83e17fa28cc68, e3acfce41c45777c, c996515a11b8ef9d.
- **Tests:**
  - `pausing::tests::a_run_paused_inside_steps_ends_as_an_uninterrupted_one` scripts a session:
    1. pause inside the first step,
    2. hold there for several ticks,
    3. step to finish it,
    4. step again,
    5. resume,
    6. pause inside a later step,
    7. resume.

    It checks the final state hash against an uninterrupted run, the halt reports, that states are
    only sent at whole steps, and that steps spanning a pause report no time.
  - `a_run_paused_after_its_last_step_still_ends` and `steps_only_count_while_paused` cover the
    edge cases.
  - `planet::tests::steps_taken_in_parts_end_alike` takes steps part by part, with reads in
    between.

## Pause latency

Pause latency is the time from clicking the cog to the simulation actually stopping. It was measured
in the console log timestamps in Chromium on the virtual screen (software rendering), at 7
subdivisions, seed 3. A step takes 3.8 s there. Clicks came at random times.

| | n | mean | max |
| --- | --- | --- | --- |
| web, before (main; the step in flight finishes) | 8 | **2.45 s** | 5.53 s |
| web, after | 12 | **0.57 s** | 1.40 s |
| desktop, after (step 2.9 s) | 11 | **0.37 s** | 2.05 s |

How to read the table:

- **Before**, the last state after the click arrived 0.6–5.5 s later. **After**, the worker reports
  "halted" 0.01–1.4 s later.
- **Desktop before** was not measured. It also finished the whole step, so it averaged about half a
  step (~1.5 s at 7 subdivisions).
- **Step time is unchanged**: 3.82 s per step before, 3.76 s after.

### What bounds the latency now: the longest part

The latency is now bounded by the longest part. These are native times with `--web-math`, in ms
(mean / max over 6 steps); the web takes about 1.6× as long:

| part | 6 subdivisions | 7 subdivisions |
| --- | --- | --- |
| tectonics start | 45 / 75 | 170 / 290 |
| tectonic substep (2–4 per step) | 11 / 18 | 52 / 107 |
| tectonics end | 48 / 55 | 243 / 282 |
| temperature | 102 / 104 | 163 / 176 |
| water cycle | 80 / 118 | 692 / 1278 |
| erosion | 139 / 147 | 877 / 925 |

- **6 subdivisions:** the web pauses within ~0.25 s at most.
- **7 subdivisions:** the water cycle and the erosion parts take up to 1–2 s on the web.
- **Possible follow-up:** splitting these two further, inside the water-cycle solver (month slots,
  Newton iterations) and inside `erode` (drainage, incision, routing). That means making their
  internals resumable, a larger change. Until then, "pausing…" makes the wait visible.

## Screenshots

Web (Chromium, 1280×800, 7 subdivisions):

- [web-pausing.png](web-pausing.png): "pausing…" while the water cycle or erosion part finishes.
  This was taken with an earlier build, in which the slider still changed width on pausing.
- [web-mid-step.png](web-mid-step.png): paused inside a step.
- [web-stepping.png](web-stepping.png): → was pressed and the rest of the step is being computed.

Phone size (390×844):

- [phone-strip-midstep.png](phone-strip-midstep.png): the collapsed strip, with the status below
  the time.
- [phone-bar-midstep.png](phone-bar-midstep.png): the expanded bar.
- [phone-bar-stepping.png](phone-bar-stepping.png): "stepping…" gets its own row.
- These were taken in a narrow desktop Chromium window, so the pointer is fine, not coarse. Touch
  sizes are unchanged (no new widgets).

Desktop (native window, 7 subdivisions):

- [desktop-mid-step.png](desktop-mid-step.png): paused inside a step.
- [desktop-mode-switch-mid-step.png](desktop-mode-switch-mid-step.png): switched to Plate motion
  while paused mid-step. The arrows are built by the view, and → then finished the step with the
  simulation's fields.

## Plan review (independent agent) and how it was followed

- **R1, fractional time mid-step:** states are published only between steps. There is a debug
  assert in the worker's `publish`, and the test checks whole-step times.
- **R2, stale pause reports:** requests are numbered and the report echoes the latest one.
- **R3, layout:**
  - Short status words.
  - The status width is reserved in the one-row layout.
  - The strip uses a second line.
  - "pausing" shows only after 300 ms.
  - The status is visible text, not only a tooltip.
- **R4, latency at 7 subdivisions:** measured and stated above, with the follow-up split points.
- **S1:** the view builds missing fields instead of the simulation re-sending the last state.
- **S2:** time-budget yielding with `setTimeout` instead of a `MessageChannel`.
- **S3:** the step's span is kept in `StepInProgress` and entered per part. The "Step finished"
  times sum the parts.

## Code review (independent agent) and fixes

- **No determinism bug found.** The parts follow the old step order. Nothing in the planet code
  caches through interior mutability, states are published only between steps, and the worker
  cannot schedule itself twice.
- **Fixed: "pausing…" could show after a requested step had arrived.** It happened while the
  simulation was writing the step's snapshot, before it reported the stop. The 300 ms wait now
  restarts when a new state arrives.
- **Fixed: desktop pace.** The time of an extra update with new fields no longer counts into the
  next step's time, the same as on the web.
- **Fixed:** doc comments (`Run`, the worker's flags) and the order of the enum and its impl.
- **Kept as is:**
  - `steps_taken_in_parts_end_alike` runs the same code on both sides, because `Planet::step` is the
    loop over the parts. Equivalence with `main` rests on the hash comparison above. A pinned hash
    would break with every physics change.
  - The view rebuilds missing fields on a mode switch also while the simulation runs. The cost is
    one pass over the cells, and the new mode shows at once instead of after the step in progress.
  - Reserving the status width affects only the one-row layout of wide screens. Narrow screens
    always use the two- or three-row layout, chosen by the shown text.
  - A held desktop thread repeats its halt report after every request. This is harmless, because the
    view ignores repeated reports.
