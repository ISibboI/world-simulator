# Plan: keep sediment on continental margins (issue 3 follow-up)

_2026-11-17. User: sediment first, then issue 1r. Context: issue 3 evaluation in `ISSUES.md`._

## Problem

The continental mass balance closes through a sediment loop ~5× Earth's: erosion 21–24 Gt/yr
[Earth 14–20], sediment arriving at trenches 14–18 Gt/yr [~3.5–4.5], ~82% of it accreted
(12–15 Gt/yr [≲ 2.5]). Nearly all sediment ends up on oceanic crust: shelves fill the ocean
outwards from each river mouth up to −120 m over up to 256 cells regardless of crust type, and
fans fill up to 2048 further cells up to −120 m (abyssal plains filled to shelf depth). That
oceanic crust reaches a trench within ~100 Myr. On Earth most sediment stays on passive margins
(shelf, slope and rise wedges up to 10–15 km thick on thinned continental and transitional
crust; Bradley 2008; ~80% of the sediment flux does not reach trenches), deep-sea fans are thin
far from the margin, and pelagic sediment is a few hundred metres.

Note: the issue 3 plan review advised against restricting shelves to continental crust (it
would push sediment onto oceanic fans); the evaluation recommends keeping sediment on margins.
This plan keeps it on margins by changing where the accommodation is, not by moving it to fans.

## Step 0: measure (scratch instrumentation)

Per step: sediment deposited on continental / transitional / oceanic crust; on oceanic crust,
the distance (rings) from continental crust; age of sediment-bearing oceanic crust at the trench;
share of sediment arriving at trenches from passive vs active margins; sediment stock by crust
class over time. Decide the design below on these numbers.

## Design (to adjust after step 0)

1. **Margin wedges first:** sediment fills accommodation on continental and transitional crust
   (shelves to `shelf_depth`, and below it the continental slope/rise wedge down to the
   foot of the slope) before reaching oceanic crust; where the margin is full, the wedge
   aggrades (the margin subsides under its load, isostasy already gives ~0.44 of the thickness
   under water) rather than prograding hundreds of cells over the ocean.
2. **Continental rise on oceanic crust near the margin only:** within a few rings (~300 km) of
   continental/transitional crust of the same plate, fill to a rise depth (e.g. −3000 m), so
   rise sediment rides with the continent's plate (passive margins rarely subduct).
3. **Fans thin with distance:** the rest spreads over the reachable ocean with a thickness that
   decreases with distance (e.g. exponential with ~500 km length), capped per cell; no more
   abyssal filling to −120 m. `max_fan_cells` as reach, not a fill limit.
4. **Overfill** as now (piles above the delta top), so mass is conserved.

## Targets and verification

- Sediment at trenches 3.5–6 Gt/yr; accretion ≲ 3 Gt/yr; net sediment subduction 2.6–4.5;
  erosion 14–20 Gt/yr; felsic + sediment within ±10% per 500 Myr (then the magmatic source
  must carry the balance: arc rate, subduction erosion — re-measure, adjust arcs only if
  clearly needed); mean land toward 0.84 km; shelf width (cells of shelf per km of coast),
  sediment thickness on margins (Earth: up to 10–15 km wedges).
- Tests: deposition conserves mass; with continental margin accommodation available nothing goes
  onto oceanic crust; fan thickness decreases with distance; no cell is filled above the delta
  top except as overfill.
- Runs: seeds 0–3, 500 Myr, `--material-budget`.

## Plan review (independent agent, 2026-11-17): sound with changes

Step 0 (seeds 0/1, 300 Myr, scratch `agent-scratch/review-sediment/`): at 275–300 Myr 85–89% of
ocean deposits land on oceanic crust (44–51% within 1–3 rings of the plate's own continent,
12–21% on another plate's ocean within 2 rings of a continent — active margins); shelves
migrate onto oceanic crust as continents emerge (only 7–19% of continental crust flooded);
64–72% of the sediment stock sits on oceanic crust, mostly within 3 rings of its continent;
**45–56% of trench sediment comes from own-plate passive-margin crust** (piles > 1 km), i.e.
passive margins are short-lived — margin consumption is a second root cause (issue 1 churn).
Earth: ~65–80% of the river load does not reach trenches (trench input ~1.5–2.5 km³/yr; Rea &
Ruff 1996, Plank & Langmuir 1998, Clift & Vannucchi 2004); Bradley 2008 concerns margin
lifespans, not the 80% figure.

Required: (1) measure what consumes passive margins and their lifetime (if < ~100 Myr the fix is
tectonic); (2) "aggrades as the margin subsides under its load" gives no room (local isostasy
already included) — real room needs rift thinning + thermal subsidence (issue 1r/7): consider
doing 1r stretching before or with this; (3) route by plate: own plate first, cross a boundary
only when full (needs crust class and plate in `erode()`); (4) target depths by ring from the
coast (shelf / slope −2000…−3000 m / rise −3500…−4500 m) with per-cell thickness caps (~10 km
margin, ~5 km rise, 1–2 km fans), never fill oceanic crust to shelf depth, thin fans with
realistic reach (~250 cells); (5) count overfill as a metric; (6) targets: net conversion to
metasediment (collision + accretion), sediment stock flat and mostly off oceanic crust, flooded
continental crust 25–30%, share of shelf cells on continental/transitional crust.

## Plan v2 (2026-11-17, after issue 1r follow-up and its evaluations)

User: terrane fixes, then sediment routing; asked whether wave erosion would help (no: it makes
more sediment and lowers land further); include compaction and conversion of deeply buried
sediment as candidate mechanisms.

### State now (evaluation `agent-scratch/eval-terranes/`, seeds 0–3, means 200–500 Myr)

- **Sea level rises ~580 m in 500 Myr** (dry offset −0.41 → +0.16 km) because the sediment stock
  never levels off (0 → 0.65–0.80e9 Gt at 200 Myr → 1.2–1.6e9 Gt at 500 Myr). This drives most of
  the flooding (30–47% of continental crust) and the low land (0.6–0.8 km; land on 30–35 km
  crust at 210–260 m, eroded to a rising base level).
- Margins: COB crust 25.5–28 km (β 1.18–1.29), −380…+80 m, 2.1–2.4 km sediment, no heat left;
  first ocean ring 6.1–8.3 km of sediment at −1.4…−2.4 km, ring 2 2.7–5.6 km, ring 5 0.6–1.8 km.
  Stretching covers only 26–44% of the kinematic continental opening.
- Erosion 21–24 Gt/yr [14–20], trench accretion of sediment ~12–15 Gt/yr [≲ 2.5] (issue 3
  evaluation; to re-measure).

### Steps

0. **Measure** (permanent diagnostics where cheap): sediment stock by crust class (continental,
   transitional, oceanic) and by ring from the COB; sea-level offset drift per 100 Myr; sediment
   sinks per process (subducted, accreted, overfill, conversion); age of sediment-carrying ocean
   at trenches; passive-margin lifetime (time from rift to consumption).
1. **Margin accommodation first:** find why stretching covers < half of the continental opening
   and why COB cells stay ~26 km (the opening test, the breakup criterion, resampling at the
   COB, erosion thinning?). Earth: COB crust < 10–15 km, margins subside to −2…−4 km.
2. **Route by plate and ring from the coast** (`erode` gets crust class and plate per cell):
   shelf to −120 m only on continental/transitional crust; slope to −2…−3 km and rise to
   −3.5…−4.5 km by ring from the coast; per-cell caps (~10 km margin wedge, ~5 km rise, 1–2 km
   fans); own plate first, cross a boundary only when full; never fill oceanic crust to shelf
   depth; fans thin with distance (~250 cells reach). Overfill as a metric.
3. **Compaction:** porosity falling with burial depth (Athy: φ = φ₀ e^(−z/λ), φ₀ ≈ 0.5–0.6,
   λ ≈ 1.5–2.5 km) instead of a fixed 25%: denser deep sediment, more accommodation, less sea
   level displacement per rock mass.
4. **Conversion of deeply buried sediment** on continental/transitional crust (wedges > ~10 km,
   or below a burial depth at which it metamorphoses) to crust (metasediment, felsic): margins
   thicken back, the sediment stock levels off. Measure after 2–3 whether it is needed.

### Targets

Sediment stock flat after ~200 Myr; sea-level offset drift ≲ 100 m per 300 Myr (primary);
first ocean ring ≲ 2–3 km of sediment, margin wedges up to 10–15 km on thinned crust;
sediment at trenches 3.5–6 Gt/yr, accretion ≲ 3 Gt/yr; erosion 14–20 Gt/yr; flooded continental
crust 25–30% including shelves at a stable sea level; mean land against an ice-free Earth
(~0.7 km? check); felsic + sediment within ±10% per 500 Myr.

### Plan v2 review (independent agent, 2026-11-17): sound with changes — adopted

Measured (`agent-scratch/review-sediment2/`): the sediment part of the sea level at 500 Myr is
+552…+703 m (everything else nets ~0…−100 m), still growing 50–90 m per 100 Myr; stock mostly on
oceanic crust (0.9–1.2e9 Gt vs 0.4–0.6e9 on continental-origin and 0.1–0.15e9 on transitional).
**Missed error: pore water.** The ocean volume is fixed and sediment displaces water with its
bulk volume incl. 25% pores; on Earth the pore water comes out of the ocean, so the water
displaced per metre of solids is (ρm−ρg)/(ρm−ρw) = 0.264, not 0.587 — the model overstates the
sediment's sea-level effect ~2.2×. Fix first: ocean volume = open water + pores of submerged
sediment. After that, compaction does not change sea level per rock mass (only accommodation).
Earth: sedimentary shell ~2–3e9 Gt (mostly lithified continental cover; Garrels & Mackenzie
1971, Ronov 1982); marine + margin sediment ~0.4–0.8e9 Gt (GlobSed, factor ~2 uncertain),
in steady state by sea-floor recycling; river load ~14 Gt/yr pre-human (Syvitski et al. 2005);
the model's 1.4–1.8e9 Gt is 2–3× and not levelled.
Required: (0b) pore-water fix + attribution metrics (sediment part of the sea level; residence
time = stock / input) — raw sea-level drift is confounded by ridge volume (thermal part varies
~340 m); (2) initial sediment cover (pelagic by age + margin wedges) or targets relative to an
equilibrium after spin-up; (3) routing is a residence-time question, not a sea-level fix
(stock = input × residence; moving sediment onto unsubducted margins grows the stock unless
margins are consumed) — add the input lever (erosion 21–24 vs 14–20 Gt/yr); (4) target flat
crystalline felsic mass (falls ~13%, continents thin, 30–35 km crust 43–49% flooded); (5) step 1
(stretching) is tectonics — move it to issue 1r/7, use integrated margin subsidence over rings
0–2 as target (a 35→<10 km taper spans 1–3 cells), check the kinematic-opening count;
(6) first ocean ring ≲ 6–8 km (continental rises hold 5–10+ km), rings ≥ 3 ≲ 1–2 km, open basin
≲ 0.5 km; (7) conversion only renames sediment as crust (−5–10% sea-level effect); restrict to
continental-origin crust (else more unsubductable margins). Optional: Athy compaction low
priority (+15–20% accommodation in a 10 km wedge; store solid thickness); routing needs only
plate id + crust class per cell in `erode`, one multi-source BFS for rings, two-phase BFS per
mouth, ~250-cell reach. Simpler order: pore water → metrics → initial sediment → erosion input
→ re-measure → minimal routing (no oceanic crust filled to shelf depth, depth targets by ring)
→ compaction/conversion only if needed.

## Full routing (2026-11-17, user: "rather do full routing right away")

After the pore-water fix and the input check (input 11.5–14 Gt/yr, Earth-like; residence ~2×
too long), the user chose the full routing of step 2 over the minimal version. Implemented in
`src/erosion.rs`:

- `erode` gets per cell the plate and a `CrustClass` (continental origin / transitional /
  oceanic, `planet.rs::crust_class`). Zones: non-oceanic crust = **shelf**; oceanic crust by
  rings within its own plate from the nearest non-oceanic crust: ring 1 or ≤ 150 km = **slope**,
  ≤ 500 km = **rise**, else **fan** (also oceanic plates without margin crust).
- Per river mouth: BFS over shelf/slope/rise ocean cells (≤ 256 cells); fill nearest first to
  the zone's target (shelf −120 m / slope −2000 m / rise −3500 m) with sediment caps (10 / 8 /
  5 km); **the plate of the land the river drains comes first**, other plates' margins only when
  it is full.
- Rest: deep-sea fan over fan-zone cells within 256 ocean cells of the mouth, in proportion to
  room below the slope depth (not the rise depth: young sea floor near ridges at −2600 m would
  otherwise overfill), cap 1.5 km × exp(−distance / 500 km).
- Then delta plains (+2 m) on shelf cells only, then overfill piled over the margin reached
  (counted as before).
- Diagnostics: deposition per zone (km³/yr) and sediment stock per crust class (Gt) in the log.
- Tests: margins fill before oceanic crust; oceanic crust never above the slope depth and below
  its cap; own plate first; fans thin with distance and stay below their cap; mass conserved.

## Plan v3: sediment sinks — compaction and conversion (2026-11-17)

User: go ahead with compaction and conversion (plan steps 3–4), with a plan review first.

### Measured (routing runs `agent-scratch/routing/`, overfill diagnostic `agent-scratch/diag-overfill/`, seeds 0/1, 300 Myr)

- Overfill 36–67k km·cells per 500 Myr (~20% of the input). **45–55% of it comes from mouths
  whose reachable margin has ≤ 3 cells and almost no fan room**: small enclosed seas and
  depressions below sea level that the old code filled to the delta top over all visited cells,
  but that the routing only fills on shelf cells. 26–44% comes from large margins (≥ 20 cells):
  shelves there are depth-bound (at −120 m, ~99%); slope and rise cells are cap-bound and
  depth-bound in about equal numbers.
- Stock at 500 Myr 1.3–1.6e9 Gt: continental-origin 0.6–0.7e9, transitional 0.12–0.18e9,
  oceanic 0.68–1.01e9 (own-plate slope/rise). Net growth ~1.5–2 Gt/yr at 300–500 Myr vs input
  11.5–14 Gt/yr; trench input 4–8 Gt/yr, of which 60–75% is accreted.

### Steps

A. **Enclosed water bodies fill** (accounting fix, not a sink): when the fan search explores
   the whole connected water body before its reach limit (an enclosed sea or depression), fill
   all its cells up to the delta top in BFS order like the old code did, instead of booking
   overfill. Expected: overfill roughly halves.

B. **Compaction (Athy):** porosity φ(z) = φ0·exp(−z/λ) with φ0 ≈ 0.6, λ ≈ 2 km (shale/sand mix;
   Sclater & Christie 1980: φ0 0.49–0.63, λ 1.4–3.7 km), grains 2700 kg/m³. Keep
   `Crust::sediment` as the rock-mass-equivalent thickness (mass / 2050, unchanged semantics for
   every mass budget) and derive the bulk thickness h(s) and pore volume from Athy in closed form
   (or a precomputed table); `dry_elevation`, the pore volume in the sea-level solve and
   `sediment_sea_level` use h(s). In `erode`, deposits change the elevation by the exact
   difference E(s + ds) − E(s) (the marginal response of a deposit is the porosity at the base,
   dh/ds = (1 − φref)/(1 − φ(h))): `Seabed` gets the per-cell sediment and computes room by
   inverting it (bisection or Newton). Effects: thick wedges denser → more room per metre of
   rise (review estimate +15–20% in a 10 km wedge), caps on rock-equivalent mass; no change of
   sea level per rock mass (pores are ocean water already).

C. **Conversion (metamorphism) of deeply buried sediment** on crust of continental origin and
   transitional crust: sediment buried below `metamorphic_depth` (bulk, ~10 km: greenschist
   facies, ~250–300 °C) becomes felsic crust at the same mass (`add_crust` with the existing
   `METASEDIMENT_DENSITY` 2750 kg/m³, felsic 1), each tectonic step; book it in the ledger and the
   material budget as a separate term. The column loses its remaining pores (it subsides a
   little: room), the sediment caps free up, the stock levels off on long-lived margins, and
   margins thicken back towards normal crust (they are 26 km at the COB now). Not on oceanic
   crust (the review: would create unsubductable margins); thick wedges there stay sediment
   until subducted or accreted. Note: conversion renames sediment as crust; its sea-level
   effect is only the expelled pores (review: −5–10%).

D. Measure after A–C (seeds 0–3, 500 Myr, `--material-budget`): overfill, stock by class and its
   trend, sediment part of the sea level and its drift per 100 Myr, converted mass (Gt/yr; Earth:
   metamorphism of margin wedges is part of the ~1–2 Gt/yr? to check), flooded, mean land,
   felsic + sediment, COB thickness, trench sediment input/accretion.

### Targets

Stock trend ≲ +0.5 Gt/yr after 300 Myr; overfill ≲ 5% of the input; sediment part of the sea
level stable after ~300 Myr; flooded 25–30%; felsic + sediment within ±10%.

### Open questions for the review

- Is a stock that levels off at ~1.3–1.5e9 Gt acceptable, given that the model's "sediment"
  includes lithified margin wedges and flooded continental cover (Earth's sedimentary shell
  2–3e9 Gt incl. continental cover; marine + margin 0.4–0.8e9)? Which Earth number is the right
  comparison for each crust class?
- Is compaction worth its complexity for +15–20% room, or should conversion at depth do the
  job alone (it removes the pores anyway)?
- Conversion depth and whether to convert under oceanic crust at all (Gulf of Mexico, Bengal:
  wedges on oceanic/transitional crust up to 15–20 km).

### Plan v3 review (independent agent, 2026-11-17): sound with changes — adopted

Measured (`agent-scratch/review-sinks/`, seeds 0/1 to 500 Myr, sediment histograms by class):
continental-origin 0.63–0.71e9 Gt, almost none above 10 km (≤ 48 cells); transitional
0.14–0.18e9 at the 8–12 km cap; oceanic 0.68–0.96e9, of which **0.15–0.43e9 in 12–15 km
columns above every cap — built by the overfill**, which is piled evenly over all margin cells
incl. slope/rise (~2.4 Gt/yr ≈ the net stock growth). Sediment part of the sea level seed 1
346 → 396 → 495 m (still rising), seed 0 ~330 m. Arithmetic: conversion at 10 km bulk never
fires below a 10 km rock-equivalent cap (Athy: cap = 8.7 km bulk); compaction room is
non-linear (Athy φ0 0.6, λ 2 km vs fixed 25%: rock-equivalent for a submerged rise of 0.9 km
−40%, 2 km ±0, 3.5 km +46%, 5 km +70%); conversion under Athy at 10 km removes only ~2% volume
(grains 2700 → 2750), under fixed porosity 0.26 m/m (artefact); transitional crust + 2–5 km
metasediment crosses `resists_subduction` and continental origin. Earth comparison: sedimentary
shell 2–3e9 Gt (~60–70% platform/orogen cover, ~20% margin wedges, ~10% deep ocean); model
continental 0.6–0.7 (fine), transitional 0.14–0.18 (low: missing stretching), oceanic 0.7–1.0
(~2× GlobSed 0.3–0.5). Target by sea level instead: sediment part ≲ 250 m and flat (Earth's
marine sediment ~110–220 m). Greenschist at 10–12 km bulk; expect ≲ 0.5 Gt/yr conversion.

Required (adopted): (1) on continental-origin crust replace the shelf cap by conversion at
`metamorphic_depth` (~11 km bulk), keep caps elsewhere; (2) convert only continental-origin
crust; (3) compaction (B) before conversion (C); (4) overfill must not pile onto slope/rise —
put it on shelf cells or leave it on land; (5) expected effect corrected (large gains on deep
slope/rise and thick shelves, less room for fresh shelf fill); (6) switch every sediment
elevation/pore use to rock-equivalent thickness with E(s) (crust `dry_elevation`,
`sediment_sea_level`, pores in the solve and its test, incision, lakes, `Seabed`, tests);
(7) book `add_crust` truncation. Known artefact: porosity as a state function decompacts
exhumed sediment and draws water into pores of submerged columns — document. Optional:
permanent cap-/depth-bound/overfill diagnostics per zone; accreted share of trench input
(60–75%, Earth mostly erosive margins) as the other residence lever.
