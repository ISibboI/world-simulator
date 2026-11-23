# World Simulator

A simulation of an Earth-like planet: plate tectonics, climate, water cycle and erosion on an
icosphere, with a 3D viewer (eframe/egui + wgpu). It runs as a desktop program
(`cargo run --release -- --help`; `--headless` for batch runs with PNG snapshots) and in the
browser: **https://isibboi.github.io/world-simulator/** (parameters as URL query parameters,
e.g. `?seed=3&subdivisions=5`).

## Supported browsers

The web version supports these platforms and browsers (current versions):

| Platform | Browsers |
| --- | --- |
| Desktop (Linux, macOS, Windows) | Firefox, Chrome, Safari (macOS) |
| Android | Chrome, Firefox |
| iOS / iPadOS | Safari (Chrome and Firefox on iOS use Safari's engine, WebKit, and behave like it) |

Other browsers built on the same engines (Chromium, Gecko, WebKit) usually work as well but are
not tested; Opera is not tested separately. The web version needs WebGL 2 (or WebGPU), module
Web Workers and WebAssembly; the frame history of the time bar is kept in IndexedDB (in memory
if the browser's storage is not available).

### Browser tests

`web/tests/browser_test.py` checks in one browser that the simulator loads (no error banner, the
simulation worker runs), that the 3D view draws the planet, and that the frame history is
stored in IndexedDB and read back. The workflow `.github/workflows/browser-tests.yml` runs it
on Linux (Chrome, Firefox, WebKit), macOS (Safari, Chrome, Firefox), Windows (Chrome, Firefox),
in an Android emulator set up like a low-end phone (Chrome, Firefox) and in the iOS Simulator
(Safari). It runs only on request: by hand (Actions → Browser tests → Run workflow), or on a
pull request with the label `browser-tests`. Locally, e.g.:

```sh
trunk build --release
pip install selenium pillow
python web/tests/browser_test.py --browser firefox --dist dist
```
