# Review of the plan for #123 (breakup by internal stress)

_2026-11-26, independent review of `notes/issue-123/plan.md` and commit e068172
(`src/tectonics/breakup.rs`). Read-only; one numerical check of the capacity formula (below), no
simulation runs._

**Verdict.** The idea is sound and answers Sebastian's question: the stress across a section of a
rigid block follows from statics, and a rigid-plastic section check is a cheap, physically
defensible breakup criterion without any shape rule. The load assembly is correct (no sign or
frame bug found). But the capacity formula as implemented **almost never sees bending**, which is
the very mechanism the plan relies on for necks and U-arms (finding 1). Fix that before any
calibration. Findings in order of importance.

## 1. The overload ignores bending (must fix) — `breakup.rs:354-377`

`overload_of` checks only one axis, `â = T/|T|`: `S = |T| / D(â)` with
`D(ω) = Σ F s ℓ |ω × r_e|`. The set of torques a section can carry is the convex set whose
support function is `D`; the load is admissible only if `ω·T ≤ D(ω)` for **every** `ω`, so the
true overload is `S* = max_ω ω·T / D(ω) ≥ S`. Checking `ω = â` alone is necessary, not sufficient.

This matters a lot on the sphere: a torque about the planet's centre splits, at the section's
centroid `c`, into a tangent part (the net force across the section, lever arm 1) and a radial part
(the in-plane bending moment, lever arms ~ section length `L ≪ 1` rad). Unless the net force is
almost zero, `â` lies nearly in the tangent plane and `D(â) ≈ F s L`, so `S` reduces to
**|net force| / (F s L)**, a tension/shear check. Numerical check (straight section, uniform `s`):

| case | code `S` | exact `S*` |
|---|---|---|
| L = 0.08 rad, tension at 50% of capacity + moment at 75% (on the plastic interaction curve) | 0.50 | 1.00 |
| L = 0.08, tension 30% + moment 91% | 0.30 | 1.00 |
| neck 400 km, arm 2000 km loaded by a shear at its tip (shear 5% of capacity, M = V·arm) | 0.055 | 0.96 |

So for the U/L arms and strips of #123 the code underestimates the overload by ~an order of
magnitude; the "first evidence" (S = 2–7 at the neck of plate 475) is the net force (slab pull
along the arm's trench, ~1e13 N/m × trench length) exceeding `F s L`, not the bending of the neck.
Also the module doc (`breakup.rs:18`) calls `C` a "lower bound on the load needed to break it"; it
is the load of one kinematic mechanism, i.e. an **upper** bound on the collapse load, so `S*` (once
fixed) is a lower bound of the true overload — the right direction for a breakup criterion.

Fixes, cheap enough for the level scan:
* Exact: maximise `ω·T / D(ω)` over the unit sphere (quasi-concave: `D` is convex and
  1-homogeneous). Start at `â`; a few Newton/subgradient or Nelder–Mead steps in 2 parameters
  (tangent plane of `ω`) converge; ~20 evaluations × the section's edges per level is still small.
* Closed form for nearly straight sections: in the frame of the section centroid compute the net
  force `P` (tangent part of `T`), the moment `M = T·c`, `N_p = Σ F s ℓ`,
  `M_p = Σ F s ℓ |x_e − x_0|` (`x` along the section, `x_0` the strength-weighted median = plastic
  neutral axis); for a rigid-plastic section `(P/(S N_p))² + |M|/(S M_p) = 1`, i.e.
  `S = (m + √(m² + 4p²))/2` with `m = |M|/M_p`, `p = |P|/N_p`. Good for the scan, exact max for the
  chosen section. Level sets around a source in a compact plate are strongly curved; use the exact
  form there.
* Add a unit test: a straight strip with a tip load, compare `S` with the beam value
  `V·a / (F w²/4)`.

After the fix, overloads of arms will rise sharply; any calibration of `F` done before it is void.

## 2. Compression and weak orogens: break–suture churn (high risk)

The traction limit is isotropic (`|t| ≤ F s`), and the most overloaded section may be loaded in
compression. Collision resistance and continental drag (×4) put large compressive loads across
continents, and orogenic belts have `s` down to `SUTURE_WEAKENING` = 0.25 (`weak_zones.rs:41`)
(also the plate's own sutures). Expected failure mode: a continent breaks along a young suture in
compression → the new boundary is a stalled collision → `suture_colliding_plates`
(`events.rs:1776`) merges it again → it breaks again. Compressive failure of a continent is real,
but it is the sheet's job (thickening/shortening of weak lithosphere), not a new plate.
Suggestions: (a) record the sign of the normal traction across the section (e.g. the sign of
`T·Σ ℓ_e (r_e × n_e)`) and the crust type along it for every break; (b) use Byerlee's asymmetry
(compressive strength ~2–3× tensile) or break only sections in net tension/shear; (c) measure the
re-merge rate (children sutured or captured within 20 Myr) as a must-pass metric.

## 3. Strength used here differs from the sheet's — `breakup.rs:186` vs `sheet.rs:498-515`

Breakup uses `lithosphere_strength` (floor 0.05, young ocean ~√(age/100): 0.1 at 1 Myr, 0.3 at
10 Myr). The sheet uses `sheet_strength`: oceanic floored at `oceanic_min_strength` 0.35 and
thick continental crust weakened by `(h_ref/h)²`. So young sea floor is up to 7× weaker in the
breakup than in the sheet, and the min-cut refinement (`breakup.rs:513`, capacity `ℓ·s`) will be
drawn to young ocean next to ridges and behind arcs. That gives ridge-parallel slivers / ridge
jumps (real processes, but they should be measured, not accidental). Either use one strength
(state the reason if not) or measure the share of breaks whose section lies in ocean < 10 Myr.

On the scaling: `F·s` linear in mechanical thickness is a defensible first order, but the
integrated strength of a frictional (Byerlee) lid grows ~`h²` (stress ∝ depth), i.e. ∝ age for
`h ∝ √age`; the linear form understates the old/young contrast. Fine to keep, but say so.

**Range of F.** 5e12–5e13 N/m is a fair literature range (old oceanic lithosphere in tension
~1–5e13 from integrated yield-strength envelopes; Buck's ~1e13 for rifting normal continental
lithosphere without magma, ~3e12 with dyking). Compare with the model's loads: transmitted slab
pull is `slab_pull · buoyancy − resistance · v_n` (`dynamics.rs:625-694`), i.e.
`2e13 · (1 − v_n/10 cm/yr)`, ≈ 1e13 N/m at 5 cm/yr — at the top of Earth estimates of the net
pull reaching the plate (most of the gross pull is balanced locally by the slab's own mantle
resistance). Basal drag 1e15 Pa s/m gives ~1.6 MPa at 5 cm/yr (Earth estimates ~0.1–1 MPa),
×4 under continents: a 3000 km continent moving 5 cm/yr relative to the mantle collects
~2e13 N/m. So the model's loads are at the high end and `F` will tend to be calibrated upward to
compensate; any `F` outside ~5e12–5e13 means the loads, not `F`, need attention. Report the load
budget (pull/drag/ridge push/collision shares of `|T_A|`) at broken sections.

## 4. Section search — `breakup.rs:414-480`

Sound for strips and U shapes (BFS from the tips gives sections across the arms, `{d < c}` is a
connected BFS ball). What it misses:
* **Trench-parallel and boundary-parallel sections** of compact plates (the most likely failure of
  a compact plate whose one side is pulled): level sets from three boundary points are arcs
  centred on those points. Cheap additions with the same prefix-sum scan: multi-source BFS from the
  plate's trench cells, from its ridge cells and from each neighbour's boundary; and planar sweeps
  (sort by `r·n` for ~6–12 directions; for these check that the chosen part is connected).
* **Disconnected far side.** From a tip of a Y- or T-shaped plate, `{d ≥ c}` beyond the junction is
  two arms; the code checks the combined section (correct statics for `A`, but weaker than the
  separate checks of each arm) and, if that side is the smaller one, splits off two disconnected
  pieces (`breakup.rs:219-240`), each possibly < 1% → captured by `separate_detached_pieces`.
  Check connectivity of the split-off part and use per-component sections.
* The min-cut minimises total `ℓ·s`, not overload; that is acceptable as a refinement because the
  overloads are compared afterwards (`breakup.rs:535-547`). The network spans the whole piece
  (`breakup.rs:500-516`); restricting it to the band plus two super-nodes saves time on big plates.

Alternative worth one validation run (not as the default): a second sheet solve with every sample
weak (`η ∝ s`) gives a statically admissible, full stress field; comparing its `|τ|/(F s)` map with
the section overload map at seed 0, 403 Myr, would show whether the section search finds the
right places. Too expensive per step, cheap as a one-off check.

## 5. Load assembly — correct, with small notes (`breakup.rs:168-189`, `373-408`; `sheet.rs:773-783`)

* Frames/units: `sheet.velocity` is rad/Myr in the plate frame relative to the new frame
  (`sheet.rs:1270-1380`); `plate.rotation * u` plus `(ω_new − ω_old) × r` (= `frame × r`) is the
  material velocity relative to the rotation `cell_forces` used; ×`R/s_per_Myr` gives m/s. The
  drag coefficient matches `cell_forces` (`sheet.rs:563-571`). Torques `r × f` (N per unit
  radius) and capacity `F R face s |â×r|` (N) share the factor R: consistent. Only `|T|` is used,
  so the side choice is sign-free (and `T_A = −T_B` after balancing).
* Removing the net torque with a drag-weighted rotation is reasonable, but log its size
  (`|T_res| / Σ|r × load|` per plate): it absorbs the lagged deformation coupling
  (`sheet.rs:796-828`), ridge push spread vs. at the ridge, GPE (if not in the torque balance),
  the dropped smaller pieces and hidden samples. If it is large for some plates, their overload is
  partly fictitious drag.
* Coupling forces on the deformation velocity are not in the loads (only the rotation part is in
  `cell_forces`); fine for rigid plates, slightly off at deforming margins.
* The analysis runs only inside `deform_plates`, i.e. not with `sheet.enabled=0`
  (`sheet.rs:683`); document it or warn.

## 6. Breakup mechanics — `breakup.rs:212-241`, `556-596`

* Determinism: plates in index order, cells ascending, BFS and Dinic deterministic, tie-breaks by
  index, no hash maps: fine. Switches off: `cell_forces(&deforming)` as before and the extra
  forces of non-deforming plates are never added to `w.force` (`sheet.rs:783-794`), so the
  hash-identity claim is plausible.
* The timer follows the plate's *current* most overloaded section, which may jump between sections;
  5 Myr of overload anywhere then breaks wherever the maximum is at that step. Either require the
  same section (overlap of the small side ≥ ~70% with the previous step's) or, more physically,
  accumulate damage: weaken the lithosphere (e.g. via `orogenic`-like weakening) along overloaded
  sections, so the sheet localises the deformation and the existing sheet split takes over. The
  latter also unifies rule (3) and this rule.
* The timer resets on a single step with `S ≤ 1`: noisy `S` near 1 can postpone breaks
  indefinitely; consider decay instead of reset.
* Identity: the larger part keeps id/colour — fine. Cells grown after the analysis go to the
  larger part; sizes are rechecked (note `min_cells` uses `ceil` in the analysis, not in
  `break_overloaded_plates`, `breakup.rs:160` vs `557`; harmless).
* `max_plates` (40): breakups compete with rifts/initiations for slots; measure the time spent at
  the cap.
* Order in `plate_events` (`events.rs:349-356`): sutures and captures run first, then rifts,
  sheet splits, breakups; `split_plate_cells` clears both parents' pending breakups
  (`topology.rs:294-306`), merges clear the receiver's (`topology.rs:350-352`): consistent.
* Cost: `largest_piece` and `balanced` allocate mesh-sized vectors per plate per step
  (`breakup.rs:269`, `409`); fine at 40 plates but trivial to reuse.

## 7. Calibration and measurements

* Calibrate only after finding 1 (and decide finding 2); otherwise `F` will be tuned against a
  net-force criterion.
* The target "narrow plates broken within tens of Myr" should be stated against Earth history, not
  as fast as possible: Farallon was a long strip for a long time and fragmented when the
  Pacific–Farallon ridge reached the trench (~30–20 Ma), where young, buoyant slab cut slab pull
  along part of the trench — exactly a differential-load case (plus the Galápagos plume for
  Cocos–Nazca, 23 Ma). Kula/Izanagi similar. So: check that model breaks of strips coincide with
  strong along-strike variation of the trench load (young slab, ridge approach), and report the
  narrow-episode durations with Earth's tens of Myr as orientation.
* Earth comparison at the same threshold: PB2002 has ~14–15 plates ≥ 0.5% (incl. Arabia,
  Philippine Sea, Caribbean, Cocos); Juan de Fuca-sized fragments (~0.05%) are below `min_area`
  and the grid — say so on the issue.
* Add per-break records (time, parent/child ids, child area, `S`, section length, mean `s`,
  continental share, tension/compression, ocean age along the section, the new boundary's type
  after 1/5/10 Myr, child lifetime/re-merge), the residual-torque size (finding 5), and the share
  of breaks by part shape (narrow arm vs compact).
* Watch collision length, continental fragment count and the continental mass balance: breaks
  through continents create new margins (sediment, initiation hazard), which feed the sediment and
  trench metrics of the earlier issues.
