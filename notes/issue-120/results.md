# Issue #120: results

## What changed
- **View play/pause** (▶/⏸, space) acts on the view only. Pausing freezes the view on its frame
  (read from the history, so it has every field for any render mode); the simulation goes on and
  the slider keeps growing ("476 Myr · live 499"). Playing walks forward through the recorded
  frames at 5 frames/s (`timeline::WALK_RATE`), each frame held 1/5 s from when it is shown; at
  the newest frame the view goes live and follows the simulation at its own pace.
- **Simulation pause/resume**: a new cog button (and the key **P**). While the simulation is
  paused the cog is drawn pressed with pause bars in its hub, and the time readout is orange, so
  the state is visible on touch screens without tooltips. Plumbing (desktop thread, web worker)
  unchanged.
- Layout: expanded bar `⌄ ⏮ ▶/⏸ ⏭ Live ⚙ [slider] time`; collapsed strip `⌃ ▶/⏸ ⚙ time` (one
  flat row; 3 touch buttons + "543 Myr · live 577" fit a 390-px phone).
- ←, →, slider: stop the walk (a drag would otherwise fight it). → on the newest frame: computes
  one step if the simulation is paused (and goes live to show it), else goes live from a paused
  view. End / Live: live. A read error stops the walk; play after an error goes live.
- Pausing the view while live with a *full* history (newest state not recorded) shows the last
  recorded frame — the only one that can be held.

## Decisions
- Space now plays/pauses the view (it paused the simulation before); P pauses the simulation.
- Once live, the view shows every state as it arrives (not throttled to 5/s). If the simulation
  computes faster than 5 steps/s (small grids: 0.15 s/step at 4 subdivisions), a playing view
  stays behind and never catches up, as the issue asks; End/Live jumps to it.
- Walk rate fixed at 5 frames/s, no option.

## Tests (vscreen)
- Desktop (1280×800, `--subdivisions 4`): pause view, live keeps growing; play walks ~4.7
  frames/s (551 → 693 Myr in 30 s with the simulation paused, then live); P pauses, → steps;
  collapsed strip. `desktop-states.png`, `desktop-view-paused.png`.
- Web, chromium 1280×800: same states (`web-desktop-states.png`). Under SwiftShader software
  rendering the walk ran ~2 frames/s: the walk waits until each frame is shown, and every frame
  costs an IndexedDB read, a surface rebuild and a slow software render. On a real GPU it should
  reach 5/s; it never skips frames.
- Web, phone 390×844 with touch emulation: strip and three-row bar with touch-sized buttons,
  view pause, play with the simulation paused catching up to live (`web-phone.png`,
  `web-phone-states.png`).
- Unit test for the walk pacing (`the_walk_goes_on_only_once_its_frame_is_shown`), forward rule
  for a paused view on the newest frame.

## Reviews
- Plan review (fresh subagent): freeze must read the frame (fields for other modes, full
  history), walk only after a successful show and stop on errors, reset the timer, → on a
  frozen newest frame, visible paused state on touch. All adopted (see `plan.md`).
- Code review (fresh subagent): hold each frame from when it is shown (not asked for), stop the
  walk on mesh-mismatch, → goes live from a paused newest frame, legible cog at desktop size (6
  teeth), tooltip from `WALK_RATE`, loading cue while the simulation is paused. All adopted.

## Not changed
- Walking backwards: no reverse play; ← decodes from the keyframe as before (#121).
- The time text (#119).

## PR #177 follow-up: 1 frame/s everywhere (Sebastian's comment)
Request: keep space as is. The throttle applies in any situation, also once live and when live
from the start, at 1 frame/s (fast/slow forward comes later).
- `timeline::Walk::rate` is one field (default `VIEW_RATE = 1.0`) that sets the pace of a
  playing view. It applies when walking through stored frames and when following the simulation
  live. A later speed control only has to change it; `Walk::interval` clamps it to 0.001–1000.
- **Live:** the view shows the **newest** state at each 1 s tick and skips the states in
  between. *Why:* live then stays live, never more than 1 s behind the simulation. Showing every
  state at 1/s would put a fast simulation further behind every second; that is what *playing*
  from a paused frame does now. A run's last state still shows at the next tick.
- **Shown at once, outside the pace:** slider drags, ←/→, End/Live, a single step asked for
  with → while the simulation is paused, the first state of a new run, and the newest state
  re-sent with the fields of a newly chosen render mode. Each of these starts a new 1 s interval.
- Live, the time label shows the time of the state on screen. The slider handle stays at the
  right end, so it can lead the picture by up to 1 s.
- Pausing the view while live freezes on the state on screen, which may lag the newest state
  (the recorded frame at or before it).
- With the simulation usually at 1–4 steps/s, playing from a paused frame (1 frame/s) does not
  catch up. Live/End jumps ahead.

Tests (vscreen): desktop: live label 203 → 210 → 217 → 225 Myr, one change per ~1 s, in shots
0.33 s apart (≈7 steps skipped per tick). The walk goes 280 → 283 Myr in ~3 s. Live after a
view pause shows in < 0.15 s (`desktop-1fps.png`). Web 1280×800: the same live ticks and walk
(`web-desktop-1fps.png`). Phone 390×844 touch: live ticks 468 → 474 → 479 Myr in 2.3 s
(`web-phone-1fps.png`). fmt, clippy (native and wasm32), doc and the full test suite
(485 passed) are clean with warnings denied.

Review (fresh subagent) found three problems, all fixed:
- A state re-sent for a new mode is now recognised by its time not being newer.
- The first state of a run starts the 1 s interval.
- The rate is clamped.

Two findings were accepted as they are:
- The slider/label mismatch of up to 1 s while live.
- A step asked for just after pausing can take one tick, if a state that was already on its way
  is taken as the step.

## PR #177 follow-up 2: live falls behind (Sebastian: "when live, don't jump ahead")
- This replaces the "newest state at each tick" rule above. A live view shows **every** state
  in turn, at most `Walk::rate` (1) per second.
- When a new state arrives before its tick, the view **falls behind**. It holds the recorded
  frame on screen and walks on through the recorded frames at the same rate, exactly like Play
  from a paused frame. Once it catches up with the newest state it is live again.
- What the UI shows:
  - **Live:** the view is on the newest state. The label shows the time, and the Live button
    is greyed out.
  - **Behind:** the label shows "X Myr · live Y". Live and End are enabled and jump to the
    newest state at once; the view then falls behind again if the simulation is quicker.
  - ⏸ still pauses the view where it is.
- If the history is full, the state on screen may not be recorded. The view then shows the
  newest state at the next tick, the only fallback left.
- Tests:
  - Desktop, live from the start: 16 → 17 → 17 → 18 → 19 → 19 Myr in shots 0.7 s apart, while
    the simulation went 116 → 142. Live jumped to 194 at once (`desktop-behind.png`).
  - Web desktop and phone: the same behaviour (`web-desktop-behind.png`,
    `web-phone-behind.png`). Under software rendering the web view managed a little under
    1 frame/s.
  - fmt, clippy (native and wasm32), doc and the full test suite (485) are clean.
