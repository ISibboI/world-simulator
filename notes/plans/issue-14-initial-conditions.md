# Plan: issue 14, natural initial continents and plates (2026-11-19)

Research: `research/initial-conditions.md`. Problem: `generation::grow_regions` grows continents
and plates by Dijkstra with i.i.d. edge costs (first-passage percolation), whose limit shape is
a rounded hexagon; margin thinning and ocean ages use mesh graph distances (hexagonal too).

## Design

All in `tectonics/generation.rs`, called from `Simulation::new`; deterministic per seed (all
randomness from the simulation's ChaCha RNG, including the noise permutation tables).

1. **Noise.** Own 3D gradient noise (Perlin 2002 style: seeded permutation table, 12 edge
   gradients, quintic fade; ~60 lines, no new dependency — the noise crates add far more than
   the one function needed and their seeding/versions would affect determinism). fBm (octaves,
   lacunarity 2, gain 0.5) and domain warping (positions displaced by a 3-component fBm) at the
   cell's unit vector, so the field has no mesh anisotropy.
2. **Continents.** Field = large-scale "craton" bumps (`continent_count` random centres with
   random radii, smooth cap profile) + warped fBm detail. Continental = cells above the
   quantile of the field that gives exactly `round(continental_fraction · cells)` cells (ties
   by index). Dispersed: centres uniform on the sphere → a few large continents plus fragments
   from the fBm. Supercontinent (`--initial supercontinent`): centres within a cap around a
   random pole, and the continent is grown by a priority flood over the field from its maximum
   (frontier cell with the highest value next), stopped at the exact count → one connected
   cluster shaped by the same noise.
3. **Margin thinning** from the great-circle distance to the nearest oceanic cell: multi-source
   Dijkstra that propagates the nearest source cell and ranks by the true angle to it (vector
   propagation, near-exact Euclidean distance, no hexagonal metric). Same thickness law and
   parameters (`continental_coast_thickness`, `continental_interior_thickness`,
   `continental_margin_width`, ±500 m jitter).
4. **Plates.** Noise-warped, multiplicatively weighted Voronoi regions: score of plate i at cell
   x = angle(warp(x), centre_i) / w_i, weights lognormal so plate areas span ~1–25%. Built by a
   priority flood from the centre cells ranked by that score (each cell goes to the first plate
   that reaches it), so every plate is connected and non-empty. `--plates` unchanged.
5. **Ocean ages** as now (triangular age–area distribution by rank of distance from the
   boundaries that are no trenches), but ranked by great-circle distance (item 3's propagation)
   instead of graph distance.
6. Remove `grow_regions`, graph `distance_to`, `sample_distinct_cells` if unused.

## Verification

- Tests: exact continental fraction (several seeds, both modes); coastline box-counting
  dimension (or no 6-fold symmetry of the distance-from-centroid profile); plate area spread
  (largest/smallest ratio, a few plates > 10%); supercontinent is one connected component;
  determinism (same seed → identical crust); existing tests pass (adapt only tests that encode
  the old geometry, with reasons).
- Look at the t = 0 elevation and plate maps (seeds 0–2, both modes) vs the old ones
  (`agent-scratch/i14/before-*`).
- Seeds 0–5 × 500 Myr on tktie vs the baseline at f037944 (`consumed` runs): continental %, land
  %, ocean age, plates, collision length, flooded, mean land; no regressions beyond seed noise,
  ideally a shorter initial transient.
