# #92 stage 3: the atmosphere

The Realistic mode now has an atmosphere, true to scale: single scattering of sunlight in an
exponential atmosphere of Earth-like composition. It replaces the fake limb haze of stage 1.

- **On the disc:** the air between the ground and the eye dims the surface and adds its own
  light (aerial perspective): a faint blue veil over the whole disc, a pale haze towards the
  limb. The sunlight reaching the ground loses its blue on the long slant path near the
  terminator, so the terminator is reddened.
- **Off the disc:** a thin blue glow along the limb of the day side. With the sun behind the
  planet, there is a ring of sunlight scattered forward by air and aerosols: blue outside, cream
  inside, with a thin orange inner edge where the light grazed the lower atmosphere.
- **Option:** *Atmosphere* (on by default) in the mode's options switches it off, with any
  light. Its sample count follows the *Detail* level.

Code: `src/viewer/atmosphere.wgsl` (shared by `realistic.wgsl` and `sky.wgsl`, prepended with
`concat!` because WGSL has no includes), `src/viewer/atmosphere.rs` (constants and the same model
in f64 for the exposure meter and the tests), `sky.rs` (exposure cap), `modes/realistic.rs`
(option). No new frame data, no textures, no new pipeline. The data modes are unchanged: the
atmosphere is off there, and the sky pass is not drawn.

## Model

Lengths in planet radii; the planet has Earth's radius (6371 km), so the atmosphere is very
thin. One scale height of the air (8 km) is a third of a pixel at the view's distance on the
desktop, a fifth on a phone.

| | Air (Rayleigh) | Aerosols (Mie) |
|---|---|---|
| Scale height | 8 km | 1.2 km |
| Vertical optical depth (680 / 550 / 440 nm) | 0.046 / 0.108 / 0.265 (Bruneton & Neyret 2008) | 0.040 / 0.050 / 0.063 (Ångström exponent 1) |
| Phase function | Rayleigh | Cornette–Shanks, g = 0.76 |
| Absorption | none | single-scattering albedo 0.9 |

- **Optical depths** to space come from the Chapman function. For each species it is
  `sqrt(πx/2) · erfcx(μ sqrt(x/2))`, with erfcx approximated as
  `2 / (√π (y + sqrt(y² + 4/π)))`. A ray below the horizon (μ < 0) passes a lower tangent point,
  so its column is twice the horizontal one from there minus the one above the point. The tests
  check this against numerical integration: within 7% for heights 0–30 km and μ from −0.04 to
  1. The exponents are clamped, so f32 cannot overflow.
- **Sunlight** at a point is the transmittance towards the sun times the share of the sun's
  disc above the point's horizon, from the sun's elevation (`asin μ ≈ μ`) and the horizon's dip
  (`acos(1/(1+h)) ≈ sqrt(2h)`), both within 1% where it matters. This gives a soft shadow edge
  over the solar disc and no light below the horizon.
- **Units:** the shader's sunlight makes a white surface under the zenith sun show the sunlight
  itself, so scattered light shows as `sunlight · π · ∫ β ρ P T_view T_sun ds`.
- **On the disc** (`surface_air`): the view's transmittance is closed form. Where the
  eye looks down steeply (μ > 0.2), so is the scattered light: the sunlight's transmittance is
  taken as constant along the short path, one scale height above the ground. Near the limb
  (μ < 0.12) the path is 300–600 km long and crosses a few degrees of the sun's elevation, the
  terminator's width. There the light is integrated numerically like the rays that miss the
  planet, so the disc meets the limb glow without a seam. In between, the two are blended.
  - The sunlight on the ground is relative to its value under the zenith sun. The multiple
    scattering this model leaves out (the sky's own light) makes up for most of the blue the
    direct sunlight loses by day, and an eye adapts its white to daylight. So noon keeps the
    surface colours of stage 1, and only towards the terminator does the light turn red and
    fade.
- **Off the disc** (`limb_light`, in the sky pass, which runs exactly on the pixels the planet
  does not cover): 8 / 12 / 16 points (Low / Medium / High) along the ray. They are spaced
  `s = L u|u|` around the tangent point, where the air is densest. The result is averaged over
  2 / 3 / 3 heights across the part of the pixel above the surface, because a pixel spans ~2.5
  scale heights and a single sample would show the glow as a staircase along the circle. With 12 points the sum is within
  15% of a converged integral (tested). Rays the mesh does not cover but that pass below the
  sphere (the triangles' chords sag inside it) count as grazing the surface. The top of the
  atmosphere follows the exposure: 10 scale heights (80 km) in daylight, one more per factor e
  of gain, at most 20 (160 km) dark-adapted, so that the air above adds less than 1/1000 of
  white. In daylight this halves the band of pixels the limb costs.
- **The flat triangles sag** below the true surface between the cell centres: ~1 km at 5
  subdivisions, ~4 km at 4. That is one to three scale heights of the aerosols. At grazing views it showed the mesh as
  a lattice of dots in the haze. The shader therefore measures the air from the interpolated
  radius of the triangle's corners, passed in the w component of the position.
- **Behind the atmosphere:** stars and the sun's disc are dimmed and reddened by the
  transmittance of their pixel's ray, the sun's glow by that of the ray to the sun. The sun sets
  red behind the limb.

Not modelled (limits): multiple scattering (the limb and the twilight are somewhat darker than
real, and the sky's light on the ground is folded into the white balance above), ozone (whose
absorption makes the twilight bluer), refraction (which bends red light into the shadow during
an eclipse), and clouds (a later stage).

**Earth-like, not Earth.** The simulation has a constant surface pressure of 1 atm
(`climate::SURFACE_PRESSURE`). The epoch and CO₂ settings change only the greenhouse forcing,
and nothing in the simulation describes the air's composition or aerosols. The coefficients are
therefore constants for an Earth-like atmosphere with a clean, natural aerosol load (optical
depth 0.05 at 550 nm, about pre-industrial over the ocean). If pressure ever becomes a
parameter, Rayleigh and the scale height scale with it.

## Exposure: nothing but the sun clips

The atmosphere is lit by the same sunlight and exposure as the surface (`sky::Adaptation`).
#159's rule applies: only the sun, its glow and its glint may clip.

- **A hue-preserving shoulder** (`shoulder`) handles anything brighter than the brightest
  reflectance (0.86, fresh snow). It scales the colour so that its largest channel approaches 1
  logarithmically: 3 stops above the knee reach half of the remaining range. Light many stops
  too bright for the exposure keeps its hue and some gradation, and never turns white. It acts
  on the surface with its haze, not on the glint, which may still clip. Lit surfaces stay below
  the knee by the #159 cap, except that the ambient fill of *Sun + ambient* and *Camera* can
  lift fresh snow a little above it (compressed by a few per cent). With the atmosphere off,
  the shader is as before (no shoulder).
- **With a lit surface in view**, the exposure is also capped by the brightest point of the
  limb (`Exposure::headroom`). The meter evaluates `atmosphere::Model` at 48 points around the
  limb and 5 heights, 0.3 ms per frame. It leaves out the limb within 10° of the sun, which may
  clip. Without this cap, a crescent asks for up to 2–4 stops more than daylight (#159). The
  forward-scattering haze over it then sat in the shoulder as a flat cream band that hid the
  crescent. With the cap, the limb's brightest point shows at most at the knee. At #159's
  angle (40° from the eclipse) the target is 0.93 stops instead of 1.13. The meter is
  conservative: the shader averages over a pixel's heights, which lowers the peak.
- **In an eclipse** (only the night side in view) there is no cap. The view adapts fully dark
  (14 stops) as before, all the stars appear, and the ring sits in the shoulder: blue outside,
  then cream, with a thin orange inner edge, 4–6 pixels wide on the desktop. The orange layer
  (the light that grazed the lowest ~5 km) is thinner than a pixel (~20 km), so it mixes with
  the brighter layers above it. A real eclipse ring is redder: refraction bends the reddened
  light of the lower atmosphere into the shadow, which this model leaves out. The ring is not metered,
  because it covers well under 1% of the frame and the eye adapts to the mean.
- **With the sun just behind the limb**, the side of the ring towards the sun, together with the
  sun's glow, reaches white. That is the allowed exception "near the sun".
- With the **camera light** the gain is 1, and the limb shows the back-scattered blue line. The
  sun and the stars stay off (the sky pass draws only the atmosphere then).

## Frame times

Median frame time while dragging, 5 subdivisions, 1280×800, Realistic with the Sun light,
default view (day side and limb in view). Both renderers run on the CPU, so these are worst
cases and only the ratios matter. On real GPUs, phones included, this is far below a frame.

| | Low | Medium | High |
|---|---|---|---|
| Desktop, llvmpipe (2 threads, MSAA 4×), before | 83 ms | 104 ms | 104 ms |
| Desktop, after | 100 ms (+20%) | 124 ms (+19%) | 127 ms (+22%) |
| Desktop, after, Atmosphere off | 84 ms | 106 ms | 106 ms |
| Web, chromium with SwiftShader (WebGL2), before | 304 ms | 390 ms | 395 ms |
| Web, after | 405 ms (+33%) | 486 ms (+25%) | 485 ms (+23%) |

- About half of the cost is on the disc (a few Chapman columns, exponentials and phase
  functions per pixel), half in the limb band of the sky pass (8–16 points × 2–3 heights per
  pixel, in a band ~4–8 pixels wide).
- The first version cost +40%. Three changes brought it down:
  - columns towards the sun shared between the ground light and the haze;
  - the night side skipped;
  - the sky pass's powers limited to the pixels the atmosphere touches, and the top of the
    atmosphere made to follow the exposure.
- The CPU meter of the brightest limb takes 0.3 ms per frame while the view adapts or moves.
- *Atmosphere: Off* restores the old cost.

## Checks

- Unit tests (`atmosphere.rs`, `sky.rs`):
  - the shader's constants equal the physical inputs;
  - vertical optical depths;
  - the Chapman approximation against numerical integration;
  - sunlight reddens towards the horizon and vanishes below it;
  - the sampled limb converges;
  - the day-side limb is pale low down, blue higher up, and below the knee;
  - aerosols scatter forward;
  - the eclipse ring is red below and blue above and keeps both through the shoulder at 14
    stops;
  - the shoulder never reaches white and keeps dark colours;
  - the brightest limb;
  - the limb cap at the crescent angles of #159, while the eclipse still adapts fully.
- The naga test translates the concatenated shaders to GLSL ES 3.00 (WebGL2).
- Visual: desktop (llvmpipe), web (chromium with SwiftShader WebGL2) and phone width
  (390 × 844).

## Screenshots ([atmosphere/](atmosphere/))

5 subdivisions, 10 Myr, seed 0, Realistic, Sun, Medium:

- `desktop-full.png`: the default view. A faint blue veil over the disc, haze towards the limb,
  a thin blue line along the day-side limb.
- `desktop-terminator.png`: the terminator in the middle, with the sunlight reddening and
  fading towards it.
- `desktop-crescent.png`: #159's angle (~40° from the eclipse). The crescent is dominated by
  the forward-scattering haze, below the shoulder's knee thanks to the limb cap.
- `desktop-eclipse.png`: the sun behind the planet, dark-adapted. Stars, and the ring: blue
  outside, cream, a thin orange inner edge.
- `desktop-sunrise.png`: the sun just behind the left limb. The sunward arc of the ring clips,
  together with the sun's glow (the allowed exception).
- `desktop-closeups.png`: 3× crops of the terminator, the crescent's limb and the eclipse ring.
- `web-full.png`, `web-crescent.png`, `web-eclipse.png`: the web version (chromium with
  SwiftShader WebGL2). The eclipse is taken with the sun just behind the left limb.
- `phone.png`: phone width (390 × 844). The options with the new *Atmosphere* switch, and the
  eclipse ring after dark adaptation.
