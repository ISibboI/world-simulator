# #92 step 2: stars

Sebastian (comment 7131174342): "Stars are probably not visible when the sun is in view, and
otherwise the planet will reflect too much light for the stars to be visible? But when the
planet is hiding the sun, then stars may be visible? Anyways, stars are a second step if they
make sense."

## Do stars make sense?

Yes, but only in a few views, and only with the sun's light. Your reasoning matches the physics:

- Per pixel, a sunlit surface is about 10⁷ times brighter than even the brightest stars. A
  sunlit landscape is ~10⁴ cd/m²; a magnitude-0 star gives ~2 × 10⁻⁶ lux, which spread over the
  solid angle of a pixel is 10⁶–10⁸ times fainter than the day side, depending on the pixel
  size. An exposure that shows the day side therefore records no stars. Photos of Earth from
  orbit and from the Moon show a black sky for this reason.
- The sun, or its glare, in or near the frame sets the exposure even more strongly.
- Stars show up in long exposures of a dark scene: the night side with the sun hidden behind
  the planet (an eclipse seen from space) or out of view. ISS night photos are an example.

So the view draws stars only with the **Sun** and **Sun + ambient** lights, and only as bright as
a camera's automatic exposure would let them be.

- **Camera light:** no stars. The camera light lights the whole visible side, so every view is a
  daylight exposure. The sun is not drawn either, as in step 1.
- **Sun + ambient:** stars as with **Sun**. The ambient fill is a visual aid, not light, and the
  meter ignores it. A real night side that bright would hide the stars too, so stars next to a
  readable night side are artistic, like the fill light itself.

It is automatic, not an option: the exposure decides. The hover text of the Light option
mentions the stars.

## The realistic mode (Sebastian on #148)

Sebastian asked to rename "the render mode from light to realistic" and to put it on top of the
list. He later clarified that the light is not an option for the data modes, only for the
realistic one, and that the realistic mode can show the topographic planet as a placeholder for
now. So this step changes the following:

- A new render mode **Realistic** (`--render-mode realistic`, `?render-mode=realistic`), first
  in the mode list. For now its surface is the topographic planet, with the exaggerated relief.
  The realistic surface (colours from the climate and crust, relief true to scale, detail)
  follows in later steps. Its options are **Light: Sun | Sun + ambient | Camera**, with Sun as
  the default, and a line about the sun of the shown time. The sun and the stars belong to this
  mode.
- **All data modes** are lit from the camera again, exactly as before #148, and have no Light
  option. Nothing called "Light" appears outside the realistic mode.
- **The default mode at start-up stays Topographic.** The list order only puts Realistic first.
  While it shows the topographic map with a dark night side, it is not yet the better first
  view; it can become the default once it has its realistic surface (a decision for Sebastian).
  On the web a mode chosen earlier is remembered as before.

## Exposure model (`src/viewer/sky.rs`, `Exposure::new`)

A camera with automatic exposure that sets the brightest ~0.1–1% of the frame to white
(highlight-weighted metering) behaves as follows:

1. **Sunlit share:** the share of the view covered by the sunlit planet, weighted by the cosine
   of the sun's incidence (the brightness of a Lambertian surface). It is sampled on a 200 × 150
   grid of rays against the unit sphere every frame (30 000 ray–sphere tests, well under a
   millisecond). The relief is ignored at the limb.
2. **Glare:** 1 while the visible sun is in the frame. It fades to 0 at 1.5 times the view's
   half-width beyond it, because lens flare and veiling glare reach past the frame edge.
3. **Metered value:** the larger of the two. The stars are fully visible below 0.1% (about 1000
   pixels of a megapixel frame) and gone above 1%, with a smoothstep in between. They fade
   smoothly while the terminator, a thin crescent or the sun comes into view.
4. **Sun behind the limb:** the share of the sun's disc that the planet hides is computed
   analytically (linear in the overlap of the two discs). It dims the sun's glow, so no glow
   remains around a hidden sun, and it removes the glare from the meter.

With the camera 4.5 radii from the planet, a 16:10 window, the sun over the equator and the
camera turning in the equatorial plane, measured from the direction opposite the sun:

| Camera angle from the anti-sun direction | What is in view | Stars |
|---|---|---|
| 0–12° | night side only, sun hidden behind the planet | full |
| 13–25° | sun beside the planet, in view | none |
| 28–38° | sun just outside the frame (glare) | none |
| 40° | thin crescent, 0.19% | 97% |
| 50° | crescent, 0.52% | 55% |
| 55° | 0.78% | 15% |
| ≥ 60° | 1.1% and more | none |

So stars show in a narrow window behind the planet (the eclipse view, as you expected) and in a
band 40–60° from it, where the sun is out of view and only a thin crescent is lit.

## Star field (`sky.wgsl`)

This is a procedural sky, not Earth's: no real constellations. It is deterministic, fixed in
space (the planet does not spin), and costs no frame data and only a few uniforms.

- **Cells:** a cube map with 180 × 180 cells per face, about 0.5° each. A hash (PCG) of the face
  and cell decides whether the cell holds a star, its place in the middle 60% of the cell, its
  magnitude and its colour. The probability is scaled by the cell's solid angle, so the density
  is uniform over the sky; corner cells are ~3× smaller than central ones. Each pixel checks its
  cell and the 8 neighbours on the same face, so stars are never cut at cell edges inside a face.
  At face edges the neighbours on the next face are not checked; a star there can lose part of
  its glow. That is rare and hardly visible.
- **Brightness distribution:** star counts grow by 10^0.45 ≈ 2.8 per magnitude, like the real
  sky's counts to magnitude ~8 (2.5–3 per magnitude). Magnitudes are sampled as `m = 7 +
  log10(u)/0.45` for u uniform in (0, 1). About 15 000 stars down to magnitude 7 cover the whole
  sky, about 600 of them in a 16:10 view. The naked eye sees ~9000 to magnitude 6.5; a long
  exposure records more.
- **Brightness on screen:** flux `10^(−0.4 (m − 1.5))`, so magnitude 1.5 fills a pixel with
  white. The point spread is a Gaussian of ~0.55 px, wider (up to 1.6 px) for stars that
  overflow, as in photos. It is gamma-encoded, so stars of magnitude 7 still show as dim dots,
  multiplied by the visibility from the exposure.
- **Colours:** black bodies (the same CIE fit as the sun, normalised to the brightest channel)
  at 3200 K (M, 8%), 4000 K (K giants, 22%), 4800 K (K, 17%), 5600 K (G, 12%), 6500 K (F,
  11%), 8000 K (A, 12%), 11 000 K (late B, 11%) and 20 000 K (early B, 7%). The weights follow
  the naked-eye stars, most of which are K giants and A–B stars because of their luminosity.
  The colours are computed on the CPU and passed as 8 uniform colours.
- **No twinkling:** there is no atmosphere between the camera and the stars.

**Cost:** for every sky pixel and only while stars are visible, 9 cell hashes plus a few
multiply-adds. The work is skipped entirely when the visibility is 0, by a branch on a uniform.
It is cheap on WebGL2: no textures and no storage buffers. The uniforms grow from 208 to 352
bytes.

## Screenshots (4 subdivisions, realistic mode, Sun light)

See [step2/](step2/):

- `desktop-mode-list.png`: the mode list with Realistic first, and the topographic mode lit
  from the camera without a Light option.
- `desktop-eclipse.png`: the eclipse view on the desktop.
- `desktop-crescent.png`: a crescent with fading stars on the desktop.
- `desktop-sun-in-view.png`: the sun in view, no stars, on the desktop.
- `web-eclipse.png`: the web version in chromium with SwiftShader.
- `phone-eclipse.png` and `phone-legend.png`: phone width (390×844); the Light row wraps.

## Not done

- No atmosphere glow at the limb: that is the atmosphere step.
- No Milky Way band or nebulae.
- Stars do not dim the planet's ambient fill.
- No exposure adaptation over time: the exposure follows the view at once.
