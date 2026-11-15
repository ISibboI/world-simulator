# Plan: thermal uplift and dense lower crust (issue 4)

_2026-11-15. Issue 4 in `../ISSUES.md`: thick young crust is 1–1.5 km too high (highest cells
8.4–11.4 km) because the oceanic thermal uplift `thermal_uplift(age)` (GDH1 depth–age curve,
up to ~1.8 km dry at age 0) is applied to every crust sample, including continental and arc crust
that inherited a young age from the oceanic sample it was built on; and 70 km crust stands at
~5.9 km (Tibet ~5 km, Altiplano ~4 km) because the crust has one density throughout. Research
(`../research/feasibility-2026-11-15.md`): continental crust needs its own geotherm (no GDH1
subsidence; reset only by rifting), and lower crust deeper than ~50–55 km densifies (granulite,
eclogite), which limits elevation and enables delamination._

## Current model (`src/tectonics/crust.rs`)

`dry_elevation = thickness · (ρm − ρ)/ρm + sediment · (ρm − ρs)/ρm + thermal_uplift(age) −
COMPENSATION`, with one density per sample (mass-mixed when crust is added), ρm = 3300,
continental 2800, oceanic 2900, arc 2800 kg/m³; `is_continental` ⇔ crustal buoyancy ≥ 3000 m.

## Changes

1. **Thermal uplift only for oceanic lithosphere.** Weight the thermal term by how oceanic the
   crust is: `w = 1` for crustal buoyancy at or below that of normal oceanic crust (~850 m),
   `w = 0` at or above the continental threshold (3000 m), linear in between. Continental and
   mature arc crust then gets no GDH1 term (its lithosphere has a continental geotherm), and the
   transition is continuous, so crust maturing from arc to continental does not jump in
   elevation. Thermal resetting by rifting (passive-margin subsidence) belongs to issue 7.
2. **Dense lower crust in thick orogens.** Crust deeper than 50 km (the part of `thickness`
   beyond 50 km) has density 3000 kg/m³ (granulite / partial eclogitisation), so it contributes
   only `(3300 − 3000)/3300 ≈ 0.09` m of elevation per m instead of ~0.15. With the current
   compensation, 35 km continental crust stays at ~+570 m and 70 km gives ~4.7 km (Tibet
   ~5 km). This affects only elevation (buoyancy); mass bookkeeping keeps the sample density
   (the densification is a phase change at constant mass, small enough to ignore in the
   ledger), and erosion removes crust from the top (upper-crust density).
3. The 75 km cap with spill-over stays (recorded as delamination in the ledger).

## Verification

- Unit tests: 35 km continental crust ≈ +570 m (unchanged); 70 km → 4.5–5.5 km; normal
  oceanic crust elevations unchanged (ridge −2600 m, old −5651 m); a continental sample with
  age 0 stands as high as one with age 1000 Myr; elevation is continuous across the continental
  threshold while thickening an arc.
- Runs (seed 0, 500 Myr, s6): max elevation and land-elevation percentiles (95th, 99th),
  mean land elevation, hypsometry (land/ocean area by height class), compared with the run
  before; continental fraction and land fraction.

## Not in scope

Thermal age reset by rifting and margin subsidence (issue 7), a lithospheric mantle root
term, delamination dynamics beyond the cap, calibration of continental thickness.

## Plan review (independent agent, 2026-11-15): sound with changes

Adopted:

1. **Ramp:** the proposed weighting (full at 850 m, zero at 3000 m crustal buoyancy) flattens and
   drowns young arcs (thermal term at age 0 is **2.1 km dry**, ~3 km wet, not 1.8 km; a 15 km
   arc on age-0 crust would drop from −840 to −2560 m) and makes subduction polarity fragile
   (young ocean would override arcs). Arcs and back-arcs are genuinely hot (Hyndman et al. 2005).
   → Keep the full thermal term up to the continental threshold (3000 m) and ramp it to zero at
   the buoyancy of normal 35 km continental crust (~5300 m); continuous, slope margin ~9%.
2. **Mass-consistent dense layer:** "higher density at constant thickness and mass" is
   contradictory. The part beyond 50 km contributes `excess · (ρ/ρd) · (ρm − ρd)/ρm` (its volume
   shrinks when it densifies), so `mass()` and the ledger need no change. With ρd = 2950 kg/m³:
   70 km → ~4.85 km, 75 km cap → ~5.2 km. ρd is an effective value between granulite and
   eclogite; at ≥ 3300 thickening would lower the surface, which needs delamination dynamics.
   Clamp ρd ≥ sample density. Depth-only densification (no kinetics) is acceptable here.
3. **Erosion response:** `erode()` uses one `bedrock_density` per cell for mass and for the
   isostatic rebound. With a dense root recomputed from total thickness, removing h from the top
   of a column thicker than 50 km lowers the surface by the dense-layer slope. → Pass a separate
   per-cell isostatic density to `erode()` (`ρm (1 − dElevation_dry/dThickness)`), keep the
   sample density for mass. (A persistent root field would be more physical — real eclogite stays
   dense when exhumed — and leads to delamination; deferred.)
4. `is_continental` stays consistent if the dense term lives in `crustal_buoyancy()` (still
   monotonic; threshold reached at ~22 km); the ramp weight uses the same function.

Verification additions: static before/after comparison on the same crust state; elevation
strictly increasing with thickness for ages 0, 1, 5, 20, 1000 Myr with a minimum slope; a
15–20 km arc on age 0–5 crust still overrides age-0 ocean in `buoyancy()`; erosion response of a
> 50 km column matches the elevation formula; run metrics incl. arc elevations and ledger.
