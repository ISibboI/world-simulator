# #213: results — start near the tectonic steady state, explicit ramp-up phase

Plan and review: [plan.md](plan.md). Project goal: an Earth-like planet, not Earth; the targets
are the model's own steady state.

## What changed

1. **Initial plates** (`tectonics/generation.rs`): default `--plates` 12 → **21**, with the
   model's steady plate-size distribution instead of a Zipf `rank^-0.9`: the plate of rank r
   (0 = largest) gets a share ∝ `exp(−r^0.74 / 1.8)` (± 25% log-normal scatter, at least 0.5%
   of the surface). The shape is fitted to the rank–area table of `main` with #195 (4 seeds ×
   1000 Myr, `--plate-log`, 300–1000 Myr: 18 plates, the largest 31%, a third below 1%; fit
   error 0.03 in squared log share, `tools/fit_shape.py`). It depends on the rank only, so any
   `--plates` takes the first ranks. Small plates (< 2%) start in the ocean where possible, and
   the weight iteration keeps its best partition (a small plate could flip between a few cells
   and a whole continent).
2. **Primed mantle** (`tectonics/mantle.rs`, `MantleFlow::prime`, `mantle.steady_start`, on by
   default): after the first step, the downwellings hold the trenches' subduction rates, every
   sunken-slab stage the fixed point of its turnover, and the plumes are the steady population
   of the birth process (~9, all ages).
3. **Ramp-up phase** (`src/ramp_up.rs`, `--set ramp_up.…`): replaces the fixed 150-Myr hold
   (`carbon.calibration_time` and `calibration_span` are gone). Every 10 Myr, after at least
   30 Myr, the least-squares trends over the last 30 Myr are checked: the tectonic degassing
   (without the φ-dependent seafloor-carbon return) must not have grown by more than 5%
   (one-sided) and the effective plate count `1/Σaᵢ²` must not have changed by more than 25%.
   Then the phase ends, φ is calibrated on exactly that 30-Myr span, and the CO₂ follows the
   cycle; at 150 Myr it ends anyway (warning). Logged: start (with the criterion), every check,
   the end.
4. **UI** (`viewer/parameters.rs`): under "t = X of Y Myr" in the parameter panel, a small
   global-status section (for #209 to grow): "Ramp-up (tectonics settling) — CO₂ held" / "Until
   the plate count and the degassing are steady.", then "Ramp-up ended at X Myr" / "The
   tectonics had settled; the carbon cycle sets the CO₂ since." It is a property of the run
   (like its time), sent with every update (desktop channel, web worker message), not stored
   in the history frames, so no frame-codec change. Plain labels (no new touch targets), font
   from egui/the browser; on phones the panel is collapsed by default as before.
   Screenshots: [desktop, running](desktop-ramp-up.png), [desktop, ended](desktop-ended.png),
   [phone 390×844, running](phone-ramp-up.png), [phone, ended](phone-ended.png).

## How it was evaluated

#195 was merged into `main` while this was in progress. It changes the steady plate count
(~28 → ~18 long-term) and what ships is #196 + #195 + this, so all runs below use local
merges with `origin/main` (not pushed): **base** = #196 head (f7090f6) + main, **new** = this
branch + main. Runs via `runs`, `--snapshot-interval 10`; analysis with
[tools/analyse.py](tools/analyse.py) and [tools/criterion.py](tools/criterion.py).

## Default barren planet, 8 seeds × 1000 Myr (seed means)

Degassing: 10-Myr means of the logged sources (Tmol C/yr).

| t (Myr) | base: plates | base: degassing (ridges/arcs/plumes/meta) | new: plates | new: degassing (ridges/arcs/plumes/meta) |
|---|---|---|---|---|
| 10 | 12.8 | 4.9 (1.7/3.1/0.0/0.2) | 22.1 | **9.6** (3.0/5.1/1.0/0.4) |
| 20 | 13.4 | 6.4 | 22.8 | 10.5 |
| 50 | 13.9 | 8.1 (2.7/4.8/0.2/0.3) | 21.1 | **10.5** (3.3/5.8/1.0/0.4) |
| 100 | 17.0 | 9.4 (3.1/5.4/0.5/0.4) | 22.0 | 11.5 (3.5/6.5/0.9/0.6) |
| 150 | 20.5 | 11.1 | 22.2 | 12.0 |
| 200 | 22.6 | 13.5 (4.0/7.8/1.0/0.7) | 24.6 | 13.0 (3.7/7.3/1.0/1.1) |
| 300 | 25.2 | 13.5 | 24.1 | 12.9 |
| 500 | 21.5 | 12.7 | 25.9 | 14.4 |
| 1000 | 16.2 | 11.5 | 16.5 | 11.8 |

| | base | new |
|---|---|---|
| degassing at 10 Myr / at 200–500 Myr | 4.9 / 13.2 (37%) | 9.6 / 13.6 (71%) |
| degassing at 50 Myr / at 200–500 Myr | 61% | 77% |
| ridges (pure tectonic rate) at 10 / 50 Myr vs 200–500 | 44% / 70% | 80% / 87% |
| plumes at 10 Myr | 0.0 (0 plumes) | 1.0 (10 plumes, steady ~9–10) |
| mean ocean age at 0 / 50 / 100 / 200 Myr | 60 / 70 / 69 / 58 | 60 / 67 / 64 / 61 |
| largest plate at 0 / 200 / 1000 Myr | 27 / 21 / 32% | 27 / 22 / 34% |
| ramp-up end (Myr) | fixed 150 (offline replay of the new criterion: 60–150, mean 99) | **30, 60, 40, 50, 90, 40, 90, 40 (mean 55, median 45)** |
| φ | 1.04–1.44 | 0.92–1.40 |
| pCO₂ (bar) at 0 / 200 / 300 / 1000 Myr | 0.493 / 0.561 / 0.548 / 0.351 | 0.493 / 0.536 / 0.577 / 0.341 |
| global temperature at 50 / 1000 Myr (°C) | 13.9 / 17.6 | 14.0 / 18.0 |

- **Most of the ramp is gone.** The degassing starts at ~70% of its 200–500-Myr level instead
  of ~37%; the ridge source (the new sea floor) at ~80–87% within 50 Myr instead of 44–70%.
  The plumes are there from the start.
- **A slow rise remains** (10.5 → 13 Tmol/yr from 50 to 200 Myr, ~25%): partly the plate count
  (21 → 24–25 by 150–200 Myr, the model's count over its first few hundred Myr, before its
  long-term decline to 16–18 with #195), partly arcs, and the metamorphic stock (0.4 → 1.1,
  option E of the analysis, not done). After the phase ends this rise raises the CO₂ by
  ~15% (0.49 → 0.58 bar at 300 Myr; the base, which calibrates later at 150 Myr, rises by
  ~11%). Both then fall with the brightening sun (0.34–0.35 bar at 1000 Myr).
- **The criterion discriminates**: the same criterion replayed on the base's old 12-plate start
  ends at 60–150 Myr (mean 99), on the new start at 30–90 (mean 55). Two seeds of eight need
  90 Myr: their degassing rose for a while with a plate-count change.
- The long-term state is unchanged within the seed scatter (plates, largest plate, temperature,
  CO₂ at 500–1000 Myr).

## Variants (4 seeds × 300 Myr, CO₂ held throughout)

Degassing (Tmol/yr) at 20 / 50 / 100 / 150 Myr and the plate count at 150 Myr:

| variant | 20 | 50 | 100 | 150 | plates at 150 |
|---|---|---|---|---|---|
| 15 plates | 9.4 | 10.5 | 10.9 | 11.3 | 19 |
| 18 plates | 9.2 | 10.6 | 12.0 | 15.2 | 28 (overshoot) |
| **21 plates** | **11.1** | **10.4** | **12.4** | **12.4** | **26** |
| 18 plates, mantle not primed | 6.9 | 8.4 | 11.7 | 13.1 | 26.5 |

21 plates gives the flattest early degassing; the long-term mean of 18 (with #195) overshoots
in the first 150 Myr, because the model's plate count over its first few hundred Myr is
higher (22–29) than later. The mantle priming adds ~2 Tmol/yr in the first 50 Myr (the plume
source and faster plates).

## Modern Earth (`--biosphere modern --epoch 4`) and supercontinent, 4 seeds × 1000 Myr

| | ramp-up end (Myr) | degassing at 20 / 100 / 200 Myr | φ |
|---|---|---|---|
| base modern | 150 (fixed) | 4.5 / 6.9 / 10.3 | clamped to 3 (balancing 4.3–5.7) |
| new modern | 30, 30, 40, 50 | 8.0 / 9.8 / 11.2 | clamped to 3 |
| base supercontinent | 150 (fixed) | 4.8 / 8.3 / 10.9 | 1.0–1.4 |
| new supercontinent | 40, 40, 110, 40 | 9.4 / 12.0 / 11.6 | 1.17–1.74 |

- Modern: φ hits #196's plausible-range clamp in both (a #196 matter); the CO₂ then drifts in
  both after the calibration.
- **Supercontinent caveat**: the superocean becomes the largest plate, and the remaining plates
  mostly fall inside the supercontinent, so 21 plates cut it into ~10–12 pieces (12 plates cut
  it too, into fewer). The plate count falls back to 17–19 within 100 Myr (sutures), and the
  largest plate stays at 26–30% instead of growing to 35–40% as in the base. A supercontinent
  start may want fewer plates, or a partition that keeps the supercontinent together (a
  follow-up if wanted).

## Caveats

- **#195**: the numbers are with #195; the default `--plates` 21 and the size shape are for
  it. This branch alone (pre-#195, where the model settles at ~28 plates) would start a little
  low. When #196 picks up main, nothing needs retuning; if the plate dynamics change again,
  `tools/rank_areas.py` + `tools/fit_shape.py` refit the shape from a `--plate-log` run.
- The model has no true plate-count steady state with #195 (22–25 plates at 100–500 Myr, 16–18
  at 800–1000 Myr, the largest plate growing to 31–35%): "near the steady state" means near the
  first few hundred Myr.
- Boundaries and ocean ages are still not consistent with the motion (option B of the
  analysis), and the metamorphic stock still builds up over ~250 Myr (option E).
- The mantle priming happens after the first step (it needs the first solve's trenches), so the
  plumes' swells and the slab loads appear at t = 1 Myr; no jump in the sea level beyond the
  usual first-step reorganisation was found.
- Grid: at 4–5 subdivisions (web, quick runs), the degassing per step is noisier; the phase
  then ends later or at the safety net more often (a 4-subdivision GUI check of the pre-#195
  branch ended at 120 Myr).
