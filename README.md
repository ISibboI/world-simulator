# World Simulator

A simulation of an Earth-like planet: plate tectonics, climate, water cycle and erosion on an
icosphere, with a 3D viewer (eframe/egui + wgpu). It runs as a desktop program
(`cargo run --release -- --help`; `--headless` for batch runs with PNG snapshots) and in the
browser: **https://isibboi.github.io/world-simulator/** (parameters as URL query parameters,
e.g. `?seed=3&subdivisions=5`). Without a seed, the desktop 3D view and the web page start a
new planet with a random seed; the parameter panel shows it, and the web page puts it into its
URL, so a reload or a shared link makes the same planet. `--headless` runs use seed 0 unless
given one.

By default a run starts with a **barren planet at the earliest start** (#190): `--epoch 0` and
`--biosphere abiotic`, i.e. 4.0 Ga, a sun at 74% of today's brightness and a planet without life
(bare land, no transpiration, braided rivers, no biogenic pelagic sediment). The greenhouse
forcing offsets the faint sun at the start; then a **carbon cycle** (#154) changes the CO₂ in
every step: tectonic degassing (ridges, arcs, plumes, metamorphism) against silicate weathering on
land and seafloor weathering, the carbonate–silicate feedback that counters the brightening sun.
Each step logs the CO₂, the forcing and the fluxes (`carbon:` lines); `--no-carbon-cycle` keeps
the forcing of the start for the whole run, and `--set carbon.NAME=VALUE` changes its parameters
(`src/carbon.rs`, `notes/issue-154/`). Bare rock weathers into a thin **regolith** (#197) that
the rivers carry away: erosion is weathering-limited where the rock weathers slowly (cold or dry
bare highlands) and transport-limited where the regolith is thick, and the land weathering of the
carbon cycle follows the regolith (`--set erosion.regolith=0` turns it off; `src/erosion/regolith.rs`,
`notes/issue-197/`). Today's living Earth, the default before #190, is
`--biosphere modern --epoch 4` (URL `?biosphere=modern&epoch=4`).

`--epoch` (URL `epoch=`) sets the time of the planet's history at the start of a run, in Gyr
counted forwards: `0` is the earliest start (4.0 Ga, once the planet has cooled enough for plates
and oceans; the default), `0.5` the Archean (3.5 Ga), `4` today and up to `5.90` the future,
when the sun is 1.2 times as bright as today: beyond, an ocean planet enters a runaway greenhouse,
which the model does not simulate, so later epochs are refused and a run that would pass it is
shortened to end there (#212). It sets the brightness of the sun and the default
greenhouse forcing. Until #150 it counted backwards (Gyr before present); old values `x` are now
`4 − x`. `--biosphere` (URL `biosphere=`) is `abiotic` (the default) or `modern`.

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
on Linux (Chrome, Firefox, WebKit), Windows (Chrome, Firefox) and in an Android emulator set up
like a low-end phone (Chrome, Firefox). It runs only on request: by hand (Actions → Browser
tests → Run workflow), or on a pull request with the label `browser-tests`. The runners have no
GPU, so everything renders in software. Safari (macOS, iOS) is not tested in CI: WebKit on
GitHub's macOS runners has no working WebGL, so the 3D view cannot run there at all.
Safari's engine, WebKit, is covered by the Linux WebKit test. Locally, e.g.:

```sh
trunk build --release
pip install selenium pillow
python web/tests/browser_test.py --browser firefox --dist dist
```
