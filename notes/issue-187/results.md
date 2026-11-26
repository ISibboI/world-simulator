# #187: slow cloud drift with the wind (results)

Design and plan review: [plan.md](plan.md). Sebastian's decision: drift only at detail **High**;
Low and Medium keep the static clouds of #183.

## What was built

- **Wind per vertex:** `DetailVertex.wind` (Snorm8x4, 68 → 72 bytes): the season's low-level wind
  of the cell (the option *Season*: year, January or July) as a world-frame vector / 20 m/s,
  only at High with clouds on (zero otherwise). One more varying: 11 of WebGL2's 15.
- **Flow noise** (`clouds.wgsl`, `cloud_field`): two layers per scale, half a period apart,
  each advected along the wind for at most ±½ period and cross-faded with `sin(πφ)` and
  `|cos(πφ)|`; a layer takes a new seed (a lattice offset within ±32) where its weight is 0.
  The pressure field ψ (lows, highs) has a period of 6 days, the finer octaves 2 days. The
  weights' squares sum to 1 and the layers are independent, so the field keeps unit variance
  and the quantile threshold still gives the cover: the cloud amount, type, optical depth and
  height per cell are untouched, only the shapes move. The swirl around the lows follows the
  blended ψ.
- **Display clock:** egui's input time, 1 hour of weather per second. The CPU computes phases,
  weights, shifts and seeds in f64 (`modes::realistic::cloud_drift`) and passes them as five
  vec4 uniforms (`detail.drift`), written every drawn frame.
- **Third pipeline:** `CLOUD_DRIFT_COMPILED` (constant replaced per variant): without clouds,
  static clouds (Low/Medium, the code path of #183), drifting clouds (High), each created when
  first used. The naga test translates all three to GLSL ES 3.00 and counts 11 varyings.
- **Repaint:** while Realistic is shown with the clouds on at High, `request_repaint_after` an
  interval that moves 10 m/s clouds by half a pixel at the centre of the disc, clamped to 2–15
  per second, and only once the surface with clouds is shown: 4.1 per second in a 1280×800 window (measured: 241 ms between idle frames).
  Nothing else changes: Low/Medium do not repaint while idle.
- **Legend:** "…their pattern is noise, which drifts slowly with the season's winds (an hour of
  weather per second)" at High, "…fixed in space (it drifts with the wind at detail High)"
  otherwise.

## Checks

- **Same field as before at Low/Medium:** with the drift variant given the static uniforms
  (seed 0, weight 1, no shift, a temporary debug hook not committed), High rendered the same
  image as #183's High (largest difference 1/255 on one pixel of a 600×600 crop). Low/Medium
  use the static variant, whose arithmetic is #183's (only the octave sum is accumulated
  separately).
- **Unit tests** (`modes/realistic.rs`):
  - `drifting_clouds_keep_their_cover`: a Rust port of the drifting field with a zonal wind of
    trades, westerlies and polar easterlies, at 12 moments, both spectra, 6 octaves: the share
    of points above the threshold is the cover within 0.06 for covers 0.05–0.95 (the tolerance
    of #183's cover test; single moments scatter by ~0.03 with the realisation of the few dozen
    lows), and the cloud amount does not pulse: averaged over six periods, the moments with one
    layer of the finer octaves and those with an even blend differ by at most 0.007 (bound 0.01);
  - `drift_layers_fade_out_before_they_restart`: over 1000 s of display time in 50 ms steps, a
    layer changes its seed only where its weight is < 0.01, otherwise its shift moves
    continuously and grows (downwind); the layers of a scale have different seeds; seeds within
    ±32;
  - `clouds_drift_only_at_high_with_the_clouds_on`: the drift (and the wind in the mesh) only at
    High with the clouds on, zero drift in the uniforms built per display; the repaint interval
    (0.2–0.3 s in a 1280×800 window, never under 1/15 s, never over 0.5 s);
  - `the_drift_wind_points_along_the_wind`: east/north to world, clipping along the direction,
    NaN;
  - `shaders_translate_to_webgl2`: three realistic variants, 11 varyings.

## Cost

Seed 0 at 10 Myr, 4 subdivisions, 1280×800, Realistic, Sun light, the default view. Software
renderers on the CPU (llvmpipe with 2 threads, SwiftShader), so only the ratios mean anything;
on a real GPU, phones included, the drawing is far below a frame.

| | before (#183) | with the drift |
|---|---|---|
| Desktop, median frame time while dragging, Medium | 131 ms | 132 ms (+1%) |
| Desktop, median frame time while dragging, High | 149 ms | 177 ms (+19%) |
| Desktop, idle CPU, Medium | 0% | 0% |
| Desktop, idle CPU, High | 0% | 152% of a core (llvmpipe drawing 4.1 frames/s) |
| Web (chromium, SwiftShader), idle CPU, Medium | — | 7% (page idle) |
| Web (chromium, SwiftShader), idle CPU, High | — | 588% (SwiftShader draws continuously) |

- **Low/Medium:** unchanged (the extra vertex attribute and varying cost ~1%, within the
  noise of the measurement).
- **High, per frame:** the drift doubles the noise of the clouds (two layers of ψ and of every
  octave, also for the shadows near the terminator): +19% on llvmpipe at this view. #183
  measured clouds at High as +40% over no clouds, so roughly +60% over no clouds now.
- **High, idle:** the view redraws ~4 times a second. On a GPU that is ~4 cheap frames per
  second (the CPU side per frame is egui's layout and a uniform write). With software
  rendering every frame is expensive: llvmpipe at 4 frames/s keeps 1.5 cores busy, and
  SwiftShader, which takes longer than the 241 ms interval per frame, draws without pause and
  used ~6 cores of this machine. A phone or browser without a GPU would therefore run hot at
  High; Medium (the default) stays idle.
- The web frame times while dragging were not measured (the page logs at INFO; #183 needed a
  local DEBUG build for them); the ratio is the same shader cost as on the desktop.

## Screenshots ([screenshots/](screenshots/))

- `desktop-drift-0s-10s.png`: High, Camera light, 10 s apart (10 hours of weather): the
  mid-latitude bands and swirls move east with the westerlies, the trade cumulus west; shapes
  form and dissolve as layers fade.
- `desktop-high.png`: the whole window at High with the drift.
- `web-drift-0s-10s.png`: the web version (SwiftShader, WebGL2), High, 10 s apart.
- `phone.png`: 390×844, with the options open (Detail High, the legend's new sentence) and
  closed; the panels stay as before.
- `swirls-183-vs-other-seed.png`: the far side of the planet, #183's static High (left) and the
  drift's field with another seed (right). See the caveat below.

## Code review (independent agent)

No blocking findings: layouts (vertex 72 bytes, the uniform array's alignment), pipeline
selection, the static path's equivalence, the sign of the advection, the layer maths and the
precision were confirmed. Fixed:
- the cover test was too weak for "the amount does not change": it now compares single-layer and
  blended moments (no pulsing, above) and pins the downwind direction;
- the repaint only once the surface with clouds is there (not while it loads);
- at most 15 instead of 30 repaints per second (large screens);
- winds above 20 m/s are clipped along their direction, not per component.

Not changed: the extra vertex attribute (4 bytes) and varying in all variants, which a separate
vertex stage for the drift would avoid. Medium measured +1% (131 → 132 ms), within the noise;
kept simple.

## Caveats

- **Swirls at High look stringier than in #183's default view.** The fine octaves bent around
  the lows (#183's swirl) look combed where a low is strong. Seed 0's static pattern happens to
  be calm on the side shown at start, but #183's High has the same combed swirls elsewhere
  (`swirls-183-vs-other-seed.png`); ψ's spread over the sphere is the same for every seed
  within ~5%. With the drift, every realisation passes by, so they show more often. Tests with
  the ψ shift off and with shorter periods looked the same, so it is not shear from the drift.
  If it bothers, the swirl could be weakened at High (a #183 tuning, not part of this change).
- **A new frame or a season switch** changes the wind, so the pattern jumps by up to
  Δv·T/2 (1 m/s: ~260 km for ψ, ~90 km for the fine octaves), mostly hidden by the cover
  changing in the same update. Switching to or from High swaps the static pattern for another.
- **The clouds keep moving while the timeline is paused** (display clock).
- **Steering wind:** clouds move with mid-level winds (10–15 m/s in the mid-latitudes); the
  model only has the low-level wind (westerlies ≤ 8 m/s), which suits trade cumulus and is
  plausible overall.
- **Grid:** the wind is the cell's (110 km, 220 km on the web), interpolated over the
  triangles; the model's wind profile is smooth, so this shows no seams.
