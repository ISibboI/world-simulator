# Review of plan.md (#173)

_2026-11-26, independent read-only review against main (e7d2a14). Line numbers are main's
(`git show HEAD:…`). The worktree already has implementation changes; I did not review them,
except to note that `MIN_LAKE_MIXED_LAYER = 1 m` (finding S4) is already in it._

## Verdict

The plan describes the current code correctly and the design is sound. The plan's choices hold
up: a third column per coarse cell, sharing the ocean column's code, no ocean heat transport for
lakes, actual air humidity, and a lake freezing point lowered by the lapse rate. Switch-off bit
identity is achievable if the fractions are written carefully (M1). The main gaps are:
- NaN and mask mismatches between the lake evaporation return and the new monthly lake ice (M2);
- the gyre, Ekman and free-air masks that silently become "1 − land" (M3);
- land-ice albedo that still counts lake cells (S1);
- the viewer, which throws away the new lake temperatures for the lakes that matter most (S5);
- a measurement plan that cannot isolate the maritime effect because seeds diverge (S6).

## Is the description of the current code right?

Yes, all of it checks out:
- land fraction from the view, `temperature.rs:599`;
- exchange `k(1−f)` and `k f`, `:863`, `:873`;
- gyre mask `f < 0.5`, `:707`;
- fine cells from land (with lapse rate and free-air shift) or ocean, `:1014`;
- `ice` is sea ice or ice sheet, `:1036`;
- marine air for the ocean and actual humidity for land, `climate.rs:667–675`;
- the `ice_free` ramp, `:1151`;
- step order and the one-step lake lag, `planet.rs:448–532`.

A small addition: `ocean_lake_evaporation` gives ocean-view cells a ramped `lake_potential` too
(`climate.rs:664`). That path stays as it is.

## Must

**M1. Bit identity depends on how `f_ocean` is computed.** Today the code uses `1.0 - f`
(`temperature.rs:603, 675, 818, 863, 937, 955`). Compute `f_ocean = (1.0 - f_land) - f_lake`
with `f_lake = 0.0`. Then:
- every new term is `x + 0.0·T`, which is exact while the virtual lake state stays finite (never
  NaN or inf);
- the damping `k(f_o + f_lake)` equals `k(1−f)`.

`ocean_cells / cells` is **not** bit-identical to `1 − land_cells/cells`. The safest form is a
per-cell branch on `f_lake == 0.0` (or on the switch) that runs the old expressions unchanged.

`climate_elevations` (`planet.rs:844`) must stay untouched with the switch off. It is also
called at `planet.rs:218`.

**M2. Mask and NaN consistency in the water cycle.** `add_lake_evaporation`
(`climate.rs:744–770`) runs on cells with last step's `lake_evaporation > 0`. `monthly_lake_ice`
is NaN off "lake cells not seen as ocean". These sets differ in two cases:
- lake cells that the current view (after plate motion) sees as ocean;
- the edge case `covered > 0` with depth 0.

A NaN ice share makes the weight NaN, and the NaN spreads into precipitation everywhere. Fall
back to the ramp, or to even weights, wherever the ice share is not finite. Also compute the
weights' mean over the months with the same per-cell rule, so that they still average to one
and conservation holds exactly. Add a test with a lake cell that the view sees as ocean.

**M3. List every "1 − land" use and say which fraction it becomes.** Once lake cells leave the
land count, `1 − land_fraction` means ocean + lake. Each of these must become `f_ocean`:
- band ocean width for Ekman, `:675`;
- band SST, `:767–773`;
- gyre mask `is_ocean`, `:707` (otherwise a lake-dominated coarse cell gets gyres, and
  `ocean_current`/`gyre_heating` at `:1057–1066`);
- ocean area per cell for the gyre heat, `:818`;
- subduction weights, `:937`;
- the free troposphere's `sea`, `:955`.

Each of these must include `f_lake`:
- `heat_capacity`, `:602`;
- `mean`, `:851`;
- `seasonal_mean`, `:916`.

The plan says this only in general terms. Make it a checklist, because each miss silently
changes physics.

## Should

**S1. Land-ice albedo still counts lake cells.** `land_cells_list` and the `land_cells`
denominator (`temperature.rs:594–597, 748–757`) use `elevation >= 0`, so lake cells would put
ice-sheet albedo on the land column. Exclude lake cells there, and in the fine `ice` and
`surface` branches (`:1014, :1036`).

**S2. Partly covered lake cells.** `lake_depth > 0` holds for `ceil(covered)` cells
(`erosion.rs:1360–1363`). The last cell of every body counts as fully lake although it is
partly covered; for small lakes that overstates their area by up to one cell. Export the cover
fraction (or the water level) in `SurfaceWater`. That is not in the hash, so it is safe. Use the
cover fraction for `f_lake`.

**S3. Fine lake temperature vs the column's freezing point.** The plan lowers the fine
temperature by `Γ z_fine` and adds the free-air shift (`:1141`), while the column freezes at
`Γ z̄`. Fine cells therefore show open water below 0 °C, or ice above 0 °C, when:
- the cell's own level differs from z̄ (several lakes at different levels in one coarse cell);
- the free-air shift is non-zero (tropical high lakes).

Two options:
- leave out the free-air shift for lake cells, or include it in the column's freezing point;
- clamp open-water months at ≥ 0 °C and ice-covered months at ≤ 0 °C on the fine cells.

Either way, state which you chose. It matters for Köppen-like readouts and for the evaporation
of plateau lakes.

**S4. Shallow-lake heat capacity.** `mean(min(depth, H))` with a 1 m floor gives
4.2e6 J/m²/K, which is below the land's air column (1e7, `temperature.rs:293`). A thin lake
would then react faster than land. Floor the lake column at `land_heat_capacity`, i.e. about
2.4 m of water.

**S5. The viewer drops the new lake temperatures.** `realistic.rs:659–700` (`lake_air`)
replaces the temperatures of every lake whose floor is below sea level with the mean of the
land around it. Its comment ("the climate gives such cells the ocean's temperature") is
already stale. That covers exactly the cut-off seas this issue is about. The lake-ice heuristic
(`realistic.rs:289–300, 436–447`) also stays separate, while the issue asks to "unify with
existing lake ice".

At minimum, skip `lake_air` when lake temperatures come from the model, and open a follow-up
for drawing the model's `monthly_lake_ice`. That needs a CellInfo/codec field, and the UI
guidelines apply. Tell Sebastian it is deferred.

**S6. The measurement cannot isolate the effect.** Seeds diverge after the first changed step,
so the "difference of differences" for rings around lakes compares different lakes on different
planets. That is noisy, and an 8-seed t-test will probably not show a 1–2 K maritime effect.

Add a **paired diagnostic**: at each snapshot, also run the temperature model (and one water
cycle) with the switch flipped on the same view and lakes, and dump both fields. This gives the
effect on lake and near-lake temperature, ice and potential evaporation with no chaos. It costs
one extra `update` per snapshot and runs only with a diagnostic flag. Keep the 4-set runs for
the long-term outcomes (lake water, endorheic share, sea level, rule verdict).

**S7. Write down the rule verdict criteria before running.** For example, the rule stays off if
all of these hold:
- the deficit after the sea-level solve stays ≈ 0;
- lake water (mean and largest snapshot) is not worse than `main-nosea`;
- the largest cut-off basin shows ocean-like temperatures and seasonal ice;
- global temperature, frozen fraction, aridity, Köppen and hypsometry stay within scatter of
  `main`.

Also say plainly that the lake column changes the *climate* of cut-off basins, not *whether*
they fill. Filling is set by P − E. E may rise a little (warmer water in winter), so a filled
300 m basin is still expected with the rule off; per #64 it is a grid effect to explain, not to
fix.

**S8. Land statistics now include water temperatures.** `statistics()` counts lake cells as
land (`planet.rs:633`), so land means, aridity and the frozen fraction (`:703`,
`presets.rs:431`) shift when lake cells take water temperatures and lake ice. For comparisons,
compute land-only metrics without lake cells (or report both), or the sets differ by
definition.

## Could

**C1. Water-surface elevation.** The plan rebuilds it from the current `elevation`, last
step's `load_correction` and `lake_depth`. `load_correction` is relative to the erosion's
*sample* surface (`sample_surface` is on by default, `erosion.rs:207, 1498–1508`), while the
current elevation is per cell, one step later. The error is tens of metres, which is negligible
for the lapse rate (< 0.5 K). Using the exported water level from S2 is cleaner. Cap it at no
lower than the current view elevation for cells uplifted since the last step.

**C2. Perennial lake ice at `max_ice_thickness`.** Clipping the enthalpy at
`temperature.rs:908` is an energy source that the ocean explains by ice drifting away. Lakes
have no drift. This is acceptable at 10 m, but document it, or give lakes a conduction-limited
equilibrium.

**C3. Virtual columns jump when they become real.** A lake appearing or changing z̄ or depth
keeps its enthalpy, so T jumps by Γ Δz̄ or by the change in heat capacity. That is harmless: the
relaxation time C/(B + k) ≈ 0.5 yr is short against the 2 years per update.

**C4. Large cut-off basins have no ocean heat transport.** A 3850-cell polar basin
(spill-results) gets no gyre or Ekman heat. That is defensible: an enclosed sea has internal
gyres but exports almost no heat across latitudes. Say it in the PR as a known simplification,
together with the 50 m cap on deep basins, which ignores autumn overturning (Lake Superior
rarely freezes).

**C5. Ocean evaporation under sea ice.** It is not reduced today (`climate.rs:667`). With lake
ice now cutting lake evaporation, lakes and sea treat ice differently. The plan notes this;
open a follow-up issue.

**C6. `sea_seeds`.** Returning early at `>= 1.0` (today `> cell_count`, `sea.rs:159`) gives
the same results as the old `=1` runs, so `main-nosea` stays comparable. Keep `MIN_SEA_FRACTION`
as a named constant: tests in `erosion.rs` and `sea.rs` pass it explicitly. Document the new
default in `tectonics/mod.rs:360`.

## Simpler alternatives considered

- **Put lake cells into the coarse ocean fraction** (temperature model only), with a separate
  ocean mask for the currents. This needs less code, but high lakes and sea-level ocean would
  share one column (wrong freezing point and lapse rate), and so would deep and shallow water.
  The plan's third column is little extra code once the column physics is factored out. Keep
  the plan.
- **Open water only for lake bodies of N or more cells.** This is not simpler, and the plan's
  argument (influence scales with area share) holds. Not recommended.

## Efficiency

One more column per coarse cell per substep: 2562 cells × 384 substeps × 2 years, which is
negligible. The restricted, renormalised interpolation applies only to lake fine cells, which
is also negligible. The paired runtime measurement in the plan is enough.
