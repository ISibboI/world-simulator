# Issue #122: load failures in the loading banner, cache-safe deploys

## Cause (from the issue, confirmed locally)

The site kept fixed file names (`world-simulator.js`, `world-simulator_bg.wasm`, `worker.js`), and
GitHub Pages serves every file with `Cache-Control: max-age=600`. On a reload after a deploy,
browsers revalidate the page itself but take its scripts from the cache while they are fresh, so
the new `index.html` met the old `world-simulator.js`; its integrity hash did not match, the
import failed, and nothing reported it: the page stayed at "Loading the simulator…".

## Plan

1. **Cache-safe names.** Trunk's `filehash = true`: the JavaScript module and the wasm file get
   content hashes in their names, so a page only ever names the files of its own build.
   `web/worker.js` cannot be hashed (Trunk copies it), so the page tells the worker which files
   to load: the worker URL is `worker.js?module=<hashed js>&wasm=<hashed wasm>&<options>`. The
   query string makes the worker URL unique per build too, so a cached `worker.js` of another
   build is never used. `src/web.rs` reads the two names from Trunk's preload links in the page;
   `src/query.rs` builds the worker query (`worker_query`) and strips the file keys before the
   options are parsed in the worker (`without_worker_files`). `worker.js` imports the module
   dynamically and accepts only same-origin URLs.
2. **Failures in the banner.** An inline script at the top of `web/index.html` (before Trunk's
   loader) listens, until the view's first frame (`data-first-frame`), for
   - `error` events of `<script>`/`<link>` elements (capturing; they do not bubble): network
     errors, 404, integrity mismatches → "Could not load the simulator (file)."
   - uncaught errors and unhandled rejections (Trunk's loader awaits `init` at the top level):
     compile/link errors of the wasm → "Could not load the simulator.", traps (panics) and
     errors after the module started → "The simulator could not start."
   - a 30 s timer → "Still loading the simulator after 30 s… If nothing happens, reload the page."

   The banner says what failed and "Please reload the page."; the details go to the console;
   `data-load-failed` records it for the tests. A failure before the module started reloads the
   page once (fetching the page and the failed file with `cache: "reload"` first, so that a page
   from the cache that names files a deploy removed is replaced); `sessionStorage` allows one
   automatic reload per minute, so a lasting failure cannot loop.
3. **Worker failures** are shown by the view's banner ("Simulation failed: …"): `worker.onerror`
   now takes a plain `Event` (a worker script that does not load gives no `ErrorEvent`; reading
   `ErrorEvent.message` from it would throw), and `worker.js` reports a failure to load the module
   or wasm as "the worker could not load the simulator. Reload the page."
4. **Browser tests** (`web/tests/browser_test.py --scenario`): `broken-js` (module with a wrong
   integrity hash that also fails when run), `broken-wasm` (a wasm module of "another build" whose
   import is missing), `broken-worker` (404 for `worker.js`): the banner must show the failure with
   a reload hint, and the page may reload itself at most once. `redeploy`: the site is served with
   `max-age=600`; after the page passed, a new build (the same build with a changed module,
   renamed if names are hashed) replaces it, the page reloads, and it must pass again as the new
   build. The workflow runs these on Linux in Chrome, Firefox and WebKit.
