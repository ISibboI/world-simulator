# #37 option 2: independent review of plan-kernels.md (2026-11-27) and how it was addressed

Reviewer: an independent agent (read the plan, the feasibility notes, the prototype and
`mantle.rs`; no code changes). Verdict: approach sound, go ahead. Findings and responses:

1. **Gram–Schmidt must track the basis change** for the surface stress. — Done: the surface
   normal stress of each downward column goes through the same linear map
   (`kernels.rs::Pair::orthonormalise`). Tests: kernels agree with the (fixed) prototype to
   < 1e-4 and converge (6 vs 48 steps per `l+3` per unit ln r: < 1e-5).
   Found on the way: the prototype read the deeper layer's viscosity at the RK points on a layer
   boundary, ~1% off in layered profiles; fixed in `kernels_prototype.py`.
2. **Degree 1 is frame-dependent** (core held fixed, no self-gravitation) and would add a
   hemispheric tilt. — Accepted: the sum starts at l = 2.
3. **Grid resolution.** — Measured (`mantle.rs::tests::slab_field_sensitivity`, 5000 km trench):
   at slab grid 3 the tent spreading over ~890 km cells widens the loads and the peak is 15–21%
   shallower than converged (slab grid 5–6 on the cells: −1.71…−1.74 km). Slab grid 4 with the
   evaluation grid 4: −1.57 km (−10%); evaluation 5: −1.62 km (−6%) but 7× the cost. Chosen:
   slab grid 4 = evaluation grid 4, with each pair of cells visited once (46 ms per call with
   every slab cell loaded, vs 20 ms at slab grid 3). The remaining −10% is a grid artefact (#64).
4. **The 300 km load width must not act as a tuning knob.** — It does matter: 200 / 300 /
   450 km give −1.93 / −1.56 / −1.36 km on the cells (+24% / −13%). It is a physical quantity
   (horizontal extent of the slab load: the upper-mantle slab spans 100–660 km offset,
   σ ≈ 160–200 km perpendicular to the trench; lower-mantle slabs thicken to 200–400 km and fold
   into piles), left at 300 km, the wide end, and reported as an uncertainty, not fitted.
5. **α profile from memory.** — Kept as an approximate, documented profile (3.1 → 2.0 at 660 km →
   1.45 at 1500 km → 1.0 × 10⁻⁵ K⁻¹ at the CMB), a decrease by ~3× over the mantle as in
   Chopelas & Boehler 1992; exact literature values not re-checked. Simplifications named in the
   module docs (no self-gravitation, no 660 km phase-boundary compensation, radial viscosity).
6. **Stage 4 / CMB** consistent; the exponential residences spread slabs over depth (some mass
   reaches deep stages early). — Comment in `step` rewritten; steady-state test uses all four
   stages (199 Myr, 1000 steps).
7. **Test band.** — The trench test now asserts −1.35…−1.9 km on the slab line (−1.57 km at the
   default grids, −1.74 km converged), i.e. inside the expected −1.7…−2.4 km once the grid
   artefact is taken out. Added: Green's function integrates to zero, kernel limits, half-space
   limit, convergence.
8. **Evaluation additions** (field statistics on its own, sea-level shift, LM 100× envelope,
   cost per step). — Field statistics, sea level and run time are in the results; LM 100× is
   reported from the trench test (−1.03 km on the cells, −41%) rather than full runs.
9. Cost/determinism estimates right; `--web-math` hash check done (two runs identical).
10. Viscosity profile fine.
