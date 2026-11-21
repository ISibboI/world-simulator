# Issue #72: cell information widget and outline: plan

Goal (issue #72): a widget in the 3D view with information about the cell under the mouse, and
a frame around that cell on the planet. Same UI on desktop and web; on touch screens a tap
selects a cell.

## How the data reaches the view

The view only received vertex buffers. Now every surface update also carries one
`CellInfo` per cell (`src/viewer/cell_info.rs`): 12 `f32` values and 2 `u32` (elevation,
crust thickness, lithosphere age, sediment, orogenic share and time since the last orogeny,
annual/January/July temperature, precipitation, permanent ice, discharge, plate, a
continental flag): 56 bytes per cell, `Pod`, so on the web it crosses the worker boundary as
one transferred `ArrayBuffer` next to the vertices and is copied into a `Vec` without parsing.

- Alternatives: point queries to the simulation would answer only between steps (seconds at
  the default resolution) and need a message channel into the worker loop, which blocks; an
  attribute array per update is instant and simple.
- Cost: built in the simulation's publish callback, one pass over the cells, about a third of
  the time of the vertices (measured below). Memory 2.3 MB per update at 6 subdivisions,
  0.57 MB at 5 (the web default), on top of 1.6 / 0.41 MB of vertices; the view keeps only
  the latest one.

## Picking

`src/viewer/picking.rs`: the ray from the camera through the pointer (unprojected with the
inverse view-projection matrix) is marched through the shell between radius 1 (ocean) and the
highest displayed vertex in half-cell steps; the cell below each point comes from the
nearest-vertex walk of `SphereMesh` (warm-started), and the first point at or below its cell's
displayed radius is refined by bisection. The relief exaggeration (40×) matters: mountains
stand up to ~0.05 radii, several cells, so a plain ray–sphere intersection would pick cells
behind mountains near the limb. The view builds a `SphereMesh` from the received triangles and
vertex directions (`SphereMesh::from_faces`, factored out of `icosphere`) once per mesh.

## Outline

The barycentric dual cell: triangle centroids and edge midpoints around the vertex, so every
segment lies in one displayed triangle. Drawn by a second wgpu pipeline (`outline.wgsl`) as
screen-space quads of constant pixel width (a light 1.5 pt line over a dark 3.5 pt border, for
contrast on any terrain), instanced, depth-tested against the planet without writing depth;
the points are lifted towards the camera by half a cell spacing so the line is not hidden by
the surface it lies on, but mountains in front still hide it.

## Interaction

- Mouse: the cell under the pointer is shown (not while a button is down); a click selects a
  cell, which is shown while the pointer is not over the planet (so the pointer can move to
  the widget); a click beside the planet clears it.
- Touch: egui reports no hover after the finger is lifted; a tap (a press that moves less than
  egui's click distance) selects, a drag rotates.
- Widget: bottom left (the parameter panel is top right), egui window like the panel, with a
  close button; hidden, a button remains that shows position and elevation of the shown cell
  (the compact form on phones, where the widget starts hidden as the panel does). The
  parameter panel's height stops above the widget where they could overlap (narrow screens).
