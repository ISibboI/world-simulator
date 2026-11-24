# #102 plan: hydrology render mode (rivers, drainage basins, lakes)

_2026-11-24. Branch `agent/issue-102` from `main` (72d612b)._

## What the frame already has, what is missing

Every frame (live and in the history, #115) has the elevation and the discharge (log-coded) of
every cell. Not there: which cell a cell's water flows to (the receiver of the erosion's drainage),
which water is ocean (#58/#114: the water below sea level connected to the world ocean), which
land is in a closed depression, and the depth of lake water. Basins, lakes and endorheic areas
cannot be rebuilt reliably from the quantised elevations (the drainage is a priority flood with
ε-filled flats and the lake levels come from a water balance with runoff and potential
evaporation), so the erosion step exports them.

## Export (no change of the simulation)

- `erosion::erode` returns, next to the discharge, a `SurfaceWater` per cell: the receiver as
  the *water* sees it (the drainage receiver, but none at the exit of an endorheic lake, whose
  water does not leave; none for ocean cells), the ocean mask it used, whether the cell lies in a
  closed depression (`Lakes::lake != NO_LAKE`, i.e. ≥ 1 m below its spill level), and the lake
  depth (water level − floor, for the cells the lake covers). Only collected, nothing reads it
  in the simulation; the state hash must stay identical to `main`.
- `Planet` keeps it like the discharge (before the first erosion step: the ocean mask of the
  initial topography, no receivers).
- `CellInfo` gets two fields: `lake_depth: f32` (m) and `drainage: u32` — bits 0–2 the receiver
  as the slot in `mesh.neighbors(cell)` (7 = none), bit 3 ocean, bit 4 closed depression.
  92 bytes per cell instead of 84.
- History codec: two channels, `lake_depth` as `Logarithmic(1/1000)` (0.1 % relative, 2 bytes)
  and `drainage` as a separate 1-byte integer channel (the 1-byte flags channel is fully used by
  open PRs #140/#142; the receiver slot alone needs 3 bits). 39 → 42 bytes per cell, version 5.

## Derived in the view (pure functions, unit-tested)

`viewer/hydrology.rs` (or in the mode): from the cells of a frame,
- the basin of every land cell: the cell where its water's path ends — the river mouth (the
  last land cell before an ocean cell) or the exit of an endorheic lake (receiver none);
  computed with an iterative, memoised walk (O(n));
- endorheic = the path ends without reaching the ocean;
- per basin: cell count; per lake cell: closed (endorheic) lake or spilling lake.

## Sebastian's rule (#128) in this mode

- ocean (bit) → ocean colour;
- in a closed depression with lake depth ≥ 1 m → lake, coloured by depth (log scale);
  closed lakes (no outflow) stippled;
- in a closed depression with < 1 m water → dry depression: drawn as land (its basin colour),
  stippled darker;
- else land.
The classification lives in `CellInfo` methods (`water_body()`), so #128 can reuse it for the
topographic colours and the 3D view.

## The mode `hydrology`

- Colours (lit, the relief stays visible): ocean dark blue; land by drainage basin, categorical
  (palette by a hash of the outlet cell, so a basin keeps its colour as long as its mouth stays);
  basins that drain to the ocean in a light palette, endorheic basins in a distinct warm/ochre
  palette; small coastal basins (< 8 cells) in one neutral colour (avoids confetti along coasts).
- Overlay: rivers as lines from each land cell to its receiver where the discharge exceeds a
  threshold, width and colour by log discharge (end at the coast / lake shore); basin outlines
  (`category_boundaries` over the basin labels, land-land only); lakes/dry depressions stippled.
- Options: Basins (All / Endorheic / Off), Rivers (Large / All / Off), Basin outlines (on/off).
- Legend: description with live numbers (basins to the ocean, largest basin share, endorheic
  share of land, lake share of land), lake-depth scale, swatches.
- Cell widget: "Surface" says ocean / lake, N m deep (closed) / dry depression / land; a new row
  "Drainage basin" (to the ocean at lat/lon or endorheic, its size).
- Past frames: everything from the frame's cells.

## Checks

- State hash unchanged against `main` (headless, a few steps, two seeds).
- Unit tests: codec round trip of the new channels, basin labelling on a small hand-made graph
  and on a small planet (every land cell has a basin, ocean none, endorheic iff the terminal is
  not a mouth), receivers point to neighbours.
- Plausibility (3 seeds): largest basins, endorheic share, lake share, vs. #114/#58 numbers
  (endorheic ~19.5 % of land, lakes ~3.6 %).
- Desktop and web at phone width via vscreen; screenshots here.

## Changes after the plan review ([`review-plan.md`](review-plan.md))

- A bit "lake (≥ 1 m water)" (bit 5) decides lake vs dry depression, so live and past frames
  agree whatever the quantisation of the depth; the depth is for colours and the widget only.
- With erosion disabled the ocean mask is refreshed every step from the seeds `Planet::step`
  already finds (no receivers, no lakes).
- The cut receivers are a separate vector; the erosion's own receivers are not touched, and no
  loop of the erosion is reordered.
- Spilling vs closed lake in the view: follow receivers while the depression bit is set; the
  lake is closed if that exit has no receiver. Unit-tested with two lakes in a chain and a planet
  without ocean (the lowest cell is an endorheic end, not a mouth).
- Rivers are not drawn between two lake cells. The lake share in the plausibility check is
  compared with the simulation's fractional coverage explicitly.
