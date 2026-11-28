# #204: abiotic cloud nuclei — results

Model: [plan.md](plan.md) (with the review's changes, see below); code in `src/aerosol.rs`.
Runs of commit 33f4837 against main (e55bf1d), 1000 Myr, means over 200–1000 Myr unless
stated; per-seed numbers in [analysis.txt](analysis.txt) (made by [analyse.py](analyse.py)).

- **off** = this branch with `--set temperature.aerosol.enabled=0`: bit-identical to main
  (same state hashes and statistics; checked on all 12 seeds), and it logs the planetary albedo
  and the sources that main does not.
- **new** = this branch (defaults). **static** = `temperature.aerosol.dynamic_sources=0`
  (volcanic and dust sources held at Earth's, only the missing biosphere acts).

## Barren default (epoch 0, abiotic), 8 seeds

| | off (= main) | new | static |
|---|---|---|---|
| planetary albedo | 0.3159 | **0.3069** | 0.3078 |
| Δ albedo open ocean / snow-free land | 0 / 0 | −0.0101 / −0.0043 | −0.0117 / −0.0062 |
| marine CCN / droplets (cm⁻³) | (100 / 83 Earth) | 57 / 49 | 53 / 46 |
| continental CCN / droplets (cm⁻³) | (300 / 198 Earth) | 134 / 107 | 105 / 87 |
| low-cloud albedo ocean / land | (0.509 Earth) | 0.465 / 0.458 | 0.460 / 0.441 |
| sources volcanic / dust (× Earth) | (1.03 / 2.02, not applied) | 1.03 / 1.92 | 1 / 1 |
| global mean T (°C) | 17.03 | **18.06** | 18.13 |
| equator / poles (°C) | 25.5 / −2.7 | 26.7 / −2.0 | 26.6 / −1.6 |
| permanently frozen | 1.15% | 0.76% | 0.61% |
| pCO₂ (bar) | 0.475 | 0.466 | 0.490 |
| φ (8 seeds) | 0.92–1.40, mean 1.18 | 0.83–1.22, mean 1.02 | 0.82–1.19, mean 1.01 |
| first 50 Myr: T / ice | 13.1 °C / 4.5% | 14.4 °C / 3.1% | 14.4 °C / 3.1% |
| runtime (s, GitHub runners) | 854 (main: 883) | 926 | 972 |

- The albedo falls by 0.009, i.e. **~2.3 W/m² more absorbed sunlight** (mean insolation
  ~255–275 W/m² under the young sun), almost all over the ocean: marine droplets 83 → 49 cm⁻³,
  effective radius ×1.19 (12 → 14.3 µm). Within the few to ~10 W/m² that Goldblatt & Zahnle
  (2011) and later Archean studies give, at its low end (Twomey effect only).
- **The planet warms by ~1.0 K**, with ~1/3 less permanent ice. No φ hits its clamp.
- **The CO₂ barely changes (−2%)**; φ drops by ~15% instead. The carbon cycle holds the CO₂ of
  the start through the ramp-up and calibrates φ there, so that the sinks balance the sources at
  that CO₂ in the ramp-up's climate. The darker clouds are already active then, so the warmer
  climate is built into φ (faster weathering per unit φ) and the long-term CO₂ stays where the
  epoch's start forcing put it. The effect therefore shows as temperature, not as less CO₂. With
  the greenhouse forcing of an epoch read as a prediction rather than a prescribed start, the
  carbon cycle would instead have drawn the CO₂ down by roughly 2.3 W/m² ≈ 0.25 e-folds at this
  forcing level; that is a property of the calibration (#154/#213), not of this change.
- **Dynamic vs static sources:** nearly the same mean (18.06 vs 18.13 °C). The bare land emits
  ~1.9× Earth's dust (more in the drier seeds, 1.4–2.3×), which adds a few continental CCN and a
  direct cooling of ~−0.2 W/m²; the volcanic source varies ±30% between seeds and in time
  (0.7–1.5), ±0.003 in ocean albedo. Neither changes the result much; the missing biogenic
  nuclei do.

## Today's Earth (`--biosphere modern --epoch 4`), 4 seeds

| | off (= main) | new |
|---|---|---|
| planetary albedo | 0.3048 | 0.3037 |
| Δ albedo ocean / land | 0 / 0 | −0.00003 / −0.00003 (mean); per update within ±0.003 |
| droplets marine / continental | (83 / 198) | 83 / 198 |
| global mean T (°C) | 22.17 | 22.47 (seeds −0.2…+1.0 K) |
| permanently frozen | 3.5% | 3.2% |
| pCO₂ (bar) | 0.00051 | 0.00053 |
| φ | 3.0 (clamped, all seeds) | 3.0 (clamped, all seeds) |
| first 50 Myr: T | 14.52 °C | 14.52 °C |
| runtime (s) | 975 (main: 905) | 1030 |

- **Essentially unchanged**: the first 50 Myr agree to 0.01 K; later the runs diverge chaotically
  (small albedo changes perturb the climate, the carbon cycle and through erosion the tectonics),
  and the seed means differ by 0.3 K with a spread of −0.2…+1.0 K per seed, with no systematic
  albedo change (mean Δ ≈ 0). The modern dust source comes out at 0.9–1.3× Earth's (dust
  reference from Köppen group B; the model's modern B share is close to Earth's), which confirms
  the reference.
- **φ = 3 (clamped) in every modern run, also on main**: a pre-existing issue of the modern
  preset since the ramp-up (#213), not caused by this change. The modern runs then warm to
  ~22 °C because the sinks cannot balance the sources at today's CO₂. Worth its own issue.

## Changes after the plan review

1. Volcanic reference: the plan divided by Earth's fixed degassing. The quick checks showed
   the model's subaerial mantle degassing at ~1.3–1.5× Earth's (its tectonics run ~1.7× Earth's
   spreading), which would brighten every run's clouds, today's Earth too (+0.002 ocean albedo).
   Now the reference is the planet's own degassing when the ramp-up ends (as φ), so the
   volcanic source follows the changes after it; `temperature.aerosol.volcanic_reference`
   (mol C/yr) fixes it instead. Smoothed over 20 Myr.
2. Sources at the first update (no carbon step, no water cycle yet): Earth's.
3. `dynamic_sources = 0` switch and the static run above.
4. Dust reference corrected to (0.19 + 0.11 · 0.5) · 0.29 = 0.071 of the planet.
5. Cloud albedo in the √3 two-stream form (Lacis & Hansen, 1974), `τ/(τ + 7.7)`, τ₀ = 8
   (α_c 0.51).
6. Dust direct effect kept at Kok et al.'s central net −0.2 W/m² (sign uncertain, range −0.5
   to +0.2): the issue asks for it, and it is small (−0.0006 albedo at 2× dust).
7. Sea ice: the change is part of the open-water albedo, so partial ice weights it by the open
   share.

## Caveats

- Cloud albedo effect only: no adjustments of liquid water path or cloud cover (their sign and
  size are uncertain; including them would probably enlarge the effect).
- Global sources (one marine and one continental air mass) plus zonal sea salt; no aerosol
  transport, no seasons. The winds are prescribed, so sea salt is the same on every planet.
- Jones et al. (1994) was fitted to accumulation-mode aerosol, not CCN; its slope at marine
  numbers (d ln N_d / d ln N_a ≈ 0.9) is at the high end of observations.
- The calibrated albedos are today's, so they contain the anthropogenic aerosol (~0.5–1 W/m²);
  the reference is pre-industrial.
- Volcanic sulfate pulses (LIPs, #106/#221) are not included; `Sources` can take them as an
  extra term once the volcanism record is merged.
- The Earth shares of the CCN sources are literature estimates with wide ranges (plan.md).
