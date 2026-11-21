# Review of the batch 4 plan and the uncommitted #40 change

_2026-11-21, independent review of `plan.md` and `git diff -- src` (not compiled, not run)._

**Verdict: approve with changes.** The bookkeeping is correct at the three mixing sites and I
see no compile errors. Before this lands, it needs a dilution measure next to the over-spread
measure, a rift-preference statistic that does not depend on a threshold, and a decisive
diagnostic for #52.

1. **Dilution is the new bias; measure it.** A minimum cut follows a line. A belt held as
   f = 1 in one cell row costs 0.5 per link along it. The same orogenic mass smeared over three
   rows at f = 1/3 costs 0.83 per link, almost as much as a craton. Conserving Σf therefore
   does not conserve how strongly belts attract rifts. `heal_gap` (boundaries.rs:1100-1129)
   fills resampling holes, which are numerical artefacts, not physical mixing. Repeated many
   times inside continents, it acts as numerical diffusion of f. A hole in a 2-cell-wide
   suture gets f ≈ 0.3-0.5. Physically, a 110 km cell with 30% orogenic lithosphere holds a
   ~35 km weak strip: a cut running along the strip is as weak as the strip (0.5), and a cut
   running across it is about the linear mean (0.85). `1 − 0.5 f` is right only for the second
   case. So the old one-cell mark was not wrong at grid scale. Only its growth over repeated
   mixings was. Change: log, over time, the histogram of f among cells with f > 0, the share of
   Σf held in cells with f ≥ 0.9, and `enclosed_fills` on continental crust per 100 Myr. If
   belts dissolve into f < 0.3 by 1000 Myr, compare option 1 (majority) on a branch before
   choosing.
2. **Rift statistic (events.rs:928).** Counting cells with `orogenic ≥ 0.5` makes the number
   incomparable with main's 52% / 26%, and it ignores the cells with f < 0.5 that the cut
   still sees as weak. Change: also accumulate the mean f on boundary cells and on all
   continental cells of the two plates (no threshold). Also keep the old count on `orogeny >
   NO_OROGENY`, which still follows the max rule in the branch, so the branch can be compared
   with main directly. Expectation (inferred): both the base share and the boundary share drop
   below main's, because there is less belt. The enrichment ratio (boundary / base) should stay
   near main's ~2. If it falls toward 1, dilution has made belts irrelevant.
3. **Mixing weight.** The doc comment (crust.rs:79-85) says "mass fraction of the crystalline
   crust". But `add_crust` (crust.rs:262), erosion and metamorphism change the crust mass and
   leave f alone, which is right because the weakness is in the lithosphere. So f is really the
   share of the cell's lithosphere, and Σ thickness·density·f is conserved only within one
   mixing event. Weighting by crust mass counts a 70 km belt donor double against a 35 km
   craton that gives the same share of its column. Change: either weight by the column share
   given (`share` in `stretch_rift` / `heal_gap`; `fitting / crust.thickness` against 1 in
   `add_crystalline`), or keep mass weighting and fix the doc and the plan's "conserves the
   weakened amount" claim. The first is more consistent and costs nothing.
4. **#52: by construction the raw cut touches the plume.** The spine (events.rs:681-690)
   forces cells next to the nucleus onto both sides. So any division in which both spines are
   non-empty has a boundary link within 1-2 cells of the nucleus. A boundary more than 500 km
   away can come only from: (a) post-processing. Pieces that are not the largest piece of
   their side flip (events.rs:778-842). A spine that is short on one side (plume close to an
   edge in that direction) can be enclosed cheaply, end up as a small pocket, flip away, and
   take the boundary near the plume with it. Inferred: orientations with a short spine are
   cheap, so `min_by` (events.rs:699) prefers them. (b) A spine with no cells on one side
   (plume at the plate edge). (c) A spine crossing the plume's own uncut rigid component on
   both sides. That orientation then returns `None`, so this explains failed rifts, not far
   ones. Majority reassignment (events.rs:724-738) is a no-op on the weak path, because
   `UNBREAKABLE` links (weak_zones.rs:350-357) never split a block, and it acts only on the
   great-circle fallback. The planned logs (`edge`, `nucleus_rigid`, `continent_distance`,
   `area`) are only indirect. Change: log inside `local_split_piece` the spine cell count per
   side, the distance from the nucleus to the boundary of the chosen raw cut and of the final
   division, and the number of cells flipped. Also log whether the nucleus lies in a rigid
   component other than `cut` (`nucleus_rigid` does not say this). A gap between the raw and
   final distances settles cause (a).
5. **#53 deferral: the reasoning holds** (verified). `birth` drives thermal uplift
   (crust.rs:113), slab age and ridge test (dynamics.rs:129-133), initiation hazards
   (events.rs:985-1554) and age statistics. The swell is in the mantle dynamic topography
   (mantle.rs:74-81, 133-134). One remark: a separate decaying "plume heat" f64 would cost
   the same as `orogenic`, so cost is no reason to defer. The real reasons are PR #50's
   ablation and #52. With `rift_delay` = 20 Myr, the heated lithosphere lies on a track of up
   to ~1000 km behind the plume, while the field sits at the plume. Mention this on #53.
6. **Places where crust is created or moved** (verified). Mixed correctly: `add_crystalline`
   (topology.rs:50-65; the receiver's fraction is computed before the add),
   `stretch_rift`, and `heal_gap` (the fraction is taken before the donor is thinned, with the
   same `given` and density as `mass`). In those three, zeroing f in non-continental mixtures
   is right (#31). Unchanged and right: crustal flow (boundaries.rs:157), because lower-crustal
   flow onto the foreland does not move mantle fabric; collision spill (741); arc and accretion
   additions; subduction erosion (413); metamorphism (mod.rs:1191); `apply_surface_changes`
   (mod.rs:1256); plate splits and hidden samples (topology.rs:156-190, which move whole
   samples); merges onto free vertices (topology.rs:354); and the consumed loser
   (boundaries.rs:263), whose lithosphere sinks, so losing its f is right. Collision mark
   (boundaries.rs:723-731): setting f = 1 is right at grid scale, because a new orogeny
   deforms the whole cell, and `thickness > OROGENIC_THICKNESS` still works as an
   "is this an orogen" test. The rule is not new, but any `fitting > 0` on crust already over
   35 km resets f to 1 up to 4 rings out. Note this; it is outside the scope of this change.
   `mark_suture` (events.rs:1107) is fine.
7. **Tests missing.** `heal_gap` (boundaries.rs:1325-1351) asserts only `orogeny`: add the
   weighted f and its conservation. `assert_same_state` (matching.rs:313-321) compares bits
   without `orogenic`: add it, or determinism of the new field goes unchecked. Add a test that
   a collision addition resets a diluted sample to f = 1. Add a `weak_zone_cut` test in which
   a belt at f = 1 attracts the cut and a three-row band at f = 1/3 does not (documents the
   dilution behaviour of finding 1). Validation: the plan's guard rails are good, but the
   over-spread and dilution measures need time series (statistics output or dumps at several
   times), not only the end-of-run cell dump column.
8. **Is it worth it?** For: the max rule causes drift, with the marked share rising ~4 points
   per 100 Myr to 44%. That is a growing artefact, not a fixed grid error. If almost half the
   continents are "belts", belts stop mattering, and the fix costs one f64. Against: per-cell
   binary marks are the right sub-grid aggregate for cuts along belts, and mass mixing swaps
   over-spreading for dilution. My view: worth doing, because it stops the drift and is cheap,
   provided findings 1-2 show that belts keep their contrast. If they do not, majority mixing
   in `heal_gap` only (pure artefact), with mass mixing elsewhere, is the fallback to measure.
