# Results: sea and lakes spill into neighbouring dry basins (#168)

_2026-11-25. Request: [#168 comment](https://github.com/ISibboI/world-simulator/pull/168#issuecomment-7171197285).
Plan: [spill-plan.md](spill-plan.md), plan review: [spill-plan-review.md](spill-plan-review.md),
code review: [spill-code-review.md](spill-code-review.md). Full table:
[table-spill.md](table-spill.md) (script [scripts/compare.py](scripts/compare.py)). Goal: an
Earth-like planet, not Earth; grid effects (~110 km cells) accepted (#64)._

## What changed

Three changes, each behind a switch (all off reproduces the branch before this work bit for
bit; checked by state hash):

1. **Fill–spill–merge inside closed depressions** (`erosion.lake_hierarchy`,
   `src/erosion/basins.rs`). Before, a depression with several sub-basins held one lake that
   covered its lowest cells first, whichever sub-basin the water drained into. Now the
   depression is a tree of sub-basins (local minima, joined at inner sills; sub-basins less than
   1 m below their sill are not counted). Rivers and runoff fill the sub-basin they drain into;
   a full sub-basin spills over its inner sill into its neighbour; when both are full they merge
   into one lake. A lake full to its spill level overflows into the next basin downstream (this
   cascade existed already, also into dry basins and on to the sea).
2. **The sea pours over its sill into the basin behind it** (`erosion.sea_spill`). At the end
   of each erosion step, the ocean holds the planet's water less all lakes. If it then stands on
   the sill of a basin it cannot fill (the sea-level solve's *deficit*), that water now flows into
   the basin and stays there below the sill: a lake fed by the sea, whose evaporation the sea
   replaces (Messinian Mediterranean; Lake Assal on a small scale). If the basin fills up, it
   becomes part of the ocean. Before, this water was not placed anywhere: the basin was drawn as
   full sea or as a lake with only its own water, and up to 160 m of sea level were missing.
   A sill on which the sea stands now closes the basin for the climate, the erosion and the
   statistics alike (before, sills within 10⁻⁶ m of sea level were open).
3. **Bug fix from the plan review: lakes on cells the climate saw as ocean did not evaporate**
   (`climate.ocean_lake_evaporation`). The climate and the erosion find the ocean on slightly
   different surfaces. A basin the climate counted as sea got a lake potential evaporation of 0,
   so the lake filled it to the brim. This made most of the "huge inland seas" of the #126 run
   set without the 0.2% rule; it also happened with the rule (one 1915-cell lake of 196 m of sea
   level). Such lake cells now evaporate as open water.

## Runs

8 seeds × 1000 Myr, 6 subdivisions, means over snapshots 500–1000 Myr, then over seeds (range),
Welch t against the first column of each pair.
- `base`: branch before this work (592da7d, = #126 merged with main), 0.2% rule on.
- `spill`: this work (ba040ab), rule on.
- `base-nosea`, `spill-nosea`: the same without the rule (`--set min_sea_fraction=1`).
- `evap-nosea`: only change 3, without the rule (ablation).

| metric | base | spill | t | base-nosea | evap-nosea | spill-nosea | t vs base-nosea |
|---|---|---|---|---|---|---|---|
| endorheic (% of land) | 5.7 (2.9…9.0) | 6.9 (2.2…12.5) | +0.7 | 13.3 (5.8…30.2) | 8.1 (2.8…17.7) | 9.2 (3.3…19.6) | −1.0 |
| lake cells (% of land) | 1.77 | 1.72 | −0.1 | 3.56 | 3.21 | 3.53 | 0.0 |
| lake water (m of sea level) | 1.9 (0.5…3.8) | 2.4 (0.1…7.8) | +0.5 | 30.7 (3.7…177) | 14.2 (0.2…36) | 12.6 (0.8…41) | −1.0 |
| lake water, largest snapshot (m) | 8 (2…26) | 12 (0.2…72) | | 164 (9…851) | 92 (1…293) | 92 (5…314) | |
| deficit after the sea-level solve (m of sea level) | 0.005 | 0.020 | | 3.6 (0…15) | 3.4 (0…26) | **0.03 (0.002…0.2)** | |
| deficit, largest snapshot (m) | 0.03 | 0.14 | | 38 (0…159) | 27 (0…200) | **0.3 (0.02…2.1)** | |
| dry land below sea level (cells) | 21 | 22 | +0.3 | 132 | 52 | 100 | −0.6 |
| closed depressions below sea level (cells) | 47 | 38 | −0.9 | 469 | 278 | 315 | −0.6 |
| playas (% of land) | 0.53 | 0.67 | +0.8 | 1.75 | 0.97 | 0.83 | −1.6 |
| depressions holding several lakes (per snapshot) | – | 3.4 | | – | – | 3.1 | |
| sea-fed lakes (per snapshot; cells) | – | 0.01; 0.01 | | – | – | 0.15; 12 | |
| sea water in them (m of sea level; largest) | – | 0.1; 10 | | – | – | 0.8; 27 | |
| lake median depth (m) | 96 | 78 | −0.9 | 295 | 156 | 176 | −1.4 |
| precipitation on lake cells (m/yr) | 1.34 | 1.10 | −3.6 | 1.27 | 1.34 | 1.41 | +1.0 |
| land precipitation (m/yr) | 1.29 | 1.21 | −1.4 | 1.21 | 1.20 | 1.30 | +1.0 |
| arid land, P < 0.25 m/yr (% of land) | 16.4 | 18.3 | +0.7 | 19.2 | 17.8 | 15.9 | −1.1 |
| Köppen B (% of land) | 28.5 | 31.5 | +1.3 | 29.7 | 30.0 | 27.6 | −0.6 |
| land p50 / p90 (m) | 392 / 1860 | 533 / 1960 | +1.1 / +0.4 | 582 / 2110 | 488 / 1940 | 486 / 1950 | |
| sea level (m) | −141 | −176 | −1.2 | −178 | −206 | −177 | |

- **With the 0.2% rule, nothing changes beyond the seed scatter.** Endorheic share, lake area,
  lake water, playas, aridity, Köppen and hypsometry are within scatter. About 3 of ~50 lake
  depressions per snapshot now hold more than one lake (sub-basins below inner sills). The sea
  rarely stands on a sill (the rule makes large remnants sea anyway). Precipitation on and next
  to lakes is lower (t −3.6), together with a drier land on average in this seed set (−0.08
  m/yr, t −1.4). Seeds diverge chaotically after the first changed step, so this is probably
  not caused by the change, but it is the one number outside the scatter.
- **Without the rule, water is now conserved.** The water that the old version did not place
  (mean 3.6 m of sea level, up to 159 m in one snapshot) is gone: 0.03 m on average, at most
  2 m. The rest is the known difference between the erosion's sample surface and the per-cell
  surface of the sea-level solve (#112 b).
- **Without the rule, the "huge inland seas" are mostly gone, and mostly because of fix 3**:
  lake water 30.7 → 12.6 m of sea level (evap-only: 14.2), endorheic 13.3 → 9.2% (evap-only:
  8.1), dry land below sea level 132 → 100 cells. The spill on top matters for conservation more
  than for the means.
- **What remains without the rule** are rare, real events: an ocean basin of ~10% of the planet
  is cut off from the world ocean by land whose lowest pass is above sea level (seed 7 at
  800 Myr: a south polar ocean, 3850 cells, 2.5 km deep; seed 5 at 1000 Myr: a north polar
  ocean). With 2.6–3.3 m/yr of precipitation its own water balance is positive, so it fills to
  its sill, up to 314 m of sea level, and spills into the ocean, like the Black Sea when it was
  cut off. These events occur in about 1 of 6 snapshots (largest snapshot per seed 5–314 m).
  On Earth such a basin would almost certainly keep a strait narrower than a cell (a grid effect,
  #64). The climate also treats it as land with lake evaporation, not as sea (no maritime
  temperatures), which is coarse for a water body that size.
- Sea-fed lakes below sea level (the sea pours over a sill into a basin that evaporates more
  than its rivers bring) occur in about 1 of 7 snapshots without the rule, covering up to ~50
  cells and holding up to 27 m of sea level.

## The 0.2% rule

The rule makes every closed body of water below sea level of ≥ 0.2% of the surface (~80 cells)
a sea, as if a strait narrower than a cell connected it. It measures the basin filled to its
pass, at any pass height (plan review, finding 8): a small pit inside a large high basin can
count as sea.

- Without the rule, the model is now consistent: water is conserved, basins below sea level
  hold what their own balance gives them, the sea pours into basins on whose sill it stands,
  full basins spill or merge with the ocean. That is the physically plausible treatment at
  this resolution, with no area threshold.
- What it lacks is sub-grid straits: a Mediterranean behind a 14 km Gibraltar can only be
  represented if a cell of the strait is below sea level. A sill-height criterion ("connected if
  the sill is less than h above sea level") was considered and not implemented: at 110 km cells
  the cell-mean sills of real straits (Bosporus, Gibraltar, Bab-el-Mandeb ~+100–300 m) and of
  real closed basins (Manych sill of the Caspian ~+20 m, Danakil, Qattara) overlap, so it would
  be another tuning parameter.
- Numbers: with the rule endorheic land is 6.9% (2.2–12.5), without 9.2% (3.3–19.6); Earth
  ~18–20%. Without the rule, cut-off polar or ocean basins sometimes hold up to 300 m of sea
  level as lakes, which the climate sees as land.

**Recommendation:** dropping the rule is now possible without losing water. I would still keep
it as the default until the climate treats large lakes as open water (temperature and
evaporation of the water surface); then drop it. Decision for Sebastian.

## Water conservation

- Planet water = ocean + lakes holds by construction (lake water is taken from the ocean). New:
  the water the ocean has above a sill goes into the basin behind it (counted as lake water), so
  the sea-level solve stands on that sill without a deficit (unit test
  `the_sea_pours_over_a_sill_below_the_sea_level_of_the_frame`). Residual deficits: mean 0.03 m,
  max 2 m (#112 b frame difference), against 3.6 / 159 m before.
- Sea-fed lakes evaporate more than their own inflow; the excess (≤ 0.5 × 10³ km³/yr) is the
  sea's inflow over the sill and goes to the air like all lake evaporation.
- Merging sub-basins keeps their water exactly (unit test with a shallow absorbed pit).

## Runtime (6 subdivisions)

Paired local runs (seed 0, 200 Myr, one core each, at the same time), base vs this work: 111.9 →
114.3 s and 105.7 → 111.8 s (+2…6%). The extra work: one pass-level flood and one sea-level
solve per step, and the hierarchy per depression. The run-set times mix local and GitHub
machines and are not comparable.

## Caveats

- Lakes, also very large ones, are land for the climate and temperature models (with their
  evaporation as a moisture source, #126). Matters only without the 0.2% rule.
- Sediment still settles over the whole depression from its lowest floor up (`fill_lake`), not
  per sub-basin.
- The lakes are equilibria at the end of each 1 Myr step and use the erosion's sample surface;
  the sea-level solve uses the per-cell surface (#112 b), which leaves small deficits.
- Water the lakes cannot hold up to the pores of the basin's sediment is counted with the lake
  water but drawn in no lake (the sea-level solve fills the pores of a basin it joins; lakes do
  not count pores).
