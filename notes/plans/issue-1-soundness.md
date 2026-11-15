# Plan: soundness fixes for the plate dynamics (issue 1, findings 1c–1i)

_2026-11-15. From the independent evaluation of c449e2f (see `../ISSUES.md`, issue 1). The
aim is to make the mechanisms physically sound, not to calibrate. Issue 5 (gap filling, deferred
consumption of overridden samples) is done first and addresses the flicker part of 1a/1b._

## Fixes

1. **Trenches only where the plates converge (1c).**
   - In `boundary_cells` (dynamics.rs), check for a ridge (diverging, young oceanic crust) before
     the trench check, and classify a cell as a trench only if the pair's polarity says this
     plate subducts **and** the plates converge there (relative normal velocity > 0).
   - **Slab pull grows with the slab length**: track per plate pair the length of slab that has
     subducted (time-integrated mean convergence rate along the pair's trench cells, capped at
     ~600 km, decaying when convergence stops), and scale slab pull by `min(length, 600 km) /
     600 km`. A new subduction zone thus starts with little pull and grows, instead of pulling
     at full strength at once. Stored in `Polarity` (renumbered by the topology code already).
   - Ocean–ocean subduction initiation then also works at pairs that have a polarity but no
     convergent trench cells? No — keep it to pairs without polarity, but polarities of pairs
     that stop converging are forgotten after `polarity_memory` (already).
2. **Rifts don't become trenches (1d).** Oceanic subduction initiation requires the boundary
   cells considered to be converging (relative normal velocity > a small threshold). A fresh
   rift has zero relative velocity in the step it is created and diverges afterwards.
   Rifting by plumes is restricted to plumes below continental crust or below very large plates
   (> 15% of the surface), as continental break-up is what plumes are known to cause.
3. **Split plates start without collision history (1e):** `split_plate` resets
   `collision_time` of inherited polarities to 0.
4. **Suturing counts only converging continental contact (1f):** collision cells count towards
   `collision_time` only where the plates converge.
5. **Thinned continental crust is not old ocean (1g):** add `Crust::is_oceanic()` (normal
   oceanic crust: thickness below ~12 km); ocean-age statistics and subduction-initiation
   candidates use it instead of `!is_continental()`.
6. **Mantle kernel without antipodal phantom sinks (1h):** replace `1/(2π sin θ)` with the
   Green's function of a point source on the sphere with uniform return flow,
   `cot(θ/2) / (4π)` = `sin θ / (4π (1 − cos θ))`, softened within the core radius.
7. **Tiny plates merge (1i):** each step, plates smaller than 0.3% of the surface merge into the
   neighbouring plate with the longest shared boundary (micro-plates are captured by their
   neighbours on Earth, and these tiny plates reach 20–57 cm/yr).
8. **Minor:** the collision active-set loop iterates until it converges (with a cap); remove the
   useless regularisation or make it meaningful; update the module docs of `tectonics/mod.rs`.

## Verification

- Unit tests: diverging boundary with a polarity is not a trench; slab pull of a new subduction
  zone grows over time; split resets collision time; kernel points away from a source at all
  angles (no reversal beyond 90°); tiny plates are merged.
- Runs (seed 0 at subdivisions 6 for 500 Myr; seeds 1–2 at subdivisions 5): trench length
  fraction and slab age at trenches (diagnostic), plate count without hitting the cap, births vs
  deaths, mean/median ocean age and the area–age histogram, max plate speed, continental area,
  run time.

## Not in scope here

Calibration (production rate, birth rates), trench propagation and age-hazard initiation, and
extensional collapse of thick continents — after issues 3 and 4.

## Plan review (independent agent, 2026-11-15): sound with changes

Fixes 3, 6, 8 right as written; required changes adopted:

- **R1 Meaning of a polarity.** Polarities are created at every overlap, so nearly every
  neighbouring pair has one. After fix 1 a polarity only says *who would subduct if the plates
  converge* (set by buoyancy); a trench is polarity + convergence + slab state. Converging
  oceanic pairs then become trenches automatically (induced initiation), so
  `initiate_oceanic_subduction` becomes inert → remove it (spontaneous age-hazard initiation
  comes later).
- **R2 Hysteresis.** After a passive-margin split both halves have the same ω, and the
  classification uses last step's ω. Trench rule: polarity exists, this plate is the lower one,
  its crust is oceanic, and either it converges faster than a small threshold or it has a slab
  (L > 0) and is not clearly diverging (normal velocity > −ε). L grows only from convergence.
  Passive-margin initiation seeds L ≈ 150 km (induced pre-existing slab).
- **R3 Slab length L per pair:** L += mean convergence · dt (capped 600 km, ~upper-mantle slab,
  Conrad & Lithgow-Bertelloni 2002); exponential decay with τ ≈ 10 Myr without convergence; reset
  to 0 when the polarity flips; updated once per step from the same boundary set as
  `subduction_rates`, before `plate_events`. No steady-state form (positive feedback).
- **R4 Suturing** counts contact that is not diverging (normal velocity > −ε), since collision
  resistance drives convergence to ~0; decay `collision_time` instead of resetting it.
- **R5 Oceanic class:** thickness < 12 km is not enough (subduction erosion floors at 7 km) →
  also require oceanic density (> 2850 kg/m³) until issue 4 adds provenance/thermal age;
  statistics divide by the oceanic count, not `cells − continental`.
- **R6 Kernel test:** the old kernel also points away from the source; its defect is the speed
  rising again towards the antipode. Test flux through the circle at θ = Q(1 + cos θ)/2,
  monotonic decay to 0 at the antipode, near field unchanged. Softening
  `sin θ / (1 − cos θ + (1 − cos c))`. Far-field drive halves → slower plates (calibration later).
- **R7 Acceptance criteria:** every trench cell non-diverging; trench vs ridge length plausible
  (Earth ~55k vs ~65k km); global slab pull 0.5–1e21 N within a factor of 2; plate count below
  the cap; max speed ≲ 20 cm/yr; no suture within 30 Myr of a split; compared to a baseline of
  c449e2f + issue 5 with the same seeds.

Recommendations adopted: S2 (rift restriction tests continental crust within a radius of the
plume), S3 (tiny plates merge into the neighbour with the smallest relative velocity along the
shared boundary, longest boundary as tie-break; smallest first; counted separately; log lost
continental volume), S4 (active set up to 20 iterations with a log on non-convergence;
regularisation scaled by `basal_drag`), S5 (ridge/trench order irrelevant once disjoint).
Deferred and noted: S1 split geometry (flood-fill split at passive margins, shortest cut for
plume rifts), S6 ridge push as a body force.
