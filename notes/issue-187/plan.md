# #187: slow cloud drift with the wind (Realistic, detail High)

Sebastian's decision (2026-11-26): the clouds drift only at detail level **High**; Low and
Medium keep the static clouds of #183 (`notes/issue-92/clouds.md`) in look and cost.

## Constraints

- The drift moves only the shapes: cover, type, optical depth and height per cell stay as they
  are, and the share of cloudy pixels stays the cover (the threshold is the normal quantile of
  a unit-variance field).
- No shear or tearing over time, also where the wind belts meet.
- Continuous redraws only while Realistic is shown with clouds on at High, throttled.
- WebGL2: varyings (≤ 15; the naga test counts them), uniforms, both/all shader variants
  translate.
- An Earth-like planet, not Earth; grid artefacts accepted (#64).

## Design

### Wind per vertex

`DetailVertex` gains `wind: [i8; 4]` (Snorm8x4): the season's low-level wind of the cell
(`Period::wind`, the model's three-cell circulation: trades, westerlies, polar easterlies,
following the ITCZ by longitude and season) as a world-frame tangent vector, divided by 20 m/s.
Zero unless the drift is on, so the mesh of Low/Medium is unchanged apart from four bytes per
vertex (68 → 72). One more varying (`wind`, vec3): 11 of WebGL2's 15. The world vector is
interpolated over the triangles (no east/north basis per pixel, no pole singularity).

### Flow noise: two phase-shifted layers per scale, cross-faded

A pure advection `u − v t` of the noise shears without bound where the wind changes (the trade
winds against the westerlies), so the standard flow-map technique is used: each layer is
advected for a limited time only, `D = v · (φ − ½) T` with its phase `φ ∈ [0, 1)`, and two
layers half a period apart are cross-faded with the weights `sin(π φ)` and `|cos(π φ)|`. A
layer is reset where its weight is 0, so the reset is invisible; the largest advection while a
layer is visible is ± T/2.

- **Independent layers:** each reset draws a new seed (an offset in the noise lattice, hashed
  from the cycle number), so the two layers are independent realisations and the pattern never
  repeats. With weights `w_a² + w_b² = 1`, the sum of two independent unit-variance fields has
  unit variance again (and stays near-Gaussian), so the quantile threshold still gives the
  cover: **the cloudy share does not change** at any moment. (Same-seed layers would be
  correlated by an amount that depends on the shift and the wind, and the variance would pulse.)
  Clouds thus form and dissolve as they drift, as real ones do.
- **Two time scales:** the synoptic pressure field ψ (the lows and highs, ~3000 km) has a period
  of 6 days; the finer octaves (≤ 1000 km) a period of 2 days. Real lows live about a week,
  smaller cloud systems hours to days. (First planned 3 and 1 days; the plan review showed that
  then the shapes would change faster than they move, boiling in place instead of drifting.) The swirl around ψ's lows is evaluated from the blended
  ψ at the pixel, so the finer octaves are bent around the moving lows.
- **Shear:** the model's wind is a zonal-mean profile (`climate::low_level_wind`), piecewise
  smooth, with at most ~5·10⁻⁶ s⁻¹ of shear at the 30° and 60° belt boundaries; its meridional
  part (≤ 0.6 m/s) converges at the ITCZ by only ~10⁻⁶ s⁻¹. The largest deformation of a layer
  is ±0.43 for the fine octaves (±24 h) and ±1.3 for ψ (±72 h), both where the layer's weight
  is near 0; at weight 0.7 (a quarter period from the peak) half that. ψ is smooth, so this
  shows as slow changes of the lows' shapes, not as tearing. (A wind field with jumps from cell
  to cell would tear; the analytic profile has none.)

### Display clock

The drift runs on the display's own clock (egui's input time), not the simulation's: **1 hour
of weather per second** (a day in 24 s). At 8 m/s (the model's westerlies) the clouds move
~29 km/s, 1–2 pixels per second in the default 1280×800 view: a slow drift. The CPU computes
the phases, weights, shifts and seeds in f64 and passes them as uniforms (`drift`, 5 vec4), so
the GPU never sees a large time value.

### Shader variants

A third pipeline: clouds with drift (`CLOUD_DRIFT_COMPILED`), created when the drift is first
shown. Low/Medium use the clouds pipeline of #183 unchanged; the non-drift code path evaluates
the same field as before (seed 0, no shift). The drift doubles the noise evaluations of the
clouds (two layers of ψ and of every octave, also for the shadows).

### Repaint

While Realistic is shown with clouds on at High, the view asks for a repaint after an interval
that moves the fastest clouds (10 m/s) by half a pixel per frame at the centre of the disc,
clamped to 2–30 per second: ~4 per second in a 1280×800 window, more on large screens
(the view has no zoom). No continuous repaint otherwise. The page stops drawing in a hidden
tab by itself.

### Known effects (accepted)

- A new frame or a season switch changes the wind; since the shift is `wind × (φ − ½) T`, the
  pattern then moves at once by up to Δv·T/2 (1 m/s: ~260 km for ψ). Mostly masked by the
  cover changing in the same frame.
- Switching to or from High swaps the static pattern for another realisation.
- The clouds keep moving while the timeline is paused (display clock).
- Clouds are steered by mid-level winds (10–15 m/s in the mid-latitudes); the model has only the
  low-level wind, which fits trade cumulus and is plausible overall.

## Checks

- Unit tests: a port of the blended field (cloudy share = cover at several moments); the
  weights' squares sum to 1 and layers reset only at zero weight; the repaint/drift only at High
  with clouds on; the mesh's wind only then; the naga test for all three variants (11
  varyings).
- Visual: two screenshots at High some seconds apart; the web version at 390×844.
- Cost: idle CPU with drift on/off, natively and on the web; frame times at High.
- Independent code review.

## Plan review (independent agent)

Adopted: longer periods (fine 2 d, ψ 6 d) so that the clouds drift rather than boil; the
repaint interval from the on-screen speed instead of a fixed 10 Hz; the cover test at covers
0.05–0.95 and several phases; the blend per scale before the single normalisation (as
planned); the legend no longer says "fixed in space" at High; the known effects above.

Not adopted: a `wind` varying only in the drift variant (a second output struct): the measured
cost of the unused varying at Low/Medium is within noise (see results.md); three layers a third
apart (1.5× rather than 2× cost relative to static, but not needed with the longer periods);
blending the old and new wind after a frame change.
