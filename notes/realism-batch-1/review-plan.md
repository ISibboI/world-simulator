# Review of the batch 1 plan (#47, #46)

_2026-11-20, independent reviewer. Code read at the worktree's HEAD; no runs._

**Verdict: approve with changes.** The diagnosis of #47 is right and the graded fan pass is the
right fix. The upstream aggradation (change 2) will not store sediment in this model as written,
and the 12 m `LAKE_MIN_DEPTH` (change 3) removes the process that currently erases pits. Both
need the changes below before implementation.

## Findings, most important first

1. **Aggraded sediment is incised again in the next step (change 2).** Incision is detachment-
   limited with no transport threshold (`erode`, src/erosion.rs:471-530). For a trunk at slope
   1e-4 with Q ~ 1e4 m³/s: K Q^0.5 ≈ 0.08 /yr, F = 2·K Q^m dt / L ≈ 1.5, so the implicit solve
   removes most of the 12 m per cell above the receiver in one step. The line is cut back, the
   sediment returns to the same mouth, finds the same full sea and is aggraded again. Storage is
   ~zero over time, and "sediment to the ocean" and "produced" are inflated by recirculation.
   Change: either make the grade the model's own equilibrium (no incision where the slope is at or
   below the transport slope, i.e. `base = max(receiver, receiver + alluvial_slope·L)` for
   sediment-covered trunk cells), or drop change 2 and let the sea take it (see 2). Add a
   two-step test: aggrade, then erode with no new load; the aggraded mass must mostly stay.
2. **The capacity of upstream aggradation is small and one-off.** It only fills cells below the
   grade line anchored at the mouth; most trunks are already steeper than 1e-4 (mean land
   ~550 m). A 10-cell lowland holds ~1.5–2 km of rock-equivalent sediment in total, while one
   large river leaves "tens of km" per step. On Earth a full receiving basin makes the delta
   prograde (sea becomes land, the mouth moves seaward), not the river aggrade 1000 km inland.
   Change: rank the fan pass first (it fixes 53% + most of 42%), then let the delta plain
   prograde along the sea path beyond `delta_reach` on any zone (graded seaward at the
   alluvial slope from `delta_top`), and use upstream aggradation only for the last remainder.
   Measure overfill after the fan pass alone first; the ≤ 100 target may already be close.
3. **A 12 m `LAKE_MIN_DEPTH` stops pits from being filled and lets rivers cut below the spill.**
   Pits vanish today (<1% survive 100 Myr) because `fill_lake` (src/erosion.rs:1004-1041) fills
   every depression > 1 m to its spill. Below 12 m, `Lakes::new` (src/erosion.rs:915) drops them:
   no trapping, and `water_level` stays −∞ (src/erosion.rs:413), so the incision base of cells
   draining into the pit is the pit floor (src/erosion.rs:479). Neighbours cut down to it and the
   depression widens. The "land draining into pits" target is then met by definition while the
   pits persist. Change: separate the two roles. Keep sediment trapping (fill to spill) for all
   depressions > 1 m; apply only the water balance / endorheic status from the new threshold;
   and use the spill level (`filled[receiver]`) as the incision base for shallow depressions.
   Also: at 12 m, multi-pocket depressions whose pockets connect through cells shallower than
   12 m split into several lakes whose exits drain to non-lake cells inside the depression
   (src/erosion.rs:918-927); check `lake_balance`'s spill for such exits.
4. **Shared crust samples double deposits.** `apply_surface_changes` sums the deltas of all
   cells showing one sample (src/tectonics/mod.rs:1195-1205), so a cell whose sample is shown
   twice gets twice its deposit on both cells. Half the pits sit on duplicated samples; a pile or
   an aggraded trunk cell on a shared sample rises 2×. This is a spike source the erosion tests
   cannot see, and it breaks the "≤ alluvial_slope·spacing above the downstream cell" rule in
   the full run. Change: report spikes and slope violations split by shared/unshared sample;
   open a tectonics issue if shared cells dominate (do not fix it in this batch).
5. **The attribution switches are wrong.** `fan_gradient=0` makes the graded surface flat at
   `slope_depth` everywhere (more fill, not off). `alluvial_slope=0` would set the lake
   threshold to 0 m (every 1 mm hollow a lake, worse than main's 1 m). Use explicit booleans
   (`erosion.graded_fans`, `erosion.aggradation`, `erosion.lake_min_depth` as its own number).
6. **Sea-level guard rails will move more than "rise".** 0.55 km³/yr more to the sea is ~15% more
   than the 3.75 delivered; the sediment part (283 m at 1000 Myr) should end ~10–20% higher, and
   flooded continental crust rises with it. Today this mass mostly becomes land metasediment
   (spikes metamorphose at 11 km bulk); afterwards it metamorphoses at fan heads on oceanic crust
   (`tracked = 1`, src/tectonics/mod.rs:1148-1152) and subducts. So burial metamorphism shifts
   from continental to oceanic, felsic + sediment net and trench sediment rise. Write these
   expectations into the targets before the runs, with rough magnitudes.
7. **Uncapped graded fill reaches trenches and ancient basins.** The fan path accepts any ocean
   cell within `fan_climb` (src/erosion.rs:1286-1293), including trenches (−6 to −10 km) and the
   880 Myr basins of #14 that already hold 45% of oceanic sediment. Without a cap they become the
   main sink. Trench fill is real (Makran, Cascadia) but feeds subduction. Change: add guard
   rails "sediment on oceanic crust ≥ 200 Myr" and "max/mean sediment on oceanic crust"; decide
   explicitly whether trench cells (by plate boundary) take part.
8. **Implementation details for the fan pass.** (a) Do the second pass inside `fill_fan` on the
   same `path`, before the marks are reset (src/erosion.rs:1296-1298); the BFS stops when
   `remaining` hits 0 (src/erosion.rs:1270), so the path is complete only when the first pass
   left something, which is the case that matters. (b) Use hops·spacing as in the first pass.
   (c) Anchor: the plan uses `slope_depth` (−2000 m); the cited Bengal head is ~−1400 m — fine,
   but say so. (d) Fill nearest first; mouths are processed by index, so an early small river
   can take a shared basin from a later large one (already true; note it). (e) A sill higher
   than `lowest + fan_climb` blocks the path within the step even if the ponded fill reaches it;
   it opens in later steps, acceptable.
9. **Implementation details for aggradation (if kept).** Walk in pass B on `seabed.elevation`
   (post-incision, shifted to the new sea level), anchored at 0 / `delta_top`, never on the
   pre-step `elevation`. After the shift, `source_cell` may itself be below sea level; start at
   the first cell ≥ 0. Trunk donor: scan the neighbours `n` with `receiver[n] == cell` and take
   the largest `discharge[n]` (it is already net of lake evaporation, src/erosion.rs:435).
   Stop at the first lake cell (`lakes.lake[c] != NO_LAKE`): the lake's level and endorheic
   status were computed before and would not be updated.
10. **Physics magnitudes are acceptable.** Mean fan gradients of 1–5 m/km are right as averages,
    but fans are concave (upper fan 5–10 m/km, lower <1 m/km, Amazon/Bengal); a constant 2 m/km
    overfills the far fan and underfills the head. A geomorphologist would ask for a concave
    profile (e.g. gradient falling with distance over `fan_length`). The 16.5 km of Curray et
    al. is the total under the northern Bay of Bengal incl. pre-fan strata, not fan alone.
    Alluvial slopes 1e-5–1e-4 are right for large lowland rivers; 1e-4 is the steep end.
11. **Other single-cell paths checked.** `None` receiver (src/erosion.rs:569-573): only planets
    without ocean. `fill_lake` is bounded by the spill. Wave "stayed" (src/erosion.rs:1184-1186)
    returns a cell's own material; cliff bedrock → sediment raises the cell slightly (density
    2800→2050, fraction ≤ 0.3% per step), negligible. Delta and enclosed-sea fills stop at
    `delta_top`. Metamorphism does not create relief; it fixes it (crust erodes 2× slower). A
    pile also diverts the trunk in the next drainage, so it loses its discharge and survives.

## Tests and validation to add

- Two-step recirculation test (finding 1); a 5 m pit on the path still fills with sediment and
  no neighbour is cut below its spill (finding 3); a fan path through a trench cell; the sea-level
  budget test (`routing_keeps_the_sea_level_budget`) with a load large enough to use the
  uncapped pass.
- Validation: share of land cells that are pits and their 100 Myr persistence (not only the
  drained area); overfill split by cause (fan reach / narrow basin / small sea) after each
  change; max sediment on oceanic crust and on crust ≥ 200 Myr; burial metamorphism split
  continental/oceanic; spikes on shared vs unshared samples; a fan surface profile from a
  snapshot to check the achieved gradient.
