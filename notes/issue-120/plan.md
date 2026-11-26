# Issue #120: play/pause walks the view, a separate simulation pause

Issue: the ⏸/▶ button of the time bar pauses the simulation. It should instead play the *view*:
walk forward through the stored frames at a reasonable rate (~5 frames/s) until the newest one,
then follow live. Pausing it freezes the view only; the simulation keeps running. Pausing the
simulation gets its own button.

## Today (main at 2fcc647)
- `Viewer.viewing: Option<usize>`: `None` = follow the newest state, `Some(i)` = recorded frame
  `i` (decoded by `History`, forward deltas from a keyframe every 25 frames, incremental from the
  last decoded frame). A past frame is implicitly "frozen"; the slider grows meanwhile.
- `paused`/`step_pending`: the simulation pause (`SimulationControl::set_paused` → desktop thread
  `Request::Paused`, web worker `{paused}`), → computes one step while paused on the newest frame.
- Keys: space = simulation pause, ←/→ = frame back/forward, End = live.

## Design
View state = `viewing` plus a new `playing: bool` (true at start and after a restart).
- **Live** = `viewing == None` (always "playing"). Unchanged: every new state is shown at once.
- **View pause** (play/pause button or space, while live): freeze on the shown state —
  `viewing = Some(last recorded)`, without a new read, since the display already shows it. If the
  history is full (the newest state was not recorded) the view shows the last recorded frame;
  documented, rare (4 GB desktop / browser quota).
- **Play** (from a past frame): `playing = true`. Each UI frame, if no read is in flight and
  ≥ 1/WALK_RATE s passed since the last advance, `view(position + 1)`; reaching the last position
  goes live (as `view` already does). A repaint is requested for the remaining interval. Frames
  decode incrementally forward (one delta per step), so 5 frames/s is cheap; if decoding or the
  surface rebuild is slower, the walk just slows down (it waits for each frame to be shown).
- **Live speed vs walk rate**: step times are 0.25–1 s by design (`pace.rs`), i.e. live is slower
  than 5 frames/s, so the walk catches up and then follows live at live speed. If a small run
  computes faster than 5 steps/s the walk stays behind (never catching up) — as the issue reads.
  Once live, the view shows every state as it arrives (as today); it is not throttled.
- `WALK_RATE = 5.0` frames/s, a constant; no option (not worth UI space now).
- Manual navigation stops the walk: ←, →, the slider (Seek) set `playing = false` (a drag would
  otherwise fight the walk). Live / End → live (playing).
- **Simulation pause**: a separate button and the key **P**. Icon: a drawn cog (the
  "computation"), with a pause badge (two bars) while running → click pauses; with a play badge
  while paused → click resumes. Tooltips "Pause the simulation (P)…" / "Resume the simulation
  (P)". Disabled when the run is complete. Behaviour unchanged (desktop thread and web worker
  untouched).
- → on the newest frame while the simulation is paused still computes one step (unchanged).
- **View play/pause** keeps the ▶/⏸ icons and space; tooltips "Play: walk through the frames
  (5/s) up to live (space)" / "Pause the view; the simulation continues (space)". Always
  enabled (replaying a finished run works).
- Layout: expanded bar `[⌄] [⏮] [▶/⏸] [⏭] [Live] [⚙]` (media controls together, simulation
  apart after Live); collapsed strip `[⌃] [▶/⏸] [⚙] time` — still one flat row, one extra square
  button (~48 px on touch; fits a 390-px phone with "275 Myr · live 317"). `buttons_width` counts
  one more square button for the 1/2/3-row decision.
- Time label tooltip: "Simulation paused" while paused.
- Web: nothing worker-specific; the walk is viewer-side, IndexedDB reads wake the view already.

## Changes after the plan review (fresh subagent)
- Freezing at live reads the frame from the history (`request(last)`) instead of only setting
  `viewing`: the shown state then has all fields (`Fields::from_frame`), so switching modes while
  frozen does not wait for layers the simulation will never resend to a past view, and with a
  full history the label, slider and shown state agree (the view shows the last kept frame).
- The walk advances only after the frame at `viewing` was shown successfully (tracked index of
  the shown history frame), not merely "no read in flight"; a read error stops the walk.
- The walk timer resets on play (the first step is immediate) and on restart.
- → on a frozen newest frame: goes live, and computes one step if the simulation is paused
  (otherwise disabled, as on live); keys obey the same enabled rule as the buttons.
- Simulation button: drawn *selected* while the simulation is paused, cog icon with pause bars in
  its hub when paused; the time label is tinted (warning colour) while the simulation is paused,
  so the state is visible on touch screens without tooltips. Text unchanged (#119).
- `request_repaint_after(remaining)` only while walking; nothing extra while paused.

## Not in scope
- Walking backwards (no reverse play). ← uses the existing decode (from the keyframe); #121.
- #119 label jitter: the time text is unchanged.

## Tests
- Unit tests for a small pure `Walk` helper (when to advance given playing / loading / elapsed),
  the existing timeline tests updated (button widths).
- vscreen: desktop 1280×800 and web in chromium 1280×800 and 390×844 touch; screenshots in
  `notes/issue-120/`.
