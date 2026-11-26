# Review of notes/issue-121/plan.md (#121, backward slider drag does not update)

Read-only review against the code in ~/worktrees/issue-121 (timeline.rs, history/mod.rs,
web/frames.rs, viewer/mod.rs, egui 0.37 sources) and the two web logs.

## Verdict

The diagnosis is right, and the logs support it. Cause 1 (the slider's width changes with the
label) shows up as seeks that swing between the live end and a past frame: 118/122 at 6
subdivisions and 227/248 at 5. At 5 subdivisions this is the only cause; that log has no
`shown=false`. Cause 2 (on the web, only the latest request is shown) shows up as runs of
`shown=false` in the second half of each key interval: 35 of 92 reads at 6 subdivisions. But the
diagnosis is **incomplete for phones**, and fix 1 as written leaves a drag-killing bug there.
Fixes 2 and 3 are sound, with a few details to pin down. Ranked findings follow.

## 1. (High) On narrow/touch screens, the Two/Three row flip kills the drag. The plan says it is harmless.

The plan says the Two vs Three choice "only moves the slider row vertically". That is not true:
the slider's **egui Id changes**. The slider uses an auto Id, and in `Rows::Three` a `time_label`
is allocated in the same parent `Ui` before the slider (`ui.label` → `allocate_exact_size`
advances `next_auto_id_salt`). In `Rows::Two` the label sits inside the preceding `horizontal`.
So the slider in Three has a different Id than in Two.

egui drags are tied to the Id that was pressed (`interact_pointer_pos` is only set for that Id).
When the Id changes, the new slider never receives the drag until the pointer is released.

This is exactly the phone case. The test `time_text_moves_below_the_buttons_when_it_does_not_fit`
shows that a 390–412 px touch screen is `Two` with "275 Myr" (live) and `Three` with
"275 Myr · live 317" (past). A backward drag from the live end therefore does one seek, the
layout flips to Three, and the slider stops responding: "does not update". This happens only
backwards, because only a backward drag starts live. Forward drags start in the past layout and
are stable.

Fix (small; both parts are cheap):
- Give the slider a stable Id independent of its position in the tree. For example, wrap it in
  `ui.scope_builder(egui::UiBuilder::new().scope_id(egui::Id::new("time slider")), …)`; egui
  0.37 has `UiBuilder::scope_id` for exactly this. This also guards One/Two and any future layout
  change.
- Moving to Three does not move the slider vertically: the Area is anchored `LEFT_BOTTOM`, so
  the bar grows upward and the slider stays under the finger. Its horizontal mapping is
  unchanged (`inner` width in both). So with a stable Id, Two/Three can keep using the shown
  text. That preserves the #161 compact live layout on phones.
- Add the 390×844 touch backward drag to the vscreen check (the plan lists it; make sure it
  starts from the live end).

## 2. (Medium) Fix 1 (reserve the longest text in the One-row layout) is correct for desktop. Details:

- One vs Two: deciding with the reserved width is right. At about 600–750 px non-narrow windows,
  live currently fits One and past does not, so the bar flips One↔Two at drag start. That flip
  changes both the Id and the width. With a stable Id (finding 1) the flip would still change the
  width, so the reserved-width decision is needed too.
- Reservation: make sure the label really occupies exactly the reserved width, right-aligned. For
  example, `allocate_ui_with_layout(vec2(w, h), Layout::right_to_left(Center), …)` or
  `allocate_exact_size` + `put`. Plain `add_sized` centres the text.
  `"{live:.0} Myr · live {live:.0}"` is a valid upper bound, because the shown time ≤ live and
  the font is monospace.
- The live time grows while running: the reservation grows by one character at each new digit
  (99→100 Myr), which is a rare one-frame shift of a few px. It is acceptable. To remove it,
  reserve for `max(live, run duration)`; `self.duration` is available in `time_bar`.
- A related drift that the plan does not cause and need not fix: while the simulation runs,
  `positions` grows, so the same pointer x maps to a slowly increasing index during a drag. This
  is harmless, but mention it if results look slightly off.
- The collapsed strip has no slider, so it is unaffected.
- Unit test: compare the `rows()` decision and the slider rect/Id between live and past, for
  example by running `ctx.run_ui` twice and reading the slider response. A test of only `rows()`
  would not catch finding 1.

## 3. (Medium) Fix 2 (hand out every decoded frame) is necessary and correct on the web. Pin down:

- It is the essential web fix. With a seek every egui frame and a read latency ≥ 1 frame
  (`take_read` is only polled once per frame, and IndexedDB reads are about 30–240 ms here),
  "latest only" never shows anything during a continuous drag. Fix 3 does not change read
  latency. read_ms is about the same for 1 and 20 frames, so the read cost is fixed per request.
- Stale `ready` after `go_live`: `go_live` does not touch `History`. A read in flight or a filled
  `ready` survives. Once `poll_history` accepts any frame while `viewing.is_some()`, re-entering
  the past (a click far away) can briefly show a frame from before going live. Add
  `History::cancel()` (drop `wanted` and `ready`, let the read finish only as a base) and call it
  from `go_live`. Alternatively, tag requests with a generation number.
- Duplicate shows: `request(p)` while `p` itself is being read sets `wanted = p`. With fix 2 the
  finished read is handed out, and then the `decoded` shortcut hands out `p` again, so `show()`
  runs twice (8–25 ms each on the web). Skip `wanted` when it equals the read in progress.
- Overwrite order is fine. In `finish_read`, the intermediate `ready` is set before
  `start_read`, which overwrites it with the wanted frame or with the wanted frame's error. A
  wanted frame's error is never hidden. Keep reporting errors only for the wanted request.
- Label vs view: showing an intermediate frame while the label shows the wanted position (greyed
  while `loading`) is acceptable. `show()` sets `shown_time` and the picking cells from the
  frame, so the sun and the cell info stay consistent with what is drawn.
- Desktop: no change in behaviour, because reads are synchronous.
- Rename and replace the `only_the_latest_request_is_shown` test, and add "a read finishing
  after go_live is not shown".

## 4. (Low–medium) Fix 3 (decoded checkpoints) is correct in principle. It is optional and can be simpler.

- Value: it cuts backward-step decode from ≤ 24 deltas to ≤ 4. That is about 100–200 ms → about
  30 ms on the web container, and about 50 ms → about 10 ms on desktop at 6 subdivisions, where
  reads block the UI thread. It is worth having; the owner's hypothesis points at it. But fixes
  1+2 already resolve the reported symptom. If the PR grows, fix 3 could be a follow-up.
- Simpler bound: keep checkpoints only for the **key interval of the latest read**, at every 5th
  frame from its actual key plus the last decoded state. That is ≤ 6 states (about 13 MB at 6
  subdivisions). It needs no eviction policy, and the cache is cleared when a read uses a
  different key. Keep a byte cap only as a guard for larger meshes (about 9 MB/state at 7
  subdivisions), and always keep at least the last decoded state. wasm memory never shrinks, so
  avoid making 32 MB a floor.
- Correctness points:
  - Find the key as now (scan `entry.key`, not `index / 25`), so that forced keyframes and frame
    0 work.
  - Accept a checkpoint `c` only if `key <= c <= target`. A checkpoint from an earlier interval
    is then never used across a forced key.
  - Store the chosen base index in `reading` (`(base, from, target, started)`), and in
    `finish_read` use that checkpoint. Today the code relies on
    `decoded.take()` with `index + 1 == from`.
  - `codec::decode` takes `Option<&State>`, so no clone is needed. The states in the loop are
    retained instead of dropped.
  - Lost frames: `start_read` already rejects a lost target before any cache use. `lost` is a
    suffix, so every checkpoint ≤ a non-lost target is valid. Still, purge checkpoints
    ≥ `failed` in `check_writes` for clarity.
- Codec unchanged: agreed.

## 5. Other things checked (no problem found)

- egui Slider maps the absolute pointer x to a value on its current rect every frame
  (`value_at_pointer`), which confirms the mechanism of cause 1. Arrow keys are consumed by
  `timeline::keys` before the bar is drawn, so slider keyboard steps do not interfere.
- `view()`: a pointer past the end of the short slider maps to the last position, so
  `index + 1 >= positions()` triggers `go_live`, and each `time_action` requests a repaint. On
  desktop this is a 60 fps live/past flip loop that rebuilds the surface every frame while the
  pointer sits in the dead zone. It confirms the plan's desktop explanation.
- IndexedDB store: one read at a time, `getAll` on a bound key range, the `wake` closure
  requests a repaint when a read completes, and the read before the database opens is queued.
  Read semantics are not a factor.
- `poll_history`'s exact-index check is the view-side half of cause 2, and the plan addresses it.
- Remove the `PERF` instrumentation before the PR.

## Suggested minimal set

Stable slider Id (`UiBuilder::scope_id`), the One-row reserved label width with the One/Two
decision using it, fix 2 with a `cancel` on `go_live`, and fix 3 with the per-interval checkpoint
bound (or deferred).
