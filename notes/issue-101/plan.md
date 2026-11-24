# Issue #101: lithosphere-strength render mode — plan

## What the model has
`weak_zones::lithosphere_strength(crust, time)` = max(thermal × thinning × orogenic, 0.05):
- **thermal** = √(min(strength_age / 100 Myr, 1)), strength_age = time − max(birth, reheated):
  young sea floor, young arcs, rifted continental crust (birth reset by stretching), and
  lithosphere a plume heated (`reheated`, carried with the plate).
- **thinning** = min(basement thickness / 35 km, 1) for continental-origin crust, else 1
  (basement = crystalline crust without metasediment).
- **orogenic** = 1 − 0.75 × `Crust::orogenic` (remaining orogenic weakening, heals with τ = 200 Myr):
  collision belts, sutures, accreted terranes.

## Can the viewer derive it from the frame? No.
The frame (`CellInfo`) has `age` (birth-based), `orogenic`, `crust_thickness` (with metasediment)
and the `continental` flag (`is_continental`, not `is_continental_origin`). Missing: the plume
reset (`reheated`), the basement thickness and continental origin. So the simulation exports the
two terms it cannot rebuild.

## Data: two 1-byte channels
`CellInfo` gets `strength_thermal` and `strength_thinning` (f32, 0–1; CellInfo 84 → 92 bytes);
the history stores both as `Code::Share` (1 byte each, steps of 1/254; 39 → 41 bytes per
cell). No flag bits. The combined strength is computed in the viewer by the simulation's own
function (`weak_zones::combine_strength(thermal, thinning, orogenic)`, which
`lithosphere_strength` now calls too: same operations, bit-identical results), with the frame's
`orogenic` (strength error ~0.005 with all terms rounded). Plume heating is recognised where the thermal
term is clearly below the one of the frame's `age` (`thermal_strength(age)` − thermal > 0.025).
Alternative considered: storing the combined strength directly (1 byte) — but then the terms
(sub-modes, cause) could not be separated where the floor 0.05 clamps; two terms are the
smallest set from which everything follows.

## The mode `lithosphere-strength` ("Strength")
- Option **Show**: Strength | Weakest | Thermal | Thinning | Orogenic.
  - Strength / the three terms: relative strength 0–1 on a reversed viridis scale (perceptual,
    colour-blind safe; weak = bright yellow, strong = dark purple), unlit.
  - Weakest: the weakest term per cell, colour by cause (Okabe–Ito): young lithosphere
    (thermal, by age), plume-heated (thermal, reset), thinned continental crust, orogenic belt /
    suture; blended towards neutral grey as the strength rises (full strength = grey).
- Switch **Plate boundaries** (default on): boundary lines by class as in the Plates mode
  (shared code), so rifts/ridges/collisions can be compared with the weak zones.
- Switch **Deforming bands** (default off): stippled as in the Plates mode (#69).
- Switch **Weak-zone outline** (default off): isoline at strength 0.5 (= the floor of the
  deforming-band weights, `deformation::MIN_BAND_STRENGTH`).
- Coastlines always (unlit colours).
- Legend: scale or cause swatches, line swatches, description.
- Cell widget: row "Strength": "0.42, weakest: orogenic" (or "full").
- Works on past frames (everything from the frame).

## UI
Shared egui code; options wrap (horizontal_wrapped) on phones; no new widgets.

## Results to report
Strength distribution for seeds 0–3 (cell dumps of the main-commit runs from #99, 500 Myr):
by crust class, by weakest term, near boundaries by kind; do weak zones sit at ridges, rifts,
orogens/sutures, young ocean? Weaknesses → comment on an existing issue or a new labelled one.

## Independent review (summary) and changes
- Terms export sound and minimal; refactor keeps the same f64 operations (results unchanged).
- Plume heuristic works with a thin margin near the ridge axis (age rounding to 0.25 Myr moves
  the term by ≤ 0.015) → margin set to 0.025 and a unit test over 0–25 Myr ages with both
  roundings. The suggested flag bit (exact) is not used: the flag bits are taken by open
  PRs #140/#142.
- Combined-strength error is ~0.005 with all three terms rounded (not 0.002).
- Blending categories towards grey loses hue first for colour-blind viewers → categories in full
  colour only below strength 0.75, greys above; Okabe–Ito blue, orange, bluish green, reddish
  purple (no orange/vermillion or blue/sky-blue pair).
