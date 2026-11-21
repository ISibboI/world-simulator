# #80: what was tested

## Automated

- Unit tests of the banner state (`banner::tests`): "Starting…" until the first update,
  "Restarting…" after a restart until the first update of the new run, failures shown before and
  after the first update. No egui UI smoke tests (decided against on #61).
- CI checks locally with `CARGO_BUILD_WARNINGS=deny`: fmt, clippy (native and wasm32), all tests
  (235 passed), doc.
- Headless output byte-identical to main (`--headless --subdivisions 4 --duration 20`: all 12
  output files identical, logs identical apart from timings).

## Visual (virtual screen and headless Chromium, software rendering)

| case | screenshot |
| --- | --- |
| desktop, starting (window open, no update yet) | [desktop-start.png](desktop-start.png) |
| desktop, restarting (Restart clicked; old planet still shown) | [desktop-restart.png](desktop-restart.png) |
| web 1200×800, starting | [web-start.png](web-start.png) |
| web 1200×800, restarting | [web-restart.png](web-restart.png) |
| phone 390×844 @3×, touch, starting | [phone-start.png](phone-start.png) |
| phone, restart from the panel (panel closes, banner visible) | [phone-restart.png](phone-restart.png) |
| phone, the worker fails (`?set=no_such_parameter…`): long text wraps | [phone-failed.png](phone-failed.png) |
| phone, error before the view exists (`?subdivisions=9`): HTML `#status` in banner style | [phone-page-error.png](phone-page-error.png) |

The banner disappeared with the first update in every case (checked by a later screenshot), and
it stays below the parameter panel, its ⚙ button and the cell widget/button.

## Not tested

- Real devices (phones, Windows/macOS, a real GPU); only software rendering on Linux X11 and
  headless Chromium's device emulation.
- The HTML "Loading…" state in the new style (too short to capture locally; the same CSS is
  shown by the error case above).
- A native start failure on desktop (same code path as the web failure: `SimulationEvent::Failed`).
- The 4-row limit with a very long message (only a 4-row message was shown).
- The desktop start banner is visible only briefly: on desktop the initial planet is built before
  the window opens, so the window shows it only until the first frame of the simulation arrives.

## Review

One independent reviewer read the diff against the egui 0.37 sources: no correctness bugs.
Fixed: the panel hid the restart banner on phones (now closes on Restart on narrow screens);
very tall error text is now limited to 4 rows; a stale comment. Kept: a failure stays in the
banner until the next restart (it is not dismissable); safe-area insets are not handled (no
effect today, the page does not use `viewport-fit=cover`).
