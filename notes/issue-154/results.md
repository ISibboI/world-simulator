# #154 results: the carbon cycle against a fixed forcing

Commit `78e2338` for the "on" and "modern on" runs, `a7e8031` for the "off" runs (`--no-carbon-cycle`
gives bit-identical planets on both: only the log format of the fluxes differs). 8 seeds (0–7)
× 1000 Myr, 6 subdivisions, snapshots every 50 Myr, through `runs` (GitHub runners and tktie).
An independent evaluation of an earlier round is in [`review-results.md`](review-results.md);
its findings are addressed below ("Evaluation and changes").

| set | arguments |
|---|---|
| **on** (new default) | `--duration 1000` (barren, epoch 0, carbon cycle on) |
| **off** | `--duration 1000 --no-carbon-cycle` (the default of #193) |
| **modern on** | `--duration 1000 --biosphere modern --epoch 4` |
| **modern off** | `--duration 1000 --biosphere modern --epoch 4 --no-carbon-cycle` |

The CO₂ stays that of the start for the first 150 Myr in every run (the calibration window,
below), so on and off are identical until then.

## Temperature, ice and CO₂ (8-seed means; ranges are per-seed values)

| | on | off | modern on | modern off |
|---|---|---|---|---|
| global mean at t = 0 (°C) | 12.3 | 12.3 | 12.4 | 12.4 |
| at 150 Myr (end of the window) | 15.1 | 15.1 | 16.6 | 16.6 |
| mean 200–1000 Myr | **16.9** (15.7–18.5) | 17.5 (17.3–18.2) | **19.3** (16.8–21.5) | 21.0 (19.9–21.6) |
| at 1000 Myr | **17.6** (16.3–19.3) | 19.6 (18.7–21.2) | **20.8** (18.9–23.2) | 25.5 (24.1–26.1) |
| **warming 150 → 1000 Myr** | **+2.5 K** | +4.4 K | **+4.3 K** | +8.9 K |
| part of the "off" warming that remains | **57%** | — | **48%** | — |
| trend 200–1000 Myr (K/Gyr) | +2.3 (−0.3…+3.5) | +6.0 (3.2–8.0) | +3.4 (1.1–6.0) | +10.7 (7.3–13.5) |
| permanently frozen, mean 200–1000 | 1.6% (0.3–3.0) | 1.2% | 6.8% (4.5–10.1) | 4.9% |
| permanently frozen, at 1000 Myr | 1.2% | 0.5% | 5.8% | 1.8% |
| permanently frozen, most at any snapshot | 7.5% (t = 0) | 7.5% (t = 0) | 16.2% (t = 0) | 16.2% (t = 0) |
| pCO₂ at the start | 0.49 bar | 0.49 bar | 280 ppm | 280 ppm |
| pCO₂ at 1000 Myr | **0.37 bar** (0.24–0.56) | 0.49 | **68 ppm** (27–183) | 280 ppm |
| pCO₂ range over the run | 0.23–0.76 bar | — | 17–920 ppm | — |
| forcing, mean 200–1000 (W/m²) | 60.7 | 61.8 | −3.1 | 0 |
| φ (calibration) | 0.95–1.57 | | 3.5–8.2 | |

Seed-mean trajectories (°C; mean pCO₂ of "on" in bar, of "modern on" in ppm):

| t (Myr) | 0 | 100 | 150 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900 | 1000 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| on | 12.3 | 14.8 | 15.1 | 15.9 | 16.1 | 16.5 | 16.7 | 16.8 | 17.1 | 17.6 | 17.5 | 17.6 |
| off | 12.3 | 14.8 | 15.1 | 15.4 | 15.7 | 15.9 | 16.5 | 17.5 | 18.6 | 19.0 | 19.1 | 19.6 |
| pCO₂ on (bar) | 0.49 | 0.49 | 0.49 | 0.54 | 0.55 | 0.54 | 0.50 | 0.44 | 0.43 | 0.41 | 0.38 | 0.37 |
| modern on | 12.4 | 15.6 | 16.6 | 18.1 | 18.4 | 18.5 | 18.9 | 19.2 | 19.7 | 20.2 | 19.8 | 20.8 |
| modern off | 12.4 | 15.6 | 16.6 | 17.0 | 18.0 | 18.3 | 19.6 | 21.1 | 22.4 | 23.1 | 24.0 | 25.5 |
| pCO₂ modern on (ppm) | 280 | 280 | 300 | 420 | 400 | 330 | 240 | 160 | 140 | 100 | 90 | 68 |

Per-seed values: [`per-seed.txt`](per-seed.txt) (from [`analyse.py`](analyse.py)); flux tables
by time: [`fluxes.txt`](fluxes.txt).

**What it means.**

- **The carbon cycle halves the warming from the brightening sun.** From 150 to 1000 Myr the
  sun adds ~11 W/m² of absorbed sunlight in the barren runs (+21 W/m² in the modern runs). With
  the cycle the planet warms 2.5 K instead of 4.4 (modern: 4.3 instead of 8.9). The rest is the
  price of the feedback: the weathering needs a warmer climate to remove the CO₂ (a finite gain;
  the plan review expected about half to remain). The end state is 2 K cooler than without the
  cycle (modern: 4.7 K).
- **The spin-up of the tectonics shows after the window too.** The degassing grows from
  12 Tmol/yr at 150 Myr to 14–15 at 200–400 Myr (plates and plumes still gathering pace), so the
  CO₂ first rises (barren 0.49 → 0.55 bar at 300 Myr, modern 280 → 420 ppm at 200 Myr) and only
  then falls under the brightening sun. This is the model starting from rest, not the planet's
  physics; a tectonic spin-up before t = 0 would remove it (suggested as a follow-up).
- **Modern Earth (`--biosphere modern --epoch 4`)**: the CO₂ falls to 68 ppm at 1000 Myr,
  27 ppm in the lowest seed, 17 ppm at its lowest: close to the 10 ppm limit of C4
  photosynthesis (`presets::MIN_CO2`), which the cycle does not enforce; Caldeira & Kasting
  (1992) find the same fall over ~1 Gyr for Earth.
- **Why the first 150 Myr warm in both**: of the +2.8 K (barren) the brightening sun gives
  ~1 K; the rest is the initial highlands (~1 km mean land) wearing down to ~0.5 km (#190), so
  bright cold land and initial ice go. Modern: +4.1 K, ~2.3 K of it the sun.
- **Ice**: the barren default keeps 1–2% permanent ice (off: 0.5% at the end), the modern runs
  ~6% (off: 1.8%). Polar caps remain in every run.

## Sources and sinks (Tmol C/yr, 8-seed means at the time)

| t (Myr) | pCO₂ | forcing | degassing: total (ridges, arcs, plumes, metamorphic) | sinks: total (land, seafloor) |
|---|---|---|---|---|
| **on** 1 | 0.49 bar | 61.76 | 0.3 (0.1, 0.2, 0, 0) | 4.2 (4.0, 0.2) |
| 50 | 0.49 | 61.76 | 8.3 (2.8, 5.1, 0.2, 0.3) | 8.7 (3.6, 5.1) |
| 100 | 0.49 | 61.76 | 10.4 (3.4, 5.9, 0.5, 0.6) | 9.7 (3.4, 6.3) |
| 150 | 0.49 | 61.80 | 12.4 (3.8, 7.3, 0.8, 0.5) | 12.1 (3.5, 8.5) |
| 200 | 0.54 | 62.84 | 14.2 (4.3, 8.3, 1.0, 0.7) | 14.2 (3.6, 10.6) |
| 400 | 0.54 | 62.61 | 15.2 (4.0, 8.7, 0.9, 1.5) | 15.1 (3.9, 11.2) |
| 600 | 0.44 | 60.34 | 13.9 (3.9, 8.2, 0.8, 1.1) | 14.0 (4.1, 10.0) |
| 800 | 0.41 | 59.27 | 15.4 (4.2, 8.8, 0.9, 1.4) | 15.6 (4.0, 11.5) |
| 1000 | 0.37 | 58.04 | 15.3 (4.1, 8.8, 0.9, 1.6) | 15.4 (4.3, 11.1) |
| **off** 1000 | 0.49 | 61.76 | 18.2 (4.2, 11.3, 0.9, 1.8) | 21.6 (6.1, 15.5) |
| **modern on** 100 | 280 ppm | 0 | 7.6 (3.6, 3.1, 0.6, 0.4) | 3.1 at φ = 1 (2.1, 1.0) |
| 200 | 420 ppm | 1.84 | 11.8 (4.3, 6.1, 0.9, 0.5) | 11.9 (5.2, 6.7) |
| 600 | 160 ppm | −3.20 | 11.8 (4.2, 5.9, 0.9, 1.0) | 11.9 (6.5, 5.4) |
| 1000 | 68 ppm | −8.82 | 11.3 (3.9, 5.3, 1.0, 1.2) | 11.3 (7.0, 4.3) |

- **Balance**: with the cycle on, sources and sinks agree within the step noise at every time;
  the logged reservoir change matches the integrated fluxes to 0.1% of the reservoir in the
  barren runs and within the rounding of the log in the modern runs (whose reservoir is 50×
  smaller); the unit tests check the exact conservation. With the cycle off, the sinks at the
  end exceed the sources by 3.4 Tmol/yr on average (−0.2 to +8.2 per seed): the fixed forcing
  keeps a climate that would draw CO₂ down.
- **Gross degassing 12–16 Tmol/yr**, about twice Earth's 6–10: the model makes sea floor
  ~1.5–2× faster than Earth (80–140 Gt/yr of crust; mean ocean age 55–70 Myr), and **arcs
  return half of the seafloor's carbon**: of the arcs' 8.8 Tmol/yr, ~5 are that return, so the
  primary arc flux (wedge and sediment, ~3.5) is about that of the ridges. Likewise the net
  seafloor sink (uptake minus return, ~5.5) is about the land's (4.3) at 1000 Myr. The absolute
  sizes set the timescales; the climate follows from the ratios, which φ calibrates.
- Seafloor weathering dominates the gross sink at epoch 0, as in Archean carbon-cycle models
  (Sleep & Zahnle, 2001; Krissansen-Totton et al., 2018). Land weathering is mostly
  supply-limited: a quarter of the Ca + Mg of the eroded rock dissolves (weathering intensity
  0.22–0.28), against ~0.5 in the modern runs (plants, saturating CO₂ law).
- **φ**: 0.95–1.57 barren, so the literature laws give the right weathering for the prescribed
  forcing of epoch 0 within a factor 1.6; 3.5–8.2 modern: at today's CO₂ the laws give too
  little weathering for the model's degassing (twice Earth's).

## Calibration and timescales

- The tectonics starts at rest: the degassing grows from 0.3 Tmol/yr in the first step to
  ~12 at 150 Myr and 14–15 after 200 Myr. The CO₂ stays that of the start for 150 Myr, and φ
  balances the sinks against the sources over 100–150 Myr. A first round calibrated over the
  first 20 Myr: the CO₂ then rose with the spin-up to ~1 bar and the barren planet ended 1.4 K
  **warmer** than without the cycle.
- At the end of the window the change is smooth in the barren runs (forcing +0.01…+0.08 W/m²
  in the step). In the modern runs it jumps by −0.9 to +2.1 W/m² (later steps: s.d.
  0.17–0.33 W/m²): their CO₂ equilibrates within the step, to the climate of step 150 rather
  than the window's mean. That is ~1 K once; a second round had −1.8 to +5.4 W/m² (fixed by
  smoothing the tectonic rates from the window's mean, see below).
- **Timescales**: the barren reservoir is 1.5 × 10²⁰ mol (60% in the air), 10 Myr of
  degassing; the detrended ln pCO₂ follows the degassing with correlation 0.4–0.65 at lags of
  0–5 Myr. The modern reservoir (3 × 10¹⁸ mol, mostly ocean) is 0.3 Myr of degassing: the CO₂
  is in equilibrium in every step (correlation 0.6–0.8 at lag 0, ~0 by 20 Myr).

## Stability, snowball and runaway

- **No numerical oscillation.** Barren: step-to-step forcing changes s.d. 0.03–0.06 W/m²,
  lag-1 autocorrelation +0.73 (smooth). Modern: s.d. 0.17–0.33 W/m² (largest 2.2), lag-1
  autocorrelation +0.17: the equilibrium follows the residual noise of the tectonic rates; no
  growing or periodic mode. That moves the modern temperature by ~0.1–0.25 K per step.
- **Snowball**: none. The most ice in any run is at t = 0 (7.5% barren, 16% modern); the
  coldest end state is 16.3 °C barren, 18.9 °C modern (8% ice). A cooling slows the
  weathering, so the CO₂ builds up.
- **Runaway**: none. The warmest end states are 19.3 °C (barren) and 23.2 °C (modern). The
  modern runs approach the limit of the feedback under a brightening sun: the CO₂ runs out
  (27 ppm in one seed at 1000 Myr).

## Response to tectonic events

- The **degassing** moves the CO₂: the 50-Myr changes of ln pCO₂ correlate with those of the
  degassing at 0.49 (barren) and 0.62 (modern), pooled over 8 seeds × 16 intervals. This is
  partly circular, since the arcs' return of seafloor carbon grows with the CO₂; the ridges are
  nearly neutral at epoch 0, because more sea floor brings both more ridge degassing and more
  seafloor weathering.
- **Supercontinents** leave at most a weak trace: the changes of the largest land mass (it
  jumps between ~50% and ~95% of the land when two continents join or part, often by a narrow
  connection at this grid) correlate with the CO₂ changes at 0.09 in both sets (an earlier
  round: 0.19–0.29, assembly raising the CO₂ slightly, as drier interiors weather less).
  Examples (barren): seed 3 assembles (36% → 85%) at 600–650 Myr with Δln pCO₂ −0.01; seed 6
  breaks up (98% → 44%) at 900–950 Myr with +0.00. At epoch 0 the land is a minority of the
  sink and supply-limited, so the land's geography matters little; in the modern runs single
  events give ±0.1–0.5 in ln pCO₂ with both signs (seed 4 breaking up at 800–850 Myr: −0.47).

## Runtime

The carbon step takes ~1 ms per step during the window, ~12 ms per step after it (60
bisection iterations over ~12 000 land cells) and 0.6 s once for the calibration: ~2% of a
0.56 s step at 6 subdivisions (local, single-threaded). The run times of the sets (13–18 min)
differ more between runners than between on and off.

## Evaluation and changes

The independent evaluation of the previous round ([`review-results.md`](review-results.md))
confirmed the numbers, conservation and stability, and found:

| finding | change |
|---|---|
| compensation overstated ("¼ remains" from the most favourable trend) | Now the endpoint numbers: about half remains. |
| modern runs jump at the calibration step (−1.8 to +5.4 W/m²) | The tectonic rates (degassing and the new crust of the seafloor sink) are smoothed, starting from the window's mean: jumps now −0.9 to +2.1 W/m² (remaining: the step's own climate). |
| modern step noise from the unsmoothed new crust in the seafloor sink | Same change: s.d. 0.26–0.52 → 0.17–0.33 W/m². |
| spin-up continues after the window, also in the barren runs | Stated above. |
| early warming attributed to erosion only | Sun ~1 K of 2.8 K (barren). |
| predicted ΔT biased by −0.1 to −0.2 K (`toa_imbalance` ≈ −0.2 W/m² at rest) | Not changed: it shifts the predicted equilibrium by ~0.15 K; the coarse cells are not exactly of equal area and the diagnostic does not include the ice-sheet albedo's slow part. Noted as a caveat. |
| modern bottom water below −1.8 °C half the time | Not changed: the seafloor sink has no temperature feedback in a cold modern ocean, which weakens the modern feedback; a real bottom water cannot be colder than freezing. |
| docs: 20 Myr in plan §8, CLI help | Updated (150 Myr; the cycle acts after it). Runs shorter than 150 Myr never activate the cycle. |

## Caveats

- **The cycle starts after 150 Myr.** Before, the CO₂ is held (the tectonics spins up from
  rest); runs shorter than that behave as with `--no-carbon-cycle`.
- The CO₂ in bar is indicative: the forcing fit is extrapolated above 10 000 ppmv (§1 of the
  plan); 0.37–0.5 bar may be ~0.1–0.3 bar in a radiative–convective model.
- The degassing is ~2× Earth's because the model's tectonics is faster; φ calibrates that away
  for the climate, but the timescales are correspondingly shorter.
- The modern runs' CO₂ goes below 30 ppm in one seed; there is no floor for plants (a plant
  weathering factor that weakens between ~150 and 10 ppm would give one, as a follow-up).
- Carbon buried as carbonate is not added to the sediment (~1.5 Gt/yr of CaCO₃, ~10% of the
  river sediment), and the plates do not carry it: fixed concentrations and a 100-Myr mean of
  the seafloor uptake stand in for that.
- Grid: at ~110 km cells the erosion that supplies the land weathering is net per cell and
  step (a lower bound of the gross turnover), and the sea-floor production jumps when cells
  open; the 3-Myr smoothing of the tectonic rates takes the worst of that out.
