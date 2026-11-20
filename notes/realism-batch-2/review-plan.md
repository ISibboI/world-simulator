# Review of commit 1e9e267 (buoyant young lithosphere, initiation resistance)

_2026-11-20, independent reviewer. Code read at 1e9e267; nothing was built or run._

**Verdict: approve with changes.** The idea is sound and the torque coupling is correct (symmetric,
right sign and normal, consistent units). But the Picard stopping test lets yielding contacts carry
2–3× the strength, the resistance misses the first step of every new trench, locked contacts creep
into self-sustaining slabs, and the buoyancy offset lowers the pull of mature 20–60 Myr slabs by
15–57%. That last point is a recalibration, not "unchanged by construction".

## Findings, most important first

1. **Picard tolerance is absolute, so a yielding contact carries too much force** (dynamics.rs:341-347).
   `scale.max(*coupling)` is always `scale = S/v_locked`, because the coupling never exceeds it. The loop
   stops once `|c − target| ≤ 0.04·scale`, and the last solve used that coupling. The force is then
   `c·v ≤ S·(1 + 0.04·v/v_locked)`: up to 3× S at 5 cm/yr and 5× S at 10 cm/yr. Starting from the lock
   and halving, the loop reaches that point after about 5 iterations, so this is the normal outcome.
   Contacts that open (`speed ≤ 0`, target 0) also keep a coupling of up to 0.04·scale, which resists
   opening. *Change:* test the force relative to the strength, for example
   `|c·max(v, v_locked) − S| ≤ 0.05·S` for converging contacts. Set the coupling to 0 at once for
   contacts that open. Re-solve after the last update.
2. **The resistance misses the first step of every new trench** (dynamics.rs:117-121, 178;
   mod.rs:851, 877-878; polarity.rs:266). `boundary_cells` classifies with the velocities of the last
   step. A new segment is created during the substeps with slab 0. At the end of that unresisted step
   it becomes a trench and its slab grows by `v·dt`: about 68 km at the model's median 6.8 cm/yr and
   dt = 1 Myr. That removes ~45% of the resistance before it ever acts, and the loss depends on dt.
   This fits the unchanged trench length in measurement.md §4 (130 vs 131). *Change:* in
   `update_plate_velocities`, apply the strength to every contact whose segment makes the oceanic
   side subduct and whose slab is shorter than `self_sustaining_slab_length`, whatever the current
   convergence. The force is plastic, so it is zero where the contact opens.
3. **Locked contacts creep into self-sustaining slabs** (dynamics.rs:501-503, 540-541). A slab grows
   at any positive convergence and decays only at ≤ 0. A locked contact moves at up to `v_locked`
   (1 km/Myr). Its slab is > 0 after finding 2, so the hysteresis in dynamics.rs:120 keeps it a trench.
   It therefore never decays and reaches 150 km within ~100 Myr without the strength ever being
   overcome. Then it pulls. A contact that stays locked is physical; this creep is not. *Change:* grow
   slabs only where convergence exceeds `min_trench_convergence` (or `locked_convergence`) and let
   them decay otherwise. A locked incipient trench then expires after ~10·ln(L/20 km) Myr, and the
   segment stays for the next contact.
4. **Mature slabs pull less: a recalibration** (dynamics.rs:447-452, 373). Pull relative to `main`:
   20 Myr 0.43×, 40 Myr 0.73×, 60 Myr 0.87×, 80 Myr 0.95×, 100 Myr 1.00×, 120 Myr 1.04×. The median
   subducting age is 32 Myr (measurement §2), so a typical slab loses ~30%. This is the source of the
   expected "slab pull −20%". Physically, crustal buoyancy offsets only the shallow slab: below
   ~60-100 km the basalt is eclogite and denser than the mantle. A constant offset on the whole pull of
   a 600 km slab overstates the loss. *Change:* subtract the crustal term only over the shallow slab,
   for example `pull ∝ θ(age)·f_L − θ₀·min(L, L_s)/L_max`, with `L_s` ≈ `self_sustaining_slab_length`.
   If you keep the form instead, call it a deliberate recalibration in plan.md, and check the
   `slab_sinking_speed` calibration and the plate speeds of 20–60 Myr slabs.
5. **The buoyant resistance should not scale with the full slab length** (dynamics.rs:465-467). It
   rises to `2e13 × 0.256 ≈ 5e12 N/m` for 2 Myr crust with a 600 km slab, more than the initiation
   resistance and the ridge push. Only the shallow, unbent, not-yet-eclogitised part is buoyant.
   When a ridge nears a trench, the deep old slab keeps pulling. The model takes the age at the trench
   for the whole slab, which is older behaviour, but the new negative branch amplifies it. *Change:*
   scale the buoyant term with `min(L, L_s)/L_s` (shallow part), not `slab_length_factor`.
   Representing it as a plastic strength (opposes subduction, never drives retreat) is fine.
6. **The force per cell uses two different lengths.** The new terms use `boundary_length_per_cell()`
   = R·edge/1.1 (dynamics.rs:469; mod.rs:769-771), while slab pull, ridge push and slab resistance use
   R·edge (dynamics.rs:370, 434). The buoyant resistance is therefore 9% weaker than the pull at the
   same |factor|. The comparison "initiation ≈ ridge push (3e12)" is off by 10% in the same direction.
   *Change:* use one length. The corrected one is right, but switch the old forces in a separate change.
7. **One test does not test what it claims** (dynamics.rs:880-884). "Buoyant vs neutral" compares a run
   with a pull of ~2.8e12 N/m (neutral age 0) against one without pull. It would pass with no buoyant
   resistance at all. *Change:* compare neutral age 2 Myr (no pull, no resistance at 2 Myr) against
   10 Myr. The test that a mature slab does not resist (dynamics.rs:876-878) is trivial: `nascent` is
   empty. The yield test (`> 0.5·free`) would also pass with the excess force of finding 1.
   *Missing tests:* force ≤ (1 + tol)·S at a yielding contact; no resistance to opening; the reaction
   on a free overriding plate; over several steps, that a new contact is resisted from its first step
   and that a locked contact does not grow a slab.
8. **Non-convergence goes unnoticed** (dynamics.rs:354-356). The message is at debug level and still
   says "collision active set". *Change:* count non-converged solves in the statistics. Measure
   iterations per solve in the validation, because runtime is a guard rail.
9. **Event-initiated trenches never feel the resistance.** Passive-margin and induced initiation seed
   `initiation_slab_length = 150e3`, which equals `self_sustaining_slab_length` (events.rs:124, 1326,
   1588). The strength therefore acts only on segments created by overlap. That is defensible, since
   the events model spontaneous foundering, but plan.md should say so, because those initiations are
   the main trench source (measurement §4).

## measurement.md / plan.md

- Checked against the code and correct: the 4× continental drag (mod.rs:305, dynamics.rs:165-171);
  every passive-margin initiation splits the plate (events.rs:1193-1194, `split_locally`); ridge push
  3e12 (mod.rs:302); a trench from 0.1 cm/yr convergence (dynamics.rs:117-121), though it also needs
  an overlap segment.
- §4, "passive-margin initiation is the largest single lever": the −26% row turns off two switches,
  initiation and invasion. Initiation alone gives 6.0 against 6.15 km²/yr (1 seed). Say "initiation
  plus invasion", or test invasion alone.
- §4, this batch's row: the plate count rises from 26.8 to 30.6 (+14%). It is not commented, and it
  works against the stated aim (too many slab-attached plates). Explain it or track it in the
  validation.
- plan.md, "mature subduction unchanged by construction": true only at exactly 100 Myr (finding 4).
- §5, "about half real": the numbers give 0.43/0.72 = 60% real area, 40% churn. That is acceptable
  wording.
- Magnitudes: 2–4e12 N/m for initiation and 100–150 km to self-sustaining are consistent with
  Gurnis et al. 2004 and Toth & Gurnis 1998, as I remember those papers. The neutral age of ~10 Myr
  is the young end of published estimates; others reach 20–40 Myr. I did not check either against
  the papers. Treat 10 Myr as a lower bound and include `neutral_buoyancy_age` = 20 in the
  sensitivity runs.
