# Plan: issue 1 tuning — plate birth/death balance and ocean production

_2026-11-15. After the issue 1 mechanisms, their soundness fixes, and issues 5, 3, 4 (clean
bookkeeping, mass ledger, elevations), issue 1 still misses its targets (500 Myr, 3 seeds,
commit 5395931/293a882): mean ocean age 31–37 Myr [Earth ~64], plates 25–30 (cap 30 hit),
births 52–76 vs deaths 34–63 (mostly passive-margin initiations), max plate speed 28–54 cm/yr
[≲ 20], new sea floor 280–340 Gt/yr [~70], trench 150–190k km vs ridges 250–360k km [55k / 65k],
continental area 30% → 24–26%. Research (`../research/feasibility-2026-11-15.md`): a stable state
needs state-dependent birth/death hazards; trench propagation along old passive margins is the
cheapest high-impact addition; slowing of continental plates is real; fix with trench length,
not faster continents._

## Changes

1. **Subduction initiation as an age hazard** instead of one event per fixed interval:
   each old oceanic cell at a passive margin (oceanic and continental crust on the same plate)
   has a probability per Myr `h(age) = h_max · ((age − 80) / (180 − 80))²` for 80–180 Myr,
   `h_max` above (backstop), scaled per margin cell so the global rate follows the amount of old
   margin. At most one initiation per plate per step (splits renumber plates).
2. **Trench propagation:** the hazard is multiplied (e.g. ×10) for margin cells within a few
   cells (~300 km) of an existing trench of the same oceanic plate, so trenches extend along old
   margins (Lesser Antilles, Scotia).
3. **Transform/boundary friction:** at boundary cells that are neither ridges nor trenches
   (transform faults, locked contacts), a coupling like the collision resistance but weaker
   (e.g. 5× basal drag of a cell) opposes relative tangential motion. Microplates on Earth are
   slowed by their boundaries; currently a tiny plate with a trench along its edge is driven by
   slab pull against only its small basal drag. Added to the linear torque system (couples
   plates).
4. **Calibration** (only now, after the mechanisms are sound and the bookkeeping is clean), in
   this order, with seeds 0–2, 500 Myr: friction coupling → max speed; hazard `h_max` and
   propagation factor → plate count and age distribution; basal drag → mean speed; arc and
   subduction-erosion rates → continental mass trend (ledger). Targets: mean ocean age 55–75 Myr
   (median ~45–55), max ≲ 250 Myr, 8–20 plates without trend, births ≈ deaths, mean speed
   4–6 cm/yr, max ≲ 20 cm/yr, new sea floor within 2× of Earth (~70 Gt/yr), continental mass
   trend < ±10% per 500 Myr.

## Verification

- Unit tests: hazard rises with age and is zero below 80 Myr; propagation raises it near a
  trench; friction slows a tiny plate driven by slab pull; friction does not act at ridges.
- Runs: time series of the targets for 3 seeds; area–age histogram (triangular?); trench and
  ridge length; birth/death by mechanism; ledger rates.

## Not in scope

Plate splitting geometry (issue 10), ridge push as a body force, sea level (issue 2).

## Plan review (independent agent, 2026-11-15): sound with changes — to adopt when implementing

- **R1** Ridge length is double-counted (both flanks are `Ridge` cells); trench counted once.
  Real comparison ~125–180k km ridge vs 150–190k km trench. Also measure the mesh zig-zag
  factor (boundary cells × edge length on a known great circle, probably 1.1–1.3).
- **R2** Production diagnostic first: kinematic production Σ (one flank) opening rate × edge
  length vs booked new sea floor. If booked ≫ kinematic, over-production is still numerical
  (finding 1b). Make production (km²/yr, Earth ~3.4) the primary target; mean age ≈ ocean area /
  (2 × production) for a triangular distribution.
- **R3** Trench propagation as a hazard multiplier only makes more great-circle splits (more
  plates, more boundary). Instead: (a) merge the split-off continental part into the overriding
  plate of the nearby trench at once, or (b) split along the actual continent–ocean margin
  (connected region; pulls part of issue 10 in). Do one or drop item 2.
- **R4** Fast small plates: add **trench-local slab resistance** (viscous resistance of the
  sinking slab + bending; Forsyth & Uyeda 1975, Conrad & Hager 1999): at trench cells
  τ = −k_t ((v_p − v_mantle)·n) n, block-diagonal, no active set; k_t ≈ 50 cells of basal drag
  so full slab pull gives ~8–10 cm/yr. Transform friction optional and weak.
- **R5** If friction is kept: apply tangential coupling at all boundary cells, assembled
  symmetrically per cell pair (the current one-sided pattern, also used for collisions,
  leaves a spurious net torque).
- **R6** Hazard per unit margin length, p = 1 − exp(−H dt), calibrated via an interpretable
  waiting time; Earth's 180 Myr Atlantic margins have not subducted, so it must be slow; test
  rate and resolution independence (icosphere 4 vs 5).
- **R7** Splits append (no renumbering); merges renumber. Avoid stale candidates: one global
  Poisson draw per step + weighted pick; log saturation; exclude plates born in the same step
  (`birth_time`); no logic depending on `HashMap` iteration order.
- **R8** Raise `max_plates` (e.g. 40) and log when hit; calibration must not lean on the cap.
- Notes: moving the onset to 80 Myr enlarges the candidate set → start `h_max` low; old interior
  ocean far from margins is consumed only at trenches (diagnose where > 180 Myr crust sits;
  maybe a fracture-zone backstop); bending resistance limits old, not small slabs.
- Calibration: iterate once (drag rescales everything), time means over 200–500 Myr, spread
  across seeds, hold out seeds 3–4 and one 1 Gyr run, one-parameter sweeps; report the 99th
  percentile of area-weighted speed / RMS speed of plates > 1% instead of max cell speed.
  Diagnostics: production, corrected lengths, area–age histogram, plate-size distribution,
  lifetimes, births/deaths per 100 Myr by mechanism, ΣH, propagated trench length, substeps,
  run time.
- Optional: ridge push as a body force; delay full slab pull of a new margin trench;
  cache `boundary_cells()` once per step (recomputed 4–5 times).
