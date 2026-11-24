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
2. **Target exposure.** The scene asks for `target = 14 stops × darkness` above the daylight
   exposure (a stop is a factor of 2).
3. **Adaptation.** The view's exposure `E` (in stops, i.e. in log luminance) moves exponentially
   towards the target in real time:

   `E ← target + (E − target) · exp(−Δt / τ)`, with τ = **2 s** when the scene gets darker
   (dark adaptation, `E < target`) and τ = **0.3 s** when it gets brighter (light adaptation).

   This is the exact solution for a constant target, so the result does not depend on the frame
   rate. The target of a frame holds until the next frame: after a long idle pause the first
   frame of a changed view still starts from the old exposure, so the change is not skipped.
   When `E` is within 0.02 stops of the target it snaps to it, and the view stops requesting
   frames (egui `request_repaint` only while `E` moves). It keeps drawing while the simulation
   is paused and the camera is still, until it has converged.
4. **What the exposure does.**
   - **Planet:** the direct sunlight is multiplied by `2^E` (then clipped to white, as before).
     The ambient fill of **Sun + ambient** is not: it is a visual aid, not light, and is not
     metered either, so the readable night side keeps its brightness.
   - **Sun:** the disc by `2^E`, its glow by `min(2^E, 16)`: a dark-adapted view flashes when the
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

`E(t) = 14 e^(−t/0.3)`: the sunlit side is white for about half a second (`2^E` = 145 at 0.2 s,
6 at 0.5 s), normal after about 1 s (×1.4) and converged after ~2 s.

### Intermediate scenes

Steady states between the two follow the meter as before: a thin crescent (0.1–1% of the frame)
or the sun just outside the frame gives a partial exposure, e.g. darkness 0.5 → 7 stops →
limiting magnitude 3.7 (~20 stars), and the crescent itself is over-exposed (white), as in a
photo exposed for the sky.

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
invisible, and skips the remaining work of stars beyond the limit. While adapting, the view
draws every frame for up to ~13 s (dark) or ~2 s (light), then goes idle again.

## Screenshots (4 subdivisions, realistic mode)

In [eye/](eye/), taken on virtual screens (software rendering, a few frames per second; the
times are wall-clock seconds after the drag ended, which itself took ~1 s):

- `desktop-dark-adaptation.png`: the sky beside the planet after rotating from the day side to
  the eclipse view (sun behind the planet), at 0, 1, 2, 3, 4, 6 and 15 s: the brightest star
  first, then more and fainter ones.
- `desktop-eclipse-adapted.png`: the whole eclipse view, adapted.
- `desktop-light-adaptation.png`: from the adapted eclipse view back to the day side: adapted
  eclipse, then ~0.3, 0.6, 1, 1.4 and 2.3 s after the drag: over-exposed, settling within ~1 s.
- `desktop-ambient-and-sunrise.png`: **Sun + ambient** adapted in the eclipse view (the night
  side keeps its fill brightness, stars as with Sun), then the sunlit crescent coming into view
  (0, 0.5, 2 s): over-exposed, then normal.
- `web-dark-adaptation.png`: the same dark adaptation in the web version (chromium, SwiftShader
  WebGL2) at 0, 1, 3, 6 and 15 s.
- `phone-eclipse.png`: phone width (390 × 844), adapted eclipse view and with the legend open.
