# #197 results: regolith and weathering-limited erosion

Commit 038c6c7 (`agent/issue-197`), against main at e55bf1d. All runs 1000 Myr, `--cell-dump
--snapshot-interval 100`, through `runs` (GitHub runners):

| case | seeds | this branch | main |
|---|---|---|---|
| default (barren, epoch 0) | 0–7 | r1128-012343-8c55 | r1127-232452-8033 (#65's baseline) |
| `--biosphere modern --epoch 4` | 0–3 | r1128-012343-c8e6 | r1127-232452-5022, seeds 0–3 |
| sensitivity `--set erosion.fresh_rock_erodibility=1` | 0–3 | r1128-012343-3758 | — |

Means over seeds of the 200–1000 Myr snapshots (± standard deviation over seeds) unless noted;
`agent-scratch/analyse.py` of the branch's worktree (not committed). Project goal: an Earth-like
planet, not Earth; Earth numbers are orientation only.

Model and parameters: [plan.md](plan.md) (with the changes after the review in §6,
[review-plan.md](review-plan.md)).

## 1. Barren default, 8 seeds

| quantity | main | regolith | change |
|---|---|---|---|
| calibrated φ | 1.18 ± 0.13 (0.91–1.40) | **1.37 ± 0.16 (1.16–1.71)** | +16%, inside 0.3–3 |
| global T (°C), 200–1000 Myr | 17.0 ± 0.8 | 16.5 ± 0.6 | −0.6 K |
| T at 1000 Myr | 17.9 ± 0.8 | 17.3 ± 1.3 | −0.5 K |
| permanently frozen (%) | 1.1 | 1.7 | +0.5 |
| pCO₂ (bar) | 0.48 ± 0.08 | 0.47 ± 0.07 | −1% |
| forcing (W/m²) | 61.1 | 61.0 | |
| degassing (Tmol/yr) | 12.2 | 12.4 | |
| land weathering (Tmol/yr), ramp-up (0–30 Myr) | 3.64 | 2.83 | −22% |
| land weathering, 200–1000 Myr | 3.51 ± 0.36 | **2.77 ± 0.22** | −21% |
| seafloor weathering | 8.75 | 9.67 | +10% |
| weathering intensity (used / supplied Ca+Mg) | 0.26 | 0.22 | |
| land fraction (%) | 27.8 | 28.3 | |
| **mean land elevation (m)** | 571 ± 150 | **784 ± 160** | +213 m (+37%) |
| highest point (m) | 3930 ± 240 | 4480 ± 360 | +550 m |
| land hypsometry 10/50/90/99% (m; 500 + 1000 Myr dumps, seed means) | 25/225/1690/3230 | 22/336/2640/4180 | upper half higher |
| sediment to the ocean (km³/yr) | 4.67 ± 0.69 | 4.05 ± 0.43 | −13% |
| rivers to the sea (Gt/yr) | 9.9 | 8.5 | −15% |
| continental denudation (Gt/yr) | 14.6 | 13.2 | −9% |
| continental cover stock (10⁸ Gt) | 4.95 | 4.29 | −13% |
| sediment part of the sea level (m) | 216 | 192 | −24 m |
| land runoff (m/yr) | 0.24 | 0.27 | higher land, more orographic rain |
| runtime (s, GitHub runners) | 859 ± 44 | 934 ± 110 | ~+9% (runner noise is ~±10%) |

### Regolith thickness against climate

Eroding land cells (bare fraction > 0; 500 and 1000 Myr dumps of the 8 seeds; "dry" = runoff
< 0.1 m/yr). The regolith is the rock-equivalent thickness at the end of the step (bulk ≈ 1.3×).

| climate | cells | regolith 10/50/90% (m) | mean bare fraction | P₀ median (m/Myr) | bedrock lowering 10/50/90% (m/Myr) |
|---|---|---|---|---|---|
| cold < 0 °C, dry | 6143 | 0.04 / 0.28 / 0.92 | 0.47 | 11 | 0.4 / 6 / 38 |
| cold < 0 °C, wet | 11078 | 0.00 / 0.12 / 0.55 | **0.66** | 28 | 3 / 18 / 106 |
| 0–15 °C, dry | 32675 | 0.11 / 0.40 / 0.85 | 0.39 | 11 | 0.8 / 4 / 19 |
| 0–15 °C, wet | 32802 | 0.10 / 0.34 / 0.78 | 0.43 | 41 | 5 / 20 / 112 |
| warm ≥ 15 °C, dry | 4816 | 0.18 / 0.58 / 1.14 | 0.27 | 14 | 0.2 / 4 / 26 |
| warm ≥ 15 °C, wet | 9640 | 0.20 / **0.57** / 1.07 | **0.28** | **80** | 5 / 20 / 123 |
| all eroding land | 97154 | median 0.36 | 0.42 | | |

- A barren planet gets a thin rubble mantle: decimetres to ~1 m, thickest in warm climates
  (fast production) and in dry, slowly eroding lowlands; thinnest in cold, wet, steep land
  where rivers cut faster than the rock weathers (2/3 of the time bare rock). Plausible for a
  planet without soils; Earth's soil-mantled hillslopes have 0.3–1 m of soil on top of thicker
  saprolite, which the exponential production law does not make (stated in the plan).
- Depositional land (alluvium, deltas, basins) has no regolith label: its rock is under the
  deposit, which does not weather further in the model.

### Regimes

Per snapshot step (200–1000 Myr means): of ~12 000 land cells, 3770 eroding cells end
transport-limited (production faster than transport: regolith ≥ H* ln 2) and 2420
weathering-limited; the rest are covered by deposits all step (no production under them).
The weathering-limited cells are the fast-eroding ones and lose **78%** of all bedrock, the
transport-limited ones 22% (mean lowering ~15 m/Myr vs ~57 m/Myr per cell). Erosion through
flat or deposit-covered cells is nil (production under more than a few metres of deposit is
~0).

### Relief and sensitivity to the fresh-rock erodibility f

| quantity (seeds 0–3) | main | f = 0.25 (default, 8 seeds) | f = 1 |
|---|---|---|---|
| mean land elevation (m) | 575 | 784 | 548 |
| highest point (m) | 4050 | 4480 | 4050 |
| φ | 1.10 | 1.37 | 1.33 |
| land weathering (Tmol/yr) | 3.53 | 2.77 | 2.89 |
| sediment to the ocean (km³/yr) | 4.96 | 4.05 | 4.77 |
| global T (°C) | 17.5 | 16.5 | 17.4 |

The relief change comes from f, the land-weathering change from the regolith cover (both f
values lower the land sink by ~20%). With f = 1 (fresh rock as erodible as weathered rock) the
landscape is that of main, as designed: the transport-limited limit equals the old law. With the
default f = 0.25 the land is ~200 m higher on average: bare highlands erode at `P₀ + f(T) T`
instead of `T`; the fastest rivers (f(T) → 1 above ~0.5 mm/yr) are barely affected, so the
highest peaks rise only ~10%. The barren main is low (571 m; the barren preset doubles `K`); the
regolith brings it to 784 m, Earth's mean land elevation is ~840 m.

### Carbon cycle

- **φ stays inside 0.3–3**: 1.16–1.71 (main 0.91–1.40), never clamped. The stop rule of the plan
  (φ > 2.5) is far away.
- The land sink is ~21% smaller at the same CO₂ (the kinetic rate on partly bare rock, mean bare
  fraction 0.42 → factor ~0.62 on the eroding cells, and ~10% less supply). The calibration
  raises φ, which also raises the seafloor sink, so the sum balances the same degassing: the CO₂
  ends at the same level (0.47 vs 0.48 bar). The planet is 0.5 K colder on average, within the
  seed spread (±0.6–0.8 K); higher land (lapse rate) and 0.5% more permanent ice are a plausible
  part of it. The intensity drops
  from 0.26 to 0.22 (bare rock: more kinetic-limited, West's thin-regolith regime).
- The land share of the sink falls from ~29% to ~22%; seafloor weathering dominates more, at
  the upper end of the Archean models (plan of #154, phi-barren.md §2).
- Supply: unchanged definition (bedrock lowering incl. regolith production); the regolith
  removed later is not counted as eroded sediment again.

## 2. Modern (`--biosphere modern --epoch 4`), seeds 0–3

| quantity | main | regolith |
|---|---|---|
| φ | 3 (clamped; balancing 3.5–8.2) | 3 (clamped) |
| global T (°C), 200–1000 Myr | 22.1 ± 0.6 | 23.5 ± 0.4 |
| pCO₂ (µbar) | 505 | 716 |
| forcing (W/m²) | 2.4 | 4.4 |
| land weathering (Tmol/yr) | 5.66 | 4.49 |
| seafloor (Tmol/yr) | 4.23 | 5.23 |
| mean land elevation (m) | 836 ± 340 | 840 ± 410 |
| highest point (m) | 4250 | 4100 |
| sediment to the ocean (km³/yr) | 3.7 | 2.8 |
| regime split of bedrock lowering, transport/weathering-limited | — | 52% / 48% |
| runtime (s) | 983 | 1112 |

The modern preset was already clamped at φ = 3; the regolith factor lowers its land sink at the
start further, so after the ramp-up the CO₂ settles ~40% higher and the planet ~1.4 K warmer than
on main. This is a property of the clamped modern preset (its degassing is ~2× Earth's, see
[phi-modern.md](../issue-154/phi-modern.md)), not of the barren default. On a planet with
plants, the regolith is thicker and held in place by roots (#198, not in this PR): there the
covered fraction should rise towards 1 and this extra warming would largely go away. The modern
relief is unchanged within the (large, 4-seed) spread.

## 3. Mass conservation

Regolith is stored in the sediment layer; the conversion of bedrock into regolith books
`bedrock_delta < 0` and `sediment_delta > 0` of the same mass, as eroded bedrock does. Unit
tests: `regolith_conserves_rock_mass` (three climates, relative error < 10⁻⁹),
`untransported_regolith_grows_logarithmically` (production on flat land stays in place),
the exact ODE solution against an explicit integration, the steady-state thickness, and
`regolith_off_keeps_the_old_model`. The runs' crust-mass and sea-level budgets close as on main
(`unaccounted` and `unexplained` terms at rounding). `--set erosion.regolith=0` reproduces main's
state hashes (checked, seed 1, 4 subdivisions, 20 Myr).

## 4. Caveats

- **Grid:** "weathering-limited" here means the cell-mean bedrock lowering is limited by
  regolith production. On a 110 km cell the regolith is a cell mean, too; hillslope thresholds,
  landslides and the sub-grid split between channels and hillslopes are parameterised by
  `f(T)` (0.25 → 1 around 0.5 mm/yr), not resolved (#64).
- The regolith is thin because the exponential (Heimsath) law makes no deep saprolite. A thicker
  weathering zone would raise the covered fraction and the land sink.
- The regolith label stays with the cells while the plates move (~half a cell per step); it is
  clamped to the sediment present, so this can only mislabel, never create or lose mass.
- Deposited sediment does not weather further (no production under > a few metres of cover);
  its weathering is the existing `sediment_weatherable` term.
- Wave erosion and sediment diffusion do not move the regolith (diffusion is kept for the
  deposits above it); cliff retreat can remove it.
- The modern preset gets warmer (above), and #198 (plants) should revisit both production and
  cover for it.

## 5. Decisions for Sebastian

1. **f = 0.25** (fresh rock 4× less erodible than weathered rock; Murphy et al. 2016, Sklar &
   Dietrich 2001) raises the mean land by ~200 m; f = 1 keeps main's relief and still gives the
   regolith and the carbon coupling. Default: 0.25 (the issue's point: stream power overestimates
   erosion where weathering is slow).
2. The kinetic weathering factor on bare rock, **b = 0.1** (`carbon.bare_rock_weathering`), is
   the main lever of the land sink change (−21%). b = 1 would switch the carbon coupling off
   (supply only).
