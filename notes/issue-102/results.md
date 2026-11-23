# #102 results: hydrology render mode

_2026-11-24. Plan: [`plan.md`](plan.md), plan review: [`review-plan.md`](review-plan.md),
results review: [`review-results.md`](review-results.md) (seed labels and the endorheic
conclusion corrected, lake outflows drawn, the coastal-basin colour darkened after it)._

## What it shows

Render mode **Hydrology** (`--render-mode hydrology`, `?render-mode=hydrology`):

- **Drainage basins**: every land cell coloured by the basin its water ends in. Basins that drain
  to the ocean in a light palette, **endorheic** basins (whose water never reaches the ocean) in
  warm sandy hues; small coastal basins (< 0.05% of the surface, 20 cells of the default grid,
  5 on the web grid) in one neutral colour. A basin's colour follows its outlet cell, so it
  stays as long as the river mouth stays in the same cell (it changes when the mouth moves, which
  deltas and coasts do every few Myr). **Drainage divides** as thin dark lines between basins
  (`category_boundaries`, land–land only).
- **Rivers** as lines from each cell to the cell its water flows to, width and colour (light to
  dark blue) by log discharge; from ~3000 m³/s ("Large", as the topographic mode) or ~300 m³/s
  ("All"); a river ends at the shore of the sea or a lake.
- **Lakes** by Sebastian's rule of #128: a closed depression with ≥ 1 m of water is a lake,
  coloured by its depth (1–1000 m, log scale); **closed lakes** (no outflow, i.e. the end of an
  endorheic basin) are stippled; closed depressions with < 1 m of water are **dry depressions**:
  land in their basin's colour, darker and stippled.
- Options: Basins (All / Endorheic only / Elevation colours), Rivers (Large / All / Off), Divides.
- Legend with live numbers (basins to the ocean, largest basin, endorheic share, lakes, dry
  depressions).
- Cell widget: "Surface" now says ocean / lake, N m deep / land, dry depression / land (by the
  ocean bit, not by elevation < 0); new rows "Drainage basin" (to the ocean or endorheic, its
  share of the land; for lakes "closed lake" or "lake spills") and "Basin outlet" (position and
  cell of the mouth or endorheic exit).
- Past frames show alike: everything comes from the frame's cells.

Screenshots: [desktop, basins](desktop-basins.png), [desktop, endorheic only with the cell
widget](desktop-endorheic-cell.png), [desktop, past frame](desktop-past-frame.png),
[web at phone width, legend](web-phone-legend.png), [web at phone width, cell](web-phone-cell.png).

## What was exported (simulation unchanged)

`erosion::erode` returns a `SurfaceWater` next to the discharge: per cell the receiver as the
water sees it (the drainage receiver, cut at the exit of an endorheic lake and for ocean
cells), the ocean mask it used, closed depression (`Lakes::lake`), and lake depth (water level −
floor). Only collected, read by nobody in the simulation. With erosion disabled the planet
keeps the ocean mask of each step's topography.

`CellInfo` gains `lake_depth: f32` and `drainage: u32` (bits 0–2 receiver slot in
`mesh.neighbors`, 7 = none; bit 3 ocean; bit 4 closed depression; bit 5 lake ≥ 1 m — a bit of
its own so that live and past frames agree whatever the quantisation of the depth): 84 → 92
bytes per update and cell. Frame history: two channels, lake depth `Logarithmic(1/1000)`
(2 bytes) and drainage (1 byte, separate from the full flags byte), 39 → 42 bytes per cell,
codec version 5. Measured (`frame_sizes`, 5 subdivisions, 100 steps): the two channels add
0.5 + 1.5 kB of ~113 kB stored per frame (+1.8%).

State hash identical to `main` (72d612b) for seeds 1 and 2 (5 subdivisions, 20 Myr, 3
snapshots each) and with `--no-erosion` (seed 3).

## Plausibility (3 seeds, default grid, 600 Myr)

`cargo test --release hydrology_statistics -- --ignored --nocapture` (`HYDROLOGY_SEED`,
`HYDROLOGY_STEPS=600`, `HYDROLOGY_EVERY=100`). "Endorheic" is the land whose basin has no
outlet to the ocean (the view's labelling); "sim." is the simulation's own count
(`Hydrology::endorheic_cells`); lakes are cells with ≥ 1 m of water against the simulation's
fractional lake cover.

| seed, t | largest basins (% of land; e = endorheic) | endorheic % (sim.) | in single-cell pits | lakes ≥ 1 m % (sim. fractional) | closed lakes % | dry depr. % |
|---|---|---|---|---|---|---|
| 0, 300 | 11.6 6.0 3.4 3.3 3.2 | 10.2 (10.2) | 4.4 | 2.9 (2.6) | 0.6 | 1.5 |
| 0, 600 | 3.7 2.2 2.2 1.8 1.6 | 14.0 (14.0) | 9.4 | 3.0 (2.6) | 0.9 | 1.2 |
| 1, 300 | 5.0 4.6 4.2 4.0 3.5 | 15.0 (15.0) | 6.9 | 2.6 (2.3) | 0.7 | 1.0 |
| 1, 600 | 9.0 6.2 4.2 4.2 4.1 | 13.6 (13.6) | 9.6 | 3.2 (3.0) | 0.5 | 1.3 |
| 2, 300 | 8.1 6.3 5.6 3.2 2.1 | 12.0 (12.0) | 5.4 | 3.5 (3.4) | 0.8 | 1.4 |
| 2, 500 | 9.7 8.0 7.3 4.6e 2.9 | 27.9 (27.9) | 12.0 | 2.9 (2.7) | 0.8 | 2.4 |
| 2, 600 | 12.1 6.6 5.1 3.7 3.4e | 41.6 (41.6) | 18.5 | 3.4 (3.7) | 0.7 | 4.8 |

Full output per 100 Myr: [`stats/`](stats/) (its count of larger basins used an earlier threshold of 8 cells).

- **Consistency**: the view's endorheic share equals the simulation's to the cell in every
  snapshot (the receivers survive the coding; endorheic exits are cut exactly where the
  simulation stops the water). Lakes ≥ 1 m are 0.1–0.4 points above the fractional cover,
  as expected (a partly covered last cell counts whole).
- **Largest basins** 4–12% of the land, the five largest together 12–30%. Earth: Amazon ~4%
  of the land, Congo 2.7%, Nile 2.3%, Mississippi 2.2%. Plausible for an Earth-like planet with
  fewer, larger continents; the largest basins here are ~2× Earth's.
- **Endorheic** 10–17% of the land from 300 Myr on (2–8% at 100–200 Myr; #114 measured 19.5%
  over 600–1000 Myr; Earth ~18–20%), but **single-cell pits (#129) make 30–70% of it** in every
  snapshot: without them endorheic land is 4–8% (seed 2 at 500–600 Myr: 16–23%). Seed 2 rises to
  28% and 42% at 500–600 Myr, 12–19 points of it in pits. So the endorheic share that looks
  Earth-like is largely the pit artefact; real closed basins are rarer than on Earth. The mode
  shows the pits as many small orange basins with a dot each (dry depressions or closed lakes).
- **Lakes** 2.6–3.5% of the land (#114: 3.6%, Earth ~2%); closed lakes 0.5–0.9%; dry closed
  depressions 1–2.4% (4.8% in seed 2 at 600 Myr, again the pits).
- **Coasts**: 1300–2200 basins drain to the ocean, most of them one or two coastal cells (on the
  ~110 km grid every coastal cell that is not on a river is its own basin). They are drawn in
  one neutral colour; this is a grid artefact (#64), not a property of the planet.

## Caveats

- The receivers, ocean mask and lake depths are those the erosion step saw, before its own
  changes of the surface; the frame shows the elevations after it (the discharge always had this
  lag). A cell can rarely be ocean by the bit but just above sea level after the step (or the
  reverse); the mode and the widget use the bit.
- Basin colours change when an outlet moves to another cell; there is no tracking of basins over
  time (a basin has no identity beyond its outlet).
- Lit shading (relief visible) makes the palette colours a little darker or lighter than the
  legend.
- The cell widget decides ocean/land by the ocean bit (before the step) but shows the elevation
  after it, so a cell can rarely read "+3 m, ocean"; #128 should keep this in mind.
- The basins are labelled once per update for the widget and again by the mode (colours,
  overlay, legend): O(cells) each, well below a millisecond; not shared.
- The legend's statistics paragraph uses the small description font of all modes.
- The 1 m rule here is only in this mode and the cell widget; #128 (all modes, the topographic
  colours and the 3D view) can reuse `CellInfo::water_body()`, which every frame has.

## Possible conflicts with open PRs

#138, #139, #140, #142, #143 and #145 also add fields to `CellInfo` / channels to the codec,
rows to the widget and modes to the registry. Conflicts are mechanical (field lists, channel
indices, `ROW_LABELS`, the version byte); whichever merges second renumbers its channels.
