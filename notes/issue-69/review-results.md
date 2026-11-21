# Review of the results and code of #69 (deformable plate margins)

_2026-11-21. Independent review of branch `agent/issue-69` at fc73693. I read the plan with its
revisions, the plan review, results.md, guard-full(-s02).md and belts.md. I read the code diff
against `origin/main` (deformation.rs, `apply_additions_inner`, `stretch_rift`, the belt
resistance in dynamics.rs, the suture default, the params). I re-checked the raw outputs in
`~/tasks/issue-69/runs` with the branch's own scripts (notes/issue-43/compare.py,
notes/realism-batch-4/scripts/compare.py, notes/issue-69/scripts/belts.py) and a few short
scripts of my own (in `~/tasks/issue-69/tmp/review/`). I did not build or run anything. The
standard is physical plausibility for an Earth-like planet, not Earth; coarse-grid artefacts are
accepted._

## Verdict

The code is correct where it matters. The band distribution conserves mass. The belt-resistance
units are consistent, and it counts each collision once. The Picard iteration converges: every
run has 0 unsettled velocity solves. Most numbers in results.md match the data. Its central
attribution is right and honestly stated: the band and the belt resistance fix thickness, and only
the suture threshold moves the boundary shares. There are four weak points:

- one key t-value is misreported;
- the decision rule for 0.2 does not test the threshold itself;
- the ablations are compared against unpaired seeds;
- the mean-land drop is really a consequence of adding a second collision resistance on top of a
  calibrated one.

The 0.2 default is justified on the data, more strongly than results.md shows (finding 2). None of
the findings blocks a PR once the text is fixed. Findings 3 and 4 decide how the PR should frame
the hypsometry.

## Findings

### 1. The t-value of `full-s02` vs `main-s02` on crust > 50 km is wrong, and the PR-vs-`main` t is off (medium)

results.md (primary metrics, the note under the table) gives "t of full-s02 vs main-s02 on crust
> 50 km −2.4". This number is used in decision rule (b). Recomputed:

| comparison | cell dumps (belts.py) | log (batch-4 compare.py) |
|---|---|---|
| `full-s02` vs `main-s02-16` | 10.26 → 2.95, **t −4.9** | 10.27 → 2.91, **t −4.8** |
| `full-s02` vs `main` | 6.84 → 2.95, t −2.4 | 6.86 → 2.91, t −2.4 |

So the −2.4 is the comparison against `main`, not against `main-s02`. The table's "t (PR vs main)"
column gives −2.6 on crust > 50 km, but both sources give −2.4. The other t-values I checked
against `tmp/main-full-s02.md`, `tmp/s02.md` and the guard tables all match:

- slow share +2.4, continental share +1.5, mean land −2.0, stacking −1.4;
- vs `main-s02`: mean land −4.7, stacking −4.6, slow −1.4, continental −2.3;
- sutures 0.88 → 1.32 and 0.35 → 0.99;
- p99 62 → 55 km, > 60 km 2.2 → 0.6%;
- highest cell 5.4 → 4.7 km;
- the seed ranges.

The conclusion of rule (b) gets stronger, not weaker.

**Fix:** use −4.9 (cells) or −4.8 (log) for `full-s02` vs `main-s02`, and −2.4 in the PR-vs-`main`
column.

### 2. Decision rule (b) tests the band, not the threshold. 0.2 is justified, but by a comparison results.md does not show (medium)

Rule (b), "`full-s02` beats `main-s02` on crust > 50 km", compares band code with main code at the
same threshold. It shows that the band and the belt resistance fix thickness. It says nothing about
whether 0.2 is better than 0.5. Rule (a) has only upper bounds; for example, mean land has no lower
bound. Rule (c) is about the band. So the rule as written would also pass a threshold that adds
nothing.

The comparison that decides the threshold is `full` vs `full-s02`: the same code, threshold
0.5 → 0.2 (issue-43 compare.py, 8 vs 16 seeds):

| | full (0.5) | full-s02 (0.2) | t |
|---|---|---|---|
| slower than 2 cm/yr (%) | 12.05 | 16.86 | **+4.2** |
| continental boundaries (%) | 13.70 | 16.47 | **+3.2** |
| crust > 50 km (%) | 2.1 | 3.0 | (both in band) |
| mean land (m) | 598 | 679 | +1.7 |
| continental crust flooded (%) | 33.4 | 28.6 | −2.2 |
| collision stacking (Gt/yr) | 17.1 | 18.9 | +0.9 |
| sutures / 100 Myr | 1.32 | 0.99 | −2.0 |
| largest plate (%) | 26.8 | 22.4 | −3.0 |

On the band code, 0.2 moves both boundary targets clearly. It moves mean land and flooding *towards*
Earth and towards `main`, and keeps thickness and stacking in band. That is a better case than
results.md makes.

Two points of honesty belong next to it:

- **Part of what 0.2 does is offset extra sutures caused by the belt resistance.** The resistance
  raises sutures from 0.88 to 1.32 per 100 Myr at 0.5. With 0.2 they come back to 0.99, about
  `main`'s rate. The largest plate also returns from 26.8% to `main`'s 22.4%.
- **Most of the slow boundary length gained is creeping or near-stalled collision belts.** I
  decomposed the boundary dumps over 500–1000 Myr. The script `tmp/review/slow.py` measures the
  slow share of all boundary cells, the slow collision cells, and the collisions converging at
  0–0.5 and 0–0.2 cm/yr. From `full` to `full-s02`, slow 11.9 → 16.7%:
  - slow collision cells give +3.1 points of the +4.8;
  - collision cells converging at < 0.5 cm/yr go 1.7 → 3.4% of all boundary cells;
  - those at < 0.2 cm/yr go 0.8 → 1.7%.

  That is plausible: Earth's slow class includes the Alps and similar belts. The PR should still
  say it.

Is 0.2 a recalibration in disguise? I think it is not. It is a threshold set inside the physically
quoted range (Gordon 1998's lower bound of diffuse boundaries; the slowest active belts at
~0.2–0.5 cm/yr). It is documented, `--set` restores #68, and on the same code it improves the
targets without a guard-rail cost. It is still a calibration choice, not a consequence of the band.
The results text should not suggest that the band "made" 0.2 possible.

**Fix:**
- Add the `full` vs `full-s02` table to "The suture threshold" as the evidence for 0.2.
- State the suture-offset point and the creeping-collision composition.
- Mark rule (b) as what it is: a check that the band code tolerates 0.2, not a test of 0.2.

### 3. The belt resistance is a second collision resistance on top of a calibrated one. "No recalibration" understates its effect on hypsometry (medium)

`collision_resistance` (viscous, 50 cells of basal drag) was calibrated on `main` to represent the
whole resistance of collision belts, ~2–3·10¹² N/m at 1–2 cm/yr according to its own doc. The new
plastic term adds ΔGPE(h) on top. That is ~2.6·10¹² N/m at 50 km and 7.6·10¹² N/m at 70 km, so at a
thick belt the total resistance roughly doubles or triples. The belt term is also biased high in
three ways:

- h is the **maximum** over the cell and its 6 neighbours, and ΔGPE grows with h².
- The thickening of the lithospheric mantle, which reduces ΔGPE and can make it negative, is
  neglected. results.md names this as a caveat. It makes factor 1 an upper bound rather than "the
  physical value".
- The ablation shows the size of the effect. `gpe-only` (seeds 0–3, paired against `main` seeds
  0–3):
  - plate speed 4.15 → 3.69 cm/yr (t −2.5);
  - mean land 900 → 487 m;
  - flooded continental crust 25 → 40%.

The PR's mean land of 0.68 km is (t −2.0 vs `main`):

- still plausible for an Earth-like planet;
- inside the old calibration band of 0.6–0.8 km;
- below Earth's ~0.84 km.

Seeds go as low as 0.44 km. The sediment chain follows the lower relief: largest river −30%
(t −3.3), runoff −10% (t −2.3), terminal fans −44% (t −2.5), trench sediment −25% (t −2.1),
sediment to the ocean −20% (t −2.0). None of these is in results.md (see 9).

**Fix:**
- Before asking Sebastian, run one 4-seed sensitivity set with `belt_gpe_resistance=0.5`, and
  optionally one with `collision_resistance` lowered (e.g. 30) at factor 1. The decision then
  comes with numbers.
- Call the factor "an upper bound (no mantle-lithosphere thickening)" in the doc of
  `belt_gpe_resistance` and in the PR.
- Put "mean land 0.68 km, decide between a lower `collision_resistance` and a lower GPE factor" in
  the PR's "Decisions needed".

### 4. The ablations are compared against unpaired seeds, and two statements do not survive pairing (medium)

`band-only` and `gpe-only` use seeds 0–3. results.md compares them with `main` over seeds 0–7. Seeds
0 and 1 of `main` are the thickest: crust > 50 km is 13.5, 9.0, 4.0 and 1.5% for seeds 0–3. Paired
against `main` seeds 0–3:

- **Rifts in `band-only`:** 2.30 → 1.50 per 100 Myr, t −1.2 (Welch), not 2.6 → 1.5 with t −2.2.
  The sentence "band-only has fewer rifts … probably via the smaller orogenic area" should go or
  be weakened.
- **The band alone is not a thickness reducer.** Paired crust > 50 km per seed, `main` → `band-only`:
  13.5 → 3.2, 9.0 → 3.9, 4.0 → 4.4, 1.5 → 3.6 (mean difference −3.2, paired t −1.1). The band
  removes the extreme seeds and *raises* crust > 50 km in the seeds that were thin. This is
  consistent with the deterministic test's remark that spreading ~400 km of shortening over
  500 km still makes ~60 km crust. `full` shows the same pattern: 3.1 vs 1.5% in seed 3. The
  reduction of the mean comes from taming the outlier seeds (seed range 1.5–13.9 → 1.0–6.6) and
  from the belt resistance (gpe-only: paired t −2.1).
- Band-only stacking (15.7 vs 25.5 Gt/yr, t −1.3) and collision length (19 vs 29·10³ km) are
  within scatter. A mass-conserving redistribution has no direct way to change the stacking rate,
  so this is chaotic divergence, not an effect.

**Fix:**
- Redo the ablation table against `main` seeds 0–3 (paired differences are the right test with 4
  seeds).
- Say "the band reshapes and homogenises, the belt resistance reduces".
- The headline "thickness is fixed" stays true for `full-s02`. The attribution sentence should
  read "mostly the belt resistance, by less convergence".

### 5. Duplicated samples get multiple weights in the band (low–medium; not a mass bug)

`apply_additions_inner` pushes `self.owners[cell]` for every reached cell. A sample shown at k cells
appears k times in `reached`, each time with its own ring and full weight. The cell dumps show
duplicated samples at:

- ~16% of all cells overall (log: "16.3% of cells show a duplicated sample (at most 4 cells)");
- **25–35% of collision boundary cells** (seeds 0–1 at 700 and 1000 Myr, both sets).

In pass 0, all entries use the weight computed from the same pre-addition state. A duplicated
sample therefore takes 2–4× its share. Its `fitting` is computed sequentially against the current
thickness, so the 75 km cap and mass conservation hold. `MIN_OROGENIC_STRAIN` is checked per
entry, so k small fittings can each fall below 1% while their sum does not. This existed with the
4-ring profile too. The band reaches ~91 cells instead of ~37 and is where duplicates
concentrate, so the bias is now larger.

**Fix:** de-duplicate `reached` by `Ownership` and keep the smallest ring, e.g. with a
`HashSet<Ownership>` or a per-event stamp on (plate, local). Add a test with a sample owned by two
cells in the band that checks it gets one share.

### 6. The band is a disc around the addition cell on the whole overriding plate (low)

The BFS expands through every cell of the overriding plate, oceanic ones included. `band_weight`
then accepts any continental-origin cell within 5 rings. Two consequences:

- Crust can go across a strip of oceanic lithosphere of the same plate (a back-arc basin or a
  rifted sliver) to a continent up to 500 km away. Stress is not transmitted to thicken a far
  continent through ocean that does not itself deform.
- At the ends of a collision, and for short collisions (terranes), the disc spreads crust 500 km
  along strike beyond the contact.

Both are small at 110 km cells.

**Fix (optional):** for `use_band`, expand the BFS only through continental-origin cells, and
keep the full expansion for the spill and the profile fallback. Otherwise, name it as a caveat.

### 7. Smaller code points (low)

- **Orogenic marking order dependence.** Marking happens per pass and per addition. A cell marked
  by an earlier addition in the same substep is 8× softer for the next one, and from pass 1 on for
  the same one. This is mass-neutral and bounded by the floor of 0.3 and by room, so a doc
  sentence is enough.
- **Plateau limit is soft.** In pass 0 a cell below 70 km can be filled up to 75 km, because room
  is only a weight and `fitting` is limited by `max_crust_thickness`. That is intended ("75 km
  stays the hard cap"), but the doc of `PLATEAU_THICKNESS` reads as a limit. Say "weight → 0 at
  70 km; the hard cap is `max_crust_thickness`".
- **Wider fallback spill.** When the band is on but no cell has weight (oceanic surroundings), the
  profile fallback runs over a `reached` set that extends 2 rings further than on `main`
  (rings = band + 1). Spill can therefore go 2 rings further. It is harmless; mention it in the
  comment.
- **`stretch_rift`.** The (h/h₀)² weights keep the sum of donor *fractions* at N/(N + 1). Thick
  donors give larger fractions of larger columns, so the filled vertex gets thicker crust than on
  `main` and breakup (`given < breakup_thickness`) comes later in thickened crust. That is
  physically fine (thick crust rifts wide). The mass test covers conservation. One sentence in the
  doc would make the side effect visible. No bug.
- **Mass bookkeeping.** I checked it and it is correct:
  - `remaining` carries the leftover through the passes, then the nearest-first spill;
  - the unplaced rest shows up in `AdditionsLost` through `actual − nominal`;
  - sediment is placed once, in pass 0, by weights that sum to `total_weight`;
  - the fallback uses the profile weights over the same reached set.
- **Belt resistance.** I checked it and it is correct:
  - units are N per boundary cell (ΔGPE in N/m × ½ × boundary length), consistent with
    `initiation_strength`;
  - the coupling is symmetric (p, q blocks once per cell);
  - collision cells occur on both sides in about equal numbers, e.g. 299 collision cells with
    290 matched pairs (seed 0, 600 Myr), so the ½ is right;
  - the force is released when the contact opens and never pushes;
  - the Picard iteration starts locked (strength/lock), is damped, and converged in every run.

### 8. Lock speed and the missing collapse (low; physics)

The GPE term is rate-independent, so treating it as plastic is physically right (England &
Houseman 1989). Its regularisation `locked_convergence` = 0.1 cm/yr has consequences. Once the
driving force falls below ΔGPE · L, a belt slows to ≪ 0.1 cm/yr. That is below either suture
threshold, so it sutures after 30 Myr. On Earth, such a belt would collapse and extend, the
GPE-push follow-up named in the plan. That explains why sutures rise with the resistance.

**Fix:** none needed now. Name it in the caveats ("a stalled thick belt locks and sutures; it does
not collapse"). Optionally run a 4-seed sensitivity with the collision lock speed at 0.2–0.5 cm/yr.

Citations: I checked the references against what they are used for: England & McKenzie 1982;
Houseman & England 1986; England, Houseman & Sclater 1985; England & Houseman 1989; England &
Molnar 1997; Molnar & Lyon-Caen 1988; Gordon 1998; Buck 1991; Neil & Houseman 1997; Vilotte et
al. 1982; Hyndman et al. 2005. They are appropriate, and the bibliographic details look right.
The ΔGPE formula and its numbers (2.65 and 7.64·10¹² N/m) are correct for Airy isostasy.

### 9. Guard rails in results.md leave out five |t| ≥ 2 metrics (low)

The table says "|t| ≥ 2 or relevant" but leaves out these metrics from guard-full-s02.md:

- runoff (−2.3);
- largest river (−3.3);
- sediment to the ocean (−2.0);
- terminal fans and deltas (−2.5);
- trench sediment (−2.1).

All of them follow from the lower relief.

The old-sea-floor regression is correctly reported: ≥ 200 Myr 2.1 → 3.2%, t +2.8. It is within
earlier batches' 3–4%, but it moves in the direction of #14.

**Fix:** add one line, "lower relief: less runoff and sediment (largest river −30%, fans −44%)".

### 10. Promised diagnostics that are missing, and a dangling runtime line (low)

These are promised in the plan revisions but not reported:

- stacked mass per km of collision boundary (revision 2). From the logs it is
  stacking / collision length, in Gt/yr per 10³ km:

  | main | main-s02 | full | full-s02 |
  |---|---|---|---|
  | 0.91 | 0.83 | 0.84 | 0.67 |

  So the PR stacks 27% less per km of belt than `main`, which is the belt resistance at work.
- the largest-receiver and ring-0 shares per addition (revision 3);
- the "band without softness" ablation (plan review 3).

Either report them or say that they were dropped.

"Runtime: see below (paired local seed)" has no section below. The data exist:
`runs/time-main` 498 s vs `runs/time-full` 486 s (seed 0, local). So there is no runtime cost;
fill it in.

### 11. Housekeeping before the PR (low)

- `notes/**/__pycache__/sim_classes.cpython-313.pyc` is modified in the diff. Revert it, and add
  `__pycache__/` to `.gitignore` if it is not there.
- `scripts/analyse.sh` writes `log` symlinks into the raw run directories. That is harmless, but
  writing them into a scratch copy, as for `cmp/`, is cleaner.
- The runs were made at e834b96, before the rebase onto #77. Between e834b96 and HEAD only
  `main.rs`, `query.rs`, `web.rs` and the viewer change, plus the suture default that the s02 sets
  set with `--set`. "No change of simulation output" is therefore plausible. State it as checked
  from the diff, or run one seed.
- The PR should state CI status (fmt, clippy, tests with `CARGO_BUILD_WARNINGS=deny`). I could not
  check it without a build.
- The "Decisions needed" in the PR should list:
  1. the 0.2 default (with the table of finding 2);
  2. mean land 0.68 km and what to lower (finding 3);
  3. the order of merging with #75 (the call to `band_strength`).

## Summary

- **Code.** Mass is conserved through the passes, spill, fallback and sediment. The belt
  resistance has consistent units (N per cell, ½ for two sides, which I checked against the
  boundary dumps). The Picard loop converges in every run. One real weakness: duplicated samples,
  which are 25–35% of collision cells, get k× weight in the band. De-duplicate by `Ownership`.
- **Numbers.** Most match the data. The t of `full-s02` vs `main-s02` on crust > 50 km is −4.9,
  not −2.4, and the PR-vs-`main` t is −2.4, not −2.6.
- **Attribution.** "Boundary shares move only through the threshold" is right. The ablations must
  be paired with `main` seeds 0–3. Then the band turns out to reshape and homogenise, not reduce
  thickness (it raises crust > 50 km in thin seeds). The belt resistance does the reducing, and
  the `band-only` rift claim disappears.
- **0.2.** It is justified, but by `full` vs `full-s02` (slow share t +4.2, continental +3.2, mean
  land and flooding towards Earth), which results.md does not show. Rule (b) tests the band, not
  the threshold. Part of what 0.2 does is offset the extra sutures of the belt resistance, and
  most of the gained slow length is creeping collision belts. The PR should say both.
- **Mean land.** The drop to 0.68 km comes mainly from stacking a second, upper-bound collision
  resistance on a calibrated one. Run a 4-seed sensitivity set (factor 0.5 or a lower
  `collision_resistance`) before handing Sebastian the decision.
- **Before the PR:** fix the t-values, add the threshold table, the paired ablations, the
  missing guard rails, the runtime and the stacking per km, revert the .pyc, and state CI status.
