# #121 (and #119): results

Cause and plan: [plan.md](plan.md); plan review: [review-plan.md](review-plan.md).

## What changed

- **Time bar (`timeline.rs`).** The one-row layout reserves the width of the longest time
  text (`"{live} Myr · live {live}"`), right-aligned, so the slider keeps its width when the view
  leaves or returns to live. The choice between one and two rows uses that width; two vs. three
  rows (#161, phones) still uses the shown text. The slider keeps one egui id in every layout
  (`scope_id`), so a drag survives the time text moving to its own row on phones.
- **History (`history/mod.rs`).** Every decoded frame is handed to the view, also when a newer
  request waits (the label keeps the requested position, greyed while loading); `cancel()` on
  going live drops requests and marks a running read stale; a request for the frame being read
  waits for it. Decoded checkpoints of the current key interval (every 5th frame from its
  keyframe, plus the last decoded frame; ≤ 6 states, ~13 MB at 6 subdivisions, capped at 32 MB)
  replace the single last decoded state. Codec format unchanged (version 11).

## Measurements

Slow drags with xdotool on a virtual screen (software rendering, so egui frames are slow:
~100 ms on the web).

| run | seeks | frames shown | seeks back to live | frames per read (mean) | decode per read (mean / max) |
| --- | --- | --- | --- | --- | --- |
| web, 6 subdivisions, backward from the live end, before | 122 | 58 | 21 (flipping) | 12.1 | 78 / 205 ms |
| same, after | 97 | 97 | 0 | 3.8 | 29 / 190 ms |
| web phone 390×844 touch, 5 subdivisions, backward from live, after | 170 | 170 | 0 (drag continues into the three-row layout) | | |
| desktop, 5 subdivisions, backward from live, after | 73 | 72 | 0 | | |
| desktop, forward to the right end (#119), before | 190 | | 47 jumps back (past ↔ live flipping) | | |
| same, after | 95 | | 0 | | |

The 190 ms maximum after the fix is the one read per key interval that crosses into the
previous interval and decodes it from its keyframe; the other backward steps read ≤ 4 frames.
Decode cost per delta frame: ~0.45 ms native at 5 subdivisions, ~7–10 ms on the web at 6
subdivisions in this container (one contended core). IndexedDB reads return at the next egui
frame (~100 ms here).

Before, the web log at 6 subdivisions also had 35 of 92 reads discarded because a newer request
waited (runs of up to 10 positions without update in the second half of each key interval);
after, no read is discarded. In this container every read lands within one frame, so the
starvation case is covered by the unit test `frames_of_older_requests_are_shown_while_scrubbing`
rather than by the drag.

Screenshots: [slider width live vs. past, before](slider-width-live-vs-past.png),
[web after](after-web-drag.png), [phone after](after-phone-drag.png),
[desktop after](after-desktop-drag.png).

## Code review

[review-code.md](review-code.md): no serious findings. Addressed: L1 (a write that fails during
a read now reports "this frame could not be stored"), L2 (checkpoints are capped while
decoding, not only afterwards), L3 (documented: checkpoints stay after going live, bounded),
L7 (tests for a failed write with checkpoints after it and for forced keyframes off the
cadence). Left as they are: L4 (the globe may briefly show an older frame under the greyed
label of the requested one while scrubbing), L5, L6 (the slider end moves by one character
once when the live time gains a digit; a drag continues, the id is stable).
