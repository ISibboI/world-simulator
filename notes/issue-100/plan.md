# #100 Sediment thickness render mode — plan

Issue: a render mode for sediment thickness (log scale) on land and sea floor, showing deltas
and fans (#47), shelves, enclosed basins and accretionary wedges, with a legend.

## Data (no frame growth, no simulation change)

- Every frame of the history (#115) already keeps `CellInfo::sediment`, the rock-equivalent
  thickness (mass / sediment rock density), at 1 m up to 65 km (`history::codec`). Nothing is
  added to the frame or to the flags channel.
- The bulk (compacted, with pores) thickness, what seismic isopach maps of Earth show, is a pure
  function of it: `isostasy::sediment_bulk_thickness` (a cached table, no transcendental math at
  call time). The viewer computes it per cell when colouring (41k table lookups per update).
- Deposition/erosion rates are not in the frame and not exported; a difference between frames
  would mix deposition with the advection of the crust by the plates. Not done (listed in the
  PR as possible follow-up).

## Mode `sediment` (label "Sediment")

- New `src/viewer/modes/sediment.rs`, variant `RenderModeId::Sediment` (name `sediment`), at
  the end of the switcher list (the open PRs #138/#139/#140/#142 also add variants there; a
  trivial conflict).
- Options: `Thickness` = Bulk (default) | As rock; `Isopachs` switch (default off).
- Colours: ColorBrewer YlOrBr, 9 classes (sequential, colour-blind safe, lightness falling
  monotonically from pale yellow to dark brown), spread evenly on log10 from 10 m to 20 km;
  values below 10 m (bare rock at ridges, eroding mountains) take the palest colour, above 20 km
  the darkest. Ticks 0.01, 0.1, 0.3, 1, 3, 10 km. Shown unlit (colours match the legend) with
  the coastlines as lines, as the temperature and moisture modes do.
- Isopachs (option): lines at 0.1, 1 and 5 km of the shown thickness via `overlay::isolines`,
  dark with a white border, wider for thicker; one legend swatch.
- Legend: the scale, coastline (+ isopach) swatches, a description of what is thick where on
  an Earth-like planet.
- Cell widget: the existing "Sediment" row shows both, e.g. `690 m (512 m as rock)`.

## Tests

- Scale: monotonic lightness, ends clamp, ticks inside the range.
- Bulk ≥ rock, option switch changes colours, isopach option adds overlay vertices, legend
  swatches follow options (pattern of the temperature mode's tests).
- Cell widget row text.

## Checks

- Desktop via vscreen and the web build at phone width (touch targets come from the existing
  switcher; nothing new in the UI beyond the options).
- Plausibility for 4 seeds at 500 Myr from `--cell-dump` (bulk thickness by setting: young/old
  ocean floor, shelves, land) against Earth orientation values (mean oceanic ~0.5 km, near zero
  at ridges, passive margins/deltas up to 10–15 km).

## Plan review (independent agent) and what changed

1. Bulk is not always thicker than the rock equivalent: rock-equivalent is mass at 2050 kg/m³
   (24% pores); bulk ≈ 1.9× rock near the surface, equal at ~5 km, less above (20 km rock ≈
   16 km bulk). → Test checks monotonic growth, bulk > rock when thin and < rock when thick;
   the hover text says so.
2. The widget example was wrong; the row switches to km from 1 km. → Row is
   `<bulk> (<rock> as rock)`, km from 1 km; test updated with real numbers.
3. A near-black coastline vanishes on the dark browns along coasts. → Blue coastline with a
   thin white border.
4. Isopachs should be drawn on the log axis (as the colours). → Done.
5. Zero thickness must not give −inf. → Clamped before log10; tested.
6. Isopach levels 0.1, 1, 10 km (on the decade ticks) rather than 5 km; one range for both
   options; "stops", not "classes". → Done.
7. Bulk table is built on first use (a few ms, also on the web). Accepted.
8. Plausibility compares bulk values with Earth's (bulk) values; ~110 km grid caps narrow
   wedges and deltas. → In results.
