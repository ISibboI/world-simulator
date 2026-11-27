# Review of `plan.md` (#51 plume rifts from physics, #54 extension fabric)

_Independent plan review, 2026-11-27, code at 57ae1ed plus the uncommitted diagnostics in the
worktree. Code reading only, nothing was run._

## Verdict

**Approve with changes.** The physics is sound and it follows "no shape rule, only physics".
Swell GPE as a load, magma-assisted opening weakening and extra candidate sections are each
defensible, and the units match `cell_forces`. Two things need fixing before implementation.
(a) The plan's force comparison is per metre of section, but #182 judges a whole plate-wide
section. With that taken into account, the plume's own loads plus the 0.1 magma factor are
unlikely to rift plates on their own (finding 1). The decision point should expect this, and a
cheap measurement should come first. (b) The evaluation's supercontinent metric does not work
with the default `--initial dispersed` (finding 9). The other findings are medium or low.

## Findings

**1. (high) The plume can only weaken part of a plate-wide section; the overload estimate should come first.**
#182's overload is `ω·T / Σ_e F s_e ℓ_e (…)` over the *whole* section (`breakup.rs:300-387`,
`section_overload` 585-636). A section through a plume crosses the whole plate (3000–6000 km),
but the swell load and the magma factor act only over the head's chord (≤ 2000 km). The net force
of the swell on one side is the line integral of ΔE along the section:
ρ g L h₀ σ√(2π) ≈ 2×10¹² N/m × 1.1×10⁶ m ≈ 2.4×10¹⁸ N. A 4000 km section at s ≈ 0.5 carries
F·s·ℓ ≈ 10²⁰ N. The swell alone therefore gives an overload of about 0.02–0.03, not the 0.05–0.2
of the per-metre estimate. Averaged over the chord, `1 − 0.9(1 − (d/R)²)` gives m ≈ 0.4, so the
section's total strength drops only by a factor of ~1.3–2, not 10. In bending mechanisms the
closing end also stays at k·s = 3s.
In practice the plume will mostly **relocate** the breakups that far-field loads already nearly
cause (main: 2.8 per 100 Myr) rather than add new ones. This is physically honest (Buck's 10¹²
N/m refers to the whole rift length, and real plume rifts propagate along a magma-fed axis).
- Before writing the sheet/fabric code, add a **step 0**: in diagnose mode on the baseline,
  evaluate the plume great-circle sections with the swell load added to the analysis loads only
  and with m applied (no effect on the dynamics). Log their overloads against the plate's best
  section. This settles the decision point in about a day, before steps 1–5 are built.
- In the decision point, state the expected outcome ("plumes set *where* plates break, rarely
  *whether*") and fix a success threshold in advance, e.g. ≥ 1 opening breakup per 100 Myr
  within 500 km of a plume, against a chance null (finding 10).
- Possible physics extension if the outcome falls short, to report rather than tune: melt
  channelled along the opening section (Sleep 1997; Buck 2006 "magma supply along the rift"),
  i.e. m applied along an opening section's edges within a propagation length of the plume.
  This is a separate physical assumption and must be named as one.

**2. (low) Swell load formula: sound.** At a fixed depth inside a column lifted by h, the excess
pressure is ρ(z) g h, so ΔE = ∫₀ᴸ ρ g h dz ≈ ρ_m g h L. Under water the excess is
(ρ_m − ρ_w) g h_wet = ρ_m g h_dry, so using the dry height is right (`mantle.rs:346-349`
returns dry heights). The units of `−ρ_m g L ∇h · area / R`, with ∇h per unit sphere, match ridge
push (`sheet.rs:657-659`). The global mean that `dynamic_topography` subtracts (409-412) drops
out of the gradient. The 4σ cut-off (398) leaves a step of h·e⁻⁸, which is negligible; still,
the analytic helper should use the same cut-off. Caveats to note:
- L should be the *heated* lithosphere's thickness (thermal strength 0.5 at the centre → ~50
  km). A fixed 100 km overestimates the load by up to 2×. Either use L·thermal_strength or say so.
- The crustal GPE is multiplied by `sheet.gpe_factor` = 2, which is said to partly offset the
  sheet's discretisation stiffness (`sheet.rs:86-93`). Decide explicitly whether the swell load
  gets the same factor in the sheet. If it does not, say why. In #182 neither load should get it.
- **No double counting with the outflow drag.** The drag (`sheet.rs:565-582`) is the shear
  traction of the plume flow; the swell is its normal traction. The two are separate
  parameterisations of one flow (flux scales both), so they are consistent but not derived
  together. That is worth a sentence in the notes.

**3. (medium) Magma weakening in the sheet is mostly cut off by the floor and the rigid threshold.**
Using the defaults (`sheet.rs:108-116`: 2×10³¹, power 6, rigid 10²⁸, floor 10²⁷, continental
0.5):
- Continental head centre (thermal 0.5): s = 0.25, so η = 4.9×10²⁷. Times 0.1 that is 4.9×10²⁶,
  which the floor raises to 10²⁷, an effective factor of 0.2.
- At half the radius, η goes from 1.9×10²⁸ (rigid) to 6×10²⁷ (weak). Here magma mainly widens
  the weak disc.
- Oceanic heads stay rigid, since 0.5⁶·2×10³¹·0.1 = 3×10²⁸ > 10²⁸. Only young sea floor near
  ridges becomes weak, because the floor of 0.35 gives 3.7×10²⁷. The risk paragraph overstates
  how deformable "oceanic lithosphere over plume heads" becomes.
- Placing the factor on η, not on s, is right, and consistent with #182's factor on force. Say
  in the notes that the floor caps it.

**4. (medium) The fabric weakening goes into `sheet_strength` to the 6th power. That is inconsistent with the argument in point 2 and makes the effect a threshold.**
The plan argues that the magma factor goes on η and not on s, because s enters to the 6th
power. It then puts `RIFT_FABRIC_WEAKENING` into `combine_strength`, which `sheet_strength` raises
to the 6th power (`sheet.rs:507-511`). The orogenic term sets a precedent for this, but then
the plan's own argument should be applied to both, or the difference justified. In numbers: on
cooled continental lithosphere (thermal 1), η = (0.5·(1 − 0.5r))⁶·2×10³¹ falls below rigid only
for r > 0.87. With the 200 Myr healing, a fully marked margin or failed arm is deformable in the
sheet for only ~28 Myr. After that it is rigid unless it is also hot or orogenic. Consequences:
- The "shortening on fabric" (inversion) metric will show little.
- Failed arms will be reactivated only through #182, where the fabric acts linearly.
Say this up front, so that a null result for inversion is not read as a bug.

**5. (medium) Fabric marking: strain noise can ratchet it up.** The plan adds `rifted += strain` for
positive dilatation only (as orogenic does for shortening at `sheet.rs:1577-1592`). Dilatation
that oscillates in weak continental samples then accumulates. At a steady state with 200 Myr
healing, a background of 10⁻¹⁶ s⁻¹ (3×10⁻³ /Myr) gives fabric ≈ 0.6. Before choosing the rule,
check the distribution of `dil` in continental interiors from main's `--cell-dump`. Possible
fixes:
- mark only above a strain-rate threshold, as the sheet's `weak` classification does;
- or mark the net strain over the analysis interval.

Also, `stretch_rift` marks every boundary-rift margin fully (`boundaries.rs:1237-1355`). Fabric
area will therefore be dominated by passive margins, not failed arms (see finding 11).

**6. (low) Opening-only weakening in the IRLS: convex and easy to implement.** With per-edge
weights w_o (opening), w_c (closing) and w_s (sliding), the dissipation is
w_s|v_t| + a_e|v_n| + b_e v_n, where a_e = ½(w_o + k w_c) and b_e = ½(w_o − k w_c). This stays
convex because a_e ≥ |b_e| when the weights are ≥ 0.
- In `limit_overload` (`breakup.rs:338-362`), the scalar `a`/`b` become per-edge values, and
  `linear = Σ q_e b_e`.
- `quick_overload` (252-292) and the opening share (377-385) need the same change.
- `SectionEdge::halves` should keep a constructor with m = 1, so the plastic-beam tests (969-1030)
  stay unchanged.
- Add a test: with m < 1, a pure-shear mechanism's limit is unchanged and the opening limit
  falls by 1/m.
- Floor s·m at `MIN_STRENGTH` (0.05, the ridge-axis strength, `weak_zones.rs:75`). A magmatic
  rift is a nascent ridge axis, and should not end up weaker than the axis itself. This also
  avoids s·m ≈ 0.025 on plume-heated, thinned, orogenic crust.

**7. (medium) Great-circle candidate sections are acceptable but need care.** They improve the
search; they do not add a rule. Each section's S is a bound by the upper-bound theorem, and more
candidates only bring the result closer to the true maximum. The implementation needs these
details:
- **Side connectivity.** Distance sublevel sets are connected; half-planes of a non-convex piece
  are not. A disconnected side would be wrongly treated as one rigid body. Take side A as the
  connected component of {signed distance < 0} next to the plume, and B as the rest; then check
  min_cells for both. The pending split must also come out connected
  (`break_overloaded_plates`, 850ff.).
- **Signed distance.** The refinement takes a `u32` level field (`breakup.rs:776-847`). Use a BFS
  from the circle's crossing cells with an offset. Refine only the best plume candidate.
- **Cost.** The plan does 6 full analyses plus a BFS plus a min-cut per plume. Do instead: one
  BFS per orientation, `quick_overload` for all six, then `limit_overload` and refinement for the
  best one. Measure the share in `BreakupStatistics::seconds`.
- **Wording.** RIFT_ORIENTATIONS = 6 is removed, and the new search uses the same 6. State that
  this is a resolution choice of the search and not a physics parameter.

**8. (medium) Removal of `rift_above_plumes`: items the plan does not list.**
- `matching.rs:682-698`, the test `splits_and_merges_keep_both_modes_identical`, sets
  `rift_delay = 1` and asserts `births()+deaths() > 5`. Plume rifts are a main source of births
  in that test. The test needs a new split source, so that matching across splits stays covered.
- `EventCounts::births()` (`events.rs:396-399`) is `rifts + initiations + breakups` (detached
  pieces). #182 and sheet splits are **not** counted. After removal, births and plate turnover
  are undercounted, and the plan's "births and deaths" metric is wrong unless `births()` adds
  `breakup_statistics.splits` and `sheet_statistics.splits`. This also affects the baseline arm.
- `split_locally` (`events.rs:535-560`): the `initiation` flag and the Rift branch of the minimum
  area become dead code. The Rift branches in `local_split_piece` (702-906) go too.
  `record_split_geometry` (1113) and `split_contact_km`/`rift_contact_km` are rift-only, and so
  are `rift_suture_cells`, `rift_orogenic`, `rift_weak_cells`, `rift_motion_alignment`,
  `split_deviation_km[0]`, `weak_zone_splits` and `recut_splits` for rifts. Their log lines are in
  `main.rs:801-868`.
- Documentation:
  - `weak_zones.rs:1-16` (module doc);
  - `BAND_WIDTH` doc (79-81, "through the plume");
  - `PLUME_HEAD_STAGE` doc (66-72, "equals the default `rift_delay`").
- `mod.rs:484-485`: the `--set` names. Scripts that set them will fail. Mention it in the PR.
- `SplitCause::PlumeRift` (new): keep it, so that the baseline's tallies can be compared.
- Tests in `events.rs`, all using `has_rifted`: 2696-2880, 3166-3180 and 3228ff.; also
  `weak_zones.rs:713,814,840` and `mantle.rs:293,530,621`.

**9. (high) The supercontinent metric does not work with the default start.** The default is
`InitialConfiguration::Dispersed` (`generation.rs:20-26`, `main.rs:88-89`). "First time the
largest continental mass falls below 50%/35%" is meaningless for runs that start dispersed.
Instead, either:
- add an arm pair with `--initial supercontinent` (e.g. 8 seeds × 500 Myr; plume rifts matter
  most there, and the archive notes slow supercontinent breakup, 200–300 Myr); or
- measure cycles: the largest mass's up- and down-crossings of 50%, and the period.
Ideally do both.

**10. (medium) "At a plume" needs a chance baseline.** With 5–10 plumes and boundaries thousands
of km long, a large share of all new boundaries pass within 1000 km of *some* plume by chance.
Report the same share for random positions (e.g. plume positions rotated by a random rotation,
deterministic per seed) or for main's non-plume splits. Also report the distance from the plume
to the boundary, not only the threshold counts.

**11. (medium) Failed-arm reactivation needs failed arms told apart from margins.** Passive
margins (`stretch_rift`, fully marked) will dominate the fabric area. "Boundary on fabric vs
plates" then mostly measures margin reactivation, which is in part tautological, because new
rifts often form at existing margins. Options:
- tag the source of the fabric (sheet vs `stretch_rift`), e.g. a second diagnostic-only channel
  or a bit;
- or count only fabric that was ≥ 2 cells inside the plate before the split. Track the time
  since marking (median age of reactivated fabric).
Also state the expected geometry. With radial push on a nearly uniform sheet, the stretching
forms a *disc*, not arms. Arms only come from heterogeneity (sutures, orogenic belts), power-law
softening and the fabric's own localisation. A disc-shaped fabric patch at 110 km cells is an
honest outcome (#64), not a failure.

**12. (medium) Too few arms to attribute effects.** Two arms mix #51 (swell, magma, sections) with
#54 (fabric). If plate count or the supercontinent timing changes, the cause cannot be found.
Make `MAGMA_WEAKENING` and `RIFT_FABRIC_WEAKENING` settable (`--set`, for ablation, not tuning),
and add a 4-seed ablation arm with the fabric off. Also add metrics the plan names only under
risks:
- the deforming area of the sheet (continental and oceanic);
- oceanic splits and ridge jumps at plumes;
- weak-zone initiation geometry (initiation cuts will now follow fabric, `lithosphere_strength`
  is shared).

**13. (low) Places in the code to cover for the new field.**
- Non-test `Crust` constructors: `crust.rs:437-455`, `mod.rs:842,1068`, `initial_sediment.rs:173`,
  `matching.rs:94`, `boundaries.rs:1338,1432`, `sheet.rs:412-452` (Mixture).
- Mixers: `topology.rs:56-66` (collision stacking or spill) and `mix_orogeny`
  (`crust.rs:304-311`), which zeroes `orogenic` on oceanic mixtures and should treat the fabric
  the same way.
- `matching.rs:600-611` (`--verify-matching` bit comparison): add `rifted`.
- `history/codec.rs:149,344` (format change), `history/mod.rs:479` and `cell_info.rs`.
- `mark_orogeny` (`crust.rs:315`): keep the fabric on collision; inverted rifts remain weak.
- `state_hash` (`planet.rs`) does not hash `orogenic`/`reheated`, so it will not catch fabric
  divergence. That is acceptable, but do not rely on it for the fabric.
- The diagnostics commit must not draw from the RNG (`extension_fabric` stub at
  `events.rs:315` is fine).

**14. (low) Also fine:**
- Swell net torque inside a plate is ~0, and #182 removes the net torque by a drag-weighted
  rotation (`breakup.rs:658-672`).
- The magma field in the mantle frame, kept out of `lithosphere_strength`, is right: it is a
  current melt supply, not a history.
- Leaving the swell out of the rigid balance follows the crustal GPE. Keep the caveat.
- 0.1 is within Buck 2004/2006 (10¹² instead of ~10¹³ N/m) and is not tuned. That is fine as long
  as finding 1's limits are reported with it.
