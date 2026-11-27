# #154 results: the carbon cycle against a fixed forcing

Commit `5048ce1` (final; "on" and "modern" runs), `a7e8031` for the "off" runs (only the log
format differs; `--no-carbon-cycle` is bit-identical to the forcing of the base). 8 seeds (0–7)
× 1000 Myr, 6 subdivisions, snapshots every 50 Myr, through `runs` (GitHub runners and tktie).

| set | arguments |
|---|---|
| **on** (new default) | `--duration 1000` (barren, epoch 0, carbon cycle on) |
| **off** | `--duration 1000 --no-carbon-cycle` (the default of #193) |
| **modern on** | `--duration 1000 --biosphere modern --epoch 4` |
| **modern off** | `--duration 1000 --biosphere modern --epoch 4 --no-carbon-cycle` |

The CO₂ stays that of the start for the first 150 Myr in every run (the calibration window,
see "Calibration" below), so on and off are identical until then.

## Temperature, ice and CO₂ (8-seed means; ranges are per-seed values)

| | on | off | modern on | modern off |
|---|---|---|---|---|
| global mean at t = 0 (°C) | 12.3 | 12.3 | 12.4 | 12.4 |
| at 150 Myr (end of the window) | 15.1 | 15.1 | 16.6 | 16.6 |
| mean 200–1000 Myr | **16.7** (15.6–18.2) | 17.5 (17.3–18.2) | **19.3** (16.4–21.6) | 21.0 (19.9–21.6) |
| at 1000 Myr | **17.1** (15.1–18.4) | 19.6 (18.7–21.2) | **20.7** (16.2–25.0) | 25.5 (24.1–26.1) |
| trend 200–1000 Myr (K/Gyr) | **+1.4** (−0.5…+2.6) | +6.0 (3.2–8.0) | **+4.0** (1.7–7.2) | +10.7 (7.3–13.5) |
| trend 400–1000 Myr (K/Gyr) | **+0.8** (−1.6…+2.8) | +6.5 | **+5.0** (3.1–7.9) | +11.6 |
| permanently frozen, mean 200–1000 | 1.7% (0.2–2.9) | 1.2% | 6.8% (4.3–10.4) | 4.9% |
| permanently frozen, most at any snapshot | 7.5% (t = 0) | 7.5% (t = 0) | 16.2% (t = 0) | 16.2% (t = 0) |
| pCO₂ at the start | 0.49 bar | 0.49 bar | 280 ppm | 280 ppm |
| pCO₂ at 1000 Myr | **0.33 bar** (0.26–0.39) | 0.49 | **~75 ppm** (13–150) | 280 ppm |
| pCO₂ range over the run | 0.25–0.74 bar | — | 13–890 ppm | — |
| forcing, mean 200–1000 (W/m²) | 60.4 | 61.8 | −2.8 | 0 |

Seed-mean trajectories (°C, and the mean pCO₂ in bar for "on"):

| t (Myr) | 0 | 100 | 150 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900 | 1000 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| on | 12.3 | 14.8 | 15.1 | 15.9 | 16.2 | 16.6 | 16.9 | 16.8 | 16.9 | 16.9 | 17.1 | 17.1 |
| off | 12.3 | 14.8 | 15.1 | 15.4 | 15.7 | 15.9 | 16.5 | 17.5 | 18.6 | 19.0 | 19.1 | 19.6 |
| pCO₂ on (bar) | 0.49 | 0.49 | 0.50 | 0.54 | 0.54 | 0.52 | 0.48 | 0.46 | 0.43 | 0.37 | 0.35 | 0.33 |
| modern on | 12.4 | 15.6 | 16.6 | 18.2 | 18.5 | 17.8 | 19.1 | 19.0 | 19.5 | 20.4 | 20.9 | 20.7 |
| modern off | 12.4 | 15.6 | 16.6 | 17.0 | 18.0 | 18.3 | 19.6 | 21.1 | 22.4 | 23.1 | 24.0 | 25.5 |

Per-seed values: [`per-seed.txt`](per-seed.txt) (from [`analyse.py`](analyse.py)); flux tables by time: [`fluxes.txt`](fluxes.txt).

**What it means.**

- **The drift is mostly gone in the barren default.** After the window the sun brightens by
  +11 W/m² of absorbed sunlight over 850 Myr. The carbon cycle takes the forcing down by
  ~4.7 W/m² (0.50 → 0.33 bar) and the planet warms 1.4 K/Gyr instead of 6.0 (0.8 against 6.5
  after 400 Myr). Full compensation is not expected: the weathering needs a somewhat warmer
  climate to remove more CO₂ (the feedback has a finite gain; the reviewer's estimate before
  the runs was "about half of the warming remains"; it is ~¼).
- **Modern Earth (`--biosphere modern --epoch 4`)**: the brighter sun adds +23 W/m² over 1 Gyr.
  The cycle draws the CO₂ from 280 ppm (and a peak of ~400 ppm while the tectonics spins up)
  down to ~75 ppm, the forcing to −8 W/m², and halves the warming (4.0 against 10.7 K/Gyr).
  One seed reaches 13 ppm, near the 10 ppm limit of C4 photosynthesis that `presets::MIN_CO2`
  uses for future epochs; the carbon cycle has no such floor.
- **Why the first 150 Myr warm in both**: the start is not in equilibrium for reasons the carbon
  cycle cannot see: the initial highlands (~1 km mean land) wear down to ~0.5 km over the first
  ~100 Myr (#190), so the bright cold land and the initial ice go. That warming (+2.8 K in the
  barren runs) happens with the CO₂ held at the start's value.
- **Ice**: the barren default ends with 2% permanent ice (off: 0.5%), the modern runs with 6.5%
  (off: 1.8%). Polar caps remain in every run.

## Sources and sinks (Tmol C/yr, 8-seed means at the time)

| t (Myr) | pCO₂ | forcing | degassing: total (ridges, arcs, plumes, metamorphic) | sinks: total (land, seafloor) |
|---|---|---|---|---|
| **on** 1 | 0.49 bar | 61.76 | 0.3 (0.1, 0.2, 0, 0) | 4.2 (4.0, 0.2) |
| 50 | 0.49 | 61.76 | 8.3 (2.8, 5.1, 0.2, 0.3) | 8.7 (3.6, 5.1) |
| 100 | 0.49 | 61.76 | 10.4 (3.4, 5.9, 0.5, 0.6) | 9.7 (3.4, 6.3) |
| 150 | 0.50 | 61.81 | 13.0 (4.0, 7.7, 0.8, 0.5) | 12.4 (3.5, 8.9) |
| 200 | 0.54 | 62.71 | 14.4 (4.4, 8.4, 1.0, 0.7) | 14.6 (3.6, 11.1) |
| 400 | 0.52 | 62.16 | 15.3 (4.2, 8.8, 0.9, 1.4) | 15.4 (4.0, 11.4) |
| 600 | 0.46 | 60.92 | 16.0 (4.3, 9.1, 0.9, 1.7) | 16.1 (4.5, 11.6) |
| 800 | 0.37 | 58.50 | 15.5 (4.2, 9.0, 0.8, 1.5) | 15.3 (4.3, 10.9) |
| 1000 | 0.33 | 57.11 | 16.3 (4.3, 9.5, 0.9, 1.6) | 16.2 (5.2, 11.0) |
| **off** 1000 | 0.49 | 61.76 | 18.2 (4.2, 11.3, 0.9, 1.8) | 21.6 (6.1, 15.5) |
| **modern on** 100 | 280 ppm | 0 | 7.6 (3.6, 3.1, 0.6, 0.4) | 3.1 at φ = 1 (2.1, 1.0) |
| 200 | 420 ppm | 1.84 | 12.0 (4.3, 6.1, 1.0, 0.7) | 12.1 (5.3, 6.7) |
| 600 | 200 ppm | −2.83 | 12.3 (4.2, 5.8, 1.0, 1.3) | 12.3 (6.6, 5.8) |
| 1000 | 75 ppm | −8.14 | 12.0 (3.9, 5.5, 1.2, 1.4) | 12.0 (7.1, 4.8) |

- **Balance**: with the cycle on, sources and sinks agree within the step noise at every time
  (and the logged reservoir change matches the integrated fluxes to 0.1%, the precision of the
  log; the unit tests check the exact conservation). With it off, the sinks exceed the sources
  by up to 3.4 Tmol/yr at 1000 Myr: the fixed forcing keeps a climate that would draw CO₂ down.
- **Total degassing 12–16 Tmol/yr**, about twice Earth's 6–10: the model's sea floor is made
  ~1.5–2× faster than Earth's (80–140 Gt/yr of crust; its mean ocean age is 55–70 Myr), and
  arcs return half of the seafloor's carbon, which a high-CO₂ planet takes up a lot of. The
  absolute numbers only set the timescale (§ below); the climate follows from the ratios.
- **Arcs are the largest source** (9.5 of 16 Tmol/yr in the barren runs): 2 from the mantle
  wedge, 1–1.5 from subducted sediment, ~6 the returned half of the seafloor uptake. **Seafloor
  weathering is the largest sink** at epoch 0 (11 of 16; net of the return ~5.5), as in
  Archean carbon-cycle models (Sleep & Zahnle, 2001; Krissansen-Totton et al., 2018). Land
  weathering takes 3.5–5 Tmol/yr, mostly supply-limited (weathering intensity 0.2: a fifth of
  the Ca + Mg of the eroded rock dissolves, against ~0.5 in the modern runs, where plants and the
  saturating CO₂ law make the kinetics faster).
- **Calibration φ**: 0.95–1.57 in the barren runs, i.e. the literature laws give the right
  weathering for the prescribed forcing of epoch 0 within a factor 1.6. In the modern runs φ =
  3.5–8.2: at today's CO₂ and climate the laws give too little weathering for the model's
  degassing (twice Earth's).

## Calibration and timescales

- The tectonics starts at rest: the degassing grows from 0.3 Tmol/yr in the first step to
  ~13 at 150 Myr and ~15 after 300 Myr (plates speed up, sea floor opens, plumes rise,
  metamorphism starts). A first version calibrated φ over the first 20 Myr: the CO₂ then rose
  with the spin-up to ~1 bar and the barren planet ended 1.4 K **warmer** than without the
  cycle. The window is now 150 Myr, balanced over its last 50 Myr; after it the CO₂ changes
  smoothly (0.495 → 0.497 → 0.507 bar at 150, 151, 160 Myr). A second version showed a
  −2.7 W/m² dip at the first active step of the modern runs: the degassing smoothing lagged the
  slab carbon that φ adds; it now restarts at the calibration.
- **Timescales**: the reservoir at epoch 0 is 1.5 × 10²⁰ mol (60% in the air), 10 Myr of
  degassing; CO₂ follows the degassing with a 5–10 Myr lag (correlation of the detrended
  ln pCO₂ with the degassing: 0.4–0.5 at lag 0, 0.5–0.7 at 5–10 Myr). In the modern runs the
  reservoir (3 × 10¹⁸ mol, mostly ocean) is 0.3 Myr of degassing: the CO₂ is in equilibrium
  every step (correlation 0.5–0.7 at lag 0, falling to 0 by 20 Myr).

## Stability, snowball and runaway

- **No numerical oscillation.** Barren: the step-to-step forcing changes are small (s.d.
  0.04–0.07 W/m²) and positively autocorrelated (lag-1 autocorrelation +0.27). Modern: they
  follow the noise of the tectonic rates (s.d. 0.26–0.52 W/m², largest 3.4 W/m²), with a lag-1
  autocorrelation of −0.24, which is what equilibrium tracking of white-ish noise gives (−0.5
  for white noise, less with the 3 Myr smoothing); there is no growing or periodic mode.
  That noise moves the temperature by ~0.2–0.4 K from step to step in the modern runs.
- **Snowball**: none. The most ice in any run is at t = 0 (7.5% barren, 16% modern); the
  coldest barren seed at the end is 15.1 °C with 4% ice. The weathering slows as the climate
  cools, so a cooling is countered by CO₂ build-up.
- **Runaway**: none. The hottest barren end state is 18.4 °C, modern 25 °C (one seed, whose
  CO₂ is already at 13 ppm: the feedback runs out of CO₂ to remove, the limit of the
  carbonate–silicate feedback under a brightening sun).

## Response to tectonic events

- The **degassing** is what moves the CO₂: the 50-Myr changes of ln pCO₂ correlate with those of
  the degassing at 0.45 (barren) and 0.54 (modern), pooled over 8 seeds × 16 intervals.
- **Supercontinents** leave no robust signal: the changes of the largest land mass (in these
  runs it jumps between ~50% and ~95% of the land when two continents join or part, often by a
  narrow connection) correlate with the CO₂ changes at +0.29 (barren; assembly slightly raises
  the CO₂, as drier interiors weather less) and 0.06 (modern). Examples: seed 5 assembles
  (33% → 94%) at 400–450 Myr with ln pCO₂ −0.08 and degassing −0.9 Tmol/yr; seed 0 breaks up
  (95% → 49%) at 400–450 Myr, ln pCO₂ −0.14. At epoch 0 the land is a minor sink (a quarter of
  the total, supply-limited), so the land's geography matters less than the sea floor's
  production; in the modern runs the land is half the sink but the assemblies there coincide
  with both signs of the CO₂ change.

## Runtime

The carbon step takes ~1 ms per step during the window, ~12 ms per step after it (60
bisection iterations over ~12 000 land cells) and 0.6 s once for the calibration: ~2% of a 0.56 s
step at 6 subdivisions (local, single-threaded). The run times of the sets (13–18 min) differ
more between runners than between on and off.

## Caveats

- The CO₂ in bar is indicative: the forcing fit is extrapolated above 10 000 ppmv (§1 of the
  plan); 0.33–0.5 bar may be ~0.1–0.3 bar in a radiative–convective model.
- The degassing is ~2× Earth's because the model's tectonics is faster; its effect on the
  climate is calibrated away by φ, but the timescales are correspondingly shorter.
- The modern runs' CO₂ goes below 100 ppm by 1000 Myr with no floor for plants.
- Carbon buried as carbonate is not added to the sediment (~1.5 Gt/yr of CaCO₃, ~10% of the
  river sediment), and the plates do not carry it: fixed concentrations and a 100-Myr mean of
  the seafloor uptake stand in for that.
- The temperature dependence of the seafloor sink (92 kJ/mol on the bottom water, the
  surface temperature poleward of 55°) carries much of the feedback at epoch 0; with an ice-free
  ocean the bottom water would warm with the polar surface (it is 0–3 °C in these runs).
- Grid: at ~110 km cells the erosion that supplies the land weathering is net per cell and
  step (a lower bound of the gross turnover), and the sea-floor production jumps when cells
  open; the 3-Myr smoothing of the degassing takes the worst of that out.
