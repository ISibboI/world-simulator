# Issue #122: results

Plan: [plan.md](plan.md).

## Reproduction (before the fix)

Build A = `main` at da7f5fd (fixed file names). `web/tests/browser_test.py --scenario redeploy`
serves it with `Cache-Control: max-age=600` like GitHub Pages; after the page ran, it replaces it
with the "next build" (the same build with one more line in `world-simulator.js` and the new
integrity hash in `index.html`) and reloads the page. Chromium (Playwright 159): the reloaded page
got the new `index.html` (revalidated) but the old `world-simulator.js` from the cache, the
integrity check blocked it, and the page stayed at "Loading the simulator…" until the 90 s
timeout, without any visible error, as reported.

## After the fix (local, tktie)

Release build with `filehash = true`, Chromium 159 and Firefox (Playwright), on a virtual screen:

| Scenario | Chromium | Firefox | Banner / outcome |
| --- | --- | --- | --- |
| load | passed | passed | normal start |
| redeploy | passed | passed | reloaded page runs as the new build (new hashed module fetched) |
| broken-js | passed | passed | "Could not load the simulator. Please reload the page." after one automatic reload |
| broken-wasm | passed | passed | same text (instantiation fails: wasm of "another build") |
| broken-worker | passed | passed | view banner "Simulation failed: its worker did not start. Reload the page." |

WebKit could not be launched locally (missing system libraries in the container). On GitHub, the
browser-test workflow ([run 45248454876](https://github.com/ISibboI/world-simulator/actions/runs/45248454876))
passed: every scenario on Linux in Chrome, Firefox and WebKit, the normal load on Windows (Chrome,
Firefox) and Android (Chrome, Firefox).

Seen along the way:
- Chrome runs a module that is imported while its `modulepreload` failed the integrity check
  without checking it again (race). A stale module of another build could therefore run with the
  page's wasm under the old fixed names; with hashed names that combination cannot happen any
  more. The `broken-js` scenario uses a module that fails when it runs, to be deterministic.
- Firefox runs out of memory on a *truncated* wasm file and reports it only in the console (no
  event the page could catch); then only the "Still loading the simulator after 30 s… If nothing
  happens, reload the page." hint appears (seen locally).
- Chrome ignores the `integrity` attribute of the wasm preload.

## Independent review

A review agent checked the diff; fixed: guard for browsers without WebAssembly (banner says so at
once), one automatic reload per failure streak (cleared on the first frame) instead of one per
minute, a 5 s limit on the cache refresh before the reload, worker errors with a message keep
their message, worker.js names a missing `module` parameter as a build mismatch, tests demand
"Please reload" and exactly one automatic reload (counted as navigations on the server), and the
`broken-wasm` scenario removes the wasm preload's integrity so that instantiation is what fails.

## Caveats

- **One-time transition:** for up to 10 minutes after the first deploy with hashed names, a
  browser that still has the *old* `index.html` cached (it has no failure reporting) but no
  longer its old files can still hang; a reload fixes it. This cannot be fixed from the new page.
- Not covered by a test: a cached old page reached by normal navigation (not a reload) after a
  deploy, whose files are gone. Its 404 takes the same path as `broken-js` (banner + one reload
  with the page fetched anew), which is tested.
- A deploy that changes only `worker.js` keeps its URL; browsers may use the previous version
  for up to 10 minutes, so `worker.js` must stay compatible with the page's query and messages.
