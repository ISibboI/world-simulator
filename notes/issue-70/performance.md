# Performance of the deforming plates (#70)

_2026-11-25. Question (Sebastian): can the desktop still handle 6 subdivisions after the change?
Make sure the code is optimised; profile even if the runtime does not increase._

## Answer

**Yes.** At 6 subdivisions the branch runs at the same speed as `main` or slightly faster:
the deformation adds ~5% to a step, and an unrelated optimisation found by the profile (the
thermal equator of the temperature model, bit-identical) saves ~8%. Details and the criterion
below.

Criterion: wall time per simulated Myr in a headless run on one core (the simulation is
single-threaded; the 3D view runs the same simulation on a background thread and only renders
the finished frames, so the viewer's frame rate does not depend on the simulation, see
"Viewer"), paired runs started at the same time on the same machine (tktie, shared with other
jobs, so absolute numbers vary by ±10%; the pairs see the same load).

SUBDIVISION_TABLE

## Where the time goes (profile)

`perf` cannot be used in the container (`perf_event_open` is blocked: "No permission to enable
cpu-clock event", also for user-space-only events), so the profile is from a sampling profiler
in the process (pprof-rs, SIGPROF at 997 Hz) in a scratch build with debug symbols
(`agent-scratch/prof`, not part of the branch), seed 0, 300 Myr, 6 subdivisions:

| part of a step | share of the run time |
|---|---|
| tectonics step | 36% |
| … of it: deformation solve (`deform_plates`, incl. forces) | 5.2% |
| … of it: transport of crust and moving fronts (`transport_crust`) | < 0.6% (not resolved) |
| … ownership resolution and matching | 10.6% |
| temperature model update | 21.9% (before the fix below) |
| … of it: thermal equator | 8.1% |
| erosion | 18.2% |
| … of it: drainage (priority flood, binary heap) | 9.6% |
| water cycle (climate) | 14.0% |
| sea level solves (flooding the ocean, 2–3 per step) | 8.5% |

The deformation's own accounting (`deformation:` log line, seed 0, 300 Myr): 8.0 s in the
solve and 0.9 s in the transport over 300 steps, i.e. ~27 ms + 3 ms per step of ~0.5 s.

### Optimisations done
1. **Deformation solve** (first prototype 60–120 ms per step → ~27 ms):
   * loops over each plate's samples instead of all local vertices of every plate (the plate
     arrays have one slot per mesh vertex; scanning 41k × ~25 plates of 96-byte `Option<Crust>`
     several times per step was memory-bound);
   * plates without weak samples are skipped entirely (no forces, no unknowns);
   * rigid blocks: strong lithosphere is one rotation per connected block, so only the weak
     ~10% of samples are unknowns (3–6k samples, ~100 blocks);
   * warm start from the last step's velocities (relative to the plate's mean rotation),
     40–50 conjugate-gradient iterations per step at a relative residual of 10⁻³;
   * node velocities computed once per operator application;
   * viscosity relaxation by a geometric mean (`sqrt`) instead of `powf`/`log10`.
2. **Transport** (first prototype ~35 ms per step → ~3 ms): only samples with a deformation
   velocity and their faces; the mass ledger is updated from the changed samples instead of
   summing all plates twice per substep.
3. **Thermal equator** (`temperature.rs`, not part of the deformation): the Gaussian weights of
   the coarse cells per sector of longitude (an `atan2` and an `exp` per cell, recomputed for
   each of 12 months × 24 sectors × 2562 cells every step) and the smoothing kernel are now
   computed once. Bit-identical (same state hash as `main` with `--set sheet.enabled=0`),
   saves ~8% of the run time.

### Promising further optimisations (not done; independent of #70)
* **Drainage priority flood** (`erosion::drainage`, ~9%): the binary heap dominates. If its key
  is a total order (elevation, then cell), a faster queue (bucketed/radix heap, or the
  "priority-flood + plain queue for flats" variant of Barnes et al. 2014) gives the same order
  and could halve it.
* **Sea level solves** (`sea::ocean_flood`, ~8.5%): the ocean mask is flooded 2–3 times per
  step (after the tectonics, after the dynamic topography, after the lakes); the first two could
  share one flood, or the flood could be incremental (most of the mask does not change).
* **Temperature model** (~14% after the fix): the per-cell interpolation from the coarse mesh is
  the bulk; it could run on fewer fine cells (land only for some fields).

## Viewer

The 3D view draws the same mesh with the same vertex data as before; the only new per-frame
work is adding the deformation velocity to the plate-velocity layer (one rotation and addition
per cell when the plate-motion mode is shown). Checked on a virtual screen (software
rendering, plate-motion mode, 4 subdivisions): the view runs and shows the deformation. Frames
are rendered independently of the simulation thread, so the frame budget is unchanged; the
simulation's pace (simulated Myr per second in the view) follows the table above.

## Memory

Per plate and mesh vertex the deformation keeps a velocity (24 bytes), front position, strain
rate, dilatation and viscosity (4 × 4 bytes): 40 bytes per slot, ~1.6 MB per plate at 6
subdivisions (the crust array is ~4 MB per plate). With ~25 plates, +40 MB.
