# Research: better initial conditions for the plate simulation (2026-11-19)

User: the initial continents are hexagonal and some stay so for a long time; research how a
better initialisation could look.

## Why the continents are hexagonal

`tectonics/generation.rs::grow_regions` grows continents (7 seeds, 40% of the surface) and plates
from seed cells by Dijkstra over the mesh with independent random edge costs (1–100). That is
first-passage percolation: with i.i.d. edge weights the grown region converges to a deterministic,
convex limit shape (shape theorem: Richardson; Cox & Durrett; Kesten) that inherits the lattice's
symmetry — on the icosphere's near-hexagonal lattice, a rounded hexagon. Independent noise per
edge averages out over the ~20–40 cells of a continent's radius; it never makes long-range
irregularity (bays, peninsulas, elongated shapes). The same applies to the plates and to the
graph-distance fields used for margin thinning (`distance_to` coast) and initial ocean age
(ridge distance), which are hexagonal in the mesh metric too.

## Earth's statistics a generator should reproduce

- Continents are elongated and irregular, with fractal coastlines (Richardson/Mandelbrot:
  dimension ~1.0–1.1 for smooth coasts like South Africa or Australia, ~1.25 for Britain, up to
  ~1.5 for fjord coasts); the continental crust (incl. shelves) is ~40% of the surface, in a few
  large blocks plus many microcontinents and fragments.
- Plate sizes: a few large plates plus many small ones; areas roughly power-law distributed
  (Bird 2003; Sornette & Pisarenko 2003, exponent ~0.25 for the cumulative distribution); over
  200 Myr the largest 6–7 plates follow a shallower slope (~0.3) and the small ones a steep one
  (3–5) (Morra et al. 2013).
- Ocean floor ages from ridges, ~symmetric, max ~180 Myr; continental margins thinned over
  100–400 km.

## Options

1. **Correlated noise instead of independent edge costs** (standard in procedural planet
   generation): a 3D fractional-Brownian-motion noise field (sum of octaves of gradient noise,
   evaluated at the cell's unit vector, so there is no mesh anisotropy), domain-warped (the noise
   is evaluated at positions displaced by another noise field) to break regularity and make
   elongated, curved shapes; continents = cells above the threshold that gives exactly 40%
   (a quantile, so the fraction is exact). The spectral slope controls coastline roughness
   (fBm with Hurst exponent H: D = 3 − H for the surface; fewer high octaves → smoother coasts;
   coastline dimension can be checked by box counting). Large continents from a low-frequency
   octave, microcontinents from higher ones. Crust thickness can follow the same field (thick
   interiors, thinned margins) instead of graph distance. Plates: Voronoi cells of random
   centres with noise-warped great-circle distance and weights drawn from a power law, so plate
   sizes span large and small plates.
2. **Supercontinent start + spin-up** (Wilson cycle): one noise-shaped supercontinent
   (Pangaea-like, ~30–40% of the surface) on one or a few plates in a superocean; the simulation
   breaks it up by its own rifting (plumes under the supercontinent), and continents get their
   shapes from rifts, collisions and accretion. Optionally discard the first ~200–400 Myr as
   spin-up for statistics (the model's diagnostics already show a ~200 Myr initial transient).
   Physically the most natural Earth-like start and shows the model's own dynamics; shapes still
   inherit the initial outline until rifting has cut it several times.
3. **Many small cratons accreting** (Archean-style): emergent shapes, but slow and dominated by
   collision churn, which the model already has too much of. Not recommended.
4. Independently of 1–3: replace graph distances by great-circle distances (or the noise field)
   wherever initial fields are derived from distance (margin thinning, ocean age from ridges),
   so that no hexagonal imprint remains.

## Recommendation

Option 1 for shapes (cheap, deterministic per seed, exact continental fraction, no mesh
imprint), combined with option 2's configuration as a choice (`--initial supercontinent`
assembling the noise-shaped continents into one cluster vs `dispersed`), and option 4 for the
derived fields. Verify: coastline box-counting dimension ~1.1–1.3, continent size distribution
(a few large + fragments), plate area distribution (power-law-like), unchanged initial isostasy
and sea level, and a shorter initial transient in the existing diagnostics.

Sources: shape theorem and limit shapes of first-passage percolation (Auffinger, Damron &
Hanson survey; Ahlberg 2011, arXiv:1107.2280); noise-based terrain and domain warping (Galin et
al., review of digital terrain modelling; The Book of Shaders ch. 13); coastline box counting for
generated terrain (arXiv:1610.03525); procedural tectonic planets (Cortial et al. 2019, CGF
38); plate-size statistics (Sornette & Pisarenko 2003, arXiv:cond-mat/0202320; Morra et al.
2013, EPSL; GSA Today 2018).
