# Project summary

_Last updated: 2026-11-16 (issue 1 tuning: slab resistance, initiation hazards)_

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
  snapshots. Default run (40,962 cells, 500 Myr, seed 0) takes about 58 s (was 414 s before
  incremental plate matching and the issue 1 tuning).
- **3D view** (default; `--headless` for batch runs): eframe/egui window, planet drawn with
  wgpu via an egui paint callback, camera rotates by click-and-drag. Simulation runs on a
  background thread and sends finished vertex buffers through a channel; window title shows the
  simulated time. PNG snapshots are still written in both modes.
- Temperature outcome over 500 Myr: global mean 10–13 °C, equator 23–25 °C, poles −11…−17 °C,
  permanent ice on 6–14% of the surface; continental interiors swing ~30 K over the year.
- Outcome over 500 Myr (thickness model, commit fb5ffb6): continental 30% → 20%, mean land
  0.7–1.4 km, land precipitation 0.46–0.66 m/yr, sediment to ocean 3–7 km³/yr, global
  temperature 13–16 °C, equator 22–26 °C, 3–7% permanently frozen.

## Architecture

- `src/sphere.rs`: icosphere mesh (`10·4^n+2` vertices, 5–6 neighbours each), greedy
  nearest-vertex walk with warm start.
- `src/tectonics/`: the plate simulation.
  - `mod.rs`: parameters, `Plate` (rigid rotation + angular velocity; crust samples in its own
    local frame, indexed by mesh vertex; lookup cache; `consumed` flags), `Simulation`, step loop
    (velocities → substeps → crustal flow → dead plates → events → mantle).
  - `crust.rs`: crust samples (thickness, density, formation time `birth`, sediment); elevation from Airy isostasy
    (`src/isostasy.rs`: mantle 3300, water 1030, sediment 2300, oceanic 2900, continental 2800,
    arc 2800 kg/m³) plus thermal uplift from the GDH1 depth–age curve (ridge −2600 m, old ocean
    −5651 m; 35 km continent ≈ +570 m). "Continental" = crustal buoyancy ≥ 3000 m.
  - `boundaries.rs`: ownership resolution per substep (most buoyant crust wins; subduction
    polarity per plate pair), deferred consumption of overridden samples (only once invisible),
    arcs (500 m·cell per subducted sample in a band behind the trench), subduction erosion
    (300 m·cell at the trench), continental collisions stack the loser's crust onto the winner
    (spilling outward above 75 km), crustal flow of crust > 40 km, gap filling (new sea floor
    only where plates diverge, healed copies for resampling gaps, shared samples at boundary
    slivers where crust was consumed).
  - `dynamics.rs`: boundary classification (trench, ridge, collision) and a quasi-static
    torque balance per plate (slab pull ∝ √age, ridge push, basal drag against the mantle flow,
    ×4 under continents, slab resistance at trenches capping slab-driven speed at 10 cm/yr,
    collision resistance with an active set), one dense linear system. Slabs grow with
    convergence, decay when it stops and break off below 20 km.
  - `mantle.rs`: mantle flow from slab downwellings (decaying memory) and plumes.
  - `topology.rs` / `events.rs`: split, merge, remove plates; rifting above plumes, subduction
    initiation as an age hazard per margin length (passive margins split the plate; existing
    boundaries, mostly polarity reversals where old ocean overrides younger ocean), suturing,
    capture of tiny plates. `--set NAME=VALUE` overrides tunable parameters.
  - `generation.rs` (initial conditions), `statistics.rs` (diagnostics incl. ocean-age
    percentiles, plate births/deaths, gap statistics, connected land masses).
- `src/climate.rs`: water cycle. Three-cell prevailing winds (trades, westerlies, polar
  easterlies) carry moisture from the ocean (evaporation ∝ warmth) over land; per-cell rain-out
  fraction from latitude (ITCZ, polar front) and orographic lift, reduced in descending air
  (foehn) → rain shadows, subtropical deserts. 50% of land precipitation is recycled
  (evapotranspiration). Upwind neighbour per cell precomputed; 150 transport iterations.
- `src/erosion.rs`: priority-flood drainage from the ocean (lakes spill), discharge accumulation,
  implicit stream-power incision (Braun & Willett; `K = 1.5e-7`, `m = 0.5`) with **sea level as
  base level**; eroding crust thins it (isostatic rebound via `isostasy`); sediment fills lakes
  and builds shelves outward from river mouths to −120 m, then delta plains to +2 m; sediment
  loads the crust (rises ~0.44 of its thickness under water); any rest is lost to the deep ocean.
- `src/temperature.rs`: energy balance model `C dT/dt = Q(1−α) − (A + BT) + transport` with a
  seasonal cycle. Earth orbit (e = 0.0167, obliquity 23.44°, perihelion early January, Kepler
  solved) → daily-mean insolation per latitude and season (24 seasons/yr). Runs on a coarse
  icosphere (subdivision 4, 2562 cells), interpolated to the fine mesh, lapse rate 6.5 K/km.
  Heat transport is mostly **zonal** (implicit 1D diffusion over 18 equal-area latitude bands,
  D = 0.45) plus weak local 2D diffusion (0.05); full 2D diffusion tied land to the ocean and
  killed continental seasonality. Ice albedo uses the **previous year's annual mean** (< −10 °C,
  Budyko); seasonal snow albedo caused a snowball planet. Land ice albedo is scaled by snowfall
  (full at ≥ 0.3 m/yr of last step's precipitation), so cold deserts don't glaciate. 20 spin-up years, then 2 per Myr step.
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

- Continental shelves are too wide (sediment spreads over up to 256 cells per river mouth).
- Land fragmentation largely fixed by the thickness model (at 200 Myr: land masses 105 → ~30,
  continental crust masses 146 → ~17–25); still 15–20 single-cell specks.
- **Continental area declines** 30% → ~20% in 500 Myr because collisions thicken crust (area
  shrinks at conserved volume) and nothing thins it again: no continental rifting/extension,
  no plate splitting. Continents end up too high mid-run (mean land 1.1–1.4 km vs Earth 0.84).
- Collision churn is high (≈ the whole initial continental volume passes through collisions in
  300 Myr); partly real collisions driven by random reorganisations, partly boundary noise.
- **Open physical-accuracy issues are tracked in `notes/ISSUES.md` (with order of work);
  plans in `notes/plans/`, research in `notes/research/`. Issues 1 (mechanisms + tuning), 5, 3,
  4 done and the runtime optimisations. **Next: issue 2 (sea level).** Issue 1 leaves 7–12%
  ancient (≥ 200 Myr) ocean in slow continent-bearing plates (needs issue 10).
- **Independent physics review (2026-11-15, see session log)** — top issues: ocean floor too old
  and plates only die (no slab pull / subduction initiation / rifting); sea level fixed so water
  volume isn't conserved; continental crust not conserved with erosion (several leaks); thermal
  uplift on young-aged continental crust inflates mountains; new crust created at convergent
  gaps; implicit erosion ignores isostasy; shelves are 10+ km sediment piles on oceanic crust;
  mid-latitude precipitation too low; poles too warm.
- Subduction model gaps: no trenches, slabs/slab pull, back-arc extension, accretionary
  wedges, obduction, subduction initiation; polarity is per plate pair, not per boundary
  segment; transform boundaries are not distinguished (overlap noise is treated as convergence).
- Climate has no ocean currents, glaciers don't erode, precipitation does not fall as snow; the
  precipitation map shows slight striping from the discrete upwind neighbour.
- Temperature is computed from the climatological seasonal cycle per Myr step (no Milankovitch
  cycles, constant solar luminosity).
- No hillslope diffusion (sub-grid at ~120 km cells); sediment deposited on oceanic crust is lost
  at subduction.
- Single-threaded by user choice (efficiency before parallelism). Plate–cell matching is
  incremental (`src/tectonics/matching.rs`: crust cells + 3 rings, full-scan fallback,
  `--verify-matching`; hidden samples tracked with a nearby cell). Per-sample scratch maps are
  dense arrays (`sample_index`); other tectonics maps use `rustc-hash` via the `HashMap`/`HashSet`
  aliases in `tectonics/mod.rs`. Remaining options in `research/profiling-2026-11-15.md`.
- 3D view under KDE Wayland only draws its first frame when launched from Claude's sandboxed
  shell (compositor sends no redraw events). The user confirmed it works normally when launched
  from their own terminal. For screenshots from the sandbox, use X11:
  `env -u WAYLAND_DISPLAY`.
- CI passes on GitHub with the GUI dependencies (Linux, Windows, macOS; no extra system
  packages needed). The repository is public, so CI status can be read via the GitHub REST API
  without the gh CLI.

## Next steps (ideas)

- Parallelise ownership resolution (rayon).
- Better initial conditions (noise-based continents, interior relief).
- Glacial erosion and snow; ocean currents for coastal climate.

## User preferences

- Keep session notes in `notes/`, checked into git (see `CLAUDE.md`).
- CI: follow the user's existing "Tests and Lints" workflow template; pin actions to the latest
  major version tag.
- Before each issue, have an independent agent review the plan; get independent evaluations
  of results; agents' temporary files go to `agent-scratch/` (never `/tmp`).
- Calibrate only as far as needed to confirm soundness; prefer fixing physics over tuning.
