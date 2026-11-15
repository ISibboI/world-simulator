# Project summary

_Last updated: 2026-11-15 (session 1)_

## Goal

Simulate a planet. The first component is a tectonic plate simulation on a sphere; later
components (climate, erosion, …) are expected to build on it.

## Current state

- Rust 2024 binary crate, toolchain pinned to 1.100.0.
- GitHub Actions CI modelled on the user's other Rust repositories (evalexpr, fedi-post-rs,
  sebschmi/* repos). Actions are pinned to **major version tags** (`actions/checkout@v7`,
  `actions-rust-lang/setup-rust-toolchain@v2`, `actions-rust-lang/rustfmt@v1`), not SHAs.
  Warnings are denied via `CARGO_BUILD_WARNINGS=deny` (set by setup-rust-toolchain v2), not
  `RUSTFLAGS`.
- Working rigid-plate tectonics simulation coupled with an Earth-like water cycle, fluvial
  erosion and a seasonal temperature model (Earth-like orbit). Writes equirectangular elevation
  (with rivers), plate, precipitation and temperature (annual, January, July) maps as PNG
  snapshots. Default run (40,962 cells, 12 plates, 500 Myr) takes about 70 s.
- **3D view** (default; `--headless` for batch runs): eframe/egui window, planet drawn with
  wgpu via an egui paint callback, camera rotates by click-and-drag. Simulation runs on a
  background thread and sends finished vertex buffers through a channel; window title shows the
  simulated time. PNG snapshots are still written in both modes.
- Temperature outcome over 500 Myr: global mean 10–13 °C, equator 23–25 °C, poles −11…−17 °C,
  permanent ice on 6–14% of the surface; continental interiors swing ~30 K over the year.
- Calibrated outcome after 500 Myr: peaks ~7 km, mean land elevation ~750–1150 m (Earth 840),
  land precipitation ~0.9 m/yr, sediment to the oceans ~15 km³/yr (Earth ~8–10), largest river
  ~50,000–80,000 m³/s.

## Architecture

- `src/sphere.rs`: icosphere mesh (`10·4^n+2` vertices, 5–6 neighbours each), greedy
  nearest-vertex walk with warm start.
- `src/tectonics.rs`: the simulation.
  - Each plate is a rigid rotation (quaternion + angular velocity vector) and stores its crust
    (kind, age, elevation) in its **own local frame**, indexed by mesh vertex. No resampling
    error accumulates because plates are rigid.
  - Each substep: rotate plates → age/erode crust → for every world cell, find which plates have
    crust there → resolve overlaps (subduction) and gaps (new oceanic crust) → apply uplift →
    collision drag.
  - Subduction polarity is decided **per plate pair** at first contact (buoyancy: continental >
    young oceanic > old oceanic) and remembered; it flips when continental crust reaches the
    trench. Per-cell decisions made plates fragment.
  - Continental collisions pull the angular velocities of both plates together. Random plate
    reorganisations (mean every 150 Myr per plate) keep the system moving.
  - Single-cell holes inside a plate are resampling artifacts and get a copy of neighbouring
    crust; other gaps get new oceanic crust (ridges).
  - Uplift saturates towards 9 km; uplifted oceanic crust accretes into continental crust.
  - Crust samples carry a separate `sediment` layer on top of the bedrock `elevation`, so that
    oceanic thermal subsidence does not erase sediment shelves.
- `src/climate.rs`: water cycle. Three-cell prevailing winds (trades, westerlies, polar
  easterlies) carry moisture from the ocean (evaporation ∝ warmth) over land; per-cell rain-out
  fraction from latitude (ITCZ, polar front) and orographic lift → rain shadows, subtropical
  deserts. Upwind neighbour per cell precomputed; moisture transport iterated to steady state.
- `src/erosion.rs`: priority-flood drainage from the ocean (lakes spill), discharge accumulation,
  implicit stream-power incision (Braun & Willett; `K = 1.5e-7`, `m = 0.5`), isostatic rebound
  (surface drops only 18% of eroded bedrock), sediment fills lakes and builds shelves outward from
  river mouths to −120 m, then deltas up to sea level; any rest is lost to the deep ocean.
- `src/temperature.rs`: energy balance model `C dT/dt = Q(1−α) − (A + BT) + transport` with a
  seasonal cycle. Earth orbit (e = 0.0167, obliquity 23.44°, perihelion early January, Kepler
  solved) → daily-mean insolation per latitude and season (24 seasons/yr). Runs on a coarse
  icosphere (subdivision 4, 2562 cells), interpolated to the fine mesh, lapse rate 6.5 K/km.
  Heat transport is mostly **zonal** (implicit 1D diffusion over 18 equal-area latitude bands,
  D = 0.45) plus weak local 2D diffusion (0.05); full 2D diffusion tied land to the ocean and
  killed continental seasonality. Ice albedo uses the **previous year's annual mean** (< −10 °C,
  Budyko); seasonal snow albedo caused a snowball planet. 20 spin-up years, then 2 per Myr step.
  Month 0 = January (year starts at perihelion).
- Evaporation over the ocean follows sea-surface temperature (+5%/K).
- `src/planet.rs`: couples tectonics (substeps) with temperature, precipitation and erosion per
  time step.
- `src/render.rs`: equirectangular PNG rendering (elevation with rivers, plates, precipitation,
  temperature) and `surface_color` (also used by the 3D view; adds permanent ice < −10 °C).
- `src/viewer/`: 3D view. `mod.rs` (eframe app, orbit camera with yaw/pitch, vertex building
  with `--exaggeration` 40× relief, ocean at radius 1), `renderer.rs` (wgpu pipeline, buffers in
  egui `callback_resources`, depth buffer Depth32Float requested via `NativeOptions`, 4× MSAA),
  `planet.wgsl` (diffuse lighting from a camera-relative light). Projection via
  `glam::camera::rh::proj::directx` (wgpu NDC: Z in [0, 1], Y up).
- GUI stack decision: eframe (egui) + wgpu rather than Bevy: in-view UI with egui, runs on
  Windows/macOS/Linux(+web), far lighter than a game engine for a single-mesh viewer.
- `src/main.rs`: clap CLI (`--seed`, `--subdivisions`, `--plates`, `--duration`, `--dt`,
  `--no-erosion`, `--river-threshold`, …).
- Logging: `tracing` + `tracing-subscriber` (fmt, env-filter) to stderr; `RUST_LOG` controls the
  level (default `info`; `debug` shows per-step phase timings in a `step{time=…}` span). Chosen
  over `log`+`simplelog`/`env_logger` for spans, structured fields and later profiling layers.
  Use `tracing` macros, never `println!`, for console output.

## Known issues

- No plate rifting/splitting: the plate count only decreases over time (12 → ~9 in 500 Myr).
- Mean ocean floor age drifts upwards (~145 Myr after 500 Myr; Earth ≈ 60 Myr). Plates slow down
  through collision drag (~3 cm/yr on average).
- Continental area slowly decreases (30% → ~24% in 500 Myr) because continental collisions
  destroy the losing crust instead of thickening the winner.
- Continental shelves are too wide (sediment spreads over up to 256 cells per river mouth).
- **Scattered land masses** (diagnosed 2026-11-15, not yet fixed), ranked:
  1. Island-arc accretion converts single oceanic samples above −200 m to continental crust →
     speckled continental crust (41.6% of continental crust is accreted after 500 Myr; disabling
     accretion: 146 → 15 continental components at 200 Myr). Fix: accrete coherent terranes only /
     majority filter.
  2. Delta fill raises shelves to exactly 0 m: counted as land but drawn light blue (18–20% of
     "land"). Fix: fill to below sea level, consistent sea-level convention, fewer shelf cells.
  3. No isostatic support of continents after erosion was added: 10–14% of continental crust
     sinks below sea level. Fix: slow isostatic relaxation of continental bedrock.
  4. Continental collisions delete the loser's continental crust (~42% of initial continental
     area destroyed in 200 Myr), backfilled by speckled accretion. Fix: conserve crust.
  5. Initial continents ignore plate boundaries (5 continents → 27 per-plate pieces at t=0) and
     plate regions fragment by resampling noise (12 → 59 components). Fix: grow continents within
     plates, majority filter on ownership.
- Climate has no ocean currents, glaciers don't erode, precipitation does not fall as snow; the
  precipitation map shows slight striping from the discrete upwind neighbour.
- Temperature is computed from the climatological seasonal cycle per Myr step (no Milankovitch
  cycles, constant solar luminosity).
- No hillslope diffusion (sub-grid at ~120 km cells); sediment deposited on oceanic crust is lost
  at subduction.
- Single-threaded; ownership resolution (~35 ms per substep) dominates.
- 3D view under KDE Wayland only draws its first frame when launched from Claude's sandboxed
  shell (compositor sends no redraw events). The user confirmed it works normally when launched
  from their own terminal. For screenshots from the sandbox, use X11:
  `env -u WAYLAND_DISPLAY`.
- CI not yet run with the GUI dependencies (eframe/wgpu); Linux build should not need system
  packages since wayland-sys uses dlopen, but this is unverified.

## Next steps (ideas)

- Plate rifting, e.g. splitting large or continental-heavy plates along a random great circle.
- Slab pull as the driving force instead of random reorganisations.
- Conserve continental crust in collisions (thickening / terrane transfer to the winner).
- Parallelise ownership resolution (rayon).
- Better initial conditions (noise-based continents, interior relief).
- Glacial erosion and snow; ocean currents for coastal climate.

## User preferences

- Keep session notes in `notes/`, checked into git (see `CLAUDE.md`).
- CI: follow the user's existing "Tests and Lints" workflow template; pin actions to the latest
  major version tag.
