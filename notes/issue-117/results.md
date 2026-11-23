# Issue #117: results

Plan: [plan.md](plan.md). Workflow: `.github/workflows/browser-tests.yml`, test program:
`web/tests/browser_test.py` + `web/tests/probe.js`.

## What the test checks

Per browser, against a locally served `trunk build --release` (`Cache-Control: no-store`, so
the stale-cache problem of #122 cannot interfere):

1. the loading banner is gone and the view drew its first frame (`data-first-frame`);
2. the simulation worker sent ≥ 2 states (`data-surface-updates`; ≥ 1 for the run with the
   device's default parameters);
3. the frame history wrote frame 0 to IndexedDB and read it back identical
   (`data-frame-history=indexeddb`);
4. no uncaught error, no simulation failure, for 30 s after passing (catches late crashes);
5. a screenshot shows the planet (≥ 5% ocean-blue pixels in the middle; typical 40–66%).

Page parameters: `?subdivisions=3&duration=50` (642 cells, a few seconds); on Android and iOS
also without parameters (the device's default). The page log (console, uncaught errors, state
changes, stalls of the page > 1 s) is in each job's `results-*` artifact.

## Results (CI runs on the branch, 2026-11-23; the last one: run 45230310335)

| Platform | Browser | Result | Time to pass |
| --- | --- | --- | --- |
| Linux (ubuntu-latest) | Chrome 157 | ✅ | 2–5 s |
| Linux | Firefox 160 | ✅ | 2.5–3 s |
| Linux | WebKit 27.2 (Playwright, Safari's engine) | ✅ | 1–2 s |
| Windows | Chrome 157 | ✅ | 3–5 s |
| Windows | Firefox 160 | ✅ | 3.5–4 s |
| macOS (macos-15-intel) | Chrome 151 | ✅ | 2.6–3.3 s |
| macOS | Firefox 154 | ✅ | 3.9–4.9 s |
| macOS | Safari 26.6 | ❌ environment: WebGL context lost at once | — |
| Android 15 emulator, 2 GB RAM | Chrome 124 | ✅ quick and default | 19–23 s |
| Android 15 emulator, 2 GB RAM | Firefox 160 (Fenix) | ✅ quick and default | 8–24 s |
| iOS 26.2 Simulator (iPhone SE 3) | Safari | ❌ environment: WebGL out of memory | — |

Safari and iOS fail on GitHub's macOS VMs (no GPU) **in the same way for the build before #115**
(a temporary baseline job built `7579a1f^1` and ran the same test: wgpu "Out of Memory" in the
iOS Simulator after a 15–30 s first frame, on arm64 and Intel runners, iPhone 16-class and
iPhone SE). Desktop Safari on the Intel runner loses its WebGL context on creation
(`glow` panics on `getSupportedExtensions() == null`). So these are limits of the CI machines,
not of the app; the workflow still runs them (reported, not failing). WebKit's IndexedDB and Web
Locks are covered by the Linux WebKit test, which passes.

## Findings

- **Phone hang**: no platform-specific cause found. All engines (Blink, Gecko, WebKit) load,
  store and read back the history; Android (low-memory emulator) passes with Chrome and
  Firefox, quick and default parameters. The hang Sebastian saw is most likely #122 (cached
  `world-simulator.js` of the previous build after a deploy, blocked by SRI).
- **Fixed: WebGL with a 4096 texture limit.** The web view asked for 8192-pixel textures
  (egui's default for depth buffers of 4K screens). The Android 14 emulator's Chrome 113
  (SwiftShader GLES, `MAX_TEXTURE_SIZE` 4096) refused: "failed to start the 3D view: Limit
  'max_texture_dimension_2d' value 8192 is better than allowed 4096" (an error in the banner,
  not a hang). Some phones' WebGL has the same limit. The view now asks for at most what the
  adapter has (`web_device_descriptor` in `src/viewer/mod.rs`).
- **Fixed: storage can no longer stall or silently lose the history.** IndexedDB must open,
  and frame 0 must read back identical, within 10 s of a responsive page (a timer that fires
  more than a second late, because the page was stalled, waits again, up to 6 times); else the
  history continues in memory with the frames written so far. Tried by hand with a sabotaged
  IndexedDB (Playwright init scripts: `open` never completes → memory after 10.2 s; `get`
  returns another key → memory at once); the view kept running in both. Storage never blocked
  the first frame (it did not before either).
- **Seen, not fixed (upstream/environment):** a WebGL context that is lost when the view starts
  makes `glow` panic before the first frame, which leaves "Loading the simulator…" on the
  screen; with #122's error reporting in the band this would show an error instead.
- Emulator details: the API 34 image has Chrome 113 and its SwiftShader canvas is black in
  screenshots with `-gpu swiftshader_indirect` and `guest`; API 35 with `-gpu swangle_indirect`
  works (Chrome 124). Firefox's onboarding, the default-browser prompt and Chrome's
  notification prompt are tapped away through `uiautomator` (they cover the page otherwise).
- GitHub's macOS arm64 runners were often not acquired at all ("capacity constraints"); the
  macOS jobs use `macos-15-intel`, and the three desktop browsers per OS run in one job.

## Cost per run

Wall time ~9 min (the jobs run in parallel after a ~1 min build): Linux ~3 min, Windows ~2 min,
macOS ~5 min, Android ~3.5 min per browser, iOS ~8.5 min. The last run on the branch
(45230310335) passed with all jobs green except iOS (known, not failing the workflow).
