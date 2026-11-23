# #58: independent review of the plan

_2026-11-21, by an independent reviewer agent (read-only review of [`plan.md`](plan.md) against
the code at 67ca0a2). Condensed; the plan's "Changes after the review" section says what was
adopted._

1. **Jump/tie consistency (high).** In the jump case the solve returns the pass, the basin is
   excluded, and the ocean holds less than its volume by up to the whole basin's volume (not a
   sliver; a 100-cell, 1 km deep basin ≈ 3 m of sea level), possibly for many steps: document
   and measure. A separate BFS on `elevation < 0` in the erosion can disagree with the solve at
   a sill pinned at 0 ± 1e-13 (the `to_dry`/`to_surface` round trip): derive the mask from the
   same pass levels and a strict tie rule, test a sill exactly at the solved level.
2. **Seed (medium).** The deepest cell can be an isolated pit (rift, resampling artefact). Use
   the largest connected body below the current sea level and its deepest cell; check in tests.
3. **Missed `elevation < 0` sites.** Temperature (land fraction, sea ice vs ice sheet), climate
   (runoff, ocean evaporation, evapotranspiration): give the clamped view only to temperature
   and climate, the erosion needs the true elevations plus the mask. The lapse rate already
   uses `max(0)`, so the missing depression warming is the status quo. `wave_bases` seeds from
   `elevation < open_water_depth`: a deep dry depression would be open water; it is `pub`
   (planet statistics). Wave erosion's three sites. Tectonics statistics (sea floor, flooded
   continental, water depth), cell-dump coast, render, planet statistics (land, marine,
   flooded, largest river). Tests computing the volume over `e < 0` (erosion helper
   `ocean_volume`, `after_step`, `the_ocean_holds_its_volume`) need the mask and a tolerance
   for the jump.
4. **Budget terms.** `sediment_sea_level_of` removes sediment and re-solves: a removal can
   reopen a filled basin, so reservoir parts can jump; warn. Per-cell vs per-sample (#57)
   differences land in `sharing`; report it.
5. **Flicker measurement.** A per-cell flip counter mostly counts basins carried by plates;
   count changes of the number/area of inland bodies per step instead. The no-hysteresis
   argument within a solve is correct (monotone volume, unique solution).
6. **Erosion details** confirmed: drainage seeding from the mask, `Lakes::new` skipping ocean
   cells, the incision base, pass B's two closures must agree, diffusion (depressions now get
   the marine rate and 0 m floor; test it). The climate clamp lands on the `>= 0` branches.
7. **Tests to add:** sill at the solved level; jump deficit; deep isolated pit as a seed
   candidate; waves leave a dry depression alone; diffusion into a depression; climate sees a
   depression as land with runoff and potential evaporation.
8. **Measurement:** adjacency for `main` comes from `mesh.csv`; report how often and how far
   the sea level is pinned at a pass, the deficit, lake volume in sea-level metres; flag that
   land/ocean fractions move by definition.
