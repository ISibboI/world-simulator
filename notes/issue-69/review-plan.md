# Review of the plan for #69 (deformable plate margins)

_2026-11-21. I read the plan and checked it against `main` at 536cecb: `boundaries.rs`
(`resolve_ownership`, `apply_additions_inner`, `crustal_flow_inner`, `stretch_rift`),
`weak_zones.rs::lithosphere_strength`, `events.rs::suture_colliding_plates`, `dynamics.rs`
(collision resistance) and `mod.rs` (params). I also read notes/issue-43 (results, investigation),
notes/realism-batch-4/compare-main-branch.md and the #75 plan. Nothing was built or run. The
standard is an Earth-like planet, not Earth._

## Summary verdict

The physics is sound and the citations are fine. The mass bookkeeping can be made safe. As
written, though, the band is **a hypsometry change only**. On its own it cannot move the issue's
boundary targets (slow share, continental share). They can only move through the
`suture_convergence` change, whose best case is roughly #43's v1 result. The weighting
`s⁻³ × taper × room` also has two weak spots. The weight contrast is unbounded (up to 8000×), and
the "room" saturates at the numerical cap of 75 km instead of the plan's own GPE limit. Either one
can make the band put crust into a few cells and fill them to the cap, which is what the band is
meant to stop. The measurement design is underpowered for the thickness metric. It needs a decision
rule fixed in advance so that the suture recalibration is not credited to the band.

## Findings

### 1. The band cannot change the boundary classes. Only the suture threshold can. (high)

Collision resistance in `dynamics.rs` (around line 335,
`coupling = collision_resistance * basal_drag * cell_area`) is a plain viscous coupling on normal
motion. It does not depend on crust thickness, belt width or GPE. No other force in the torque
balance reads the thickness of the overriding plate's margin. So where the stacked crust ends up has
no effect on plate speeds. The slow share (< 2 cm/yr) and the continental share are set by the
kinematics and by when pairs suture (`suture_colliding_plates`, which uses only convergence speed).
The plan says this itself ("boundary mix unchanged within scatter"). The issue's targets of
11% → 39% and 11% → 30% therefore depend entirely on `suture_convergence = 0.2`.

What 0.2 can give at best: #43 v1 effectively never sutured creeping belts, which crept at
~0.3 cm/yr. It reached a slow share of 21% and a continental share of 19%. That is about half of
the gap to Earth's values, not all of it.

**Fix:**
- Say this plainly in the plan and later in the PR: "the band fixes thickness; the boundary shares
  move only through the threshold, at most to about the v1 level."
- Consider the coupling that would make the band matter for the dynamics. Add a GPE-dependent
  collision resistance: a force that opposes convergence with the excess GPE of the belt at the
  contact, ΔGPE(h) = ½ g ρc (1 − ρc/ρm)(h² − h₀²). Handle it like `initiation_resistance`, as a
  plastic force linearised by Picard iteration, and add it to or in place of part of the viscous
  term.
- With that force, thick belts slow their collisions down gradually into slow, persistent creep.
  That mechanism is what keeps Earth's slow continental boundaries slow (England & Molnar 1997),
  and it ties the belt width (this band) to the speed. The plan lists it under "Not done". If it
  stays out, record the issue's boundary targets as not addressed by this PR.

### 2. The 0.2 threshold leaves stacking and mean land outside their bands. The band cannot fix them. (high)

The band conserves mass and leaves the stacking rate unchanged. That rate is the consumed mass,
v · L · h, set by the kinematics in `resolve_ownership`. In #43 v1 (creeping belts that never
suture), stacking was 26 Gt/yr (band 13–23) and mean land 1.04 km (about 0.84 km wanted). With the
band, both numbers stay about the same in `band-s02`. Only the share of crust over 50 km may fall.

The band also loses less at the cap. Today, crust that finds no room is removed as `AdditionsLost`
(delamination). A wider band keeps more of that mass, so the stock of continental crust and the mean
land rise over 1000 Myr.

The band also has a capacity problem. A belt creeping at 0.3 cm/yr for 300 Myr takes up about
900 km of shortening, or ~900 km × 35 km ≈ 3·10⁴ km² of crust per km of belt. A 500 km band filled
from 35 km to 75 km holds 500 × 40 = 2·10⁴ km² per km. Long-lived belts saturate the band within
~200 Myr. After that the crust spills further, up to 8 more rings, and the area over 50 km grows
again.

**Fix:**
- Make stacking rate, mean land and `AdditionsLost` primary criteria for the 0.2 decision, next to
  crust > 50 km.
- Report the cumulative `AdditionsLost` per set (ledger).
- Report the stacked mass per km of collision boundary, so that saturation can be seen.

### 3. Softness s⁻³ with a strength floor of 0.05 makes weights differ by up to 8000×. Crust lands in one cell, which fills to the cap. (high)

Two strength values bound the softness range:
- `lithosphere_strength` floors at 0.05 (`MIN_STRENGTH`), so s⁻³ reaches 8000.
- Orogenic crust has strength 0.5, so s⁻³ is 8.

Within the band, continental-origin cells with low strength are common, and they sit exactly where
collisions start:
- Recently stretched margins: `stretch_rift` resets `birth`, so the thermal factor √(age/100) is
  0.1–0.3 for 1–10 Myr.
- Thinned margins: their thinning factor is below 1.
- Young accreted terranes and arcs, if they count as continental-origin columns. The plan's "0 for
  oceanic crust" leaves transitional arc crust unspecified.
- After #75, plume-reheated crust.

Each addition is the crust of one overridden sample, ~35 km thick or more. One cell with a weight
10³ times its neighbours' takes the whole share, up to its room. The leftover then spills "nearest
first" and ignores the weights (see #6). The `room` factor is read once per addition, before the
addition is applied, so it does not taper the share inside one addition. The likely result is the
old pattern again (cells filled to 75 km next to the contact), just in different cells, and the
share of crust over 50 km may not fall at all.

Thin-sheet physics does support ε̇ ∝ s⁻ⁿ for elements in series under the same stress. But
`lithosphere_strength` is a mechanical-thickness proxy built for choosing where plates break. Its
floor of 0.05 marks the axis of a spreading ridge. It is not calibrated as an integrated viscous
strength B for continental lithosphere.

**Fix:**
- Bound the contrast in `band_strength`. Clamp s to ≥ 0.3–0.5 for this use, which gives
  ≤ 8–37×, or cap the softness directly. Name the bound in the doc comment.
- Distribute by water-filling. Iterate the weighted split over the cells that still have room,
  with 2–3 passes, and re-evaluate `room` each pass, before spilling. Then the GPE limit actually
  spreads the crust.
- Add an ablation set, "band without softness" (taper × room only), on seeds 0–3. This shows
  whether the softness helps or concentrates.
- Add diagnostics per run: the share of each collision addition taken by its largest receiver, and
  the share taken by ring 0.

### 4. Orogenic marking creates a feedback, and the band enlarges the permanently weakened area. (medium)

The marking rule stays unchanged. A cell is marked fully (`orogenic = 1`, via `mark_orogeny`) if it
gets `fitting > 0`, is of continental origin and is thicker than 35 km. In a 500 km band, every
cell over 35 km that receives even metres of crust is marked as a whole. On `main`, marking is
permanent: there is no healing until #75. It halves the cell's strength.

That has two effects:
- (a) A self-reinforcing ×8 attraction inside the band. Marked cells take up to 8× more and stay
  marked. Only `room` stops this. With softness 8, `room` at 70 km (0.165) still gives a weight of
  1.3. An unmarked cell at ring 3 has weight taper × 1 ≈ 0.5. Near cells therefore fill to
  ~72–73 km before the far rings get a comparable share.
- (b) A larger weak area. Because `lithosphere_strength` also drives the weak-zone cuts, this
  changes where plates break, and so rifts and plate births: a guard-rail metric that moves for an
  unrelated reason.

**Fix:**
- Mark only where the added thickness is material, for example > 1–2 km or > 5% of the column.
  Alternatively, mix the mark by the mass fraction added (`mix_orogeny` with
  share = added / new mass) instead of marking the whole sample. This changes the test
  `collisions_deform_the_whole_sample`; state that change.
- Report the orogenic-marked area (ever marked, and `orogenic` ≥ 0.5) per set.
- With the softness bounded (#3), feedback (a) also becomes small.

### 5. The `room` limit uses the 75 km numerical cap instead of the GPE balance the plan derives. (medium)

The plan's physics point 2 says thickening stops where ΔGPE equals the boundary force. Its own
numbers:
- ΔGPE ≈ 2.7·10¹² N/m at 50 km and 7.7·10¹² N/m at 70 km. I checked the arithmetic:
  ½ · 9.8 · 2800 · 0.1515 · (h² − h₀²) gives 2.65·10¹² and 7.64·10¹².
- The model's collision resistance is 2–3·10¹² N/m.

So by the plan's own physics, thickening under the model's force would stop at about 50–53 km, and
under a Tibet-like force at about 70 km. `max_crust_thickness` = 75 km is a numerical cap. Using it
as h_max means the plateau saturates at 75 km, not at the 65–70 km the plan cites, and the weight
falls to ~0 only near the cap.

**Fix:**
- Define room from a limiting force: room = clamp(1 − ΔGPE(h) / F_max, 0, 1).
- Take F_max as Tibet's ~7.7·10¹² N/m (Molnar & Lyon-Caen 1988; England & Molnar 1997), which is
  equivalent to h_lim ≈ 70 km. This is derived, not tuned.
- Keep 75 km as the hard cap for spill.
- If finding 1's GPE resistance is added, use the same ΔGPE function there.

### 6. Bookkeeping in `apply_additions_inner`: spill, fallback and ring count. (medium)

- **Spill ignores weights.** The second loop walks `reached` in BFS order and fills the nearest
  cells with room first, including strong cratonic cells that the softness deliberately avoided.
  The water-filling pass from #3 removes most of the spill. For the rest, spill in descending weight
  order within a ring, or keep nearest-first and say so.
- **Fallback.** The plan says: "If no cell has positive weight, the old profile is used." Today
  `total_weight <= 0` means `continue`. The addition is then dropped and shows up only in
  `AdditionsLost`, because `apply_additions` compares nominal and actual mass. The fallback needs a
  second `reached` pass with profile weights, and BFS must still reach `rings + SPILL_RINGS` rings
  for spill. Test that a collision against an all-oceanic overriding margin conserves mass as
  before.
- **Sediment.** Sediment is split by `weight / total_weight` with no cap. For Collision additions
  it is 0 today, because sediment becomes metasediment. Keep an assertion or comment so that a later
  change cannot route sediment by the s⁻³ weights.
- **Off-by-one.** With a ~110 km spacing, `rings = ceil(500/110) = 5`, and the taper
  `1 − ring/(rings+1)` gives 1 … 0.17 over rings 0..=5 (~550 km). Say whether ring `rings` is
  included. Also, the ring counts distance from the contact *cell* (radial BFS), not from the
  boundary. That is fine, but the test "none beyond `collision_band_width`" must use rings, not
  distances.
- **Order.** Additions are applied one after another within a substep, and the weights read the
  state as updated by earlier additions. This is deterministic and fine. State it.
- **Tests.** Besides the mass tests, add one: the orogenic mass after one addition matches the
  marking rule, and with #4 the marked area is bounded.

### 7. The measurement cannot detect the expected effect on crust > 50 km. (medium)

Batch 4 on `main`: crust > 50 km is 4.45%, ranging 0.8–9.2% by seed, so the seed SD is about 3
points. With n = 8 per set, the smallest difference Welch t reaches 2.1 on is ≈ 2.1 · 3 · √(2/8) ≈
3 points. A halving from 4.5% to 2.2% would show as "not significant". Most of the scatter comes
from how much collision a seed has: collision length 0.9–35·10³ km.

Pairing seeds does not help. Branch and `main` diverge after the first collision.

With ~50 metrics, a few with |t| > 2 are expected by chance.

**Fix:**
- Add a **controlled collision diagnostic**: a deterministic scenario or test with two continents
  converging at a fixed 1–2 cm/yr for 50–100 Myr, run on old profile vs band. Compare the thickness
  profile across the belt, the belt width, the maximum thickness and `AdditionsLost`. This answers
  "is the band wider and lower" without seed noise.
- In the runs, **normalise by collision activity**: the area over 50 km per km of collision
  boundary, or per cumulative stacked mass. Width of thickened belts is already planned.
- Name 4–5 primary metrics in advance: crust > 50 km (normalised), slow share, continental share,
  mean land, stacking. Treat the rest as guard rails.
- Use 16 seeds for the two sets the 0.2 decision rests on (`main-s02`, `band-s02`). The runs are
  single-core and ~500 s each, so this is cheap.

### 8. A decision rule fixed in advance keeps the suture recalibration from being credited to the band. (medium)

The `main-s02` control is the right design. But the stated rule ("if `band-s02` keeps crust > 50 km
within ~3–6% … propose 0.2") can pass even if the band does nothing. `main-s02` may also land in
3–6% by chance, given the scatter; v1's 8% was under a different suture rule.

**Fix:** write the rule down before running. Propose 0.2 only if all of these hold:
- (a) `band-s02` is in band on crust > 50 km, mean land and stacking;
- (b) `main-s02` is not, or `band-s02` vs `main-s02` differs significantly on the thickness metric
  in the direction expected;
- (c) the controlled diagnostic (#7) shows the band widening the belts.

Also measure beforehand, on the existing `main` runs or a cheap logging run, what share of colliding
pairs stall with a mean convergence between 0.2 and 0.5 cm/yr. That predicts how much 0.2 changes
at all: if almost every stalled belt creeps at ~0.3 cm/yr, 0.2 means "almost never suture".

Report the knock-on effects on sutures, rifts and plate births per 100 Myr and on the largest
continent share. #43 showed fewer sutures → fewer rifts → fewer plate births.

### 9. The taper and softness are combined inconsistently with the cited law. (low)

If strain rate ∝ (τ/B)ⁿ and the taper stands for the decay of *stress* into the plate, the weight
should be (taper/s)ⁿ, which is much steeper (ring 4 of 5: 0.33³ ≈ 0.04). If the taper stands for
the decay of *strain rate* for uniform strength (England, Houseman & Sclater 1985), taper × s⁻ⁿ is
right.

**Fix:** state which one is meant in the doc comment. Prefer the second, since that paper gives
strain-rate length scales. The plan does not give that paper's length-scale factor. The cited
"~1000 km behind a ~2500 km indenter" supports 500 km as a conservative choice. Note it as such.

### 10. Rift GPE weighting: the "same total" is not automatic. (low)

In `stretch_rift` the weight sets each donor's *fraction* of its own thickness
(`base / ring`, capped at `1 − breakup/h`), and `base` is normalised by the count and the sum of
1/ring. Multiplying the fraction by (h/h₀)² gives an effective ∝ h³ in thickness, and it changes the
total given unless `base` is renormalised with the new weights. The cap `1 − breakup/h` can also
bind differently.

**Fix:**
- Specify the weight as w = (h/h₀)²/ring on the fraction (or on thickness, decide which).
- Renormalise: base = count/(count+1)/Σw.
- Test that the new sample's thickness for uniform 35 km donors equals today's.

The physics (extensional collapse of thick crust; Buck 1991; Hyndman et al. 2005) is fine.

### 11. Interaction with #75. (low)

After #75, orogenic weakness heals and plume reheating moves onto the crust, so the band's weights
change: young belts and plume-heated crust become the attractors. If #75 lands before this PR, the
measurement on bare `main` no longer describes what ships.

**Fix:**
- Run the final sets on the rebased branch if #75 merges first, or at least a 4-seed check with #75
  merged.
- Bound the softness (#3) so that plume-reheated crust (thermal factor ~0.5 at the centre) does not
  become an 8× attractor in any collision near a plume.

### 12. Citations and smaller points. (low)

- The citations are appropriate at abstract level: England & McKenzie 1982; Houseman & England
  1986; England & Houseman 1986/1989; Molnar & Tapponnier 1975; Vilotte 1982; Neil & Houseman 1997;
  Buck 1991; Gordon 1998 (diffuse boundaries ~15% of the surface). n ≈ 3 is the standard
  olivine/thin-sheet value.
- "Tien Shan ~1000–2000 km north of the Himalayan front": ~1000–1500 km is closer.
- The GPE formula leaves out the thickening of the lithospheric mantle, which lowers ΔGPE. That is
  acceptable for a rough limit, but say so.
- **Lower plate:** leaving it out is right for this representation. The upper-plate side
  dominates in Tibet. Keep it as the named caveat.
- **Runtime:** BFS depth grows from 12 to ~13–14 rings (rings + `SPILL_RINGS`), plus one
  `lithosphere_strength` call per reached cell. Negligible; the paired runtime check is enough.
- **Kinematics:** crust is moved up to 550 km inland in the same substep, as thickening without
  horizontal shortening of the band. This is acceptable for rigid plates and is named in the plan.

## Most important points

1. The band cannot move the slow or continental boundary shares. Collision resistance does not
   depend on thickness, so only `suture_convergence` = 0.2 can, at best to about v1 (21% / 19%).
   To make belt thickness and width feed back into speed, add a GPE-dependent collision
   resistance, or record the boundary targets as not addressed.
2. With 0.2, stacking (~26 Gt/yr) and mean land (~1.0 km) stay out of band whatever the band does.
   Make them primary criteria, report `AdditionsLost`, and check band saturation for long creeping
   belts (~3·10⁴ vs 2·10⁴ km² per km).
3. s⁻³ with the 0.05 floor gives up to 8000× weights. Together with per-addition room, nearest-first
   spill and permanent whole-sample orogenic marking, this likely fills a few cells to 75 km again.
   Bound the softness, use water-filling, mark only on material thickening, and run a "no softness"
   ablation.
4. Base `room` on ΔGPE against a Tibet-like force (h_lim ≈ 70 km), not on the 75 km numerical cap.
5. With n = 8, the thickness metric can only detect changes of about 3 points. Add a controlled
   two-continent collision diagnostic, normalise by collision activity, name the primary metrics
   and the 0.2 decision rule in advance, and use 16 seeds for the s02 sets.
