# Plan: non-rigid plates (#70)

_2026-11-24. `main` at 4200b10. Goal: an Earth-like planet, not Earth; effects of the
~110 km grid are accepted (#64) and named where they matter. Sebastian's instructions
(2026-11-24): make the change, profile (can the desktop still handle 6 subdivisions?),
optimise, recalibrate the whole simulation, compare in detail, report the key takeaways._

## 1. What is wrong

Every plate is one rigid rotation. All relative motion happens on the one-cell-wide plate
boundaries; plate interiors cannot deform. On Earth, ~15% of the surface deforms in diffuse
zones that are hundreds to > 1000 km wide (Gordon 1998): continental interiors (Tibet, Tien
Shan, the western US, the Aegean), and slowly straining "rigid" oceanic plates (Central Indian
Basin). What the rigid model does instead is rule-based: collision bands (#69) spread the
stacked crust over 500 km kinematically, rifts stretch a fixed 200 km band, crustal flow
moves thick crust down its elevation gradient, and diffuse oceanic zones (#42) stack crust at
one cell. The relative motion is never partitioned: the whole convergence of a collision is
consumed at the boundary, and the plate behind it never moves differently from its own
interior. #43 found almost no slow continental boundaries (9% of the boundary length vs ~30%
on Earth); #123 found long narrow plates that never break, since nothing reacts to stress.

## 2. Approach: a rigid reference frame plus a viscous deformation field per plate

Velocity of the lithosphere at a crust sample `i` of plate `p`:

    v_i = ω_p × r_i + u_i

* `ω_p` is the plate's rotation from the existing torque balance (`dynamics.rs`, unchanged).
* `u_i` is a **deformation velocity**, tangent to the sphere, solved every step from the
  balance of forces on the plate's lithosphere treated as a **thin viscous sheet** (England &
  McKenzie 1982; Houseman & England 1986) with basal drag, and with a viscosity that follows
  the lithosphere's strength (rheology of crust and mantle lithosphere, section 4).
* By construction the drag-weighted rigid part of `u` is zero (section 3), so `ω_p` stays the
  plate's mean motion, and a stiff plate gets `u = 0` exactly: **rigid plates are the stiff
  limit of the new model**, not a separate model.

Crust then moves between samples of a plate by `u` (section 5), mass-conserving, with
thickening where the sheet converges and thinning where it stretches; plate edges move with
the material (section 6). Where the deformation separates a plate into parts that move apart
fast enough, the plate splits there (section 7): diffuse zones turn into discrete boundaries
by the physics, not by a rule.

Plates stay entities. They are the frames in which crust is stored (`Plate::crust`, indexed
by mesh vertex in the plate frame), and polarity segments, slabs, ownership resolution,
matching, gap filling, events and the viewer all key on them. Inside them, the crust now
deforms.

### Why not a fully Eulerian global velocity field (no plates)?

Considered and rejected:
* Plate boundaries are one cell wide. A continuous viscous solve would need viscosity
  contrasts of 10⁵–10⁶ between a ridge/trench cell and the plate beside it to keep plates
  rigid; that solve is badly conditioned (hundreds to thousands of CG iterations at 6
  subdivisions, i.e. several times the current step time), and trench/ridge kinematics
  (polarity, slab pull, consumption, sea-floor production) would have to be re-derived from
  strain rates. That is a rewrite of ~15 000 lines with every calibration lost.
* The decomposition used here is the standard one of plate kinematics (rigid rotation plus
  residual strain, e.g. block models in geodesy, Gordon 1998's "nonrigid plates"). Its
  physics is the same as a global sheet wherever the deformation is inside a plate; the
  difference is only that boundaries stay sharp where the model already has them.

## 3. Forces and the deformation solve

### Forces on each sample
For every world cell, the forces the torque balance applies there, **evaluated with the solved
ω** (so they come out of the existing code, not a copy):
1. basal drag `c_i A (v_mantle − ω × r R)` (continental ×4 as now);
2. slab pull at trench cells; slab resistance; shear coupling; collision, belt (GPE),
   initiation and diffuse-zone resistances with the converged couplings of the active-set
   iteration;
3. **ridge push distributed** over the young sea floor as the gradient of its potential energy
   (`F(age) ∝ age` up to `ridge_age`, Turcotte & Schubert §6-20), instead of the line force on
   the ridge cell: the axis of a ridge is the weakest lithosphere, and a line force there would
   crumple it; on Earth the force grows with age away from the ridge. Its net torque per plate
   equals that of the line force up to the lever arm; the difference is removed with the
   projection below. The torque balance itself keeps the line force (no change of the
   calibrated rigid motion).
4. **potential energy of thickened continental crust** (new body force): `−∇ΔGPE(h)`,
   `ΔGPE = ½ g ρc (1 − ρc/ρm)(h² − h₀²)` (the function `deformation::excess_potential_energy`
   of #69). It spreads plateaus and orogens (gravitational collapse, England & Houseman 1989)
   and resists their thickening. Internal only (its net torque is projected out; a GPE force in
   the torque balance stays a possible follow-up, as in #69).

These forces sit on the sample shown at the cell (samples shown at two cells get both). Their
sum over a plate has zero torque by the torque balance (exactly when the active set settled).
Any residual torque (unsettled solve, the ridge-push redistribution, GPE) is removed by
subtracting the drag of a rigid rotation `W × r` with `W` chosen to cancel it (the drag matrix
of the torque balance). So the deformation never moves a plate as a whole.

### The sheet
Unknowns: `u_i`, 2 components in a tangent basis per sample. Equation per sample:

    Σ_j k_ij ((u_j − u_i) · e_ij) e_ij  −  C_i u_i  +  f_i  =  0

* Viscous term: **central springs on the mesh edges inside the plate** (in the plate frame),
  `e_ij` the unit chord, `k_ij = (4/√3) B_ij`, `B_ij` the harmonic mean of the depth-integrated
  viscosities `B = η L` (Pa·s·m) of the two samples. On a triangular lattice central springs
  give an isotropic viscous sheet with shear viscosity `B` and bulk ≈ shear (thin-sheet: bulk =
  2 × shear; factor 2 accepted). Central springs in 3D chords are exactly invariant under rigid
  rotations of the sphere, so the operator's null space is the plate's rigid motions.
* `C_i`: basal drag `c_i A` (tangent projector), plus the diagonal part of the boundary
  resistances (slab resistance, collision/belt couplings, shear coupling) at boundary samples,
  with the other plate's `u` of the previous step on the right-hand side (lagged). The drag
  makes the system positive definite.
* Free edges (stress-free plate boundaries, loaded by the boundary forces).

### Rigid blocks (what makes it cheap)
Most lithosphere is strong (`B` ≥ ~10²⁹–10³¹: old ocean, cratons). A plain CG over all samples
with that contrast needs ~10³ iterations. Instead, samples with `B_i ≥ B_rigid` are **rigid**:
the connected components of rigid samples of a plate become **blocks** with 3 unknowns each (a
rotation `W_b`, `u_i = W_b × r_i`), and only the weak samples (expected 10–25% of the
lithosphere) keep their 2 unknowns. Edges inside a block carry no strain and are skipped. The
system (blocks + weak samples) is solved by preconditioned CG (block-Jacobi: 3×3 per block,
2×2 per sample), matrix-free, **warm-started from the previous step** (`u` and `W` per sample
and block, carried in the plate frame). `B_rigid` is chosen so that the deformation length
`ℓ = √(B / c)` of the weakest rigid sample (≥ ~3000 km) is many cells: a sample that stiff
would strain < ~10⁻¹⁶ s⁻¹ anyway. Cost estimate in section 9.

A plate that is all rigid is one block, gets `W = 0` exactly (zero net torque), and costs one
3×3 solve: today's behaviour.

### Strain rate
Per sample, the velocity gradient in its tangent plane is fitted by least squares to the
in-plate neighbours (`v = ω × r + u`; rigid rotations give zero strain), giving the strain-rate
tensor, its second invariant `ε̇_II` and the dilatation `ε̇_kk` (positive = stretching). Used
for the rheology (next), for marking deformation, for statistics and the cell dump.

## 4. Rheology: strength of crust and mantle lithosphere

`B_i = B_strong · s_i^p · (ε̇_II,i / ε̇_ref)^(1/n − 1)`, clamped to `[B_min, B_rigid]` (rigid
above), with

* `s_i` the relative depth-integrated strength (0–1) — the existing
  `weak_zones::lithosphere_strength` (thermal age with plume reheating, thinning of stretched
  continental lithosphere, remaining orogenic weakening; #75/#53) times two new terms:
  * **continental composition**: quartz-rich crust is weaker than olivine at the same
    temperature; a continental lithosphere of the same thermal thickness carries a fraction
    `continental_strength` (~0.5, to calibrate) of the oceanic strength (Kohlstedt, Evans &
    Mackwell 1995; Burov & Watts 2006). It also answers point 1 of #144 in the new model:
    young ocean is not weaker than a continental belt of the same thermal state.
  * **crustal thickness**: thickened crust puts its weak, hot lower crust where the strong
    mantle lid was; `(h₀ / h)²` above `h₀` = 35 km (70 km: 0.25). With the GPE force this
    lets plateaus spread instead of thickening without limit.
* `p` (≈ 4–6) maps the proxy to a viscosity: depth-integrated strength is dominated by the
  coldest part of the lithosphere and grows much faster than linearly with its thickness. The
  target contrast (Earth): cratons and old ocean ~10³⁰–10³¹ Pa·s·m (rigid within ~10⁻¹⁸ s⁻¹
  at ~10¹³ N/m), young orogens and plateaus ~10²⁷ (Tibet: ~5 × 10¹² N/m at ~10⁻¹⁵ s⁻¹;
  England & Molnar 1997), active rifts and ridges weaker.
* Power-law creep, `n = 3`, as an effective viscosity from the previous step's strain rate
  (one Picard step per time step, under-relaxed in log space): deforming zones soften and
  localise (shear zones, rifts).

`B_strong`, `p`, `continental_strength` and `n` are the calibration knobs (section 10).
`lithosphere_strength` itself is not changed (the splits of #75 still use it).

## 5. Moving crust with the deformation

Every substep, inside each plate (plate frame, mesh edges between its samples), a
**finite-volume upwind transport** of the columns by the face velocity
`½(v_i + v_j) · e_ij` (the rigid part cancels inside the plate) over the dual (Voronoi) face
of the edge, length from the circumcentres of the two triangles: the volume flux is
`h_up · u_n · L_ij · dt`, divided by the (uniform) cell area as everywhere else in the model.
Conserves crust and sediment mass exactly. Thickness changes by the divergence: shortening
thickens, stretching thins. A face flux is limited so that no sample gives more than half its
column per substep, and a receiver above `max_crust_thickness` takes only its room (as in
`crustal_flow`; nothing is lost at the cap). The transported mass mixes like the existing
mixing code: density, felsic, metasediment by mass; sediment and its cohort; orogenic
weakening by mass, `orogeny` and `reheated` by latest; the thermal age by the mass-weighted
thermal fraction (heat), as in `stretch_rift`.

Strain feeds back into the lithosphere's memory:
* **stretching** of continental lithosphere resets its thermal age by McKenzie's β (the rule
  `stretch_rift` already uses): stretched lithosphere is hotter and weaker;
* **shortening** of continental crust marks it orogenic in proportion to the strain
  (`orogenic += strain`, capped at 1; `orogeny` set once the strain exceeds the 1% of #69):
  deformed belts stay weak and heal with #75's 200 Myr.

The `u` field is constant through the substeps of a step (stored in the plate frame, so it
rotates with the plate).

The CFL number `|u| dt / d` is ≤ ~0.3 for `u` up to ~3 cm/yr at the default 1 Myr step and
0.5 cell substeps; the transport sub-cycles if it is larger.

**Grid caveat:** upwind transport smears gradients inside deforming zones over a cell or two;
it does nothing outside them (`u = 0` in blocks).

## 6. Plate edges move with the material

Without this, the boundary would still be consumed at the rigid rate while the margin behind
it shortens as well: a collision would be shortened twice. Each edge sample (a sample with a
mesh neighbour not of its plate) carries a **front position** `a` (fraction of a cell covered,
1 = the front at the cell's outer face). The normal component of `u` at the edge moves the
front: `Δa = Σ_external faces (u · n) L dt / A`.
* `a < ½`: the front has retreated past the middle; the sample is **vacated** — its column
  goes to its in-plate neighbours (by the inward flux direction, mass-conserving) and the slot
  becomes empty (not `consumed`: no crust was destroyed). The neighbouring plate then covers
  the cell (it advances into the space), or, if nothing covers it, the usual gap rules apply.
* `a > 3⁄2`: the front advanced past the middle of the outside cell; the plate **grows** there:
  a new sample in the outer slot takes the share `(a − 1)/a` of the column (both thinner by
  extension), `a` resets.

So a margin that shortens retreats from its boundary and the other plate is consumed less
(convergence is partitioned between the boundary and the deforming zone), and an extending
margin advances (trench retreat with back-arc extension). New samples join the incremental
matching (`matching::TrackedCrust`) directly; vacated ones drop out as consumed ones do. Front
cells show their column as a cell average (thinner or thicker by up to half); named as a grid
artefact.

## 7. From diffuse zones to plate boundaries

After the solve, the blocks of a plate move relative to each other. If two blocks of one plate,
each at least `min_plate_area`, move apart, together or past each other faster than
`deformation_split_speed` (~1–2 cm/yr, calibrated: Earth's diffuse zones move at 0.2–1.5
cm/yr, discrete boundaries faster), sustained over `deformation_split_time` (a few Myr), the
plate **splits** between them: each cell goes to the block whose rigid motion is closer to its
own velocity (the partition follows the deforming zone), pieces are then cleaned up by the
existing connectivity rule, and `split_plate_cells` does the rest (as for every split). An
extending zone becomes a ridge once the gap opens (sea-floor spreading by the gap rules; a
continental rift is stretched first by the transport itself); a converging one becomes a
diffuse convergent contact of #42 (polarity segment, not localised) and only builds a slab by
the existing initiation rules; a shearing one becomes a transform.

The existing events stay (plume rifts, subduction initiation, sutures, terrane accretion,
captures): this adds one more way to break a plate, the one that follows from stress. Expected
effect on #123: long narrow strips get stress concentrations in their necks and break when
those are weak (young sea floor); strong old strips still do not (no plastic yield of rigid
blocks — named).

## 8. What else changes, and what does not

* `update_plate_velocities`: unchanged physics; it returns its converged state (boundary cells,
  smoothed normals, couplings) so that the forces of section 3 are the torque balance's own.
* Collision band (#69), crustal flow, belt resistance, diffuse oceanic zones (#42), rift
  stretching: **kept** for the first version. The band places the consumed crust of the lower
  plate; the sheet deforms both plates. Overlap with crustal flow (both move thick crust) is
  checked in calibration (option: switch crustal flow off if the GPE force does its job).
* Mass ledger: new terms `Deformation` (transport; zero up to rounding) and
  `DeformationFront` (vacate/grow); the material budget gets a phase.
* Log: one new line per snapshot (`deformation: ...`): area by strain-rate class
  (≥ 10⁻¹⁶, ≥ 10⁻¹⁵ s⁻¹, oceanic/continental), deforming area of plate interiors (not at a
  boundary), weak samples and blocks, rms/max `|u|`, solver iterations and time, fronts
  vacated/grown, splits by deformation. Cell dump: `strain_rate`, `dilatation`, `u_east`,
  `u_north`.
* Viewer (`src/viewer/`): the plate-velocity layer (plate-motion mode) adds `u` live; the
  history keeps the plates' rotations (frames are not changed, so the frame format and codec
  stay as they are; history frames show the rigid part). PlateInfo, plate colours, the
  plates mode and the realistic mode's detail anchoring (plate orientation, #158) need no
  change: the plate frame still exists and moves with the plate's mean motion; the detail of a
  deforming zone does not follow its strain (sub-cell detail is decoration). No new render
  mode in this PR (a strain-rate mode is an easy follow-up if wanted).
* Determinism: all loops in index order, no hash-map iteration in results, transcendental
  math through `crate::math` (`powf`), so web = desktop with `--web-math`.
* Parameters: `deformation.*` via `--set` (`enabled`, `strong_viscosity`, `strength_exponent`,
  `continental_strength`, `stress_exponent`, `split_speed`, `split_time`, …);
  `--set deformation.enabled=0` restores today's model bit for bit (for the comparison and as
  an escape hatch).

## 9. Runtime estimate

`main` at 6 subdivisions: 0.41 s per Myr step (30 Myr headless, one core). Per step, new:
* assembly of forces, rheology, strain rates: a few passes over ~41k samples and ~120k edges:
  ~5–10 ms;
* PCG on blocks + weak samples (~5–10k samples, 10–20k unknowns, ~6 edges each), 30–100
  iterations warm-started at ~0.1 µs per edge term: ~10–40 ms;
* transport per substep (only samples with `u ≠ 0` and their faces): ~1–3 ms × substeps
  (usually 1–3).

Expected +5–20% per step at 6 subdivisions; the profile will tell. Budget: ≤ +25%, otherwise
optimise first (sparser assembly, fewer iterations, solving every second step).

## 10. Calibration

Knobs (deformation): `B_strong`, `p`, `continental_strength`, `n`, `B_rigid`, split speed and
time. Then the existing ones that the new model affects: collision band width, crustal flow
(on/off, diffusivity), suture threshold, possibly basal drag. Prefer physics over tuning; stop
when the model is in its plausibility bands.

Targets (Earth-like plausibility; sources in #43, #69, #75 notes):
* deforming area: ≥ 10⁻¹⁶ s⁻¹ on ~10–20% of the surface (Gordon 1998: ~15% diffuse
  boundaries), mostly continental and young oceanic; plate interiors otherwise ≤ 10⁻¹⁷ s⁻¹;
* continental deforming zones 300–2000 km wide behind collisions, strain rates ~10⁻¹⁵ s⁻¹,
  relative motion across diffuse zones 0.2–1.5 cm/yr; share of slow (< 2 cm/yr) and
  continental boundaries up (Earth 39% / ~30%; main ~13–20% / ~13–17%);
* thickness and relief kept in their bands: crust > 50 km 3–6% of continental crust, mean land
  ~0.6–1.1 km, plateaus ≤ ~70 km crust, highest points ≤ ~9 km;
* guard rails: plate count and size distribution, speeds (RMS ~4–5 cm/yr), trench/ridge/
  collision lengths, sea-floor production and age (mean ~60–70 Myr), continental area,
  collision stacking, mass ledger closed, sediment budget, climate summaries;
* #123: share of plates with long thin arms / elongation (measured, not a target).

Procedure: short sweeps (4 seeds × 300–500 Myr) over `B_strong`/`p`/`continental_strength`
first (deforming area and thickness), then split speed (plate count, slow boundaries), then
the band/crustal-flow overlap; full runs only for the final comparison.

## 11. Comparison (after calibration)

`main` vs branch, 8 seeds × 1000 Myr (window 500–1000 Myr), snapshots every 50 Myr with
`--cell-dump`, via `runs`. Metrics: plate count/size distribution and shape (elongation,
minimum width; #123), velocities, boundary classes with `notes/issue-43/sim_classes.py`
(diffuse/slow share, continental boundaries), strain-rate distribution and plate-interior
deformation, orogen widths (`notes/issue-69/scripts/belts.py`), relief/hypsometry, sea-floor
age, continental area, sediment budget, lakes/closed depressions, climate summaries, runtime;
PNG snapshots side by side. Plus the runtime answer at 4/5/6 subdivisions (headless Myr per
wall second, step time, and the viewer's frame budget on a virtual screen).

## 12. Tests

* Solver: a uniform stiff plate under balanced forces gets `u = 0`; a rigid rotation is in the
  null space of the springs; uniform extension of a weak strip gives the analytic strain rate
  of a sheet with drag (within ~20% on the mesh); blocks + weak samples give the same `u` as
  the full solve on a small mesh.
* Transport: mass of crust, sediment, felsic and metasediment conserved exactly; a uniform
  translation of a weak region does not change thickness (within the mesh error at
  pentagons); convergent flow thickens by `1 / (1 − strain)`; nothing is lost at the cap.
* Fronts: a retreating margin vacates cells and the other plate covers them; an advancing one
  grows; mass conserved; matching stays exact (`--verify-matching` mode in the existing tests).
* Collision partitioning: two continental hemispheres converging with a weak margin: the
  overriding plate's margin thickens over several cells and the lower plate's consumption is
  less than the rigid convergence.
* Splits: a plate made of two strong blocks joined by a weak strip and pulled apart splits
  along the strip; a strong plate does not.
* `deformation.enabled=0` gives the same state hash as `main`; determinism test; existing tests.

## 13. Risks

* Lagged couplings (rheology, other plate's `u`) oscillate → under-relaxation; the solve is
  once per step, so a small oscillation costs nothing but noise.
* Front vacate/grow noise at plate boundaries (thickness jumps of up to half a column at the
  front cell); measured.
* Many new splits (plate count up) → split speed/time calibrated against plate count.
* Young sea floor near ridges deforming under ridge push → the distributed ridge push starts
  at zero at the axis; measured (oceanic crust thickness distribution, #141).
* Runtime above budget → see section 9.

## References

Burov & Watts 2006 (GSA Today 16); England & McKenzie 1982 (GJRAS 70); England & Houseman
1989 (JGR 94); England & Molnar 1997 (Science 278); Gordon 1998 (Annu. Rev. Earth Planet.
Sci. 26); Houseman & England 1986 (JGR 91); Kohlstedt, Evans & Mackwell 1995 (JGR 100);
McKenzie 1978 (EPSL 40); Turcotte & Schubert, Geodynamics (ridge push). Numbers are textbook
values, cited from memory at abstract level.

## Revisions after the plan review ([`review-plan.md`](review-plan.md)) and the first prototype

The review's staging advice (measure the solve first) was followed: a prototype of sections
3–7 ran before the revisions below; its numbers decided several of them.

1. (high, rigid motion never feels the deformation) **Accepted.** The torque balance's
   couplings (boundary shear, collision, yielding/belt resistance) now act on the relative
   velocity of the *material*, rigid part plus the last solve's `u` on both sides (lagged, on
   the right-hand side and in the active-set tests), and boundary classification, segment
   rates and the trench tests use the material velocity too (`BoundaryCell::convergence`,
   `update_segment_motion`). The projection uses the sheet's own drag (per sample); the
   boundary resistances enter the sheet as forces evaluated at the solved velocities, not as
   implicit terms (a simplification: they are plastic at speed in the default model, so their
   force does not depend on the speed).
2. (high, rigid lock-in) **Fixed before it bit:** rigidity is decided from the
   strain-rate-independent viscosity `B_strong · s^p`; power-law creep only *softens* above the
   reference strain rate (capped at 5×), it never stiffens a slow sample. Viscosities are
   under-relaxed in log space between steps. The threshold `B_rigid = 10²⁸` is a modelling
   choice ("strain below ~3 × 10⁻¹⁶ s⁻¹ is not resolved"); the deforming-area statistics are
   reported at 10⁻¹⁶ and 10⁻¹⁵ s⁻¹ with that caveat.
3. (high, transport inside blocks) **Accepted** (was already planned in the prototype): each
   plate's frame follows its largest block (if that block holds ≥ 20% of the plate and is at
   least a minimum plate in size); only weak samples and smaller blocks move through the frame.
4. (high, front thickness) **Partly.** Fronts never store a cell average: the front position
   is a separate number; the column changes only by the fluxes between samples. A retreating
   front drains (thins) its edge sample before it is vacated (at `a < ½`), so edge samples of
   deforming margins can be thinner by up to about half; measured in the comparison (thin
   continental slivers at collision fronts). Vacated slots reset their front; slots that stop
   being edge samples reset to 1; growth clears `consumed` and registers the sample with the
   matching (`matching::track_sample`).
5. (high, runtime) **Measured on the prototype at 6 subdivisions:** after fixes, 0.53 s per
   Myr against 0.45 s on `main` over the first 100 Myr of seed 0 (+18%; solve ~50 ms,
   transport ~25 ms per step); PCG needs 30–80 iterations warm-started. Profiling and
   optimisation follow in `performance.md`.

Other changes from the prototype:
* single-sample blocks have a zero spin mode: a tiny spin drag (10⁻³ of a cell's drag) keeps
  the block matrices positive definite (without it, PCG diverged);
* least-squares gradients (ridge push, GPE) need ≥ 3 neighbours not on a line (a nearly
  collinear pair gave forces of 10²² N);
* samples with fewer than 3 neighbours in their plate are never weak (slivers moved at metres
  per year);
* oceanic lithosphere has a strength floor of 0.35 (`sheet.oceanic_min_strength`): newly
  formed sea floor at ridge axes (strength 0.05) took the boundary forces and moved at
  1–6 m/yr. The ridge axis is the plate boundary; young sea floor beside it does not deform
  internally on Earth (Gordon 1998). Oceanic diffuse deformation of old lithosphere (Central
  Indian Basin, plastic failure) is therefore not represented by the sheet; it remains the
  diffuse convergent zones of #42;
* lowest viscosity 10²⁷ Pa·s·m (Tibet ~5 × 10²⁷): with 10²⁶, weak rifts tore apart at tens
  of cm/yr.

Not adopted:
* constant-strain triangles instead of central springs (review, medium): the central-spring
  network is exactly rotation-invariant, symmetric and was verified positive definite by a
  debug check; its anisotropy on the icosphere is a grid effect of the same order as the
  mesh's own irregularity. Bulk/shear ratio 1 instead of 2 named as a caveat.
* belt GPE twice (review, medium): the torque balance's belt resistance is the force a belt
  transmits across the boundary; the body force spreads the belt inside the plate. Different
  forces of the same energy; the calibration checks thickness and relief.
* crustal flow and rift stretching are kept; calibration checks thickness, and they act at
  different places (crustal flow: across boundaries too; rift stretching: at diverging
  boundaries, the sheet inside plates).
