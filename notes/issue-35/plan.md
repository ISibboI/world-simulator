# #35: continental cover — export split and upper half-life bound

Follow-up of #34 (PR #34, dynamic topography). Diagnostics only: nothing the simulation reads
changes, and the state hash of a run is bit-identical with and without them (checked: seed 1,
5 subdivisions, 80 Myr, all five snapshot hashes equal to `main`).

## Definitions

**Continental cover**: all sediment on crust samples whose basement (the crust without its
metasediment) is of continental origin (`Crust::is_continental_origin`), summed over all samples,
hidden ones included. Same definition as #34's cohort.

**Cover cohort** (#34): at `--cover-cohort` (default 500 Myr) all cover sediment is tagged.
New deposits are new rock and stay untagged. Two tracers now follow the tag (`Crust::cohort`,
type `Cohort`):

| tracer | removal from a column (erosion, subduction erosion, burial metamorphism, scraping at trenches) | half-life is a |
|---|---|---|
| `mixed` (#34) | takes the cohort in proportion (well-mixed column) | **lower bound** |
| `lifo` (new) | takes everything deposited after the cohort first; the cohort is the bottom of the column and goes only when the column is stripped below it (youngest first) | **upper bound** |

Both mix by mass where sediment is added or moved sideways: a lateral share of a column is a
vertical slice of it, so it carries both fractions unchanged (stretching, gap healing,
non-rigid-plate transport, merges, distributed shortening). At accretionary trenches the
décollement scrapes off the top of the column, the rest is subducted; the LIFO cohort, at the
bottom, is subducted first. Burial metamorphism physically converts the bottom of a column, i.e.
the oldest sediment; the LIFO tracer still takes it youngest first so that it stays a strict
bound (any real stratigraphic order lies between the two tracers). How much this matters is
visible in the fate tables (metamorphosed share of the cohort).

**Half-life**: `t ln 2 / −ln(f)`, `f` = fraction of the cohort still sediment on continental
crust after `t` Myr, per tracer.

## Export split: where the material eroded from continental crust goes

Per erosion step and cell (cell class by the basement of the shown sample, as `crust_class`):

- **continental denudation** `L` = sediment removed from continental cells (net per cell and
  step; reworked cover) + bedrock eroded on them (fresh basement detritus), as mass;
- deposits on continental cells by place (from the erosion step's view of the surface at its
  start): **continental land** (above sea level, outside closed depressions), **closed
  depressions** (lakes, endorheic and dry basins), **flooded continental crust** (connected
  ocean: shelves, epicontinental seas);
- **ocean basins (net)**: the net mass gain of all other cells (transitional and oceanic crust).

Because erosion conserves mass, the four destinations sum to `L` (logged "unaccounted",
tested). The load is not traced from cell to cell, so the cover part of `L` is assumed to share
the destinations of the whole step's continental denudation; the cohort's eroded mass of a step
is split over the step's positive destinations in proportion. Everything is net per cell and
step, i.e. a lower bound of the gross turnover (as #34's turnover line).

## Budget by process

Three masses (cover; cohort mixed and LIFO, anywhere) are recorded where they change, sample by
sample, without extra passes over the crust per step:

| term | where |
|---|---|
| surface | `apply_surface_change` (erosion/deposition, incl. columns whose class changes by bedrock erosion) |
| metamorphism | `metamorphose_sediment` |
| subducted / accreted | consumed lower-plate samples at trenches (below / above the décollement) |
| collision | buoyant lower-plate samples stacked into orogens (sediment → crust) |
| subduction erosion | forearc sediment of the overriding plate |
| other tectonics | the rest of the change since the start (transport, shortening, gap healing, flow, plate events, class changes), computed when reported |

So the terms sum to the change exactly; the test checks that the recorded surface and
metamorphism terms equal the actual change of all three masses across `apply_surface_changes`,
that trench sinks only remove, and that the LIFO cohort never falls below the mixed one.

## Output (log only, every snapshot)

- `continental denudation … Gt/yr (…% reworked cover): continental land …%, closed depressions
  …%, flooded continental crust …%, ocean basins (net) …%`
- `continental cover budget (Gt/yr): surface …, metamorphism …, subducted …, … | stock change …`
- `cover cohort of 500 Myr: … left … well mixed, … youngest first | preservation half-life … Myr
  (well mixed, lower bound) to … Myr (youngest first, upper bound)`
- `cover cohort fate (well mixed / youngest first, % of it): on continental crust, sediment on
  other crust, eroded to <4 destinations>, metamorphosed, subducted, accreted, collision,
  subduction erosion, other tectonics` — sums to 100%.

## Measurement

4 seeds × 1000 Myr (defaults, 6 subdivisions) through `runs`; results in `results.md`.
