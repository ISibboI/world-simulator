# Plan: end collisions locally by terrane accretion (issue 1r follow-up, step 4)

_2026-11-17. User: follow the proposed order (split bug, stretching, smoothed collision normals,
then local suture, then sediment routing)._

## Problem

Collision boundaries are 17–21k km long (Earth ~5000 km). The evaluation of the follow-up
(`agent-scratch/eval-followup/`, means 200–500 Myr, seeds 0–3) attributes **35–65%** of it to
plate pairs that can never suture: `suture_colliding_plates` (`events.rs`) merges a pair only
while its colliding cells are at least `suture_trench_ratio` (1.0) × its trench cells, and the
large plates almost always share a long trench elsewhere. Their suture clocks sit at 0 for
5–12k km of collision. 14–24% is diverging rift contacts; only 2–5k km is in pairs whose clock
ran ≥ 10 Myr. The trench-ratio rule is right that merging two plates that still subduct along a
long common trench would delete an active subduction zone — but then the collision persists
forever instead.

On Earth, collisions end locally: an arc, microcontinent or continental fragment on the lower
plate jams the trench, its slab breaks off, and the terrane is accreted to the upper plate
(Cordillera, Wrangellia, Taiwan, Timor/Banda); subduction then steps back behind it (subduction
jump) or flips (Ontong Java–Solomon). Only when the whole lower plate is continent do the plates
weld (India–Asia is the long-lived exception, with the collision ~2500 km).

## Design

In `suture_colliding_plates`, per pair whose collision clock reaches `suture_time`:

1. If the trench-ratio condition holds (collision dominates), merge the plates as now.
2. Otherwise **accrete terranes**: the colliding components — connected cells of crust that
   resists subduction on one plate, touching such crust of the other plate (the same components
   as in `local_split_piece`) — of the **lower plate** (the subducting side of the pair's
   segments at the contact, else the plate whose colliding components are smaller) move to the
   upper plate: `split_plate_cells(lower, component cells)` → new plate → `merge_plates(upper,
   new)`. Only if their total area is at most `terrane_max_fraction` (e.g. 0.5) of the lower
   plate; a component that is most of its plate means the plates themselves collide: merge
   only if the trench-ratio condition holds, else leave it (as now).
3. Reset the pair's collision clock. The ocean behind the terrane now meets the upper plate:
   if it is old enough and converging, the existing boundary initiation (`induced` subduction at
   boundaries) can start a trench there; otherwise it stays a boundary of kind `Other` until
   initiation. No new polarity is forced (measure first how often a trench re-forms).
4. Book and count: `terrane_accretions`, accreted cells; mass conserved (split + merge are
   sample-based and conserving).

## Verification

- Tests: a pair with a long trench and a colliding microcontinent on the lower plate: after
  `suture_time` the microcontinent belongs to the upper plate, both plates survive, mass
  conserved, the trench elsewhere keeps its polarity; a pair where collision dominates still
  merges; a huge colliding component is not split off.
- Runs (seeds 0–3, 500 Myr): collision length (target well below 17–21k, ideally < 10k km),
  converging share, stacked mass (Earth ~10–15 Gt/yr), plates, initiations/reversals, speeds,
  ocean age and ancient ocean, mean land, > 50 km, flooded, felsic + sediment. Then re-tune
  `collision_resistance`.
