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

## Plan v2: keep passive margins out of the mantle (2026-11-17, user: ok)

Problem (result above): felsic + sediment −2…−5 Gt/yr; trench input 3–10 Gt/yr (Earth
~3–4.5), 20–30% on thinned continental margins (felsic > 0.2), 50–65% in columns > 5 km.
`Crust::resists_subduction` uses the crystalline buoyancy only (≥ 2000 m dry; continental
crust ≳ 13–15 km), so a stretched margin (10–13 km crust) with a 5–10 km wedge subducts.

Options:

A. **Buoyancy of what goes down:** crystalline buoyancy + the dry elevation of the subducted
   part of the sediment (channel + (1 − f) × excess). Estimate: old oceanic crust (850 m) with
   8 km sediment subducts 5.9 km of it, which adds ~1.6 km → 2.45 km > 2000: sediment-rich
   oceanic subduction (Makran, 7 km fill) would jam — unrealistic, because the slab's negative
   buoyancy comes from the old mantle lithosphere, which the criterion does not see.
B. **Sediment counts on continental-origin lithosphere only:** for crust with felsic > 0.5 (or
   > 0.2), resistance = crystalline buoyancy + sediment dry elevation; oceanic crust keeps the
   crystalline criterion. Rationale: thinned continental lithosphere has no dense old oceanic
   mantle lithosphere below it, so a thick wedge on it makes the column buoyant (Banda/Timor,
   Taiwan: the margin jams the trench and is stacked into the orogen). Effect: wedges on
   thinned continental crust go to collision stacking (sediment → metasediment at the same
   mass, staying on the surface) instead of the mantle.
C. **Fewer passive margins at trenches:** measure first what brings them there — subduction
   initiation at passive margins (the trench is placed along the coast, so the margin's own
   slope/rise is the first thing subducted) vs closure of an ocean basin (the margin arrives
   at the end, a real collision). If initiations dominate, place the new trench on the
   oceanic side beyond the rise (Earth: initiation at old oceanic fracture zones / transform
   margins, rarely at the COB itself).

Step 0 (measure, cheap scratch instrumentation): share of trench sediment and of subducted
felsic mass (a) within N Myr after an initiation on that pair/segment vs later, (b) on own-plate
margin crust of the subducting plate; where the felsic loss comes from (subducted
transitional, subduction erosion, sediment).

Then implement B (and C if initiations dominate). Criteria as before (felsic + sediment |net|
≲ 1 Gt/yr after 150 Myr; sediment part of the sea level < 10–15 m per 100 Myr after 300 Myr;
trench input toward 3–5 Gt/yr), plus collision length and flooded continental crust.

### Plan v2 review (independent agent, 2026-11-17): B narrow — adopted; A rejected; C not needed

Step 0 (`agent-scratch/review-margins/`, seeds 0/1, 151–277 Myr, Gt/yr): trench sediment 4.8 /
6.2, of which on felsic > 0.2 crust 0.83 / 1.22 (17–20%), on transitional crust 0.44 / 0.68 —
~80% arrives on oceanic crust (continental rise and fans), which B does not touch. Within 30 Myr
after an initiation or reversal on the pair: sediment 2% / 21%, subducted felsic 1% / 14–18% —
initiations do not dominate (C not justified now; if wanted later, lower the passive-margin
hazard). **Felsic loss breakdown:** felsic inside subducted crust classed oceanic 3.0 / 3.3,
subducted transitional 1.2 / 1.5, subduction erosion of crust 1.1 / 1.0 and of sediment
0.95 / 0.9; the felsic in oceanic-class crust comes from arcs built on oceanic-class upper
plates (3.7–4.4 of the ~7.3 Gt/yr arc output, 54–60%, booked felsic = 1) — intra-oceanic arcs
recycled. B (felsic > 0.5, not oceanic) would redirect only ~0.6–0.8 Gt/yr; the felsic
criterion cannot be met through passive margins. A rejected (Makran, Cascadia, Bengal subduct;
column buoyancy at the trench is not the criterion). B accepted in narrow form: sediment counted
at grain density, only on felsic > 0.5 non-oceanic crust; `is_continental_origin` stays on the
crystalline crust (it drives routing, metamorphism, rifting, statistics); the column predicate is
used consistently at consumption, polarity, dynamics, splits and terranes. Watch flicker of the
classification, collision length, sutures, flooded. Recommendations: arc crust on oceanic-class
upper plates should carry an intermediate felsic fraction (0.3–0.5), or define the criterion as
felsic in continental-origin crust + sediment; re-derive the |net| target from Earth's fluxes
(Scholl & von Huene 2007; the model's trench length 73–105k km is 1.5–2× Earth's); next trench
lever: accretionary vs erosive margins with a décollement (most input is thick fill on oceanic
crust, mostly accreted on Earth).

Implemented B as `Crust::resists_subduction` (the trench predicate at all its call sites) with a
crystalline-only `crust_resists_subduction` behind `is_continental_origin`. Tests: predicate
(wedged margin resists, its crust is not continental origin, 8 km fill on oceanic crust still
subducts); a wedged margin arriving at a trench is not subducted (the ocean subducts below it, or
it is stacked onto a continent with its sediment).

### Result of B (seeds 0–3, 500 Myr, `agent-scratch/margins/`)

Felsic + sediment mean net after 150 Myr −0.7…−2.2 Gt/yr (before −1.8…−3.5), over 500 Myr
−6…−9.5% (before −9…−12%) — the ~1 Gt/yr the review predicted; criterion still missed in 3 of 4
seeds (arc bookkeeping). Trench sediment 2.4–6.9 Gt/yr, 18–22% accreted, 13–18% on continental
margins at 500 Myr (was 13–30%). Sediment part of the sea level 300 → 500 Myr +228…+274 →
+310…+351 m: still rising ~40 m per 100 Myr (not improved; seed spread is large). Collision
length 8–12k km, flooded 14–27%, mean land 0.50–0.68 km, ocean age 59–76 Myr.
