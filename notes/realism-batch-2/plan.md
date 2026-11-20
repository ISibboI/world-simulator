# Batch 2 plan: buoyant young lithosphere and resistance to subduction initiation

_2026-11-20. Measurement: [`measurement.md`](measurement.md). Issues: #42, #32 (in part)._

## What is wrong

1. **Slab pull is `slab_pull × √(age / 100 Myr)`, positive at every age.** Real oceanic
   lithosphere younger than ~10 Myr is lighter than the asthenosphere: its 7 km of basaltic
   crust and its depleted mantle outweigh the thermal contraction (Cloos 1993; Oxburgh &
   Parmentier 1977; Afonso et al. 2007 give 10–30 Myr). In the model 17% of the trench length
   subducts crust younger than 10 Myr, and 27% of the subduction flux is crust younger than
   20 Myr (Earth ~11%).
2. **Nothing resists the start of subduction** (#42). A contact becomes a trench as soon as it
   converges at 0.1 cm/yr. Bending an unbent plate and breaking a thrust through it needs
   ~2–4 × 10¹² N/m (McKenzie 1977; Mueller & Phillips 1991; Toth & Gurnis 1998; Gurnis et al.
   2004), about the ridge push (3 × 10¹² N/m in the model), and the resistance falls once
   100–150 km of slab have gone down.

## Change (`src/tectonics/dynamics.rs`), as revised after the plan review ([`review-plan.md`](review-plan.md))

1. **Net slab buoyancy** `slab_buoyancy(age, slab length)`, relative to a full slab of the
   reference age:
   `(√(age/100 Myr) × L/600 km − √(age₀/100 Myr) × min(L, 150 km)/600 km) / normalisation`,
   with the new parameter `neutral_buoyancy_age` age₀ = 10 Myr.
   - The thermal term acts along the whole slab, as before.
   - The crustal term acts on the shallow slab only: below ~80–100 km the crust is eclogite and
     no longer floats (review finding 7 and its physics note). The first version subtracted it
     along the whole slab, which cut the pull of a 20 Myr old full slab to 0.43× and of a
     40 Myr old one to 0.73×: a hidden recalibration of mature subduction.
   - Now a full slab pulls 0.89× at 20 Myr, 0.95× at 40 Myr and 1× at 100 Myr of what it did.
     A slab shorter than 150 km of lithosphere younger than 10 Myr has negative buoyancy.
   - Slab pull uses the positive part.
2. **Resisting strength** per boundary cell (`initiation_strength`), a force of fixed magnitude
   that opposes convergence:
   - `initiation_resistance` (3 × 10¹² N/m) × (1 − slab length / 150 km) wherever the plate
     would subduct without a self-sustaining slab: at trench cells, at cells of a segment in
     which the plate is the lower one even if they do not converge at the moment, and at
     oceanic boundary cells without a segment (half from each side). The first version only
     looked at cells already classified as trenches, so a new contact converged freely for its
     first step and gained ~70 km of slab before the resistance acted (finding 2).
   - plus `slab_pull` × the negative part of the buoyancy at trench cells.
   - Trenches started by the initiation events (passive margins, fracture zones, induced) begin
     with a 150 km slab and so do not feel the initiation resistance (finding 4): those events
     already stand for a completed initiation and have their own waiting times.
   - Lengths: R × edge angle, as for slab pull and ridge push (finding 5).
3. **Torque balance:** effective viscous coupling `strength / convergence speed` between the
   two plates, updated inside the existing active-set loop (Picard iteration, damped by ½).
   The loop ends only when the force of the last solve is within 5% of the strength at every
   converging contact and no opening contact is coupled (finding 1: the first version tested
   the coupling with an absolute tolerance and could leave 2–3 times the strength). Contacts
   slower than `locked_convergence` (0.1 cm/yr) are locked viscously.
   The loop may take up to 100 solves (20 left a fifth of the solves unsettled); the log
   counts solves that do not settle.
4. **Slabs grow only above `min_trench_convergence`** (0.1 cm/yr); a creeping, locked contact
   builds no slab and an existing one decays (finding 3).
5. Diagnostics: length of incipient trenches (slab < 150 km) and the convergence there and at
   the other trenches, in the segments line of the log.

## Expected (from the prototype on seeds 0–3 and 0–2, before the full runs)

Production −7 to −11%, median relative speed −0.7 cm/yr, slab pull −20%, sea floor younger than
20 Myr 36 → 34%; trench length unchanged. This does not bring the model to Earth's 3.4 km²/yr
and is not meant to: the measurement shows the remaining factor is in how the plates are
organised.

## Tests

- `young_lithosphere_is_buoyant_and_old_lithosphere_pulls`: sign of the shallow slab's buoyancy
  around the neutral age; a full slab of the reference age pulls exactly as calibrated, full
  slabs of 40–80 Myr within 7% of before; a full young slab still pulls.
- `plates_without_a_slab_resist_starting_to_subduct`: a dragged plate without a slab converges
  less than half as fast and pushes its neighbour along; a strong drag makes the contact
  yield with little loss of speed (the force stays at the strength); a self-sustaining slab is
  not resisted at all; opening is not resisted.
- `a_short_slab_of_young_lithosphere_resists_being_pushed_down`: with and without the crustal
  buoyancy, same viscous slab resistance.
- `creeping_contacts_build_no_slab`.

## Validation (8 seeds × 1000 Myr, `main` vs branch, 500–1000 Myr, Welch t)

Metrics of the issues: production (kinematic and created), trench and ridge length, relative
speed (median, share below 2 cm/yr), sea floor younger than 20 Myr, mean sea-floor age, the
plate-structure table of the measurement. Guard rails: sea floor ≥ 200 Myr, sediment-part slope
and level, hypsometry (highest cell, mean land), flooded continental crust, runoff, sediment to
the ocean, trench sediment, felsic + sediment net, collision length and stacking, dynamic
topography minimum (#37), plate count and births, runtime (≤ +10%; the Picard iteration adds
solves).
