# Plan: force-driven plates with mantle flow, plate birth and merging (issue 1)

_Created 2026-11-15. Goal: fix issue 1 in `notes/ISSUES.md` — ocean floor too old (mean
117–175 Myr vs Earth ~64, max should be ~180–200), 22% of the surface at the depth asymptote,
plates only die (12 → 5–10)._

## Approach

Plate velocities follow from a **torque balance** on every plate (quasi-static: the mantle is so
viscous that forces balance at every instant), recomputed every time step:

- **Slab pull** at trench cells of a subducting plate, towards the trench. Physically
  `F = ε · Δρ g L h(t)` per metre of trench, lithosphere thickness `h = 2.32 √(κ t)`; effective
  values ~1–3e13 N/m for old slabs (most of the gross pull is balanced locally by slab
  resistance, hence the efficiency `ε`).
- **Ridge push** at the flanks of spreading ridges, away from the ridge: ~3e12 N/m.
- **Basal drag** under every cell: `τ = c (v_mantle − v_plate)` with `c = η / h_asthenosphere`
  (calibration constant, ~1e15–1e16 Pa·s/m), stronger under thick continental keels.
- **Collision resistance** where continental crust of two plates converges: a stiff coupling
  that opposes relative normal motion (active set: only where they actually converge).
- **Mantle flow** `v_mantle` (approach B mixed in): a smooth, slowly evolving large-scale field
  from downwellings where slabs sank (with a memory of ~100 Myr) and upwellings at mantle
  plumes, which appear preferentially far from subduction zones.

With drag linear in velocity, each plate's torque balance is linear in its angular velocity ω:
`Σ c A (I − r rᵀ) ω = Σ r × (c A v_mantle) + Σ r × F_boundary`, and collision coupling links
pairs of plates → one dense `3P × 3P` linear system (P ≤ ~30), solved directly.

**Plate birth and death:**

- **Rifting:** a plume under a large plate splits it, after a delay, along a great circle
  through the plume. Mantle upwelling drags both halves apart and a new ridge forms.
- **Subduction initiation:** old oceanic lithosphere (≳ 150–180 Myr) at a passive margin (ocean
  and continent on the same plate) starts to sink: the plate is split along the margin and the
  oceanic part subducts under the continental part, with slab pull from the start.
- **Suturing:** two plates whose continents have collided for a sustained time merge into one.
- Plates without any crust left are removed.

This replaces the random plate reorganisations and the ad-hoc collision drag.

## Should the work be split? Yes — sequential milestones

Every step is built, tested, run (500 Myr, several seeds) and committed on its own, so
regressions can be attributed. They are sequential, not parallel: all of them change the core
tectonics code and build on each other, so parallel agents would conflict.

0. **Restructure** `tectonics.rs` (~1100 lines) into a module directory without behaviour change:
   crust/isostasy calibration, initial conditions, ownership resolution, and new files for
   dynamics, mantle flow and plate topology changes.
1. **Boundary analysis + torque-balance solver:** classify boundary cells (trench, ridge,
   collision, transform) from polarity and relative motion; slab pull, ridge push, basal drag,
   collision coupling; dense solver; remove random reorganisation and collision drag.
   Tests: a free plate on a moving mantle follows it; slab pull moves a plate towards its trench;
   collision coupling stops convergence; solver matches a hand-computed case.
2. **Mantle flow field:** downwelling memory from subducted slabs, plume life cycle, flow
   evaluation. Tests: flow points towards a downwelling and away from a plume; field is smooth.
3. **Plate topology operations:** split a plate by a great circle; merge two plates; remove dead
   plates with index remapping (owners, polarities, mantle). Tests: crust volume and sample
   counts conserved, ownership consistent.
4. **Events:** rifting above plumes, subduction initiation at old passive margins, suturing after
   sustained collision. Tests: each event fires in a constructed scenario.
5. **Calibration and diagnostics:** ocean-age statistics (median, 90th percentile, max), fraction
   deeper than 6 km, plate count, births/deaths; GDH1 depth-age curve (Stein & Stein 1992,
   asymptote 5651 m); tune drag, slab pull efficiency, event rates over several seeds.
   Targets: mean ocean age 50–90 Myr, max ≲ 250 Myr, < 5% of the surface deeper than 6 km,
   8–20 plates without a trend, mean plate speed 3–6 cm/yr, oceanic plates faster than
   continental ones.
6. **Independent review** of how well issue 1 is solved.
