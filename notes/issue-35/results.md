# #35 results: where the continental cover goes, and its half-life bounds

Commit `97e7686` on `agent/issue-35`, 4 seeds (0–3) × 1000 Myr, defaults (6
subdivisions), cohort tagged at 500 Myr, snapshots every 50 Myr. Rates are means over
500–1000 Myr (10 snapshot intervals), per-seed range in brackets. Definitions: [plan.md](plan.md).

The diagnostics do not change the simulation: all snapshot state hashes equal `main`'s (seed 1,
5 subdivisions, 80 Myr). Runtime difference is within noise (21.8/22.0 s vs 22.2/22.4 s for
`main`, 100 Myr at 5 subdivisions); nothing adds a pass over all crust samples per step.

## 1. Cover preservation half-life: lower and upper bound

Cohort of 500 Myr after 500 Myr (at 1000 Myr):

| seed | left, well mixed | left, youngest first | half-life lower bound | upper bound |
|---|---|---|---|---|
| 0 | 0.1% | 0.8% | 51 Myr | 71 Myr |
| 1 | 2.8% | 8.9% | 97 | 143 |
| 2 | 2.7% | 6.3% | 96 | 125 |
| 3 | 12.7% | 32.1% | 168 | 305 |
| **mean** | 4.6% | 12.0% | **103** | **161** |

- The half-lives are fairly stable through the 500 Myr (seed 1: 64 → 97 lower, 80 → 143 upper;
  seed 3: 129 → 168 and 182 → 305): later cohorts sit on more stable cratons.
- **The bounds are close (factor 1.4–1.8).** Ordering hardly matters, so the cover is mostly
  lost by columns being stripped to their basement or consumed as a whole, not by young
  sediment being reworked on top of it.
- The well-mixed bound is not strictly a lower bound where burial metamorphism eats the
  bottom of a column (the cohort), but metamorphism takes only 1.9% (0.5–4.1%) of the cohort,
  so the caveat does not matter here.
- **Earth.** Mass half-ages of sedimentary rock of ~300–600 Myr (Garrels & Mackenzie 1969;
  Veizer & Jansen 1979, 1985), the 100–600 Myr range used in #34; Macrostrat survival
  half-lives are shorter: ~30 Myr for all sedimentary rock, ~99 Myr non-marine (Peters &
  Husson 2017, area-based). The model's 103–161 Myr bracket lies at the low end of the
  mass-based range: plausible for an Earth-like planet, but its cover recycles faster than
  Earth's. Seed 0 (51–71 Myr) is below it. A likely contributor is the collision sink below.
- Compared with #34 (145 Myr lower bound, 8 seeds), the lower bound is now 103 Myr on 4 seeds;
  the code changed a lot since (non-rigid plates, stress breakup, lakes), and the seed sets
  differ.

## 2. Export split: where the material eroded from continental crust goes

Continental denudation **15.1 Gt/yr** (11.1–18.6), of which **41% is reworked cover** (40–43%)
and the rest fresh basement detritus. Its destinations (net per cell and step):

| destination | share |
|---|---|
| continental land (outside closed depressions) | 7.9% (6.5–8.8) |
| closed depressions (lakes, endorheic and dry basins) | 15.0% (10.1–23.9) |
| flooded continental crust (shelves, epicontinental seas) | 23.0% (13.5–33.3) |
| ocean basins, net (transitional and oceanic crust) | 54.2% (48.5–57.4) |

Closure: the four shares sum to the denudation to ~1e-14 Gt/yr in every interval (mass
conservation of erosion; tested in `planet::tests::cover_export_and_budget_close`).

## 3. Fate of the 500 Myr cohort after 500 Myr (% of it; sums to 100%)

| | well mixed | youngest first |
|---|---|---|
| still on continental crust | 4.6 (0.1–12.7) | 12.0 (0.8–32.1) |
| sediment on other crust | 0.2 | 0.5 |
| eroded → continental land | 5.7 | 4.7 |
| eroded → closed depressions | 10.0 | 8.2 |
| eroded → flooded continental crust | 19.3 | 14.8 |
| eroded → ocean basins | 39.1 (36.1–41.0) | 32.2 (26.8–34.6) |
| metamorphosed (burial) | 1.9 | 0.0 |
| subducted / accreted at trenches | 0.7 | 0.9 |
| **collision** (stacked into orogens) | **16.5 (7.4–24.1)** | **24.5 (10.9–34.1)** |
| subduction erosion | 2.0 | 2.1 |
| other tectonics (hidden crust lost in merges etc.) | 0.0 | 0.0 |

So of the cover that leaves, ~74% (well mixed) is eroded, ~17–25% is consumed in collisions,
~2% by subduction erosion, ~2% by burial metamorphism and <1% subducted. Eroded cover counts
as lost even when it is redeposited on continental crust (~35% of the eroded part), because new
deposits are new rock; this is #34's definition, matching Earth's mass–age accounting.

## 4. Budget of the whole cover (Gt/yr, 500–1000 Myr)

Stock 2.5–6.5 × 10⁸ Gt at 1000 Myr.

| term | mean | range |
|---|---|---|
| surface processes (weathered basement in, net export to ocean basins out) | +0.68 | +0.21…+1.09 |
| burial metamorphism | −0.20 | −0.14…−0.31 |
| collision | −1.38 | −0.57…−2.31 |
| subduction erosion | −0.33 | −0.26…−0.51 |
| subducted, accreted | ~0 | |
| other tectonics (crust becoming or ceasing to be of continental origin, transport) | +1.27 | +0.72…+1.82 |
| **stock change** | **+0.03** | −0.24…+0.43 |

The cover is near steady state. Gross erosion of the cover (6.2 Gt/yr reworked) is nearly
balanced by redeposition and basement weathering, so its net exchange with the surface is small.
Tectonically it loses ~1.9 Gt/yr (mostly collisions) and gains ~1.3 Gt/yr as sediment-covered
crust turns continental (arc and transitional crust thickening, accreted terranes).

## Plausibility (Earth-like planet, not Earth)

- **Denudation 15 Gt/yr** compares with Earth's pre-human river suspended load of ~14 Gt/yr
  (Syvitski et al. 2005), ~20 Gt/yr with bedload and before damming (Milliman & Farnsworth
  2011). Plausible.
- **~41% reworked cover** is low next to Earth, where most erosion is of sedimentary rock
  (sedimentary rocks crop out on ~2/3 of the land; post-Archean sediment is mostly recycled
  sediment, Veizer & Jansen 1985). The model's cover is also thinner than Earth's (stock
  ~3–4 × 10⁸ Gt here vs ~2 × 10⁹ Gt of continental sedimentary rock on Earth, Garrels &
  Mackenzie 1971), so more of the eroded surface is bare basement.
- **Ocean basins ~54%, flooded continental crust ~23%.** On Earth most river sediment stays on
  margins (deltas, shelves). Many large deltas (Mississippi, Niger, Ganges–Brahmaputra fan) sit
  on transitional or oceanic crust, which the split counts as "ocean basins", so the two are
  not directly comparable. Grid artefact (#64): with ~110 km cells, a shelf is often one or two
  cells wide, and a cell is continental or not as a whole, so the margin's share is coarse.
- **Closed depressions ~15%** looks on the high side. About 20% of Earth's land drains
  internally, but it is mostly arid with low sediment yields. The share includes
  closed depressions filled by a step's own sediment and small pits that the coarse grid makes
  (grid artefact).
- **Collisions as the main tectonic sink (1.4 Gt/yr, 17–25% of the cohort)** is plausible in
  kind: the cover of a subducting margin is folded and metamorphosed into orogens (Himalaya,
  Alps). It is probably high in amount, because the model's collision boundaries are long and
  persistent (known issue; collision stacking 1.7 Gt/yr in seed 0). That makes the half-life
  shorter. A follow-up only if the collision length is revisited.
- **Burial metamorphism (0.2 Gt/yr) and sediment subduction of the cover (~0)** are small. On
  Earth, sediment subduction is ~1.3–1.7 km³/yr overall (Plank & Langmuir 1998; Clift et al.
  2009), but mostly of pelagic and trench sediment, not continental cover. That fits: here the
  cover's subducted share is <1%, and the model's trench sediment (6 Gt/yr in seed 0) is mostly
  sediment already in the ocean basins.

## Caveats

- Net per cell and step (1 Myr): erosion and redeposition within a cell and step are
  invisible, so gross turnover and the eroded shares are lower bounds.
- The eroded load is not traced from cell to cell: the cover part shares the destinations of
  the whole step's continental denudation.
- 4 seeds; the seed spread (half-life 51–168 Myr) is larger than the bound gap.
