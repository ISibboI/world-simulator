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
