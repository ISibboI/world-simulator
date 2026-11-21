# Plan: deformable plate margins (#69)

_2026-11-21. `main` at 536cecb (after PR #74). Goal: an Earth-like planet, not Earth; effects
of the ~110 km grid are accepted (#64) and named where they matter. Plates stay rigid (#70 is
the non-rigid change)._

## What is wrong

At a continental collision the overriding plate receives the crust of the lower plate that it
overrides (`resolve_ownership` → `CrustAddition { kind: Collision }`), and `apply_additions`
spreads it with a fixed profile `collision_profile = [1, 0.75, 0.5, 0.25]` over 4 rings of the
overriding plate around the contact (~330 km). Across a straight boundary this puts ~40% of the
shortening into the first row of cells, ~30% into the second. Crust above the cap (75 km) spills
outwards; crust above 40 km later flows down its elevation gradient (`crustal_flow`).

On `main` the result is already at the upper end of the calibration band: crust > 50 km
4.5% of the continental crust (0.8–9.2% by seed; batch 4, 500–1000 Myr), 7.0% (1.6–14.4%) on
the batch-4 branch. The first version of #68 (belts that creep without suturing) reached 8% with
mean land 1.04 km. #68 then let belts that creep slower than 0.5 cm/yr suture after 30 Myr,
which removed most of the slow continental boundaries it had gained (slow share 21% → 10.5%,
continental boundaries 19% → 11%; Earth 39% and ~30%, PB2002, notes/issue-43).

On Earth, convergence between continents is taken up over zones hundreds to >1000 km wide
(Tibet and the Tien Shan ~1000–2000 km north of the Himalayan front; Zagros ~300 km; the Alps
~200 km), and the crust there thickens by distributed shortening, preferentially in weak
(previously deformed, hot, young) lithosphere, until its gravitational potential energy (GPE)
balances the compressive force; then the zone widens instead of thickening further (plateaus
at ~65–70 km of crust). Diffuse boundaries cover ~15% of Earth's surface and move at
0.2–1.5 cm/yr (Gordon 1998).

## Physics used

Thin-viscous-sheet description of continental deformation (England & McKenzie 1982; Houseman
& England 1986; England & Houseman 1986, 1989):

1. **Strain rate ∝ (stress / B)ⁿ with n ≈ 3** (power-law creep of the lithosphere): for the
   same stress, lithosphere of relative strength s strains ∝ s⁻ⁿ. Belts of weak lithosphere take
   up most of the shortening (Tien Shan reactivated ~1500 km from the Indian indenter; Molnar &
   Tapponnier 1975; Vilotte et al. 1982; Neil & Houseman 1997 for strength heterogeneities in the
   thin sheet).
2. **GPE limits thickening.** The excess GPE of a thickened column under Airy isostasy is
   ΔGPE ≈ ½ g ρc (1 − ρc/ρm) (h² − h₀²) per unit length (England & Molnar 1997). With
   ρc 2800, ρm 3300, h₀ 35 km: 2.7·10¹² N/m at 50 km, 7.7·10¹² N/m at 70 km, i.e. Tibet's
   value (Molnar & Lyon-Caen 1988). Once the boundary force is used up, the thick part stops
   thickening and the deforming zone grows outwards (plateau growth; Argand number,
   England & McKenzie 1982). The collision resistance of the model (doc of
   `collision_resistance`: 2–3·10¹² N/m at 1–2 cm/yr) is in the same range.
3. **The deformation penetrates a distance comparable to the width of the indenting boundary**
   (England, Houseman & Sclater 1985: the strain rate decays away from a boundary over a length
   proportional to the wavelength of the boundary velocity, longer for larger n; I do not use
   their exact factor; Tibet: ~1000 km behind a ~2500 km indenter). I take one fixed band width (500 km behind the boundary, ~4.5 cells)
   and let points 1–2 shape the distribution inside it; the spill (below) lets it grow further.

In a rigid-plate model the relative motion of two plates at their boundary is fixed by the
plates. "A band takes up part of the motion" is represented kinematically: the area lost at the
boundary each substep (the overridden samples, v · L · dt) is redistributed as pure-shear
thickening of the band, mass-conserving. The plate's rigid rotation is not changed (that would
be #70). What changes is where the shortened crust ends up.

## Changes

### 1. Collision band (the core)

New module `src/tectonics/deformation.rs` (kept separate from `boundaries.rs` and from the
strength code of #75):

- `pub(super) fn band_weight(crust: &Crust, time: f64, ring: usize, rings: usize, max: f64) -> f64`
  = taper × softness × room, for crust of continental origin (column); 0 for oceanic crust
  (old oceanic lithosphere is strong and is not part of a continental orogen; at ocean-side
  contacts nothing changes).
  - taper = 1 − ring / (rings + 1) (linear, ring 0 = the contact cell): the stress of the
    boundary decays into the plate (point 3).
  - softness = s⁻ⁿ, n = `DEFORMATION_EXPONENT` = 3, with s = the lithosphere strength
    (point 1). **Interface to #75:** s comes from one function,
    `deformation::band_strength(crust, time)`, which calls
    `weak_zones::lithosphere_strength(crust, time, 1.0)` (thermal age, stretching, orogenic
    share; floor 0.05). After #75 lands this becomes `lithosphere_strength(crust, time)` and
    picks up the healing of orogenic weakness and the plume reheating with no other change.
    The plume-head cap (mantle frame) is not used here (1.0): it only exists for cuts on `main`.
  - room = clamp((h_max² − h²) / (h_max² − h₀²), 0, 1) with h = the crystalline thickness,
    h₀ = 35 km (reference continental crust), h_max = `max_crust_thickness` (75 km): the GPE
    argument (point 2), a column's resistance to further thickening grows with h² − h₀². No new
    parameter: the cap that already exists is where thickening stops.
- `apply_additions_inner`, `AdditionKind::Collision`: instead of `collision_profile`, the
  addition is distributed over the cells of the overriding plate within
  `collision_band_width` (new `TectonicParams` field, 500e3 m, settable with `--set`) of the
  contact, `rings = ceil(width / cell spacing)`, by `band_weight`. If no cell has positive
  weight (no continental crust within reach, or all of it at the cap), the old profile is used
  (terranes hitting an oceanic edge etc.). Spill of the leftover above the cap as now
  (nearest first, up to `SPILL_RINGS` beyond the band). Orogeny marking unchanged (crust of
  continental origin pushed above 35 km by a collision addition).
- Only the overriding plate's side, as now. The lower plate's side near the contact is consumed
  every substep (that is how the rigid plates represent its shortening); adding crust there
  would be re-consumed and stacked again, inflating the collision-stacking statistic (13–23
  Gt/yr calibration band) without physical meaning. The pro-wedge on the lower plate is
  therefore represented by the consumed edge itself. Named as a caveat.
- `collision_profile` stays for `AdditionKind::Shortening` (oceanic diffuse zones of #42/#73;
  oceanic lithosphere, not in scope).

### 2. Rift band (extension): GPE-weighted stretching

Continental rifts already stretch a band (`stretch_rift`: donors within `rift_width` = 200 km,
weight 1/ring, mass-conserving, thermal subsidence). The only change: donor weight × room⁻¹ in
GPE form, `(h/h₀)²` for h > h₀ (1 below): thickened crust in a rift band thins first
(extensional collapse of over-thickened crust drives wide rifts: Basin and Range, Aegean,
Tibetan grabens; Buck 1991, Hyndman et al. 2005). Same donor set, same total, same breakup rule.
No strength weighting for rifts: thinning lowers the thermal age and so the strength, which
would localise every rift (necking feedback) and shorten continental rifts, a separate question
(wide vs narrow rifts, Buck 1991), named but not done.

### 3. Sutures of creeping belts (measured, decision for Sebastian)

#68's threshold `events.suture_convergence` = 0.5 cm/yr was set to stop creeping belts from
stacking crust onto their edge cells. With the band, that reason may fall away. Earth's diffuse
continental zones converge at 0.2–1.5 cm/yr (Gordon 1998); 0.2 cm/yr is about the lower end
of active belts. I run both 0.5 (unchanged default) and 0.2 and report; the default is changed
only if hypsometry stays in its bands with 0.2 on the band code and Sebastian agrees (the PR
will state which way it went).

### Not done (named)
- Velocity partitioning inside the plate, true intraplate diffuse zones (Indian Ocean) — #70.
- A GPE force in the torque balance (thick belts pushing their plates apart, collapse into
  extension when the driving force goes away). Physically sound and the natural next step, but
  a change of the force balance with its own calibration; proposed as a follow-up.
- The band on the lower plate (see 1).

## Tests

- `band_weight`: taper, softness (orogenic share 1 → strength 0.5 → weight ×8), room 0 at the
  cap and 1 at ≤ 35 km, 0 for oceanic crust.
- Collision band conserves mass: one collision addition onto a continent: the mass added to the
  overriding plate equals the addition (ledger `AdditionsLost` = 0) as long as there is room;
  at the cap the leftover spills and only what finds no room is lost (as now).
- The band is wider than the old profile: the share of the addition in the first row is below
  the old ~40%; cells up to `collision_band_width` receive crust, none beyond (without spill).
- Weak lithosphere takes more: in a band with one orogenic stripe, the stripe thickens more than
  the same ring outside it.
- Plateau growth: repeated additions do not raise any cell above the cap; once the near cells
  approach the cap the far rings receive the larger share.
- Rift: GPE weighting conserves mass; a thick donor gives more than a 35 km donor in the same
  ring; the existing rift tests pass.
- Existing collision tests (`collisions_deform_the_whole_sample`, collapse tests, ledger tests)
  pass or are adapted where they assert the old profile.

## Measurement

Code: `main` (536cecb) and the branch. Sets (8 seeds 0–7 × 1000 Myr, `--duration 1000
--snapshot-interval 50 --width 256 --cell-dump`, via `runs`):

| set | code | suture threshold |
|---|---|---|
| `main` | main | 0.5 |
| `main-s02` | main | 0.2 (`--set events.suture_convergence=0.2`; control: creeping belts persist without the band) |
| `band` | branch | 0.5 |
| `band-s02` | branch | 0.2 |

Sensitivity (seeds 0–3): band width 300 and 800 km on the band code.

Metrics, mean over seeds of the 500–1000 Myr snapshot means, seed range, Welch t (n = 8):
1. Boundary classes with notes/issue-43/sim_classes.py: share slower than 2 cm/yr (Earth 39%),
   continental boundaries (rift + transform + convergent; Earth ~30%), continent–continent
   convergent length (Earth 23·10³ km), median relative speed.
2. Thickness/relief: crust > 50 km (target ~3–6% of continental crust), mean land (~0.84 km),
   highest point, hypsometry percentiles of land, flooded continental crust; width of thickened
   belts (area of crust > 40 km per km of collision boundary, from the cell dumps).
3. Guard rails with notes/realism-batch-4/scripts/compare.py and notes/issue-43/compare.py:
   plate speeds, trench/ridge/collision length, sea-floor production and age, collision stacking
   (13–23 Gt/yr), sutures, terranes, rifts, initiations, continental area.
4. Runtime: one paired local seed (main vs branch, `--placement local`).

Expected: `band` vs `main`: crust > 50 km and highest point lower, belts wider, mean land
about unchanged (same mass), boundary mix unchanged within scatter. `band-s02` vs `main-s02`:
the same slow/continental share, but thickness back in band. If `band-s02` keeps crust > 50 km
within ~3–6% and raises the slow and continental shares clearly, propose 0.2 as default.

## Interaction with #75/#53 (branch `agent/issue-75-53`)

#75 makes orogenic weakness heal (τ = 200 Myr) and carries plume reheating on the crust
(`Crust::reheated`), and changes the signature to `lithosphere_strength(crust, time)`. The band
reads strength only through `deformation::band_strength`; whichever PR lands second changes that
one call (and adds `reheated` to any new `Crust` literal in tests). With #75, old healed belts
attract less shortening than on this branch alone (softness 1.0–1.5 instead of 8 for a belt
older than ~300 Myr), so deformation will concentrate more on young belts and the thermal state;
that is the intended behaviour, and the measurement here is without it.

## References

- Buck, W. R. (1991), Modes of continental lithospheric extension, JGR 96, 20161–20178.
- England, P., McKenzie, D. (1982), A thin viscous sheet model for continental deformation,
  GJRAS 70, 295–321.
- England, P., Houseman, G., Sclater, J. G. (1985), Length scales for continental deformation
  in convergent, divergent, and strike-slip environments, JGR 90, 3551–3557.
- England, P., Houseman, G. (1986), Finite strain calculations of continental deformation 2:
  comparison with the India-Asia collision zone, JGR 91, 3664–3676.
- England, P., Houseman, G. (1989), Extension during continental convergence, with application
  to the Tibetan Plateau, JGR 94, 17561–17579.
- England, P., Molnar, P. (1997), Active deformation of Asia: from kinematics to dynamics,
  Science 278, 647–650.
- Gordon, R. G. (1998), The plate tectonic approximation: plate nonrigidity, diffuse plate
  boundaries, and global plate reconstructions, Annu. Rev. Earth Planet. Sci. 26, 615–642.
- Houseman, G., England, P. (1986), Finite strain calculations of continental deformation 1,
  JGR 91, 3651–3663.
- Hyndman, R. D., Currie, C. A., Mazzotti, S. P. (2005), Subduction zone backarcs, mobile belts,
  and orogenic heat, GSA Today 15(2).
- Molnar, P., Lyon-Caen, H. (1988), Some simple physical aspects of the support, structure, and
  evolution of mountain belts, GSA Spec. Pap. 218, 179–207.
- Molnar, P., Tapponnier, P. (1975), Cenozoic tectonics of Asia, Science 189, 419–426.
- Neil, E. A., Houseman, G. A. (1997), Geodynamics of the Tarim Basin and the Tian Shan in
  central Asia, Tectonics 16, 571–584.
- Vilotte, J. P., Daignières, M., Madariaga, R. (1982), Numerical modeling of intraplate
  deformation: simple mechanical models of continental collision, JGR 87, 10709–10728.

Sources cited from memory of the literature (abstract-level); the numbers used (n ≈ 3, ΔGPE at
70 km ≈ 7–8·10¹² N/m, belt widths) are textbook values.
