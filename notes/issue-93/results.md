# #93 Plate render mode: results

Plan: [plan.md](plan.md); its independent review and what was done about it:
[plan-review.md](plan-review.md).

## What changed

- **New render mode "Plates"** (`--render-mode plates`, `?render-mode=plates`, the switcher):
  - every plate in its own colour, darker over oceanic crust so the continents stay visible,
    with lit relief;
  - boundary lines by what the boundary does: subduction (red, with teeth on the upper plate),
    continental collision, diffuse convergent zone (#42/#73), mid-ocean ridge, continental rift,
    transform, other convergent, other divergent, inactive;
  - stippled **deforming bands** (#69/#81): continental crust of the upper plate within the
    collision band (500 km) of a converging collision, and within the rift width (200 km) of a
    stretching rift;
  - yellow arrows: plate motion over the mantle (as the plate-motion mode, length ∝ √speed, full
    at 10 cm/yr), left out next to boundary arrows;
  - white **paired arrows along the boundaries**: on each side the motion of that plate relative
    to the one across (converging arrows point at each other, diverging away, transforms pass
    each other), length ∝ √speed, full at 15 cm/yr;
  - legend: palette, nine boundary types and the band in two columns, two arrow scales in
    cm/yr. It scrolls rather than run into the cell widget; on phones it starts hidden as before.
- **Cell widget**: a "Boundary" row (e.g. "trench, overriding plate; deforming band"); "Plate"
  shows the plate's stable id, the number its colour follows.
- **Simulation side** (display only, nothing feeds back): stable plate ids (`Plate::id`, a serial
  number that survives the renumbering when plates are removed) and
  `Simulation::boundary_map()` (`src/tectonics/display.rs`): the velocity solve's own
  classification (trench, diffuse, ridge, collision), the upper side of trenches and diffuse
  zones from the cell across, and kinematics for the rest (transform within
  `min_trench_obliquity` = 20° of the strike, as PB2002; inactive below
  `min_trench_convergence` = 0.1 cm/yr; rifts as the stretching rule: continental-origin crust
  thicker than the breakup thickness opening faster than `min_stretching_rate`). Collisions
  count as collisions only while they converge.
- **Reusable building blocks** for the modes to come (wind, currents, climate zones, …):
  - `viewer::overlay::OverlayBuilder`: layers of arrows, lines, triangles and dots, each with a
    dark border, borders drawn before fills; `RenderMode::overlay` is the one hook (its default
    draws `RenderMode::arrows`, so vector modes stay one function);
  - `overlay::category_boundaries`: boundary lines between any per-cell categories (marching
    triangles on the displayed surface);
  - `overlay::anchored`: thinning anchored to a fixed spatial grid, so markers and arrows along
    moving lines do not jump between frames;
  - `viewer::plate_colors::PlateColors`: stable categorical colours (16 muted colours);
  - legend swatch shapes (fill, line, teeth, dots, palette) and several labelled arrow scales.

### Plate colours

The view assigns a colour when a plate id first arrives (in `receive`, before the frame is
recorded, so every past frame finds its plates): among the colours none of its current
neighbours has, the one fewest plates have. A colour never changes afterwards, so a split-off
plate gets a new colour and the rest keeps the old one. When plates drift into contact later,
two neighbours can end up with the same colour; the boundary line still separates them
(stability chosen over re-colouring).

## Frames (past frames work in every mode)

Everything the mode shows comes from what a frame keeps: the boundary class (4 bits) and the
band (1 bit) go into the cell flags (an 8-bit channel already, bit 0 was "continental"), the
plate id next to each plate's rotation (`PlateInfo`, 16 bytes instead of 12; frame format
version 2). Raw size per frame: +4 bytes per plate (48 bytes at 12 plates).

`frame_sizes` (seed 0, 200 steps, means over the second 100; before = this branch's base):

| | before | after | change |
| --- | --- | --- | --- |
| 5 subdivisions, stored per frame | 81.5 kB | 82.7 kB | +1.2 kB (+1.5%) |
| 6 subdivisions, stored per frame | 322.7 kB | 326.6 kB | +3.8 kB (+1.2%) |
| flags channel delta, 6 subdivisions | 1.5 kB | 5.3 kB | |
| encode / decode a delta frame (6 subdiv.) | 1.50 / 3.13 ms | 1.49 / 3.13 ms | none |

A default 500-frame run at 6 subdivisions grows from ~161 MB to ~163 MB of history.

## Cost

- `boundary_map` runs with every state sent (in the simulation thread / web worker, in every
  mode, since the history needs it): 5.6 ms at 6 subdivisions against a 356 ms step (1.6%);
  `cell_infos` with it 8.0 ms (`cargo test --release boundary_map_cost -- --ignored
  --nocapture`).
- Building the display in the plates mode (colours and overlay, 80k overlay vertices at 6
  subdivisions): 18 ms (9 ms at 5) per state, in the view.

## Checks

- Unit tests: class codes and order; transforms, opening and closing on a split test planet
  (including continental crust on both sides: rifts, a band); flag bits through `cell_infos`;
  `PlateInfo` through the codec and history; line combination of the two sides; colours
  (neighbours differ, stable under renumbering, a split-off plate gets a new colour, restart);
  overlay layer order, line and triangle geometry, closed boundary lines, thinning and its
  anchoring; the mode on a small planet (colours, overlay, legend).
- GUI (vscreen): desktop at 5 and 6 subdivisions, a past frame (20 of 30 Myr: same colours and
  boundaries), web (Chromium, SwiftShader) wide and in a 400 px window with the legend opened.
- fmt, clippy (native and wasm32), doc and tests with warnings denied.

Screenshots: [desktop](desktop-plates.png), [past frame](desktop-past-frame.png),
[close-up at 5 subdivisions](closeup-teeth-bands.png) (teeth, ridge beside a rift with its
band, paired arrows), [web](web-plates.png), [phone-width legend](phone-legend.png).

## Caveats

- **Coarse grid**: boundaries zig-zag along the cells (~110 km, ~220 km on the web); a cell
  keeps one class, so at triple junctions a line piece may show the class towards the third
  plate. Short transform offsets between ridge segments are sub-grid and appear as oblique
  ridges or "other divergent" (as accepted on #43).
- "Other convergent/divergent" are contacts the solve does not class (e.g. a fresh split, old
  oceanic crust opening, ocean–continent convergence without a decided polarity).
- The collision band is drawn on the upper plate only, as the simulation applies it.
- Plate colours are muted and need the boundary lines where two neighbours share a colour
  later in a run.
- No zoom in the view: at phone size the arrows and teeth are small.
