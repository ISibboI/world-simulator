# #197 results: regolith and weathering-limited erosion

Branch `agent/issue-197` against main at e55bf1d. Every run is 1000 Myr with
`--cell-dump --snapshot-interval 100` and went through `runs` on GitHub runners.

| case | seeds | this branch | main |
|---|---|---|---|
| default (barren, epoch 0) | 0–7, twice | 038c6c7 (r1128-012343-8c55) and the final 792a095 (r1128-015454-75ae) | r1127-232452-8033 (#65's baseline) |
| `--biosphere modern --epoch 4` | 0–3 | 038c6c7 (r1128-012343-c8e6) | r1127-232452-5022, seeds 0–3 |
| sensitivity `--set erosion.fresh_rock_erodibility=1` | 0–3 | 038c6c7 (r1128-012343-3758) | — |

The final commit 792a095 changes only the carbon cycle's bookkeeping of the regolith label (the
evaluation's m1); it does not change the erosion. After the ramp-up its runs diverge chaotically
from those of 038c6c7. The two 8-seed ensembles therefore count as **16 seeds** of the same model.
The regolith-by-climate table uses the final ensemble. Unless noted, the numbers are seed means of
the 200–1000 Myr snapshots, ± the standard deviation over seeds, made with
`agent-scratch/analyse.py` (in the worktree, not committed).

Project goal: an Earth-like planet, not Earth. Earth numbers are orientation only.

The model and its parameters are in [plan.md](plan.md); §6 lists the changes after the review in
[review-plan.md](review-plan.md). The independent evaluation of these results is in
[review-results.md](review-results.md), and its fixes are applied below.

## 1. Barren default

| quantity | main (8 seeds) | regolith (16 seeds) | change |
|---|---|---|---|
| calibrated φ | 1.18 ± 0.13 (0.91–1.40) | **1.37 ± 0.16 (1.16–1.71)** | +16%, inside 0.3–3 and never clamped |
| global T (°C), 200–1000 Myr | 17.0 ± 0.8 | 16.5 ± 0.5 | −0.6 K |
| T at 1000 Myr | 17.9 ± 0.8 | 17.2 ± 1.1 | −0.7 K |
| permanently frozen (%) | 1.1 | 1.7 | +0.5 |
| pCO₂ (bar) | 0.48 ± 0.08 | 0.46 ± 0.07 | −4% |
| forcing (W/m²) | 61.1 | 60.7 | −0.4 |
| degassing (Tmol/yr) | 12.2 | 12.3 | |
| land weathering, ramp-up 0–30 Myr (Tmol/yr) | 3.64 | 2.83 | −22% |
| land weathering, 200–1000 Myr (Tmol/yr) | 3.51 ± 0.36 | **2.79 ± 0.22** | −21% |
| seafloor weathering (Tmol/yr) | 8.75 | 9.60 | +10% |
| weathering intensity (dissolved / supplied Ca+Mg) | 0.26 | 0.23 | |
| land fraction (%) | 27.8 | 28.2 | |
| **mean land elevation (m)** | 571 ± 150 | **754 ± 290** | **+183 m (+32%)** |
| land hypsometry 10/50/90/99% (m; seed means of the 500 + 1000 Myr dumps) | 25 / 225 / 1690 / 3230 | 18 / 398 / **2440** / 3930 | P90 **+44%** |
| highest point (m) | 3930 ± 240 | 4400 ± 510 | +12%, a noisy statistic |
| sediment to the ocean (km³/yr) | 4.67 ± 0.69 | 3.99 ± 0.61 | −15% |
| rivers to the sea (Gt/yr) | 9.9 | 8.3 | −16% |
| continental denudation (Gt/yr) | 14.6 | 13.1 | −11% |
| continental cover stock (10⁸ Gt) | 4.95 | 4.37 | −12% |
| sediment part of the sea level (m) | 216 | 193 | −23 m |
| land runoff (m/yr) | 0.24 | 0.27 | higher land gets more orographic rain |

**Runtime.** GitHub runners give 859 ± 44 s for main and 934 ± 110 s and 1164 ± 43 s for the
branch's two ensembles; the runner hardware differs between batches. On tktie, side by side
(seed 3, 80 Myr, one core each), the times are 77 s with the regolith and 78 s on main: **no
measurable cost**.

The −0.6 K comes from the higher land at the same forcing (−0.4 W/m²). 183 m × 6.5 K/km over
~28% land is ≈ 0.35 K; the lower forcing adds ~0.2 K. The seed spread is ±0.5–0.8 K.

### Regolith thickness against climate

These are the eroding land cells (bare fraction > 0, about half the land) in the 500 and 1000 Myr
dumps of the final 8 seeds. "Dry" means runoff below 0.1 m/yr. The regolith is the rock-equivalent
thickness at the end of the step; its bulk thickness is ≈ 1.3× that.

| climate | cells | regolith 10/50/90% (m) | mean bare fraction | P₀ median (m/Myr) | bedrock lowering 10/50/90% (m/Myr) |
|---|---|---|---|---|---|
| cold < 0 °C, dry | 7055 | 0.04 / 0.29 / 0.86 | 0.48 | 11 | 0.8 / 6 / 36 |
| cold < 0 °C, wet | 16336 | 0.00 / 0.11 / 0.54 | **0.67** | 25 | 4 / 18 / 115 |
| 0–15 °C, dry | 27185 | 0.11 / 0.39 / 0.85 | 0.39 | 12 | 0.8 / 5 / 20 |
| 0–15 °C, wet | 29500 | 0.11 / 0.35 / 0.79 | 0.42 | 42 | 5 / 20 / 112 |
| warm ≥ 15 °C, dry | 5764 | 0.20 / 0.55 / 1.04 | 0.28 | 22 | 0.7 / 5 / 26 |
| warm ≥ 15 °C, wet | 11032 | 0.18 / **0.47** / 1.00 | **0.32** | **82** | 5 / 27 / 126 |
| all eroding land | 96872 | median 0.35 | 0.44 | | |

- The barren planet gets a thin rubble mantle, from decimetres to about 1 m. On eroding rock it is
  thickest in warm climates, where production is fast, and in dry lowlands that erode slowly. It
  is thinnest on cold, wet, steep land, where rivers cut faster than the rock weathers; that land
  is bare two thirds of the time.
- This is plausible for a planet without soils. Earth's soil-mantled hillslopes carry 0.3–1 m of
  soil over thicker saprolite, and the exponential (Heimsath) production law makes no saprolite
  (stated in the plan).
- The maximum is ~2.7 m, with no runaway thickness, no negative values and no NaN.
- Depositional land (alluvium, deltas, basins) has no regolith label, because its rock lies under
  the deposit. Counted over all land, the median regolith is 0 in the warm bands; "thickest in
  warm climates" holds only for eroding rock.

### Regimes

These are means of the 200–1000 Myr snapshots of the final ensemble. The log counts 10 600 land
cells with a receiver: 3610 eroding cells end transport-limited and 2290 end weathering-limited.
The rest (~4 400) are covered by deposits all step, and no rock is produced under them.

| regime | cells | bedrock lowering per cell | share of all bedrock lowering |
|---|---|---|---|
| transport-limited | 3610 | ~13 m/Myr (median ~11) | 22% |
| weathering-limited | 2290 | ~78 m/Myr (median ~69) | 78% |

"Weathering-limited" here only means that P₀ is below the transport capacity at the end of the
step, i.e. bare rock. Many of these cells are fast rivers where fresh rock erodes almost at the old
rate: `f(T)` rises from 0.25 towards 1 around 0.5 mm/yr and is already ~0.36 at 75 m/Myr. With
f = 1 the label limits nothing.

### Relief and sensitivity to the fresh-rock erodibility f

All columns use seeds 0–3; the f = 0.25 column is the 038c6c7 ensemble.

| quantity (seeds 0–3) | main | f = 0.25 (default) | f = 1 |
|---|---|---|---|
| mean land elevation (m) | 575 | 733 | 548 |
| highest point (m) | 4050 | 4260 | 4050 |
| φ | 1.10 | 1.31 | 1.33 |
| land weathering (Tmol/yr) | 3.53 | 2.73 | 2.89 |
| sediment to the ocean (km³/yr) | 4.96 | 3.87 | 4.77 |
| global T (°C) | 17.5 | 16.8 | 17.4 |

- **The relief change comes from f.** With f = 1 the landscape is about that of main: the
  transport-limited limit equals the old law. Bare rock then erodes at `P₀ + r x`, slightly faster
  than main, hence the 27 m lower mean.
- **The land-weathering change comes from the regolith cover.** Both values of f lower the land
  sink by ~20%.
- **f = 0.25 raises the upper half of the land.** Bare highlands erode at `P₀ + f(T) T` instead of
  `T`. The 90th percentile of land rises from 1690 to 2440 m: 10% of the land lies above ~2.4 km,
  where Earth has ~2 km.
- **The fastest rivers barely change**, because f(T) → 1 above ~0.5 mm/yr. The highest point rises
  by only +5% (seeds 0–3) to +12% (16 seeds).
- The barren main is low at 571 m, because the barren preset doubles `K`; the regolith brings it to
  ~750 m. Earth's mean land elevation is ~840 m.

### Carbon cycle

- **φ stays inside 0.3–3.** It ranges 1.16–1.71 (main 0.91–1.40) and is never clamped. The plan's
  stop rule (φ > 2.5) is far away.
- **The land sink is ~21% smaller at the same CO₂:**
  - partly bare rock lowers the kinetic rate: with a mean bare fraction of 0.44 on the eroding
    cells, the factor is 0.1 + 0.9 × 0.56 ≈ 0.6 there;
  - the supply is ~10% smaller.
- **The calibration compensates.** It raises φ, which also raises the seafloor sink, so the sinks
  balance the same degassing and the CO₂ ends about where it was (0.46 vs 0.48 bar). The land
  share of the sink falls from 29% to 22%; seafloor weathering dominates more, at the upper end of
  the Archean models (#154 plan, phi-barren.md §2).
- **Weathering intensity** drops from 0.26 to 0.23. Bare rock makes the land more kinetic-limited,
  which is West's (2012) thin-regolith regime.
- **Stability** (from the evaluation): step-to-step pCO₂ noise is the same as on main (rms Δ ln
  pCO₂ 0.0032). The land sink's step-to-step noise is ~40% higher (relative SD 0.029 vs 0.021),
  with no oscillation.
- **No double counting.** The supply has the same definition as before: bedrock lowering,
  including regolith production. Regolith removed in a later step is not counted again as eroded
  sediment; since 792a095 this uses the start label as the erosion step clamped it.

## 2. Modern (`--biosphere modern --epoch 4`), seeds 0–3, 038c6c7

| quantity | main | regolith |
|---|---|---|
| φ (clamped) / balancing φ | 3 / 3.7–8.7 | 3 / 4.8–10.3 |
| global T (°C), 200–1000 Myr | 22.1 ± 0.6 | 23.5 ± 0.4 |
| pCO₂ (µbar), mean of 200–1000 Myr | 505 | 716 |
| forcing (W/m²) | 2.4 | 4.4 |
| land weathering (Tmol/yr) | 5.66 | 4.49 |
| seafloor (Tmol/yr) | 4.23 | 5.23 |
| mean land elevation (m) | 836 ± 340 | 840 ± 410 |
| highest point (m) | 4250 | 4100 |
| sediment to the ocean (km³/yr) | 3.7 | 2.8 |
| bedrock lowering, transport- / weathering-limited | — | 52% / 48% |

- **The modern preset was already clamped at φ = 3.** The balancing φ rises from 3.7–8.7 to
  4.8–10.3, so the clamp is now further from balance.
- **The CO₂ cannot be compensated.** With φ fixed, the smaller land sink raises CO₂ and temperature
  until seafloor plus land balance the degassing.
- **The CO₂ never settles.** On both branches it drifts down 3–6× from 200 to 1000 Myr (e.g. seed 0:
  6.4 → 1.7 PAL here, 4.2 → 0.7 on main). Along that drift it is ~40% higher on average, and the
  planet is 1.4 K warmer.
- **This belongs to the clamped modern preset,** whose degassing is ~2× Earth's (see
  [phi-modern.md](../issue-154/phi-modern.md)), not to the barren default. With plants (#198, not
  in this PR) the regolith is thicker and roots hold it in place. The covered fraction should then
  approach 1, and most of this warming should go away.
- **The modern relief does not change** within the large 4-seed spread. The modern preset does not
  double `K`, so its rivers are slower and more cells are transport-limited (52/48 vs 22/78).

## 3. Mass conservation

- **Storage.** The regolith lives in the sediment layer. Turning bedrock into regolith books
  `bedrock_delta < 0` and `sediment_delta > 0` of the same mass, exactly as eroded bedrock does.
- **Unit tests:**
  - `regolith_conserves_rock_mass` (three climates, relative error < 10⁻⁹);
  - `untransported_regolith_grows_logarithmically` (production on flat land stays in place);
  - the exact ODE solution against an explicit integration;
  - the steady-state thickness;
  - `regolith_off_keeps_the_old_model`;
  - the carbon test `bare_rock_slows_the_kinetics`.
- **Budgets in the runs.** Every `unaccounted` (Gt/yr) and `unexplained` (m) term in the runs'
  crust-mass and sea-level budgets is ≤ 1.4 × 10⁻¹³, as on main.
- **Switch.** `--set erosion.regolith=0` reproduces main's state hashes (checked for seed 1, 4
  subdivisions, 20 Myr).

## 4. Caveats

- **Grid.** "Weathering-limited" means that regolith production limits the cell-mean bedrock
  lowering. On a 110 km cell the regolith is a cell mean too. Hillslope thresholds, landslides and
  the split between channels and hillslopes below the cell size are parameterised by `f(T)`, not
  resolved (#64).
- **No deep weathering zone.** The regolith is thin because the exponential law makes no saprolite.
  A thicker weathering zone would raise the covered fraction and the land sink.
- **Labels stay with the cells while the plates move** (~half a cell per step). A label is clamped
  to the sediment present at the start of the next step, so it can mislabel but never create or
  lose mass.
- **The dumps are written before that clamp.** About 1.5% of land cells show regolith above their
  sediment, and some coastal cells that just flooded show regolith or bedrock lowering.
- **Deposited sediment does not weather further.** There is no production under more than a few
  metres of cover; the weathering of deposits is the existing `sediment_weatherable` term.
- **What moves the regolith.** Sediment diffusion does not: it acts only on the deposits above.
  Cliff retreat can remove it.
- **Ice.** Permanent ice stops production, but rivers keep eroding fresh rock under it, as before
  (the model has no glacial erosion). The fastest bedrock lowering, 5 mm/yr, is on such a cell.
- **Modern preset.** It gets warmer (§2); #198 (plants) should revisit both production and cover
  for it.
- **Overlap with open PRs.** #221 (volcanism, mafic crust) touches `src/carbon.rs` and
  `src/main.rs`, and its mafic crust feeds the regolith's lithology factor. #220 touches
  `src/main.rs`. The overlaps are textual; there is no semantic conflict.

## 5. Decisions for Sebastian

1. **Fresh-rock erodibility f = 0.25.** Fresh rock is 4× less erodible than weathered rock
   (Murphy et al. 2016; Sklar & Dietrich 2001). This raises the mean land by ~180 m and the 90th
   percentile by ~750 m (1690 → 2440 m). f = 1 keeps main's relief and still gives the regolith and
   the carbon coupling. The default is 0.25, because that is the issue's point: the stream-power law
   overestimates erosion where weathering is slow.
2. **Bare-rock weathering factor b = 0.1** (`carbon.bare_rock_weathering`). It is the main lever of
   the −21% land sink. b = 1 would switch off the carbon coupling, leaving the supply only.
