# Plan: sediment at trenches — subduct most of it (issue 3 / sediment follow-up)

_2026-11-17. User: push, then look at the trench side (after routing, compaction and
metamorphism left the sediment part of the sea level rising 25–50 m per 100 Myr)._

## Problem

At a subducting sample, `boundaries.rs` scrapes **all sediment above
`subduction_channel_thickness` (1 km)** off into an accretionary wedge (metasediment crust on
the overriding plate) and subducts only the channel. Sediment reaching trenches is mostly thick
(passive-margin slope/rise wedges of 3–8 km), so 60–75% of the trench input is accreted
(runs `agent-scratch/sinks2/`: subducted sediment 4–9 Gt/yr, accreted 2.5–6.9). Net removal to
the mantle is ~1.5–2.5 Gt/yr against an input of 11.5–14 Gt/yr; the rest stays on the surface
(as sediment or accreted crust), and the marine stock and its sea-level part keep growing.

Earth: frontal accretion takes only the part above the décollement; even at accretionary
margins ~70% of the incoming sediment column is subducted below the forearc (Clift & Vannucchi
2004; von Huene & Scholl 1991); 57% of margins are erosive (thin cover, everything subducts plus
forearc erosion). Globally ≲ 20–30% of trench sediment is accreted; sediment subduction is
~1–1.6 km³/yr (Scholl & von Huene 2007), comparable to arc production.

## Design

1. **Partial frontal accretion:** of the sediment above the channel, only the fraction
   `frontal_accretion_fraction` (default 0.3) is accreted; the channel and the rest subduct.
   Thin cover (≤ channel) subducts entirely as now. Book the parts as now (SubductedSediment for
   all, Accretion for the wedge addition).
2. Keep subduction erosion as is.
3. Diagnostic: accreted share of the trench input (Gt/yr) in the log; trench input split by
   the arriving sediment thickness (≤ 1 km, 1–3, > 3 km) to see where the input comes from.

## Expected effect and checks

Accreted share 60–75% → ~20–30%; net sediment subduction ~1.5–2.5 → ~4–6 Gt/yr; marine stock
and the sediment part of the sea level level off (target ≲ 250 m and flat after ~300 Myr).
Watch: felsic + sediment (more continental mass goes to the mantle: arcs 7–8 Gt/yr must keep
the felsic mass roughly flat, ±10%), forearc growth (accreted crust), continental fraction,
mean land, flooded, collision length.

Tests: a subducting sample with thick sediment accretes the given fraction of the excess and
subducts the rest; mass booked consistently; thin cover subducts entirely.

Second lever (later, measure first): passive-margin lifetime — 45–56% of trench sediment came
from own-plate passive-margin crust (review of plan v1).

## Plan review (independent agent, 2026-11-17): sound with changes — adopted

Measured (`agent-scratch/review-trench/`, seeds 0/1 to ~276 Myr): trench sediment mass by
arriving column ≤ 1 km 13–15%, 1–3 km 12–14%, 3–5 km 15–18%, 5–8 km 36–40%, > 8 km 14–22% (columns
> 3 km: ~5% of samples, 73% of the mass; 83% of samples ≤ 250 m pelagic); mass above the
channel 66–69% (accreted now), with f = 0.3 → ~20%; 15–18% on samples with felsic > 0.2 (thinned
continental margins), 8–10% on transitional crust; 61–63% under continental upper plates. Logs
150–300 Myr: stock +2.3–4.4 Gt/yr, accretion 2.6–6.7 Gt/yr, so f = 0.3 subducts +1.8–4.7 Gt/yr
(≈ the growth); felsic + sediment +0.6…+2.1 Gt/yr now, expected −0.5…−3 Gt/yr after. Earth: arcs
7–8 Gt/yr and subduction erosion ~3.7 Gt/yr are matched; sediment subduction ~3–4.5 Gt/yr. A
fixed f matches the global accreted share by calibration, not mechanism (thick-fill margins
accrete half or more frontally; the ~70% includes underplating and later erosion).
Required (adopted): (1) felsic + sediment criterion as a rate, |net| ≲ 1 Gt/yr after 150 Myr;
if it fails, look at the felsic loss in "subducted transitional" first (not arcs); (2) f is a
global calibration — re-measure the passive-margin share (columns > 5 km, felsic > 0.2) in the
same runs and keep margin lifetime in this issue's evaluation; (3) sea-level target as a trend:
sediment part changes < 10–15 m per 100 Myr after ~300 Myr. Optional (not now): erosive vs
accretionary by convergence < 7.5 cm/yr and fill > 1 km with a décollement at a fraction of
the column; underplating.

## Result (seeds 0–3, 500 Myr, `agent-scratch/trench/`)

Accreted share of trench sediment 17–22% (was 60–75%). Trench input 3–10 Gt/yr (Earth ~3–4.5),
20–30% of it on thinned continental margins (felsic > 0.2) and 50–65% in columns > 5 km by
400–500 Myr. Sediment part of the sea level 300 → 500 Myr: +195…+289 → +253…+335 m (before:
+288…+332 → +314…+383); seed 2 flat at ~200–250 m, the others still +30…+40 m per 100 Myr.
Felsic + sediment −9…−12% per 500 Myr, net −2…−5 Gt/yr after 150 Myr: **fails** the rate
criterion (|net| ≲ 1). Flooded 11–17%, mean land 0.62–0.75 km, collision length 9–13k km,
ocean age 58–76 Myr.

Diagnosis: too much sediment and felsic crust reaches trenches at all — thick wedges on thinned
continental margins subduct, because `Crust::resists_subduction` uses the crystalline buoyancy
only. On Earth a passive margin with a thick sediment wedge arriving at a trench jams it
(Banda/Timor, Taiwan) and its sediment ends up in the orogen. Next candidate: include the
sediment's buoyancy in the subduction resistance (sediment then goes to collision stacking as
metasediment, a real sink from the marine stock into continental crust).
