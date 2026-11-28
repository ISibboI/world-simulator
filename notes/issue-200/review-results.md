# Review of notes/issue-200/results.md (independent, read-only)

Checked against analyse.py re-run on runs/default-final vs baseline/default2 (8 seeds) and
runs/modern-final vs runs/base-modern (4 seeds), the per-snapshot "carbon reservoirs" lines of
every seed, the 1000-Myr cell dumps (tmp/cells.py), summary.txt files and the carbon.rs /
carbonate.rs diff. Overall: the tables are reproduced exactly by analyse.py, the closure is real
(per-step ledger, ≤1.3e-14 barren), and the headline physics (barren Neritan ocean, carbonate on
warm shelves, ~1.5 wt% carbonate in barren subducting sediment, ~9 wt% modern vs GLOSS ~7 wt%)
is plausible for an Earth-like planet. The main gaps are interpretive: what #200 does and does not
change in the arc source, the size of the one-way marble sink, and two unsupported side claims.

## Numbers (spot checks)

- All §1/§2 table values match analyse.py (pCO₂, T, φ, sources, sinks, Ω, depth/lat bins,
  fractions, reservoirs, subducted 0.089/5.83 Gt/yr = 1.52 wt%, modern 0.372/4.08 = 9.1 wt%).
  Modern pCO₂ from the logs: 726 vs 1069 ppm (matches ~730 / ~1070).
- minor: "Closure residual ≤ 1e-14": seed 4 reaches 1.3e-14 (mean of per-seed max 8.5e-15).
  Modern runs reach 3.5e-13 (still negligible) — state both.
- minor: "platforms of ~1.1 km (24% of their sediment); 45% on land, 18% at 200–3000 m, 2%
  below 3 km". My per-cell sums over the 8 dumps at 1000 Myr: shelf (−200…0 m) carbonate mean
  ~840 m (range 520–1530 m per seed), share ~18% (12–23%); location of carbonate mass: land 42%
  (15–65%), shelf 36%, 200–3000 m 19%, >3 km 3%. Close, but the 1.1 km/24% look like a subset
  of seeds or a different weighting. Say how it was computed and give the seed range (it is
  large). Also ~1% of carb_dep sits on cells with elev ≥ 0 (multi-cell samples, grid #64) —
  harmless, worth one clause.
- minor (§3 runtime): the barren comparison 1125 ± 152 vs 1158 s mixes placements (default-final:
  4 local ≈ 980 s, 4 GitHub ≈ 1280 s), so it says nothing. Modern was all-GitHub in both batches
  and is +24% (819 → 1018 s, SD ~10 s each); "the runners differ" is unsupported. Either rerun a
  local A/B for modern (ooze + 27% deep deposition may cost more) or report +24% as observed.

## Claims and interpretation

- **major: say how small the carbonate part of the arc source is.** Arcs are 6.43 Tmol/yr, of
  which only 0.44 is sediment carbonate; ~6 Tmol/yr is the altered-crust return, which is still
  the fixed-composition booking (seafloor sink 8.56 of 11.1 Tmol/yr total burial goes there, not
  into carbonate sediment). So #200 makes ~23% of the burial and ~7% of the arc source "follow
  the actual carbonate". The text ("arc/metamorphic CO₂ follow the actual carbonate") reads
  broader. Add one sentence and keep the altered-crust tracking as the bigger follow-up.
- **major: the altered-sea-floor reservoir is not just "−2 ± 130".** Per seed at 1000 Myr it
  ranges −197 (seed 1) … +161; seed 0 sits at −259 for 200 Myr (900 Myr), i.e. ~2.5× the whole
  exogenic reservoir (~100). Negative means the arcs/mantle got carbon that was never taken up —
  an unphysical source of ~0.3–0.5 Tmol/yr in those seeds, comparable to the whole carbonate arc
  term #200 introduced. Ensemble-neutral, so the means are fine, but the caveat should give the
  per-seed extremes and say it is pre-existing (the booking predates #200; only its reporting is
  new). Suggested follow-up: cap the slab carbon at the booked reservoir, or per-sample tracking.
- **major: the marble sink is as large as the new fluxes and keeps growing.** Late-run growth is
  ~0.5–0.7 Tmol/yr in every seed (crust C 33 → 627 in seed 0), vs carbonate arc 0.44 and
  metamorphic 0.38 Tmol/yr. It locks 30% of all crustal carbonate after 1 Gyr (582 of 1962) and
  would be ~2×10²¹ mol after 4 Gyr. Effects: arcs biased low (subducted metasediment carries no
  carbonate), silicate supply biased high (eroded marble counted as silicate), and the cover's
  carbonate share is depressed. Quantify at least the second (eroded metasediment mass × its
  would-be carbonate share → Tmol/yr of spurious silicate sink) or make Crust::marble a decision
  for Sebastian, not just a follow-up line.
- minor: "Ω 4.2 inside the Precambrian range 3–10 (pass)". S is solved so that precipitation
  equals the burial flux, so Ω is set by the assumed k₀ = 0.2 mol/m²/yr and n = 2, not by ocean
  chemistry (alkalinity, pCO₂ are not coupled). Call it a consistency check of k₀, not a pass.
- minor: "steady state of the cover's share ~20%": true for the share (21 → 20 → 18.6%, slightly
  falling), but the reservoirs are not in steady state: sediment carbonate grows 745 → 1380 and
  marble 0 → 582 (×10¹⁸ mol), together ≈ net mantle degassing 5981 − 4688 = 1293 over 1 Gyr
  (~1.3 Tmol/yr). Worth stating: crustal carbon accumulates ~2×10²¹ mol per Gyr, the right order
  for an Earth-like planet (Earth's crust holds ~5×10²¹ mol after 4.5 Gyr). Exogenic carbon and
  pCO₂ also drift down (0.45 → 0.30 bar), as in the baseline (pre-existing).
- minor: recycling 0.64 Gt/yr vs Earth carbonate weathering ~12 Tmol/yr ≈ 1.2 Gt/yr: fine, but
  note the model's value is erosion-driven only (no karst), so it is a lower bound.
- ok: arc decomposition (−0.43 carbonate, rest from the lower seafloor uptake: sink −0.97 Tmol/yr)
  and metamorphic halving are consistent with the code (slab_carbonate_return 0.5,
  metamorphic_decarbonation) and the logs.

## Plausibility

- Barren: no pelagic calcifiers → carbonate on shelves (75% on continental crust, 69% at
  20–200 m, 50% |lat| < 20°) and ~1.5 wt% in subducting sediment is what a Precambrian-like
  Neritan ocean should do. Saturation horizon ~3.7 km for 2 °C bottom water at S = 3.8 checks.
- Modern: 9.1 wt% vs GLOSS 7.07 wt% is good. The sediment carbonate flux 0.37 Gt/yr ≈ 3.7 Tmol/yr
  is at the high end of published Earth subduction estimates for sediment carbon; acceptable for
  an Earth-like planet but worth a clause, since half of it (1.86 Tmol/yr) goes into the arcs.
- Modern sediment carbonate first falls (≈1400 → 830 ×10¹⁸ mol by 500–600 Myr: the initial ooze
  subducts) then recovers; mention, it explains the early arc/CO₂ behaviour.

## Missing caveats / decisions for Sebastian

1. **Modern preset gets warmer**: φ is clamped at 3, so +2.3 Tmol/yr arc CO₂ goes straight into
   pCO₂ (~1070 ppm) and T (24.6 °C, already 23.6 before). Decision: raise the φ clamp, lower
   slab_carbonate_return for the modern preset, or accept. Put this in the PR's "decisions".
2. Marble sink: accept as follow-up, or add Crust::marble in this PR (see above).
3. Initial barren cover carbonate 10% vs the planet's own ~20%: change the start value?
4. Altered-crust per-seed negatives (pre-existing): follow-up issue.
