# Review of the batch 2 results (measurement.md, results.md, code at ea2aa5f)

_2026-11-20, independent evaluator. Own parsers of the logs and dumps in `agent-scratch/runs/`
(scripts in `~/tasks/realism-batch-2/tmp/eval/`); nothing built or run. Welch t, n = 8._

**Verdict: ready with caveats.** The headline numbers reproduce, and the code addresses review
findings 1–6. Two statements in the notes are wrong and must be corrected before the PR: the speed
at boundaries between slab-free plates, and the convergence at incipient trenches. results.md also
leaves out the one effect that is significant, which is the effect the change was meant to have.

## Findings, most important first

1. **Wrong number in measurement.md §1 and results.md: speed between slab-free plates.** In
   `plates.py` the keys `median relative speed there` and `slower than 2 cm/yr there` are the same
   for the three pair classes, so the reported value is the mean over neither/one/both. Measured
   per class: neither 3.5 cm/yr (1.9…5.3), one 6.6, both 8.3. Their mean is 6.1, close to the
   reported 6.2. The share slower than 2 cm/yr there is 34%, not 12% (v2: 3.0 cm/yr, 29%). Earth:
   1.5 cm/yr, 59%. So the boundaries between slab-free plates are moderately slow. The difference
   from Earth is mainly how much boundary they make up, not how fast they move. *Change:* fix the
   keys and correct both tables and the "Reading" text.
2. **results.md misses its significant effect.** Subduction flux carried by crust younger than
   20 Myr: 26.7 → 23.0% (t −2.7). Related metrics move the same way: trench length subducting crust
   < 20 Myr 33.3 → 30.6% (t −2.2), flux < 10 Myr t −2.0, median subducting age 32 → 35 Myr (t +1.8).
   The plate-structure table gives no t values. No other metric has |t| > 2.4 except runtime. I
   checked about 40 log and dump metrics myself and took the rest from compare.py. With about 100
   metrics, one hit could be chance. This one was the target named in advance, though, and the
   related metrics agree. *Change:* say that buoyancy cuts the subduction of young crust by about
   a seventh, which is still far from Earth's ~11%, and that nothing else changes.
3. **The 1.9 cm/yr at incipient trenches comes from the raw cell normals.** The log uses the
   zig-zag normal of each cell. With the smoothed normals of the solver and the dump, incipient
   trenches converge at 0.51 cm/yr. 40% of their length converges at less than 0.1 cm/yr. I
   reproduced the log's 1.87 from the raw normals. Main is the same: 0.58 cm/yr, 41%, 7.1% of the
   trench length against 7.6%. "~8%" holds. "The resistance slows them but does not stop them" is
   not supported, because the incipient trenches are no slower and no rarer than on main. What the
   data show instead: the resistance has almost nothing to act on. Contacts carrying a strength
   outside trenches are ~1.8% of the boundary, and only 15–43% of those converge. Event-started
   trenches begin at 150 km and are exempt by design (old finding 9). Buoyant trench cells are 4.5%
   of the trench length on both main and v2, which replaces the "probably rare" inference.
4. **Finding 3 is fixed only for uniform creep.** `update_slab_lengths` and `boundary_cells` use
   the raw normal, `update_plate_velocities` uses the smoothed one. 35% of the incipient trench
   length is locked in the solver (< 0.1 cm/yr smoothed) but converges faster than 0.1 cm/yr raw,
   so its slab still grows. The new test moves a whole plate uniformly and cannot see this. The
   problem existed on main too. *Change:* use the smoothed normal (or the segment mean) for slab
   growth, or document it as a known gap. This is also a candidate reason why the resistance is
   inert.
5. **Review findings, status in the code.** 1: fixed. The force is tested against the strength
   with a 5% tolerance, opening contacts are released at once, and the system is solved again
   after every change. 2: fixed (`(Other, Some)` on the subducting side, `(Other, None)` half per
   side). 3: see 4. 4–5: fixed. The crustal term acts on the first 150 km only, and the
   normalisation 1/(1 − 0.079) keeps a full 100 Myr slab at 1.0. It raises the buoyant
   resistance by 8.6%, which is harmless. 6: fixed (R·edge). 7: partly fixed. The buoyancy test
   compares neutral age 10 with 0 at 2 Myr, which is pull 7e11 N/m against resistance 9.5e11, so
   it would probably still pass without the buoyant term. The review asked for 2 against 10.
   There are no tests for the force at a yielding contact or for resistance from the first step
   over several steps. 8: not addressed. Non-convergence is still only a debug message, with no
   counter and no iteration count. The damped Picard loop needs about 10 iterations per yielding
   contact out of the 20 allowed.
6. **Other new code.** No double counting: overriding sides fall into `_ => 0`, and each side
   carries its own cells. Transforms are resisted only across strike, which is correct. An
   ocean–continent contact without a segment gets only the ocean side's half, 0.5·S instead of S.
   This is minor (0.7% of the boundary is `(Other, None)`) but inconsistent. With the new growth
   threshold, mature trenches between −0.1 and 0.1 cm/yr now decay instead of growing. That is
   0.9% of the trench length, so the effect is negligible.
7. **Runtime guard rail not measured.** On main, lanes 4–7 took 735–808 s and lanes 0–3 took
   563–668 s. The v2 and first-version runs all took 520–600 s. The t = −3.9 reflects machine
   load. The ≤ +10% check is open. *Change:* run 2 seeds of each alternately on a quiet machine,
   or log Picard iterations.
8. **Earth comparison: fair, and it survives the threshold.** Attached plates ≥ 1% (model main /
   Earth), with boundary between slab-free plates in brackets. 5%: 11.0 / 8 (3.8% / 26%).
   10%: 9.5 / 5 (9% / 48%). 20%: 7.9 / 3 (19% / 68%). Counting immature trenches in the model too
   gives 10.2 at 10%, so the mature criterion favours the model. PB2002's microplates touch 47% of
   Earth's boundary length, against 26% in the model. Restricted to boundaries between plates
   ≥ 1%, the slab-free share is 54% on Earth and 11% in the model. "About twice as many" holds
   only at 10% (1.4× at 5%, 2.6× at 20%). The slab-free boundary share, 5–7× lower than Earth's,
   is the robust statement. Earth is one snapshot; the model at 10% fell to 4–6 attached plates in
   a few snapshots only. The sensitivity runs leave this share at 8–12% (supercontinent, nobound,
   nooceanrift; noinit 7.8%), so the exclusions hold, with only 3 seeds each.
9. **Verified as reported:** production 5.90/5.75 (t −1.0), trenches 123/125 (10³ km), median
   relative speed 6.90/6.71, < 2 cm/yr 5.6/6.0%, sea floor < 20 Myr 34.5/34.9%, mean age
   55.1/51.7, ≥ 200 Myr 4.04/3.35%, slab pull 1.39/1.34, incipient 9.8 (10³ km), runtime
   689/567. The plate-structure table, apart from finding 1. Ancient sea floor: 4.9% of the
   oceanic crust, 76% on plates that are not attached, 82% farther than 550 km from a trench
   (great circle), 78% within 300 km of the plate's own continental crust (84% reported; hop
   metric), 60% shallower than 2 km, 8.9 km sediment. The first version: production −12%, slab
   pull −25%, relative speed 6.5 → 5.7 (seeds 0–2), as stated. Minor: the full table is labelled
   "500–1000" but uses 600–1000 for levels.

## Merge?

Yes, with the notes corrected. The buoyancy is simple, physical and tested, and it is the only
part with a measurable effect (finding 2), which goes in the right direction. The initiation
resistance closes a real gap at no measured cost, but it is almost inert by construction (finding
3). Add a counter for non-converged solves, fix or document finding 4, and say plainly in the PR
that #42 is closed in principle but makes no difference at planet scale.
