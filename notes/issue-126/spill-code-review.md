# Code review of a20521d (#168, fill–spill–merge and sea spill)

Scope: diff 592da7d..a20521d. Read-only review. I added two throw-away tests to a copy of the
tree (`~/tasks/issue-126/tmp/review-src/src/erosion/basins.rs`, tests `review_*`). The random
test builds 300 random hierarchies (shallow 0.5 m steps, ties, random inflow), then solves and
pours into each.

Checked and found correct:
- union–find and part tops; one component per depression (a `Lakes` entry is a receiver subtree, so it is mesh-connected)
- absorbed nodes are always leaves: an inner node is always deeper than LAKE_MIN_DEPTH below any later merge
- the `leaf_of` chain resolves without cycles to a leaf in the tree
- DFS leaf ranges
- `child_with_leaf` never panics (no panic in 300 random cases)
- Σ evaporation equals the inflow for non-spilling depressions (random test)
- `total == Σ inflow` (own + arriving equals `discharge[exit]`)
- surplus cascade, with no double counting of evaporation
- `discharge` stays non-negative
- determinism (index orders, stable sorts, no hash maps)
- the off-switch path

## Findings

1. **should — pour creates water when sub-basins merge (absorbed cells).** `basins.rs:422-451`. When `pour_into` merges a partial node, the new lake starts at `merge` over `subtree_cells(node)`. That includes the parent's `own` cells absorbed as shallow sub-basins (floors less than LAKE_MIN_DEPTH below the inner sill). Those cells were dry, but at the merge level they hold up to 1 m × WATER_LOADING each, and that water is counted nowhere. Confirmed by the random test: in 4 of 300 cases, the lake-volume change exceeds the poured volume (`sea_water`) by exactly 0.5·W or 1.0·W per absorbed cell. So `hydrology.lake_volume` > flux + sea water. The tectonics then takes the extra from the ocean: the planet total is still closed, but `sea_water` and `lake_volume` disagree and `spill_sea`'s leftover is slightly off. Fix: count the merged lake's volume at `merge` and subtract the children's volumes (or the absorbed cells' `lake_volume` at `merge`) from `left` before the recursive pour. Alternatively, leave the absorbed cells out of the merged lake's starting volume by keeping them uncovered until the level rises.
   - Related nit: `solve` (`basins.rs:329-331`) puts a one-lake inner node through `lake_balance` over all subtree cells. With `input == below`, the absorbed cells below the merge level are covered first, so the level ends slightly below the merge level.

2. **should — a node with zero required evaporation becomes "one lake" and pour then ignores the inner sills.** `basins.rs:329`. The check `input >= below` holds when `below == 0` and `input == 0`, for example where the lake's potential evaporation is ≤ ET everywhere, or frozen cells (`ice_free` = 0) with no runoff. The whole depression is then one lake at its lowest floor. A later `pour` (`basins.rs:389-409`) fills it as one body from the lowest floor up: at level L it covers both sub-basins below L, across an inner sill above L, instead of filling the sub-basin the water enters. A probe test showed the solve returning one lake over [a, b, s] for zero excess and zero inflow. Fix: treat a node as one lake only if `input > below || (below > 0.0 && input >= below)`, or skip the shortcut when `input == 0`.

3. **should — the closed-sill semantics are applied only to erosion and climate, not to `ocean_mask`.** Planet statistics (`planet.rs:620`), the snapshot code (`main.rs:1446`) and the tectonics' `ocean_mask` (`tectonics/mod.rs:1335`, used at :1905) still open sills at sea level (`pass < SILL_TOLERANCE`). A sea-fed basin is therefore a lake in `water` and in the climate, but ocean in the land/ocean statistics, land precipitation, the largest river and the tectonic statistics. The plan review's finding 2a asked for "everywhere". Fix: give `ocean_mask` the same switch (closed when `sea_spill`), or document that the statistics deliberately use the old mask.

4. **should — `spill_sea` floods only lake members, leaving land dams inside the new sea.** `erosion.rs:1055-1064`. `flooded` requires `state.lakes.lake[c] != NO_LAKE`. The sill cell itself and the rim cells within LAKE_MIN_DEPTH of the spill level (dry < level and pass < level) stay land. The returned `surface_water.ocean` then holds ocean patches cut off from the seeds by a ring of land, and the shallow rim cells are neither sea nor lake. Fix: flood every non-ocean cell with `pass[c] < level && dry[c] < level` (optionally only those connected to a flooded lake), or recompute the ocean with `ocean_from` over a mask of these cells.

5. **should — the un-ocean branch acts planet-wide.** `erosion.rs:1079-1089`. It is documented as fixing rounding at one sill, but it sets `ocean = false` for every cell with `pass[c] >= level`. If the solved level is noticeably below the frame's 0 (lakes grew during the step, large tectonic change), the whole shelf between the level and 0 becomes land below sea level in the returned water state. Other basins whose sill lies between the level and 0 also become land, not only the basin at hand. That may even be the intended physics (the sea stands at `level`), but then the comment is wrong. Fix: restrict it to `basin` cells (plus cells whose pass equals the level), or update the comment and check the consequences for `surface_water.ocean` consumers.

6. **should — performance: one full `water_state` per target, up to 4 outer rounds.** `erosion.rs:1104-1116`. Each target depression costs a full planet `water_state` (drainage priority flood, `Lakes`, a `Basins` build and solve for every depression). Each outer round adds a `sea_level_from` and a `pass_levels` flood plus another `water_state`. A basin split into many `Lakes` entries by shallow cells means many full recomputations per step. Fix: let `water_state` take the ordered target list and cascade the leftover from one target to the next internally (one call). Or pour into the already-built `Basins` of the targets and patch only their cells' fields.

7. **nit — `pass_levels` is computed almost every step.** `erosion.rs:1048`. `reachable` is true whenever any lake cell lies below the solved level, which includes every closed depression below sea level behind a high sill (common). That costs an extra planet-wide flood per step even though `flooded` will be empty. Fix: compute `pass` once and test `pass[c] < level` in `reachable`, or check `solved.deficit > 0` first and run the flood only when a flooded candidate is possible.

8. **nit — the round cap can end without placing the deficit.** `erosion.rs:1044`. If the 4th round ends in a `continue` (flood or un-ocean branch), the loop exits with a recomputed but un-poured state. `sea_spill_deficit` still reports the deficit as "poured over sills". The water itself is not lost (the ocean keeps it via W − lakes), but the log line is wrong. Fix: set `deficit` only when a pour is actually applied, or `warn!`/`debug!` when the cap is hit.

9. **nit — the overflow test can never trigger as intended.** `erosion.rs:1120`. A deficit returned by `sea_level_from` is always smaller than the capacity of the basin cells the flood joined before the level fell below the pass. So `left > 1 m × basin cells` happens only through the flux lake already in the basin plus the deficit. A sea over-full for the basin is caught in the next round by `flooded`, not here. That is fine, but the comment ("more means the basin is full") overstates it. The 1 m × basin.len() allowance also silently drops up to that much leftover; the ocean keeps it, but the deficit stat does not show it.

10. **nit — the stat named "sea-fed basins" counts lake bodies.** `erosion.rs:1342`. `sea_fed_lakes` is incremented per lake body with `sea_water > 0`, so a depression whose pour filled two sub-basins without merging counts twice. The `main.rs` log calls them "sea-fed basins". Count per depression (`sea_fed[lake]` transitions), or rename it.

11. **nit — rounding can lose surplus.** `basins.rs:329` vs 353-370. `input` (the node's) and Σ `inputs[k]` are summed in different orders. If `input < below` by an ulp while every child has `inputs[k] >= full[c]`, the surplus has no partial sibling and is dropped. The children are then "spilling" lakes whose spill goes nowhere, and the root reports `spills() == false`. The water lost is negligible. Fix: compare against `inputs.iter().sum()`, or treat "no partial child" as one lake.

12. **nit — absorption can be quadratic.** `basins.rs:215-223`. `own` vectors are moved into the target on each absorption. A chain of merges where the accumulated node is repeatedly the shallow one moves the same cells again and again. Flat, filled-up floors make that possible, though they are mostly excluded by LAKE_MIN_DEPTH. Fix: swap the vectors when the absorbed `own` is larger than the target's (order does not matter; `subtree_cells` sorts).

No crash, NaN or determinism problems found. A negative discharge into `powf` cannot happen: a depression that does not spill sets its discharge to exactly 0, and one that spills is a single root lake with evaporation < inflow.
