# #210 results: atmospheric escape

Commit `4ad1d29`, 6 subdivisions, 1000 Myr, snapshots every 50 Myr, through `runs` (all on
GitHub runners). Per-snapshot seed means and per-seed end states:
[`runs.txt`](runs.txt) (from [`analyse.py`](analyse.py)).

| set | arguments | seeds |
|---|---|---|
| default (barren, epoch 0) | `--duration 1000` | 8 |
| modern | `--duration 1000 --biosphere modern --epoch 4` | 4 |
| epoch 7 | `--duration 1000 --epoch 7` | 4 |
| epoch 8.4 | `--duration 1000 --epoch 8.4` (ends at 9.4, the end of the sun model) | 4 |
| sensitivity | epoch 7 with `escape.cold_point_temperature=180` / `200`; epoch 8.4 with `escape.oxygen_sink=1e12` | 2 each |
| off | epoch 7 and 8.4 with `--no-escape` | 2 each |

`--no-escape` gives the same state hashes as #196's head (`afc4046`), checked on a 20 Myr run.

## Key numbers (seed means at 1000 Myr)

| | default | modern | epoch 7 | epoch 8.4 |
|---|---|---|---|---|
| global mean T (°C) | 17.4 | 21.0 | 61.0 | 102.4 |
| tropical ocean T (°C) | 29.7 | 36.4 | 83.0 | 127.7 |
| stratospheric H₂O | 2.5e-6 | 5.9e-6 | 9.5e-4 | 1.2e-3 |
| H escape (cm⁻² s⁻¹) | 1.3e8 | 2.9e8 | 4.8e10 | 5.6e10 |
| regime | diffusion | diffusion | diffusion, held down by O₂ | energy-limited for ~120 Myr (start-up transient), then co-limited by diffusion (O₂) and energy |
| water loss now (kg/yr) | 3.1e8 | 7.0e8 | 1.2e11 | 1.4e11 |
| water lost in 1 Gyr (% of the ocean) | 0.019 | 0.044 | 6.1 (5.9–6.3) | 9.9 (9.8–9.9) |
| sea level, equivalent of the lost water (m)¹ | −0.5 | −1.2 | −146 | −223 |
| sea level, on − off (same seeds, m) | | | −70, −221 | −310, −181 |
| sea-level budget term `escape` (m)² | −0.3 | −0.8 | −64 | −84 |
| O₂ produced (mol) | 7.2e18 | 1.7e19 | 2.3e21 | 3.7e21 |
| O₂ left in the air (bar) | 0 | 0 | 0.62 | 4.6 |
| ocean loss time at the end's rate (Gyr) | ~4500 | ~2000 | 11–13 | 9–11 |

¹ The lost volume over the ocean's area (75–85%), times `1 − ρ_w/ρ_m` = 0.69 for the sea
floor's rebound. It matches the on − off difference within the seeds' scatter (the runs diverge
chaotically). ² The budget term undercounts by a factor of 2–3: while the sea level stands at a
pass (the model's `sea level at a pass: deficit`, ~1e14 m³, as large as a step's loss), the
removed water first reduces the deficit, and the level falls only when the next tectonic or
erosion step unpins it, so that part is booked to those terms. The total sea level is right
(checked: the ocean volume on − off at 50 Myr equals the logged loss).

Sensitivity at 1000 Myr (2-seed means; sea level as the equivalent of the lost water):

| | water lost | sea level | O₂ in the air | end rate (kg/yr) |
|---|---|---|---|---|
| epoch 7, cold point 180 K | 3.9% | −93 m | 0.37 bar | 1.2e11 |
| epoch 7, 190 K (default) | 6.1% | −146 m | 0.62 bar | 1.2e11 |
| epoch 7, 200 K | 7.8% | −187 m | 0.82 bar | 1.1e11 |
| epoch 8.4, O₂ sink 1 Tmol/yr | 4.9% | −110 m | 5.4 bar | 8.5e10 |
| epoch 8.4, O₂ sink 3 Tmol/yr (default) | 9.9% | −223 m | 4.6 bar | 1.4e11 |

## What it means

- **A temperate planet keeps its water.** With the cold trap at 2–3 ppm (today's Earth: ~4 ppm
  of water plus methane), hydrogen escapes at the diffusion limit, ~1e8 H cm⁻² s⁻¹. The barren
  default loses 0.02% of its ocean (0.3 m of sea level) per Gyr. The modern planet loses twice as
  much from warmer tropics. All the O₂ (~0.01 Tmol/yr) goes to the sinks, so a barren planet
  has no free O₂, as photochemical models find (#201).
- **The young sun's XUV does not matter for it.** At epoch 0 the XUV is 13 times today's
  (57 mW/m²) and the energy limit 2e12 H cm⁻² s⁻¹, 20 000 times the actual escape. The cold
  trap, not the energy, limits a temperate planet. The early XUV would matter with a wet
  stratosphere or with volcanic H₂ and CH₄ (#202), which the escape is ready to take
  (`f_T` would add them).
- **Moist greenhouse at late epochs.** As the tropical ocean warms past ~60 °C the
  stratosphere gets wet. A run started at epoch 8.4 (tropics 93 °C, no O₂ yet) is first
  energy-limited: 1.7e11 kg/yr, about an ocean in 8 Gyr at the weak late-sun XUV (2 mW/m²,
  0.4 times today's). This phase is a start-up transient: a planet coming from epoch 7 would
  arrive with ~0.6 bar of O₂ already in the air.
- **The oxygen left behind slows the loss.** Losing water faster than the sinks take its
  oxygen (3 Tmol O₂/yr ↔ 1.1e11 kg of water/yr) builds up O₂: 1 bar at epoch 8.4 within
  100 Myr, 4.6 bar at the end, still rising. The O₂ dilutes the vapour at the cold trap and
  holds the stratosphere near f ≈ 1e-3 (Wordsworth & Pierrehumbert, 2014). The loss then runs at
  about the sink plus the O₂ build-up: every late run and every cold point ends at
  1.1–1.4e11 kg/yr, and with a third of the sink (1 Tmol/yr) the end rate is 0.6 times as
  large (0.4 times in mid-run). At epoch 8.4 the energy limit is close (6.3e10 H cm⁻² s⁻¹ at
  the end against 5.6e10 actual; two seeds end energy-limited), so there the sink sets the loss
  within ~25% of the energy limit; at epoch 7 it matters more. Ocean loss times: 9–13 Gyr with
  the 3 Tmol/yr sink, ~8 Gyr at the energy limit. The cold point moves only the onset (epoch 7:
  4–8% lost in 1 Gyr for 180–200 K).
- **Ocean loss times are long because the late sun is faint in XUV.** Around a Sun-like star
  6–9 Gyr old, energy-limited escape removes an ocean in ~8 Gyr (ε = 0.15; ε = 0.3 and an XUV
  radius 1.2 R would give ~3 Gyr). Taking the fresh-start runs together, a rough estimate: an
  Earth-like planet loses ~10–30% of its ocean by escape before the sun leaves the main
  sequence (epoch 9.43). Its oceans end in the **runaway greenhouse** (beyond ~1.1–1.2 S₀ the
  ocean boils into a steam atmosphere), which the model does not simulate (linear OLR: liquid
  oceans at 100 °C). The warning now says this.
- **Coupling.** The carbon cycle sees the smaller ocean (its ocean mass follows
  `ocean_volume()`). At epochs 7–8.4 the CO₂ is already ~0 (forcing at its floor), so this
  changes nothing there. Late-epoch temperature, land share and rain differ between escape on
  and off by about as much as between seeds (chaotic divergence: e.g. epoch 8.4 seed 0 at
  1000 Myr 105 °C / 14% land on, 100 °C / 23% off). The land share's spread at late epochs is
  large in both.
- **Conservation.** The water leaves the ocean: at 50 Myr the ocean volume of epoch 8.4 on − off
  (9e15 m³) equals the logged loss (8.4e15 m³, the rest is the runs' divergence). The unit tests
  check `remove_water` (the sea level falls by the volume over the ocean area times 0.69, the
  lakes keep their water) and the balance of water, H and O to 1e-9: O₂ produced = taken by the
  sinks + in the air. The O₂ taken by the sinks leaves the budget (placeholder: it would oxidise
  crust and volcanic gases).
- **Cost.** One cold-trap table (≤ 260 adiabats) and one sea-level solve per step: negligible
  (run times as before, 13–19 min on GitHub runners).

## Caveats

- The cold point is fixed at 190 K, calibrated to today's ~3–4 ppm. It is the largest
  uncertainty of the onset (×2 in water per 4 K).
- The O₂ sink is a constant placeholder (#201). A sink that grows with tectonics, weathering
  and the O₂ itself would raise the late loss towards the energy limit.
- The model's climate does not see the O₂ (bars at late epochs), the water vapour's greenhouse
  beyond its linear OLR, or a lower surface pressure. N₂ is a fixed 1 bar. Late-epoch tropical
  oceans above 100 °C under ~1.5 bar would boil; the model keeps them liquid (part of the
  missing runaway greenhouse).
- The sea-level budget term `escape` is a lower bound (see the footnote above).
- Left out: H₂/CH₄ (#202), drag of O, ion pickup, impact erosion, Jeans escape of heavy gases.
  All are minor for this planet (see the plan).

## Evaluation

An independent agent checked the numbers against the logs: every cell of the key-number and
sensitivity tables matched. It confirmed the units (seconds per year, atoms vs mol, one O₂ per
four H), the limits, the XUV scaling, the lake and floor handling and the determinism. Its
findings and what was done:

1. **The `escape` sea-level term was only 35–65% of the removed water's effect.** The cause is
   the order-dependent attribution while the level stands at a pass (see the footnote; checked
   with a diagnostic build: the first removals lower the level exactly as expected). The
   tables now give the equivalent of the lost water and the on − off difference. The term's
   documentation calls it a lower bound.
2. **"O₂-regulated" was overstated.** "Halving the sink halves the loss" was wrong (a third of
   the sink gives 0.6×). Epoch 8.4 is co-limited by the energy limit. Reworded.
3. **The energy-limited phase at epoch 8.4 is a start-up transient**, and the 10–30% is a rough
   estimate from fresh-start runs. Now labelled so.
4. **The conservation claim relied on a residual that is zero by construction.** It now cites
   the on − off water check and the unit tests.
5. **Boiling above 100 °C and the climate not seeing the O₂.** Added to the caveats.
6. **Minor.** The density is now shared with the carbon cycle, a test comment is fixed, and
   `at_floor` is no longer set by a zero loss.
