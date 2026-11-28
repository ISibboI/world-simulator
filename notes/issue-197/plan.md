# #197: regolith — weathering of bare rock and weathering-limited erosion

Issue #197 (one of the "not modelled at all" parts of an abiotic planet, from #193's
discussion). Builds on the carbon cycle of #154/#196 ([plan](../issue-154/plan.md),
[φ for a barren planet](../issue-154/phi-barren.md)). #200 (inorganic carbonate sediment from the
carbon cycle's burial) will be stacked on this branch.

Project goal: an Earth-like planet, not Earth. Earth numbers are orientation only; the coarse
grid (~110 km cells) is accepted (#64). Values marked *(mem.)* are from memory, not checked
against the source while writing; the ranges are wide on purpose.

## 1. What is missing today

Rivers incise bedrock directly with the stream-power law `E = K Q^m S` (implicit, Braun &
Willett 2013), loose sediment first at 2 K. Nothing turns rock into transportable material in
place, so the erosion of crystalline rock does not depend on the climate at all (the runoff
enters only through `Q`), and the cold or dry bare highlands of a barren planet erode as fast as
warm wet ones. The carbon cycle's land weathering takes its supply of fresh rock from that
bedrock erosion and has no idea whether the rock is covered by a weathered layer or bare.

## 2. The model

Three materials on a land cell, each with its own erodibility relative to the (calibrated)
landscape erodibility `K` (`erosion.erodibility`, ×2 on the barren planet):

| material | erodibility | meaning |
|---|---|---|
| deposited sediment | 2 K (`sediment_erodibility_factor`, as today) | alluvium, sedimentary rock |
| **regolith** (new) | **K** | rock weathered in place: what Earth's soil-mantled crystalline landscapes erode, i.e. what `K` was calibrated on |
| **fresh bare rock** (new) | **f K**, f = `fresh_rock_erodibility` = 0.25 | unweathered rock exposed at the surface |

f: weathering lowers the tensile strength of rock by a factor ~2–5 (Murphy et al. 2016, Nature
532, bedrock channels on Kauai: the erodibility follows the rainfall through chemical
weathering of the channel bed; *mem.*), and the erodibility of bedrock by abrasion scales with
`σ_T^−2` (Sklar & Dietrich 2001, Geology 29; *mem.*), so fresh rock is ~4–25× less erodible
than weathered rock. f = 0.25 is the conservative end. Bare outcrops erode ~5× slower than the
basins around them (Portenga & Bierman 2011, GSA Today: median 12 vs 54 m/Myr; *mem.*), which
is consistent.

### 2a. Regolith production (Heimsath et al. 1997)

`P(H) = P₀ e^{−H/H*}` (m/yr of bedrock turned into regolith), `H` the bulk thickness of the
loose cover (regolith plus any sediment on it), `H*` = **0.5 m** (Heimsath et al. 1997, Nature
388: P₀ ≈ 77 m/Myr, H* ≈ 0.5 m in coastal California; Heimsath et al. 2000, SE Australia: P₀ ≈
50 m/Myr; *mem.*). The climate and the rock set the scale:

`P₀ = P_ref · L(mafic) · (q/q_ref)^a · [A(T) + c_frost F(T)] · (1 − ice)`

- `P_ref` = **50 m/Myr** (5 × 10⁻⁵ m/yr) at T₀ = 15 °C, q_ref = 0.3 m/yr runoff. Earth's soil
  production rates are 10–100 m/Myr in temperate climates with a "speed limit" of ~150 m/Myr
  (Dixon & von Blanckenburg 2012, C. R. Geosci. 344: ~320 t/km²/yr; Heimsath et al. 2012,
  Nature Geosci. 5; Larsen et al. 2014, Science 343, up to 2.5 mm/yr in the wet, steep Southern
  Alps; *mem.*). Earth's rates include burrowing animals and roots; abiotic regolith production
  (frost, thermal stress, salt, wetting–drying, hydrolysis) is taken at the lower end of the
  temperate range, with no extra factor (the biological part is #198).
- `A(T) = exp(Ea/R (1/T₀ − 1/T))`, **Ea = 50 kJ/mol**, the carbon cycle's silicate weathering
  activation energy: regolith production is mostly chemical weakening of the rock (grus,
  saprolite; Dixon et al. 2009, ESPL 34: soil production rises with temperature along the
  Sierra Nevada; *mem.*).
- `(q/q_ref)^a`, **a = 0.5**: production needs water; sub-linear like the dissolution (the
  carbon cycle uses 0.65 for the dissolution flux). Runoff rather than precipitation, because
  it is what passes through the rock (the model's runoff already removes evaporation).
- **Frost**: `F(T) = exp(−((T − T_f)/σ_f)²)`, T_f = −5 °C, σ_f = 6 K, `c_frost` = 0.5:
  frost cracking is most effective at mean annual temperatures of about −3 to −8 °C (Hales &
  Roering 2007, JGR 112; Anderson 1998; *mem.*) and raised soil production in periglacial
  landscapes (Marshall et al. 2015, Sci. Adv. 1; *mem.*). It needs water too (the runoff
  factor). Without it, Arrhenius would make cold wet highlands almost inert.
- `L` = 1 for felsic rock, **2 for basalt**, interpolated by the mafic share (1 − felsic
  fraction, as the carbon cycle does): basalt weathers faster chemically (×5–10 for dissolution,
  Dessert et al. 2003), granite disaggregates easily physically (grus): a modest factor.
- Permanent ice: no production (no glacial erosion in the model).

At 15 °C and 0.3 m/yr: P₀ = 50 m/Myr (+frost 0.5 × e^{−11} ≈ 0). Hot and wet (25 °C, 1 m/yr):
≈ 180 m/Myr. Cold and dry (−20 °C, 0.05 m/yr): ≈ 1.3 m/Myr; cold and wet (−5 °C, 0.5 m/yr):
≈ 33 m/Myr (frost).

### 2b. Regolith transport, bare rock and the regimes

Per land cell, over the part of the step after any deposited sediment on top has been cut
through (as today, at 2 K), with `x` the height of the surface above its base level and
`r = K Q^m / L` (1/yr) as in the existing implicit solve:

- regolith is carried away at the transport capacity `T = r x` (m/yr bulk) where the surface is
  covered, i.e. at `T (1 − e^{−H/H*})` (the cover effect; Sklar & Dietrich 2004; the SPACE model
  of Shobe et al. 2017 uses the same exponential with the same H*);
- bare rock (the fraction `e^{−H/H*}`) is lowered by production `P₀` and by the direct erosion
  of fresh rock `f r x`.

So `dH/dt = A e^{−H/H*} − T (1 − e^{−H/H*})` with `A = P₀ ρ_rock / ρ_regolith` (the bulk
regolith one metre of rock makes), and the bedrock is lowered at `(P₀ + f r x) e^{−H/H*}`. With
`u = e^{H/H*}` this is linear, `H* u' = A + T − T u`, and has the **exact solution**
`u(t) = u_eq + (u₀ − u_eq) e^{−T t/H*}`, `u_eq = 1 + A/T`. The bedrock removed in the remaining
time τ is `c = (P₀ + f r x) I`, `I = ∫₀^τ dt/u = (1/u_eq)[τ + (H*/T) ln((u_eq + (u₀−u_eq)e^{−Tτ/H*}) / u₀)]`
(`I = (H*/A) ln(1 + Aτ/(H* u₀))` for T = 0). As in the existing solve, `x` is that at the end of
the step (implicit, stable for any step) and contains the isostatic rebound of the removed rock;
`c` is found by bisection (`c − (P₀ + f r x(c)) I(x(c))` grows with `c`).

Regimes (Riebe et al. 2004; West 2012; Carson & Kirkby 1972):

- **transport-limited** (T ≪ A): thick regolith, `H → H* ln(1 + A/T)`, the bedrock lowers at
  ≈ T, the rate of today's model (regolith at K) — the warm, wet lowlands.
- **weathering-limited** (T ≫ A): bare rock, the bedrock lowers at `P₀ + f r x` — less than
  today wherever production is slow: cold or dry highlands, the point of the issue.
- Steady state in between: `E = (P₀ + f T) T / (A + T)`.

Timescale: `H*/T` (or `H*/P₀`), ≲ 0.1 Myr for most eroding cells, so the regolith is mostly in
balance within the 1 Myr step; the exact solution also covers slowly eroding and flat cells
(at base level, T = 0), where the regolith grows logarithmically (≈ 2–4 m bulk over 1–10 Myr at
50 m/Myr): the old landscapes of a barren planet get a thin rubble mantle, not a deep soil (no
deep saprolite: an accepted limitation of the exponential production law).

Production during the (usually short) time that deposited sediment is being cut through is
neglected; where the step ends inside the deposit, production under it continues with the
cover H of the remaining deposit plus regolith (T = 0 for the regolith underneath), i.e. it is
nil under more than a few metres.

### 2c. Mass and state

- **No new mass reservoir**: regolith is stored in the sediment layer of the crust (it *is*
  loose material of the same mass per metre, rock-equivalent thickness at 2050 kg/m³), so the
  existing sediment ledger, isostasy, the cover budget and the sea-level budget see it without
  change. Bedrock turned into regolith is `bedrock_delta < 0`, `sediment_delta > 0` of the same
  mass, exactly as eroded bedrock becomes sediment today. Mass is conserved by construction
  (export = start + produced − end); unit tests check it to rounding.
- **A per-cell label** `regolith` (Planet, m rock-equivalent, ≤ the cell's sediment) marks the
  bottom part of the cover as regolith, so that the next step cuts the deposit above it at 2 K
  and the regolith in the ODE at K, and so that the carbon cycle does not count it twice (§3).
  Labels stay with the cells, not the crust samples; the plates move ~half a cell per step, so a
  label may land on another sample: it is clamped to that sample's sediment (a label error, not
  a mass error; the regolith equilibrates within ~0.1 Myr where it erodes). Ocean and lake cells
  have none (their thin rubble becomes ordinary sediment).
- Diagnostics per cell (kept for the cell dump and the statistics): regolith thickness, P₀, the
  time-mean bare fraction `I/τ`, the bedrock lowering rate, and the regime.

Switch: `--set erosion.regolith=0` gives today's model bit-identically (the old closed-form
bedrock solve; checked against the state hashes of main).

## 3. Coupling to the carbon cycle (#196)

The land weathering is `w = w_sup (1 − e^{−w_kin/w_sup})` per cell (plan §3a of #154).

- **Supply**: the fresh rock entering the weathering zone is the bedrock turned into regolith
  plus the fresh rock eroded directly: together the bedrock lowering `c`, which the supply uses
  today (`bedrock_erosion`). Unchanged. The regolith carried over from earlier steps was counted
  when it was produced, so its later removal is **not** added again to the eroded-sediment term
  (`sediment_weatherable` × eroded sediment): the sediment term counts only the deposit above
  the regolith label. No double counting.
- **Residence time / contact**: the kinetic rate gets the time-mean regolith-covered fraction
  `R = 1 − I/τ` (1 where a deposit covered the cell all step): water on bare rock runs off with
  little contact time; where the regolith is thick, the rock stays in it long enough to approach
  the supply limit (West 2012, Geology 40: weathering flux `∝ E (1 − e^{−K (Z/E)^σ})` — it grows
  with the regolith thickness Z and its residence time Z/E; Maher & Chamberlain 2014, Science
  343; Gabet & Mudd 2009, Geology 37: weathering peaks at intermediate erosion rates and drops
  where the soil thins to bare rock; *mem.*). So `w_kin → w_kin · R`. `k_land` was calibrated
  on Earth's mostly soil-mantled land (R ≈ 1), so R does not double count the plant factor or
  the calibration. The regimes then are West's: thick regolith → supply-limited; thin
  regolith and fast erosion → kinetic, and bare rock → small.
- The O₂ sinks of the escape (#210) use the bedrock erosion: unchanged meaning.

**Risk to φ.** R < 1 lowers the land sink at the start; land is ~1/3 of the epoch-0 sink, so φ
must rise to rebalance. If the mean R on the weathering land is ~0.5, φ rises by roughly
×1.2–1.5 from today's 0.95–1.6 (barren), i.e. up to ~2.4: inside 0.3–3 but closer to the clamp.
Lower erosion of cold/dry highlands also lowers the supply there, but those cells are already
supply- and temperature-poor. The modern preset is clamped at 3 already (φ balancing 3.5–8.2);
it stays clamped, a bit further from balance (its CO₂ then settles higher after the window).
If the barren φ hit the clamp, the regolith model would be too weak (P_ref too low or H*
too small) and would need a second look rather than a wider clamp.

## 4. Code

- `src/erosion.rs`: `RegolithParams` inside `ErosionParams` (`regolith`, `regolith_production`,
  `regolith_depth`, `regolith_activation_energy`, `regolith_runoff_exponent`,
  `regolith_frost`, `fresh_rock_erodibility`, `regolith_mafic_factor`, settable with
  `--set erosion.NAME`); `Surface` gets the annual mean temperature, ice, mafic share and the
  regolith label; `incise` gets the regolith phase; `ErosionResult` gets the per-cell regolith,
  bare fraction, P₀ and regime. All transcendental math through `crate::math`.
- `src/planet.rs`: keeps the label and the diagnostics, passes the bare fraction and the
  corrected sediment erosion to the carbon cycle; statistics line.
- `src/carbon.rs`: `LandCell::regolith_cover` (R), multiplied into the kinetic rate.
- `src/main.rs`: snapshot log line (regolith thickness quantiles on land by climate band,
  regime split by area and by eroded mass, mean P₀), cell-dump columns `regolith`, `bare`,
  `production`, `bed_erosion`.
- No new render mode or cell-info row (frame size); the regolith is part of the sediment shown.
- Presets: no biosphere dependence yet (#198 adds plants: faster production, cohesion).

Tests: production on flat land conserves mass and grows logarithmically; the analytic ODE
solution against a fine explicit integration; steady state `H = H* ln(1 + A/T)`; weathering-
limited cells erode slower than before and transport-limited ones as before; `regolith=0` gives
the old deltas exactly; erosion conserves rock mass with regolith; the carbon cycle's land sink
scales with R and does not count labelled regolith as eroded sediment.

## 5. Evaluation

Commit, then through `runs`: default barren, 8 seeds × 1000 Myr, against main (baseline runs
of main at e55bf1d exist: `--duration 1000 --cell-dump --snapshot-interval 100`), and
`--biosphere modern --epoch 4`, 4 seeds. Reported: regolith thickness distribution vs climate
(temperature/runoff bands), erosion rates and the regime split, land weathering and the
CO₂/temperature change, φ per seed (inside 0.3–3?), the sediment budget (to the sea,
continental cover, denudation), relief (land mean, hypsometry, max), runtime. An independent
agent evaluates. Results to `notes/issue-197/results.md`.

## 6. Changes after the review ([`review-plan.md`](review-plan.md))

| finding | change |
|---|---|
| M1 density: with transport in bulk regolith, the transport-limited limit lowers the bedrock at `T ρ_regolith/ρ_rock` ≈ 0.4 T, not T | The capacity `r x` is a **bedrock-equivalent (mass)** rate: the regolith ODE gets `T_bulk = r x ρ_rock/ρ_regolith`, so the transport-limited limit is exactly today's `r x`, and the steady state is `E = (P₀ + f T) T/(P₀ + T)` with `T = r x`. Regolith porosity **0.4** of its own (not the sediment's 0.6 surface porosity). |
| M2 f = 0.25 also hits fast-eroding orogens (Larsen et al. 2014: production keeps pace up to ~2.5 mm/yr; threshold hillslopes and landslides deliver fresh bedrock at the river's rate) | `f(T) = f + (1 − f) T/(T + T_c)`, **T_c = 0.5 mm/yr** (`regolith_threshold_erosion`): slow rivers on bare rock at 0.25 K, fast ones approach K. Montgomery & Brandon (2002). |
| M3 implicit solve | The end-of-step height uses the rebound of the removed bedrock with the dense-crust piecewise response (as today); the few metres of regolith staying in the cell are left out of the height (≪ the cell's relief). Stated. |
| M4 φ risk, bare rock still weathers | Kinetic factor `b + (1 − b)(1 − bare)` with **b = 0.1** (`carbon.bare_rock_weathering`). Stop rule: if the barren φ > 2.5, revisit before the PR. |
| minor: Ea 50 kJ/mol too steep for production | **Ea = 30 kJ/mol** (chemical + physical; Dixon et al. 2009 found no steady rise with temperature). |
| minor: dry cells | Floor **2 m/Myr** (`regolith_dry_production`) for salt and thermal weathering, also without runoff. |
| minor: e^{−H/H*} as both production law and bare fraction | Stated as a modelling choice (as in SPACE). |
| minor: diffusion moves regolith | The label is clamped to the sediment left after deposition, diffusion and waves; removal by diffusion is regolith removal (not counted as eroded sediment for the carbon cycle). |
| numerics | `u` capped at e^600, T → 0 closed form below `T τ/H* < 10⁻⁹`, A = T = 0 handled, fixed 60 bisection iterations. |
| evaluation | Relief and hypsometry against main are the main acceptance check; a sensitivity run with f = 1 (`erosion.fresh_rock_erodibility=1`) on 4 seeds if the relief changes much. |

The cold-wet example of §2a with Ea 30 kJ/mol and the frost term: ≈ 45 m/Myr at −5 °C, 0.5 m/yr.
