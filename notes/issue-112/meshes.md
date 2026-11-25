# Why a fixed world grid next to the plates' samples?

_2026-11-25, answer to Sebastian on PR #165. Code references are to `agent/issue-112`._

## What the two "meshes" are

There is only **one mesh**, the icosphere `SphereMesh` (`src/sphere.rs`). It is used twice:

- **Plate frames.** Each plate stores its crust as `Plate::crust: Vec<Option<Crust>>`, one slot
  per icosphere vertex, in a plate-local frame (`src/tectonics/mod.rs`, `struct Plate`). The
  world position of sample `j` is `plate.rotation * mesh.vertex(j)`; most slots are `None`
  (outside the plate). The sheet of #70 deforms the plate by moving crust *between* these
  vertices (mesh edges in the plate frame), and the plate's edges advance or retreat vertex by
  vertex. Every plate thus carries a rotated copy of the same icosphere.
- **World grid.** The same icosphere, unrotated (`Simulation::mesh`). `Simulation::owners`
  says for every world cell which plate and which local sample it shows
  (`Ownership { plate, local }`), rebuilt in every substep by `resolve_ownership`
  (`src/tectonics/boundaries.rs`) through the matching (`src/tectonics/matching.rs`).

The duplicates of #112 come from mapping the rotated copies onto the unrotated one: the
icosphere's cells vary in area by ±20%, so after a rotation some samples are nearest to two
cells and others to none.

## What needs the fixed grid

1. **Plate boundaries.** Plates' samples overlap where plates converge and leave gaps where they
   diverge. Deciding who is on top, what subducts or collides, and where new sea floor forms needs
   a common reference: that is the ownership resolution on the world grid. The boundary segments
   and polarities (`polarity.rs`), trenches, arcs and gaps are all per world cell.
2. **Everything in the fixed frame.** The mantle flow and dynamic topography (`mantle.rs`,
   `Simulation::dynamic_topography` per world cell), the sun and latitude, the climate
   (`climate.rs`: per-cell areas, edges, fixed Gauss–Seidel sweep orders and a multigrid
   hierarchy of icosphere levels, warm-started vapour fields), the temperature model
   (`temperature.rs`: a coarse icosphere with fixed interpolation weights, ocean heat and ice
   sheets remembered per cell), the ocean circulation (`ocean.rs`, multigrid). The atmosphere
   and ocean do not move with the plates, so these fields have to live on a grid that does not
   move either.
3. **Water and sediment cross plate boundaries.** Rivers, lakes, the sea's connectivity, sea
   level, sediment routing, fans and waves (`erosion.rs`, `sea.rs`) need one connected graph
   with every place of the surface exactly once. The plates' samples overlap and have gaps, so
   they do not form such a graph.
4. **Viewer and output.** One static GPU mesh; every frame of the history is a set of per-cell
   arrays on it (`src/viewer/history`); cell dump, picking, snapshots, the web worker transfer.

## Could the surface work on the plates' samples instead?

In principle: build one **surface mesh from the visible samples of all plates** every substep
(drop overlapped samples, fill gaps, triangulate the rest on the sphere), and run drainage and
erosion on it. Every sample would be one node, so there would be no duplicates or hidden samples
at all, and #112 would disappear by construction.

The cost:
- The ownership resolution is still needed to decide which samples are visible at overlaps and
  what fills gaps; only its output becomes a node list instead of a per-cell index.
- A spherical Delaunay triangulation of ~40k points per substep (O(n log n), affordable), but
  with cells of changing area and connectivity: every algorithm on cells must handle irregular,
  changing neighbourhoods (most already accept arbitrary neighbour lists).
- The climate, temperature and ocean solvers rely on a fixed mesh (multigrid hierarchy, sweep
  orders, coarse-to-fine weights, state kept per cell). They would stay on the fixed grid, with
  interpolation from the surface mesh to the grid and back every step, which is a resampling
  again, just moved to the climate.
- Persistent per-cell state (ice sheets, precipitation, discharge) would need remapping every
  step; the viewer would need a new GPU mesh per frame and the history a mesh per frame
  (several times the memory of the history, which compresses per-cell arrays on a fixed mesh).

That is a rewrite of the tectonics–surface coupling and of the viewer, weeks of work and risk,
for the remaining duplicates (11% of cells after (a)) and their pits. Options (a) and (b) attack
the same artefact locally; a surface mesh from the samples would be the clean solution if the
grid artefacts of the resampling ever become the limiting problem.
