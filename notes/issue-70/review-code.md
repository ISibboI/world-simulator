# Code review: non-rigid plates (#70), `4200b10..d9ae225`

Independent review of the code in `src/` (mainly `src/tectonics/sheet.rs`, the feedback in
`dynamics.rs`, `events.rs`, `topology.rs`, the statistics, the cell dump and the viewer). This
review is from reading the code only. Nothing was run. Line numbers refer to `d9ae225`.

Overall the structure is sound. The CG operator is symmetric and positive definite as built:
springs and couplings are both quadratic forms `k((v_b−v_a)·e)²` and `k(v_b·d_b − v_a·d_a)²`,
blocks get the extra `1e-3` drag, and the preconditioner diagonal matches `apply`. The units
check out: m/s or m/s per unit radius inside the solve, rad/Myr in `PlateSheet::velocity`,
N·s/m for drag, springs and couplings, and N·m torques with `R` in `coupling_torque`. Transport
and moving fronts conserve mass, and the ledger records the rounding. The temperature change is
bit-identical, because the expression order and the kernel index `other + 30 − bin` match the
old code. I found no determinism hazards: `target` in `move_fronts` is only used for lookups,
`touched` is sorted, transcendental math uses `math::powf`, and `Instant` only feeds statistics.

## High

### H1. The coupling feedback on the plates' rotation is counted twice (net-torque removal)
`dynamics.rs` (new `coupling_torque` terms for shear and nascent couplings) puts the lagged
deformation Δu into the torque balance. ω_n therefore satisfies
`torque(F_rot(ω_n)) = torque(C·Δu_{n−1})`. `cell_forces` gives the sheet only the rotation part
of these couplings (`sheet.rs:596–606`, `616–626`). So for each plate, the net torque of the
forces is exactly the lagged coupling torque, not round-off. `deform_plates` then removes it
(`sheet.rs:801–815`) and applies the same couplings implicitly on u (`sheet.rs:875–913`). As a
result the rigid mode responds to the boundary coupling of Δu twice: once in ω_n (lagged) and
once in the rigid part of u (implicit), which the frame then moves into `angular_velocity`.
Summing the forces after the solve gives a total torque residual of ≈ `torque(C·Δu_{n−1})` per
plate instead of 0.

With `k_shear ≈ 0.5·1e15·2e5·1.1e5 ≈ 1e25 N·s/m` per boundary cell, comparable to one cell's
basal drag, the error is moderate. Nascent couplings (`strength / lock`) can be much stiffer
than drag, though, and a doubled lagged response with gain near 2 can make the step-to-step
iteration oscillate.

Fix: when the torque balance carries the feedback, remove only the remainder,
`torque(f) − Σ torque(C·Δu_lag)`. Alternatively drop the removal for these terms, or drop the
feedback from the torque balance and let the frame carry it. Add a test: after `deform_plates`,
the per-plate net torque of `f − D u − C Δu` (world frame, including the frame shift) is about
0 relative to the drag torque. The module comment "what is left is removed" (`sheet.rs:11`) is
no longer true.

### H2. Grown samples keep a stale deformation velocity that is never cleared
`move_fronts` sets `plate.sheet.velocity[n] = u` for a newly grown sample (`sheet.rs:1654`) but
does not add `n` to `sheet.active`. `deform_plates` only zeroes velocities listed in `active`
(`1328–1330`) and writes new values only where they are non-zero (`1337–1340`). If the new
sample ends up with u = 0 in later solves, it keeps the old u. That happens exactly whenever it
belongs to the main block, because `a×r − a×r` is exactly 0. Typical case: a margin grows and
then stiffens, or the plate stops deforming.

Consequences:
- `deformation_velocity` returns a phantom u. It feeds `BoundaryCell::convergence`, trench and
  ridge classification, `update_segment_motion`, sutures, display, the cell dump and the viewer.
- The torque-balance coupling (`deformation_velocity_absolute`) sees the same phantom value.
- In `transport_crust`, the skip rule `other != ZERO && n < local` (`1456`) assumes that a
  neighbour with a non-zero velocity is active. A face between an active sample and a stale
  sample with a smaller index is then never transported, and other faces use the stale velocity.

Fix: in `deform_plates`, write `velocity[local]` for every sample (zero included), or push `n`
onto `active` in `move_fronts`. Iterating `0..len` over the original length keeps the current
loop correct. Add a test: after `transport_crust` and a re-solve with `sheet.enabled` and stiff
plates, every non-active velocity is zero.

## Medium

### M1. Segment motion after the velocity solve mixes frames
At the end of `update_plate_velocities`, `angular_velocity` is the new ω, the drag-weighted
rotation without the frame. But `update_segment_motion(&boundaries, true)` (`dynamics.rs`
~576, and the code changed at ~240) adds `deformation_velocity`, which is relative to the
previous step's frame. The frame's rigid motion (`sheet.frame × r`) is missing, so segment
`rate`, `subducting` and `localised` (with `record = true`, it counts restarts) are judged with
a velocity that is off by the main block's motion relative to the plate's mean. For a strongly
deforming plate that can be cm/yr. Every other user is consistent: `boundary_cells` runs before
the solve, with ω including the frame plus u relative to the frame, and calls after
`deform_plates` see the frame already moved into ω.

Fix: use `deformation_velocity_absolute` in `update_segment_motion` when it is called from the
velocity solve, or call it after `deform_plates`.

### M2. Thickening and thinning from advection are taken as strain (orogenic marking, McKenzie heating)
`transport_crust` (`sheet.rs:1519–1535`) labels a sample "stretched" or "shortened" from
`new.thickness / old.thickness`. Upwind advection of a thickness gradient changes thickness
without any strain. One example is a weak region drifting uniformly through its frame, which
the `moving_fronts_conserve_mass` test sets up. Crust downstream of thicker crust is then marked
orogenic (`orogenic += 1 − 1/ratio`, `orogeny = time`). Crust downstream of thinner crust is
reheated (younger `birth`, so weaker). At a continental margin, where thickness changes from
35 km to 7 km over one or two cells, any tangential u produces large spurious ratios.

Fix: take the strain from the divergence of u, using the stored `sheet.dilatation · dt` per
substep, or from the face fluxes as `Σ_out − Σ_in` of the volume fraction rather than of
thickness.

### M3. Sheet state is not reset for crust created by paths other than merges and fronts
`PlateSheet::reset` is only called in `merge_plates_inner` and `move_fronts`. Other paths write
crust into a local vertex that previously held different crust, with no reset: new sea floor in
gap filling (`boundaries.rs:649`), `heal_gap` and enclosed fills (`boundaries.rs:1241`, `1330`),
the suture crust (`1548`), and `topology.rs:484`. Two kinds of stale state follow:
- `viscosity` (never cleared for vanished samples) feeds the under-relaxation
  `sqrt(old · target)` (`sheet.rs:748–754`). New sea floor at a slot once held by weak
  continental crust gets `sqrt(1e27 · 3.7e28) ≈ 6e27 < rigid_viscosity` and is weak for a step.
- `front` keeps the previous occupant's value. A stale 1.5 (the clamp at `1643`) makes the new
  sample grow at once. A stale value near 0.5 makes it vacate at once.

Fix: in `deform_plates`, reset every sample that was not a sample at the last solve (keep a
`had_crust` bitset in `PlateSheet`), or call `reset` at those sites.

### M4. Panics with out-of-bounds indexes if an owner points at a local vertex without crust
`sheet.rs:781` (`w.index[owner.local]` → `u32::MAX` → `w.force[4294967295]`) and `899`
(`w.node[i]`) assume that every owner's local vertex holds crust. The redirect chain in
`move_fronts` and the test (`1945–1948`) keep this invariant for vacates. A violation anywhere
else, such as merge destinations or erosion paths, now panics with an unhelpful index error deep
in the solve, where `surface_crust` used to give a clearer one. Suggest
`debug_assert!(i != u32::MAX, …)` with context at both sites, or skip that owner with a counted
warning in release builds.

## Low

- **L1. `pending_split` and `split_timer` are cloned into children by other splits.**
  `split_plate_inner` clones the whole `sheet` (`topology.rs:292`). `rift_above_plumes`, which
  runs before `split_deforming_plates` in `plate_events`, and initiation splits therefore copy a
  pending side list and the timer into the new plate. `split_deforming_plates` iterates over the
  new indices as well and may split the rift child along the parent's old line. Fix: clear
  `pending_split`, `split_timer` and probably `frame` on the clone. Also, a failed split (piece
  too small) does not reset the timer, so the split is retried every step. That is harmless but
  costs a full owners scan per step per plate.
- **L2. Block-speed measure includes coupling nodes.** `block_edge` (`sheet.rs:1221–1226`) takes
  every `Node::Rigid`, including the boundary-coupling nodes added at `900–907`. Those lie on the
  plate's outer edge, so `fastest` measures relative block speed far from the blocks' contact and
  overestimates it against `split_speed`. Only count rigid nodes adjacent to a weak sample.
- **L3. Strain-rate degeneracy guard is absolute.** `xx.determinant().abs() < 1e-6`
  (`sheet.rs:1293`) has units of m⁴ with entries ~1e10. In practice it never triggers, so samples
  with 2 (nearly opposite) neighbours give a huge g. Use a relative test as in `gradient`
  (`det < 0.025·tr²`) or require `count >= 3`. These strain rates drive softening (capped by
  `max_softening`) and the area statistics.
- **L4. Shear force placed on a different cell than the coupling.** `cell_forces` puts the
  reaction of shear and nascent forces on `other_cell` (the first neighbour of the other plate,
  `570–576`). The implicit coupling and the torque balance use `boundary.neighbor`. Use
  `boundary.neighbor` and drop the search.
- **L5. Hidden samples are part of the sheet with drag but without forces or mantle drag in the
  torque balance.** They also count towards `neighbours >= 3` and receive transported crust. That
  may be intended (the slab stays attached), but it shifts the drag-weighted rotation removed at
  `801–815` relative to the torque balance, which counts only owned cells. Document it, or
  exclude hidden samples from the drag weights of the removal.
- **L6. Vacated slots are not marked `consumed`.** If gap healing later sees the world cell as
  this plate's gap away from other plates, it fills it with new sea floor. That is correct for
  extension but not for a margin that retreated under shortening. Consider
  `consumed[l] = true` when the retreat was convergent.
- **L7. Collision coupling is explicit and lagged in `cell_forces` (`612–614`) while the shear
  and nascent couplings are implicit.** It is off by default (`collision_resistance = 0`) and
  stable at the documented magnitudes (coupling 50·drag is far below spring stiffness
  ~1e28 N·s/m). Mention it in the docs or make it implicit like the others.
- **L8.** The `"Fast plate check"` trace block in `Simulation::step` (`mod.rs:1042–1058`) is
  leftover diagnostics with an O(plates × cells) scan. Remove it before the PR. The cell dump
  (`main.rs:1424`) converts with `EARTH_RADIUS` rather than `params.planet_radius`; check whether
  the other dump columns do the same.
- **L9. Minor performance.** `deform_plates` builds a `cell_count`-sized `index` and a full
  `0..cell_count` scan per plate per step, and `transport_crust` allocates three
  `cell_count`-sized scratch vectors per substep. Both are fine at 41k cells and ~20 plates. If
  profiling shows them, keep them in `Simulation` as scratch.

## Tests worth adding
1. A torque-closure test (H1).
2. Stale velocity after grow-then-stiffen (H2).
3. A uniform-drift transport test asserting that `orogenic` and `birth` stay unchanged (M2).
4. Running `--verify-matching` for a few hundred Myr with `fronts = 2`. Grown samples go through
   `track_sample`, so the matching invariants after grow and vacate deserve a run-time check.
