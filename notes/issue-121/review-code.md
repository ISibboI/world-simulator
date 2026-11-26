# Code review of cf7dd6a (issue #121): time bar slider + history checkpoints

Read-only review. Nothing was built or run. Files: `src/viewer/history/mod.rs`,
`src/viewer/timeline.rs`, `src/viewer/mod.rs`, `src/viewer/history/store.rs`,
`src/web/frames.rs`, plus egui 0.37 sources for `UiBuilder::scope_id`.

**Verdict: no serious (correctness or crash) bugs found.** The request/cancel/stale state
machine, the checkpoint/base handling and the slider id fix hold up under every scenario I traced.
Below are minor issues, ordered by severity, each with a fix.

## What I checked and found correct

- **Base lookup** (`start_read`, mod.rs:310-316) only uses checkpoints in `key..=target`, so
  checkpoints from other key intervals, and forced off-cadence keyframes, cannot give a wrong
  base. The checkpoint grid `(index - read.key) % CHECKPOINT` counts from the real keyframe.
  `retain` short-circuits, so `index - key` cannot underflow. Since `base >= key`, the first
  frame read is never a keyframe when `base` is `Some`.
- **Base gone between start and finish:** only `check_writes` can remove a checkpoint while a
  read runs (`decode` runs once per read, and only one read runs at a time). That case returns an
  error and never decodes on a wrong base. See L1 for the error message.
- **Ordering:** only one read runs at a time, and a checkpoint hit for `wanted` is served only
  after the read in flight finishes (in `finish_read` → `start_read`, where it overwrites the
  older `ready`). So an older frame is never shown after the newest requested frame.
- **Requesting the frame being read** (also after `cancel`) clears `wanted` and `stale`.
  `cancel` clears `ready`, so a result already decoded is not shown after going live. The stale
  read is still kept as a checkpoint. `is_loading` ignores a stale read.
- **Errors:** an error from an older request is dropped when a newer request waits, and an
  error from a stale read is dropped. `decode` returns before it touches `checkpoints`, so a
  failed decode leaves the cache as it was.
- **Slider id:** with `UiBuilder::scope_id(Id::unique("time slider"))`, the child Ui's
  `scope_id` and `unique_id` are the given id (egui ui.rs:255-263; `Id::unique` is a
  deterministic hash despite its name). The slider's auto id is therefore the same in Rows
  One/Two/Three. The slider is drawn at most once per frame, and the strip has no slider, so the
  id is unique.
- **Longest text:** the label is monospace, and `shown <= live_time` (times start at 0), so
  "X Myr · live L" is never wider than "L Myr · live L". The `.max(width)` covers the rest.

## Findings (all low severity)

### L1: misleading error when a write fails under a read in flight (history/mod.rs:218-222, 383-390)
`check_writes` drops checkpoints at or after `failed`. If the read in flight has
`base >= failed`, `decode` fails with "the frame before it is gone". The real cause is that the
target is lost, and `start_read` would have said "this frame could not be stored".
**Fix:** at the top of `finish_read`, after `take_read`, check
`self.frames[read.target].lost` and replace the error with the same text `start_read` uses.
Alternatively, map a missing base to that message in `decode`.

### L2: memory peak in `decode` above the guard (history/mod.rs:396-431)
`kept` clones every checkpoint state of the read. A read from the keyframe to frame 24 holds 5
clones, plus `state`, plus the clone pushed for `read.target`, plus the existing checkpoints.
Only after all that does the 32 MB guard evict. Then `frame()` builds the `Vec<CellInfo>`
(larger than the 55 B/cell planes). At 6 subdivisions this is about 14 MB of planes, which is
fine. At 7–8 subdivisions (9–36 MB per state) the peak is several times `CHECKPOINT_BYTES`, and
wasm memory never shrinks.
**Fix:** compute `size(state)` once (all states of a mesh are about the same size) and only
push into `kept` while `(checkpoints + kept + 1) * size <= CHECKPOINT_BYTES`. Prefer the
checkpoints nearest the target, as the eviction does.

### L3: checkpoints stay in memory while live (history/mod.rs `cancel`)
After `go_live`, up to 6 decoded states (≤ 32 MB) stay in memory until the run ends. This is
deliberate (it speeds up the next step back) and bounded, but it is not documented.
**Fix (optional):** say so in the doc comment of `cancel`, or keep only the last decoded state
on `cancel`.

### L4: the time label and the globe disagree while scrubbing (viewer/mod.rs:992-1007, 1116/1138)
`poll_history` now shows any decoded frame while `viewing.is_some()`. Meanwhile the time label
and the slider show the time of `viewing` (the requested frame). So for one or more frames the
globe shows frame A under a label for frame B. The label is greyed (`loading`), so this is
acceptable UX.
**Fix (optional):** show the label from `shown_time` while loading, e.g. "≈ A Myr". Also
document in `TimelineView` that the label can lead the picture.

### L5: a cached checkpoint waits behind a slow read (history/mod.rs:254-262)
`request(i)` where `i` is a checkpoint, with an unrelated read in flight, waits for that read
(an IndexedDB round trip) instead of showing `i` at once. This is correct as written. Serving
the frame at once would need marking the read in flight as superseded, so its result is not
shown afterwards (otherwise it would arrive out of order).
**Fix (optional):** in `request`, if a checkpoint equals `index`, set `ready` and
`stale = true` for the read in flight, and clear `wanted`. Add a test for the ordering.

### L6: layout edge cases (timeline.rs)
- **Live time gains a digit** (999 → 1000 Myr) while viewing the past during a drag: `longest`
  grows by one monospace character. In Row One the slider's right end moves about 7–9 pt once,
  and at the threshold the layout can switch from One to Two once. The id stays the same, so the
  drag continues. This is a one-off jump, not the old flip loop, so it is acceptable.
  **Optional fix:** reserve one more digit, e.g. `format!("{0:.0}0 Myr · live {0:.0}0")`. This
  moves the jump to the next power of ten, and only for the reserved width.
- **Rows Two → Three** on leaving live (narrow screens): the slider keeps its width (`inner`) and
  stays the last row of a bottom-anchored Area. So it moves only for the one frame in which egui
  places the Area with last frame's height. The error row (`view.error`) below the slider moves
  it up by a row while dragging over a lost frame. Neither ends the drag, thanks to the stable
  id. No fix needed.
- `TimeBar::slider` is written every frame in production but read only by tests. It is
  harmless. `#[cfg(test)]` on the field, with a `cfg`-gated assignment, would be cleaner than
  `allow(dead_code)`.

### L7: test gaps (history/mod.rs tests, timeline.rs tests)
The new tests are good: requests overtaken while reading, a request for the frame being read,
cancel and re-request, steps back with bounded read lengths, and the slider rect/id across
live/past at five widths. Missing:
- a failed write (`failed_write`) with checkpoints at or after the failed frame, and the read in
  flight on such a base (would have caught L1);
- off-cadence keyframes. `stepping_back_reads_a_few_frames` detects "crossing" with
  `range.end % KEY_INTERVAL == 0`, which assumes the 25-frame cadence. A test that records a
  forced keyframe (e.g. via `FrameEncoder::force_keyframe` in the test's `record`) and steps
  back across it would cover the `key_of`/grid logic;
- the `CHECKPOINT_BYTES` eviction (e.g. a test-only smaller limit, or many cells);
- a stale read that fails, followed by a new request (the error must not be shown, and the new
  request must start);
- the viewer side: no test asserts that `poll_history` shows an intermediate frame while
  `viewing` is set, or that nothing is shown after `go_live`. Both are covered only through
  `History`.
- The assertion `frames < 3 * steps` in `stepping_back_reads_a_few_frames` is very loose. The
  per-range assertion carries the test, so it is fine as is.

## Summary
No serious bugs. Recommended before merge: L1 (one-line error mapping) and the failed-write and
off-cadence tests from L7. Everything else is optional polish or documentation.
