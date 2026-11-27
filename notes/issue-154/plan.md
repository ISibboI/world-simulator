# #154: carbon cycle (carbonate–silicate feedback) for a barren planet

Issue #154; Sebastian's decisions on it: a **barren planet** (no organic carbon, no
biosphere until #90), **no spatially resolved volcanoes** (#106), and **all mantle degassing
approximated from the tectonics** (ridges, arcs, plumes, metamorphic and collisional
decarbonation). Stacked on #193 (`--epoch` counts forwards, the default run is a barren planet
at epoch 0, 4.0 Ga), which warms ~7.6 K over 1000 Myr because the greenhouse forcing stays fixed
while the sun brightens.

Values marked *(mem.)* are from memory and not checked against the source while writing this;
the ranges are wide on purpose. The model does not need them to be exact: what matters for the
dynamics is the **sensitivity** of the sinks to CO₂ and temperature, and the **relative**
changes of the sources with the tectonics (see §4, the calibration at the start).

## 1. State variable and greenhouse forcing

**State:** the exogenic carbon `N` (mol) of atmosphere + ocean, one global number. It is the
conserved quantity: `N(t+dt) = N(t) + dt (D − W)`, sources `D`, sinks `W` (mol C/yr).

**Partitioning** (the atmospheric CO₂ follows from `N` each step; the ocean is in carbonate
equilibrium on the 1 Myr step, its mixing time is ~1 kyr):

- atmosphere: `N_atm = p A / (g M̄)` with `p` the partial pressure of CO₂, `A` the planet's
  area, `M̄` = 0.029 kg/mol. 280 µbar → 5.0 × 10¹⁶ mol (600 GtC ✓).
- ocean: `N_oc = M_oc (DIC₀ r^{1/3} + K₀ p)` with `r = p / 280 µbar`, `M_oc` the ocean's mass
  (from the model's ocean volume), DIC₀ = 2.0 mmol/kg (pre-industrial surface ~2.0, deep ~2.25;
  *mem.*), K₀ = 0.04 mol/(kg bar) (Henry's constant at ~10 °C; *mem.*). The bicarbonate scales
  as `p^{1/3}` when calcite saturation and charge balance by Ca²⁺ fix the carbonate system
  (`[HCO₃⁻]³ ∝ p` for `[Ca²⁺] ≈ alk/2`, `[Ca²⁺][CO₃²⁻] = K_sp`): the long-term (carbonate
  compensated) buffer, not the short-term Revelle buffer. Today: 2.8 × 10¹⁸ mol in the ocean,
  2.9 × 10¹⁸ total (38 000 GtC ✓).

**Forcing:** `F(r) = 5.35 ln r + 0.39 (ln r)²` for `r > 1`, Myhre et al. (1998) `5.35 ln r`
for `r ≤ 1`. The quadratic term is Byrne & Goldblatt (2014, GRL 41, 152; *checked*: their fit is
`F = 5.32 ln(C/C₀) + 0.39 ln²(C/C₀)`, fitted 200–10 000 ppmv, 38.1 W/m² at 50 000 ppmv), with
Myhre's 5.35 so that the forcing is smooth at today's CO₂ and the existing low-CO₂ floor
(`MIN_CO2`, −17.8 W/m²) stays. Above 10 000 ppmv it is an extrapolation. Myhre alone
underestimates the forcing at high CO₂ (it gives 29 bar for the epoch-0 forcing).

- One forcing law for everything: `presets::co2_forcing(r)` and its inverse; `--co2` also uses
  it (changes the forcing of `--co2` > 1: e.g. 10 × → 14.4 instead of 12.3 W/m²).
- **Initial CO₂ follows from the start's forcing** (the epoch's default, `--co2`, or `--set
  temperature.greenhouse_forcing`), by the inverse, so the start is continuous:
  - epoch 0 (+61.8 W/m²): r ≈ 1770, **p ≈ 0.50 bar**. Radiative–convective models give
    ~0.1–0.3 bar for this forcing at 4 Ga without methane (Kasting, 1993; Haqq-Misra et al.,
    2008; von Paris et al., 2008; *mem.*): the extrapolated fit probably underestimates the
    forcing of a thick CO₂ atmosphere (pressure broadening, more bands), so the CO₂ in bar is
    indicative. What the dynamics see is `dF/d ln r` = 5.35 + 0.78 ln r ≈ 11 W/m² per e-fold
    at epoch 0 (2 × Myhre), i.e. the CO₂ needed to change the forcing by some amount, which
    the fit captures better than a pure log.
  - epoch 4 (`--biosphere modern --epoch 4`): 0 W/m², 280 ppm.
  - `p` is reported as `r × 280 µbar`; the extra pressure of a thick CO₂ atmosphere is ignored.
- The energy balance model gets the forcing through `TemperatureParams::greenhouse_forcing`
  as before (`A − F + B T`), set each step by the carbon cycle.

**Reservoir and timescales** (`τ = N / D`, with `D` ≈ 7 Tmol/yr):

| | p | N_atm | N_oc | τ |
|---|---|---|---|---|
| today (280 ppm) | 280 µbar | 5 × 10¹⁶ | 2.8 × 10¹⁸ | 0.4 Myr |
| epoch 0 | 0.5 bar | 9 × 10¹⁹ | 6 × 10¹⁹ | ~20 Myr |

The **feedback timescale** is `τ_fb = τ · (d ln N / d ln r) / (d ln W / d ln r)`. At epoch 0:
`d ln W / d ln r` ≈ β (0.5) + (dF/d ln r) λ (Ea/RT² + runoff) ≈ 0.5 + 11 × 0.6 × 0.10 ≈ 1.2,
`d ln N / d ln r` ≈ 0.8: τ_fb ≈ 15 Myr. Today: ≈ 0.4 × 0.35 / (0.5 + 5.35 × 0.6 × 0.1) ≈
0.2 Myr, shorter than the 1 Myr step. Both regimes must be stable → §5.

## 2. Sources: tectonic degassing (mol C/yr)

Earth's total magmatic + metamorphic degassing: ~6–10 Tmol C/yr (*mem.*: Marty & Tolstikhin,
1998, ~6 incl. plumes; Burton et al., 2013, ~12 incl. diffuse; Plank & Manning, 2019; Werner et
al., 2019), split roughly ridges 1.5–2.5, arcs 2–3.5 (Kelemen & Manning, 2015: 18–43 Mt C/yr
= 1.5–3.6 Tmol/yr), plumes/hotspots 0.5–3 (poorly known), metamorphic 0.5–2 (Kerrick &
Caldeira, 1998; Becker et al., 2008; *all mem.*).

Each source is a coefficient (mol C per unit of a tectonic rate) set from Earth's flux divided
by Earth's rate, so the model's planet degasses more when its tectonics is faster. Rates come
from the cumulative tectonic diagnostics (differences per step), all already tracked:

| source | model rate (per step) | Earth rate | coefficient | model (#190 runs) |
|---|---|---|---|---|
| ridges | `MassLedger::new_sea_floor` (kg/yr) | ~65 Gt/yr (3.4 km²/yr × 6.5 km × 2.9 t/m³) | 2 Tmol/yr / 65 Gt = **0.03 mol/kg** (≈ 0.13 wt% CO₂ in the melt) | 80–140 Gt/yr → 2.4–4.2 Tmol/yr |
| arcs, slab crust | `subducted_oceanic` (kg/yr) | ~65 Gt/yr | **0.025 mol/kg**: altered crust ~0.1–0.2 wt% CO₂, about half returns (Kelemen & Manning, 2015; *mem.*), plus the wedge's own carbon | 80–140 Gt/yr → 2–3.5 |
| arcs, slab sediment | `subducted_sediment` (kg/yr) | ~2–3 Gt/yr | **0.15 mol/kg**: barren planet without pelagic carbonate: ~3 wt% CaCO₃ (today's GLOSS ~7 wt%; Plank & Langmuir, 1998; *mem.*), half returns | 5–10 Gt/yr → 0.8–1.5 |
| plumes | Σ `Plume::flux()` (m²/s, head growth and fading included) | — | **1 Tmol/yr per 0.036 m²/s** (the model's typical ~9 plumes; Earth hotspots 0.5–3 Tmol/yr, *mem.*) | 0–1.4 |
| metamorphic / collisional | sediment metamorphosed (`metamorphosed`, both parts) + sediment stacked in collisions (`TrenchSediment::stacked`) (kg/yr) | — | **0.5 mol/kg**: barren-planet sediment with ~10 wt% carbonate (inorganic platform/evaporitic carbonate; *assumption*), half decarbonated | 0.5–3.5 Gt/yr → 0.25–1.8 |

Total for the model's tectonics: ~6–12 Tmol/yr, Earth-like. The plumes start at zero (no
plume at t = 0); their first ~50 Myr raise D by ≲ 10%.

Not modelled (later, with #106 or on request): a hotter young mantle with faster degassing,
carbon reservoirs on the plates (the carbon a step buries is not tracked to its later
subduction; fixed concentrations stand for its steady state), LIP pulses, thermogenic carbon.
#106 must book any new magmatic source as one of these fluxes (or a new one) without double
counting (e.g. plume-fed crust is either "plumes" or "ridges", not both).

## 3. Sinks

On a barren planet the long-term sink is **silicate weathering followed by carbonate
burial**: CaSiO₃ + 2 CO₂ + H₂O → Ca²⁺ + 2 HCO₃⁻ + SiO₂, then Ca²⁺ + 2 HCO₃⁻ → CaCO₃ + CO₂ + H₂O,
net 1 C per Ca (Mg) (Walker et al., 1981; Berner, 2004). The ocean's alkalinity adjusts within
~10–100 kyr, so on the 1 Myr step the **inorganic carbonate burial equals the Ca + Mg delivered
by silicate weathering**. No organic burial, no pelagic biogenic carbonate (barren). Carbonate
weathering and re-precipitation are neutral and left out. Mg partly goes into clays/hydrothermal
exchange in reality; here Ca and Mg both count (upper bound).

### 3a. Silicate weathering on land, per land cell

`w = w_sup (1 − exp(−w_kin / w_sup))` (mol/m²/yr): kinetic where fresh rock is plentiful,
supply-limited where it is not (West, 2012; Gabet & Mudd, 2009; Maher & Chamberlain, 2014 —
the MAC idea that weathering saturates with supply; *mem.*).

- **Supply** `w_sup = (E_bed ρ_bed χ + f_sed E_sed ρ_sed χ_felsic)`: bedrock eroded on the cell
  in the step (`ErosionResult::bedrock_delta`, m, per year), plus eroded sediment that is still
  partly weatherable (`f_sed` = 0.3: reworked, partly weathered cover; *assumption*). `χ` is
  the Ca + Mg content: 1.3 mol/kg for felsic crust (upper crust 3.6 wt% CaO, 2.5 wt% MgO;
  Rudnick & Gao, 2003, *mem.*), 3.7 mol/kg for basalt (11% CaO, 7% MgO), interpolated by the
  bedrock density between 2750 and 3000 kg/m³. Depositional cells have no supply: their
  sediment was weathered where it was eroded.
- **Kinetics** `w_kin = φ k_land L(ρ) b q^0.65 exp(Ea/R (1/T₀ − 1/T)) g(r) (1 − ice)`:
  - runoff `q` (m/yr) from the water cycle, exponent 0.65 (GEOCARB; Berner, 1994; *mem.*;
    kinetic weathering grows sub-linearly with runoff, dilution);
  - Arrhenius with the annual mean surface temperature of the cell, **Ea = 50 kJ/mol**
    (range 40–75: Brady, 1991; West et al., 2005: 74; Li et al., 2016, basalts: 42; Maher &
    Chamberlain, 2014; *mem.*), T₀ = 288 K. At 288 K that is +7%/K; Walker et al. (1981)
    used exp(ΔT/13.7 K) = 7.3%/K (*mem.*), GEOCARB 9%/K;
  - **CO₂**: abiotic `g = r^0.5` (GEOCARB's pre-plant soil, where soil CO₂ is atmospheric;
    Berner, 1994; *mem.*; WHAK used r^0.3); with plants (`--biosphere modern`) GEOCARB's
    `(2r/(1+r))^0.4`, which saturates because roots already raise soil CO₂;
  - **plants**: `b` = 1 with plants, **0.25 barren** (pre-vascular weathering 2–10× slower;
    GEOCARB's f_E = 0.25; Moulton & Berner, 1998; *mem.*);
  - lithology `L` = 1 for felsic, 5 for basalt, interpolated by density (basalts weather 5–10×
    faster: Dessert et al., 2003; also the #106 design note);
  - permanent ice (`TemperatureFields::ice`) shuts it off; the frozen months have no runoff in
    the water cycle anyway.
  - `k_land` = 0.073 mol m⁻² yr⁻¹ (m/yr)^−0.65: modern Earth, plants, 15 °C, 280 ppm, mean
    runoff 0.3 m/yr on 1.5 × 10¹⁴ m² gives ~5 Tmol/yr Ca + Mg from silicates (Gaillardet et al.,
    1999: 11.7 Tmol CO₂/yr consumed by silicate weathering, half of it long-term; *mem.*).

### 3b. Seafloor weathering (low-temperature alteration of young oceanic crust)

Justified for a barren high-CO₂ planet: it is the main sink in Archean carbon-cycle models
(Sleep & Zahnle, 2001; Krissansen-Totton et al., 2018; *mem.*), needs no land, and is tied to
ridge production, which the model has. Carbon taken up per kg of new crust:

`F_sf = M_ridge c`, `c = c_max (1 − exp(−c_kin/c_max))`,
`c_kin = φ c₀ r^0.23 exp(E_sf/R (1/T_d0 − 1/T_d))`:

- c₀ = 0.023 mol/kg: ~1.5 Tmol/yr on today's 65 Gt/yr of crust (Alt & Teagle, 1999: 1.5–2.4;
  Krissansen-Totton & Catling, 2017, smaller; *mem.*);
- r^0.23 (Brady & Gíslason, 1997, as used by Krissansen-Totton & Catling, 2017; *checked
  through a secondary source*);
- E_sf = 92 kJ/mol (Coogan & Dosso, 2015; range 40–110; *secondary source*), T_d the bottom
  water temperature: the annual mean surface temperature of the ocean poleward of 55° (where
  deep water forms), not below −1.8 °C; T_d0 = 2 °C;
- c_max = 0.26 mol/kg: the upper ~500 m (7%) of the crust fully carbonated (3.7 mol/kg Ca + Mg).

At epoch 0 (r ≈ 1770) r^0.23 ≈ 5.6: seafloor weathering then takes a large share of the sink,
as in the Archean models.

## 4. Calibration at the start (φ) and steady state

The literature laws (§3) at the start's climate and CO₂ need not balance the tectonic sources:
the start's forcing is prescribed, and `k_land`, `b`, `χ` are uncertain by factors of several.
To keep the start **continuous** (the brief), one factor **φ** scales both kinetic constants
(not the supply limits) so that the sinks equal the sources in the first step:
`W(φ; r₀, climate₀) = D₁`. φ is logged: φ ≈ 1 means the literature laws and the prescribed
start forcing agree; φ ≫ 1 that the start would need faster weathering than the laws give, so
the carbon cycle on its own would settle at higher CO₂. If even an unlimited φ cannot balance
the sources (all supply-limited), φ is capped (10⁴) and the imbalance logged.

**Decision for Sebastian:** calibrated start (default, continuous) or φ = 1 (the laws as they
are; the planet then finds its own CO₂ at the start, which can jump). `--set carbon.phi=1`
gives the latter.

After the start φ stays fixed; the CO₂ then responds to the brightening sun (equilibrium:
the weathering stays ≈ the degassing, so the temperature stays roughly constant; it rises by
the part that `g(r)` and the seafloor's r^0.23 absorb), and to the tectonics (degassing up →
CO₂ up; more fresh rock, more land in warm wet latitudes, basalt → CO₂ down).

## 5. Coupling and numerical stability

Order in `Planet::step`: tectonics → temperature (with the current forcing) → water cycle →
erosion → **carbon cycle** (needs the step's climate, runoff and erosion) → new forcing for the
next step's temperature.

**Implicit (backward Euler) solve per step** for `x = ln r` at the step's end:
`N(x) + dt W(x) = N_old + dt D`. Both terms grow monotonically with x, so there is one root;
bisection on x (robust; ~60 iterations over ~12 000 land cells, < 20 ms). Backward Euler is
unconditionally stable for this decaying mode and gives the equilibrium when τ_fb ≪ dt.

The climate is not re-run inside the solve. The weathering sees the step's temperatures shifted
by a **predicted** change `ΔT = λ (F(x) − F_now + ΔS_abs)`, where `ΔS_abs` is the extra absorbed
sunlight of the next step, and the runoff scaled by `exp(ε ΔT)` (ε = 0.03/K, the hydrological
sensitivity, *mem.*). The deep water shifts by the same ΔT.

Why this is stable: in the fast regime the solve sets the forcing so that the *predicted*
temperature balances the weathering; the next climate update gives the *actual* response
`λ_true`. The temperature error then shrinks per step by `(1 − λ_true/λ)` — stable for
`λ > λ_true / 2`, monotone (no oscillation) for `λ ≥ λ_true`. So λ is taken from the model's
own sensitivity and rounded **up** (measured with ±4 W/m² runs before implementing; the EBM's
no-feedback value is 1/B = 0.48 K per W/m²; ice-albedo raises it). In the slow regime (epoch 0,
τ_fb ≈ 15 dt) the reservoir damps further. Near a snowball the true sensitivity can exceed 2λ;
the runs check for oscillation (sign changes of ΔF between steps) and the step-to-step
temperature noise.

`--no-carbon-cycle` (the switch) keeps the forcing fixed as before (bit-identical runs) but
still computes and logs the fluxes at the fixed CO₂, so the imbalance that the fixed forcing
implies can be compared.

## 6. Code

- `src/carbon.rs`: `CarbonParams` (all constants above, `set` for `--set carbon.NAME=VALUE`),
  `CarbonCycle` (state `N`, φ, previous cumulative tectonic rates, last fluxes), the forcing
  law and its inverse (or in `presets.rs`, used by both), the solve. Transcendental math via
  `crate::math` (`ln`, `exp`, `powf`, `cbrt`/`powf(1/3)`, `sqrt`), no hash maps.
- `Simulation::carbon_drivers()`: the cumulative ledger terms, metamorphosed and stacked
  sediment, Σ plume flux (cheap, no full statistics).
- `Planet`: owns `Option<CarbonCycle>`-like state; after erosion calls it, sets
  `temperature.params_mut().greenhouse_forcing`. `PlanetStatistics` and the snapshot log get
  pCO₂, forcing, fluxes.
- CLI: `--no-carbon-cycle` (a flag like `--no-erosion`, so the panel and the URL get it), in
  the panel's main options; `--set carbon.…`. Docs: CLI help, `presets`/`sun` module docs
  ("no carbon cycle" sentences), README.
- Log: one `info!` line per step (time, pCO₂ bar and × PAL, forcing, each source, each sink,
  predicted ΔT), and the snapshot line.
- Viewer: no global climate readout exists; a CO₂ line in the info panel is optional and only
  if cheap.
- Tests: forcing law/inverse round trip and continuity at r = 1; partitioning monotone; the
  solve conserves carbon (`N_new − N_old = dt (D − W)`); equilibrium when τ ≪ dt; φ calibration
  balances the first step; off switch keeps the state hash; stability of a toy linear climate
  with λ_true = 1.5 λ_pred.

## 7. Evaluation

8 seeds × 1000 Myr through `runs`, one commit:

1. **on**: defaults (barren, epoch 0);
2. **off**: `--no-carbon-cycle`;
3. **modern on**: `--biosphere modern --epoch 4`.

(and if cheap, modern off from #190's "old" runs for reference). Reported: global mean
temperature trajectory (start, 200–1000 Myr mean, end, drift per Gyr), pCO₂ trajectory and
range, permanent ice, sources and sinks over time and their balance, the response to tectonic
events (correlate CO₂/T with ridge production, land area, largest continent share —
supercontinent assembly/breakup), snowball/runaway risk (min/max T, max ice, oscillation
check), runtime against off. An independent reviewer evaluates the results.

## 8. Changes after the review ([`review-plan.md`](review-plan.md))

| finding | change |
|---|---|
| 1 (blocker) φ on the first step: no sea floor opens in the first ~2 Myr, erosion still relaxing | **Calibration window**: the CO₂ stays that of the start for `carbon.calibration_time` = 20 Myr; φ then balances the sinks against the sources summed over the window (at the start's CO₂, each step's climate). Confirmed in a test run: step 1 had zero degassing. |
| 2 lag of the energy balance model (2 years per update, ~3 yr relaxation) | The temperature model now reports the **top-of-atmosphere imbalance** of its last year (`TemperatureFields::toa_imbalance`, a diagnostic, no state change). The predicted change is that to the *equilibrium*: `λ (ΔF + ΔS + N_toa)`. With it a lagged toy climate converges monotonically for λ_pred from 0.75 to 1.7 × λ_true (unit test `lagged_climate_does_not_oscillate`). λ_pred = 0.8 K per W/m²; measured λ_true = 0.58 (epoch 0) and 0.72 (`--biosphere modern --epoch 4`) from ±4 W/m² runs. |
| 3 φ ≪ 1 expected, land supply-limited | Kept the calibration (continuous start), φ range capped to 10^±3 with a warning; φ is logged and reported per seed. The φ = 1 alternative is a decision for Sebastian. |
| 4 slab carbon fixed | Arc degassing = mantle wedge (0.015 mol/kg of subducted crust) + **half of the seafloor uptake** carried by the subducting crust (`slab_return` 0.5 of the uptake averaged over `slab_age` 100 Myr) + subducted sediment (0.15 mol/kg). The calibration accounts for the returning half. |
| 5 expectation and β | Expect partial compensation, not a constant temperature. Barren CO₂ exponent **0.3** (WHAK; GEOCARB's 0.5 overstates it at r ~ 10³). |
| 6 `--no-erosion` | No erosion → no supply → no land weathering; φ balances with the seafloor alone. Documented, not a default case. |
| 8 grid noise of the degassing | Sources smoothed over 3 Myr (`degassing_smoothing`; the budget uses the smoothed fluxes). |
| 9 lithology by density | Mafic share = 1 − the crust's felsic fraction. |
| 12 bit identity | The off switch keeps the start's forcing exactly: `--no-carbon-cycle` gives the same state hashes as the base commit (checked, 4 snapshots at 4 subdivisions). `--co2` > 1 changes its forcing (new law). |
| 13 | `bedrock_delta` is per step (divided by the step's years); ledger counters are cumulative; stacked sediment and burial metamorphism are separate counters (`TrenchSediment::stacked` becomes metasediment directly). |
| 14 seafloor c₀ | 1.0 Tmol/yr today (0.015 mol/kg) between Alt & Teagle and Krissansen-Totton & Catling. |
| 7, 10, 11 | Not changed (minor; p^{1/3} vs p^{1/2} changes only the reservoir, the bottom water cannot be colder than freezing, ε stays 0.03/K). |
| 15 evaluation | Added: per-step budget check, φ per seed, lagged correlations with ridge production and land, step-to-step oscillation count. |
