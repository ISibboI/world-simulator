# Plan: plume rifts that propagate across the plate (#51, option 3)

_2026-11-27. Sebastian chose option 3 on PR #195 ("number 3"). Background:
[`results.md`](results.md) (why the physics of the PR alone gives no plume rifts),
[`plan.md`](plan.md). Goal: an Earth-like planet, not Earth; effects of the ~110 km grid are
accepted (#64). Physics before tuning; one new parameter, from the literature._

## The problem

#182 breaks a plate along a section only when a rigid-plastic limit analysis of the **whole**
section gives an overload `S > 1` for 5 Myr. A plume weakens the lithosphere in opening only
over its head (radius ~1000 km, magma factor 0.1, Buck 2004/2006) and pushes it apart by its
swell, but a section must cross the whole plate (often 4000–10 000 km). In the PR's runs the
best section through a plume reached `S ≥ 1` in 0.9% of the analyses; 2 plates broke along one
in 8 × 1000 Myr.

In nature plume rifts **propagate**: the dykes fed by the plume open a rift at the plume; the
load the rift no longer carries is concentrated on the intact lithosphere ahead of its tip,
which yields there and lets the dykes advance; melt is channelled along the opening rift
(Afar: Ebinger & Sleep 1998, plume material channelled > 1000 km along thinned lithosphere;
dykes of giant radiating swarms run 1000–2500 km from a plume centre, Ernst & Buchan 1997).
The rift crosses the plate segment by segment over millions of years (Courtillot 1982,
"propagating rifts and continental breakup"; Afar/Red Sea/Gulf of Aden; Woodlark;
South Atlantic). The plate never has to yield along the whole section at once.

## Model

### Criterion: can a magma-fed rift open the plate along this section?

A rift tip in a loaded plate concentrates stress (in the elastic limit without bound), so the
intact lithosphere at the tip yields long before the whole section does. What limits a
propagating rift is therefore not the strength of the intact section but the energy balance of
the whole process: the work the plate's loads do when the two parts separate must pay for
opening the rift at the strength it has **once it is magma-fed**. In #182's terms (upper-bound
limit analysis, the same mechanism `ω`):

`S_rift = max_ω (ω · T_A) / D_rift(ω)`, where `D_rift` is #182's dissipation with the
strength in **opening** reduced by the plume's magma factor `m` along the **whole** section
(`s_open = max(s · m, min(s, MIN_STRENGTH))`); sliding and closing keep their full strength
(dykes only open).

`S_rift ≥ 1`: the loads can open the plate along this section as a magma-fed rift, so a rift
that starts at the plume propagates along it. `S_rift < 1`: even a magma-filled rift along the
whole section could not be opened by the loads, so the rift stalls (and its dykes freeze).

`m` is the magma weakening at the plume's centre now (`weak_zones::magma_weakening`, 0.1 while
the plume is at full flux, less while the head arrives or the plume fades; no rift from a plume
without flux). The rift's magma comes from the plume along the rift; that it reaches the whole
section is the stated assumption of this model (see Caveats).

### Time: the rift propagates at a finite speed

The tip advances at a speed `v_p` while `S_rift ≥ 1`. The plate breaks when the rift has crossed
it: when the time the plate's best rift section has been loaded (`rift timer`) reaches

`t_cross = max(L / v_p, breakup.time)`, `L` = the distance from the edge of the plume's magma
footprint (head radius during the head stage, tail radius after) to the farthest point of the
section (the rift propagates from the plume along both arms at once; 0 where the head already
covers the section). `breakup.time` (5 Myr) is #182's localisation time.

While `S_rift < 1` the timer runs back as fast (the yielding zone heals as #182's timer does:
the dykes freeze). A split or merge of the plate resets it (its sections change), like #182's.

**The one new parameter: `breakup.propagation` = `v_p` = 50 km/Myr (5 cm/yr).** Published rift
propagation rates span about an order of magnitude: ~1.5 cm/yr for the propagation of
continental breakup in 3D models (Jourdon et al. 2020, JGR), ~2 cm/yr for the westward
propagation of spreading in the Gulf of Aden (spreading at Shukra El Sheik at 17.6 Ma,
the western gulf at 10 Ma, Tadjoura 2 Ma; d'Acremont et al. 2010), ~14 cm/yr for the Woodlark
basin (Taylor et al. 1995), and the South Atlantic opened northward from the Paraná–Etendeka
plume over ~4000 km in a few tens of Myr (~10 cm/yr). 5 cm/yr is the middle of this range on a
log scale; it is not tuned. Sensitivity (not calibration): 4 seeds × 1000 Myr each at 1.5 and
14 cm/yr. At 5 cm/yr a rift crosses 2000 km beyond the head in 40 Myr, against plume lifetimes
of 80–200 Myr.

### Where the rift runs

The candidate rift sections are the PR's sections through each active plume above the plate
(two arms in 12 directions, 90–150° apart or straight, 42 per plume), ranked by `S_rift`
(quick estimate for all, full limit analysis for the best per plume); the best one over all
plumes is refined along the weakest lithosphere within `breakup.band` cells with the same
min-cut as the other sections, using the magma-fed opening strength, if `S_rift > 0.5`; the
more loaded of the two is kept. The plate's ordinary #182 search is unchanged.

### Breakup

Per analysis (every 2 Myr) and plate: if #182's ordinary criterion is due, it breaks as before.
Otherwise, if the best rift section has `S_rift ≥ 1` and the rift timer has reached `t_cross`,
the plate breaks along that rift section (the smaller part becomes a new plate), counted as a
**plume rift** (`SplitCause::PlumeRift`, the old rule's cause, now "plume rift"; included in
`births()`). Both parts ≥ `breakup.min_area` as for every breakup.

Approximation: the timer belongs to the plate, not to a fixed rift path: if the best rift
section moves between analyses (another orientation or plume), the time carries over. Same
approximation as #182's timer; the rift's path is not stored in the lithosphere.

### Consequences inside the existing machinery

* The rift is not carried as a weak zone before it breaks (no partial rift weakening in the
  sheet). After the break the new boundary is a ridge/rift like any other, and stretching marks
  the extension fabric (#54) as before.
* The magma weakening of the head (`magma_weakening`) in #182 and the sheet stays as in the PR.

## Implementation

* `breakup.rs`:
  * `BreakupParams::propagation` (km/Myr, `--set breakup.propagation=`; 0 switches the rifts
    off, for the ablation).
  * `section_edge`/`section_overload` take an optional magma factor for the whole section
    (`tension = min(tension, dyked(shear))`).
  * `plume_sections` computes for each wedge the quick ordinary and the quick magma-fed
    overload in the same pass (one torque, two edge lists), keeps the best of each per plume,
    analyses both fully; returns the best ordinary (as now) and the best rift candidate
    (side, `S_rift`, opening, reach `L`).
  * The band refinement and the level field of a side are factored out into helpers, shared by
    the ordinary sections and the rift.
  * `analyse_sections`: rift timer, trigger, statistics; `SectionRecord` gets `rift_overload`,
    `rift_timer`, `rift_reach_km`.
  * `break_overloaded_plates`: cause `PlumeRift` when the pending breakup is a rift.
* `sheet.rs::PlateSheet`: `rift_timer`, `breakup_rift`; reset with `breakup_timer`
  (`topology.rs`).
* `events.rs`: `SplitCause::PlumeRift` documentation/name ("plume rift"), `births()`.
* Statistics/log: propagated plume rifts and the `S_rift` distribution (< 0.25, < 0.5, < 1,
  ≥ 1) per analysed plate with a plume; `compare.py` accepts both "plume rule" (main) and
  "plume rift".
* Tests: a plate with a plume and a uniform pull apart whose ordinary overload is < 1 and whose
  magma-fed overload is > 1 breaks along a section through the plume only after `L / v_p`;
  with the pull too weak (`S_rift < 1`) it never breaks; `propagation = 0` never breaks by a
  rift; `magma_weakening = 1` gives `S_rift = S`.
* Determinism: transcendental math through `math`, no hash-map iteration; check same state hash
  twice and with `--web-math`.
* Cost: one more edge list per wedge in the quick pass, one more full analysis and refinement per
  plate with a plume: expected a few % of the section analysis (which is ~5% of a step).

## Evaluation

8 seeds × 1000 Myr, `--duration 1000 --snapshot-interval 10` (current main's defaults), three
arms: **main** (current main + the diagnostics commit c77fad9, no state change), **PR before**
(the PR head merged with main, e844fdf), **now** (this model). Sensitivity: `propagation` 15 and
140 km/Myr, 4 seeds × 1000 Myr. Metrics per window (0–500, 500–1000 Myr): plate count, largest
plate (mean and max per seed), births/deaths, splits by cause, plume rifts per 100 Myr, splits
near plumes vs antipodes, breakups along plume sections, `S_rift` distribution, continental
area/land/flooding/thick crust, fabric numbers (#54), largest continental mass over time
(supercontinent dispersal) and from `--initial supercontinent` (4 seeds × 500 Myr, all three
arms), runtime (paired local 200 Myr). Independent evaluation of the results.

Expected: plume rifts again (order 1–3 per 100 Myr), plate count and largest plate closer to
main. If plume rifts are far more frequent than on main (e.g. > 5 per 100 Myr) that is reported,
not tuned away.

## Caveats

* Magma along the whole section is an assumption: dyke swarms reach 1000–2500 km, the rift itself
  melts by decompression once it thins, but very long sections (> 5000 km) may need more than a
  plume provides. No second parameter (a magma reach) is added.
* The propagation speed spans an order of magnitude in the literature; the sensitivity runs show
  how much it matters.
* Rift path not stored; the timer is the plate's (as #182's).
* Grid (#64): sections are resolved to a cell; 12 arm directions (30°) are the search's
  resolution.

## Overlap

PR #211 edits the slab part of `dynamic_topography` in `mantle.rs`; this change does not touch
`mantle.rs`.

## Revisions after the review ([`review-plan-propagation.md`](review-plan-propagation.md), approve with changes)

1. **Magma along the whole section (high).** Agreed that this assumption, not `v_p`, decides
   *whether* plates rift: without it the tip argument reduces to the PR's ordinary plume
   section. Kept as the model's stated assumption (an opening rift melts by decompression
   itself once thinned, and is fed along its axis), but its sensitivity is measured:
   `--set breakup.rift_reach=2000` (magma only within 2000 km of the plume, the reach of giant
   dyke swarms and of the Afar channelling) as a 4-seed arm. A switch for the sensitivity, not a
   tuned parameter; default 0 = the whole rift.
2. **Timer resets (high).** A split of the plate hands the rift timer to the part that contains
   the rift's plume (other part: 0); a merge keeps the longer of the two plates' timers. Only the
   split by the rift itself resets it.
3. **Pre-check and success criteria (high).** Before the 8-seed runs: 2 seeds × 400 Myr with
   `breakup.propagation=0` (rifts off; the `S_rift` distribution and the reach are logged), and
   2 seeds × 400 Myr with the model. **Fixed in advance:** success = plume rifts ≥ 1 per
   100 Myr over 500–1000 Myr, rift-type splits within 500 km of plumes above the antipode null,
   and the largest plate's mean (500–1000 Myr) within main's seed range. More than 5 plume rifts
   per 100 Myr or a plate count above main's range is reported as a problem, not tuned away.
4. **Tails (medium).** The plume's age when its rift started is counted (head stage < 20 Myr,
   < 50, < 100, older) and reported; if most rifts start from old tails, that goes to Sebastian
   as a caveat/decision (no restriction added now: tails also feed melt, and the head-only
   restriction would be another assumption).
5. **Timer across plumes (medium).** The plate keeps the rift's plume (its cell's position); a
   best rift from another plume (farther than the magma radius) restarts the timer.
6. **Plate drift relative to the plume (medium):** caveat; the section is re-evaluated every
   2 Myr through the plume's current position, so the break is at the plume at the time of the
   break.
7. A pending break is cleared by an earlier split of the same plate in the step (existing
   `split_plate_cells` behaviour). `propagation = 0` is handled before the division. Both edge
   lists are built in one pass.
8. **Citations corrected:** Gulf of Aden: spreading reached the western gulf ~10 Ma and the
   Gulf of Tadjoura ~2 Ma after starting in the east at 17.6 Ma (d'Acremont et al. 2010),
   a few cm/yr; South Atlantic: breakup propagated northward towards the Paraná–Etendeka
   plume (not from it), ~10 cm/yr; Jourdon et al. 2020's ~1.5 cm/yr is a model rate (not
   verified in the full text). The range 1.5–14 cm/yr and its log-middle (~5 cm/yr) stand.
9. Evaluation additions: `propagation=0` gives the PR head's state hash; plume age at onset;
   reach and `S_rift` distributions (section records with `--set breakup.diagnose=1` in the
   pre-check); small plates. Sensitivity arms: `propagation` 15 and 140, `rift_reach` 2000
   (4 seeds × 1000 Myr each).
