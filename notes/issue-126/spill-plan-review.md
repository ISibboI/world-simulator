# Review of notes/issue-126/spill-plan.md

Reviewed against branch agent/issue-126 (592da7d), src/erosion.rs, src/sea.rs, src/planet.rs, src/climate.rs, src/tectonics/mod.rs, and the logs and cell dumps in agent-scratch/runs/after-nosea and agent-scratch/runs/after.

## Summary

The physical rules (1–6) are sound for an Earth-like planet at 1 Myr steps. Algorithm A is basically correct. The plan's description of what already exists (fill and spill between depressions, the cascade order, the deficit) checks out against the code. But the diagnosis of the huge inland seas misses the main cause, and B as written would make that cause worse. Most of the big "lakes" are lakes with zero evaporation on cells the climate treated as ocean. This comes from a knife edge at a sill that stands exactly at sea level.

## Findings

### 1. must-fix: the diagnosis misses the main cause, zero lake evaporation on cells the climate saw as ocean

- **What the code does.** `climate.rs:647-661` sets `lake_potential = 0` (and computes ocean evaporation instead) for every cell with climate elevation < 0. `climate_elevations` (planet.rs:821) makes cells < 0 only where `ocean_from` reaches them, and `ocean_from` passes sills with `e < SILL_TOLERANCE`.
  - When the sea stands at a pass (deficit > 0), the sill's surface elevation is exactly 0. So at the start of the next step the climate sees the whole basin behind the sill as ocean.
  - The erosion's end state uses `ocean_from(end_elevation)`, where `end_elevation` uses sample means, the fluvial shift and the step's deposition. There the sill is almost never exactly 0, so about half the time the basin is land.
  - Then `excess = max(PE_lake − ET, 0) = 0` on every basin cell, `lake_balance` can never balance, and the lake fills to its spill level with any inflow.
- **Evidence from the dumps** (`ocean` column = `ocean_mask` with the tolerance):
  - seed 1, 850 Myr: of 7792 lake cells below sea level, 7706 have `lake_evap = 0`, while the climate's ocean evaporation `evap` there averages 2.28 m/yr. Of 8177 lake cells, 7761 are also ocean in the mask. The log shows "lake potential evaporation 0.20 m/yr" and "lakes 38.4% of land".
  - seed 0, 1000 Myr: 1563 lake cells, 1103 of them ocean in the mask, 1087 with zero lake evaporation.
  - seed 2, 1000 Myr: 3021 of 3048 lake cells are ocean in the mask, 1178 have zero lake evaporation.
- **Not specific to rule-off runs.** `after` (rule on) seed 2 has a 1915-cell lake below sea level, 196 m of sea level, lake PE 0.10 m/yr, deficit 0. The likely cause is a strait that closes during the step: the seeds are fixed at the start, the end state cuts the basin off, and the climate saw ocean.
- **Consequence.** The 906 m "full lake" of seed 1 is not "the instantaneous equilibrium itself is right". It is an evaporation-free artifact. With land-like PE (~1.5–2 m/yr over ~10⁸ km², about 1.5×10⁵ km³/yr against 8×10⁴ km³/yr of global runoff) that basin would be largely desiccated, Messinian-style.
- **Suggestions.**
  - (a) In `water_state`, for cells whose climate saw ocean (lake PE 0 because elevation < 0, not because of ice), use the climate's open-water evaporation (`water_cycle().evaporation`, which already exists per cell) as the lake's potential evaporation. This is cheap, physical, and independent of the masks.
  - (b) Rewrite "Why huge inland seas" to give this cause. The deficit is the second effect.

### 2. must-fix: B, as written, keeps the sea exactly on the sill, so the start-of-step ocean, climate, stats and end state flip

- B's goal is "the tectonic solve finds the ocean exactly at the sill". That makes the sill's surface exactly 0, which `ocean_from`/`ocean_mask` (SILL_TOLERANCE) treat as passable. In the next step the climate and the erosion's start (sediment routing, wave base, marine diffusion) then see the sea-fed basin as ocean.
- This brings back finding 1, and the cycle can oscillate. Step n: sea-fed partial lake. Step n+1: climate says ocean, so lake PE is 0, the basin's own lake fills to spill, and B sees "V_own + D > capacity" and makes it ocean. Step n+2: the tectonic solve again stands at the sill with a deficit…
- The frames make it worse: the erosion's sea-level solve (sample means, start-of-step seeds) and the tectonic solve (per-cell crust, `ocean_flood` reseeded at the current sea level) differ. So after `set_lake_water(L + D)` the tectonic solve lands randomly at the sill with a small deficit, or just below it.
- **Suggestions.**
  - (a) Change the sill semantics. A sill at which the sea *stands* is closed (passable only if `e < −tol`). A basin behind it is a sea-fed lake everywhere: climate, erosion start and end, `ocean_mask`, stats. This replaces the "arm of the sea" reading in the SILL_TOLERANCE doc, which B makes obsolete. A full basin becomes ocean only when the sea is above the sill. Consider a small hysteresis margin of order 1 m, since a lake at the sea's level and the sea are physically the same.
  - (b) Compute D in the frame that defines the next step. Let the tectonics' `set_lake_water(L)` return its deficit `D_t` (after `apply_surface_changes`). Place `D_t` in the erosion's end-state basin geometry (keep the end `Lakes`/hierarchy in the result), then `set_lake_water(L + D_t)`.
  - (c) Log the residual tectonic deficit per step (debug) and assert it is small.

### 3. should: the coupling between L and the sea level is solved once; state the one-step lag

- B solves the sea level for `W − L_new` while the end lakes were computed with the ocean at the frame's 0, which was solved for `W − L_prev`.
- If L changes a lot, for example when a large zero-PE lake dries after fix 1, the sea moves by tens to hundreds of metres. Cells between 0 and S are then treated as land or lakes although they are flooded, and vice versa.
- Accepting a one-step lag is fine (the next step corrects it), but say so. Also make sure that the "made land / made ocean → recompute water state" branch re-solves D after recomputing L. Otherwise V_own is counted twice: when the basin was ocean in the end state, the first solve's deficit already contains its own water.

### 4. should: B's "basin's depression" is not always one `Lakes` entry that holds water

Cells behind the pass with floor < p can fall outside a usable `Lakes` entry in several ways:

- They are excluded by `LAKE_MIN_DEPTH`. A broad flat floor 0.5 m below p over 1000 cells holds ~6000 km³ and has no `Lakes` cell.
- They are split into several `Lakes` entries. `Lakes::new` starts a new entry wherever the receiver chain passes through an excluded cell.
- They belong to a depression with `holds_water == false` (`lake_onset_depth`, not continuing).
- They form two basins sharing the one sill cell.

Suggestions:

- Define the sea-fed basin from the sea flood (pass == p, floor < p) and place D by fill-and-spill from the sill over those cells, whatever their `Lakes` membership.
- Sea inflow overrides `lake_onset_depth`.
- Compute the volume with `lake_water_depth` (WATER_LOADING), so that it matches the sea solve's `(S − d)·W` and `hydrology.lake_volume`.

### 5. should: A's spill level per node is mis-stated

- "spill level its merge level" should be: spill level = the *parent's* merge level (root: the depression's spill). F(n) is the evaporation of n's subtree filled to that level.
- The merged lake of n then lies between m_n and spill(n), and `lake_balance` over the subtree cells sorted by floor is correct, because union–find guarantees that every subtree cell below the level is connected.
- With this definition the "fullness depends only on total input" argument holds:
  - If n is not full, nothing leaves it, so its evaporation equals X(n) < F(n).
  - Overflow over the shared sill cell always reaches a partial sibling.
  - Equality ties with zero-excess cells (frozen lakes, `lake_potential = 0`) are harmless if the code uses `≥` consistently with `lake_balance`.

### 6. should: A, routing and edge cases

- **Steepest descent must be computed fresh.** Inside a depression, the priority-flood receivers point *towards the exit*, not downhill. Use the lowest neighbour with a lower floor that is in the same entry.
- **Inflow entry points.** Split `discharge[exit]` into the members' own runoff plus, per member, the discharge of non-member cells whose receiver is that member (rivers and upstream lake overflow).
- **Sibling overflow.** With ≥ 3 siblings, overflow must cascade (a sibling filled by overflow passes its surplus on). Iterate until stable, ordered by the lowest cell next to the sill among the *partial* siblings.
- **Components that never merge below the root.** Sub-basins linked only through cells excluded by `LAKE_MIN_DEPTH` stay separate. Make the root a k-way node with merge level = spill.
- **Spurious leaves.** Flat floors, ties and rounding noise create leaves with zero or tiny capacity whose "full" state still demands the evaporation of a fully covered cell. Require an inner-sill prominence (≥ `LAKE_MIN_DEPTH` above the leaf's floor, or a few metres), otherwise merge into the parent.
- **Statistics and fields.** `covered`-prefix indexing over `lakes.cells[lake]` is used everywhere (water_level, lake_evaporation, lake_volume, inland_water_cells, playas, lake PE). Replace it with per-cell coverage from the hierarchy, and test that the per-cell lake evaporation still sums to the balance total.
- **Default-on changes the state hash.** Expect golden/hash tests to change. Keep the `lake_hierarchy=0` path literally untouched for bit-identity.

### 7. should: sediment trapping keeps the old simplification

`fill_lake` still levels sediment over the whole depression from the lowest floor up, so sediment lands in sub-basins no river reaches. Apply the hierarchy there too (deposit in the entry leaf, spill to siblings), or list it as a known remaining inconsistency.

### 8. should: the 0.2% rule is not what its doc says, so C's discussion is incomplete

- `sea_seeds` counts the basin **filled to its pass towards the largest body**, at whatever height, not the water body below sea level (the MIN_SEA_FRACTION doc says "bodies of water below sea level of at least this fraction").
- A small sub-sea-level pit inside a large high closed basin (Turpan-like, pass at +1000 m) is therefore seeded as sea at sea level, with no sill limit at all. That is clearly unphysical and should be stated in C.
- In C, also state the real trade-off. At ~110 km cells a 10–30 km strait (Gibraltar, Bosporus) has a cell-mean sill above sea level. Without any strait parametrisation, Mediterranean-type seas become permanently sea-fed drawdown basins (a Messinian salinity crisis as the norm). This is a grid effect (accepted per #64), but it must be explained to Sebastian, and it is the case *for* some sub-grid criterion.
- **Measure:** for every extra seed, the sill height above sea level, cells below sea level, and basin cells at the pass. If the rule stays, at least apply it to the body below sea level as documented.

### 9. should: the measurement plan misses the key diagnostics and the rarity of the events

Add:

- lake cells with zero lake PE, and lake cells that are ocean in `ocean_mask` or in the climate (both should go to ~0);
- per-step ocean↔lake flips of sea-fed basins (from the per-step lake log);
- the residual tectonic deficit after `set_lake_water` per step;
- water closure per step: tectonic ocean held + L + deficit + pores = W.

Large basins occur in 3 of 8 seeds in a few snapshots, so snapshot means and Welch t are dominated by single events. Report per event (case studies: seed 1 850, seed 0 900/1000, seed 2 1000, with time series per step) and distributions (max lake water per seed, share of snapshots with lake water > 10 m of sea level). Also measure the cost of the extra `water_state` recomputation in B.

### 10. nit: physics notes for the write-up

- **Hydraulic head.** The sea feeding a basin through a strait stands above the sill by a hydraulic head. For E − I ≈ 10⁵ km³/yr over a 110 km wide sill that is ~8 m (broad-crested weir). Neglecting it is within grid accuracy; mention it.
- **Endorheic share.** On Earth the Mediterranean catchment counts as exorheic, so for the Earth comparison count sea-fed basins' catchments as exorheic in the headline number.
- **Salt.** A sea-fed basin with E > I imports salt (km of Messinian evaporites). Not tracked; mention as a caveat.

### 11. nit: the tectonic solve can double-count lake water in drowned basins

`sea::sea_level` counts closed basins below the pass as empty ocean space, while their lake water is already in L. This only matters if the sea rises above a lake's sill in the tectonic frame; with B and finding 2a it should not happen, but log it (lake cells that are ocean in the tectonic flood).

### 12. nit: the end-of-step seeds are the start-of-step seeds

With the rule on, a basin of ≥ 0.2% cut off *during* the step loses its sea status at the end (the likely cause of the rule-on 196 m case). Either reseed the end state with `ocean_seeds(end_elevation)`, or rely on finding 1a.

## Verdict

Rules 1–6 and A are fine after findings 5–6. B needs findings 1, 2 and 4 before implementation. Fix 1a, together with the closed-sill semantics of 2a, probably removes most of the "huge inland seas" by itself, and should be measured on its own (ablation) before A and B are judged.
