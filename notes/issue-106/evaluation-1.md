# Independent evaluation: volcanism phases 0–2 (#106), run volc2 vs main

Data: 8 seeds × 1000 Myr, barren preset, branch `volc2` against `main` (same arguments). My own
scripts are in `~/tasks/issue-106/tmp/eval/` (logs.py, cells.py, pairs.py, hist.py, land.py).
The yardstick is an Earth-like planet, not Earth, and errors from the ~110 km grid are accepted (#64).

## Numbers (8-seed means; main → branch where both exist)
- **LIP rate:** 525 LIPs, one per 15.2 Myr (per seed 13.5–19.2). ≥10⁶ km³: one per 16.9 Myr
  (Earth ~20–30). ≥10⁷ km³: one per 59 Myr (Earth's plateau-class events: roughly one per 50–75 Myr). Steady over
  time (43–62 per 100 Myr per 8 seeds), no start-up burst.
- **Volumes:** all: median 4.7e6, p10 1.0e6, p90 2.0e7, max 1.0e8 (one hit the cap; none at the 10⁵ floor).
  Oceanic (396): median 6.8e6, p90 2.4e7, 9 events ≥5e7. Continental (129): median 1.4e6,
  p90 3.7e6, max 9.2e6. These ranges are good: flood basalts are Deccan/Siberia-sized and plateaus are Kerguelen/OJP-sized.
  The deviation: the model has almost no 10⁵–10⁶ events (3 below 3e5), while Earth's catalogue is dominated by them.
  So the overall median is ~0.5 decade above Earth's ~10⁶ km³. Accepted for a first step, since these events
  matter little for relief and carbon.
- **Fluxes (km³/yr):** LIPs 0.35–0.95 (mean ~0.59), tails 0.32–0.45, ridges ~39, arcs ~2.8. The
  intraplate share is ~2%, within Earth's range (tails ≈ 0.3–0.4 km³/yr oceanic intraplate). Mass: LIPs
  +1.73 Gt/yr, tails +1.06 Gt/yr. 95% of LIP volume goes into ocean.
- **Ledger:** the logs carry no explicit residual. My check: the change of the total crust mass between snapshots
  minus Σ(budget terms)×100 Myr is 3.9e6 Gt per 100 Myr on average. Main gives 3.1e6, so both sit at the
  rounding level of the 0.01 Gt/yr log figures. The budget closes. Total crust at 1000 Myr is 2.35e10 Gt
  against main's 2.17e10 (+8%), while the felsic mass is unchanged (1.422e10 vs 1.426e10): no felsic leak.
- **Classes and continents:** continental area 37.5% → 35.7% and land 29.3% → 28.0%, both inside the seed spread
  (seed 2: land 18.7% on 36% continent). Transitional area 3.1% → 5.5%: these are the plateaus. Mean felsic
  fraction of continental-class cells 0.936 → 0.905 (−3 pp of dilution by accreted or stacked mafic
  crust); mean continental thickness 27.0 → 29.1 km. The sea-level budget total is +69 m against +4 m
  (plateaus displace water; plausible, Cretaceous-like).
- **Ocean relief:** median ocean depth −3730 → −3569 m; share at 2–4 km depth 39% → 42%, deep ocean (≤ −5 km)
  unchanged (17%); oceanic crust >10 km covers 1.2% → 2.3% of the surface. Plateaus at −2 to −1 km
  and a few emergent ones (seed 3, seed 7) show clearly on the maps. Hotspot tracks show only as faint shallow
  streaks or spots (seed 7). That is expected at 110 km cells: a cell mean of +0.2–0.5 km (#64).
- **Land hypsometry:** land p50 224 → 410 m and p90 1077 → 1766 m, but this comes from felsic orogens
  in seeds 4 and 6 (land above 2 km is 98–100% felsic and orogenic in both runs). Land with felsic <0.5 doubles
  (0.56% → 1.15% of the surface), median elevation 40–600 m. I see no systematic hypsometry effect of volcanism.
- **Carbon/climate (t=1000):** pCO₂ 0.355 → 0.319 bar, T 17.9 → 17.7 °C, both at seed-noise level.
  Mean of the last 500 Myr: plume source 0.99 → 1.37 Tmol/yr (+0.38, the LIP term), land
  weathering 3.1 → 4.5 Tmol/yr. More mafic land and plateaus weather faster, which offsets the LIP CO₂.
  The ridge and arc source differences (+0.4, +1.0) are not tied to the change. Per LIP: 10⁶ km³ →
  2e17 mol CO₂, the Deccan-order estimate. Plausible.
- **Runtime:** 933 → 948 s (+1.6%, within GitHub-runner noise). The emplacement loop over all cells is cheap.

## Thick mafic columns: an artefact, not plausible
Mafic columns (felsic <0.3) thicker than 20 km cover 0.9% of the surface and thicker than 40 km 0.1%. In main the
maximum is 21 km; on the branch it is 47–75 km in every seed (75 = cap, seed 6). They are all class "transitional",
mostly on land at +0.2 to +3.7 km. They persist for hundreds of Myr: seed 7 holds 64 km columns from
600 to 900 Myr, with 70–125 cells >40 km. Two causes:
1. **Stacking of co-located LIPs:** 23 LIP pairs fall within 10 Myr and 1000 km of each other, against ~2 expected for
   random positions (the mantle clusters plumes). Seed 3: 2.7e7 + 8.9e7 km³ at 983/984 Myr, 450 km
   apart, give 54 km of felsic-0 crust standing 2.2 km above sea level. The design limit is
   `lip_plateau_thickness` 25 km per event, but nothing caps the total.
2. **Accretion and collision stacking of plateaus:** they are buoyant, so collision handles them like continents.
   Collision stacked rises from 16.4 to 24.8 Gt/yr and piles mafic crust up to 75 km.
   Earth's reference points: OJP/Kerguelen crust is 30–40 km and Iceland's ~25–40 km. Accreted plateaus
   (Wrangellia, Caribbean) are sliced thin, and mafic lower crust thicker than ~40–50 km turns to eclogite and delaminates.
   A 60–75 km basaltic highland at +3.5 km has no Earth-like counterpart, and the coarse grid does not cause it.

## Code (read-only review of volcanism.rs)
The code is clean and matches the plan:
- The ChaCha8 stream separate from the seed is good.
- Exact volume normalisation over distinct samples is tested.
- The cap only fills to `max − thickness`, so felsic crust is kept (tested).
- The ledger records the actual mass change.
- A minor point: the lid for the LIP is judged at the cell above the plume, before the plume's own crust; this is fine.

## Verdict
A sound first step and mergeable after one fix. The LIP frequency, the continental and oceanic volume
classes, the tail flux, the CO₂ booking and the ledger closure are all physically plausible, and the cost is negligible.
- **Must (before merge):** limit the pile-up of plume crust. Suggestion: cap plume additions at a total
  column of ~40 km of mafic crust (e.g. fill only to `min(max_crust_thickness, ~40 km)` when
  the column is mostly mafic, or use felsic as a weight). Then re-check max mafic thickness and emergent plateau height
  on 2–3 seeds. Alternatively, merge as is and state the 47–75 km columns clearly as a known artefact with a follow-up issue.
  That is Sebastian's decision.
- **Should (follow-up issues):** (a) eclogitisation or delamination of mafic crust thicker than ~45 km, also where it was stacked in collisions or
  accreted; (b) the many small events (10⁵–10⁶) are missing, so widen the low tail or add a
  small-event class if the activity view (#110) needs them; (c) say in the PR that land-hypsometry and
  arc/ridge CO₂ differences are seed noise, while the land weathering gain is real and offsets the LIP CO₂;
  (d) report the plume clustering (10× random) as a property of the mantle model.
