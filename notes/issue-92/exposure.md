# #92: eye adaptation (exposure over time)

Sebastian (comment 7135503710): "simulate the eye adjusting to exposure. The adjustment time
should be a few seconds for underexposure, and shorter for overexposure. This way, there should
be more stars visible when hiding the sun behind the planet with the camera."

Before this step the exposure of the realistic mode followed the view at once
([stars.md](stars.md)): the stars were faded in or out by one visibility factor, and the planet
and the sun were always shown at the daylight exposure. Now the view has an exposure that
**adapts in wall-clock time**, like an eye or a camera's auto-exposure, and that one exposure
sets the planet, the sun with its glow, and the stars.

## Model (`src/viewer/sky.rs`, `Adaptation`)

1. **Meter (unchanged).** Every frame the view measures the share of the frame covered by the
   sunlit planet (cosine-weighted, 96 × 64 rays) and the glare of the sun in or near the frame,
   as before. This gives a darkness from 0 (daylight) to 1 (a dark scene), the former star
   visibility: it falls from 1 to 0 while the sunlit share grows from 0.1% to 1% of the frame,
   and with the glare of the sun.
2. **Target exposure.** The scene asks for `14 stops × darkness` above the daylight
   exposure (a stop is a factor of 2), **capped so that no lit surface over-exposes** (#159,
   below): `target = min(14 × darkness, log₂(1 / c_max))`, where `c_max` is the largest cosine
   of the sun's incidence on the visible surface.
3. **Adaptation.** The view's exposure `E` (in stops, i.e. in log luminance) moves exponentially
   towards the target in real time:

   `E ← target + (E − target) · exp(−Δt / τ)`, with τ = **2 s** when the scene gets darker
   (dark adaptation, `E < target`) and τ = **0.5 s** when it gets brighter (light adaptation).

   This is the exact solution for a constant target, so the result does not depend on the frame
   rate. The target of a frame holds until the next frame: after a long idle pause the first
   frame of a changed view still starts from the old exposure, so the change is not skipped.
   When `E` is within 0.02 stops of the target it snaps to it, and the view stops requesting
   frames (egui `request_repaint` only while `E` moves). It keeps drawing while the simulation
   is paused and the camera is still, until it has converged.
4. **What the exposure does.**
   - **Planet:** the direct sunlight is multiplied by `2^E` in linear light (then clipped to
     white, as before). The view's colours are sRGB-encoded, so the factor on them is
     `2^(E / 2.2)` (up to ×82 dark-adapted); the stars' magnitudes are linear flux and are
     encoded in the shader, so one stop brightens both alike.
     The ambient fill of **Sun + ambient** is not: it is a visual aid, not light, and is not
     metered either, so the readable night side keeps its brightness.
   - **Sun:** the disc by the same factor, its glow by at most 16 (on sRGB values): a dark-adapted view flashes when the
     sun appears, without the glare filling the screen.
   - **Stars:** they appear `(14 − E) × 0.753` magnitudes fainter than to a fully dark-adapted
     view (0.753 = 2.5 log₁₀ 2 magnitudes per stop). On screen a star fades out between apparent
     magnitude 8 and 9 (17 and 0 of 255), so the **limiting magnitude** of the field is
     `9 − 0.753 (14 − E)`: −1.5 in daylight (no star; the brightest is −1.5), 9 dark-adapted
     (every star of the field, which goes down to magnitude 7, the faintest as dim dots).
     The brightest stars therefore appear first and the fainter ones follow as the view adapts.

**Why 14 stops.** The real gap between a sunlit day side and the stars is ~23 stops (10⁷,
see [stars.md](stars.md)), and dark adaptation of the real eye takes 20–30 minutes. Both are
compressed for the view: 14 stops is just enough for every star to vanish at the daylight
exposure, and the time constants follow Sebastian's "a few seconds" and "shorter". The model is
the usual one of games and tone-mapping papers (exponential adaptation of the log luminance with
separate time constants for the two directions, e.g. Pattanaik et al. 2000; Krawczyk et al.
2005).

### After hiding the sun behind the planet (from daylight)

`E(t) = 14 (1 − e^(−t/2))`. The limiting magnitude is where stars vanish; stars are clearly
visible (≥ ~20 of 255) about 1.5 magnitudes brighter than that. Stars in a 16:10 view, from the
~600 of the field in such a view, counted to that clear-visibility magnitude:

| Time | E (stops) | Limiting magnitude | Clearly visible to | Stars in view |
|---|---|---|---|---|
| 0 s | 0 | −1.5 | −3 | 0 |
| 1 s | 5.5 | 2.6 | 1.1 | ~1 |
| 2 s | 8.8 | 5.1 | 3.6 | ~20 |
| 3 s | 10.9 | 6.6 | 5.1 | ~90 |
| 4 s | 12.1 | 7.6 | 6.1 | ~230 |
| 6 s | 13.3 | 8.5 | 7.0 | all (~600), the faintest still dim |
| ~13 s | 14 | 9.0 | 7.5 | converged, frames stop |

### When the sunlit side comes into view (from dark-adapted)

`E(t) = 14 e^(−t/0.5)`: the sunlit side is over-exposed for about a second (factor on the
sRGB colours ×14.5 at 0.25 s, ×5 at 0.5 s, ×1.8 at 1 s), nearly normal at 1.5 s (×1.25) and
converged after ~3.3 s. (With τ = 0.3 s and the sRGB-correct factor the flash was hardly visible
in the screenshots, so τ is at the upper end of 0.2–0.5 s.)

### Intermediate scenes: no lit surface over-exposes (#159)

Before #159 a thin crescent (0.1–1% of the frame) asked for a partial or nearly full dark
exposure, because the meter only saw the share of the frame. The crescent itself, in full
sunlight, was then over-exposed to white. Sebastian saw this on his phone with the camera nearly
behind the planet. His requirement: "Exposure should be aimed such that no part of the picture is
overexposed. The sun can aim for some overexposure, because it is so bright that the eye cannot
adjust to it in real life."

Now the exposure is also capped by the **brightest lit point in view**.

- A surface of reflectance `a`, lit at the incidence cosine `c`, shows as `a · c · 2^E` (linear,
  before clipping). It stays at most `a`, its look under the zenith sun in daylight, for
  `E ≤ log₂(1/c)`. With `c_max` the largest `c` in view, the cap is `log₂(1/c_max)`. No
  reflectance exceeds 0.86 (fresh snow), so nothing reaches white.
- `c_max` is **exact and does not depend on the crescent's size in pixels**. The visible cap of
  the planet around the eye direction has the angular radius `α = acos(1/d)` (`d` the camera
  distance in radii: 77.2° at 4.5). The lit point nearest to the subsolar point is
  `max(θ − α, 0)` from it, `θ` the angle between the sun and the eye direction, so
  `c_max = cos(max(θ − α, 0))`, or 0 when that is beyond 90°.
- The share meter still darkens the scene for stars and glare as before. The cap only lowers
  the target.
- Two parts of the shader would escape the cap, so it handles them itself:
  - the **bump relief** tilts normals beyond `c_max` towards the sun, so the shader caps the
    incidence at `2^−E`. A reflectance therefore never shows brighter than under the zenith sun
    in daylight. At the daylight exposure this cap is 1 and changes nothing;
  - the **limb haze** also lights the twilight band, which `c_max` does not see. It keeps its
    daylight brightness at any exposure, so the blue arc at the limb of an eclipse view (sun
    just hidden) stays blue instead of clipping.
- What may still clip: the sun's disc and glow, and its mirror image, the glint on water, which
  is the sun seen in the water.
- The ambient fill of **Sun + ambient** is not brightened by the exposure, so the readable
  night side cannot drive anything to white either.

Camera turning in the equatorial plane from the eclipse view (sun behind the planet) towards
the day side, camera at 4.5 radii, 16:10 view (from the unit tests):

| Angle from the eclipse | `c_max` | Target before | Target now |
|---|---|---|---|
| 0–12° | 0 (night side only) | 14 | 14 (all stars) |
| 13° | 0.003 (sun at the limb) | 2.8 | 2.8 |
| 20–25° | 0.13–0.21 (sun beside the planet) | 0 (glare) | 0 |
| 30° | 0.30 | 3.6 | 1.8 |
| 35° | 0.38 | 12.9 | 1.4 |
| 40° (the report's view) | 0.46 | 13.6 (crescent white) | 1.1 |
| 50° | 0.60 | 7.7 | 0.7 |
| ≥ 60° | ≥ 0.73 | 0 | 0 |

So the crescent keeps its colours: ocean, land and ice at their daylight look. It appears up to
log₂(1/c_max) stops brighter than in daylight, so a crescent lit at a grazing angle is not
almost black. A few bright stars still show beside a thin crescent (limiting magnitude
9 − 0.753 (14 − E): about −0.2 at 1.8 stops). Only a few grazing degrees, where `c_max` is close
to 0, let the exposure rise further, and there the lit sliver is equally dark.

## Light options

- **Sun:** as above.
- **Sun + ambient:** the same exposure; only the ambient fill is not brightened (see 4).
- **Camera:** no adaptation. The camera light lights the whole visible side, so every view is a
  daylight exposure: no stars and no sun, as before. The exposure is reset to daylight, so
  switching to a sun light (or into the realistic mode from a data mode, which are lit from the
  camera) starts light-adapted and the stars fade in over seconds. The data modes are never
  affected.

## Cost

One more `exp` per frame on the CPU; no new frame data, no textures, WebGL2 as before. The
uniform `stars` now carries the star dimming (x), the pixel size (y) and the sun's gain (z)
instead of a visibility. The star shader skips its loop while even the brightest star is
invisible (with 0.1 magnitudes of margin, so a faint glare tail does not run it for nothing),
and skips the remaining work of stars beyond the limit. While adapting, the view
draws every frame for up to ~13 s (dark) or ~3.3 s (light), then goes idle again.

## Screenshots (4 subdivisions, realistic mode)

In [eye/](eye/), taken on virtual screens (software rendering, a few frames per second; the
times are wall-clock seconds after the drag ended, which itself took ~1 s):

- `desktop-dark-adaptation.png`: the sky beside the planet after rotating from the day side to
  the eclipse view (sun behind the planet), at 0, 1, 2, 3, 4, 6 and 15 s: the brightest star
  first, then more and fainter ones.
- `desktop-eclipse-adapted.png`: the whole eclipse view, adapted.
- `desktop-light-adaptation.png`: from the adapted eclipse view back to the day side: adapted
  eclipse, then ~0.3, 0.7, 1, 1.4 and 2.3 s after the drag: over-exposed, then settling.
- `desktop-ambient-eclipse.png`: **Sun + ambient**, adapted in the eclipse view: the night side
  keeps its fill brightness, the stars are as with Sun.
- `desktop-sunrise.png`: from the adapted eclipse view, the sunlit crescent coming into view
  (~0.1, 0.7, 1.2 and 3.1 s after the drag): over-exposed, then normal.
- `web-dark-adaptation.png`: the same dark adaptation in the web version (chromium, SwiftShader
  WebGL2) at 0, 1, 3, 6 and 15 s.
- `phone-eclipse.png`: phone width (390 × 844), adapted eclipse view and with the legend open.
