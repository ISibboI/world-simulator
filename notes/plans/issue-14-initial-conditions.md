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

## Implementation notes (deviations from the design)

- Plates: multiplicative weights gave circular (Apollonius) boundaries and could leave a plate
  with one cell; now **additive** weights, centres in the warped frame kept apart by ~0.6 × the
  sum of their target cap radii, and 20 iterations adjusting the weights towards Zipf-like target
  areas (rank^−0.9 × lognormal scatter σ 0.25): largest 16–35%, smallest 1.5–3% for 12 plates.
- **Continent cohesion**: with plates independent of the continents, the initial state had
  ~62–66k km of continental collisions (old and new generator alike; Earth ~5k) and the noise
  continents (larger connected masses) made the suture transient worse (6 seeds: ocean older than
  150 Myr 25.5% at 100 Myr vs 17.8%, seed 1 sutured into one plate by 50 Myr). Plates now spread
  from continental into continental crust with a score advantage of 2 rad
  (`CONTINENT_COHESION`), so continents mostly lie on one plate and boundaries follow oceans and
  coasts (initial collisions 14–19k km). Tested 0 / 0.5 / 1 / 2 rad (seeds 0–2 × 150 Myr): the
  transient shrinks monotonically.
- Margin thinning: with the linear 1000 km ramp the irregular coasts flooded 39–42% of the
  continental crust (old 26–30%); the ramp is now quadratic over the same width
  (`1 − (1 − d/w)²`, most thinning near the coast, as Earth's 100–400 km margins): 20–28%.
- Supercontinent: priority flood from the field maximum plus a bowl `−0.3 (θ/θ₀)²` around the
  pole (θ₀ the cap radius of the continental fraction), else the flood follows tendrils.
- Anisotropy test: box counting does not separate old and new (old 1.12–1.23, new 1.24–1.27, the
  cell-scale jaggedness dominates); the test uses **solidity** (area / convex hull in the
  gnomonic projection, continents > 2%): old 0.79–0.92, new 0.57–0.86, mean of 8 seeds < 0.75.

## Results (2026-11-19)

t = 0 maps: `agent-scratch/i14/before-{0,1,2}/` (old) and `after-{0,1,2}/`,
`super-{0,1,2}/` in the agent worktree. Initial state (seeds 0–2): land 30–31% (old 28–30),
flooded 20–27% (26–30), sea level −386…−402 m (−414…−425), land mean 750–770 m (820–840).

Seeds 0–5 × 500 Myr on tktie, 6-seed means (baseline: f037944 `consumed` runs):

| | t = 0 coll. | age 50 / 100 / 150 | > 150 Myr at 100 | cont. at 100 | sed. SL at 100 |
|---|---|---|---|---|---|
| baseline | 66k km | 82 / 70 / 60 | 17.8% | 36.6% | +121 m |
| noise, independent plates | 62k km | 89 / 92 / 68 | 25.5% | 36.7% | +150 m |
| **noise + cohesion** | 19k km | 67 / 62 / 54 | 13.2% | 37.5% | +100 m |

300–500 Myr (mean ± seed sd), baseline → new: continental 35.0 ± 1.2 → 36.5 ± 0.6%, land
31.5 ± 2.6 → 31.1 ± 2.8%, ocean age 50.5 ± 3.5 → 53.6 ± 5.3 Myr (seed 4 63), ocean > 150 Myr
6.3 → 7.2%, plates 18.7 → 17.1, collisions 15.3k → 11.6k km, flooded 23.5 → 26.5%, mean land
520 ± 130 → 443 ± 71 m, sediment part of the sea level +276 → +265 m, continental crust masses
82 → 70. No regression beyond seed noise; the initial transient is shorter (ocean age peak 67 vs
82 Myr, earlier and lower old-ocean peak, less early continental loss).

Supercontinent (seeds 0–2 × 500 Myr): starts with 11% flooded, 36k km collisions; stays largely
together for 200–300 Myr (largest crust mass 88–99% at 300 Myr in two seeds), ocean > 150 Myr
peaks at 20% (100 Myr); 300–500 Myr like dispersed (continental 37%, age ~51 Myr) except the
sediment part of the sea level (+311 ± 30 m vs +265 m). Breakup is
slower than Earth's (Pangaea ~100 Myr) — a rifting question, not one of the initial state.
