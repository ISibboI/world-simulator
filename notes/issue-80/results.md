# #80: what was tested

## Automated

- Unit tests of the banner state (`banner::tests`): "Starting…" until the first update,
  "Restarting…" after a restart until the first update of the new run, failures shown before and
  after the first update. No egui UI smoke tests (decided against on #61).
- CI checks locally with `CARGO_BUILD_WARNINGS=deny`: fmt, clippy (native and wasm32), all tests
  (all passed), doc.
- Headless output byte-identical to main (`--headless --subdivisions 4 --duration 20`: all 12
  output files identical, logs identical apart from timings).

## Visual (virtual screen and headless Chromium, software rendering)

Start-up phases (round 2, after Sebastian asked for the banner from the first paint):

| case | screenshot |
| --- | --- |
| desktop (vscreen): window open, generating the initial planet | [desktop-generating.png](desktop-generating.png) |
| desktop (vscreen): restart, generating the new planet | [desktop-restart.png](desktop-restart.png) |
| web (vscreen Chromium 1280×800): loading the module (HTML banner) | [web-loading.png](web-loading.png) |
| web (vscreen): generating the initial planet (egui banner, same place) | [web-generating.png](web-generating.png) |
| web (vscreen, Chromium without WebGL): error before the view exists | [web-no-webgl-error.png](web-no-webgl-error.png) |
| web (headless 1200×800, CPU ×2 slower): worker starting | [web-starting.png](web-starting.png) |
| web (headless): restarting | [web-restart.png](web-restart.png) |
| phone 390×844 @3× (network 1 MB/s, CPU ×2 slower): loading, worker starting, generating | [1](phone-1-loading.png), [2](phone-2-starting.png), [3](phone-3-generating.png) |
| phone: restart from the panel (panel closes, banner visible) | [phone-restart.png](phone-restart.png) |
| phone: the worker fails (`?set=no_such_parameter…`): long text wraps | [phone-failed.png](phone-failed.png) |
| phone: error before the view exists (`?subdivisions=9`): HTML banner | [phone-page-error.png](phone-page-error.png) |

Timings seen with throttling (phone): "Loading the simulator…" ~8 s (8 MB module at 1 MB/s), then
"Starting the simulation…" ~7 s (the worker loads and compiles the module again), then
"Generating the initial planet…" until the first update. Without throttling on desktop-size
Chromium "Starting" lasts < 1 s and "Generating" ~3 s; on the desktop app (6 subdivisions)
"Generating" ~5 s. The banner disappeared with the first update in every case and stays below
the parameter panel, its ⚙ button and the cell widget.

## Not tested

- Real devices (phones, Windows/macOS, a real GPU); only software rendering on Linux X11 and
  headless Chromium's device emulation.
- A native start failure on desktop (same code path as the web failure: `SimulationEvent::Failed`).
- The 4-row limit with a very long message (only a 4-row message was shown).
- The hand-over frame itself (that no frame without a banner is shown) is by construction
  (same task as the first canvas frame), not observed frame by frame.
- The "Restarting: generating a new planet…" text on the web (seen on the desktop; on the web
  the phase was shorter than the screenshot interval).

## Review

One independent reviewer read the diff against the egui 0.37 sources: no correctness bugs.
Fixed: the panel hid the restart banner on phones (now closes on Restart on narrow screens);
very tall error text is now limited to 4 rows; a stale comment. Kept: a failure stays in the
banner until the next restart (it is not dismissable); safe-area insets are not handled (no
effect today, the page does not use `viewport-fit=cover`).
