# Plan: ancient ocean (issue 13)

_2026-11-19. User: keep issues 10 and 13 separate; do 13 next._

## Measured (scratch worktree `agent-scratch/arc-diag/`, current model, seeds 0–3)

- Ocean ≥ 200 Myr: 2100–2900 cells (5–7% of the surface) at 1000 Myr; holds 63% of the oceanic
  sediment (~10–14 km per cell) and takes all of the oceanic sediment growth (`sed-log-*`).
- Where it sits at 1000 Myr (`anc-log-*`, ANCIENT lines): in own-plate ocean regions (connected
  cells of one plate that do not resist subduction) < 1% of the surface: **48–64%**; 1–5%:
  27–50%; > 5%: 0–12%. > 10 rings (> 1100 km) from any plate boundary: 38–61%; 0–2 rings: 6–13%.
  Adjacent to continental-origin crust of the same plate: 46–62%. Mostly on plates > 20% of
  the surface (3 of 4 seeds).
- Subduction initiation at passive margins requires the own-plate ocean region to be at least
  `min_plate_area` (1%); regions < 1% can never initiate. Hazard: waiting time 1000 Myr per
  1000 km of ≥ 180 Myr margin (seed 0: 100–211k km of old passive margin, 0.07–0.16 expected
  initiations/Myr; 21 successful + 14 failed in 1000 Myr). Boundary initiation needs an existing
  boundary.
- Creation of small enclosed ocean regions (< 1%), gross changes in cells per phase over 600 Myr
  (`bas-log-*`, BASINS lines; thousands): plate motion (substeps) +41…+50 / −24…−34; rifts
  +2.1…+4.4; initiations +0.9…+2.0; separation of detached pieces −9…−13; sutures −2.2…−4.9;
  captures −1.5…−3.6. Net growth mainly from plate motion (continents converging and cutting a
  plate's ocean into pieces? crust that resists subduction — arcs > ~14.6 km, wedged margins —
  dividing ocean regions?), not yet resolved.
- Earth: no ocean floor > ~180 Myr (Pigafetta Basin, oldest in situ; the Ionian/East
  Mediterranean ~230–270 Myr is a trapped remnant, small); trapped basins with oceanic or
  transitional crust and 15–20 km of sediment exist (South Caspian, Black Sea, Gulf of Mexico)
  but cover ~1–2 Mkm² in total vs the model's ~17–21 Mkm² in regions < 1%.

## Candidate mechanisms (for the review to judge)

A. **Closure of trapped basins:** old oceanic crust in small enclosed own-plate regions is
   consumed when the plate is under convergence across it (South Caspian subducting below the
   Apsheron–Balkhan sill; Black Sea below the Caucasus): e.g. initiation allowed in regions
   below `min_plate_area`, the basin becoming the lower side of a new boundary inside the plate
   (needs a split that the area rule currently forbids), or a dedicated "basin closure" event
   that thrusts the basin's crust and sediment onto/under the surrounding continental crust
   (stacking, mass-conserving) once the basin is older than ~180 Myr.
B. **Prevent trapping:** find what in plate motion cuts own-plate oceans into small pieces (and
   rifts that leave pockets) and fix it at the source.
C. **Interior ancient ocean far from boundaries** (the 1–5% regions on large plates): intraplate
   initiation (age hazard in plate interiors, Stern & Gerya 2018 rate is low) or a higher hazard
   at old, sediment-loaded passive margins (sediment loading promotes initiation: e.g. Regenauer-
   Lieb et al. 2001; Nikolaeva et al. 2010).

## Step 0 (before the design)

Resolve how small regions form during plate motion (which process converts a region of ≥ 1% into
pieces: convergence/consumption, resampling noise at ~1 cell, growth of resisting crust) and how
old their crust is when trapped; decide A vs B (vs both) and whether C is needed.

## Targets

Ocean ≥ 200 Myr ≲ 1% of the ocean area (Earth ~0); trapped basins (< 1% regions) ≲ 1–2 Mkm²;
then the sediment part of the sea level levels off within 1000 Myr (8 seeds); no regressions in
plate count, ocean age (mean 50–70 Myr), collision length, continental balance.

## Review and Step 0 (independent agent, 2026-11-19): keep the plan, change the premise — adopted

Findings (`agent-scratch/arc-diag/`, region tracking by (plate, local vertex); `trap-log-*`,
`encl-log-*`, scripts `trap.py`, `areg.py`, `encl.py`; seeds 0–3 × 1000 Myr):
1. **Enclosed specks** (own-plate ocean regions < 1% with no plate-boundary contact) hold 29–52%
   of the ancient cells and 46–79% of the ancient-ocean sediment: 200–279 per seed at 1000 Myr,
   median 4–6 cells, 1100–1550 cells in total (14–19 Mkm²; Earth ~1–2), **18–25 km rock-equivalent
   sediment** on 7.6 km oceanic crust (enclosed-sea fill); perimeter ~90% continental-origin
   crust of the same plate; their number only grows (seed 0: 1, 6, 27, 50, 135, 179, 269 at 50 …
   1000 Myr), 52–74% older than 400 Myr at 1000 Myr (max 1165 Myr: initial ocean never recycled).
   Metamorphism is continental-origin only, so their sediment can never leave.
2. Enclosure routes: plate motion closing continental/arc crust around open ocean (~600–1100 cells
   of regions ≥ 10 cells per 300 Myr, age 100–200 Myr); rift and initiation splits leave pockets
   (~100–220 cells, 65–195 Myr); sutures/terranes leave remnant oceans (~50–200 cells); single-
   cell resampling flicker is frequent but mostly reopens. Net ~27 specks per 100 Myr.
3. Regions dropping below 1% in larger plates: plate motion pinching off 100–400-cell open pieces
   at 40–85 Myr, and ~400-cell plates at the 1% threshold; they hold almost no ancient crust.
4. Ancient ocean in open pockets and 1–5% regions (35–65% of the ancient cells, 21–54% of its
   sediment, 5–9 km per cell): next to the plate's own continent, often with a short own trench
   within 10 rings that consumes slowly; ages up to 300–950 Myr: an **initiation-rate failure**
   (seed 0 over 1000 Myr: 21 passive-margin initiations, 14 failed, 2 induced, 10 reversals,
   ~0.03/Myr). The "old passive margins 211k km" statistic is inflated by speck perimeters.
5. Regions > 5% hold almost no ancient ocean; the "> 10 rings from a boundary" figure came from the
   enclosed specks.
Physics: Earth's oldest in-situ sea floor ~180 Myr; trapped remnants (Ionian, Herodotus 230–340
Myr) are tiny; trapped basins end by trench rollback into them / far-field underthrusting (South
Caspian, Black Sea) or by filling and burial into continental lithosphere (Precaspian ~20 km
sediment, Gulf of Mexico); at 10–20 km burial the basin base metamorphoses whatever the basement
— the continental-origin restriction has no physical basis. Initiation is dominated by induced
modes at weak zones (transference, reversal, transform/fracture-zone nucleation, lateral
invasion: Scotia, Lesser Antilles, Gibraltar; Stern & Gerya 2018, Crameri et al. 2020; ≳ 0.1 new
zones per Myr); no unambiguous Cenozoic passive-margin collapse; sediment loading helps only
modestly (Nikolaeva et al. 2010).
Design (adopted, in order): **D1** burial metamorphism on any crust (mass-conserving, capped,
logged separately); check that oceanic crust + metasediment still subducts; deep fans > 11 km
convert too (re-check trench accretion). **D2** no closure event now; re-measure after D1.
**C'** induced initiation for open old ocean: (a) old regions < 1% touching a boundary initiate
at that boundary (transference/reversal) instead of needing a split; (b) subduction invasion:
higher passive-margin hazard within ~1000–2000 km along strike of an existing trench tip in the
same ocean region; (c) investigate the 40% failed initiations and the near-absent induced ones
(issue 1l) before changing waiting times. B-lite: no deletion of 1–2-cell specks.
Targets (8 seeds × 1000 Myr): enclosed oceanic regions ≤ 2 Mkm² (~160 cells), none > 400 Myr;
ocean ≥ 200 Myr ≤ 1% of the ocean area, ≥ 250 Myr ~0; initiations of all kinds ≥ 0.05–0.1/Myr,
induced ≥ 50%; basin metamorphism in the budget; sediment part of the sea level ≤ 10 m per 100
Myr after 500 Myr; no regressions (ocean age 50–70 Myr, plates, collision length, felsic +
sediment 0 to −1.5 Gt/yr).

## D1 result (8 seeds × 1000 Myr, `agent-scratch/basin/` vs `agent-scratch/trend/`)

Burial metamorphism on any crust below 11 km bulk. Means of 8 seeds (baseline → D1): ocean
≥ 200 Myr at 1000 Myr 10.8% → 6.2% of the ocean (largely by reclassification: filled specks
thicken past 12 km and are no longer oceanic); oceanic sediment stock 1.30 → 0.61e9 Gt (Earth
0.3–0.5), continental-origin 0.77 → 1.12, transitional 0.38 → 0.20; metamorphosed by 1000 Myr
1.1e9 Gt, 0.8e9 of it on non-continental crust; felsic + sediment 500–1000 Myr −0.90 → −0.31
Gt/yr; sediment part of the sea level at 1000 Myr 493 ± 10 → 420 ± 12 m, slope 500–1000 Myr
+26 ± 2 → **+19 ± 3 m per 100 Myr** (target ≤ 10); mean ocean age (500–1000 Myr) 79 → 68 Myr;
collision length 11k → 13k km. Next: C' (induced initiation for open old ocean), starting with
why 40% of initiations fail and induced initiations are rare.

## C' step 0 and subduction invasion (8 seeds × 1000 Myr, `agent-scratch/invasion/`)

Initiation diagnostic (scratch, seeds 0–3 × 1000 Myr): 33–42 margin attempts, 23–29 succeed;
all failures from the area test (remainder 6–12, piece 2–7), none from the plate cap or missing
pieces. Old oceanic boundary cell-steps: 64% already trench/ridge/collision, 20% diverging,
5–6% facing neither continental nor younger crust, 10–11% candidates; 5–18 boundary initiations
per 1000 Myr. All kinds ~0.04/Myr: a rate problem, not failures.
Implemented **subduction invasion**: passive-margin candidates within `invasion_reach` 1500 km
(rings within the same plate) of a trench where their plate subducts use
`invasion_waiting_time` 100 Myr per 1000 km instead of 1000 (Atlantic: Scotia, Lesser Antilles,
Gibraltar within ~50 Myr along a few thousand km of margin near trench tips). Test: only margin
cells near the trench get the invasion hazard.
Result (D1 → D1 + invasion, 8-seed means): ocean ≥ 200 Myr at 1000 Myr 6.2 → 4.5% (target ≤ 1%);
initiations of all kinds 0.036 → 0.056 per Myr (target ≥ 0.05); mean ocean age (500–1000 Myr)
68 → 54 Myr; plates 13.9 → 17.4; collision length 13k → 14k km; felsic + sediment −0.31 →
+0.06 Gt/yr; oceanic sediment stock 0.61 → 0.50e9 Gt; sediment part of the sea level slope
500–1000 Myr +19 → +18 ± 4 m per 100 Myr (target ≤ 10). Stock growth 500–1000 Myr is now on
continental-origin crust (+0.85 Gt/yr), oceanic +0.02, transitional +0.08: the remaining
sea-level rise is continental sediment (flooded shelves, platform cover, filled basins now
counted as continental) — the separate sediment → crust item, not issue 13. Ancient ocean 4.5%
still above target: re-measure where it sits.
