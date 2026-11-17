# Plan: split plates without cutting through other continents (issue 1r follow-up)

_2026-11-17. User: follow the evaluation's recommended order (cap fix ✓, diagnostics ✓, then
this, then re-tune collision resistance, then margins/sediment)._

## Problem

Collision boundaries are 30–40k km long (Earth ~5000 km) at any collision resistance. Plume
rifts (`rift_above_plumes`: random great circle through the plume) and passive-margin
initiations (`initiate_subduction`: great circle tangent to the margin at one cell) split the
whole plate along a full great circle (`split_if_large_enough` → `split_plate`), cutting through
every continent on it. New diagnostic (commit 149d62a; seeds 0–3, 500 Myr, km of contact
between subduction-resisting crust on both sides created by the split):

| seed | rifts (n) | rift contact (far) | initiations (n) | initiation contact (far) |
|---|---|---|---|---|
| 0 | 13 | 58k (43k) | 28 | 149k (123k) |
| 1 | 21 | 91k (67k) | 39 | 189k (161k) |
| 2 | 23 | 122k (94k) | 26 | 105k (86k) |
| 3 | 24 | 142k (118k) | 31 | 126k (104k) |

"Far" = farther than `rift_continent_radius` from the plume / margin cell. 207–280k km per
500 Myr, ~4–5k km per split, against a standing collision length of 30–40k km (63–75%
converging, 19–26% diverging). A passive-margin initiation should create **no** continental
contact at all (its new boundary is ocean against continent), yet it creates the most.

## Design

Split by world cells (`split_plate_cells`, which already hands over polarity segments by
proximity) instead of by a half-space:

1. Components: connected sets (mesh neighbours, same plate) of cells whose crust resists
   subduction (`Crust::resists_subduction`, the predicate that defines a "Collision" contact).
2. The great circle still divides the **ocean** (cells that do not resist subduction): each
   such cell goes to the side its position is on.
3. **Passive-margin initiation:** every component goes wholly to one side: the margin's
   continent (the component(s) adjacent to the margin cell) to the continental side; every other
   component to the side holding the majority of its cells. Result: the new boundary runs
   through the ocean along the great circle and around continents along their coasts; no
   continent–continent contact is created.
4. **Plume rift:** the component under / nearest to the plume (within `rift_continent_radius`)
   is cut by the great circle (the rift); every other component goes wholly to its majority side.
5. Pieces detached by this rerouting are left to the connectivity rule (`separate_detached_pieces`,
   ≥ 1% becomes a plate, smaller ones are captured) as now.
6. `split_if_large_enough`'s minimum-area test runs on the actual piece.

## Verification

- Diagnostic: initiation contact → ~0; rift far contact → ~0; rift total contact should be of
  the order of one continent's width per rift.
- Tests: an initiation on a plate with two continents does not cut the second continent
  (no contact between subduction-resisting cells of the two plates); a rift cuts only the
  continent near the plume; masses conserved; polarity segments handed over.
- Runs (seeds 0–3, 500 Myr, `--material-budget`): collision length (and its converging share),
  breakups/captures, plate count, speeds, ocean age, mean land, > 50 km, flooded, felsic +
  sediment. Then step 4 (re-tune `collision_resistance`, expected 50–100).

## Plan review (independent agent, 2026-11-17): sound with changes — adopted

Required: (1) `seed_margin_subduction` would seed trenches along every coast of every
continent moved to the continental side — seed only near the margin cell (within
`rift_continent_radius`); (2) resolve disconnected pieces inside the split (keep each side's
main piece, flip the others to the opposite side; components stay whole, so no contact is
created), then the area test (failure = failed initiation), instead of leaving slivers to the
connectivity rule (more breakups/captures); (3) force the margin cell and its oceanic
neighbours to the oceanic side (its side is floating-point noise now). Components by
`resists_subduction` (the Collision predicate); the margin/plume continent by continental origin.
Plume rifts: orient the circle through the plume and the nearest continental cell so it cuts that
continent (a supercontinent is still cut along its length — expect one continent's width of
rift contact, which must diverge). The split diagnostic becomes zero by construction except for
rifts: verify on the standing collision length. Optional (not done now): ocean cells near a moved
continent follow it so boundaries stay ocean–ocean; flood-fill alternative for initiation.
