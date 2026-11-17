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
