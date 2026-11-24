# #159: the exposure over-exposed a thin sunlit crescent

The fix and its numbers are in [../issue-92/exposure.md](../issue-92/exposure.md), section
"Intermediate scenes: no lit surface over-exposes (#159)". In short, the exposure is capped at
`log₂(1/c_max)` stops above daylight, where `c_max` is the largest cosine of the sun's
incidence on the visible surface. It is computed exactly from the camera distance and the sun's
direction, so no lit point is shown brighter than under the zenith sun in daylight. Only the
sun, its glow and its glint on water may clip.

Screenshots: 5 subdivisions, 10 Myr, Realistic mode with the Sun light, taken after the view had
converged, with the camera turned about 40° from the eclipse view, as in Sebastian's phone
screenshot:

- `desktop-crescent.png`, `web-crescent.png` (chromium, SwiftShader), `phone-crescent.png`
  (390×844): the crescent keeps its colours (ocean, land, ice). The few white pixels are the
  sun's glint on water and ice.
- `desktop-eclipse.png`, `phone-eclipse.png`: with the sun behind the planet the view still
  adapts fully dark, and all stars appear.
