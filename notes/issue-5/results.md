# Results: relief — mountain heights (#5) and erosion inconsistencies (#7)

_2026-11-20. Plan: [`plan.md`](plan.md), plan review: [`review-plan.md`](review-plan.md).
8 seeds × 1000 Myr, `main` (4937b86) vs this branch (`final`), cell dumps every 100 Myr.
Means over 500–1000 Myr unless noted; Welch t with n = 8 (|t| ≳ 2.4 ≈ p < 0.05).
Attribution runs on seeds 0–3: `noiso` (`erosion.isostatic_incision=0`), `norunoff`
(`erosion.runoff_discharge=0`), and `branch` (all 8 seeds, without the merge-cap fix).
Scripts: `relief.py`, `evaluate.py`, `peaks.py` in the agent's task directory._

## What changed

1. **Isostasy in the implicit stream-power solve** (`erosion::incise`). The removed thickness
   solves `h = F (z(h) − z_r)`, where `z(h)` is the surface after the Airy rebound. For bedrock,
   `h = F (z − z_r)/(1 + F f)` with `f ≈ 0.15` (crust) or `≈ 0.10` (densified lower crust
   > 50 km; piecewise, in closed form). Sediment with compaction is solved by bisection.
   The surface never drops below base level.
2. **Rivers carry runoff.** Runoff is `P − ET` from the climate's own Fu–Budyko curve, with the
   current annual P and potential evaporation (`WaterCycle::runoff`, `potential_evaporation`).
3. **Lakes and endorheic basins** (`Lakes`, `lake_balance`, `fill_lake`). Every depression of
   the priority flood deeper than 1 m is a lake.
   - It rises until its open-water evaporation (PET − ET over the covered cells, from the
     lowest floor up) balances its inflow. If it reaches its pass first, it spills the rest.
   - Endorheic basins keep their water and sediment.
   - Lakes trap sediment by levelling their floor, up to the spill level after incision.
   - Lake and basin floors are not incised.
   - Rivers cannot cut below the lake surface they flow into.
4. **#5 — merging plates respects the 75 km crustal cap** (`topology::spread_crust`).
   - Sutures and captures put samples that find no free vertex onto their neighbours. This
     stacking had no cap.
   - Now crust beyond the cap spills outward ring by ring, as in collision stacking. Mass is
     conserved.
5. Diagnostics: a per-snapshot `hydrology:` log line; switches `erosion.runoff_discharge` and
   `erosion.isostatic_incision` (default on, for attribution).

## #5: where the 8–10 km peaks came from

On `main`, 4 of 8 seeds have snapshots above 6.5 km (max 10.3 km). All three spikes at 400 Myr
(seeds 2, 3, 7) and seed 0's 6.7 km cell are continental crust (cls 0) with no sediment, no
thermal uplift and |dynamic topography| < 400 m. Their height is the isostatic height of the
crust, except seed 0's cell, which is 54 km crust carrying 12.9 km of sediment. The 400 Myr
cells are **95–120 km thick**, above the 75 km cap. Those
snapshots coincide with supercontinent assembly ("largest land mass 98%", many sutures). The
only path that thickens crust without the cap is `spread_crust` in plate merges. With the cap
enforced (`final`), no snapshot of any seed has crust thicker than 75 km. The highest cells
are 75 km crust plus ≤ 350 m dynamic topography.

| | main | final | t |
|---|---|---|---|
| highest cell over all snapshots 0–1000 Myr, mean of seeds (m) | 7169 (5275…10334) | **5635** (4473…6507) | −2.1 |
| seeds with a snapshot > 6.5 km | 4 of 8 | **0** (max 6507) | |
| land p99 / max, 500–1000 Myr (m) | 3208 / 4373 | 3309 / 4551 | 0.4 / 0.6 |
| mean land (m) | 501 | 555 | 0.9 |
| local relief (m) | 269 | **330** | 2.7 |

Earth at ~1° cells: highest cell ~5.0–5.5 km, land p99 ~4–4.5 km, mean land ~700 m (no ice).
The model's mountains are now about right at the top. Its land is still too low and too flat
below the highest cells (p99 3.3 km, mean 555 m).

## #7: hydrology and erosion

| | main | final | t | Earth |
|---|---|---|---|---|
| runoff (km³/yr) | — (rivers carried P) | 39,300 | | ~40,000 |
| water to the ocean (km³/yr) | = P on land | 36,200 | | ~37,000–40,000 |
| lake evaporation (km³/yr) / lake PET (m/yr) | — | 3,140 / 1.4 | | closed lakes 1–1.5 m/yr |
| endorheic land (% of land; of it into single-cell pits) | 0 | **10.8** (4.8) | | ~18–20 |
| lakes (% of land) | — | 3.1 | | 0.5–2 |
| depressions / endorheic / with inner sills | — | 254 / 107 / 12 | | |
| sediment trapped in lakes (km³/yr) | (filled on the river's path) | 0.20 | | |
| largest river (m³/s) | 242,000 | **95,000** | −10.3 | Amazon ~209,000 |
| sediment to the ocean (km³/yr) | 3.97 | 4.01 | 0.1 | ~7–9 (14–16 Gt/yr) |
| flooded continental crust (%) | 33.2 | 31.4 | −0.6 | 25–40 |
| land (% of surface) | 28.4 | 29.4 | 0.9 | 29 |
| felsic + sediment net (Gt/yr) | −0.65 | −0.44 | 0.6 | 0…−1.5 |
| trench sediment (Gt/yr) | 4.28 | 3.83 | −1.3 | |
| mean ocean age (Myr) | 56.9 | 56.9 | 0.0 | |
| runtime, 1000 Myr (s) | 684 | 733 (+7%) | 1.3 | ≤ +10% |

Attribution (seeds 0–3; main / final / without the isostatic solve / without runoff):
- **mean land:** 514 / 493 / 670 / 475 m. The consistent solve lowers the land by ~180 m,
  because big rivers cut 3–6× more efficiently. Runoff instead of precipitation raises it.
- **land p90:** 1262 / 1232 / 1733 / 1352 m.
- **largest river:** 249k / 112k / 74k / 274k m³/s.

## Sediment part of the sea level: a regression of the window slope

| | main | final | t |
|---|---|---|---|
| slope 500–1000 Myr (m/100 Myr) | 2.2 ± 6.8 | **12.8 ± 5.2** | 3.5 |
| slope 600–1000 / 700–1000 | 2.1 / 1.3 | 12.6 / 11.6 | 3.6 / 2.2 |
| level at 500 / 1000 Myr (m) | 270 / 283 | 235 / 283 | |
| oceanic stock growth 500–1000 (Gt/yr) | −0.02 | 0.23 | 3.4 |
| sediment to the ocean at 100…500 Myr (km³/yr, 8-seed mean) | 4.2 4.2 4.7 4.6 4.4 | 3.4 3.5 3.2 3.9 3.2 | |
| … at 600…1000 Myr | 4.1 4.1 3.8 3.8 4.1 | 3.8 4.0 4.2 4.0 4.1 | |

The level at 1000 Myr is the same (283 m). During the first 500 Myr the branch delivers ~20%
less sediment to the ocean. Two reasons: lakes and endorheic basins keep sediment on land
(0.2 km³/yr), and the discharge is lower. So the oceanic reservoir fills more slowly and is
still catching up over 500–1000 Myr. Over that window the slope exceeds the target of ≤ 10. On
seeds 0–3 it is 9.9 (final), 10.0 (without the merge cap), 9.7 (without the isostatic solve) and
17.9 (without runoff). So it comes neither from the merge-cap fix nor from the isostatic solve;
the common factor is the lake treatment. **The 500–1000 Myr slope target is not met; see open points.**

## Independent results evaluation

An independent agent re-parsed the logs with its own script and got identical numbers. It
confirmed the diagnosis:
- Every cell above 6 km on `main` sits on crust thicker than 75 km (95–120 km at 400 Myr in
  seeds 2, 3 and 7).
- The branch without the cap still reaches 7.1 km, on 89 km crust.
- `final` has no crust thicker than 75.0 km anywhere.

**Verdict:**
- **Closes #5.**
- **Part of #7.** Runoff and the solver consistency are done. The endorheic share, the lake
  share and the sediment-part slope miss their targets.

Findings:
- **F2 (pre-existing, not a regression).** Single land cells at the capped sediment column
  (12.9 km rock-equivalent, 11 km bulk) stand 2–6 km above all their neighbours. Spikes more
  than 2 km above all neighbours, over all snapshots: main 8, branch 11, final 4. The 6.5 km
  cell of `final` s5 at 800 Myr is one of them. Stream-power incision cannot remove a peak that
  has no upstream area. Possible sources: overfill on land, the 11 km metamorphism cap. This is
  a follow-up.
- **F3: the slope regression is mostly a delayed transient.**
  - The sediment part is 42 m lower at 500 Myr (t −3.5) and the same at 1000 Myr (289 vs 283,
    n.s.).
  - Sediment to the ocean over 250–500 Myr is 3.42 vs 4.58 km³/yr (t −5.1). Later it is equal or
    slightly higher.
  - It is still rising over 700–1000 Myr (11.6 vs 1.3, t 2.2), so levelling off is not shown;
    that needs 1500 Myr runs.
  - Cause: the common lake part of #7 (lakes as base levels, trapping, endorheic retention).
    Not the isostatic solve; runoff lowers it. The merge cap possibly contributes a little
    (t 1.9 vs `branch`).
- **F4.** About 45% of the endorheic area drains into single-cell pits, likely tectonic or
  resampling noise. Real multi-cell endorheic land is ~6% (Earth 18–20%).
  - Lakes overshoot at 3.1%. Lake evaporation is 1.6–4.9 × 10³ km³/yr, vs ~10³ on Earth.
  - Lake PET of 1.1–1.8 m/yr is plausible.
  - Nested lakes are a minor share.
  - The snapshots look clean.
- **F5.** Sediment to the ocean is ~4 km³/yr on both, half of Earth's 7–9; not caused by this
  change.

## Open points / follow-ups

- Recommended by the evaluation:
  - 1500 Myr runs (4 seeds, main vs this branch), to see whether the sediment part levels off.
  - Ignore single-cell pits in the drainage (minimum depression area or depth), then re-measure
    the endorheic and lake shares.
  - A new issue for the single-cell sediment spikes (F2).
  - Sediment flux to the ocean ×2 short of Earth.

- **Sediment-part slope over 500–1000 Myr: 12.8 m/100 Myr** (main 2.2; target ≤ 10). It is a
  slower fill to the same level, and the slope is still ~12 over 700–1000 Myr. Options:
  - longer runs to see whether it levels off;
  - exhuming endorheic basin fill (the basins are never incised while they are depressions);
  - a decision on whether the trend target applies to a model that traps land sediment.
- **Land too low and too flat below the peaks** (mean 555 m vs ~700 m; p99 3.3 vs 4–4.5 km).
  The erodibility K = 1.5e-7 was calibrated in 2026-11-15 under the inconsistent solve with
  P-discharge. It was not retuned here.
- **Endorheic share 10.8% vs Earth ~18–20%**, half of it in single-cell pits. Lakes cover 3.1%,
  more than Earth's 0.5–2%: every spilling depression counts as fully covered.
  - Depressions with inner sills (~12 of ~250) are filled as one hypsometric curve, not by
    Fill-Spill-Merge (Barnes et al. 2020).
  - Land below sea level is always ocean in the model, so there are no Caspian- or
    Dead-Sea-type basins.
  - Lake evaporation is not returned to the atmosphere's water budget.
- Interaction with #33 (land ice on tropical highlands): the highest cells are at the same level
  as on `main` (~5–5.8 km), and the land is somewhat higher overall (+54 m mean).
