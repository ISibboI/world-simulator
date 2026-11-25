# Plan: water load only where water stands (#127)

_2026-11-25. Stacked on PR #166 (→ #165 → #163). Goal: an Earth-like planet, not Earth; grid
effects accepted (#64)._

## The problem

`isostasy::to_surface` turns a dry elevation `x` (relative to sea level, without any water load)
into the surface elevation: `x · 1.45` below sea level (`WATER_LOADING` = ρm/(ρm − ρw): the
water column of the ocean presses the column down), `x` above. It is applied to every cell
(`Simulation::surface_elevation`), so:

1. a **closed depression below sea level that is dry** (or partly dry) is drawn with the ocean's
   load: 45% deeper than its unloaded floor (the unloaded depth is 31% less);
2. a **lake above sea level** is drawn without the load of its own water: its floor is too high
   (a lake of depth h presses its floor down by r·h, r = ρw/ρm = 0.31, so in equilibrium the
   water is 1/(1 − r) = 1.45× deeper than on the unloaded floor);
3. lake volumes (`Hydrology::lake_volume`, the water taken from the ocean) are computed on these
   floors.

## Validity on #166 (agent/issue-160 tip, 8 seeds, snapshots 500–1000 Myr, from the logs)

- Closed depressions below sea level outside the ocean: **40 cells** per snapshot (0.1% of the
  surface; before #166: 66), 23 of them under lake water; 5 depressions, ~1 dry. Case 1 is real
  but small.
- Lakes: 1.9% of land (~250 cells), mostly above sea level, median depth below spill ~100 m:
  case 2 affects all of them. Lake water 0.52 × 10⁶ km³ (1.5 m of sea level).

## Fix

Per cell, the load follows the water that stands on it, as the last erosion step saw it at its
end (`SurfaceWater`, #166):

- **ocean** (connected to the world ocean, the default): `to_surface(x)` as now;
- **lake** with water depth h > 0: floor `x − r·h` (above or below sea level);
- **dry closed depression below sea level** (or a playa): `x`, unloaded.

`Simulation` keeps two per-cell fields, set by `Planet` after each erosion step together with
the lake volume (`set_lake_water` → `set_surface_water(volume, lake depths, inland mask)`):
`lake_depth` and `inland` (not ocean and below sea level at the end of the erosion step). Before
the first erosion step (and without erosion) both are empty: everything is as now. Cells whose
status changes during the next tectonic step keep last step's flags for one step (a newly
flooded coast is loaded like the ocean, because `inland` marks only cells that were in closed
depressions; a newly closed depression is loaded for one step).

The lake depth comes from the balance on the loaded floor of the step before, so the floor
converges to the equilibrium `h = 1.45 · (L − x)` geometrically (factor 0.31 per step).

**Water and sea level:** the sea-level solve uses dry elevations and loads only the ocean's
cells, so it is unchanged. Lake water stays taken from the ocean by its volume
(`set_lake_water`), which is now measured on loaded floors; the budget's `lakes` term records it.

Not changed: the erosion's internal responses (`isostasy::add_layer` treats a surface below sea
level as water-loaded, above as dry) for deposition under lake water; affects only the
deposition response in lakes (noted as caveat).

## Evaluation

Before = agent/issue-160 tip (10df505), after = branch, 8 seeds × 1000 Myr, `--snapshot-interval
50 --width 512 --cell-dump --lake-log-from 500`, window 500–1000: depths of closed depressions
(dry and with water; spill depth of lake cells), lake/endorheic area and persistence (lake
script of #160), lake volume, hypsometry, sea level and its budget, runtime at 6 subdivisions
(paired runs). Tests: a dry inland basin below sea level sits at its unloaded elevation; a lake
floor sinks by r·h; ocean cells unchanged; without surface water the old elevations.
