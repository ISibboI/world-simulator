# Plan: plume rifts from physics only (#51) and failed arms as inherited weak zones (#54)

_2026-11-27. Background: [`check-2026-11-26.md`](check-2026-11-26.md) and
[`../issue-54/check-2026-11-26.md`](../issue-54/check-2026-11-26.md). Sebastian approved both
proposals and asked for them together ("do this and 51 together", #54). Goal: an Earth-like
planet, not Earth; effects of the ~110 km grid are accepted (#64). Earlier rule for #182: "No
shape rule, only physics."_

## Where we start (main at 1418167)

`plate_events` runs three split rules: `rift_above_plumes` (a plume older than `rift_delay` =
20 Myr under a plate with continental crust nearby, or under a large plate, splits it once by
the minimum cut with the PR #50 nucleation lines), `split_deforming_plates` (sheet, #163) and
`break_overloaded_plates` (#182, limit analysis of sections). The sheet and #182 see a plume
only through the heating of the head (`reheat_above_plumes`, strength ~0.5) and the drag of its
radial outflow. On main there are ~2.7 plume rifts and ~2.8 #182 breakups per 100 Myr.

## Changes

### 1. The swell's potential energy is a load (#51 step 1)

A plume head ponding below the lithosphere lifts it by `h` (dry dynamic topography, already
computed: `MantleFlow::dynamic_topography`, Gaussian of σ = 450 km, up to 650 m). The column
above the swell has more gravitational potential energy than its surroundings:
`ΔE ≈ ρ_m g h L` per unit area (the extra overburden `ρ g h` integrated over the lithosphere of
thickness `L`; the dry height is the one that sets the pressure at depth, also under water).
Its gradient is a body force on the lithosphere, `f = −∇ΔE`, pointing away from the plume,
exactly like the GPE of thickened crust that `cell_forces` already applies (Artyushkov 1973;
Fleitout & Froidevaux 1982; Bott 1993 for swell "push"). With `L` = 100 km (thickness of the
plate model's lithosphere, as `FULL_STRENGTH_AGE` assumes) the peak is ~2 × 10¹² N/m, the order
of ridge push (3 × 10¹²).

* `mantle.rs`: `MantleFlow::plume_swell_gradient(r)` (or a field over the cells): the analytic
  gradient of the plumes' Gaussian swells, sharing the swell formula with
  `dynamic_topography` (one helper for the height of a plume's swell).
* `sheet.rs::cell_forces`: add `−ρ_m g L ∇h · area / R` per cell (all cells of the plates
  in `deforming`; also used by #182, which calls it for every plate when it analyses).
* Only the plume swells, not the slab depressions: the approved scope is the plume; the slab
  part of the dynamic topography as a load is a separate question (it changes every plate's
  stress) and is left as a follow-up note.
* Like the crustal GPE, the swell load is not in the plates' rigid torque balance
  (`dynamics.rs`), only in the sheet and in #182, where the net torque of a plate's loads is
  removed. For a swell inside one plate its net torque is ~0 anyway; a swell under a plate
  edge pushes the plate (plume push), which the rigid balance already gets partly from the
  outflow drag. Kept consistent with the crustal GPE; noted as a caveat.

### 2. Magma-assisted weakening (#51 step 2)

Rifting by dyke intrusion needs about an order of magnitude less tectonic force than
amagmatic rifting of thick lithosphere: the dykes take up the extension in the strong upper
lithosphere, and only the opening pressure of the dykes must be overcome (Buck 2004, 2006;
Bialas, Buck & Qin 2010; Kendall et al. 2005 for Ethiopia). Where a plume supplies melt,
the lithosphere's **tensile** strength is reduced by `MAGMA_WEAKENING` = 0.1 (factor on the
depth-integrated strength).

* `weak_zones.rs`: `magma_weakening(plumes, r) -> f64` in `[MAGMA_WEAKENING, 1]`:
  `1 − (1 − MAGMA_WEAKENING) · intensity · (1 − (d/R)²)` per plume (the minimum over plumes),
  with the same footprint as the heating (`R` = `PLUME_HEAD_RADIUS` during the head stage,
  `PLUME_TAIL_RADIUS` afterwards; `intensity` = flux / peak flux: the head arriving, the plume
  fading). It is a field in the **mantle frame** (where the melt is now), not carried with the
  crust; it is not part of `lithosphere_strength` (the carried strength history), and other
  splits (subduction initiation's weak-zone cut) and the strength view do not see it.
* #182 (`breakup.rs`): the limit analysis gets separate strengths per section edge for
  opening (`s · m`) and for sliding and closing (`s`): dykes open, they do not help faults
  slip or thrust. `SectionEdge` gets per-edge weights for the three terms;
  `quick_overload`/`limit_overload` use them (the IRLS weights become per edge; the existing
  plastic-beam tests stay, a new test checks that magma weakening lowers only the tensile
  limit). The band refinement's min-cut uses `s · m` (the cut it looks for opens).
* Sheet (`sheet.rs`): the viscosity of a sample is multiplied by `m` at its world position
  (before the `min_viscosity` floor). The viscosity is isotropic, so here it also softens
  shear and shortening over the head; over a swell the stress is tensile, so this matters
  little (caveat). A factor on the viscosity (∝ depth-integrated stress at a given strain rate)
  is the analogue of the factor on #182's force, not a factor on the relative strength (which
  enters the viscosity to the power 6).
* `MAGMA_WEAKENING` = 0.1 is the only new number with real leverage. It is taken from the
  literature (Buck: ~10¹² instead of ~10¹³ N/m), not tuned. If plume rifts do not happen with
  it, that is reported as an outcome (see "Decision point" below), not tuned away.

### 3. Sections through plumes in #182's search (#51 step 3)

#182 looks at level sets of the distance from three far-apart cells; they rarely pass through
a plume. For every active plume above the analysed piece of a plate (its nearest cell in the
piece), add the sections along great circles through the plume in 6 orientations (30° apart)
as candidates, evaluated in full (`limit_overload`), and refined along the weakest lithosphere
within `breakup.band` cells like the level sections (the refinement code takes a level field;
the great circle becomes the level field of the signed distance from it in cell steps). These
are candidates of the search for the most overloaded section, not a rule where plates break:
a plate breaks there only if that section is the most overloaded and stays above 1 for 5 Myr.
Cost: 6 extra full analyses per plume and plate (~5–10 plumes) per analysis every 2 Myr.

### 4. Remove the plume-rift rule (#51 step 4)

Delete `rift_above_plumes`, `SplitAnchor::Rift`, `SPINE_LENGTH`, `RIFT_ORIENTATIONS`,
`PlateEventParams::{rift_delay, rift_continent_radius, min_oceanic_rift_area}`,
`Plume::has_rifted`/`mark_rifted`, the rift-only counters (`rifts`, `head_rifts`,
`failed_plume_rifts`, `rift_contact_km`, …) and their log lines, and the tests that exercise
them (replaced by tests of the new physics). Plume rifts become #182 breakups by opening (or
sheet splits) that happen to be at plumes. `split_locally` stays for subduction initiation.
The weak-zone module documentation is updated (plume rifts no longer use the cut).

### 5. Extension fabric (#54 step 2)

New `Crust::rifted` (0–1), the analogue of `Crust::orogenic` for extension: the remaining
weakening by extensional faults and shear zones in the lithosphere (strain softening of
faults: Huismans & Beaumont 2003; mantle fabric: Tommasi & Vauchez 2001; reactivation of
rifts and aulacogens: Buiter & Torsvik 2014, Manatschal et al. 2015).

* Marked by extension of continental-origin lithosphere: in the sheet's transport,
  `rifted += strain` (dilatation × dt, like `orogenic += shortening`), at most 1; in
  `stretch_rift` (boundary rifts that stretch margins), donors get `+ (β − 1)` and the new
  stretched sample is fully marked (it was stretched to breakup thickness). Saturation at
  strain 1 follows the strain-softening range of Huismans & Beaumont (softening between plastic
  strains of 0.5 and 1.5).
* Mixed by mass everywhere `orogenic` is (sheet `Mixture`, `stretch_rift`, `heal_gap`,
  merges in `topology.rs`), only on continental-origin lithosphere (`mix_orogeny` gets the
  fabric too); oceanic crust starts unmarked. Heals with `OROGENIC_HEALING_TIME` (200 Myr),
  in the same pass as the orogenic weakening (linear, commutes with mixing).
* Weakening `RIFT_FABRIC_WEAKENING` = 0.5 for a fully marked sample:
  `combine_strength(thermal, thinning, orogenic, rifted)` multiplies
  `1 − (1 − 0.5) · rifted`. Smaller than the suture's 0.25 because the thermal part of a rift's
  weakness (β heating, thinning) is already modelled separately; this is the mechanical part.
  It enters `lithosphere_strength` (so #182, weak-zone cuts and the view) and
  `sheet_strength` (so the sheet), like the orogenic term.
* View: `CellInfo` and the frame history get the fabric (one 8-bit channel, like
  `orogenic`); the strength mode shows it as its own weakness ("rifted"); the cell info panel
  lists it. Small UI change: a legend entry and a line in the cell panel; the touch/responsive
  rules are unaffected (no new controls).
* `--cell-dump` gets the fabric column for the analysis.

### 6. Failed arms (#54 step 1)

Nothing extra: with the swell load and the magma weakening the sheet stretches the plume
head where its lithosphere is weakest, the stretching marks the fabric, and the arms that do
not become a boundary stay as weak bands (1–3 cells wide at ~110 km, #64). No arm geometry is
prescribed.

### 7. Diagnostics (needed for the comparison, also on main)

A diagnostics-only commit on top of main (`agent/issue-51-base`, not for merging; checked to
give the same state hash as main) and the same code in the branch:

* Per new plate boundary from any split (plume rift on main, #182 breakup by opening /
  other, sheet split, passive-margin initiation): count; distance of the new boundary to the
  nearest active plume (≤ 500 km and ≤ 1000 km count as "at a plume"); continental boundary
  cells, of them on orogenic belts (≥ 0.25) and on fabric (≥ 0.25), against the same shares in
  the two plates (reactivation of belts and of failed arms / rifted margins).
* Per snapshot: area of continental lithosphere with fabric ≥ 0.25, split into within 2
  cells of a plate boundary and interior (interior bands = failed arms, aulacogens, old
  margins inside plates); shortening strain of the sheet on fabric ≥ 0.25 vs all (inversion
  of rifts).

## Evaluation

8 seeds × 1000 Myr, 6 subdivisions, the default flags of main (no `--epoch`/biosphere flags,
the same for both arms; PR #193 is not merged), `--snapshot-interval 10 --cell-dump` at a
coarser interval if the volume is large. Arms: `agent/issue-51-base` (main + diagnostics) and
the branch. Metrics (means per seed, then over seeds, Welch t):

* plume rifts (main: the rule; branch: splits within 500/1000 km of a plume), per 100 Myr;
* breakups by cause (#182 opening/shear/closing, sheet splits, initiations) per 100 Myr;
* plate count over time (0–200, 200–500, 500–1000 Myr), births and deaths;
* supercontinent breakup: first time the largest continental crust mass falls below 50% and
  below 35% of the continental crust; largest continental mass over time;
* continental area, land fraction, mean land elevation, flooded continental crust, crust
  > 50 km;
* fabric: weak area (interior vs boundary), reactivation shares on new boundaries vs plates,
  shortening on fabric;
* runtime (total and the breakup analysis' share; GitHub and local machines differ, so the
  runtime is compared on one machine for a 200 Myr run per arm if needed).

An independent reviewer evaluates the results.

### Decision point

Before removing the rule (step 4), a check with the rule still on and #182 in diagnose mode
is not meaningful (the rule rifts first). Instead: implement 1–3 and 5, run 2 seeds × 400 Myr
with the rule off and look at the overloads of the plume sections and the plume-related
breakups. If plumes do not rift at all with the literature values, the honest outcome is
reported as a decision for Sebastian (e.g. accept fewer plume rifts, or a stated magma factor
outside the literature range), not hidden in a knob. If they rift, run the full comparison.

## Risks

* Plume rifts drive the supercontinent cycle: fewer plume rifts → slower breakup, fewer and
  larger plates. The comparison is designed to show this.
* The fabric feeds back (stretching weakens, weak stretches more): localisation is the
  intended physics, but it could make continents deform much more. Watch the deforming area,
  continental area and plate count.
* Magma weakening of the sheet over oceanic plumes makes oceanic lithosphere over plume heads
  deformable (it is otherwise rigid with `oceanic_min_strength`); watch ridge jumps / oceanic
  splits at plumes.
* Grid (#64): the head is ~18 cells across; sections through it are resolved to a cell; a
  failed arm is a band 1–3 cells wide.

## Revisions after the review ([`review-plan.md`](review-plan.md), approve with changes)

All findings adopted, except where noted.

1. **Whole-section estimate, step 0 first (high).** The swell and the magma factor act only on
   the head's chord of a plate-wide section, so the expected overload of a plume section is
   small (reviewer: swell alone ~0.02–0.03; the section's strength drops only 1.3–2×). Step 0:
   swell load, magma weakening and plume sections implemented (they change nothing while the
   rule rifts first only if a plume section wins; measured with the rule switched off,
   `--set events.rift_delay=inf`, 4 seeds × 400 Myr), logging the overload of the best plume
   section per analysed plate. **Success threshold fixed in advance:** at least 1 split per
   100 Myr within 500 km of a plume, above the chance rate at the plumes' antipodes. Expected
   outcome stated beforehand: plumes mostly set *where* plates break, rarely *whether*. If the
   threshold is missed, that goes to Sebastian as a decision, with the physical extension the
   reviewer names (melt channelled along the opening rift, Buck 2006) as a separately stated
   assumption, not tuned in.
2. Swell load: the lithosphere thickness is `100 km × thermal term` (the heated lithosphere is
   thinner); no `gpe_factor` on it (that factor stands in for unmodelled parts of thickened
   columns). Same 4σ cut-off as `dynamic_topography`.
3. Magma in the sheet: noted that the viscosity floor caps it (~0.2 effective) and that oceanic
   heads stay rigid; the risk paragraph is corrected accordingly.
4. Fabric in `sheet_strength` (power 6): kept, as for the orogenic term (consistency of the two
   inherited weaknesses); stated up front that inversion in the sheet will be rare on cold
   lithosphere and that reactivation goes mostly through #182 and weak-zone cuts.
5. Fabric marking: only where the sheet stretches at ≥ 10⁻¹⁵ s⁻¹ (rifting rates), so creep
   noise does not ratchet up (main at 30 Myr: median positive dilatation of continental cells
   1.7 × 10⁻¹⁷ s⁻¹, 90th percentile 6 × 10⁻¹⁶).
6. Opening-only weakening: per-edge `a`, `b` (convex), tension floored at `MIN_STRENGTH`; test
   that shear and compression limits are unchanged and the tensile limit falls by 1/m.
7. Plume sections: the side next to the plume is the connected part touching the plume's
   cell; quick estimate for all 6 orientations, full analysis and refinement for the best only;
   6 is a resolution of the search.
8. Removal: also the matching test that relies on plume rifts, `births()` (now counts #182 and
   sheet splits, also on the baseline), the rift-only statistics and log lines, dead branches of
   `split_locally`/`local_split_piece`, docs in `weak_zones.rs`; `--set events.rift_delay`,
   `rift_continent_radius` (still used by initiations, kept), `min_oceanic_rift_area` go away
   (PR note).
9. **Supercontinent:** the default start is dispersed. Added a pair of arms with
   `--initial supercontinent` (4 seeds × 500 Myr) for the breakup time, and for the default
   runs the largest continental mass over time (aggregation and dispersal cycles).
10. Chance null: the same distances to the plumes' antipodes, and the mean distance.
11. Failed arms vs margins: the per-snapshot fabric statistics separate fabric on continental
    cells next to oceanic crust or a plate boundary (margins) from fabric deep inside
    continents (failed rifts / aulacogens); reactivation is counted for the interior kind
    separately. A disc-shaped stretched patch at the plume is an acceptable outcome (#64).
12. Ablation: `--set magma_weakening=1`, `swell_load=0`, `rift_fabric=0` (all physical
    defaults; switches for attribution, not tuning). One 4-seed arm with the fabric off. Deforming
    area and oceanic splits at plumes added to the metrics.
13. The new field is covered where the review lists (constructors, mixers, matching
    verification, history codec); `state_hash` does not include it (not relied on).
