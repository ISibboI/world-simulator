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
