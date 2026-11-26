# #121: slowly dragging the frame slider backwards freezes the view — cause and plan

## What happens (measured, before the fix)

Instrumented build (`PERF` debug lines for seeks, history reads and shown frames), desktop and
web (chromium, SwiftShader) on a virtual screen, the slider dragged with xdotool from near its
right end to the left in small steps.

**Cause 1 — the slider changes its width when the view leaves "live".** In the one-row layout
(wide screens) the slider takes the room left of the time label, and the label grows from
`210 Myr` (live) to `57 Myr · live 245` (a past frame): the slider gets ~75–90 px shorter
([screenshot](slider-width-live-vs-past.png), 1280 px window: its right end moves from x≈1117 to
x≈1045). A drag that starts at the live end (where the handle is) and moves slowly left is, for
its first ~90 px, to the right of the short slider's end:

- frame A, live layout (long slider): the pointer means frame *p* → seek *p*, the view leaves live;
- frame B, past layout (short slider): the same pointer is past the end → seek to the last
  position → `go_live`;
- and so on, every frame.

Log of a 6-subdivision web run (`seek` per egui frame): `118, 122, 118, 122, 116, 122, 115, 122,
114, …` with 122 the live position. In SwiftShader (slow frames) every read lands in one frame,
so the view flickers between live and the past frame; on a fast machine the read of the past
frame spans several frames, it is dropped because the view went live in between or a newer
request is pending, and the view stays on the live state: it "does not update". Only backwards,
because only a backward drag starts at the live end; a fast drag leaves the ~90 px zone at once.
The mapping is also off by the width difference for the rest of the drag (x=1100 meant frame 227
in the live layout, ~246 in the past layout).

**Cause 2 — slow reads are never shown while the requests keep coming (web), worst backwards
(Sebastian's hypothesis).** `History` shows only the latest request: when a read finishes while a
newer one waits, the decoded frame only serves as the base of the next read and is not shown.
Deltas go forward only from a keyframe every 25 frames, so a backward step re-reads and re-decodes
the chain from the keyframe (up to 25 frames), while a forward step decodes one frame on the last
one. Measured on the web, 6 subdivisions (container, SwiftShader, contended core): IndexedDB read
~100 ms (≈ one egui frame there; it returns at the next frame), decode ~7–10 ms per frame, up to
~200 ms for a long chain; in the drag above 35 of 93 reads finished with a newer request waiting
and were not shown, in runs of up to 10 positions (1–2 s without update), always in the second
half of a key interval (long chains). Forward reads are one frame and keep up. Desktop: reads are
synchronous, every request is shown (subdivision 5: 0.45 ms decode per delta frame, ≤ 12 ms per
backward step; subdivision 6 ~4× that), so the desktop only suffers from cause 1.

## Plan

1. **Stable slider width (cause 1)** — `timeline.rs`, minimal: the one-row layout reserves the
   width of the *longest* time text (the past format with the live time, `"{live} Myr · live
   {live}"`, monospace) for the time label, right-aligned, whether live or not; the choice between
   one row and two rows uses that width too (a two-row choice puts the slider on its own,
   full-width row, so its width no longer depends on the text). Two vs. three rows (#161) still
   uses the shown text: that only moves the slider row vertically, not its horizontal mapping.
   Unit test: the one/two-row decision and the slider width do not change between live and past.
2. **Show what was decoded (cause 2)** — `History::finish_read` hands every successfully decoded
   frame to the view, also when a newer request is pending; the view shows it if it is still
   viewing the past (not only if it is exactly the wanted index). The time label stays at the
   wanted position, greyed while loading, as now; the shown time (`shown_time`) is the decoded
   frame's. So a drag updates the view at least at the rate reads complete, in both directions.
   The existing test `only_the_latest_request_is_shown` changes accordingly.
3. **Cheap backward steps** — `History` keeps decoded *checkpoints*: the state at every 5th frame
   of a decoded chain (counted from its keyframe) and the last decoded state, up to a byte budget
   (32 MB: ~14 states at 6 subdivisions, ~56 at 5; evict the one farthest from the latest
   request). A request decodes from the nearest checkpoint at or before it within its key
   interval: ≤ 4 delta frames read and decoded instead of ≤ 24, forward and backward.
   Format unchanged (no codec version bump).

Not chosen: denser keyframes (bigger history, still forward-only), reverse deltas (codec change,
double encoding cost in the simulation thread), a full LRU of every decoded frame (memory: 25 ×
2.25 MB per key interval at 6 subdivisions).

## Tests

- Unit tests: timeline layout (1); history: a frame decoded while a newer request waits is
  returned (2); backward requests read at most 5 frames once a chain was decoded, and every frame
  still decodes identically from any other (3).
- vscreen: slow backward drag from the live end, desktop and web (1280×800, 390×844 touch), the
  `PERF` log (seeks, reads, shown) before/after into `notes/issue-121/results.md`.
