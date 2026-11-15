# Plan: new oceanic crust only where plates diverge (issue 5)

_2026-11-15. Issue 5 in `../ISSUES.md`: 28–30% of crust younger than 3 Myr sits at convergent
boundaries, because every cell left uncovered after resampling the rigid plates is filled with
age-0 ridge crust. This inflates crust production/destruction churn, lowers apparent ocean ages,
puts arcs on artificially young lithosphere (feeding issue 4), and corrupts the measurements
for issue 1._

## Current behaviour (`src/tectonics/boundaries.rs`, `resolve_ownership`)

1. Each world cell takes the crust of the most buoyant plate covering it; losers are subducted.
2. A cell whose neighbours all belong to one plate ("single-cell hole") gets a copy of a
   neighbour's crust (resampling artifact).
3. All other uncovered cells are filled by a breadth-first flood from covered cells: each gets
   **new oceanic crust (age 0)** on the plate it is reached from.

## Change

For every uncovered cell `g` that is not a single-cell hole:

1. Find the nearest covered cells of the **two nearest distinct plates** within a few rings
   (BFS over the mesh, up to 3 rings ≈ 330 km).
2. If only one plate is found, treat `g` like a hole (copy, see 4).
3. Otherwise compute the opening rate across the gap: with `n` the unit tangent at `g` pointing
   from plate a's nearest cell towards plate b's, `opening = (v_b(g) − v_a(g)) · n`, using the
   plates' angular velocities (`v = ω × r`). If `opening > threshold` (default 0.5 cm/yr), the
   plates diverge: create **new oceanic crust** (age 0) as now (sea-floor spreading).
4. Otherwise (convergent or transform motion, i.e. a resampling gap): fill `g` with a **copy of
   the crust of the nearest covered cell** of the plate the flood assigns it to, keeping its
   age, thickness, density and sediment. Count these copies (cells and volume) in a new
   statistic, so the crust ledger of issue 3 can account for them.

The plate assignment by the flood stays as is (symmetric growth from both sides of a ridge).

## Verification

- Unit test: two plates converging with an artificial gap between them → the gap is filled
  with copies (no age-0 crust); two plates diverging → age-0 crust.
- Diagnostic (temporary): fraction of crust younger than 3 Myr whose cell borders a convergent
  boundary (relative velocity of the adjacent plates closing). Before: ~28–30%. Target: < 5%.
- 500 Myr run, seed 0: compare ocean-age statistics, crust production rate, number of copied
  cells per step; ridges must still produce crust (mean age must not explode).
- All existing tests pass.

## Risks

- The threshold might suppress real slow spreading (ultra-slow ridges spread at < 1.5 cm/yr
  full rate); 0.5 cm/yr is below that.
- Copies still create a little volume; quantified by the new statistic, handled with issue 3.
