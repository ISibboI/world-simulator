# #93 Plate render mode: plan

Goal (issue #93): a render mode "Plates": every plate in its own, stable colour; plate boundaries
drawn by type; arrows for the plate motion and for the relative motion across boundaries; a
legend. It must work for past frames of the frame history (#109, PR #115), so everything it
shows comes from what a frame keeps.

## What a frame needs

| Need | Source | In the frame |
| --- | --- | --- |
| plate of each cell | `CellInfo::plate` (index) | already |
| plate motion | angular velocity per plate | already (12 B/plate) |
| stable plate identity (colours) | new `Plate::id`, a serial number that survives the renumbering after `swap_remove` | **+4 B per plate** (`PlateInfo { rotation, id }`) |
| boundary type of a cell | the simulation's classification (`dynamics::BoundaryKind` + polarity segments) | **4 bits of `CellInfo::flags`** (the flags channel is a byte, bit 0 is continental) |
| deforming band | collision band (#81) and rift band of the cell's plate | **1 bit of `CellInfo::flags`** |

No new channel: raw frame size unchanged (25 B/cell) apart from 4 B per plate; the compressed
size grows by the changed flag bytes along moving boundaries (measured, see results.md).

### Boundary classes (per cell, seen from its plate)

Derived in the simulation (`Simulation::boundary_classes`, new `tectonics::display` module), from
`boundary_cells()` (the same classification the velocity solve uses) plus the polarity
segments for the overriding side:

1. subducting (lower plate of an active trench: `BoundaryKind::Trench`)
2. overriding (upper plate of an active trench: the mirrored trench condition of the segment)
3. collision (`BoundaryKind::Collision`)
4. diffuse convergent zone (#42/#73: `BoundaryKind::Diffuse`, and the upper side of such a segment)
5. ridge (divergent at young oceanic crust: `BoundaryKind::Ridge`)
6. rift (divergent at continental crust)
7. transform (relative motion within `min_trench_obliquity` (20°) of the strike, as PB2002)
8. other convergent, 9. other divergent, 10. inactive (slower than `min_trench_convergence`)

A cell bordering several plates keeps the highest-priority class (order above). Deforming band:
cells of continental-origin crust of the overriding plate within `collision_band_width` of an
active (converging) collision, and continental cells within `rift_width` of a rift on the same
plate (#69/#81).

## Viewer

- **Plate colours**: a categorical palette (16 muted colours that keep lines and arrows
  readable). The view keeps a map from stable plate id to palette index (`PlateColors`), updated
  for every received state (live and recorded alike, so past frames find their plates): a new
  plate gets the colour that none of its current neighbours has, least used, so neighbours
  differ when the plate appears; colours then never change (stability over distinctness when
  plates drift into contact later; the boundary line still separates them). Oceanic crust is
  drawn a little darker than continental crust, so the continents stay recognisable. Lit
  shading (relief visible).
- **Generic overlay building block** (`viewer::overlay`): an `OverlayBuilder` with borders and
  fills (borders drawn first), taking arrows (existing), line segments on the sphere (thin quads
  lifted above the displayed surface), and markers (small dots, triangles). `RenderMode` gets
  `overlay(&self, data, &mut OverlayBuilder)`; its default draws `arrows()`, so simple vector
  modes stay as they are.
- **Category boundaries** (generic): marching triangles over any per-cell category (here the
  plate): a segment between the midpoints of the edges of each triangle that spans two
  categories (three: to the centroid). Reusable for climate zones, crust type, coastlines.
- **Plate mode overlay**: boundary segments coloured by the combined class of their two cells;
  subduction teeth on the overriding side (thinned to a spacing); dots on deforming-band cells;
  paired relative-motion arrows at points along the boundaries (thinned to the arrow spacing):
  on each side the motion of that plate relative to the other (converging arrows point at each
  other, diverging away, transforms side by side); plate-motion arrows (as the plate-motion
  mode) on the arrow grid away from boundary arrows.
- **Legend**: generalised: swatch shapes (fill, line, teeth, dots), a palette strip, and
  several arrow scales with their labels; scrolls if taller than the screen (phones).
- **Cell widget**: "Plate" shows the stable id; a "Boundary" row shows the class.

## Tests

Unit tests: class bits round trip in flags and through the codec; boundary classes of a
two-plate test planet (converging, diverging, strike-slip); colour assignment (neighbours
differ, stable across frames and renumbering, new plates get new colours); marching-triangle
segments (count/closure on a split sphere); overlay ordering; legend for all modes.

## Size and speed

Measure with the existing `frame_sizes` test (5 and 6 subdivisions) before/after; time of
`boundary_classes` per update and of the overlay build.
