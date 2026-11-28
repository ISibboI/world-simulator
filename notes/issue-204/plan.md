# #204: abiotic cloud nuclei and cloud brightness — plan

## What the model has now

The energy balance model (`src/temperature.rs`) has no clouds of its own. Clouds are folded
into fixed **planetary (top-of-atmosphere) albedos**, calibrated on today's Earth:

- open ocean `ocean_albedo + ocean_albedo_latitude · P₂(sin φ)` = 0.31 + 0.18 P₂ (per coarse cell,
  by latitude), also used for open lake water;
- snow-free land `land_albedo` 0.32 (modern) / 0.37 (abiotic preset: bare rock, +0.05);
- sea ice 0.55 / melting 0.45, ice sheets 0.62.

The clouds of the realistic view (#183) are rendering only. So today's marine and continental
cloud brightness, which includes biogenic cloud condensation nuclei (CCN: DMS-derived sulfate,
marine organics, terpene SOA, wildfire smoke), is applied unchanged to the barren default planet.

## Model (a correction of the calibrated albedos, Twomey only)

New module `src/aerosol.rs`; parameters `AerosolParams` nested in `TemperatureParams`
(`temperature.aerosol.*`, so the presets and `--set` reach them without new wiring).

1. **Sources relative to pre-industrial Earth**, `R_k` (1 = Earth):
   - sea salt: number flux ∝ U¹⁰^3.41 (Monahan et al., 1986) with the model's zonal-mean wind
     speed (`zonal_mean_wind` plus the 4.5 m/s of weather systems, as the wind stress uses):
     a **zonal** profile, normalised to its global mean. The model's winds are prescribed, so
     salt is the same on every planet; it matters through its latitude structure and as the
     floor that remains without life. Short lifetime (~1 day), so local.
   - DMS + marine organics `R_mb` and land biogenic (terpene SOA, biogenic nucleation) + wildfire
     `R_lb`: parameters, 1 (modern), 0 (abiotic preset).
   - background volcanic sulfate `R_v`: the carbon cycle's arc + plume mantle degassing
     (`CarbonState::arc_plume_mantle_degassing`, mol C/yr, previous step) over Earth's value in
     the same coefficients (wedge 0.015 mol/kg × 65 Gt/yr + plumes 1 Tmol/yr ≈ 1.98 Tmol/yr),
     i.e. a fixed S/C ratio of subaerial degassing. Ridges degas into the ocean, not the air.
     1 before the first carbon step.
   - mineral dust `R_d`: emitting land area over Earth's. Per land fine cell (not lake, not ice
     sheet) the Köppen dryness `d = P / P_B`, `P_B = 10 (2 T + 14)` mm (the B threshold;
     T annual mean °C, P annual precipitation of the previous step; `P_B ≤ 0` → frozen, no dust).
     Emission weight `E = s(d) · b(d)` with soil dryness `s = clamp((2 − d)/1.5)` (full in
     deserts `d < 0.5`, none where it is twice as wet as the arid limit; wet soils do not emit,
     Fécan et al., 1999) and the bare fraction `b`: 1 on a barren planet; with vegetation
     `clamp((1 − d)/0.5)` (bare in deserts, closing across the steppe). Times `(1 − ice)`.
     Reference: Earth's group B covers 30.2% of land (Peel et al., 2007), about 19% BW and 11% BS
     → 0.19 + 0.11 · 0.40 = 0.234 of land, × 0.29 land = **0.068 of the planet**.
     `R_d = Σ E / N_cells / 0.068`. Global (dust lives ~4–7 days and travels across oceans).
2. **CCN per air mass** (cm⁻³, at ~0.2–0.3% supersaturation), Earth pre-industrial shares,
   `N_a = Σ_k N_k R_k`:

   | source | marine | continental |
   |---|---|---|
   | sea salt (zonal) | 20 | 15 |
   | DMS / marine organics | 35 | 25 |
   | volcanic sulfate | 30 | 60 |
   | land biogenic SOA + fire | 12 | 170 |
   | dust | 3 | 30 |
   | **sum** | **100** | **300** |

   Orientation: remote marine CCN 50–150, sea spray typically < 30% of marine CCN (Quinn &
   Bates, 2011; Quinn et al., 2017); volcanic degassing +40% pre-industrial N_d (Schmidt et al.,
   2012); DMS the largest natural CCN source over the ocean (Rap et al., 2013; Fiddes et al.,
   2018); continental pre-industrial CCN dominated by biogenic SOA/nucleation and fires (Gordon
   et al., 2016; Rap et al., 2013); dust a minor CCN number source (but giant CCN).
3. **Droplets**: `N_d = 375 (1 − exp(−2.5e−3 N_a))` (Jones et al., 1994). Earth: 83 (marine),
   198 (continental) cm⁻³.
4. **Twomey**: liquid cloud optical depth at fixed liquid water path `τ = τ₀ (N_d/N_d,ref)^{1/3}`,
   τ₀ = 10; cloud albedo (two-stream, conservative scattering, g = 0.85; Lacis & Hansen, 1974)
   `α_c = τ/(τ + 13.3)` (0.43 at τ₀); the cloud over a surface of albedo a_s reflects
   `A = α_c + (1 − α_c)² a_s / (1 − α_c a_s)`; the planetary albedo changes by
   `ΔA_p = f · T² · (A(N_d) − A(N_d,ref))` with the low liquid cloud fraction seen from space
   f = 0.30 (ocean), 0.15 (land), two-way transmission above the clouds T² = 0.8,
   a_s = 0.07 (water), 0.17 (vegetated land) / 0.30 (bare land, preset).
   No adjustments of liquid water path or cloud fraction (Albrecht, ERFaci): uncertain in sign
   and size, so the effect is the cloud albedo effect only (stated as a caveat).
5. **Direct effects** as a uniform albedo change on open ocean and snow-free land,
   `ΔA = −Σ_k F_k (R_k − 1)/340`: DMS sulfate −0.23, volcanic −0.21 W/m² (Rap et al., 2013),
   dust −0.2 W/m² net (Kok et al., 2017). Small (≲ 0.002).
6. **Applied** per coarse cell: open ocean by latitude (marine N_d of the latitude), snow-free
   land and open lake water with the continental N_d (lakes with a_s of water). Not over sea
   ice or ice sheets (bright surface, little Twomey effect). Calibration: none beyond the Earth
   reference: modern with R = 1 everywhere gives ΔA = 0 exactly; modern runs change only by
   their own R_v, R_d ≠ 1.

**Check of the susceptibility against GCMs (not tuned):** removing volcanic sulfate gives
~1.3 W/m² (Schmidt et al. 2012, PI: 1.06), removing DMS ~1.6 W/m² (Rap 2013 PD Twomey 0.76,
Fiddes 2018 PD all effects 1.7; PI larger than PD). Expected barren effect: marine N_d 83 → ~50
(r_e +18%), continental 198 → ~130, ~2.5–3 W/m² less reflected sunlight, ~1 K before the carbon
cycle answers; Goldblatt & Zahnle (2011) and later work: a few to ~10 W/m² for the Archean.

**Resolution:** sources global (dust, sulfate, biogenic) plus zonal sea salt, two air masses
(marine, continental). The coarse temperature grid could carry more, but the source data
(degassing, dust) are global, and aerosol transport is not modelled.

## Code

- `src/aerosol.rs`: params (with `set`), `Sources { volcanic, dust }`, `CloudNuclei` (N_a,
  N_d, ΔA per air mass and latitude), dust index from fine-mesh fields, unit tests (Earth → 0,
  barren → darker, monotone in N, saturation, salt profile normalised).
- `TemperatureModel::update` gets the sources (new argument or setter); ocean/lake/land albedos
  add the deltas; new fields: Bond albedo of the last year (insolation-weighted, global), and an
  aerosol diagnostic (R_v, R_d, N_a/N_d marine & continental, α_c ocean/land, mean ΔA ocean/land).
- `Planet`: computes the sources before the temperature update (previous step's precipitation,
  temperatures, ice, carbon state); `presets`: abiotic sets `R_mb = R_lb = 0`, land a_s 0.30.
- Log: the temperature line gets the planetary albedo; a new `aerosol:` line.
- `temperature.aerosol.enabled = 0` turns it off (gives main's albedos).

## Evaluation

Runs against main (e55bf1d): default barren 8 seeds × 1000 Myr; `--biosphere modern --epoch 4`
4 seeds × 1000 Myr. Report planetary albedo, cloud albedo ocean/land, global T, CO₂, φ (clamp
0.3–3), ice, runtime. Results to `results.md`, with an independent evaluation.
