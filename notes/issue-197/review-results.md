# #197 independent evaluation of results.md (commit 038c6c7)

Scope: plan.md (incl. §6), results.md, diff e55bf1d..038c6c7, raw logs/cell dumps of runs/{default,modern,f1} and base/{default,modern}. Recomputed with my own scripts (`~/tasks/issue-197/tmp/rev.py`, `cells.py`), not with analyse.py.

**Verdict: no blocker. Open the PR** after the text fixes below (M1–M4). Physics, conservation and the carbon coupling look sound. Some numbers in results.md are wrong or don't compare like with like.

## What I verified (correct)

- **φ** per seed from "carbon cycle sets … phi=": regolith 1.158–1.711, mean 1.37; main 0.914–1.397, mean 1.18; f=1 1.33; main seeds 0–3 1.10. Never clamped in barren runs. ✓
- **Carbon, 200–1000 Myr** (from the per-step lines): pCO₂ 0.471 vs 0.478 bar, land sink 2.77 vs 3.51, seafloor 9.67 vs 8.75, T 16.47 vs 17.03. Land share of the sink 22.3% vs 28.6%. Kinetic factor 0.1+0.9·0.58 = 0.62. ✓
- **Elevation**: land mean 784 vs 571 (per-seed values match). Hypsometry seed means 22/336/2637/4180 vs 25/225/1690/3230. ✓ Sediment to the ocean 4.05 vs 4.67 km³/yr, denudation 13.23 vs 14.59, cover stock 4.29e8 vs 4.95e8. ✓
- **Regolith-by-climate table**: reproduced exactly (all n, quantiles, bare means, P₀ medians, lowering quantiles; total 97 154).
- **Modern**: T 23.47±0.35 vs 22.08±0.64; pCO₂ 715 vs 506 µbar (from the PAL figures); land 4.49 vs 5.66; seafloor 5.23 vs 4.23; land mean 840 vs 836; regime split 52/48. ✓
- **Budgets**: all 120 `unaccounted` (Gt/yr) and 120 `unexplained` (m) terms in the branch runs are ≤ 1.4e-13 (main ≤ 1.6e-13). All exits are 0. No NaN/inf in any cell dump. The only "φ NaN" are the ramp-up steps, the same 31 as on main.
- **Stability** (step-to-step, 200–1000 Myr, barren): rms Δln pCO₂ 0.0032 on both branches. Sign-reversal fraction 0.137 vs 0.115. Relative step-to-step SD of the land sink 0.029 vs 0.021: a bit noisier, but no oscillation. Modern: 0.034 vs 0.046, so not worse.
- **Double counting (code)**: supply = bedrock lowering, which includes regolith production. In planet.rs, the sediment term subtracts `start label − end label`, which leaves exactly the eroded deposit. Regolith produced and carried off in the same step is counted once. ✓
- **The −0.5 K is explained by the relief**: 213 m × 6.5 K/km × 0.28 land ≈ 0.39 K at almost the same forcing (61.0 vs 61.1 W/m²). Worth adding this one-liner.

## Findings by severity

### Medium (fix the text before the PR)

**M1. Regime per-cell rates are wrong.** results.md says "~15 m/Myr vs ~57 m/Myr per cell". The log means (200–1000 Myr, 72 snapshots) are **12.0 (transport-limited) vs 75.2 (weathering-limited)**, medians 11.4 / 68.7. With 3770 and 2420 cells, 15/57 would give a 71% weathering-limited share, not 78%. The log's numbers do give 78–80%. Also, "of ~12 000 land cells": the log reports ~10 600 land cells (those with a receiver). Untransported cells are ~4 400 (41%), with a bedrock share of 0%.

**M2. The f-sensitivity table mixes seed sets.** Main and f=1 use seeds 0–3, but the f=0.25 column uses 8 seeds. Default seeds 0–3: land mean **733** (not 784), highest point **4257** (not 4480), φ 1.31, land sink 2.73, T 16.8, sediment to ocean 3.87. The conclusion still holds (main 575 / f=0.25 733 / f=1 548). But "highest peaks rise only ~10%" becomes +5% for seeds 0–3 (+14% for 8 seeds), and the max is a noisy statistic. Also, f=1 is not exactly main: bare rock erodes at `P₀ + r x` > `r x`, which fits the slightly lower 548 m and the 76% "weathering-limited" share.

**M3. Modern: "settles ~40% higher" is misleading.** pCO₂ never settles. On both branches it falls 3–6× between 200–300 Myr and 900–1000 Myr (branch ~6.4→1.7 PAL on seed 0; main 4.2→0.7). The +40% is a window mean of a drifting series. Say "stays ~40% higher on average while both drift down". Also report the branch's balancing φ: **4.8–10.3** (main seeds 0–3: 3.7–8.7; the "3.5–8.2" quoted is not these seeds). The clamp is now further from balance. Otherwise the explanation is sound. With φ clamped, the lower land sink cannot be compensated, so CO₂ and T rise until seafloor plus land balance degassing (total sinks 9.72 vs 9.89). +2 W/m² for +1.4 K is consistent. Add one sentence on why the modern relief does not change: undoubled K, so slower rivers and more transport-limited cells (52/48 vs 22/78).

**M4. The relief change is understated.** "Upper half higher" hides that the 90th percentile of land rises **+56% (1690 → 2640 m)** and the 99th from 3230 to 4180 m. 10% of the land is above 2.6 km (Earth orientation ~2 km). This is the main geomorphic consequence of f=0.25 and belongs in decision 1 for Sebastian, next to the mean (+213 m).

### Minor

- **m1. Unclamped label in the carbon bookkeeping** (planet.rs): `regolith_removed = self.regolith.thickness[cell] − result.regolith[cell]` uses the previous label before `erode` clamps it to the sediment. Where plate motion moved a label onto thinner sediment, eroded sediment is understated (clamped at ≥ 0). The effect is small: in the dumps, ~15 400 default land records (~1.5%) have regolith > sed + 0.05 m, by at most 1.74 m. Fix: return the clamped start label from `erode`, or clamp in planet.rs. A label error, not a mass error.
- **m2. Dump artefacts to mention in a caveat** (no mass effect):
  - regolith > sed: the label is not clamped until the next step.
  - Regolith on 1 077 ocean-flagged records: almost all coastal cells at −0.2 to −2.7 m after the sea-level update. 35 are deeper than −50 m (moved labels). They are zeroed at the next step.
  - `bed_erosion` > 0 on ocean cells (wave/cliff and timing).
  - 25 of 5 072 lake cells with bare > 0: negligible, but the plan says lakes get none.
- **m3. Ice:** permanent ice stops production, but fresh-rock fluvial erosion goes on at `f(T)K`. The largest `bed_erosion` (5.0 mm/yr) is a fully bare ice=1 cell. Rivers under ice sheets are pre-existing behaviour (no glacial model); worth one caveat line.
- **m4. Scope of the climate table:** it covers only eroding cells (bare > 0, about half the land). Over all land, the log's medians are 0.00 m in the warm bands, because depositional land has no label. "Thickest in warm climates" holds only for eroding rock; say so.
- **m5. Regime label meaning:** "weathering-limited" here means `P₀ < capacity` at the end of the step. With f(T) → 1 on fast rivers, many of these cells still erode close to the old law (at 75 m/Myr, f(T) ≈ 0.36). Under f=1 the label limits nothing. One sentence would avoid misreading.
- **m6.** Mention that the land sink's step-to-step variability is ~40% higher (still small, no oscillation).

## Physical plausibility (barren planet)

Decimetres to ~1 m of rubble, bare-rock fraction 0.27 (warm) to 0.66 (cold, wet), P₀ medians 11–80 m/Myr, lowering medians 4–20 m/Myr, 90% 19–123 m/Myr. All of this is plausible for an abiotic Earth-like planet. Maximum `bed_erosion` is ~5 mm/yr and P₀ goes up to ~0.4 mm/yr in hot, wet, mafic cells (below the ~2.5 mm/yr "speed limit" orientation). No negative values, no NaN, no runaway thickness: the maximum regolith in the eroding population is ~2.7 m, consistent with logarithmic growth. A φ rise of +16% (well below the 2.5 stop rule) and a land sink 21% lower with pCO₂ unchanged are what the design predicts. The model is internally consistent.

## Suggested edits to results.md

1. Regimes: "~12 000" → "~10 600"; "~15 vs ~57 m/Myr" → "~12 vs ~75 m/Myr (medians 11 / 69)"; add the meaning of the label (m5).
2. Sensitivity table: use seeds 0–3 for f=0.25 (733 m, 4257 m, φ 1.31, 2.73 Tmol/yr, 3.87 km³/yr, 16.8 °C) or label the mix clearly; peaks "+5–14%, noisy".
3. Barren table/decision 1: add P90 +56% (1690 → 2640 m) and the lapse-rate estimate of the −0.5 K.
4. Modern: "drifts down in both runs, ~40% higher on average"; branch balancing φ 4.8–10.3 (main 3.7–8.7, seeds 0–3); one sentence on the unchanged modern relief.
5. Caveats: dump labels not yet clamped (m2), ice (m3), table scope (m4). Fix or note m1.
