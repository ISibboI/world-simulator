# Review of notes/issue-197/plan.md (regolith, weathering-limited erosion)

Reviewer: independent agent, 2026-11-28. Read-only; checked against src/erosion.rs (`erode`, `incise`), src/isostasy.rs, src/carbon.rs (`weathering_cells`, `sinks`), src/presets.rs.

## Math check (passes)

- `u = e^{H/H*}`: `H* u' = u·dH/dt = A + T − T u`. Correct.
- `u(t) = u_eq + (u₀−u_eq)e^{−Tt/H*}`, `u_eq = 1 + A/T`. Correct.
- `I = ∫dt/u = (1/u_eq)[τ + (H*/T) ln(u(τ)/u₀)]`. Checked by differentiation. Correct. The T = 0 limit `(H*/A) ln(1 + Aτ/(H*u₀))` is also correct.
- Steady state: `A b = T(1−b)` gives `b = T/(A+T)`, so `E_rock = (P₀ + fT)·T/(A+T)`. Correct. But see M1 for what it means.
- P₀ examples: 15 °C → 50; 25 °C / 1 m/yr → 183; −20 °C / 0.05 → 1.2. These are right. **Cold and wet (−5 °C, 0.5 m/yr) is ≈ 46 m/Myr, not 33.** The 33 is the frost term alone; Arrhenius adds 0.21. Minor.
- Flat-land growth: 2.5 m after 1 Myr and 3.7 m after 10 Myr, with A ≈ 75 m/Myr. Consistent (but see M1: with this code's densities, A ≈ 2.6 P₀).
- Bisection monotonicity holds. As c grows, x and T fall, the bare fraction falls, so `(P₀+frx)·I` falls.

## Blockers

None in the concept. M1 and M2 must be settled before coding, because they decide whether erosion drops by ~2–4× everywhere.

## Major

**M1. The density factor makes the transport-limited regime erode about 2.5× slower than today, not "≈ T, as today".**
- In the transport-limited limit, `E_rock = (P₀+fT)T/(A+T) → T·P₀/A = T·ρ_reg/ρ_rock`.
- In this code, the bulk density of a thin cover follows `SURFACE_POROSITY = 0.6`: 0.4 × 2700 ≈ 1080 kg/m³. Bedrock is 2700–2900 kg/m³. So `A ≈ 2.6 P₀`, and soil-mantled cells lower at ≈ 0.4 T. Today they lower at T. Warm, wet lowlands would also lose ~60% of their erosion, and the surface lowering drops with it. That means relief growth everywhere, not only on cold or dry highlands.
- In mass terms, fresh rock at f = 0.25 is then only ~1.6× less erodible than regolith.
- Fix: write the regolith transport in rock-equivalent (mass) terms, so that the transport-limited limit is today's rate (`E → T` in rock metres). Or keep it bulk and recalibrate K. Then state the convention explicitly.
- Related: a porosity of 0.6 is high for regolith (soil and saprolite are ~0.3–0.5). If bulk H matters (H* is a bulk length), consider a regolith porosity of its own, ~0.4.
- The same quirk already affects deposited sediment: 2 K on bulk thickness is only ≈ 0.8 K in mass near the surface. Worth a note, not a fix here.

**M2. f = 0.25 cuts erosion of the fastest-eroding orogens by ~4×. Larsen et al. 2014 contradicts this.**
- Where T ≫ A, `E ≈ P₀ + 0.25 T`. In steep, wet, active ranges (T ~ mm/yr, P₀ ≤ ~300 m/Myr even at 10 m/yr runoff), this is ≈ 0.25–0.35 × today.
- With n = 1, the steady-state channel relief scales with 1/K_eff, so orogens could get ~3–4× steeper and higher until isostasy or other limits stop them.
- Larsen 2014 (cited in the plan) is exactly the finding that soil production keeps pace with ~2.5 mm/yr denudation in the Southern Alps: there is no fixed speed limit. In steep ranges, erosion goes through landsliding and fractured rock. Earth's K is calibrated on denudation that includes such terrain.
- The plan's `P₀` does not depend on the erosion rate or the slope, so it would mark every active orogen as weathering-limited.
- Suggested changes, any of:
  - (a) let f rise towards 1 with the transport rate or slope, e.g. `f_eff = f + (1−f)·T/(T+T_c)`, with T_c ~ 0.2–0.5 mm/yr: threshold hillslopes and tectonic fracturing;
  - (b) let P₀ grow with T, after Larsen: production scales with the denudation rate;
  - (c) at minimum, a sensitivity run with f = 0.25 / 0.5 / 1 that reports max elevation, orogen relief and hypsometry against main.
- The issue targets *cold or dry, slowly eroding* bare highlands. The rule should not hit fast, wet orogens.
- Also: Portenga & Bierman's outcrop-vs-basin contrast is a selection effect (outcrops persist *because* they erode slowly). It does not measure f. Drop "consistent" or call it weak support.

**M3. x(c) in the implicit solve must use the exported mass, not c.**
- The bedrock converted to regolith that stays in the cell is not removed from the column. Its mass stays, and its bulk volume grows by ~2.5×, so the surface rises.
- The end-of-step surface used for T = r x therefore has to come from: column − (rock-equivalent exported) + volume change of the regolith that stays.
- If it takes "isostatic rebound of the removed rock c" from today's closed form, it overstates the lowering for regolith-building cells and understates T.
- The bisection must also handle the dense-crust piecewise response, as `incise` does now. Specify this.

**M4. The φ risk needs a quantitative check before the 8-seed runs.**
- R = A/(A+T) at steady state. If much of the weathering land is at T ≫ A, mean R can fall well below 0.5. The land sink is mostly kinetic-limited on a barren planet, so φ could reach the clamp at 3.
- Cheap pre-check: from main's cell dumps (e55bf1d baseline: runoff, T, bedrock erosion rate), compute P₀, A, T ≈ the current rate and R offline. Report the area-weighted and supply-weighted R and the implied φ.
- This also previews the regime map. The uncommitted `DIAG` block in `src/erosion.rs` points this way. Remove it before committing: it is unformatted and floods the info log.

## Carbon coupling (mostly sound)

- **Supply = c, unchanged; exported regolith not counted again.** Correct and consistent with West 2012 and Maher & Chamberlain 2014: at steady state the supply is the regolith production (= denudation).
- **Kinetic × R: plausible but crude.** West's kinetic term depends on the residence time Z/E, with Z the weathering-zone depth (metres to tens of metres). R saturates at H ≈ 1.5 m. The existing `w_kin/w_sup ∝ 1/E` already carries the E part of Z/E, so R adds only a "thin-cover" penalty. That is acceptable and does not double count.
- **R → 0 on bare rock is too strong.** Rain and runoff on and in fractured bare rock still weather it. Use `R_eff = R_min + (1−R_min)R` with R_min ~ 0.1–0.2. This also reduces the φ risk in M4.
- **Label drift.** A deposit mislabelled as regolith after the plates move gets its eroded share left out of the sediment term. Small; acceptable; mention it.

## Minor

1. **Ea = 50 kJ/mol for all production is strong.** It gives ×0.056 at −20 °C. Global compilations (Dixon & von Blanckenburg 2012; Heimsath et al. 2012) find soil production only weakly temperature-dependent, because physical disaggregation is not Arrhenius. Dixon et al. 2009 (Sierra Nevada) found the highest production at *intermediate* elevations, not a monotonic rise with temperature. Suggest a chemical/physical split, e.g. `P₀ ∝ (1−s) + s·A(T)` with s ~ 0.5, or a lower Ea (~20–30 kJ/mol). Fix the citation wording.
2. **Dry cells.** With runoff → 0, P₀ → 0. Salt and thermal weathering act in deserts. Consider a small floor (~1–3 m/Myr, × (1 − ice)).
3. **Production law vs bare fraction.** `e^{−H/H*}` is used both as Heimsath's depth dependence and as the bare fraction (SPACE-style). Fine, but say it is a modelling choice. Real bare rock often produces less (humped law; Heimsath 2009, Wilkinson 2005), which matters exactly in the weathering-limited regime.
4. **Hillslope diffusion of land sediment** (`diffuse_sediment`, 10⁴ m²/yr on the barren preset) will also move regolith after the ODE. That is a second transport path that the regime analysis ignores. Decide whether the regolith takes part, and how the label is updated: removal from the top hits the deposit first, then the label is clamped.
5. **Heimsath 1997.** H* was ≈ 0.4 m (α ≈ 0.025 /cm). 0.5 m is fine, but say "0.4–0.5 m".

## Numerics and determinism

- **Overflow.** `u₀ = e^{H/H*}` overflows for H > ~350 m, which happens under deposits on the "step ends inside the deposit" branch. Work with the bare fraction `b₀ = e^{−H₀/H*}`: `I = (1/u_eq)[τ + (H*/T)·ln_1p((u_eq b₀ − 1)·(−exp_m1(−Tτ/H*)))]`. It has no overflow, and b₀ = 0 gives I = 0 cleanly.
- **T → 0.** `u_eq = 1 + A/T` blows up, and `(H*/T)·ln(…)` cancels catastrophically. Switch to the T = 0 formula or a series when `Tτ/H* < ~1e-6`.
- **A = 0** (ice, dry): `u_eq = 1`, fine. **A = T = 0**: `I = τ b₀`, needs its own branch.
- Use `math::exp`, `math::exp_m1`, `math::ln_1p` (add it to `crate::math` if missing, with a deterministic implementation), never `f64::ln`.
- **Bisection bracket.** Use `[0, (P₀ + f r x₀)τ]` and a fixed iteration count (as now: 50) for determinism. Note the cost: ~40k land cells × 50 × several exp/ln per step. Measure the runtime.

## Tests and evaluation additions

- Unit tests:
  - the transport-limited limit equals today's rate after M1's fix;
  - no NaN or Inf for H = 5 km of deposit, T = 0, A = 0, and both zero;
  - continuity across the T → 0 switch;
  - the implicit x(c) with the regolith that stays (M3);
  - with f_eff → 1 at high T, the result matches today.
- Evaluation:
  - the f and Ea sensitivity, at least 2 seeds each;
  - max elevation, orogen relief and hypsometry vs main as the primary acceptance check;
  - the regime split by *eroded mass*, not only area;
  - φ per seed, with a stop rule if it is > 2.5;
  - the modern-preset run, to make sure the change does not shift its uncalibrated CO₂ much.
