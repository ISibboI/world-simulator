# Issue #149: the climate follows the brightening sun — before/after

## What changed

- `sun::age_of_run(epoch, time)` is the one place that turns the epoch (`--epoch`, Ga before
  present) and the simulated time (Myr) into the sun's age (#150 will change only this).
  `Sun::of_run` (the view) and the climate both use it.
- `TemperatureParams` stores the run's `epoch` and `solar_brightening` (default 1).
  `TemperatureParams::solar_constant(time)` = solar constant of the start ×
  `sun::brightening(age at start, age now)` = `S(age now) / S(age at start)`. At time 0 this is
  exactly 1, so the start of every run is bit-identical to main. A
  `--set temperature.solar_constant` override is scaled the same way.
  `--set temperature.solar_brightening=0` gives main's behaviour.
- `TemperatureModel::update` takes the time. `Planet` passes `tectonics.time()`, the time after
  the step, which is also the frame's time in the view, so the climate and the view's sun line
  agree to within rounding (tested: < 1e-9 W/m²).
- Only `+ − × ÷` and clamping are used, so web and desktop give identical results.
- The greenhouse forcing of an epoch, which offsets the faint sun at the start, **stays fixed**:
  there is no carbon cycle, so long runs warm.
- The log's temperature line shows the climate's current solar constant.

## Setup

8 seeds (0–7), default resolution, two setups, main (45592d6) vs branch (0aaad21; later commits
changed only docs and log text):

- `--duration 500 --snapshot-interval 100` (epoch 0: from today into the future)
- `--epoch 3.5 --duration 1000 --snapshot-interval 100`

Table values are the mean ± standard deviation over the seeds, taken from the logs (script:
`~/tasks/issue-149/extract.py` on tktie).

- S: the climate's solar constant (W/m²; main stays at 1361.0 and 1041.8).
- T, Teq, Tpole: global, equatorial (< 10°) and polar (> 70°) annual mean surface temperature (°C).
- frozen: permanent ice (sea ice and ice sheets), % of the surface.
- precip: global precipitation (m/yr).
- ocean, land: % of the surface.
- sealevel: the dry sea-level offset (m).

## Epoch 0, 500 Myr (S +4.6%)

| t (Myr) | run | n | S | T | Teq | Tpole | frozen | precip | ocean | land | sealevel |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | main | 8 | – | 12.45 ± 0.55 | 24.55 ± 1.39 | -16.82 ± 3.39 | 14.28 ± 1.04 | 0.84 ± 0.08 | 69.20 ± 0.62 | 30.80 ± 0.62 | -339.75 ± 4.59 |
| 0 | branch | 8 | 1361.00 ± 0.00 | 12.45 ± 0.55 | 24.55 ± 1.39 | -16.82 ± 3.39 | 14.28 ± 1.04 | 0.84 ± 0.08 | 69.20 ± 0.62 | 30.80 ± 0.62 | -339.75 ± 4.59 |
| 100 | main | 8 | – | 14.29 ± 1.27 | 26.01 ± 1.68 | -13.69 ± 0.90 | 10.91 ± 2.29 | 0.93 ± 0.11 | 71.44 ± 2.17 | 28.55 ± 2.17 | -241.38 ± 67.45 |
| 100 | branch | 8 | 1373.00 ± 0.00 | 15.56 ± 1.16 | 27.31 ± 1.54 | -12.61 ± 1.10 | 9.43 ± 2.07 | 1.00 ± 0.11 | 71.58 ± 2.22 | 28.43 ± 2.20 | -228.50 ± 59.06 |
| 200 | main | 8 | – | 14.47 ± 0.85 | 26.09 ± 1.22 | -13.20 ± 1.05 | 11.29 ± 1.41 | 0.94 ± 0.08 | 72.71 ± 2.80 | 27.30 ± 2.80 | -141.12 ± 40.93 |
| 200 | branch | 8 | 1385.20 ± 0.00 | 17.06 ± 1.00 | 28.80 ± 1.26 | -11.07 ± 1.32 | 8.07 ± 1.43 | 1.10 ± 0.10 | 72.69 ± 2.66 | 27.30 ± 2.65 | -130.50 ± 40.42 |
| 300 | main | 8 | – | 14.70 ± 0.62 | 26.93 ± 0.86 | -13.24 ± 0.92 | 11.03 ± 1.22 | 0.97 ± 0.07 | 72.43 ± 1.36 | 27.56 ± 1.35 | -128.00 ± 67.71 |
| 300 | branch | 8 | 1397.70 ± 0.00 | 18.46 ± 0.56 | 30.81 ± 0.85 | -10.21 ± 0.91 | 6.80 ± 0.66 | 1.22 ± 0.07 | 72.57 ± 1.96 | 27.41 ± 1.97 | -137.00 ± 60.93 |
| 400 | main | 8 | – | 14.01 ± 0.56 | 26.25 ± 0.59 | -13.65 ± 1.02 | 11.86 ± 1.03 | 0.92 ± 0.05 | 71.18 ± 2.31 | 28.81 ± 2.33 | -156.75 ± 69.10 |
| 400 | branch | 8 | 1410.40 ± 0.00 | 18.90 ± 0.60 | 31.38 ± 0.82 | -9.59 ± 1.36 | 6.98 ± 1.36 | 1.25 ± 0.06 | 71.16 ± 2.40 | 28.84 ± 2.38 | -152.50 ± 38.40 |
| 500 | main | 8 | – | 13.64 ± 0.52 | 24.95 ± 0.85 | -13.51 ± 0.98 | 12.12 ± 1.12 | 0.87 ± 0.06 | 70.63 ± 1.83 | 29.36 ± 1.83 | -170.62 ± 50.25 |
| 500 | branch | 8 | 1423.30 ± 0.00 | 19.55 ± 0.89 | 31.30 ± 1.63 | -8.22 ± 0.90 | 5.90 ± 0.91 | 1.25 ± 0.10 | 70.53 ± 2.92 | 29.46 ± 2.93 | -170.00 ± 54.73 |

Köppen groups of land at 500 Myr (mean of 8 seeds): main A 8.6%, B 40.4%, C 12.4%, D 25.9%,
E 12.7%; branch A 12.9%, B 40.5%, C 14.0%, D 26.3%, E 6.3%.

## Epoch 3.5 Ga, 1000 Myr (S +7.2%, 1041.8 → 1116.7 W/m²)

| t (Myr) | run | n | S | T | Teq | Tpole | frozen | precip | ocean | land | sealevel |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | main | 8 | – | 14.36 ± 0.76 | 22.89 ± 1.56 | -7.01 ± 2.30 | 3.90 ± 1.02 | 0.81 ± 0.08 | 69.20 ± 0.62 | 30.80 ± 0.62 | -339.75 ± 4.59 |
| 0 | branch | 8 | 1041.80 ± 0.00 | 14.36 ± 0.76 | 22.89 ± 1.56 | -7.01 ± 2.30 | 3.90 ± 1.02 | 0.81 ± 0.08 | 69.20 ± 0.62 | 30.80 ± 0.62 | -339.75 ± 4.59 |
| 100 | main | 8 | – | 15.26 ± 1.03 | 23.93 ± 1.39 | -5.20 ± 0.72 | 3.23 ± 1.68 | 0.86 ± 0.08 | 71.21 ± 2.41 | 28.79 ± 2.41 | -241.62 ± 54.42 |
| 100 | branch | 8 | 1048.90 ± 0.00 | 15.93 ± 1.08 | 24.59 ± 1.47 | -4.56 ± 0.72 | 2.60 ± 1.36 | 0.90 ± 0.09 | 71.35 ± 2.17 | 28.64 ± 2.17 | -235.50 ± 57.67 |
| 200 | main | 8 | – | 15.30 ± 0.96 | 23.93 ± 1.14 | -5.12 ± 1.04 | 3.50 ± 1.30 | 0.87 ± 0.08 | 72.02 ± 2.04 | 27.98 ± 2.04 | -133.50 ± 51.19 |
| 200 | branch | 8 | 1056.00 ± 0.00 | 16.70 ± 0.74 | 25.68 ± 0.85 | -4.06 ± 0.98 | 2.27 ± 0.57 | 0.95 ± 0.07 | 72.74 ± 2.22 | 27.26 ± 2.23 | -130.00 ± 47.84 |
| 300 | main | 8 | – | 15.74 ± 0.58 | 24.77 ± 0.64 | -5.39 ± 1.26 | 2.85 ± 0.91 | 0.90 ± 0.06 | 71.94 ± 3.31 | 28.07 ± 3.31 | -117.00 ± 55.56 |
| 300 | branch | 8 | 1063.20 ± 0.00 | 17.19 ± 0.38 | 26.35 ± 0.58 | -4.09 ± 0.83 | 1.89 ± 0.49 | 0.97 ± 0.04 | 72.07 ± 1.39 | 27.93 ± 1.39 | -138.00 ± 65.25 |
| 400 | main | 8 | – | 15.51 ± 0.55 | 24.43 ± 0.67 | -5.16 ± 1.40 | 3.14 ± 1.00 | 0.88 ± 0.05 | 71.31 ± 1.82 | 28.68 ± 1.83 | -124.25 ± 50.24 |
| 400 | branch | 8 | 1070.50 ± 0.00 | 17.70 ± 0.56 | 26.82 ± 0.73 | -3.24 ± 1.45 | 1.66 ± 0.67 | 1.00 ± 0.06 | 70.49 ± 3.01 | 29.51 ± 3.02 | -142.62 ± 61.55 |
| 500 | main | 8 | – | 15.31 ± 0.47 | 23.79 ± 0.88 | -4.89 ± 1.09 | 3.42 ± 0.72 | 0.85 ± 0.05 | 72.28 ± 2.28 | 27.75 ± 2.28 | -124.00 ± 38.17 |
| 500 | branch | 8 | 1077.90 ± 0.00 | 18.29 ± 0.49 | 27.27 ± 0.69 | -2.99 ± 1.97 | 1.18 ± 0.72 | 1.04 ± 0.04 | 71.31 ± 2.79 | 28.70 ± 2.79 | -155.75 ± 56.92 |
| 600 | main | 8 | – | 15.19 ± 0.62 | 24.04 ± 0.90 | -5.10 ± 0.81 | 4.08 ± 0.62 | 0.86 ± 0.06 | 73.81 ± 2.32 | 26.18 ± 2.32 | -107.88 ± 25.47 |
| 600 | branch | 8 | 1085.50 ± 0.00 | 18.88 ± 0.51 | 27.68 ± 0.94 | -3.30 ± 1.89 | 0.79 ± 0.56 | 1.07 ± 0.07 | 72.42 ± 2.24 | 27.56 ± 2.24 | -137.62 ± 43.61 |
| 700 | main | 8 | – | 15.25 ± 0.68 | 23.64 ± 1.33 | -5.05 ± 1.08 | 3.74 ± 0.78 | 0.85 ± 0.07 | 74.01 ± 3.41 | 26.00 ± 3.40 | -103.00 ± 49.64 |
| 700 | branch | 8 | 1093.10 ± 0.00 | 19.65 ± 0.50 | 28.44 ± 0.95 | -1.60 ± 0.81 | 0.48 ± 0.33 | 1.11 ± 0.07 | 72.86 ± 3.21 | 27.15 ± 3.20 | -116.75 ± 42.64 |
| 800 | main | 8 | – | 15.07 ± 0.38 | 23.62 ± 0.66 | -5.41 ± 1.03 | 3.79 ± 0.48 | 0.84 ± 0.04 | 72.52 ± 3.05 | 27.48 ± 3.03 | -97.38 ± 53.30 |
| 800 | branch | 8 | 1100.80 ± 0.00 | 20.26 ± 0.52 | 29.39 ± 0.98 | -1.14 ± 0.94 | 0.71 ± 0.77 | 1.18 ± 0.06 | 72.60 ± 2.89 | 27.39 ± 2.88 | -119.38 ± 68.82 |
| 900 | main | 8 | – | 14.79 ± 0.79 | 23.32 ± 1.37 | -5.66 ± 1.56 | 3.90 ± 0.74 | 0.82 ± 0.08 | 70.39 ± 2.83 | 29.62 ± 2.81 | -125.88 ± 32.10 |
| 900 | branch | 8 | 1108.70 ± 0.00 | 21.06 ± 0.58 | 30.31 ± 0.78 | -1.16 ± 1.39 | 0.78 ± 0.76 | 1.24 ± 0.07 | 70.02 ± 2.40 | 29.96 ± 2.41 | -162.12 ± 85.34 |
| 1000 | main | 8 | – | 14.74 ± 1.13 | 23.24 ± 1.50 | -5.53 ± 1.35 | 4.16 ± 1.14 | 0.83 ± 0.08 | 72.48 ± 4.77 | 27.52 ± 4.76 | -137.25 ± 68.66 |
| 1000 | branch | 8 | 1116.70 ± 0.00 | 21.46 ± 0.47 | 30.62 ± 0.70 | -0.23 ± 0.94 | 0.53 ± 0.49 | 1.25 ± 0.08 | 70.51 ± 1.79 | 29.47 ± 1.80 | -174.38 ± 112.01 |

Köppen groups of land at 1000 Myr: main A 10.8%, B 37.7%, C 16.8%, D 21.8%, E 12.9%; branch
A 15.5%, B 39.1%, C 16.4%, D 26.2%, E 2.9%.

## Assessment

- **Warming: +5.9 K after 500 Myr from today and +6.7 K after 1 Gyr from 3.5 Ga.** The extra
  absorbed sunlight is ΔS (1 − α)/4 ≈ 10.9 and 13.1 W/m², so the sensitivity is about
  0.5 K per W/m². That is plausible for this energy-balance model and a little below the ~0.8 of
  comprehensive models. The warming is smooth, with no jumps and no runaway: 19.6 °C and
  21.5 °C at the end. The equator warms as much as the poles, or more (+6.4/+5.3 K and
  +7.4/+5.3 K), because the remaining ice is small.
- **Ice**: permanent ice drops from 12% to 6% of the surface (epoch 0) and from 4% to 0.5%
  (3.5 Ga). In the Archean runs the planet is nearly ice-free after ~600 Myr, and the polar
  annual mean nears 0 °C. The polar (E) Köppen climates shrink accordingly.
- **Precipitation rises strongly**: +44% (epoch 0) and +51% (3.5 Ga), about 7%/K. That is the
  Clausius–Clapeyron rate of the model's evaporation. Comprehensive models give 2–3%/K for
  global precipitation, because the energy budget of the troposphere limits it. This is a
  property of the existing water cycle that the warming now exposes; it may be worth an issue.
- **Sea level / ocean fraction**: the differences (a few tens of metres, ±1–2% of the ocean
  area) are within the seed spread (±40–110 m). The ice sheets are only a cover (albedo); no
  water volume is taken from the ocean for them, so melting them does not raise the sea level.
- **Faint young sun at the start**: unchanged. At 3.5 Ga the start is bit-identical to main
  (14.4 °C, 3.9% ice), since the epoch's forcing exactly offsets the faint sun there. Because
  that forcing then stays fixed, an Archean run ends ~6–7 K warmer than an epoch-0 run started
  with today's sun. A weathering feedback would draw the CO₂ down instead. Making the default
  forcing follow the sun would keep the start-epoch climate, but it would also hide almost all
  of the brightening (decision for Sebastian).
- **Time 0**: main and branch agree in every number shown, for all 8 seeds of both setups, as
  expected.
