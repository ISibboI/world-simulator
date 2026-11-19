# Plan: thinner, subsiding rifted margins (current plan item 3, issues 1r/7)

_2026-11-19. User: continental sediment first → found overfill from margins without room; then
item 3. Experiments on the server tktie (see `CLAUDE.md`)._

## Problem

Continental sediment grows +0.85 Gt/yr because margins lack accommodation: overfill piles up as
coastal land on thinned crust (session 2026-11-19). On Earth rifted margins thin from ~35 km to
< 10 km over 100–400 km before breakup (necking + hyperextension; COB crust 5–10 km, β 3–5) and
then subside thermally by several km over ~100 Myr, which holds most of the sediment flux.
Model: stretching covers < half of the continental opening; COB crust ~26 km (earlier
evaluation).

## Measured (scratch worktree `agent-scratch/arc-diag/`, `rift-log-*`, RIFTDIAG lines, seeds 0–3,
500 Myr)

Gap cells whose BFS parent is continental-origin crust: stretched 3.0–3.3k; sea floor formed
(breakup) 3.8–4.5k → stretching covers ~43% of the continental opening. Breakups after
`stretch_rift` found too little crust (`given < breakup_thickness`) 1.4–1.6k; after the parent
itself had thinned to ≤ 15 km 2.2–3.0k. Parent thickness at breakup: < 15 km 8–9%, 15–20 km
45–49%, > 20 km ~45% (> 35 km 6–18%). So margins break up at ~15–20 km, and almost half of the
breakups happen next to crust that was hardly thinned (few donors within `rift_width` 200 km
≈ 2 rings).

Mechanics (`boundaries.rs` gap filling and `stretch_rift`): a gap next to continental-origin
crust thicker than `breakup_thickness` (15 km) is filled by stretching donors within
`rift_width` (continental origin and thicker than 15 km; a 1/ring taper; each donor keeps at
least 15 km), and the new cell must get ≥ 15 km; otherwise the gap becomes sea floor. Donors
must be of continental origin, i.e. crystalline buoyancy ≥ 2000 m (~13.2 km at 2800 kg/m³), so
crust can never be thinned below ~13–15 km.

## Design (to review)

1. **Breakup thickness ~8 km** (magma-poor margins thin to < 10 km before mantle exhumation;
   COB crust 5–10 km): donors are felsic (> 0.5) crust thicker than the breakup thickness —
   not required to resist subduction (stretched margins become transitional crust, which the
   model already handles: routing zone "shelf", resists subduction only with a sediment wedge).
2. **Wider rift zone**, `rift_width` 200 → ~300–400 km, so that more donors share the thinning
   and the margin tapers over 3–4 rings (Earth: 100–400 km).
3. Keep the McKenzie heating (β-dependent thermal age) so thinned margins subside thermally.
4. Measure (seeds × 1000 Myr on tktie): stretched share of the continental opening, thickness
   profile from the COB inwards (rings 0–4), COB depth and sediment there, overfill, continental
   sediment growth, sediment part of the sea level slope, flooded continental crust (Earth
   25–30%), mean land, collision length, continental balance.

Targets: stretched share ≳ 70% of the continental opening; COB crust ≲ 10–15 km; overfill and
continental sediment growth down substantially; sediment-part slope ≤ 10 m per 100 Myr after
500 Myr; flooded 25–30%.

## Plan review (independent agent, 2026-11-19): direction right, not as written

Runs on tktie (`agent-scratch/review-margin/`, switches `MARGIN_FELSIC`, `RIFT_CONSUMED`; 500 Myr):
**diagnosis correction** — of the breakups next to crust > 20 km, 1447 of 1632 (seed 0) happen
because the plate's local slot is flagged `consumed` (set at convergent contacts, never
cleared), so rifts reopening old collision/subduction edges skip stretching and make sea floor at
once; only 157 are `stretch_rift` failures.

| variant | stretched share | overfill 300–500 | slope 300–500 (m/100 Myr) | cont. stock growth (Gt/yr) | felsic (Gt/yr) |
|---|---|---|---|---|---|
| base [4] | 0.42 | 14.2k | +32 | +0.82 | −0.65 |
| s8 (breakup 8 km) [2] | 0.65 | 7.9k | +29 | +1.09 | −1.40 |
| w (rift_width 350 km) [2] | 0.51 | 10.0k | +18 | +0.77 | −0.31 |
| f8w (the plan) [4] | 0.62 | 5.4k | +31 | +1.10 | −1.56 |
| c (consumed slots may stretch) [3] | 0.56 | 9.6k | +22 | +1.06 | −0.31 |
| cf8w [3] | 0.76 | 4.9k | +35 | +1.09 | −2.94 |

cf8w: thinned felsic crust 6–7% of the surface (Earth's hyperextended margins ~2–4%), flooded
36–44%, subducted transitional up to −4.9 Gt/yr, collision stacking 18.5 → 9.5 Gt/yr,
initiations −20%. Seed noise ±15 m per 100 Myr in the slope. Required: (1) let stretching fill
consumed empty slots (mass-conserving; possibly not next to converging contacts) — biggest lever;
(2) one riftable predicate for parent and donors (felsic > 0.5, not oceanic); (3) update every
margin predicate using `is_continental_origin` (initiation, invasion, seeding, plume, statistics);
(4) stop thin margins from subducting wholesale (Earth continental subduction ~0.4 km³/yr); (5)
keep `rift_width` ~200–300 km; (6) overfill fell ~65% but continental stock growth did not:
continental-class shelf cells have no cap and old margins fill regardless — the slope target
probably needs progradation onto oceanic crust or removal of continental sediment too. Earth:
Péron-Pinvidic et al. 2013 (necking 30 → 10 km over 10–60 km, hyperextended < 10 km over
50–200 km, margins 100–400 km per side), McKenzie τ ≈ 63 Myr, Steckler & Watts; volcanic margins
(15–30 km mafic) missing in the model.

Decision: implement (1) first (cheap, physical, measured: stretched share 0.42 → 0.56, slope
+32 → +22, felsic −0.65 → −0.31); defer (2)–(4) (they cost felsic crust and do not reduce the
continental sediment growth); report (6) to the user.

## Result of (1), consumed slots may stretch (6 seeds × 1000 Myr on tktie, `agent-scratch/consumed/`)

vs the invasion runs, seeds 0–5, means 500–1000 Myr: stretched/kinematic continental opening
0.34 → 0.52; overfill 1.63 → 1.12 Gt/yr; continental sediment growth +0.86 → +0.72 Gt/yr
(transitional +0.08 → +0.07, oceanic −0.02 → +0.17); flooded continental crust 24 → 27%; mean land
559 → 448 m; collision length 14k → 15k km; felsic + sediment +0.02 → +0.37 Gt/yr; ocean ≥ 200
Myr 4.4 → 4.7%; sediment-part slope 500–1000 Myr +18 ± 5 → +17 ± 2 m per 100 Myr (700–1000:
+13 ± 3). Test: a rift reopening an old suture (consumed empty slots) stretches (fails without
the fix). Remaining continental growth → research `research/shelf-sediment-removal.md`.
