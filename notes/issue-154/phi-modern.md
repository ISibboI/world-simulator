# #154: what is a realistic φ for the modern preset?

Sebastian on the φ clamp (2026-11-27): "the clamp was meant for barren. If the start is biotic
current earth then the clamp should naturally be realistic values for that preset." This
derives the band for `--biosphere modern` the same way as [phi-barren.md](phi-barren.md) does for
the barren default.

Project goal: an Earth-like planet, not Earth; Earth values are orientation only.

**Short answer:** about **1, with a plausible range of roughly 0.3–3**, the same band as for the
barren planet. The calibrated **3.5–8.2** of the modern runs lies **above** it. It is not
pushed there by an uncertainty of the laws. It comes from two properties of the model planet:
its ~2× Earth's degassing, and land that weathers less than Earth's at today's CO₂. Both are
things the CO₂ should respond to, so the clamp binds at 3 in the modern runs, and the CO₂ rises
after the window until the laws balance the degassing.

## 1. What φ absorbs with plants

In the modern preset the laws are those of today's vegetated Earth: `plant_factor` b = 1,
the saturating soil-CO₂ law `(2r/(1+r))^0.4`, and `k_land` and `c₀` are calibrated on
today's fluxes (plan §3–4). So φ = 1 means "today's Earth's weathering laws, applied to the
model's land, climate and sea floor".

| factor | uncertainty that φ absorbs (modern) |
|---|---|
| **Plant enhancement** (2–10× over bare rock) | **None.** It is the ratio of vegetated to bare weathering and matters only for the barren preset. `k_land` is fitted to rivers of today's vegetated continents, deserts included, so the plants are in the reference. |
| `k_land` (today's silicate Ca + Mg flux, ~5 Tmol/yr from Gaillardet et al. 1999) | Estimates of the global silicate weathering flux differ by ~1.5× (Gaillardet et al. 1999; Moon et al. 2014, secondary). |
| Reactive surface area, kinetics, lithology | ~2×, as for barren. |
| Seafloor uptake (c₀, CO₂ and temperature dependence) | ~3× (Alt & Teagle 1999 vs Krissansen-Totton & Catling 2017). About 1/3 of the sink at φ = 1 (results: land 2.1, seafloor 1.0 Tmol/yr at 100 Myr). |
| Degassing coefficients (ridges, arcs, plumes, metamorphic) | Each ~2×. |

Combined as independent factors, weighted by the sinks' shares (land ~2/3, seafloor ~1/3):
`ln σ ≈ √((⅔·√(ln²1.5 + ln²2))² + (⅓·ln 3)² + ln²2) ≈ 0.95`, a 1σ factor of **~2.6**:
0.4–2.6. **0.3–3** is again a generous plausible band. The band is no wider than the barren one:
the modern laws have no plant uncertainty (the barren laws have ×2^±1 for it), but land carries
more of the sink. So the two presets share one band, and `phi_min`/`phi_max` keep a single
default (`--set carbon.phi_min=…`/`phi_max=…` change it per run).

## 2. Why the calibrated 3.5–8.2 lies above

- **Degassing.** The model degasses 12–16 Tmol/yr after the spin-up, against Earth's ~6–10
  (plan §2). Its sea floor forms faster. That accounts for φ ≈ 2.
- **Weak land weathering at today's CO₂.** At φ = 1 the model's land removes ~2 Tmol/yr, about
  0.4× the ~5 Tmol/yr that `k_land` gives on Earth. The model's continents start colder and
  icier (16% ice at t = 0), and their runoff and fresh-rock supply differ from Earth's. Part of
  this comes from the coarse grid (#64).

Neither is a residual uncertainty of the laws. A faster-degassing Earth-like planet with plants
would hold more CO₂ until its weathering balances. So φ is not raised to make up for that, and
the clamp binds.

## 3. What the clamp does in the modern runs

- φ = 3 instead of 3.5–8.2 means the sinks fall short of the degassing by a factor of 1.2–2.7 at
  280 ppm. The CO₂ is still held for the 150-Myr window, so the forcing is continuous at its end.
- The modern sink's sensitivity is small: d ln W/d ln r ≈ 0.6, including the warming from the
  forcing. Most of it comes through the temperature, because the plants' law saturates in CO₂.
- The CO₂ settles at `(φ_needed/3)^{1/0.6}` × 280 ppm, i.e. ~1.3–5× today's, before the
  brightening sun draws it down again. Expected: +1.5 to +9 W/m², a few K warmer than the old
  runs.
- Observed (4 seeds): balancing φ 3.9–5.6, the CO₂ peaks at 3.6–5.7× today's (+7.5 to
  +10.5 W/m², with the degassing still spinning up), then falls to 75–150 ppm at 1000 Myr under
  the brightening sun. The planet ends 22.7 °C warm, against 20.8 °C unclamped. Details are in
  [results.md](results.md#clamped-φ-modern-runs).

## Sources

As in [phi-barren.md](phi-barren.md), plus Moon, S. et al. (2014), GCA 134, 257–274
(secondary).
