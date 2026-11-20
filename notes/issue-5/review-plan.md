# Independent plan review (2026-11-20)

Reviewer: independent agent. It read the plan, the draft diff, `isostasy.rs`, `crust.rs` and the
climate ET/PET code. Verdict: **sound with changes**.

Confirmed:
- Implicit solve: the closed form `h = F(z−z_r)/(1+F f)` is right, since stream power is a
  rock-removal rate and rebound is rock uplift. The sediment bisection is monotone. The time
  split is positive and the base is never undershot while `f` is constant.
- Runoff: it uses the same Fu curve and the same annual P/PET as the climate's ET, so it is
  consistent. The only mismatch is the one-month lag of P, which is negligible.
- Lake water budget: on a covered cell, `R − (PET − ET) = P − PET`. A depression is a subtree of
  the flood tree rooted at its exit. `fill_lake` conserves mass. Performance is fine.

Required changes, and how they were handled:
1. **The cut can cross the 50 km dense-lower-crust threshold.** Big rivers cut several km. Above
   50 km the removal response is ~0.10 per m, below it 0.15. **Done:** `incise` solves the two
   linear pieces in closed form and `new_elevation` applies both pieces. A guard keeps the
   response ≥ 1e-3, and a unit test checks a cut across the threshold.
2. **Nested sub-basins.** Sorting all cells of a depression by floor treats it as one hypsometric
   curve. Water from sub-basin A can then "cover" a lower sub-basin B behind a sill. A full
   depression hierarchy (Fill-Spill-Merge; Barnes, Callaghan & Wickert 2020) is out of scope.
   **Done:** the cases are measured (lakes with more than one local minimum, logged) and
   recorded as a known limitation.
3. **`fill_lake` spill level.** The pass is incised in the same step. **Done:** the spill level
   is capped by the pass after incision.
4. **Lake evaporation.** The bulk PET with desert-dry air overstates the evaporation of large
   lakes (complementary relationship: Bouchet 1963, Morton 1983). Earth's closed lakes evaporate
   ~1–1.5 m/yr, while pans read 2–4 m/yr. **Done:** the area-weighted PET of lake cells is
   logged. The results decide whether a wet-environment rate is needed.
5. **Earth targets corrected:**
   - lakes ≤ ~0.5–2% of land at 120 km cells (Messager et al. 2016: 1.8% including small lakes);
   - sediment flux 14–16 Gt/yr pre-human (Syvitski et al. 2005), ≈ 7–9 km³/yr at the model's
     2050 kg/m³;
   - highest 1° cell ~5.0–5.5 km;
   - land p99 ~4–4.5 km;
   - mean land ~700 m without Antarctica, 840 m with its ice surface.

Decision on #5: "Closes #5" is acceptable if no cell exceeds ~6.5 km in any snapshot (all 8 seeds,
including 0–100 Myr) with the #7 changes. The model being too low is the real remaining gap; it
belongs in a follow-up issue, not in #5.

Validation additions:
- lake count and endorheic share in multi-cell basins vs single-cell pits;
- sediment in endorheic basins;
- attribution runs with only one of the two fixes;
- inland cells below sea level (which count as ocean).

Optional notes:
- Flexure (Te 20–50 km) would spread the rebound; local Airy isostasy remains a caveat.
- Eroding sediment decompacts the rest of the column (pre-existing, no memory of burial).
