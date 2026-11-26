# Plan: plates break where their internal stress exceeds their strength (#123)

_2026-11-26. Request (Sebastian, #123): "Since we have non-rigid plates now, can this be solved
purely based on the internal stresses on a plate?" Goal: an Earth-like planet, not Earth;
effects of the ~110 km grid are accepted (#64)._

## What the code does now

* **Sheet (#70, `src/tectonics/sheet.rs`).** Every step, per plate, a deformation velocity is
  solved for a thin viscous sheet driven by the torque balance's forces on every cell (basal drag
  at the plate's rotation, slab pull and resistances at boundary cells, collision and yielding
  resistances, distributed ridge push, potential energy of thick crust). Samples whose
  viscosity exceeds `sheet.rigid_viscosity` (1e28 Pa·s·m) are grouped into **rigid blocks**
  with one rotation each; only weak samples carry a strain rate and a viscous stress. With
  `oceanic_min_strength` 0.35 every oceanic sample is rigid (2e31·0.35⁶ ≈ 4e28), and
  continental lithosphere is weak only below relative strength ~0.56 (young, orogenic,
  thickened). **So the stress inside oceanic plates and cratons is never computed**: the sheet
  cannot see a neck in a strong plate. The #70 results already noted this ("a strong (old)
  strip still cannot break (rigid blocks have no plastic yield)").
* **Existing split rules.** (1) Plume rifts (`events::rift_above_plumes`): a plume older than
  20 Myr splits its plate along the minimum cut of integrated lithospheric strength
  (`weak_zones::weak_zone_cut`) in a band around a great circle through the plume. (2)
  Subduction initiation at passive margins / fracture zones / boundaries splits off the
  subducting side (`split_locally`). (3) Sheet splits (`split_deforming_plates`): two rigid
  blocks of a plate separated by deforming lithosphere that move apart faster than 1.5 cm/yr
  for 5 Myr. (4) Detached pieces (`separate_detached_pieces`) become plates (≥ 1%) or are
  captured. Also sutures, captures of tiny plates and terrane accretion merge plates. None
  responds to the stress across a section of a strong plate.
* Minimum split part 1% of the surface (`events.min_plate_area`), plates < 0.3% are captured.

## Answer to the question

Yes. The stress inside a rigid block is not unknown: it follows from statics. A section that
divides a plate into parts A and B must transmit the net torque `T_A` of the external loads on A
(drag at the material's velocity, boundary forces, ridge push, GPE: exactly the sheet's forces,
which balance over the whole plate). The section can transmit at most
`C(â) = Σ_e F·s_e·ℓ_e·|â × r_e|` (traction ≤ `F·s` per metre; `F` the depth-integrated strength
of old lithosphere, `s` the relative strength `weak_zones::lithosphere_strength`, `ℓ_e` the
section length across edge e at r_e, `â` the torque axis): a rigid-plastic limit. The overload
`S = |T_A| / C` > 1 means the section yields. A neck of width w carries a bending moment of only
~F·w²/4, so arms of U/L-shaped plates and narrow strips fail first, wherever the loads on their
parts differ (slab pull along part of a trench, a continent's drag, ridge push), with no shape
rule. Weak lithosphere is already resolved by the sheet; for it the same statics holds (the
viscous stress is part of what the section carries).

**First evidence** (seed 0, 5 subdivisions, `--web-math` = Sebastian's web run, diagnostics
only): the U-shaped plate of the screenshot (plate id 475 at 403 Myr, 3% of the surface, aspect
6.4, a neck one erosion step wide) has its most overloaded section across its neck, ~400–500 km
long; its overload rises from ~0.9 (380 Myr) to 2–7 from 390 Myr on, with F = 10¹³ N/m.

## Implementation (`src/tectonics/breakup.rs`; already written as diagnostics, commit e068172)

1. **Loads.** In `deform_plates`, when the analysis is on, `cell_forces` is computed for all
   plates (identical values for the deforming ones). After the solve, per cell:
   `g = F_cell − drag·v`, with `v` the material velocity relative to the rotation the forces
   were computed at (frame change + deformation velocity). Per plate the remaining net torque
   (couplings across boundaries, solve residual, hidden samples) is removed as a drag-weighted
   rotation.
2. **Sections searched.** On the plate's largest connected piece: BFS distance fields (cell
   steps) from three far-apart cells (farthest-point sampling; for a strip or U they run along
   it, for a compact plate they sweep across it). Every level set `{d < c}` is a section;
   prefix sums of torque per level and the edges crossing each level give `S` for all levels in
   O(N log N). Both parts must have ≥ `breakup.min_area`.
3. **Weakest path.** If the best level section has S > 0.5, it is refined by a minimum cut of
   `ℓ·s` (the same max-flow network as `weak_zone_cut`, now `pub(super)`) between the cells
   more than `breakup.band` (3) levels below and above it; the part connected to the low side
   is kept. Of the level and refined sections the more overloaded one is used.
4. **Breakup.** A per-plate timer counts the time with S > 1; after `breakup.time` (5 Myr:
   localisation of a yielding zone into a boundary) the smaller part (as local vertices) is
   split off in the event phase (`break_overloaded_plates`, after the sheet splits) through
   `split_plate_cells`, so ids/colours of the larger part, boundary segments, polarity, sheet
   state and frames are handled like every other split. Timers and pending parts are reset on
   splits and merges.
5. **Switches.** `--set breakup.enabled=0|1`, `breakup.strength` (F, N/m), `breakup.time`,
   `breakup.min_area`, `breakup.band`, `breakup.diagnose`. Off (and the analysis off) →
   bit-identical to `main` (checked: same state hash with diagnostics on, seed 1, 100 Myr).
6. **Plate log** for analysis: `--plate-log MYR` writes `plate_maps.csv` (plate id per cell
   every MYR) and `plate_sections.csv` (per step and plate: overload, section length, part
   size, mean strength, timer). Changes nothing.

## Parameters and calibration

* **F** (`breakup.strength`): integrated strength of old lithosphere. Literature range
  ~5×10¹² – 5×10¹³ N/m (Kohlstedt, Evans & Mackwell 1995; Burov 2011; plate-driving forces are
  1–3×10¹³ N/m for slab pull, ~3×10¹² for ridge push). Calibrate within this range against: few
  overloaded compact plates (Earth's large plates do not break), narrow plates broken within
  tens of Myr (Farallon: a long strip from ~40 Ma, fragmented 30–20 Ma into Juan de Fuca,
  Cocos and Nazca), plate count and size distribution not exploding (`max_plates` 40).
* **min_area** 0.5% (~2.6×10⁶ km², Cocos-sized; ~200 cells at 6 subdivisions, ~50 at 5): well
  above the grid, below the 1% of the other splits; the new part is still above the 0.3%
  capture size.
* **time** 5 Myr (same as the sheet split), **band** 3 cells.
* Shape terms: none planned. If narrow plates survive with S < 1 despite visibly thin arms, I
  will report it rather than add one.

## Measurements

* Plate shape metrics (`scripts/shapes.py`): area, inscribed radius, share within 2 cells of
  the edge, intrinsic length L (double-sweep BFS), mean width W = A/L, aspect L/W, bend
  (L / great-circle distance of the ends: U/L shapes), narrowest neck (erosion depth at which
  the plate falls into two bulbs ≥ 3 steps thicker). "Narrow": aspect ≥ 5 or neck ≤ 2.
* Earth (`scripts/earth_plates.py`): PB2002 (Bird 2003) rasterised on the same 6-subdivision
  icosphere, same metrics.
* 8 seeds × 1000 Myr, `main` (= branch with breakup off and diagnostics on, same hash) vs
  breakup on: plate count and sizes, narrow plates per snapshot, narrow-episode durations
  (10 Myr maps), overload of narrow vs other plates, plus the usual log metrics
  (`notes/realism-batch-4/scripts/compare.py`: boundary types, speeds, hypsometry, land,
  runtime), window 500–1000 Myr, seed scatter and Welch t.
* Seed 0 at 5 subdivisions with `--web-math` at 403 Myr (the screenshot): plates and overload
  maps, before/after.
* Runtime at 6 subdivisions: the analysis is ~3 BFS + sorting per plate per step; measured.

## Risks

* Loads at boundary cells (slab pull 2×10¹³ N/m net of the resistance) are large compared with
  F·s for short sections; small trench-bound parts may be overloaded too often → many small
  plates. The minimum part size and F bound this; measured.
* Section search by level sets may miss oblique sections of compact plates; the min-cut
  refinement and three sources mitigate. Compact plates rarely matter for #123.
* Disconnected bits after the cut are handled by `separate_detached_pieces` (captured < 1%).
