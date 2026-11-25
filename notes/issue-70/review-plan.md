# Review of the plan for non-rigid plates (#70)

_2026-11-24, independent review of `notes/issue-70/plan.md` (commit ab359d7) against the code at
that commit (`src/tectonics/`). Read-only; nothing run. Goal: an Earth-like planet, coarse-grid
artefacts are accepted._

**Verdict.** The decomposition (rigid ω from the torque balance, plus a per-plate thin viscous
sheet `u` with basal drag) is a sound, standard choice, and it is much cheaper and less risky than
a global Eulerian sheet. The plan is not ready to implement as written, though. Five problems are
serious enough to change the design. In short: the rigid motion never feels the deformation,
so convergence is never really partitioned. The rigid/weak threshold locks samples in. Transport
is not zero inside blocks. Front cells feed wrong thicknesses into the physics. The runtime
arithmetic is off by about an order of magnitude. Fixes are proposed for each. I also suggest
delivering the work in stages (section E).

## A. High severity

### A1. The rigid motion does not see the deformation, so the convergence is not partitioned
Section 3 takes ω from `update_plate_velocities` unchanged and puts the other plate's `u` only on
the sheet's right-hand side (lagged). The decomposition itself is exact for linear forces. If
`K u − C u + f = 0` and the torque of `f` vanishes, then `Σ rᵢ × Cᵢ uᵢ = 0`, because central
springs carry no net torque. But this holds only when `C` (in the projection, too) is the full
diagonal: basal drag **plus** the plate's own diagonal shear, collision, belt and initiation
couplings. "The drag matrix of the torque balance" (`drag[plate]` in `dynamics.rs`) holds only
drag and slab resistance. With that matrix, the zero-rigid-part claim is false wherever couplings
act. **Fix:** project with exactly the `C` used in the sheet.

The physics problem is larger. The coupling between plates goes only one way. In a collision,
ω_India and ω_Asia are still set by the rigid coupling (initiation and belt plastic strength, locked
below `locked_convergence`). A weak Asian margin then shortens in response to that rigid
force, but India does not move faster because Asia is weak. Margin shortening is *added* to
the rigid convergence; it is not a share of a fixed total. On Earth the point of a weak orogen
is that it lets convergence continue. The plan's "convergence partitioned between the boundary
and the deforming zone" will happen only through front retreat (A4), at a rigid total rate.

**Fix (cheap):** in the torque balance, compute the relative boundary velocity in every coupling
term (shear, collision, nascent/initiation, belt, slab resistance) with the lagged `u` of both
sides: `v = ω × r + u_lagged`. That is, add `k (u_q − u_p)·…` terms to `rhs`. Then ω feels the
margin's softness one step late, which is consistent with the lag already accepted for `u_q`.
Also make `collision_resistance` and `belt_gpe_resistance` calibration knobs. They were calibrated
as stand-ins for the strength of the whole orogen, which the sheet now models (see B5).

### A2. Strain-rate-dependent viscosity plus the rigid threshold locks samples in as rigid
`B = B_strong s^p (ε̇_II/ε̇_ref)^(1/n−1)` uses the previous step's strain rate. A rigid sample has
`ε̇ = 0` exactly, so its next B is infinite, clamped to `B_rigid`, and it stays rigid. A
sample that once crosses into a block never leaves it, however weak it becomes (stretching,
plume heating, orogenic marking). **Fix:** classify rigid against the strain-rate-independent
part `B_strong s^p` at `ε̇_ref` (or at a floor `ε̇_min` ≈ 10⁻¹⁷ s⁻¹), use `max(ε̇, ε̇_min)` in the
power law, and add hysteresis (e.g. enter a block at `B_rigid`, leave below `B_rigid/3`) so that
blocks do not flicker step to step. Flicker matters for warm starts and for the split timer (B4).

The threshold also conflicts with the calibration metric. With `basal_drag` = 10¹⁵ Pa·s/m,
`ℓ = √(B/c) = 3000 km` gives `B ≈ 10²⁸` Pa·s·m. At 10¹³ N/m such a sample would strain at
~3 × 10⁻¹⁶ s⁻¹, not "< 10⁻¹⁶". With continental drag ×4, ℓ is half as long. So `B_rigid`
directly removes area from the ≥ 10⁻¹⁶ target class, and the "deforming area 10–20%" target
partly measures `B_rigid`. Choose `B_rigid` so that the predicted rigid strain rate is
≤ 10⁻¹⁷ s⁻¹ (B ≳ 10²⁹), and check the cost (more weak samples).

### A3. Transport is not zero inside blocks
Sections 5 and 9 say transport "does nothing outside [deforming zones] (u = 0 in blocks)". That
is false. In a block `u = W_b × r`, and only a plate that is a single block has `W = 0`. Almost
every oceanic plate has weak young sea floor at its ridge (with p ≈ 5, ocean younger than
~6–10 Myr falls below `B_rigid`). So almost every plate has a non-zero `W` on its main block,
and the face flux `½(uᵢ+uⱼ)·eᵢⱼ = W·(rᵢ×rⱼ)/|rⱼ−rᵢ|` is non-zero across the whole block. The
consequences:
- First-order upwind diffuses every coastline, the thickness, age (`birth`), felsic/orogenic and
  sediment fields of whole plates, continuously. At 0.5–2 cm/yr that is a diffusion length of
  ~25–50 km per 100 Myr, and more over 1 Gyr. Partial-cell fluxes also mix continental into
  oceanic columns, which flips `is_continental` / `resists_subduction` and blurs margins.
- The transport runs over ~all 41k samples every substep, not "only samples with u ≠ 0"
  (runtime, A5).

**Fix:** per plate, move the frame with its largest block. Use `ω' = ω + W_main` for
`move_plates`, and transport with `u' = u − W_main × r`. This is kinematically identical, the
main block's crust is then untouched exactly, and only weak zones and the (slow, < split speed)
minor blocks are advected. Keep the torque-balance ω for forces. Decide explicitly which ω
the boundary code uses (B1).

### A4. Front cells: cell-average thickness leaks into the physics; interactions with gap filling
Section 6 stores the column as a cell average, so a front cell is "thinner or thicker by up to
half". A grown sample gets `(a−1)/a = 1/3` of the column, so a 35 km continental margin produces
a ~12 km "column". Everything downstream reads `Crust::thickness` as physical thickness:
`is_continental`, `resists_subduction`, `is_continental_origin_column`, `breakup_thickness`,
polarity (`choose_overriding`, buoyant crust stays on top), the collision band's
`thickening_room`, isostatic elevation, erosion and the sea-level solve. A thinned continental
front can then be classified as subductable and be consumed (continental loss), or flip a
polarity segment. That is not just display noise. **Fix:** store `a` with the sample and make
every physical query use `thickness / a` (mass bookkeeping keeps `thickness`). Or keep
per-sample columns intact and move only the boundary: grow by splitting into two samples
of the same true thickness, with area fractions.

Gap-filling interactions that the plan does not cover (`resolve_ownership`, `heal_gap`):
- A vacated slot is `None` and not `consumed`. The lower plate's overlapped samples are consumed
  in the same substep they become hidden (`state = 2`), so there is usually **no hidden sample
  of the other plate to "advance"**. The cell becomes a gap. `opens_away` uses rigid ω only;
  at a converging boundary it is not spreading, so BFS order decides between India's consumed
  slot (`at_boundary` → shows the adjacent sample) and Asia's empty slot (`heal_gap`, not
  enclosed → shows `parent`). The end result is right one substep later, but a duplicate is
  shown in between, so specify and test it. Worse: at a *rigidly diverging* boundary a vacated
  cell gets **new sea floor** (mass created) or `stretch_rift`, even when the vacate came from
  sheet shortening.
- Fronts must be reset or folded whenever a sample stops being an edge sample. That happens
  through gap filling (new sea floor next to it), `stretch_rift`, `heal_gap`, splits and merges.
  Otherwise a sample with `a ≠ 1` becomes interior and its area excess disappears. Also, new
  sea floor filling a whole cell next to a front at `a = 1.4` double-counts 0.4 of a cell.
- Growing into a slot with `consumed = true` must clear the flag. Growth must not happen into
  a slot of the plate's own other part.
- New samples must also go into `TrackedCrust::samples`. Otherwise, a grown sample that is
  hidden on arrival is invisible to the hidden-sample tracking. Run `--verify-matching` with
  deformation on, in the tests.

### A5. Runtime estimate is off by about an order of magnitude
Section 9: "~5–10k samples … ~6 edges each, 30–100 iterations … at ~0.1 µs per edge term:
~10–40 ms". 10k × 6 edge terms × 0.1 µs is 6 ms per iteration (3 ms if each edge is counted
once), so 30–100 iterations take **90–600 ms**. That is 0.2–1.5× today's 0.41 s step. Block-Jacobi
PCG on this system is also optimistic. Weak springs are up to ~10³ × the per-sample drag
(B ≈ 10²⁸ against cA ≈ 1.2 × 10²⁵), the deformation length is ~30 cells, and the viscosity
spans 4–5 decades inside the weak set. Expect 100–300 iterations cold and maybe 20–50 warm.
Add to that transport over all samples (A3) and per-substep front bookkeeping.

**Fix:** prototype the solve on a frozen 6-subdivision state as the *first* implementation step
and measure it (iterations and ms, desktop and `--web-math` / wasm at 5 subdivisions) before
building transport and fronts on top. Have fallbacks ready:
(a) a sparse direct LDLᵀ per plate (e.g. `faer`, which builds for wasm32). A 2D-mesh system of
10–20k unknowns factors in milliseconds and is deterministic, with no tolerance to tune.
(b) Solve every k-th step (the forces change slowly).
(c) Edge terms precomputed in an SoA layout (assembled once per step; spring matrices are
3×3 outer products).
Answering Sebastian's "can the desktop still handle 6 subdivisions" needs this measurement
early, not at the end.

## B. Medium severity

### B1. All boundary kinematics stay rigid
`boundary_cells` (trench, ridge and collision classification), `update_segment_motion`, slab
growth (`update_slab_lengths`), `opens_away` (gap rules), `suture_colliding_plates` (stall
below 0.2 cm/yr), diffuse-zone `subducting`, and `polarity` all use `plate.angular_velocity`.
Where a margin moves several cm/yr by `u`, the boundary is classified with the wrong relative
velocity. For example, back-arc extension advancing a front towards a trench is invisible to slab
growth, and a sheet-converging contact can be counted as opening. **Fix:** a single
`cell_velocity(cell) = ω × r + u(sample)` used by all of these (consistent with A1 and A3).

### B2. Spring discretisation: anisotropy, wrong bulk viscosity, zero modes, hidden samples
- `k = (4/√3) B` makes an isotropic sheet only on an equilateral lattice. The icosphere has
  distorted triangles, especially around the 12 pentagons and along the icosahedron's edges, so
  the effective viscosity there becomes anisotropic and position-dependent, imprinting the
  icosahedral pattern on deformation zones. Central springs also give λ = μ (Cauchy relation).
  The thin sheet has λ = 2μ, so areal resistance is 2B instead of 3B (×1.5, not ×2 as stated).
- **Better and just as cheap:** constant-strain triangles built from the three edge elongation
  rates `(uⱼ−uᵢ)·eᵢⱼ / |rⱼ−rᵢ|`. These are exactly rotation-invariant (the same property the plan
  wants) and give the correct isotropic λ = 2B, μ = B on any triangle shape. Use only triangles
  with all three vertices in the plate.
- Zero modes: a block made of one sample (or of samples that all lie on one radius) has a spin
  mode `W ∥ r`, because the 3×3 block matrix is singular. Edge samples connected by a single
  edge (no triangle) have floppy transverse modes held only by drag, which hurts PCG. Hidden
  samples have no drag (forces "sit on the sample shown at the cell"). **Fix:** a minimum block
  size of ≥ 3 non-collinear samples (otherwise treat them as weak). Leave samples that are in no
  triangle out of the sheet and let them move with their neighbour's `u`. Add a small drag
  regularisation for hidden samples, and decide whether hidden samples awaiting consumption take
  part at all (I suggest not).

### B3. Rheology mapping is fragile; young sea floor and thick crust may become extremely weak
`B ∝ s^p` multiplies four factors before raising them to p ≈ 5. A fresh 70 km orogen gets
`(0.25 orogenic × 0.5 continental × 0.25 thickness)^5 ≈ 2 × 10⁻⁸` relative strength, and the
ridge axis gets `0.05^5 ≈ 3 × 10⁻⁷`, i.e. 7–8 decades of contrast before the power law. Young
sea floor is loaded by the distributed ridge push, by slab pull on young slabs, and by the
couplings. With a low `B_min` it will shorten or stretch substantially, changing oceanic crust
thickness (#141) and creating sea floor by stretching instead of by gaps. **Fix:** give each
term its own exponent (or work in log B with separate weights), set `B_min` from a physical
floor (e.g. ≥ 10²⁶ Pa·s·m), and make "oceanic crust thickness distribution unchanged within X%"
a hard guard rail, not just "measured".

### B4. Splits from block motion: identity, partition, oscillation
- Blocks are recomputed every step, so "sustained over `deformation_split_time`" needs a
  persistent block identity, e.g. by sample overlap with the previous step's blocks. Without
  hysteresis (A2) the timer keeps resetting.
- The partition "closest rigid motion per cell" cuts through the middle of the zone and gives
  ragged, disconnected pieces. `weak_zones` already has a minimum-cut splitter
  (`split_locally`). Use it with the two blocks as terminals and capacities from the current B
  (or 1/ε̇). That reuses tested code and gives clean boundaries.
- Oscillation risk: a converging split creates a slow continental contact, and
  `suture_colliding_plates` merges pairs that stall below 0.2 cm/yr for 30 Myr. That can loop
  split → suture → split. Add a cooldown (like `terrane_cooldown`), count split–suture
  cycles in the log, and let the converging case split only when its relative speed exceeds the
  suture threshold by a margin.
- After a split, rebase each piece's stored `u` by its block's `W` (new plate ω ≈ old ω + W_b),
  or the warm start and transport carry a spurious rotation.

### B5. Double counting that the plan leaves "for calibration"
- **Belt GPE twice:** `belt_resistance` (GPE of the thickened belt as a boundary force in the
  torque balance, hence in `f`) plus the new GPE body force. In the sheet, drop the belt term
  from `f` and let `−∇GPE`, including the difference across the boundary cell, supply it, as
  the plan does for ridge push.
- **`crustal_flow` + GPE:** both spread thick crust down its gradient. Decide before
  calibrating which process each represents (lower-crust channel flow vs whole-lithosphere
  collapse), and test with crustal flow at reduced diffusivity from the start.
- **`stretch_rift`** (fixed 200 km band at continental gaps) + sheet stretching: the same rift is
  thinned twice once the sheet stretches it before breakup. At least gate `stretch_rift` off for
  rifts that the sheet has already thinned (strain recorded in the crust), or reduce `rift_width`.
- The collision band (#69) is fine **only if** front retreat reduces consumption one-for-one
  (A4). Add a ledger check: band mass plus sheet shortening equals the rigid convergence of the
  interiors.

### B6. Positive feedbacks: localisation to the grid scale
n = 3 lagged, `orogenic += strain` (s drops to 0.25, so B drops ~10³ with p = 5), McKenzie
reheating on stretching, and the `(h₀/h)²` thickness weakening are all softening feedbacks.
With a one-step lag they can run away to `B_min` in a few steps and localise into one-cell
zones. That is physically "localisation", but the rate is set by dt. **Fix:** cap the change of
log B per Myr (not per step), and test that halving `--dt` gives the same strain-rate
distribution.

## C. Low severity
- **Ridge push from the age gradient** is noisy at fracture zones and transforms, where age
  jumps along strike give spurious lateral forces. Direct it along the smoothed ridge normal (or
  the gradient of a smoothed age) and cap it per cell.
- **GPE is zero below 35 km**, so there is no continent–ocean contrast (passive margins are
  stress-free). This is acceptable; name it.
- **Mixing `birth`** of oceanic crust by transport changes the ocean-age statistics and slab
  pull (√age). Keep the oceanic age mixing by youngest or by mass and say which.
- **The ridge-push torque mismatch** is removed by `−C(W × r)`, a fictitious drag-shaped body
  force. It is fine while small; log `|W|` per plate.
- **Warm starts across block changes:** initialise `W_b` by a least-squares fit of the previous
  `u` on the block's samples.
- **`enabled=0` bit for bit:** returning the converged state from `update_plate_velocities` must
  not change the order of floating-point operations. Add a state-hash test against `main`.

## D. Missing tests and diagnostics
- **Rigid limit:** with `B_strong → ∞`, the state equals `enabled=0` up to the ridge-push
  redistribution. This is the most important regression test.
- `Σ rᵢ × Cᵢ uᵢ = 0` per plate (with the full C), and the springs' net torque is zero.
- **Translation test** over a pentagon: uniform `u`, non-uniform h, mass and shape kept within
  the upwind error. Also check that a single-block plate with `W ≠ 0` (A3) does not diffuse its
  coastline with the frame fix.
- **dt-convergence** of the strain-rate distribution (B6). Determinism with `--web-math` and the
  PCG/direct solver (iteration counts appear in the log, and hence in the hashed state if they
  steer anything).
- **Diagnostics:** per snapshot, the share of convergence taken up by margin shortening vs
  boundary consumption per collision pair (the headline result of the issue), front
  vacate/grow counts with net area, splits by deformation and later sutures of the same pair,
  and the solver's ms and iterations.

## E. Simpler path: stage it
1. **Diagnostic sheet only** (solve, rheology, strain rates, log/cell dump; no transport). This
   is risk-free (`u` unused), measures the runtime (A5), and calibrates `B_strong`, `p` and
   `B_rigid` against the deforming-area and strain-rate targets with a fixed geometry.
2. Torque-balance feedback (A1) and per-cell velocities for the boundary code (B1), still
   without moving crust.
3. Transport in the largest block's frame (A3), with fronts using the `thickness / a` semantics
   (A4).
4. Splits via the min-cut with block terminals (B4).

Each stage can be compared with `main` and stopped if it does not pay off. Stage 1 alone already
answers the runtime question and whether the rheology produces Earth-like deformation zones on
this grid.
