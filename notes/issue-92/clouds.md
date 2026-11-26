# #92 stage 5: clouds

Stage 5 of [plan.md](plan.md) §10, "Clouds (optional): climate-derived cloud layer, option".
Sebastian's decisions that apply (§0 and his comments on #92):

- clouds are an **optional decoration** (decision 7): the simulation has no clouds, so they are
  derived from the climate fields; a *Clouds* switch in the Realistic mode, **off by default**;
- detail generated on the fly with distance-based LOD (pixel footprint), Detail Medium default;
- no zoom; relief and atmosphere true to scale;
- nothing but the sun, its glow and its glint may over-expose (#159, #162, #170's exposure);
- the Light option only in Realistic; Topographic stays the start-up mode;
- WebGL2-compatible (the naga GLSL ES 3.00 test covers the shader);
- an Earth-like planet, not Earth; grid artefacts (~110 km cells, ~220 km on the web) accepted.

The plan was reviewed by an independent agent before the implementation (§8) and the code
afterwards (§9). Sections 1–5 describe what was built.

**No frame change, no codec change.** Everything comes from fields the frames already hold:
precipitation, evaporation and temperature of January and July (and the year), the elevation,
the ocean flag and the moisture index. Checks below use seed 0 at 10 Myr, 5 subdivisions (the
state of the screenshots of stages 1–4).

## 1. Cloud cover and type from the climate (`modes/realistic.rs`)

### The column relative humidity, from the model's own rain law

The water cycle (`climate.rs`) rains with `P = W/τ · exp(a (r − r₀))`, where `r = W / W_sat` is
the column relative humidity, `W_sat` the saturated column of the zonal-mean sea-level
temperature of the month above the cell's elevation, `τ` = 10 days, `a` = 14, `r₀` = 0.7.
Given the cell's precipitation this inverts uniquely for `r`: `ln P` grows monotonically and
concavely with `r`, so Newton's method on the logarithm converges in a few steps.
`climate::ColumnSaturation::relative_humidity` does this with exactly the model's saturation column and
rain law (default parameters: the view does not know `--set climate.*` overrides). A unit test
runs the rain law forward and back (within 1e-4, also over high ground).

Relative humidity is what diagnostic cloud schemes use (Sundqvist 1989; Slingo 1987), and the
column value is the one this model has. In seed 0 it runs from 0.62–0.65 in the subtropics and
0.68 in the rain belt to 0.73–0.8 in the storm tracks and polar regions (10–90% spread within a
latitude band about ±0.05). The rain belt does not stand out in `r`: its large saturated column
lets it rain at moderate humidity. Deep convection therefore gets its own term from the rain
rate.

### Four regimes

Per cell and month, combined with random overlap (`c = 1 − Π(1 − cᵢ)`):

| Regime | Cover | Look (type) | Optical depth | Tops |
|---|---|---|---|---|
| Large-scale cloud (fronts, polar stratus, monsoon decks) | `smoothstep(0.62, 0.86, r)` | smooth sheets (0) | 6–20, thicker in warmer columns | 4–8 km |
| Deep convection (rain belt, monsoons) | `smoothstep(18, 26 °C, T_col) · (1 − e^−(P − 0.5)/1.2)` (P in m/yr) | clusters (0.5) | 40 | 12 km |
| Shallow (trade) cumulus | `0.25 · smoothstep(10, 20 °C, T_col)`, over land × moisture index 0.3 → 0.8 | small elements (1) | 5 | 2 km |
| Marine stratocumulus | `0.8 · smoothstep(1, 5 K, −ΔSST) · smoothstep(0.1, 0.6 m/yr, E − P)` | cells (0.7) | 12 | 1.5 km |

- `T_col` is the column temperature, the zonal mean of the sea-level temperature, as the water
  cycle uses it.
- ΔSST is the sea surface against the zonal mean of the ocean. The simulation has cold eastern
  boundary currents and upwelling: in seed 0 the subtropical ocean has anomalies down to −10 °C,
  correlated with the gyres' cooling (r = 0.46), with low P and high E. These are the
  Peru/California/Namibia-like decks.
- The type, optical depth and top height are the regimes' values weighted by their covers.
  Cold polar clouds are thin (τ ≈ 6) and look grey over the dark sea, invisible over ice.
- **Orographic clouds:** the rain law rains where air is forced up (the saturated column
  shrinks with elevation), so windward slopes have high RH and lee sides low RH. The cover
  inherits them; there is no separate term.
- **The year** is the mean of the January and July clouds: the model's annual precipitation is
  the mean of its months, each with its own saturation, so inverting it with the annual
  temperature is biased (the review's point; it overstated the cover by ~0.02).
- **Smoothing:** one pass of averaging with the neighbours, so that cloud edges do not trace
  the triangles where the cover jumps between cells.
- The tops are at least 1 km above the ground.
- Months the water cycle did not solve (NaN) fall back to the annual values; no climate at all
  gives no clouds.

Zonal means of the cover (seed 0, 10 Myr, before the neighbour smoothing; all / ocean / land):

| Lat | January | July |
|---|---|---|
| 70–80°N | 0.80 | 0.28 |
| 50–60°N | 0.86 | 0.48 (0.32 / 0.65) |
| 40–50°N | 0.74 | 0.40 |
| 20–30°N | 0.33 (0.48 / 0.02) | 0.54 (0.68 / 0.26) |
| 10–20°N | 0.38 (0.51 / 0.02) | 0.77 (0.89 / 0.46) |
| 0–10°N | 0.59 | 0.73 |
| 10–20°S | 0.77 (0.88 / 0.49) | 0.38 (0.50 / 0.04) |
| 30–40°S | 0.58 (0.65 / 0.35) | 0.35 (0.46 / 0.00) |
| 50–60°S | 0.44 (0.37 / 0.71) | 0.62 |
| 60–70°S | 0.47 | 0.88 |
| Global | 0.57 (ocean 0.62, land 0.48) | 0.56 (ocean 0.61, land 0.46) |

(Earth: ~0.67 globally from satellites, ocean ~0.7, land ~0.55; the rain belt ~0.7, the
subtropical highs 0.4–0.5 over the ocean and 0.1–0.3 over deserts, the storm tracks 0.8+.) The
rain belt moves with the season, the storm tracks are cloudiest in their winter, the
subtropical land is clear. The cover is a little below Earth's overall (thin cirrus, which
satellites count, is left out), and the summer storm tracks are clearer than Earth's (the
model's column humidity there is lower). Visually, the share of the disc that looks
white is lower than the cover, since thin trade cumulus and polar stratus are grey.

### CPU cost

Computed only with the switch on, per new frame or option change (never while dragging): two
zonal means and one Newton inversion per cell, twice for the year. Measured in the "Display
refreshed" log (§6).

## 2. Data to the GPU (`renderer.rs`)

- `DetailVertex` gains `clouds: [u8; 4]` (Unorm8x4): cover, type, optical depth / 64, top
  height / 16 km. 64 → 68 bytes: 16.7 MB at 6 subdivisions, 4.2 MB at 5. Zero with the switch
  off.
- One more interpolated varying: 10 of WebGL2's 15. The naga test now counts them.
- `DetailUniforms` gains `clouds: vec4` (most octaves, base frequency, relief, shadows).
- Two pipelines for the surface: without the clouds' code (used while they are off) and with
  it (created when they are first switched on). See §6 for why.

## 3. The shader (`clouds.wgsl`, used by `realistic.wgsl`)

### Pattern: world-anchored, static

The clouds belong to the atmosphere, so their noise is anchored to the **world frame**, not to
the plates. It is **static**: the same frame always shows the same clouds. Between frames only
the cover, type and thickness change with the climate, so the clouds do not jump or flicker as
the simulation runs. (An animated drift would need continuous redraws, which the view avoids:
it repaints only on interaction. A decision for later.)

- **Zonal stretch:** the noise is evaluated at `(x, y, 1.6 z)`. Features are up to 1.6× longer
  east–west than north–south at low latitudes, fading to round at the poles (bands of the
  rain belt, trade cumulus streets, frontal bands). A constant linear map, so its derivatives
  stay exact.
- **Synoptic systems:** a "pressure" noise ψ at 2 cycles per radius (wavelength ~3000 km) is
  the first octave. It enters with the opposite sign (lows cloudy, highs clear), weaker for
  cumulus fields. Its geostrophic flow `v = sign(lat) · up × ∇ψ` (counter-clockwise around lows
  in the north, clockwise in the south, faded out within ~15° of the equator where there is no
  geostrophic balance) advects the finer octaves by `0.012 · v` radii. The finer octaves are
  bent around the lows and highs into swirls and comma-like bands in the mid-latitudes. The
  swirl was chosen by comparing variants offline (a numpy port of the shader's noise):
  stronger or random warps marble the whole planet; none leaves featureless blobs.
- **Octaves:** from 6 cycles per radius (~1000 km) up to the pixel's Nyquist limit, the same
  per-pixel clamp as the surface detail with a fractional fade-in, at most 2 (Low), 4 (Medium),
  6 (High). The spectrum follows the type: persistence 0.5 for sheets (large smooth shapes) to
  1.2 for cumulus (small elements dominate). The sum is normalised by `sqrt(Σ a²)` of the
  amplitudes actually summed, so its spread stays 1 whatever the number of octaves.
- **Threshold:** cloud where the field exceeds `t = Φ⁻¹(1 − c)` (logistic approximation of the
  normal quantile), so the cloudy share of the pixels is the cover, at every level of detail.
  A unit test with a Rust port of the shader's noise checks this for 1–6 octaves and both
  spectra (within 0.06: the pressure field has only a few dozen lows and highs on the sphere,
  whose mean is not quite zero).
- **Thickness:** above the threshold the optical depth rises over a ramp (1.2 standard
  deviations for sheets, 0.3 for cumulus, widened by the field's change across a pixel for
  anti-aliasing) and grows towards the cores (×0.55 at the edge to ×1.6).

### Light

- **Reflection** of a non-absorbing cloud, two-stream (Bohren 1987): `R = x / (2 + x)` with
  `x = (1 − g) τ (1 + 1/μ₀)/2`, `g` = 0.85. The slant path of a low sun brightens thin clouds.
  τ = 40 gives R = 0.75 under the zenith sun. Diffuse transmission `T = 1 − R`.
- **The ground beneath** is lit through the cloud and seen through it. The sunlight's direct
  beam `e^−τ/μ₀` shows the relief, the rest arrives diffusely (`μ₀`, no relief). The diffuse
  light seen through a cloud is blurred over its thickness, a few km, far less than a pixel
  (10–20 km), so it keeps the pixel's colour; a thin cloud is therefore a veil. The glint is
  dimmed by the direct beam both ways, `e^−τ(1/μ₀ + 1/μ_v)`.
- **Cloud tops stay lit after sunset:** a top at height h sees the sun until it is `sqrt(2h)`
  below the horizon (12 km: 3.5°, ~400 km past the ground's terminator). Their incidence is
  `(i + dip)/(1 + dip)`, with `i` half the tilted normal's and half the smooth sphere's (a cloud
  is a volume: its shaded sides get diffuse light through the cloud and are grey, not black).
  With the atmosphere on, `surface_air` is evaluated at the cloud top: the point is lifted by
  the cloud's opacity `1 − e^−τ` times the top height. The light there reddens and sets at the
  cloud's own horizon, so the towers beyond the terminator glow orange, and the haze over a
  cloud is that above its top. The aerial perspective, limb haze and terminator reddening
  apply to the clouds as to the ground.
- **Relief of the tops** (Medium, High): the field's analytic gradient tilts the cloud normal,
  more for convective clouds. It shows only near the terminator.
- **Shadows** (High only): the ground's sunlight passes the cloud layer at `p + h tan θ₀`
  towards the sun, evaluated with the first 3 octaves, where that offset exceeds a third of a
  pixel. Without zoom this is only within a few degrees of the terminator, so Medium leaves it
  out (the review's point).
- **Limb:** a broken layer seen obliquely hides its gaps: the cover rises towards the limb,
  `c' = 1 − (1 − c)^(1/sqrt(μ_v))`.
- **Night side:** without ambient light the clouds are skipped where their tops see no sun.

### Exposure: nothing new over-exposes

- R is capped at 0.84, below the shoulder's knee (0.86, fresh snow). The cloud together with
  the ground seen through it is capped at the knee too (snow under a thin cloud would
  otherwise reach 0.86–0.98, the review's point). This holds with the atmosphere off as well,
  where there is no shoulder.
- The cloud's incidence goes through the same cap `min(·, 1/gain)` as the surface's (#159).
- The exposure meter (`sky.rs`) meters the sunlit share and the brightest incidence, not the
  albedo, so it is unchanged.

## 4. UI

- Option **Clouds** (switch, off by default) after *Atmosphere*, with a hover text.
- Legend: a *Cloud* swatch and one sentence while the switch is on.
- No new panel; on phones the options sit in the collapsed switcher as before.

## 5. Quality levels

| Level | Octaves after ψ | Relief of the tops | Shadows |
|---|---|---|---|
| Low | 2 | no | no |
| Medium | 4 | yes | no |
| High | 6 | yes | near the terminator, 3 octaves |

## 6. Frame times

Median frame time while dragging (the method of stages 2–4), 1280×800, 5 subdivisions, seed 0 at
10 Myr, Realistic with the Sun light, the default view (about two thirds of the disc lit). Both
renderers run on the CPU, so these are worst cases and only the ratios matter; on real GPUs,
phones included, this is far below a frame.

| | Low | Medium | High |
|---|---|---|---|
| Desktop, llvmpipe (2 threads, MSAA 4×), before | 104 ms | 128 ms | 131 ms |
| clouds off | 105 ms (+1%) | 129 ms (+1%) | 132 ms (+1%) |
| clouds on | 130 ms (+25%) | 164 ms (+28%) | 182 ms (+40%) |
| Web, chromium with SwiftShader (WebGL2), before | 443 ms | 530 ms | 543 ms |
| clouds off | 439 ms (±0) | 541 ms (+2%) | 542 ms (±0) |
| clouds on | 574 ms (+30%) | 697 ms (+32%) | 768 ms (+41%) |

- **Clouds off costs nothing.** A first version skipped the clouds by a branch on the uniform
  only. That cost nothing on llvmpipe but +18–22% on SwiftShader, whose compiler kept paying for
  the code in the branch. The view now compiles the shader twice, with the clouds' code and
  without it (a WGSL constant, `CLOUDS_COMPILED`), and creates the cloud pipeline when the
  clouds are first switched on. The naga test translates both variants.
- **Clouds on:** ψ plus 2/4/6 octaves of noise per pixel, against 2–18 evaluations for the
  surface. High pays for its 6 octaves and the shadows near the terminator.
- The web numbers need a local build with DEBUG logging and longer measuring windows (the
  frame-time log drops windows with fewer than 5 frames, and SwiftShader draws ~2 per second);
  neither change is committed.
- **CPU per update** (a new frame or option, never while dragging), at 5 subdivisions (10k
  cells): natively the cloud layers take 1.4 ms per month, 2.5 ms for the year (two months),
  and the detail mesh 3.5 → 6.1 ms with clouds. On the page (wasm, desktop CPU) the display
  refresh measured over six season switches went from ~19 ms to ~26 ms (median; noisy, the
  page renders with SwiftShader on the same CPU).

## 7. Screenshots ([clouds/](clouds/))

5 subdivisions, seed 0 at 10 Myr, Realistic, Detail Medium unless noted.

- `desktop-full-off-on.png`: the default view without and with clouds (the year, Sun light).
  The tropics carry broken cumulus and clusters, the mid-latitudes swirls around the lows, the
  desert of the eastern continent is clear.
- `desktop-jan-jul-tropics.png`: January and July, Camera light (the whole disc lit). The
  subtropics of the winter hemisphere clear, the rain belt moves towards the summer hemisphere.
- `desktop-jan-jul-south.png`: the southern ocean and the polar continent, January and July
  (Camera light). In January (southern summer) the polar land is warm and clear, the storm track
  breaks up; in July the storm track is a ring of swirling bands and the polar region is
  overcast above the sea ice.
- `desktop-terminator.png`, `desktop-closeups.png` (3×): the terminator without clouds, with
  clouds (cloud tops lit orange beyond the ground's terminator, the relief of convective tops),
  and the crescent (nothing but the glint saturates).
- `desktop-crescent.png`: the crescent (#159's angle) with clouds.
- `desktop-levels.png` (2×): Low, Medium, High. Low has smooth shapes (2 octaves), Medium the
  swirled bands, High fine cumulus texture.
- `web-full.png`: the web version (chromium, SwiftShader WebGL2) with the options and the legend
  (*Clouds* switch, *Cloud* swatch, the added sentence).
- `web-terminator.png`: the web version near the terminator.
- `phone.png`, `phone-options.png`: phone width (390×844): the clouds, and the options panel
  with the new switch; panels start collapsed as before.

## 8. Plan review (independent agent, before the implementation)

Adopted:
- normalise the octave sum by `sqrt(Σa²)` and test that the cover does not change with the
  number of octaves (a CPU port of the noise);
- the year as the mean of January and July (Jensen bias of the annual inversion);
- cap the cloud and the ground beneath it together at the knee, also with the atmosphere off;
  the cloud incidence through `min(·, 1/gain)`;
- the swirl warp was far too strong (folds); it was re-tuned offline (§3);
- the glint dimmed by the direct beam both ways; the ground's direct and diffuse sunlight;
- one pass of neighbour smoothing of the cloud layer;
- tops at least 1 km above the ground;
- the atmosphere lift with the opacity `1 − e^−τ`;
- shadows only on High;
- the inversion's round trip and guards (P = 0, NaN) tested.

Not adopted:
- the ground under a thin cloud seen only through the direct beam: at 10–20 km pixels, the
  diffuse light through the cloud is blurred far below a pixel, so it keeps the pixel's colour;
- validating the inversion against the model's own vapour on a spun-up planet: the round trip
  through the model's rain law and saturation column tests the same functions; the remaining
  difference (the annual mean, the months the frames do not hold) is handled above;
- the real `ClimateParams` in the view (`--set climate.*` overrides): a later change if
  overrides of the rain law are ever used for looks.

## 9. Code review (independent agent, after the implementation)

No high-severity findings. The reviewer confirmed that with the switch off every cloud term is
exactly 0 or 1 (the surface is drawn as before), the uniformity of the derivatives, the vertex
and uniform layouts, the guards against NaN, the exposure cap, the geometry of shadow, swirl and
horizon dip, and the inversion.

Fixed:
- **Shadow seams:** where no offset shadow is drawn, the ground's light used the cloud's
  transmission with the limb's increased cover; now the cell's own cover (no extra noise).
- **CPU cost:** no allocation per cell in the neighbour smoothing; the latitudes once per
  update; the saturated column once per latitude bin (the column temperature is zonal), with
  one logarithm per Newton step from a close first guess.
- Tests: the cap on cloud plus ground is now a sweep over a port of the composition; the tops
  are tested on the layers; fractional octaves in the cover test.
- The loop bound and stale comments.

Not changed:
- the ambient fill of *Sun + ambient* and *Camera* can lift a cloud a few per cent above the
  knee, as it already does fresh snow (atmosphere.md);
- the swirl's Jacobian is left out of the field's gradient, so the anti-aliasing and relief can
  be off by up to ~2× where the swirl is strongest (no artefacts seen);
- High costs the most (§6); the default stays Medium.

## 10. Limits (accepted)

- Decoration: the clouds are not simulated, have no effect on the climate, and their pattern is
  a fixed noise; only their amount, type, thickness and height follow the climate.
- The cover changes between cells (~110/220 km), smoothed over one ring; the structure inside
  is noise.
- No parallax: the clouds are drawn on the surface. 12 km is less than a pixel except at the
  extreme limb, so no clouds stand out above the limb.
- Summer storm tracks are clearer than Earth's (the model's column humidity there is lower).
- The view uses the default rain-law parameters.
