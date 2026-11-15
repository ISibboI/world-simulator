# Project summary

_Last updated: 2026-11-15_

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
- Working rigid-plate tectonics simulation that writes equirectangular elevation and plate maps
  as PNG snapshots. Default run (40,962 cells, 12 plates, 500 Myr) takes about 35 s.

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
- `src/render.rs`: equirectangular PNG rendering (hypsometric elevation, plate map).
- `src/main.rs`: clap CLI (`--seed`, `--subdivisions`, `--plates`, `--duration`, `--dt`, …).

## Known issues

- No plate rifting/splitting: the plate count only decreases over time (12 → ~9 in 500 Myr).
- Mean ocean floor age drifts upwards (~145 Myr after 500 Myr; Earth ≈ 60 Myr). Plates slow down
  through collision drag (~3 cm/yr on average).
- Continental area slowly decreases (30% → ~24% in 500 Myr) because continental collisions
  destroy the losing crust instead of thickening the winner.
- Continental interiors are flat (erode to 300 m); no continental shelves.
- Single-threaded; ownership resolution (~35 ms per substep) dominates.

## Next steps (ideas)

- Plate rifting, e.g. splitting large or continental-heavy plates along a random great circle.
- Slab pull as the driving force instead of random reorganisations.
- Conserve continental crust in collisions (thickening / terrane transfer to the winner).
- Parallelise ownership resolution (rayon).
- Better initial conditions (noise-based continents, interior relief).

## User preferences

- Keep session notes in `notes/`, checked into git (see `CLAUDE.md`).
- CI: follow the user's existing "Tests and Lints" workflow template; pin actions to the latest
  major version tag.
