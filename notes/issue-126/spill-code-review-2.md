# Code review 2: 278bd71 + ba040ab (#168, review fixes, spill frame, pore leftover)

Scope: `git diff a20521d ba040ab`. Read-only; nothing built or run. The bit-identity statement
below comes from reading the code, not from a state hash.

## Checked and found correct

- **Frames in `water_state` (ba040ab).** With `sea_spill`, `unloaded` = `to_dry(process)` for
  every cell. That is bit-identical to `spill_sea`'s `dry`, because both call `to_dry` on the
  same `end_elevation`. So the drainage's `filled`, `Lakes` and the spill level live in the same
  frame as `sea_level_from`'s `level` and `pass_levels`. When the sea stands on a sill below the
  frame's 0, the sill borders an ocean cell whose dry elevation is at most `level`: the flood
  path at or below `level` runs through cells that are ocean in the mask. The drainage therefore
  gives the basin `filled == sill dry` (+ the 1e-3 EPSILON chain), and the 5 m error is gone.
- **Other readers of the drainage at the start of the step (sea_spill on).** Ocean cells have
  dry elevation < −tol/W. Every land cell the flood reaches has `filled` ≥ −tol/W: coastal land
  is ≥ −tol, and closed depressions spill at a non-ocean sill. So all ocean seeds pop before any
  land cell in both frames. `order`, `receiver`, `Lakes` and `filled` of land cells are
  unchanged. The one exception: a land cell next to an ocean cell less than 1 mm (EPSILON) below
  sea level, where `filled = ocean + 1e-3` now differs by under a millimetre. That doesn't
  matter.
- **`to_process`:** ocean cells now get `elevation[cell]` (the process frame) for `filled` and
  `water_level`. Before, `filled[ocean]` was in the dry frame under sea_spill, which was a latent
  frame bug. Nothing reads `water_level` or `filled` of ocean cells anyway: the incision base
  (erosion.rs:615) and the lake spill in the sediment routing (:698) only use them for non-ocean
  cells.
- **`load_correction = 0` for ocean cells** is needed now that `elevation` ≠ `process` there. In
  the old path the value was `process − process = +0.0` before and is `0.0` now, so it is
  identical. `lake_depth` (−inf level) and `spill_depth` (NO_LAKE) stay 0 for ocean cells.
- **Old path (lake_hierarchy=0, sea_spill=0, ocean_lake_evaporation=0) stays bit-identical to
  592da7d (by analysis):**
  - `unloaded` is unchanged (gated on `sea_spill`).
  - `to_process` gives the same values where they are read. `filled[ocean]` already equalled
    `elevation[cell]` bit for bit. `water_level[ocean]` changes from −inf to the elevation but
    is never read.
  - `load_correction` is unchanged.
  - Without hierarchy, `Basins` has no children, so the `solve` rule change and `pour`/`hold`
    are not reached.
  - `ocean_mask(.., true)` and `wave_bases` (`!sea_spill` = true) are unchanged.
  - `spill_sea` and its `debug!` only run with sea_spill.

  A state-hash run of ba040ab would confirm this cheaply.
- **Water accounting of the pour (the question asked):**
  - `state = poured` recomputes the same lakes (same mask) plus the poured `D − left`. Then
    `lake_volume += left` gives `L = L0 + D`.
  - The tectonics gets `V_ocean = W − L0 − D = held_lower`, the water the ocean holds with the
    level exactly at the pass. In `sea_level`, popping the sill cell gives a level below the
    pass, so the solve returns `level = pass` with `deficit = max(0, target − held) ≈ 0` (±
    rounding). The sea stands at the sill without a deficit.
  - In the "joins the ocean" branch, `L` leaves out the basin, and the tectonics again puts D
    into the basin as ocean with a deficit. The water is closed either way.
  - Caveat (not new, #112 b): this holds exactly only in the erosion's frame (sample means). The
    tectonic solve uses per-cell surfaces.
- **basins.rs:**
  - The merge now keeps the children's volume (`hold` over subtree cells), then pours the rest
    into the merged lake. That conserves water up to bisection precision (2⁻⁶⁰ of the range,
    rounding up).
  - The leaf branch's `hold` from the lowest floor is equivalent to the old bisection from the
    current level.
  - The zero-evaporation rule only changes the case `below == 0 && input == 0`. The leaves then
    get `lake_balance(0)`, which puts the level at the floor with nothing covered.
  - No new panics: `cells[0]` is never empty, and `subtree_cells` is sorted by member index,
    which is ascending floor.
- **`ocean_mask(.., false)`** with `pass < −tol` is equivalent to `ocean_from(.., false)`.
- **Un-ocean branch:** `pass[c] == level` is an exact float comparison, but it is safe: both
  floods take the minimax of the same `dry` values, and `max` is exact.

## Findings

1. **should — no tests for any of the fixes.** There are no tests for:
   - the merge conserving volume (old finding 1)
   - the zero-evaporation rule (old finding 2)
   - the flooding of sills and rims (old finding 4)
   - the frame fix: a sill below the frame's sea level, next to an ocean cell
   - the pore leftover. The existing spill test (erosion.rs:3115) uses `sediment = 0`, so `pores`
     never matters, and its "joins the ocean" case goes through the `flooded` branch, not
     `left > unseen`.

   Fix:
   - Add a basins test: random or shallow hierarchies, checking that the change in lake volume
     equals the poured volume. The earlier reviewer's `review_*` tests in
     `~/tasks/issue-126/tmp/review-src` can be adapted.
   - Add a `spill_sea` test with the sill a few metres below the frame's sea level (sea level
     lowered by the solve) and non-zero sediment pores. Assert the spill level is the sill's dry
     elevation, and that `sea_level_from(W − lake_volume)` gives `level == sill` and
     `deficit ≈ 0`.

2. **nit — the `left > unseen` branch is effectively dead, and `unseen` misses the sill cells'
   pores.** erosion.rs:1153-1167.
   - `sea_level_from` returns at the first cell at the pass whose inclusion pulls the level below
     it. So `D` ≤ the capacity of the cells popped at the pass, which is at most the basin's
     capacity plus its pores, plus the pores of cells with `pass == level && dry == level` (the
     sill itself).
   - Lake capacity is measured up to `filled` ≥ sill, so it is at least the basin's. Hence
     `left` ≤ the basin's pores, plus its rim cells' share, plus the sill cells' pores.
   - The branch therefore fires only when the sill cell's pores exceed the slack. Then a full
     basin becomes ocean, which is acceptable.
   - A basin that is really overfull is handled one round later by `flooded`, where the solve
     stands above the sill.

   Fix: either count `pores` of all cells with `pass == level` in `unseen` (consistent with D),
   or drop the branch and say in the comment that `flooded` handles overfull basins.

3. **nit — un-oceaning only the `pass == level` cells can leave cut-off ocean pockets.**
   erosion.rs:1095-1104.
   - When the solved level is below the frame's 0 (the sea fell during the step), consider a cell
     X behind the basin's far rim: its rim `r` lies in (level, 0) and it reaches the sea only
     through the basin. X has dry pass `r > level` and stays ocean, while the basin around it
     becomes land.
   - X is then an isolated drainage seed and "ocean" in `surface_water.ocean` (used by
     planet.rs:339/378 and the climate), although at the sea's actual level it is above water.
   - The effect is limited to rims within the step's sea-level change. It is rare and cosmetic.

   Fix: after un-oceaning, keep only ocean cells still connected to `seeds` through ocean cells
   (a BFS like `ocean_from` over the current mask).

4. **nit — the rim flood can spread along the coast.** erosion.rs:1057-1075.
   - When `level > 0` (the sea rose during the step), the BFS from a flooded lake follows every
     connected non-ocean cell with `dry < level`, including coastal strips far from the basin.
   - The same strips elsewhere stay land unless a lake happens to be next to them. So the mask
     depends on where a lake is, not only on the topography.
   - It is small (cells within the step's sea-level rise) and physically "more correct" where it
     happens.

   Fix: none needed, or flood all `below` cells planet-wide whenever any lake floods.

5. **nit — the stats when no pour happens.** erosion.rs:1118, 1145.
   - `deficit = solved.deficit` is set before checking `poured`. If no basin cell lies in a
     `Lakes` entry (a basin made only of cells < LAKE_MIN_DEPTH deep), `poured` is `None`, and
     `sea_spill_deficit` still reports D as "poured over sills". The water itself is accounted
     for: the tectonics keeps it as the ocean's deficit.
   - Likewise, in the joins-the-ocean branch the deficit is reported as poured.

   Fix: set `deficit` only in the `state = poured` branch.

6. **nit — leftover water in the stats.** erosion.rs:1165-1166. `left` is added to
   `lake_volume` and `sea_water`, but no lake body holds it. `sea_fed_cells` and
   `sea_fed_evaporation` do not reflect it, and `lake_depth` and `water_level` don't show it. The
   snapshot line ("sea water … km³") then includes pore and rim water. That is consistent for the
   water budget, but say so in the log or doc, or report it separately (e.g.
   `sea_pore_water`).

7. **nit — the zero-evaporation rule changes body counts.** basins.rs:329. A frozen or
   zero-excess depression without inflow now yields several empty lake bodies (covered 0)
   instead of one. As a result, `lake_bodies` and `split_lakes` count dry sub-basins. Fix: skip
   bodies with `covered == 0` in the counts, or accept it.

8. **nit — the tectonics still opens sills.** tectonics/mod.rs:1340 passes `true`. So the
   tectonic statistics (statistics.rs:390: sea-floor area and ages) still count a sea-fed basin
   at its sill as sea, unlike `Planet::statistics` and the climate. The tectonics has no access
   to `sea_spill`. Fix: pass the flag down (e.g. a `TectonicParams` field set from the erosion
   params), or note the difference in the doc of `Simulation::ocean_mask`.

9. **nit — a compiled Python file is committed.** 278bd71 adds
   `notes/issue-126/scripts/__pycache__/pits.cpython-313.pyc`. Older notes have the same, but
   it is build output. Fix: remove it and add `__pycache__/` to `.gitignore` (a separate
   cleanup for the older ones).

No bugs found that lose or create water, panic, or break the old path.
