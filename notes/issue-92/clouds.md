# #92 stage 5: clouds (plan)

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

## 1. Data: no frame change

Everything comes from fields the frames already hold (no codec change):
precipitation, evaporation, temperature and the low-level wind of the year, January and July,
the elevation, and the ocean flag. The checks below use seed 0 at 10 Myr (5 subdivisions; the
same climate as the screenshots of stages 1–4).

### Cover from the model's own rain law: the column relative humidity

The water cycle (`climate.rs`) rains with `P = W/τ · exp(a (r − r₀))`, where `r = W / W_sat` is
the column relative humidity, `W_sat` the saturated column of the zonal-mean sea-level
temperature of the month above the cell's elevation, `τ` = 10 days, `a` = 14, `r₀` = 0.7.
Given the cell's precipitation this inverts uniquely for `r` (the right side grows
monotonically with `r`; a few Newton steps). Relative humidity is what diagnostic cloud schemes
use (Sundqvist 1989; Slingo 1987), and the column value is what this model has. I expose the
inversion from `climate.rs` (`column_relative_humidity`) so it uses exactly the model's
saturation column and rain law (default parameters; the view does not know `--set climate.*`
overrides).

Seed 0, 10 Myr, the column RH by latitude (annual): subtropics 0.62–0.65, ITCZ 0.68, storm
tracks 0.73–0.78, polar 0.75–0.8, Antarctic-like plateau 0.68; the 10–90% range within a band
is about ±0.05. The ITCZ does not stand out in `r` (its large saturation column lets it rain
at moderate RH), so deep convection gets its own term from the rain rate.

### Three cloud regimes per cell (season of the Season option)

1. **Large-scale (stratiform) cloud** from the column RH:
   `c_ls = smoothstep(0.58, 0.84, r)`. Fronts in the storm tracks, polar stratus, monsoon
   decks.
2. **Deep convection** where the column is warm and it rains a lot:
   `c_cv = smoothstep(18, 26 °C, T_column) · (1 − exp(−P / 1.5 m/yr))`. ITCZ, monsoons,
   summer tropics.
3. **Marine stratocumulus** over cold subtropical water under subsidence:
   `c_sc = 0.8 · smoothstep(1, 5 K, −ΔSST) · smoothstep(0.1, 0.6 m/yr, E − P)`, with ΔSST the
   cell's sea-surface temperature minus the zonal mean of the ocean at its latitude (3° bins).
   The simulation has cold eastern boundary currents and upwelling: in seed 0 the subtropical
   ocean has anomalies down to −10 °C, correlated with the gyres' cooling (r = 0.46), with low P
   and high E: the Peru/California/Namibia-like decks.

Total cover `c = 1 − (1 − c_ls)(1 − c_cv)(1 − c_sc)` (random overlap), plus per cell:
- the **convective share** `k = c_cv / (c_ls + c_cv + c_sc)` (texture: crisp, clumpy, towering
  vs. smooth sheets);
- the **mean optical depth** of the cloudy part, mixed by the shares: stratiform 6–20
  (thicker with warmer, wetter columns: `6 + 14 · smoothstep(−20, 15 °C, T_column)`, so cold
  polar and plateau clouds are thin), convective 40, stratocumulus 12;
- the **cloud-top height**, mixed by the shares: stratiform 4–8 km (warmer columns higher),
  convective 12 km, stratocumulus 1.5 km.

**Orographic clouds:** the rain law already rains where air is forced up (its saturation
column shrinks with elevation), so windward slopes have high RH and lee sides low RH: the cover
inherits them. No separate term.

Zonal means of the cover (seed 0, 10 Myr; ocean / land):

| Lat | Year | January | July |
|---|---|---|---|
| 60–70°N | 0.89 | 0.96 | 0.81 |
| 40–50°N | 0.59 | 0.85 | 0.37 |
| 20–30°N | 0.49 (0.63 / 0.21) | 0.34 | 0.57 |
| 0–10°N | 0.75 | 0.64 | 0.76 |
| 10–20°S | 0.66 | 0.78 | 0.42 |
| 30–40°S | 0.51 (0.58 / 0.27) | 0.61 | 0.35 |
| 50–60°S | 0.66 | 0.51 | 0.76 |
| Global | 0.645 (ocean 0.68, land 0.58) | 0.63 | 0.61 |

Earth: ~0.67 globally (ISCCP/MODIS, ocean ~0.7, land ~0.55), ITCZ ~0.7, subtropical highs
0.4–0.5 (deserts 0.1–0.3), storm tracks 0.8+. The ITCZ moves with the season (January peak at
10–20°S, July at 0–10°N), the storm tracks are cloudiest in their winter, the subtropical land
is clear. Polar winters are very cloudy (0.9+), more than Earth's (0.6–0.7 under the Siberian
high); they are dark in those months anyway.

Cost on the CPU per update: zonal means of the sea-level and ocean temperature (one pass over
the cells), and the RH inversion per cell (≤ 8 Newton steps, one `exp` each). Expected < 1 ms
at 5 subdivisions; measured with the "Display refreshed" log. Only computed with the switch on.

## 2. Vertex data

`DetailVertex` gains `clouds: [u8; 4]` (Unorm8x4): cover, convective share, mean optical depth
/ 64, cloud-top height / 16 km. 64 → 68 bytes (16.7 MB at 6 subdivisions, 4.2 MB at 5). One more
interpolated varying (10 of WebGL2's 15). The cover is interpolated across the triangle, so the
cloud edge between a clear and a cloudy cell falls where the interpolated cover crosses the
noise, not along the triangles. With the switch off the bytes are zero and the shader skips the
clouds by a uniform branch (no cost).

`DetailUniforms` gains `clouds: [f32; 4]`: on/off with the most octaves, the base frequency,
the swirl and the shadows (by Detail level).

## 3. The shader (`realistic.wgsl`)

### Pattern: world-anchored, static

The clouds belong to the atmosphere: their noise is anchored to the **world frame** (not to the
plates). It is **static**: a frame always shows the same clouds; between frames only the cover
changes with the climate, so the clouds do not jump or flicker as the simulation runs. (An
animated drift with the wind would need continuous redraws, which the view avoids: it repaints
only on interaction. A decision for later.)

- **Zonal stretch:** the noise is evaluated at `(x, y, 1.6 z)`: features are ~1.6× longer
  east–west than north–south at low latitudes, fading to isotropic at the poles (bands of the
  ITCZ, trade cumulus streets and frontal bands are zonal). A constant linear map, so no
  distortion of the derivatives.
- **Synoptic systems and their swirl:** a "pressure" noise `ψ` at 3 cycles per radius
  (wavelength ~2000 km) is the first octave, entering the cloud field as `−ψ` (lows cloudy,
  highs clear). It also warps the finer octaves along its geostrophic flow
  `v = sign(lat) · up × ∇ψ` (counter-clockwise around lows in the north, clockwise in the
  south), faded out within ~15° of the equator where there is no geostrophic balance. The warp
  displaces the field by up to ~0.06 radii around the lows and highs, which bends the bands into
  comma and spiral shapes. Medium and High only.
- **Octaves:** from 6 cycles per radius up to the pixel's Nyquist limit (the same per-pixel
  clamp as the surface detail, with fractional fade-in), at most 2 (Low), 4 (Medium), 6 (High)
  after `ψ`. The persistence (amplitude ratio per octave) grows with the convective share
  (0.45 stratiform: smooth sheets; 0.62 convective: clumpy), and the sum is normalised by the
  amplitudes actually summed, so the cover does not change with the level of detail.
- **Threshold:** cloud where the normalised field exceeds `t = σ · Φ⁻¹(1 − c)` (logistic
  approximation of the normal quantile), so the cloudy share of the pixels is the cover. Above
  the threshold the optical depth grows over a soft ramp (wide for stratiform, narrow for
  convective clouds), anti-aliased by the field's analytic gradient times the pixel footprint.

### Light

Two-stream reflection of a non-absorbing cloud (Bohren 1987):
`R = x / (2 + x)`, `x = (1 − g) τ · (1 + 1/μ₀)/2`, `g` = 0.85, μ₀ the sun's zenith cosine
(longer slant path at low sun brightens thin clouds); transmission `T = 1 − R`. τ = 40 gives
R = 0.75 at the zenith sun. **R is capped at 0.84**, below the shoulder's knee (0.86, fresh
snow), so a cloud is never brighter than the brightest surface and the #159/#162 cap holds
(clouds are lit by the same incidence cap `min(n·l, 1/gain)`).

Per pixel: `colour = R · L_cloud + T · T_shadow · body · L_ground` (+ the glint × e^−τ), where
the ground under a cloud is lit through it and seen through it.

- **Cloud tops stay lit after sunset:** a cloud top at height h sees the sun until it is
  `sqrt(2h)` below the horizon (12 km: 3.5°, ~400 km past the ground's terminator). The cloud's
  incidence is `(n·l + dip)/(1 + dip)` (a volume scatters light arriving from the side), and with
  the atmosphere on, `surface_air` is evaluated at the cloud-top height, which reddens and cuts
  the light at the cloud's own horizon. So towers glow pink-orange beyond the terminator.
- **Relief of the cloud tops (Medium, High):** the field's analytic gradient tilts the cloud
  normal, strongly for convective towers, little for sheets; it shows near the terminator.
- **Shadows (Medium, High):** the ground's sunlight passes the cloud layer at
  `p + h · tan θ₀` towards the sun; the field is evaluated there with the first 3 octaves.
  Only where that offset exceeds a third of a pixel (low sun: near the terminator); elsewhere
  the shadow is the pixel's own cloud (hidden under it).
- **Atmosphere:** one `surface_air` call as now, at the point lifted by `(1 − T) · h` (the
  cloud's opacity times its top height): over thick clouds the haze is that above the cloud top
  (less aerosol), the sunlight that at the cloud top (later sunset); in the gaps the ground's.
  The aerial perspective, limb haze and terminator reddening therefore apply to the clouds too.
- **Limb:** looking obliquely through a broken layer hides its gaps: the cover rises towards the
  limb, `c' = 1 − (1 − c)^(1/sqrt(μ_v))`.
- **Glint:** dimmed by the cloud (e^−τ): no glint under clouds.
- **Night side:** without ambient light the clouds are skipped where neither the ground nor a
  12 km top sees the sun (the night side costs nothing extra).

### Exposure

Nothing new over-exposes: cloud reflectance ≤ 0.84 < knee 0.86, same incidence cap, same
shoulder. The exposure meter (`sky.rs`) meters the sunlit share and the brightest incidence,
not the albedo, so it is unchanged. Brighter clouds make the planet brighter overall (Earth's
albedo ~0.3 vs ~0.15 without clouds); that is the intended look and stays below white.

## 4. UI

- Option **Clouds** (switch, off by default), after *Atmosphere*; hover: "Clouds derived from
  the simulated climate (not simulated themselves): …".
- Legend: one sentence while on; a *Cloud* swatch.
- No new panel; phones keep the collapsed switcher.

## 5. Quality and cost

| Level | Octaves after ψ | Swirl | Relief | Shadows |
|---|---|---|---|---|
| Low | 2 | no | no | no |
| Medium | 4 | yes | yes | near the terminator, 3 octaves |
| High | 6 | yes | yes | near the terminator, 3 octaves |

Expected cost on CPU renderers: +15–35% of a frame on the disc (2–7 noise evaluations per
pixel against ~6–18 for the surface). Measured as in stages 2–4 (llvmpipe and SwiftShader,
dragging, Low/Medium/High, clouds on/off); skipped where the interpolated cover is 0.

## 6. Tests

- Unit (`modes/realistic.rs`): the RH inversion against the forward rain law; cover zero in a
  dry subtropical desert, high in a wet storm track and in the ITCZ; convective share high in
  the ITCZ, low in a storm track; stratocumulus only over a cold anomaly with E > P; the
  seasonal shift of the ITCZ cover (January south, July north) on a synthetic zonal climate;
  NaN fields give no clouds; the switch off gives zeros; the reflectance cap below the knee.
- naga: the shader still translates to GLSL ES 3.00.
- Visual (`clouds/`): full disc on/off, January and July, terminator, crescent, ITCZ and storm
  tracks; desktop, web, phone (390×844).

## 7. Limits (accepted)

- Decoration: the clouds are not simulated, have no feedback on the climate, and their pattern
  is a fixed noise; only their amount, type and placement follow the climate.
- Cells: cover changes between cells (~110/220 km); the cloud structure inside a cell is noise.
- No parallax: the clouds are drawn on the surface (12 km is under a pixel except at the
  extreme limb); no clouds stand out above the limb.
- Polar winter cover is higher than Earth's (the model's RH there is high).
