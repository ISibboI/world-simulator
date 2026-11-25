# Plan: sea and lakes spill into neighbouring dry basins (#168, Sebastian 2026-11-25)

_Request ([#168 comment](https://github.com/ISibboI/world-simulator/pull/168#issuecomment-7171197285)):
"Sea and lakes should spill into neighbouring dry basins by physically plausible rules", and
re-evaluate the 0.2% rule (`min_sea_fraction`, `src/sea.rs`). Goal: an Earth-like planet, not
Earth; grid effects (~110 km cells) accepted (#64)._

## Rules (physics)

1. Water crosses a sill only when its surface is above the sill.
2. The sea stands at sea level and floods every basin it reaches over sills below sea level.
3. A basin behind a sill above sea level gets only its own water: precipitation, runoff,
   overflow of lakes upstream; it loses evaporation. It can be dry (Qattara, Dead Sea).
4. A lake filled to its spill level overflows into the next basin downstream (fill and spill),
   possibly in a cascade, possibly to the sea. Inside one closed depression with inner sills,
   water first fills the sub-basin it drains into; only a full sub-basin spills over its inner
   sill into its neighbour; when the neighbours are full, they merge into one lake
   (fill–spill–merge, Barnes et al. 2020).
5. Water is conserved: ocean + lakes = the planet's water; lake evaporation goes to the air.
6. If the ocean holds more water than fits below the sill of a basin behind it, the excess pours
   over the sill into the basin (Zanclean flood): the basin holds that water (a stock) below the
   sill, or, once full, joins the ocean.

## What exists (checked in the code)

- **Fill and spill between depressions exists** (`water_state`, `src/erosion.rs`): drainage is a
  priority flood from the ocean; the depressions are walked from upstream to downstream and each
  lake's equilibrium (`lake_balance`: the lake rises until its evaporation balances the inflow,
  else spills the rest) is computed at its exit, whose outflow adds to the discharge downstream.
  So an overflowing lake already feeds the next basin (also a dry one), cascades work, and the
  water reaches the sea. Lakes are in equilibrium every Myr step (filling/drying times of even a
  remnant sea are 10²–10⁴ yr), which is right at this time step.
- **Rule 2 exists** (`sea::ocean_from`, `sea::sea_level`): the ocean is the water below sea
  level connected to the seeds; the solve floods passes in order.
- **Missing 1 — inner sills.** A depression with several sub-basins is one lake: its cells are
  sorted by floor and covered from the lowest up, whatever sub-basin the water drains into
  (documented as "a known simplification", `Hydrology::nested_lakes`; in the logs 11–14 of ~50
  depressions per snapshot, the larger ones). Water thus appears in a sub-basin no river reaches,
  before the fed sub-basin is full (violates rules 1 and 4).
- **Missing 2 — the sea's excess over a sill is lost.** When the sea-level solve finds the ocean
  at a pass into a basin it cannot fill, it stands at the pass and returns a *deficit* (the water
  that would pour in). Nothing places that water: the basin is drawn as an "arm of the sea" (the
  mask passes sills within 1e-6 m) or as a lake with only its own water. Water is not conserved
  by the deficit. With the 0.2% rule the deficits are ~0.01 m of sea level (pores), without it
  they reach **34–608 m of sea level** (#126 run set `after-nosea`: seeds 0, 1, 2 at 850/1000 Myr).

## Why "huge inland seas" without the 0.2% rule (#126 `after-nosea`)

Seed by seed, snapshot by snapshot: the very large lake water values coincide with large
deficits. Seed 1 at 850 Myr: an ocean basin of ~8000 cells (19% of the planet, depths of 3 km) is
cut off by a sill at sea level; as a lake it fills to its spill level (906 m of sea level), the
rest of the ocean stands on the same sill with a deficit of 115 m. Physically that is one ocean
over a sill (both at the sill level, the excess pours over it); the model counts it as a full
lake plus a sea at its sill and loses the 115 m. Seed 2 at 1000 Myr: a large remnant
desiccated to 76 m of sea level as a lake, the ocean stands at its sill with a deficit of 608 m
that never reaches the basin. So:
- the instantaneous equilibrium itself is right (time scales ≪ 1 Myr);
- water conservation by the scalar lake bookkeeping is right as such (lake water is taken from
  the ocean), **but the sea's spill into a basin is not represented** (missing 2), which both
  loses water and leaves full lakes next to the ocean at the same sill;
- the lake evaporation is returned to the air (#126), so that is not the cause;
- remaining inaccuracy: the climate and the temperature model treat lake cells as land (the
  lake evaporation is a moisture source, but no maritime temperature or ocean evaporation). For
  basins of thousands of cells this matters; out of scope here (caveat, possible follow-up).

## Changes

### A. Fill–spill–merge inside depressions (`erosion.lake_hierarchy`, default on)

Per depression (a `Lakes` entry that holds water), build the depression hierarchy: union–find
over its cells in ascending floor order (ties by order); a cell without processed neighbours in
the depression starts a leaf (a local minimum), a cell touching two or more components is an
inner sill and creates a node whose children are those components (its *merge level* is that
cell's floor). The root's spill level is the depression's spill level (`filled`).

- **Inflow per leaf.** Each depression cell's own runoff plus the discharge arriving from outside
  the depression (rivers, overflow of lakes upstream) goes down the steepest descent inside the
  depression to its local minimum, i.e. to a leaf.
- **Fullness depends only on the total input** (proof: overflow of full parts is passed on to
  partial ones; if any stayed partial, all water would evaporate inside). With F(n) the
  evaporation (excess over the land's evapotranspiration, as now) of all cells of node n's
  subtree and X(n) its total input, top-down from the root with X(root) = discharge at the exit:
  - n is one lake if it is a leaf or X(n) ≥ Σ_children F(c) (all its sub-basins full): lake
    balance (unchanged `lake_balance`) over all its subtree cells with inflow X(n) and spill
    level its merge level (root: the depression's spill level).
  - otherwise the children's inputs are their own inflow (plus water given to them from above);
    full children (X(c) ≥ F(c)) spill their surplus over the inner sill into the partial
    siblings, the one whose cell next to the sill is lowest first (steepest descent), entering
    at the leaf below that cell; recurse into every child with its final input.
- The depression is endorheic iff the root does not spill (X(root) < F(root)), exactly as now;
  its outflow is X(root) − F(root) as now. The total evaporation of a spilling depression is
  unchanged; what changes is where the water stands (which cells, level, depth, volume) when it
  does not spill.
- `lake_hierarchy=0` treats each depression as one leaf: the current code path, bit-identical.

### B. The sea spills into the basin behind its sill (`erosion.sea_spill`, default on)

At the end of the erosion step (where the lakes for the next step are made, #160), after the
lake balance:

1. Solve the sea level on the end topography (dry elevations, pores of the sediment, the
   ocean's seeds; `sea::sea_level_from`) for the ocean's water `W − L`, with `W` the planet's
   water (new `Surface::water_volume` = `TectonicParams::ocean_volume`) and `L` the water of all
   lakes just computed.
2. If it stands at a pass `p` with a deficit `D` > 0 and there are basin cells behind the pass
   (pass level from the seeds = `p`, floor below `p`; not only a cell whose pores fill):
   - if any of them is ocean in the end state (the sill lies within rounding or within the
     dynamic-topography difference of the sample surface below sea level), they are made land
     (cells with pass ≥ `p` leave the ocean) and the water state is computed again;
   - the basin's depression (the lake containing those cells) receives `D` as a volume, entering
     at its exit (next to the sill) and filling the hierarchy by volume: raise the lake at the
     entry leaf to its sill, spill the rest to partial siblings, merge, …;
   - if the basin is full before `D` is used up (its own lake plus `D` exceed its volume below
     the sill), the basin joins the ocean: it is made ocean and the water state recomputed (the
     sea then rises above the sill and floods it next step).
3. The lake water reported to the tectonics (`set_lake_water`) includes the sea water in such
   basins, so the tectonic solve finds the ocean exactly at the sill with no deficit. Their
   evaporation (now larger than their own inflow; the difference is the sea's inflow over the
   sill) goes to the air like other lake evaporation.

Logged: sea-fed basins, their cells and sea water (km³), the deficit before/after. Such a basin
is a *marine inlet below sea level* (Messinian Mediterranean, Lake Assal on a smaller scale):
its catchment counts as endorheic, and its cells are counted separately so both readings are
possible.

### C. The 0.2% rule

The rule is a sub-grid strait parametrisation (every remnant ≥ 0.2% of the surface is assumed to
be connected by a strait narrower than a cell). With A and B, dropping it gives a closed,
conserving treatment: a cut-off remnant is a lake by its balance; if it dries, its water raises
the ocean, which, if it reaches the sill, pours back in (B); a remnant that fills to the sill
merges with or spills into the ocean. No area threshold. Measured both ways; the verdict follows
the numbers. A sill-height criterion (strait if the sill is less than ~h above sea level) was
considered; Earth's cell-averaged strait sills (Bosporus, Gibraltar, Bab-el-Mandeb) and closed
basins' sills (Manych ~+20 m for the Caspian, Danakil, Qattara) overlap at 110 km cells, so it
would be another tuning parameter; not implemented.

## Measurement

8 seeds × 1000 Myr, 6 subdivisions, `--duration 1000 --snapshot-interval 50 --width 512
--cell-dump --lake-log-from 500`, window 500–1000 Myr, via `runs`:
- `base` = branch head before the change (rule on), `base-nosea` (`--set min_sea_fraction=1`);
- `spill` = after (A + B, rule on), `spill-nosea` (A + B, rule off).
Metrics (compare.py from #126 plus new ones): endorheic share, lake area, lake water (m of sea
level) with seed scatter, dry land below sea level, playas, sea-fed basins (marine inlets) and
their water, deficits, nested depressions and their separate lakes, aridity/Köppen B, hypsometry,
water conservation (deficit before/after, lake water vs sea-level budget), runtime (paired local
runs at 6 subdivisions).

## Tests

- Two sub-basins, inflow only into one: it fills to the inner sill first, then the other; the
  dry one stays dry while the first is below the sill; both merge when full; the depression
  spills only when the merged lake reaches the spill level. Evaporation and volume consistent.
- With `lake_hierarchy=0` results equal the old balance.
- Sea spill: an ocean whose volume exceeds the capacity below a sill into a dry basin: the basin
  holds exactly the deficit (water conserved: ocean + lakes = planet's water), the sea stands at
  the sill; with enough water the basin becomes ocean.
- Existing lake/sea tests keep passing.
