# Plan: shear coupling across plate boundaries (issue 1q)

_2026-11-16. User decision: boundary friction next, before thickening (1r) and issue 2._

_Status: implemented; modest effect, see issue 1q in `../ISSUES.md`._

## Problem

Neighbouring plates move independently: median relative speed across boundaries 6.6–7.0 cm/yr,
only 2–11% of boundary length slower than 2 cm/yr, relative motion randomly oriented to the
boundary, ~90% of the boundary ridge or trench (Earth ~45–50%). Sea-floor production is ~2×
Earth (kinematic 6.4–7.3 km²/yr vs ~3.4) and the ocean too young (median 28–35 Myr vs 45–55),
although boundary length and spreading rate per ridge are Earth-like (evaluation of 3af40dc).
The torque balance (`dynamics.rs`) couples plates only at continental collisions (normal
component, active set); the transform friction of the issue 1 tuning plan was dropped.

## Physics

A plate boundary is a weak shear zone through the lithosphere. Strike-slip motion along it
(transform faults, oblique ridges and trenches, collisions) is resisted: viscously, a zone of
width `w` and viscosity `η` through lithosphere of thickness `H` gives a force per unit boundary
length `f = (η H / w) Δv_t`. With η = 10¹⁹–10²¹ Pa·s, H ≈ 100 km, w ≈ 10–50 km, `η H / w` spans
2·10¹⁹–10²² Pa·s, i.e. 20–10,000 km times the basal drag coefficient (10¹⁵ Pa·s/m): poorly
constrained, so it is calibrated. Opening at ridges stays free (weak, hot), closing at trenches
keeps slab and collision resistance; only the along-strike component is coupled.

## Design

- **Coupling per mesh edge across a boundary:** for every mesh edge (c, n) with owners p ≠ q,
  at r = normalize(c + n): boundary normal n̂ = tangent part of (n − c), strike t̂ = r × n̂,
  m = r × t̂. The along-strike relative velocity is `((ω_p − ω_q) × r) · t̂ R = R m·(ω_p − ω_q)`.
  The force on p is `−k ((v_p − v_q)·t̂) t̂`, on q the opposite, giving the torques
  `−k R² m mᵀ (ω_p − ω_q)` and `+k R² m mᵀ (ω_p − ω_q)`. Assembled symmetrically into the dense
  system (blocks pp, qq += K; pq, qp −= K with K = k R² m mᵀ): no net torque, symmetric matrix.
- **Strength:** `k = boundary_coupling_width · basal_drag · L_e`, with `L_e` the boundary length
  per crossing edge (Voronoi edge length `d/√3` for the hexagonal mesh, corrected so that the
  summed length matches the zig-zag-corrected boundary length used in the statistics), and
  `boundary_coupling_width` (m) the parameter: each km of boundary resists strike-slip like the
  basal drag under that many km² per km. Same for all boundary kinds (one parameter) unless
  calibration shows a need to distinguish (e.g. weaker at ridges).
- **No active set needed** (linear in ω, always acting); collision resistance unchanged.
- **Mantle flow:** unchanged (coupling is between plates only).
- **Diagnostics:** distribution of relative speed across boundaries (median, fraction < 2
  cm/yr), fraction of boundary length that is ridge/trench/other, mean |cos| of relative motion
  vs boundary normal, total shear-coupling dissipation vs slab-pull power (sanity).

## Calibration (seeds 0–2, 500 Myr, means 200–500 Myr; `--set`)

Sweep `boundary_coupling_width` ∈ {0, 100, 300, 1000, 3000} km. Targets: production → ~3.4–5
km²/yr kinematic (≤ ~110 Gt/yr), median ocean age → 45–55 Myr, relative-speed median ↓ and a
fraction of slow (< 2 cm/yr) boundary length ↑, mean plate speed stays 3–6 cm/yr, plates 8–25,
continental mass trend within ±10%. Check the chosen value against the η H / w range. Hold-out
seed 3 and subdivision 5 for resolution independence. Runtime must not grow noticeably (one more
pass over boundary edges per velocity solve).

## Verification

- Unit tests: two plates sliding past each other along their boundary are slowed (relative
  strike-slip speed decreases with coupling); opening normal to the boundary is not resisted
  (a pure spreading configuration has the same velocities with and without coupling); the sum of
  coupling torques is zero (symmetry); coupling strength per boundary length is the same at
  subdivisions 4 and 5.
- Runs: as above; also compare collisions (convergence at collision cells) and mean land
  elevation, since coupling should slow collisions too.

## Not in scope

Orogenic collapse and rift stretching (1r), numerical over-production by gap filling (1b),
breakup threshold changes.

## Plan review (independent agent, 2026-11-16): sound with changes — all adopted

1. Per-edge strike directions leak 25–37.5% of the coupling into the boundary-normal direction
   (opening would be resisted, ±8% with orientation): use a smoothed boundary normal per
   boundary cell and pair (3 rings, centroids as in `opens_away`) and assemble per boundary
   cell, half from each side (pp, qq += ½K; pq, qp −= ½K); with t̂ = r × n̂, K = k R² n̂n̂ᵀ.
2. Length per boundary cell = `boundary_length_per_cell()` (zig-zag corrected), halved for the
   two sides; test strike-slip resistance per true km at s4/s5 within ±5% and normal leak < 5%.
   (Slab pull and ridge push use the uncorrected cell length — 10% high per true km; noted.)
3. Shear coupling only damps Δv·t̂; production falls only through rigid coupling or slower
   plates. First an A/B with one moderate W; obliquity classification with smoothed normals and
   the relative-speed histogram as diagnostics. If opening does not fall, the levers are the
   relative-speed magnitude (slab sinking speed, mantle traction, plate count), not coupling.
4. Physical bound: oceanic W ≈ 100–300 km (≈ 20 MPa × 15 km brittle depth at 5 cm/yr), upper
   bound ~1000 km (Forsyth & Uyeda 1975: transform resistance small); do not calibrate beyond.
   Diagnostic: shear dissipation vs slab-pull power (a few %).
5. Separate strengths: `boundary_coupling_width` for oceanic/mixed boundaries; tangential
   coupling at collision cells tied to `collision_resistance` (always on).
6. `--set` entry; coupling blocks built once outside the collision active-set loop.
