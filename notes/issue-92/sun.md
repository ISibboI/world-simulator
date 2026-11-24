# #92 step 1: the sun and the light options

Sebastian's decision (comment 7131174342): the sun is fixed in space by default, with a
camera-light option, and mimics the real sun, including its evolution: the fainter young sun when
the planet first had plates and oceans, today's sun, and a reasonable time into the future
before the red-giant phase. Step 1 adds the sun to the simulation and three light options to the
view: Camera, Sun, Sun + ambient.

## Sun model (`src/sun.rs`)

`Sun::at_age(age)` gives the state of a solar-mass star of solar composition for an age (Gyr
since the zero-age main sequence, ZAMS) from 0 to 10 Gyr; ages outside are clamped.
`Sun::of_run(epoch, time)` is the sun of a run started `epoch` Gyr before present (`--epoch`),
`time` Myr into the run: age = 4.57 − epoch + time/1000.

| Quantity | Model | Source |
|---|---|---|
| Luminosity | `L/L₀ = 1 / (1 + 0.4 (1 − t/t₀))`, t₀ = 4.57 Gyr | Gough (1981), the formula the climate already uses for `--epoch` |
| Effective temperature | linear in age through 5596 K (ZAMS), 5772 K (today), 5820 K (7.13 Gyr, hottest), 5745 K (10 Gyr) | Schröder & Smith (2008) solar model; today's value IAU 2015 B3 (their 5774 K) |
| Radius | Stefan–Boltzmann: `R/R₀ = √(L/L₀) (T₀/T)²` | — |
| Colour | black body at T_eff, CIE 1931 2° colour matching (Wyman, Sloan & Shirley 2013 multi-lobe fit), XYZ → linear sRGB (D65), luminance 1 | — |
| Apparent size | `asin(R / d)` with R₀ = 6.957 × 10⁸ m (IAU), d = 1 AU × orbital distance | — |
| Direction | over a fixed longitude (45° west of the default camera), declination 0 (annual view); `sun::direction` gives the declination and distance of any time of the year from `OrbitParams` | `temperature::OrbitParams::position` |

The planet's orbit is taken to be 1 AU (the distance at which today's Sun gives the default
solar constant of 1361 W/m²), so the irradiance is `1361 W/m² × L/L₀`.

### Values

| Age (Gyr) | Corresponds to | L/L₀ | T_eff (K) | R/R₀ | Disc radius | Light colour (linear sRGB, Y = 1) |
|---|---|---|---|---|---|---|
| 0 | ZAMS | 0.714 | 5596 | 0.90 | 0.240° | (1.13, 0.97, 0.88) |
| 0.57 | `--epoch 4.0`, first oceans and plates | 0.741 | 5618 | 0.91 | 0.242° | (1.13, 0.97, 0.88) |
| 1.07 | `--epoch 3.5`, Archean | 0.765 | 5637 | 0.92 | 0.244° | (1.13, 0.97, 0.89) |
| 4.57 | today (`--epoch 0`, default) | 1.000 | 5772 | 1.00 | 0.266° | (1.11, 0.98, 0.91) |
| 7.13 | +2.6 Gyr, hottest | 1.289 | 5820 | 1.12 | 0.298° | (1.11, 0.98, 0.92) |
| 10 | +5.4 Gyr, end of the model | 1.906 | 5745 | 1.39 | 0.371° | (1.11, 0.98, 0.91) |

Checks against detailed solar models: Bahcall, Pinsonneault & Basu (2001) and Schröder & Smith
(2008) give 0.70 L₀ / 0.89 R₀ at the ZAMS, 1.26 L₀ / 1.11 R₀ at 7.13 Gyr and 1.84 L₀ / 1.37 R₀
at 10 Gyr. Gough's formula is within 2% before today and 2–4% above the models after 7 Gyr;
the radius within 1–2%. Unit tests check these values, monotonic growth, the clamping and the
black-body colours (6504 K ≈ white, 3000 K red).

**Range.** The Sun leaves the main sequence at 10–11 Gyr; the subgiant and red-giant phases
need other fits (luminosity and radius rise steeply, T_eff falls). 10 Gyr (5.4 Gyr from today)
is where the model ends; later ages show the 10 Gyr sun. `--epoch` itself still accepts only
0–4.57 Ga (past epochs); a future time frame needs a new option (e.g. negative epochs), which
the sun model already supports.

### The simulation's results are unchanged

(Since #149 the climate follows the brightening sun during a run; see `notes/issue-149/`.)

`presets::Epoch::solar_constant` now calls `sun::solar_constant`, the same expression as before
(bit for bit; the presets test checks the default and epoch parameters). The climate keeps the
solar constant of its epoch for the whole run: the sun model is not yet used by the climate.
Within a run of 1 Gyr the sun brightens by ~8–10% (about 1% per 100 Myr), which the view shows
and the climate does not. Making the climate follow `Sun::of_run` every step would change
simulation results (warming of a few K per Gyr without a compensating greenhouse or weathering
feedback) and is a separate decision (see the PR).

## Light options in the view

> **Changed in step 2** (Sebastian on #148): the Light option now belongs only to the new
> **Realistic** render mode (first in the list; Sun is its default). The data modes are always
> lit from the camera again, as before #148. See [stars.md](stars.md). The text below describes
> step 1.

Where: one **Light** setting of the whole view, in the mode switcher under the options of the
active mode, shown for every mode that shades its colours (topographic, plate motion, plates;
the unlit modes show their colours as in their legends and ignore it). One global setting rather
than one per mode, because it is the same choice for every shaded mode and stays when the mode
changes; the realistic mode (step 2) will use it as well. Under the choices a small line
describes the sun of the displayed time, e.g. "Sun: 4.57 Gyr old, 100% of today's brightness,
5772 K".

| Choice | Light | Ambient |
|---|---|---|
| Camera (default) | from the upper left of the camera, white — exactly the lighting before this change | 30% |
| Sun | from the sun, fixed in space, in the sun's colour | 0: the night side is black |
| Sun + ambient | from the sun, in the sun's colour | 20%: the night side stays readable |

Camera stays the default for the existing modes, since they show data that should be readable
everywhere; the realistic mode will start with Sun (decision for Sebastian, see the PR).

- **Shading:** `light = ambient + (1 − ambient) · max(n·l, 0) · colour`, multiplied onto the sRGB
  colours of the mode as before (the realistic mode will light in linear space).
- **Exposure follows the sun:** the light has the luminance of white at any age, so the planet
  under the young sun is not darker, only lit by a slightly warmer light (a camera that adapts
  its exposure). The colour difference between the young and today's sun is small (T_eff 5600
  vs 5772 K).
- **Ambient light** is artistic. A real night side gets only moonlight, airglow and starlight
  (~10⁻⁶–10⁻⁹ of daylight).
- **The sun in the background** (`sky.wgsl`): drawn with the Sun and Sun + ambient light (not with the camera light, where a visible sun
  would contradict the lighting, nor in the unlit modes), a
  full-screen triangle just in front of the far plane, depth-tested against the planet, added to
  the background: a disc of the true angular size (~0.27°, a radius of about 6 px in an 800 px high view),
  limb darkening (linear law, u = 0.6), a halo falling off as (r/θ)² and a faint ~3° glow. With
  the camera 4.5 radii from the planet (which covers 12.8° around the view direction), the sun is
  in view only when it is nearly behind the planet: between 12.8° and the edge of the view
  (17° vertically, ~26° horizontally in a 16:10 window) from the view direction. Drag the planet
  until the night side faces the camera; the sun then appears beside the planet.
- **Frame data:** none. The view computes the sun from the run's `--epoch` (from its launch
  options) and the simulated time of the displayed frame (live or from the
  history), so the frame format and the codec are unchanged (frame growth 0 bytes).

## Screenshots (4 subdivisions, topographic mode)

- [desktop, Sun](step1/desktop-sun.png): default camera, sun 45° to the left, terminator on the
  right, black night side.
- [desktop, Sun + ambient, `--epoch 3.5`, night side with the sun beside the planet](step1/desktop-sun-ambient-behind.png):
  the sun of 1.09 Gyr (77%, 5638 K), the night side readable, the thin sunlit crescent on the left limb.
- [web (chromium, SwiftShader), Sun, sun beside the planet](step1/web-sun-behind.png): the
  same on the web; the faint wide glow shows 8-bit banding.
- [web at phone size (390×844), Sun + ambient](step1/phone-sun-ambient.png): the light row
  wraps; the legend panel starts closed on narrow screens as before.

## Not in this step

Season (the sun's declination for January/July) is in the sun model (`sun::direction`), but the
view uses the annual position (declination 0) until the realistic mode adds its Season option;
none of the lit modes has a season option. Stars (see below), the atmosphere, the realistic
surface and level of detail follow in later steps.

**Stars:** in real photographs of a sunlit planet (or with the sun in view) the exposure is far
too short for stars; they appear only when the planet hides the sun and the sunlit side is out of
view. They are a later step if they make sense (e.g. only with the Sun light and the camera on
the night side).

## References

- Bahcall, J. N., Pinsonneault, M. H., Basu, S. (2001): Solar models: current epoch and time
  dependences, neutrinos, and helioseismological properties. ApJ 555, 990.
- Gough, D. O. (1981): Solar interior structure and luminosity variations. Sol. Phys. 74, 21.
- Mamajek, E. E. et al. (2015): IAU 2015 Resolution B3 on nominal solar values.
- Schröder, K.-P., Smith, R. C. (2008): Distant future of the Sun and Earth revisited. MNRAS 386,
  155.
- Wyman, C., Sloan, P.-P., Shirley, P. (2013): Simple analytic approximations to the CIE XYZ
  color matching functions. JCGT 2(2).
