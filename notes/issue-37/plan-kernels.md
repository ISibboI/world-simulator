# #37 option 2: physical dynamic-topography kernels — plan

Sebastian chose option 2 of the [feasibility notes](feasibility-2026-11-27.md): replace the
slab part of the dynamic topography (three Gaussian depth stages with hand-set responses
0.5 / 0.4 / 0.2) by surface kernels of a radially layered viscous mantle (Hager & O'Connell
1981 propagator), with the decrease of thermal expansivity with depth, and no free decay or
calibration factor. Depressions are expected to get ~10–50% deeper until #32 (slab flux ~1.6–2×
Earth's) is fixed.

## What stays

- Slab bookkeeping in `MantleFlow::step`: slab excess mass (kg) on the slab grid, handed on
  stage to stage with exponential residence times (mass-conserving). The masses themselves are
  unchanged: excess mass of the cooled lithosphere from the depth–age curve
  (`dynamics.rs::sinking_slabs`).
- Plume swells (Gaussian, `plume_swell_*`) and the zero global mean. #195 edits the plume part;
  this change keeps to the slab part so the merge stays easy.
- `mantle.topography_response` stays as a comparison switch (1 = physical, 0 = off).

## Kernels

`K_l(d)`: fraction of the weight of a degree-`l` load at depth `d` that the surface supports by
deflection, for an incompressible, radially layered Newtonian sphere with free-slip surface and
CMB, no self-gravitation (the model's `h = −σ_rr / (ρ_m g)` convention, dry). Propagator of the
poloidal state `y = (u_r, u_θ, r σ_rr/η₀, r σ_rθ/η₀)`, `dy/d ln r = A(l, η(r)) y` (matrix as in
`kernels_prototype.py`, verified there against the analytic Stokes solutions).

- **Computed at startup in Rust**, not tabulated by hand: the kernels follow from the
  parameters (viscosity layers, expansivity profile, stage depths, planet radius), so a changed
  profile needs no external script.
- **Numerics:** RK4 in `ln r`, step ≈ `1/(8(l+3))` in `ln r` (as the prototype), layer
  boundaries hit exactly. Two solution columns integrated up from the CMB (`u_r = 0`,
  `σ_rθ = 0`) and two down from the surface, **re-orthonormalised (Gram–Schmidt) after every
  step** so high degrees stay well conditioned (the prototype had no such guard and stops at
  l = 50). One upward and one downward pass per degree store the column pair at every sample
  depth, then each sample depth is a 4×4 solve with the load's stress jump. Degrees 1…`l_max`
  (`l_max = 100`, enough for the 300 km taper below: taper weight 10⁻⁵ at l = 100). Cost
  estimate: ~10⁷ flops in total, a few ms; recomputed only when the planet radius changes.
- `exp` / `ln` through `src/math.rs`; everything else plain arithmetic, so desktop, web and
  `--web-math` agree as before.

### Viscosity profile (relative to the upper mantle; only ratios matter)

| depth | η / η_UM | basis |
|---|---|---|
| 0–100 km lithosphere | 100 | Hager & Richards 1989 |
| 100–300 km asthenosphere | 0.1 | weak channel of post-glacial-rebound and geoid models, e.g. Steinberger & Calderwood 2006 |
| 300–660 km | 1 | |
| 660–2891 km lower mantle | 30 | Hager 1984, Hager & Richards 1989; later inversions 30–100× (Mitrovica & Forte 2004) |

This is the middle variant of the feasibility table. Its kernels lie between the plain
lith/UM/LM 30× profile (stronger) and LM 100× (weaker). Kept as a parameter
(`viscosity_layers: Vec<(bottom depth m, relative viscosity)>`), not tunable from the command
line.

### Thermal expansivity

A slab's excess mass is `∫ρ α ΔT dV = (α/c_p) ∫ρ c_p ΔT dV`: the heat deficit is conserved
(conduction only spreads it; radiogenic warming ~0.1 K/Myr is negligible), ρ cancels, so the
excess mass scales with `α(d)/α₀` (c_p ≈ constant). `α₀ = 3.1 × 10⁻⁵ K⁻¹` is the lithosphere
value implicit in the depth–age curve (GDH1) from which the slab mass is taken. Profile,
piecewise linear: 3.1 (surface) → 2.0 (660 km) → 1.45 (1500 km) → 1.0 × 10⁻⁵ K⁻¹ (CMB)
(Katsura et al. 2009 adiabat; Steinberger & Calderwood 2006; from memory, ±20%). It enters the
stage kernel as a factor, so the slab masses stay the conserved quantity and the expansivity
only changes how strongly they load the surface.

### Stages

Mass in a stage is taken as spread evenly over its depth range; the stage kernel is the mean
of `K_l(d) α(d)/α₀` over 8 sample depths in the range.

| stage | depth | residence | |
|---|---|---|---|
| 1 | 100–660 km | 13 Myr | 5 cm/yr (as now) |
| 2 | 660–1200 km | 45 Myr | 1.2 cm/yr (as now) |
| 3 | 1200–2000 km | 67 Myr | (as now) |
| 4 (new) | 2000–2891 km | 74 Myr | 1.2 cm/yr; the physical kernel at l = 2 is still ~0.1–0.3 here, so "below 2000 km the surface hardly feels them" is replaced by the kernel; mass leaving stage 4 rests on the CMB, where K = 0 exactly, and is no longer tracked |

`SlabStage { residence, top, bottom }` replaces `{ residence, response, width }`.

## Spatial domain: Green's function tables

Per stage a surface Green's function for a unit point mass,
`G_s(θ) = −Σ_{l=1}^{l_max} (2l+1)/(4π R² ρ_m) K_{s,l} e^{−l(l+1)σ²/2R²} P_l(cos θ)`,
tabulated on 2048 points uniform in chord length `|r − p| = 2 sin(θ/2)` ∈ [0, 2] and read
with linear interpolation (one `sqrt`, no transcendental call at runtime). `σ` =
`slab_load_width` (300 km): the lateral extent of a slab load (the upper-mantle slab spans
~660 km horizontally at 45° dip; lower-mantle slabs thicken to 200–400 km and fold into piles
of several hundred km, van der Meer et al. 2010, 2018). It is a load shape, not a response
factor; it also removes the ringing of the truncated sum. `l = 0` is absent (mass
conservation of the mantle), so the slab field has zero mean by construction.

Evaluation: for each evaluation-grid cell (subdivision 4, 2562 cells) and each slab-grid cell
with mass in any stage: one dot product, one `sqrt`, one table index, and a lerp per stage
from an interleaved table. No cut-off radius: the long wavelengths are global. Cost per step
≈ 2562 × (occupied slab cells) × ~10 ns.

- Slab grid at subdivision 3 (642 cells, ~890 km): ≤ 1.6 M pairs, ≤ ~15 ms per step, ~15 s
  per 1000 Myr run (~2% of a ~750 s run).
- Subdivision 4 (2562 cells, ~450 km): ≤ 6.6 M pairs, ≤ ~60 ms per step (~8%).

The current Gaussians (σ ≥ 600 km) hide the ~890 km spacing of the slab grid; the narrower
upper-mantle Green's function may not. Decision rule: measure the along-trench ripple of the
unit-test trench; keep subdivision 3 if the ripple is < 10% of the depression, else go to 4.
(Spectral evaluation was considered: `(l_max+1)² × (N_slab + N_eval)` ≈ 3–30 M operations per
step, no cheaper, more code.)

## Obsolete or changed parameters

- `SlabStage::response`, `SlabStage::width`: removed (replaced by kernels and the depth range).
- New: `SlabStage::{top, bottom}`, `MantleParams::{viscosity_layers, expansivity,
  cmb_depth, slab_load_width}`.
- `topography_response`: kept as a switch; documentation says it is no longer a calibration.
- `slab_offset`, `slab_memory`, `slab_grid_subdivisions` unchanged (the latter maybe 3 → 4,
  see above).

## Tests

- Kernel unit tests: `K → 1` for a load just below the surface, `K → 0` at the CMB; half-space
  limit at high degree (`(1 + kd) e^{−kd}` for uniform viscosity, l = 40, d = 330 km);
  constant-viscosity whole-mantle l = 2 value against the prototype; stage means of the default
  profile against the prototype's table (lith/asth/LM30: l = 2 stage 1 ≈ 0.87, stage 2 ≈ 0.72,
  stage 3 ≈ 0.54, without α).
- The existing 5000 km trench test (`slabs_depress_and_plumes_lift_the_surface`) updated:
  deepest point on the slab line between −1.7 and −2.4 km dry; plume swell unchanged; smooth
  at cell scale; mean zero; slab mass ≈ flux × total residence (now four stages).
- Ripple test along the trench line (see decision rule).
- Determinism: `--web-math` run hash equal between two runs; the existing determinism tests.

## Evaluation

8 seeds × 1000 Myr (`runs submit --seeds 0-7 --args "--duration 1000 --snapshot-interval 50
--cell-dump"`) on the branch and on main (same base commit); analysis with
`analyze_check_2026-11-26.py` (extended): deepest point per seed and per snapshot (dry, and
×1.45 under water), percentiles, area below −0.5/−1/−1.5 km and where it lies (land,
flooded continental crust, ocean), sea level, land fraction, hypsometry (ocean depth and land
height percentiles), mean |h| on continents, run time. Independent evaluation by a second agent.
Results to `results-kernels.md`.

## Risks

- Deeper depressions (expected, Sebastian accepted): more flooded continental crust, sea-level
  shifts through the zero-mean field. #21's cover budget and #34's sediment result depend on
  the amplitude; report, not fix.
- Wider (global) response: the low degrees carry a long-wavelength tilt (degree 1–2) of the
  whole surface over slab graveyards, ~100s of metres. Physical, but visible in hypsometry.

## Changes after the review ([review-plan-kernels.md](review-plan-kernels.md))

- Degrees start at **l = 2** (degree 1 depends on how the core is held, i.e. on the frame).
- **Slab grid subdivision 4** (= evaluation grid), each pair of cells visited once: subdivision 3
  widened the loads by its tent spreading and made the peak 15–21% shallower than converged.
  Worst case 46 ms per step.
- Runge–Kutta resolution `6 (l + 3)` steps per unit ln r (converged to < 1e-5).
- The prototype's layer-boundary sampling was fixed (its layered kernels were ~1% off).
