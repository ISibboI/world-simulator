# Review of the #42 follow-up results (diffuse convergent zones)

_2026-11-21, independent evaluator. Read: `plan.md` (with its revisions and the #43 section),
`review-plan.md`, the draft `results.md`, and `git diff origin/main -- src` at e5a6a4d. Nothing was
built or run. I re-derived the numbers from the raw logs and dumps
(`agent-scratch/runs/{main,branch}/s0..s7`, and the sensitivity outputs) with my own parsers in
`~/tasks/issue-42/tmp/eval/` (`logparse.py`, `budget.py`, `perseed.py`, `contacts_eval.py`,
`other_rows.py`, `why.py`, `incipient.py`). I skipped the two TODO sections (ablation, runtime).
Goal of the project: an Earth-like planet, not Earth._

**Verdict: the mechanism is implemented as planned, and the headline numbers reproduce exactly.
The interpretation is mostly sound. The weak points:**
- The relief guard-rail shift is borderline but coherent. The likeliest causes are parts of the
  change that do not depend on the diffuse state (findings 1–3).
- One inheritance bug (finding 2).
- Several significant metrics, and one guard rail the plan promised, are not reported
  (finding 4).

Fix 2, add 3–4, and finish the attribution before the PR.

## Findings, most important first

1. **The relief guard rails probably move for a real reason, and the diffuse state itself is
   the less likely cause.** The relief metrics reproduce from the logs: collision length
   13.7 → 10.3 ×10³ km, highest point 4567 → 4108 m, crust > 50 km 2.0 → 0.96%, permanent ice
   11.4 → 9.8%.
   - **These are one causal chain**: less collision → less thick crust → lower peaks → less
     ice → warmer. They are not four independent hits, so "four metrics at |t| ≈ 2" overstates
     the evidence.
   - **The t values depend on the window.** For highest point / crust > 50 km / collision length,
     t is −1.8 / −1.9 / −1.7 for (450, 1000] and −2.6 / −2.5 / −2.0 for (600, 1000]: the gap
     grows with time.
   - **Related dynamics move the same way**: continental crust on slab-attached plates
     15.3 → 7.2% (t −2.2), continental speed 2.0 → 1.7 cm/yr (t −1.3), collision stacking
     −28% (t −1.8). That is a consistent chain: fewer continents on pulled plates → slower
     continents → fewer collisions.
   - **Relief does not scale with the diffuse share.** In the 3-seed sensitivities,
     `diffuse_convergence` = 0.5 (0.8% diffuse) sits closest to `main` (4525 m, 1.8%). The 10¹³
     strength run (3.8% diffuse) is intermediate (4412 m, 1.4%), and the default (1.5% diffuse)
     is lowest (3937 m, 0.5%). So the cause is more likely something that acts independently
     of how much diffuse length there is:
     - (a) inherited displacement removes `main`'s full strength on re-segmented pieces next to
       trenches (finding 3), including across a polarity reversal (finding 2);
     - (b) `break_off` and slab decay no longer restore the strength, because displacement
       never decays;
     - (c) full strength on the oceanic side of ocean–continent contacts without a segment
       (the running `abl-oc` ablation);
     - (d) displacement grows from the segment's *mean* smoothed rate over all lower-plate
       cells (diverging ones included), so it grows more slowly than the slab and the strength
       lasts longer on oblique segments.

   *Change:*
   - Finish `abl-oc`, and add a second ablation for (a)+(b): inherit D only with the same
     polarity, and reset D to 0 in `break_off`, as `main` effectively did with the slab.
   - Log the number of segments with D ≥ 150 km and slab < 150 km, and the convergence they
     take (requested by plan-review finding 5, not reported).
   - In results.md, present the relief shift as one finding: likely real, about −10% highest
     point and −50% crust > 50 km, with its attribution. Put it to Sebastian as a decision if
     it stays.

2. **Bug: a new segment inherits the thrust even when its polarity is the reverse of the
   neighbour's.** In `Polarities::contact`, `thrust` is taken from the neighbour, and *afterwards*
   `overriding` may be flipped (`bottom_crust.resists_subduction() && !top_crust...`). The new
   segment then has the opposite vergence but is localised with the neighbour's displacement.
   Where continental crust arrives next to an existing trench, the former upper (oceanic) plate
   then subducts at once with no strength. `end_pass` treats the same situation (a flip) as a
   new thrust and resets D to 0, and `main` gave such a piece the full strength (slab 0).
   *Change:* set `thrust` only if the final `overriding == inherited` (else `None`). Add a unit
   test next to `thrusts_propagate_along_strike_…`: a localised neighbour, continental crust of
   the neighbour's upper plate as the new bottom, expect a diffuse segment.

3. **The strongest plate-dynamics change (incipient convergence 1.86 → 2.19 cm/yr, t +5.8) has
   a second cause that results.md does not name.** Results.md explains it as "the slow ones
   became diffuse". From the dumps (`incipient.py`, smoothed convergence of trench rows with
   slab < 150 km):
   - On pairs that also have a mature segment (re-segmented pieces next to a trench), these
     rows converge at 0.86 cm/yr on `main` and 1.29 cm/yr on the branch (t +3.3); the count
     drops from 81 to 65 rows per snapshot.
   - Isolated incipient rows drop from 20 to 8.6 rows per snapshot. Those did become diffuse
     zones.

   So both happen. The faster pieces next to trenches are the inherited-D effect: they keep a
   slab-based "incipient" label but feel no strength. Plan-review findings 2 and 5 asked for
   this to be reported as a change.
   *Change:* say so in the plate-dynamics notes, and add the D-based incipient length (D < 150 km)
   next to the slab-based one.

4. **Significant metrics and one planned guard rail are missing.**
   - From `pl-t.txt`, not in results.md:
     - ancient (≥ 200 Myr) sea floor: mean sediment 9.2 → 8.4 km (t −4.1); median age
       357 → 326 Myr (t −2.9); within 300 km of its plate's continent 86 → 81% (t −2.7);
       shallower than 2 km 63 → 56% (t −2.1);
     - all oceanic crust farther than 1000 km from an own-plate trench: 51.4 → 56.3%
       (t +2.6);
     - trenches whose overriding plate is itself slab-attached: 38 → 29% (t −2.0);
     - boundary with exactly one slab-attached plate: 50 → 55% (t +2.0).

     The ancient-ocean items matter for #43.
   - From `cmp.txt`: largest river −22% (t −2.0, probably part of the relief chain). The
     unpaired runtime is +11% (t +2.0), so the paired timing matters for the ≤ +10% rail.
   - **Plan revision 13's guard rail "mafic mass stacked onto continental crust" is not
     reported.** It is needed: at ocean–continent diffuse segments the continental plate is
     the winner. So `resolve_ownership` stacks the oceanic crust *onto the continental margin*
     with the collision profile and dilutes its felsic fraction. Physically, the shortening of
     an oceanic convergent zone thickens the oceanic lithosphere. Continental area (+0.9 points,
     t +2.1) and mean land (−80 m) move in the direction this would push.

   *Change:*
   - Add these rows to the guard-rail and plate tables.
   - Split `ShorteningStacked` (or a log counter) by whether the receiving cell is
     continental-origin, and report the mass per 100 Myr.
   - If that mass is not negligible, stack ocean–continent shortening onto the oceanic side, or
     do not stack it at all (leave it to the diffuse creep), and note this as a decision.

5. **The headline numbers reproduce; the definitions are sound. Two corrections.** My own
   count gives the same results to two decimals:
   - A, ocean–ocean: 87.1 → 84.6%;
   - B: trench 86.07 → 83.59%, diffuse 0 → 1.50%, other 13.93 → 14.91%, locked 3.76 → 2.86%;
   - ocean–ocean + ocean–continent diffuse: 1.71%;
   - quartiles of diffuse convergence: 0.21 / 0.46 / 0.69 cm/yr pooled;
   - from the logs: localisations 70.7 per 100 Myr, transient stacking 0.049 Gt/yr, diffuse
     length 2.11 ×10³ km, shortening 26 / 88 km, shortening flux 0.527 Gt/yr (consumed =
     stacked to 0.001), incipient trenches 11.1 → 8.1 ×10³ km.

   The "lower side" rule of `contacts2.py` is asymmetric: rows without a segment count only
   when converging, so locked rows without a segment are excluded. Counting them changes
   nothing material (ocean–ocean diffuse 1.49%, locked 4.7 → 3.5%), nor does dropping
   segment-less rows (1.53%). No diffuse row is on the upper side. The oceanic test (`oceanic`
   column = `is_oceanic` of the surface crust) is right.

   Corrections:
   - (i) "Other / no segment … untouched here" is only partly true. Of its +1.0 points, about
     0.7 points (0.75 → 1.46% of lower-side rows) are slab-free segments with no `t` or `d`
     row. They are mostly diffuse segments whose mean smoothed rate is ≤ 0.1 cm/yr, so they
     are classified `o`. The diffuse *state* therefore covers ~2.2%, not 1.5%. Say so.
   - (ii) The windows differ: `compare.py` uses (500, 1000], `contacts2.py`/`plates.py` use
     [500, 1000]. State this once. Results.md also quotes thick oceanic crust as 0.22/0.00 and
     0.27/0.02, while the script prints 0.22/0.01 and 0.26/0.02.

6. **"Why so little" is supported by the data, but the Earth examples are overstated.**
   `why.py` classifies converging or locked oceanic lower-side rows by the slab attachment of
   both plates (≥ 10% own mature trench):

   | Plate pair | Share of all diffuse rows | Diffuse share within that class |
   |---|---|---|
   | Both slab-free | 70% | 17.8% |
   | One plate attached | 4–12% per class | 0.3–1.4% |
   | Both attached | 14% | 0.9% |

   This is the best evidence for the argument. *Change:*
   - Add this table.
   - Write "either plate" instead of "the plate on its other side" (the own-attached /
     other-free class has the lowest share, 0.3%).
   - Fix the examples. The Central Indian Basin lies between India and an Australian plate
     pulled by the Sunda slab; Macquarie is Australia–Pacific, both attached; only Gorringe
     (Nubia–Eurasia) fits "between slab-free plates". Rephrase as: "Earth's diffuse zones sit
     where the force transmitted across the zone stays below the strength, often inside a
     composite plate; in the model this happens almost only between slab-free plates."
   - The comparison with PB2002's OCB share is fair as stated, ocean–ocean + ocean–continent
     against OCB/(OCB+SUB).

7. **Fresh contacts subduct and build an arc until the first velocity solve sees them.** The
   stacking branch requires `segment.rate > min_convergence`, and a new segment has rate 0. So
   a contact that is not next to a thrust subducts for the rest of its first step: arc
   addition, sediment subduction, trench fill. Plan E and plan-review finding 3 say "no arc,
   no subduction at diffuse contacts". This is a deliberate guard against resampling noise
   (code comment, and the unit-test comment "only segments created on the way subduct"). It is
   not quantified.
   *Change:* count the oceanic crust subducted (or arc mass added) at diffuse segments with
   rate 0, and quote it next to the transient stacking (0.05 Gt/yr). Or run one update of
   segment rates right after `contact()` creates segments.

8. **The sensitivities need one rewording.** "Seeds 0–2 react more strongly" is subset noise,
   not a seed-specific response:
   - production on seeds 0–2 is 5.99 on `main` (8-seed mean 5.68) and 5.15 on the branch
     (8-seed mean 5.48);
   - likewise crust > 50 km on seeds 0–2: `main` 3.1% (8-seed mean 2.0%).

   Every variant therefore looks "lower than main" there (regression to the mean). *Change:*
   say this, and compare the variants only with the branch on the same seeds, as the text
   already advises.

9. **Code: no other correctness bugs found; small issues.** Checked:
   - **State transitions**: diffuse → localised only on `yielding`, and a localised segment
     stays so until it expires. Merges: `localised |=`, max D, `stacked` summed, earliest
     `created`. Flips keep the thrust and reset D. `split` and `merge` carry the fields
     (clone or append).
   - **Stacking branch**: only diffuse segments that converge; either side; sediment moved
     with `add_sediment` and its cohort; ledger consumed = stacked; additions-lost accounting
     includes the sediment.
   - **`update_segment_rates`** (unit conversions correct). The yield rule is equivalent to
     F > S for the coupling S / max(v, v_d).
   - **Events**: `initiation_displacement` is wired into both event paths.
   - **Suturing and induced initiation**: count `Diffuse` like trenches.

   Small issues:
   - (a) The doc of `initiation_strength` still says the strength fades "as the slab grows"
     and that segment-less contacts carry half on each side. The doc of
     `Subduction::displacement` says "0 for a new one", but the event default is 150 km.
   - (b) The log's "stacked before N Gt" is cumulative (48 × 10⁶ Gt in s0); label it "so far".
   - (c) The log's diffuse convergence uses raw normals and can be negative (−0.30 cm/yr at
     100 Myr in s0), while the class uses smoothed normals. Log the smoothed value.
   - (d) Diffuse zones shorten by up to ~200 km early in runs without localising (max over the
     window 88 km on average). That is more than the 100–150 km of the #43 strain-weakening
     option. Mention it when that option comes up.

10. **Tests: the new ones test the new behaviour, with gaps.**
    - **The four main tests fail without the change**:
      - `diffuse_zones_shorten_instead_of_subducting` needs the stacking branch and the yield
        → localise path;
      - `diffuse_zones_creep_…` needs the diffuse lock speed (creep > 2 × locked) and the
        yield rule (stays diffuse for 10 slab updates at 3 cm/yr drag, localises at 60);
      - `event_slabs_feel_the_initiation_strength` fails with the slab-based fading of `main`;
      - `thrusts_propagate_along_strike_…` needs the inheritance.
    - **`overlaps_at_diffuse_zones_that_do_not_converge_are_not_stacked` only checks the
      unsolved case.** It passes because rate = 0 before any solve. It never checks a solved
      zone that diverges or slides.
    - **Untested**:
      - the sediment path (test crust has `sediment: 0`);
      - stacking of the *upper* plate's oceanic crust;
      - full strength on the oceanic side of ocean–continent contacts without a segment
        (`other_side_resists_subduction`);
      - suturing counting `Diffuse`;
      - inheritance next to a reversed polarity (finding 2);
      - `break_off` keeping D.

    *Change:* add sedimented crust to the stacking test (sediment stock conserved, no
    `SubductedSediment`), and add a solved strike-slip case to the non-converging test. Add
    small tests for `other_side_resists_subduction` and finding 2.
