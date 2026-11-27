# #210 plan: atmospheric escape of hydrogen and water

Stacked on #196 (carbon cycle, `agent/issue-154`). Goal: the planet loses water to space by
hydrogen escape, slowly through a cold trap in a temperate climate, fast once the stratosphere
gets wet in a hot climate (moist greenhouse), limited by the sun's XUV. The oxygen of the
escaped water stays behind as an abiotic O₂ source. A global budget, like the carbon cycle.

## Physics

### 1. Water at the homopause: the cold trap

Water reaches the upper atmosphere only as the vapour that passes the coldest point of the
troposphere (the tropical tropopause). Following Kasting (1988) and Kasting et al. (1993), the
stratospheric mixing ratio is the saturation mixing ratio where the moist adiabat of the warm
surface meets the stratosphere:

- start at the **tropical surface temperature** `T_s` (annual mean of the ocean cells within
  ±20°; all tropical cells if none are ocean) with the non-condensing partial pressure
  `p_a = p_bg + p_CO₂ + p_O₂` (background 1 bar of N₂/Ar, the carbon cycle's CO₂, the O₂ that
  escape left in the air) and the vapour `e_s(T_s)`;
- integrate the saturated (pseudo)adiabat of a non-dilute condensable upwards (Pierrehumbert,
  2010, eq. 2.33):
  `d ln p_a / d ln T = (c_pa/R_a) [1 + (c_pv/c_pa + (L/(R_v T) − 1) L/(c_pa T)) r] / [1 + L r/(R_a T)]`,
  `r = (R_a/R_v) e_s(T)/p_a`, with Clausius–Clapeyron for `e_s` (over water above 273.16 K,
  over ice below; constant `L_v = 2.5 MJ/kg`, `L_s = 2.834 MJ/kg`), RK2 in `ln T`, 200 steps;
- until the **cold-point temperature** `T_cp` = 190 K (today's tropical cold point; it gives
  today's ~3–4 ppm). The stratospheric mixing ratio is `f = e_s(T_cp) / (p_a(T_cp) + e_s(T_cp))`.

Prototype (`p_a` = 1 bar): `T_s` = 15 °C → 2 ppm, 27 °C → 3.3 ppm (today: ~4 ppm entering the
stratosphere), 50 °C → 2.3e-5, 60 °C → 1.2e-4, 70 °C → 1.3e-3, 80 °C → 2.3e-2,
100 °C → 0.25. With `T_cp` = 200 K (Kasting's choice) `f` = 3e-3 is reached at ~67 °C, the
classic moist-greenhouse limit at ~340 K; with 190 K at ~73 °C.

`T_cp` is kept fixed, not scaled with the sun: non-grey models find the stratosphere of a
warming planet cooling, not warming, until water itself heats it (Kasting et al., 2015; Leconte
et al., 2013 3-D: 140 → <120 K), and 3-D models find the moist greenhouse only near 350 K
(Wolf & Toon, 2015). A fixed 190 K is between Kasting's 200 K (wetter) and the cold non-grey
stratospheres (drier). Higher CO₂ (the 0.5 bar of epoch 0) dilutes the vapour through `p_a`;
its warming of the stratosphere is not modelled.

### 2. Diffusion limit

Hydrogen in every form crosses the homopause by molecular diffusion at most at the limiting flux
(Hunten, 1973; Walker, 1977; Catling & Kasting, 2017):

`Φ_DL(H₂) = (b/H_a) · f_T/(1 + f_T)`, `b/H_a ≈ 2.5e13 cm⁻² s⁻¹`, `f_T = f_H₂O` (H₂-equivalent;
no H₂ or CH₄ yet, #202 can add them); in H atoms `Φ_DL(H) = 2 Φ_DL(H₂)`.

Check: today's `f_T(H₂)` ≈ 7 ppm (H₂O + 2 CH₄ + H₂) gives 3.5e8 H cm⁻² s⁻¹, observed ~3e8.
For a barren planet with 2–4 ppm: ~1.5e8 → 6e8 kg of water per year, ~1 m of global sea level
per Gyr: negligible, as expected for a planet with a cold trap.

`b/H_a` is that of H₂ in N₂ air and kept constant (a CO₂-rich homopause has a smaller `b` and a
smaller scale height; within a factor ~2).

### 3. Energy limit (XUV)

The escaping gas must be lifted out of the potential well with the absorbed XUV energy
(Watson et al., 1981):

`Φ_EL(H) = ε F_XUV / (4 g R m_H)` per unit surface area (from `Ṁ = ε π R³ F_XUV / (G M)`,
`R_XUV ≈ R`, no tidal correction).

- `F_XUV` at 1 AU: Ribas et al. (2005), 1–1180 Å, `29.7 τ^−1.23 erg cm⁻² s⁻¹` (τ = age of the
  sun in Gyr; saturated before 0.1 Gyr): 4.6 erg cm⁻² s⁻¹ today (4.6 mW/m²), 59 at epoch 0
  (τ = 0.57, 13× today), 2.9 at epoch 6, 1.75 at 10 Gyr. The age comes from
  `sun::age_of_run`, so it follows the epoch and the run. The fit is extrapolated beyond today.
- `ε` = 0.15 (heating efficiency, 0.1–0.3; Watson et al., 1981; Lammer et al., 2009).
- Today: `Φ_EL` ≈ 1.6e11 H cm⁻² s⁻¹ → one ocean (1.4e21 kg, 1.8e28 H cm⁻²) in ~3.5 Gyr.

The escape is `Φ(H) = min(Φ_DL, Φ_EL)`. With a cold trap it is diffusion-limited by 3 orders of
magnitude even at epoch 0, so the strong early XUV does not matter for a temperate barren
planet without H₂ (as the issue suspected it might): the XUV only limits the escape once the
stratosphere is wet (or when #202 adds volcanic H₂/CH₄). The crossover is at `f ≈ 3e-3` today:
exactly the classical moist-greenhouse "water-loss limit".

### 4. Water, sea level, oxygen

Per step of `Δt` years and planet area `A`:

- water lost: `ΔM = Φ(H) A Δt m_H₂O / 2` (kg), taken from the ocean: `ocean_volume −= ΔM/ρ`
  (ρ = 1025 kg/m³, as `carbon::ocean_mass`). New `Simulation::remove_water(volume)` lowers
  `TectonicParams::ocean_volume` (the planet's water) and solves the sea level again; the change
  goes into a new sea-level budget term `escape`. At most down to a floor of 1% of the initial
  ocean (the climate model needs some ocean; a warning when reached).
- oxygen left behind: `Φ(H)/4` O₂. No oxygen escapes with the hydrogen (no drag of O: the
  crossover mass at these fluxes stays below 16 amu; Hunten et al., 1987): an upper bound.
- O₂ budget (#201 placeholder): `dN_O₂/dt = source − sink`, the sink a constant capacity of
  reduced volcanic gases and crust oxidation, `oxygen_sink` = 1 Tmol O₂/yr (replaced by the
  physical sinks of [o2-sinks.md](o2-sinks.md) since `e25e269`) (Earth's reductant
  flux is of order 1–5 Tmol O₂-eq/yr; Catling & Kasting, 2017), never more than the O₂ there.
  The accumulated O₂ is reported (mol, bar) and adds to `p_a` of the cold trap (it dilutes the
  vapour). With a cold trap the source (~0.01 Tmol/yr) is far below the sink, so a barren
  temperate planet keeps no O₂, as photochemical models find.
- Surface pressure: hydrogen's mass is negligible; the model has no N₂ budget. The only
  pressures that change are CO₂ (carbon cycle) and the O₂ of the escape.

### 5. Coupling with the carbon cycle (#196)

- The carbon cycle takes the ocean's mass from `Simulation::ocean_volume()` every step, so the
  exogenic carbon automatically repartitions into a smaller ocean (more CO₂ in the air at the
  same carbon): the budget stays closed.
- Weathering follows the climate's runoff; a shrinking ocean means less evaporating surface and
  less rain, and with the ocean gone weathering would stop. At the rates below a 1000 Myr run
  does not get there.
- The CO₂ partial pressure goes into `p_a` of the cold trap.

### 6. Minor processes (left out, documented)

Ion pickup / sputtering by the solar wind (a magnetised Earth-sized planet: ~10²⁵–10²⁶ O⁺/s,
~0.1–1 m of ocean per Gyr), impact erosion (after the end of heavy bombardment at epoch 0,
negligible), Jeans escape of heavy gases (λ ≫ 10 for O, N₂, CO₂: nil), drag of O.

## Late epochs: what to expect

The model's surface stays liquid at any temperature (linear OLR, no runaway). Tropical surface
temperatures: epoch 7 ~55 °C → `f` ~ 5e-5, diffusion-limited: an ocean in ~100 Gyr; epoch 9.43
~110 °C → `f` ~ 0.4, energy-limited at 1.75 erg: an ocean in ~9 Gyr. So even a moist greenhouse
loses only ~10% of the ocean per Gyr around the late Sun: the late Sun's XUV is weak. This is
the physical answer for a Sun-like star (energy-limited loss of an ocean takes Gyr); the model's
remaining problem at late epochs is the missing runaway greenhouse (beyond ~1.1–1.2 S₀ the ocean
would boil into a steam atmosphere), which stays a warning. Sensitivity: `ε` 0.1–0.3 and
`R_XUV/R` up to ~1.3 give a factor ~0.7–3.4 on the energy limit.

## Implementation

- `src/escape.rs`: `EscapeParams` (`enabled`, `cold_point_temperature`, `background_pressure`,
  `diffusion_limit`, `xuv_flux`, `xuv_exponent`, `xuv_saturation_age`, `heating_efficiency`,
  `tropics_latitude`, `oxygen_sink`, `min_ocean_fraction`; `set` by name, `escape.` prefix),
  `Escape` (cumulative water, H, O₂ produced/consumed/in the air) and `EscapeState` (last step:
  `T_s`, `f`, `Φ_DL`, `Φ_EL`, `Φ`, regime, loss rate, cumulative). Pure functions for the cold
  trap, the limits and the XUV, unit-tested (today's values, monotonic `f(T_s)`, the min, mass
  conservation of water/H/O).
- `Planet`: after the carbon step in `erode_step`, `update_escape` (tropical temperature, CO₂ from
  the carbon state, sun age) → `tectonics.remove_water`. Skipped entirely when disabled, so
  `--no-escape` is bit-identical to #196 (checked on state hashes).
- `--no-escape` flag (shown in the viewer's main options next to `no-carbon-cycle`),
  `escape.NAME=VALUE` overrides. A snapshot log line with the escape state; the sea-level
  budget gets an `escape` term. The moist-greenhouse warning (#192) is reworded: the escape is
  simulated, the runaway greenhouse is not.
- Math through `crate::math` (`exp`, `ln`, `powf`); no hash maps. Cost: one 200-step adiabat
  and one sea-level solve per step: negligible.

## Evaluation

- default barren: 8 seeds × 1000 Myr (escape on), and the #196 head for comparison (escape off
  = bit-identical, checked by hash on a short run);
- late epoch: `--epoch 7` and `--epoch 9` (the late sun), 4 seeds × 1000 Myr;
- `--biosphere modern --epoch 4`, 4 seeds × 1000 Myr.

Report water lost, sea-level change, O₂ produced, stratospheric `f`, the regime, and the ocean
loss time `M_ocean / ṁ` at late epochs.

## Plan review and changes

An independent agent reviewed this plan. It found the formulas and numbers sound: the moist
adiabat, `2.5e13` per H₂-equivalent with the factor 2 for H atoms (7 ppm → 3.5e8 H cm⁻² s⁻¹),
the energy limit (1.65e11 today, one ocean in ~3.5 Gyr), the Ribas fit, the crossover at 3.3e-3,
and no drag of O (the crossover mass stays at ~3 amu). Its findings and what changed:

1. **The 190 K cold point is the dominant uncertainty.** Today's tropics give 1.5/3.3/6.8/13.7 ppm
   at 185/190/195/200 K. 190 K is the value calibrated to today's ~3–4 ppm, and the late-epoch
   runs get sensitivity runs at 180 and 200 K. Loss times are reported as a range.
2. **`f` is convex in `T_s`, so `f(mean T)` underestimates the escape in hot climates** (×2 at
   70 °C). The cold trap is now tabulated once per step (1 K spacing, interpolated in ln f) and
   averaged over the tropical ocean cells' annual means.
3. **CO₂ dilutes the vapour, so the gas properties matter.** `R_a` and `c_pa` are now weighted by
   the mix of background air, CO₂ and O₂ (molar mass by mole fraction, `c_p` by mass fraction).
4. **Lakes and the floor.** `remove_water` takes at most the free ocean (`ocean_volume()`, not
   the lakes), lowers the planet's water and re-clamps the lakes. The 1% floor applies to the
   free ocean. A test checks that the `escape` sea-level term takes the whole change (zero
   residual).
5. **Epoch 9 for 1000 Myr runs past the sun model (9.43).** The late runs use `--epoch 7` and
   `--epoch 8.4` instead.
6. **Scale of the O₂ sink.** In the energy-limited late regime the escape leaves ~4 Tmol O₂/yr
   against the 1 Tmol/yr placeholder sink, so it gains ~0.08 bar per Gyr. This is reported. The
   consumed O₂ is a cumulative counter, so the O budget closes. #202 must share the reductant
   flux. The modern biosphere's 0.2 bar of O₂ is part of the 1 bar background.
  After a first check (epoch 8.4: 3.8 Tmol/yr net, 0.5 bar in 20 Myr), the placeholder sink is
  3 Tmol/yr: volcanic reductants, oxidation of new sea floor and of weathered Fe²⁺, each of order
  1 Tmol/yr on a tectonically active planet. The O₂ that is left dilutes the vapour at the cold
  trap and slows the loss (Wordsworth & Pierrehumbert, 2014), so the sink matters at late
  epochs.
7. **Dry tropics.** If the tropics have no ocean, all ocean cells are used; with no ocean at
   all, no water reaches the stratosphere.
8. **Minor.** The 3e-3 crossover matching the classical limit is partly a coincidence (with the
   0.5 bar of CO₂ at epoch 0 it is higher). Methane would double today's flux, which matters
   for comparisons with Earth. There is a test that the floor clamps within one step.
