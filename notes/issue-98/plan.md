# #98 Crust-age render mode: plan

## Data
Nothing new in the frame: `CellInfo::age` (lithosphere age, 0.25 Myr in the frame history),
`CellInfo::since_orogeny`, the `CONTINENTAL` flag and `plate` are already in every frame (#115),
so past frames work without a codec change. The cell widget already has the rows
"Lithosphere age" (with "(ocean floor)") and "Orogeny"; no new row.

## Mode `crust-age` (`src/viewer/modes/crust_age.rs`)
- **Ocean floor** (oceanic crust, age < 1000 Myr): continuous scale 0–200 Myr, clamped above.
  Colours: Crameri's *batlow*, reversed (young = light pink/orange, old = dark blue). Age maps
  conventionally use a rainbow from young = red to old = blue; batlow keeps that look (warm and
  light young, cool and dark old) but is perceptually uniform, has monotonic lightness (readable
  in greyscale) and is colour-vision-deficiency friendly. Crameri et al. (2020, *Nat. Commun.*
  11:5444, "The misuse of colour in science communication") use exactly the sea-floor age map
  as their example of replacing the rainbow with batlow. 11 stops every 20 Myr sampled from the
  256-entry table, linear in between.
- Oceanic crust with a nominal age ≥ 1000 Myr (crust given "older than the simulation" by the
  model) gets its own swatch colour (shown in the legend only when present).
- **Continental crust**, option "Continents": *Grey* (default, one neutral grey), *Age*
  (formation age in discrete grey classes, lighter = younger, darkest = from before the start),
  *Last orogeny* (time since the last orogeny in grey classes, darkest = none). Discrete greys
  as legend swatches, so that the one colour bar stays the ocean-floor scale and continents are
  never confused with sea floor.
- **Isochrons** option: every 20 Myr (default), every 50 Myr, off. Drawn with
  `overlay::isolines` over the faces whose three cells are oceanic, on the same plate and
  whose ages differ by at most 50 Myr, so that age jumps at plate boundaries (trenches,
  transforms, sutures) do not draw bundles of lines.
- **Coastlines** switch (default on), as in the other unlit modes (`modes::coastlines`);
  the colour edge between continent and ocean floor is the continent–ocean boundary, which
  differs from the coastline at flooded shelves.
- Unlit shading so the colours match the legend. Legend text includes the oceanic mean and
  maximum age and the oceanic share of the surface of the shown frame.
- Unit tests: scale monotonic in lightness and endpoints, option parsing, face filter,
  continental classes, a small planet through `colors` and `overlay`.

## Plausibility
The simulation already logs ocean-age statistics per snapshot (mean, median/90%/max, area per
20 Myr). Four default seeds (0–3, 500 Myr, 6 subdivisions) via `runs`; compare with Earth
(max ~180–200 Myr, mean ~60–65 Myr, area roughly linearly declining with age, Parsons 1982 /
Cogné & Humler / Seton et al. 2020). Weaknesses go to an existing or new issue, not fixed here.

## Plan review (independent agent) and what changed
1. "Oceanic" must not mean "not continental": the model has transitional crust (thick arcs,
   thinned margins) whose age is a thermal age. → New flag bit `CellInfo::OCEANIC` (bit 6, kept
   by the frame history's 8 flag bits, no codec change), from `Crust::is_oceanic` as in the
   statistics log. Transitional crust gets its own colour; the cell widget says
   "continental / oceanic / transitional".
2. The continental "formation age" has no meaningful data: `birth` of continental crust is a
   thermal age (nominal −1000 Myr, reset by rifting). → Option dropped; continents are grey or
   graded by the time since the last orogeny.
3. Old crust threshold 999.5 Myr (rounding); NaN age → "No data" colour; unit tests for both.
4. Isochron face filter kept; on the coarse grid 20 Myr isochrons at slow spreading are one or
   two cells apart (accepted).
5. Continental greys kept in the middle lightness range (115–215), away from the navy old end.
6. UI: options use the shared switcher (touch-sized buttons, collapsible panel).
