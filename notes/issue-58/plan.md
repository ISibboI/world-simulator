# #58: depressions below sea level inside continents are not sea

_2026-11-21. Plan for [#58](https://github.com/ISibboI/world-simulator/issues/58); found in
[`../realism-batch-1/results.md`](../realism-batch-1/results.md)._

## Problem

Every cell whose surface lies below sea level counts as sea: in the erosion (`elevation < 0`
in `erode`: drainage seeds, river mouths, lakes, coasts, waves, diffusion), in the sea-level
solve (`isostasy::sea_level_for_volume_with_pores` over all cells), and in the climate
(ocean evaporation, no runoff or potential evaporation). Small closed depressions below sea
level inside continents are therefore little seas: rivers end in them, the sea refuses their
sediment (39–47% of the refused sediment in the batch-1 diagnostic), and the ocean volume
fills them. On an Earth-like planet such basins are dry or hold lakes with their own water
balance (Caspian, Dead Sea, Qattara, Turpan, Death Valley).

## Rule: the ocean is the water connected to the deepest point

- **Ocean** = the cells below sea level that are connected to the deepest cell of the planet
  through cells below sea level, neighbours sharing a cell edge (`SphereMesh::neighbors`).
  Everything else is land, including dry or lake-filled depressions below sea level.
- **Seed: the deepest cell.** It lies in a trench or abyssal plain of the world ocean on any
  planet with an ocean; an isolated basin deeper than all of the ocean is not plausible with
  ~70% ocean. (A planet with two separate world oceans would need two sea levels; the single
  sea level of the model cannot represent that anyway.) A log line reports the share of
  below-sea-level cells that are ocean, to catch a wrong seed.
- **Straits at cell size.** On the hexagonal-dual icosphere every connection is through a
  shared edge, there is no diagonal ambiguity. A strait is resolved only if the cells along it
  lie below sea level on average, i.e. it is at least ~one cell (~110 km) wide. Narrower
  straits (Bosporus ~1 km, Gibraltar 14 km, Bab-el-Mandeb 26 km) are sub-grid: a sea behind
  such a strait whose cells are not below sea level is a lake here, and conversely a chain of
  cells below sea level may hide a sub-grid sill. This is a grid artefact (#64), accepted and
  explained; no tolerance is added (any tolerance would be as arbitrary as the cell size).

## Sea-level solve with connectivity

- New `pass level` of every cell: the lowest level at which the ocean reaches it, i.e. the
  minimax dry elevation on paths from the deepest cell (a priority flood on dry elevations,
  Barnes et al. 2014, O(n log n)). At sea level `S` the ocean is `{c : pass(c) < S}` with
  water depth `(S − d(c)) · W` plus the pores of its sediment.
- `sea_level_for_volume_with_pores` gets the pass levels as the sort key; the volume is still
  piecewise linear and increasing in `S`. When `S` crosses a basin's pass, the whole basin
  joins at once (a jump of the volume by the basin's volume below the pass). If the volume
  falls into such a jump, the sea level is the pass (the sea spills over the sill into the
  basin; the same treatment as the existing pore jump). The solve is a function of the
  topography and the volume alone and has a unique result.
- Used everywhere the sea level is solved: `Simulation::solve_sea_level`,
  `sediment_sea_level_of` (the sea-level budget and the reservoir metrics, so that the
  differences stay consistent), and the erosion's internal solves (`Seabed::sea_level`,
  `initial_sea_level`).
- **Water volume.** The ocean holds exactly `ocean_volume` (except while the sea level sits
  on a pass in a jump, bounded by that basin's volume; measured). Lakes, including those in
  depressions below sea level, are fed by runoff and lose water by evaporation; their water
  belongs to the hydrological cycle and, as for every lake since #44, is neither taken from
  nor added to the ocean volume. Their total volume will be logged in metres of sea-level
  equivalent to show that this is negligible (Earth's lakes: ~0.5 m). When a depression
  becomes connected, the ocean floods it from its volume (sea level drops by the basin
  volume / ocean area); when it is cut off, its trapped sea water is not tracked: it goes back
  to the ocean volume and the lake balance decides the basin's level from then on (a
  Dead-Sea-sized body evaporates in 10³–10⁴ yr, far below a 1 Myr step).

## Erosion

- At the start of `erode`, the ocean mask is computed by a BFS from the deepest cell through
  `elevation < 0`. With the elevations relative to the connected sea level this equals the
  solve's `pass < S`.
- **Drainage** seeds the priority flood from ocean cells only. Disconnected depressions are
  then filled to their pass like any closed depression and go through the lake treatment of
  #44 (`Lakes::new` skips ocean cells instead of `elevation < 0`): the lake rises until its
  evaporation balances its inflow, can be dry, traps sediment up to its spill level.
- Runoff, the endorheic/lake metrics and `to_ocean` use the mask. The incision base is the
  receiver's surface or lake level, and sea level only where the receiver is ocean (the
  `.max(0.0)` would stop rivers above the floor of a dry depression).
- Pass A (margins, fans) is unchanged in effect: it starts from ocean river mouths and spreads
  through cells below sea level, which are connected to the mouth by construction. Pass B
  after the fluvial sea-level shift: ocean = the original ocean plus cells now below sea level
  connected to it (the sea may now spill into a depression whose pass it overtopped). The
  coastal test for the graded shelf counts depressions as land.
- Diffusion: non-ocean cells use the land rate; land is lowered at most to sea level only
  towards ocean neighbours, towards land (a depression) down to the neighbour's surface.
- Waves (cliffs, wave-base reworking, offshore transport) and `wave_bases` act on the ocean
  mask of the current seabed only, not on lakes or dry depressions.

## Climate and statistics

- The climate and temperature see the ocean through `elevation < 0`. They get a view of the
  elevations in which non-ocean cells are clamped to ≥ 0 m: depressions are land (runoff,
  evapotranspiration, potential evaporation for the lake balance — without that the lake
  balance of a depression would have zero potential evaporation and always spill). The
  missing lapse-rate warming of a depression (0.65 K per 100 m) is neglected.
- `Planet` statistics (land, largest river, reservoirs, shelf, wave bases) use the mask.
  Render/viewer colouring stays elevation-based (shown as a caveat; a follow-up if wanted).

## Hysteresis

None: the classification is a pure function of the topography and the ocean volume at each
step, so within a step nothing can oscillate (the solve has a unique result, and the
erosion's mask follows from it). Between steps a basin flips only when its pass crosses the
sea level by tectonics, erosion or deposition; a flip moves the sea level by the basin
volume / ocean area (a 5-cell basin 300 m deep: ~0.1 m) and switches its sediment from marine
to lake routing, both of which fill it. A counter logs the cells that flip per 100 Myr; if
it shows persistent flicker of the same basins, a tolerance on the pass is the fallback.

## Tests

- A depression below sea level on a continent, cut off from the ocean, is not ocean: no water
  in the volume solve, rivers end in it as a lake, arid → dry, humid → spills at its pass.
- The same depression connected by a channel of cells below sea level is ocean.
- The connected solve holds the volume exactly (`ocean_volume` over the connected cells
  equals the target), and a jump case returns the pass.
- Existing erosion/sea-level budget tests keep passing.

## Measurement (8 seeds × 1000 Myr vs `main`, 500–1000 Myr, Welch t)

Reuse the `main` run of the #57 agent (same commit 67ca0a2 and arguments: `--duration 1000
--snapshot-interval 100 --material-budget --cell-dump --width 512`). Report: sediment that
finds no room at the mouth (terminal fans and deltas, overfill), inland below-sea-level
depressions (number, area, dry vs lake; on `main` from the cell dumps), endorheic share, lake
area, lake volume, sea level budget/sediment part, ocean area, land, mean land, relief,
flips, runtime (paired local timing), and the usual guard rails. Expected: terminal fans and
deltas fall (rivers no longer end in inland seas), lakes and endorheic land rise slightly,
sea level rises by < 1 m (less volume trapped inland).

## Interaction with #57

#57 changes `apply_surface_changes` (duplicated samples), which removes single-cell pits;
both change pit/lake/endorheic metrics. This change stays in erosion/drainage/sea-level code;
whichever lands second rebases and re-measures briefly.

## Changes after the review ([`review-plan.md`](review-plan.md))

- **One helper for the solve and the masks** (`src/ocean.rs` or a new module): pass levels by
  a priority flood from the seed; the mask is `pass < S` from the same numbers. In the erosion
  the mask is computed on the surface elevations relative to the solved level with a strict
  tie rule: a cell is ocean only if its pass is below −1 mm, so a sill pinned at the sea level
  (± rounding) is never ocean, as in the solve, where the pinned basin is not counted.
- **Seed:** the deepest cell of the largest connected body of cells below the current sea
  level (by cell count); tests include an isolated pit deeper than the ocean.
- **Jump deficit:** documented as "the sea stands at the sill; the water that would spill
  into the basin is not represented" (as for the pore jump). The tectonics logs the deficit
  in metres of sea level and the results report how often and how far it occurs.
- Temperature and climate get the clamped view (lapse rate unchanged: it already clamps);
  erosion gets the true elevations. `wave_bases` takes the mask. Tectonics and planet
  statistics, the cell-dump coast column and largest river use the mask; render/viewer stay
  elevation-based (caveat).
- Flicker: per snapshot the number and area of inland below-sea-level bodies and dry vs lake;
  per step changes of the inland area are accumulated (|Δ area|, cells per 100 Myr).
- Tests as listed in the review; the existing volume tests use the mask and allow the jump.
