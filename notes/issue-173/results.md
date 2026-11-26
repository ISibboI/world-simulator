# Results: lakes as open water in the climate, 0.2% rule off (#173)

_2026-11-26. Issue: [#173](https://github.com/ISibboI/world-simulator/issues/173). Plan:
[plan.md](plan.md), plan review: [plan-review.md](plan-review.md), code review:
[code-review.md](code-review.md). Full table: [table.md](table.md) (script
[scripts/compare.py](scripts/compare.py); case studies [scripts/cases.py](scripts/cases.py),
[scripts/lakeocean.py](scripts/lakeocean.py)). Goal: an Earth-like planet, not Earth; grid
effects (~110 km cells, ~440 km for the temperature model) accepted (#64)._

## What changed

1. **Lake columns in the temperature model** (`temperature.open_water_lakes`, default on). Each
   coarse cell (~440 km) has a lake column beside its land surface and its ocean column, with the
   ocean column's physics (one shared `WaterColumn`: mixed layer or ice after Wagner & Eisenman,
   the open-water and sea-ice albedos), but:
   - heat capacity of the lakes' own mixed layer: their mean depth, between ~2.4 m (the land's
     heat capacity) and the ocean's 50 m;
   - fresh water freezing at 0 °C at the water surface (in the model's sea-level temperatures:
     at the lapse rate times the mean water-surface elevation of the cell's lakes);
   - no ocean heat transport (no Ekman cells, no gyres).

   All lake cells (≥ 1 m of water, as in the view and the climate classes) count; a lake's
   weight is its area share of the coarse cell, so small lakes get a water temperature of their
   own with little effect around them, while large lakes and cut-off seas dominate their coarse
   cells. Land, ocean and lakes exchange heat through the air by area share, and the lakes' share
   enters the air temperature and the meridional transport: the maritime effect on the land
   around them. Fine lake cells interpolate the lake columns of the lake-bearing coarse cells;
   they get lake ice (`ice`) and a monthly lake-ice field. A column whose lakes change keeps its
   water temperature (code-review fix).
2. **Water cycle:** the lake potential evaporation uses the water surface temperature (bulk
   formula with the actual air humidity) and the lake ice of the temperature model (open share
   of the month) instead of the −2…+2 °C ramp; the lake evaporation still returns to the air in
   full. Lake cells sit at their water surface for the lapse rate and the saturation column.
3. **0.2% rule off by default** (`min_sea_fraction` = 1, `crate::sea::NO_SIZE_RULE`);
   `--set min_sea_fraction=0.002` restores it.
4. Viewer: low dry depressions still take the air of the land around them; lakes keep the
   model's temperatures. Diagnostic `--lake-climate-contrast` (below).

With `--set temperature.open_water_lakes=0 --set min_sea_fraction=0.002` the run is bit-identical
to main (state hashes of seed 0 at 0–150 Myr equal those of the `main` run set; same with the rule
off against `main-nosea`).

## Runs

8 seeds × 1000 Myr, 6 subdivisions, means over snapshots 500–1000 Myr, then over seeds (range);
Welch t against `main` unless stated.
- `main`: e7d2a14 (rule on); `main-nosea`: e7d2a14 with `--set min_sea_fraction=1`;
- `lakes-rule`: this work (b354606) with `--set min_sea_fraction=0.002`; `lakes`: this work with
  its defaults (rule off).

| metric | main | main-nosea | lakes-rule | lakes | t lakes vs main |
|---|---|---|---|---|---|
| endorheic (% of land) | 6.9 (2.2…12.5) | 9.2 (3.3…19.6) | 6.4 (2.7…9.4) | 12.6 (7.6…21.3) | +2.4 |
| lake cells (% of land) | 1.7 | 3.5 | 1.8 | 4.8 (2.5…7.7) | +4.4 |
| lake water (m of sea level) | 2.4 (0.1…7.8) | 12.6 (0.8…41) | 2.0 (0.2…5.3) | 24.2 (6.5…73) | +2.6 |
| lake water, largest snapshot (m) | 12 (0.2…72) | 92 (5…314) | 7.6 (0.4…24) | 138 (25…388) | |
| snapshots with > 10 m lake water (of 11) | 0.1 | 1.8 | 0.3 | 3.3 (2…4) | |
| largest lake (cells) | 53 | 259 | 67 | 375 (143…861) | |
| deficit after the sea-level solve (m; largest) | 0.02; 0.14 | 0.03; 0.3 | 0.004; 0.02 | 0.013; 0.11 | |
| lake |Jul − Jan| (K) | 32.3 | 37.1 | 24.9 | 20.6 (11.8…27.1) | −5.1 |
| large lakes (≥ 20 cells): |Jul − Jan| minus land at their latitudes (K) | +3.6 | +5.2 | −7.1 | −13.1 | −10.1 |
| land next to large lakes: |Jul − Jan| anomaly (K) | +3.4 | +4.2 | −2.7 | −5.9 | −9.8 |
| land 2–3 cells from large lakes: |Jul − Jan| anomaly (K) | +3.0 | +3.4 | −1.4 | −3.4 | −7.7 |
| lake cells with permanent ice (%) | – | – | 11.7 | 11.6 | |
| lake potential evaporation (m/yr) | 1.81 | 2.20 | 1.81 | 1.82 | +0.1 |
| precipitation on lake cells (m/yr) | 1.10 | 1.41 | 1.10 | 1.20 | +1.2 |
| land next to large lakes: precipitation anomaly (m/yr) | +0.01 | +0.28 | +0.09 | +0.09 | +1.0 |
| global mean temperature (°C) | 22.8 | 22.9 | 22.9 | 22.7 | −0.3 |
| permanently frozen surface (%) | 3.4 | 3.7 | 3.6 | 3.8 | +0.5 |
| land without lakes: precipitation (m/yr) | 1.21 | 1.28 | 1.22 | 1.18 | −0.5 |
| land without lakes: arid, P < 0.25 m/yr (%) | 18.4 (7.7…25.8) | 16.3 | 17.8 | 21.3 (14.5…28.5) | +1.0 |
| Köppen B (% of land) | 31.6 | 28.2 | 30.6 | 32.3 | +0.3 |
| land p50 / p90 (m) | 533 / 1960 | 486 / 1950 | 481 / 1820 | 552 / 2060 | +0.1 / +0.4 |

("Anomaly": against land farther away in the same 5° latitude band of the same snapshot; seeds
diverge chaotically after the first changed step, so only such within-snapshot contrasts and the
paired diagnostic compare lakes between sets.)

### Paired diagnostic (same state, lakes as water vs as land)

`--lake-climate-contrast` runs the temperature model again at every snapshot from the same state,
20 years with lakes as open water and 20 years as land. Means over snapshots and seeds (range):

| cells | set | seasonal range water / land (K) | annual mean water / land (°C) | ice > 0.5 water / land (%) |
|---|---|---|---|---|
| all lakes | lakes | 22.3 / 40.0 | 14.6 / 15.7 | |
| large lakes (≥ 20 cells) | lakes | 20.4 (14…25) / 43.0 (34…56) | 12.9 / 13.8 | 12.9 / 6.2 |
| land next to them | lakes | 26.0 (22…32) / 41.4 (33…53) | 11.6 / 11.5 | |
| land 2–3 cells from them | lakes | 27.7 / 40.3 | | |
| large lakes | lakes-rule | 26.9 / 40.4 | 9.2 / 9.9 | 17.6 / 8.0 |
| land next to them | lakes-rule | 30.8 / 39.6 | 8.2 / 8.1 | |

- **Lakes have the seasonal inertia of water:** the seasonal range of large lakes halves (43 →
  20 K; Earth's large mid-latitude lakes ~20–25 K at the surface, the Caspian ~25 K). Their
  annual mean changes little (−1 K: ice albedo in winter, no summer heating of the land).
- **Maritime climate around them:** land next to large lakes swings 15 K less over the year, land
  2–3 cells away 13 K less, at ~440 km resolution of the temperature model (the effect reaches as
  far as a coarse cell; on Earth the lake effect is strongest within ~50–100 km — a grid effect).
- **Lake ice:** large lakes carry permanent ice (cover > 0.5 over the year) twice as often as the
  land ice sheets the same cells got as land; 12% of all lake cells are ice-covered most of the
  year.

### Cut-off seas (case studies, `lakes`, snapshots with > 10 m lake water)

| seed, time | cells | latitude | Jan / Jul (°C) | ice | P / E (m/yr) |
|---|---|---|---|---|---|
| 0, 650 Myr (south polar sea) | 1811 | −60° | 7.7 / −7.2 | 29% | 0.67 / 0.43 |
| 0, 700 Myr | 844 | −64° | 6.1 / −16.3 | 39% | 0.64 / 0.25 |
| 4, 750 Myr (north polar sea) | 1609 | +62° | −3.4 / 10.1 | 22% | 1.01 / 0.78 |
| 5, 500 Myr (south polar sea) | 794 | −74° | 6.7 / −20.1 | 49% | 0.63 / 0.24 |
| 3, 850 Myr (subtropical sea) | 3826 | −31° | 28.3 / 25.1 | 0 | 3.57 / 4.06 |
| 1, 850 Myr (equatorial sea) | 2712 | −4° | 36.3 / 35.8 | 0 | 4.77 / 6.24 |

Before (`main-nosea`), the same kind of basins were land for the temperature model: the south
polar sea of seed 7 at 800 Myr had **+48.8 °C in January and −21.2 °C in July**, P 2.5 and
E 2.6 m/yr (a hot summer surface as a moisture source); the north polar one of seed 5 at
1000 Myr +47.9 / −9.2 °C. Now polar cut-off seas have a 15–25 K seasonal cycle, ice in winter,
and evaporate 0.2–0.8 m/yr against 0.6–1 m/yr of precipitation.

## The 0.2% rule — verdict: dropped (default off)

Criteria (written before the runs, [plan.md](plan.md)): (a) water conserved, (b) large closed
basins open water in the climate with plausible temperatures, (c) global temperature, frozen
share, land precipitation, aridity, Köppen B within the seed scatter of the rule-on sets.

- (a) Deficit after the sea-level solve 0.013 m of sea level on average, at most 0.6 m (main
  with the rule: 0.02 / 0.5). ✔
- (b) Seasonal range of large lakes 20 K instead of 43 K, polar cut-off seas ice-covered in winter
  instead of +49 °C summers (above). ✔
- (c) Global mean 22.7 vs 22.8 °C, frozen 3.8 vs 3.4%, land precipitation 1.18 vs 1.21 m/yr,
  arid land 21.3 vs 18.4% (t +1.0), Köppen B 32.3 vs 31.6% (t +0.3). ✔ (Against `main-nosea`
  the land is drier: arid +5 points, t +2.0, because the old land-temperature lakes of cut-off
  seas were a hot-summer moisture source, 2.2 vs 1.8 m/yr of lake potential evaporation.)

**What remains without the rule** (by design, not a defect): cut-off ocean basins of hundreds to
thousands of cells hold more water than before — lake water 24 m of sea level on average (6…73
per seed), the largest snapshot 25…388 m, more than 10 m in 3 of 11 snapshots; endorheic land
12.6% (7.6…21.3; Earth ~18–20%, with the rule 6.9%). The main reason is physical: an ice-covered
polar sea evaporates little (0.2–0.4 m/yr) and fills from its own rain and rivers to its sill, as
the Arctic Ocean would if it were closed; before, its land temperatures made it evaporate as much
as it received. On Earth such a basin would almost certainly keep a strait narrower than a cell
(#64): at ~110 km cells the model only knows a connection if one cell of it is below sea level.

If such long-lived cut-off seas are unwanted, the minimal fix is not a size rule but a sub-grid
strait: let a sill within a few hundred metres of sea level connect a basin below sea level
(#168 discussed it; at this resolution the sills of real straits and real closed basins overlap,
so it would be a tuning parameter). The old rule stays available as
`--set min_sea_fraction=0.002`.

## Water conservation

Unchanged by the climate side: the lake evaporation that the water balance removes returns to the
air in full (unit test with lake ice of 0, ½ and 1; the monthly weights still average to one).
Deficits above.

## Runtime (6 subdivisions)

Paired local runs (seed 0, 200 Myr, one core each, at the same time): switch off 106.5 s vs on
108.2 s (+1.6%, both with the rule); off with the rule 131.0 s vs the new defaults 134.2 s
(+2.4%). The run-set times mix local and GitHub machines and include the paired diagnostic (40
extra temperature updates per snapshot), so they are not comparable.

## Caveats and follow-ups

- **Large tropical lakes are too warm and evaporate too much.** A lake column has the ocean's low
  open-water albedo but neither the ocean's heat export by currents nor the latent heat that its
  net evaporation carries away (the energy balance model has no explicit latent heat). Large
  lakes within 30° of the equator are on average 5 K colder than the ocean at their latitude
  (many are high), but the warmest 10% are 13 K warmer; in hot epochs (solar constant ~1470
  W/m² at 850 Myr) the equatorial cut-off seas reach 36–41 °C and evaporate 6–8 m/yr, more than
  the energy available (before: land temperatures, ~4 m/yr). Their balance is negative, so they
  do not fill. A proper fix couples the lake's evaporation into the column's energy balance (or
  limits potential evaporation by the available energy, which land lacks too) — a follow-up
  issue.
- A basin cut off from the ocean in a step is land for the climate in that step (the lakes are
  known from the end of the previous step, when it was still ocean); from the next step on it is a
  lake. Seen in one snapshot (seed 0, 600 Myr).
- Lake ice in the viewer still uses its own temperature rule; it could use the model's lake ice
  (follow-up). Ocean evaporation is not reduced under sea ice, lake evaporation is.
- Lake ice is capped at 10 m (as sea ice, where thick ice drifts away); on a lake the cap stands
  in for melting from below.
- Partly covered shore cells count as whole lake cells (≥ 1 m of water), as in the view.
- A fine lake cell whose height differs from its coarse cell's mean lake height can show open
  water slightly below 0 °C or ice slightly above (the freezing point is per coarse cell).
