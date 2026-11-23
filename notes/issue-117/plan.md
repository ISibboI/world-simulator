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

Most likely #122 (found by the orchestrator, a separate issue, not worked on here): after a
Pages deploy the browser mixes a cached `world-simulator.js` of the previous build with the new
`index.html`; the SRI check blocks it and the banner stays. The tests serve a local build with
`Cache-Control: no-store`, so they are not affected by it. Here: look for platform-specific
causes on the Android emulator, the iOS Simulator and WebKit, and make the frame-history
storage robust: IndexedDB must open within 10 s and frame 0 must read back as written within
10 s, else the history is kept in memory (with the frames written so far). Storage never blocks
loading.
