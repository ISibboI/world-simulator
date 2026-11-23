# Issue #117: supported browsers and basic browser tests — plan

Agreed with Sebastian on #117 (2026-11-23): supported platforms are desktop Firefox, Chrome,
Safari; Android Chrome, Firefox; iOS Safari (iOS Chrome/Firefox use WebKit). Opera is not tested.
Per platform, three checks: the simulator loads (no error banner, the worker runs), the 3D view
draws a frame, the frame history (#115) is stored in IndexedDB and read back. The tests run only
by hand (`workflow_dispatch`) or on a PR with the label `browser-tests`. No device farm.

## Design

One test program for every platform, `web/tests/browser_test.py`:

- It serves the built site (`dist/`) on `localhost` and injects `web/tests/probe.js` as the
  first script of `index.html` (only in the test server; the site is unchanged). The probe
  wraps the console, collects uncaught errors and posts the page state to the server twice a
  second. The verdict is made on the server, so every way of opening the page works the same:
  WebDriver (Selenium) for the installed desktop browsers, Playwright for WebKit on Linux,
  an `adb` VIEW intent for Android (with `adb reverse`, so the page is on `localhost`, a secure
  context: Web Locks need one), `xcrun simctl openurl` for the iOS Simulator. A run token in
  the URL fragment keeps reports of restored old tabs out.
- The page state: the page sets `data-first-frame`, `data-surface-updates`,
  `data-simulation-failed` and `data-frame-history` on its root element (`mark` in
  `src/web.rs`). Harmless in production, no other test hook.
- Pass: banner gone + first frame, ≥ 2 states from the worker (≥ 1 for the run with the
  device's default parameters), `data-frame-history=indexeddb`, no failure, no uncaught error,
  and a screenshot (driver, `adb screencap`, `simctl io screenshot`) with ≥ 5% ocean-blue
  pixels in the middle (the planet). The page's WebGL canvas cannot be read back from the
  page (no `preserveDrawingBuffer`), hence the screenshot.
- The read-back of the frame history is a check the page does itself once per run: after
  frame 0 is written, it reads it back from IndexedDB and compares it. That is also the
  robustness fix below: a storage that does not work is detected and replaced by memory.
- Quick parameters: `?subdivisions=3&duration=50` (642 cells, a few seconds). On Android and
  iOS also a run without parameters (the device's default subdivisions), as a phone user gets.

Matrix (`.github/workflows/browser-tests.yml`): Linux Chrome/Firefox (Selenium, under
`xvfb-run`: headless Firefox has no WebGL) and WebKit (Playwright); macOS Safari
(`safaridriver`), Chrome, Firefox; Windows Chrome, Firefox; Android emulator API 34
`google_apis` x86_64 with 2 GB RAM (low-end phone), Chrome (as debug app with a command-line
file that skips the first-run screens) and Firefox (Fenix release APK from Mozilla's archive);
iOS Simulator (newest iPhone runtime on `macos-latest`), Safari.

## Phone hang ("Loading the simulator…" forever since #115)

The loading banner disappears only at the end of the view's first frame. Anything that stops
the page before that (the module failing to load or start, a panic) left it on the screen with
no message. Changes:

1. `web/index.html`: uncaught errors and rejections before the first frame are shown in the
   banner ("The simulator could not start: …") instead of hanging silently.
2. Frame history (`src/web/frames.rs`): IndexedDB must open within 10 s and frame 0 must read
   back as written within 10 s, else the history is kept in memory (with the frames written
   so far). Storage never blocks loading (it never did block the first frame; this makes it
   robust when it hangs or misbehaves, as WebKit's IndexedDB has done in some versions).
3. Reproduce on the Android emulator, the iOS Simulator and WebKit; fix the cause if found.
