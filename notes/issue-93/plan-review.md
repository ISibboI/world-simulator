# #93 Plan review (independent agent) and what was done about it

1. **Collisions were drawn by crust only** (`BoundaryKind::Collision` = continental crust on both
   sides, whatever the motion): rifting continents showed as collisions. Fixed: collision
   cells go through the kinematic tests too (converging → collision, diverging → rift, along
   strike → transform, slow → inactive); the tests now check this without filtering collisions.
2. **Overriding side duplicated the trench condition** from one neighbour's crust. Fixed: a second
   pass marks a cell overriding (or diffuse) when a neighbouring cell of the other plate is that
   plate's trench (or diffuse zone) under this plate. No duplicated condition.
3. **Priority order** plan vs code: now subducting, overriding, collision, … in both; teeth are
   drawn from either side (subducting wins, then overriding).
4. **Rift band** now matches the stretching: rings `ceil(rift_width / spacing)`, crust of
   continental origin thicker than the breakup thickness, opening faster than
   `min_stretching_rate`.
5. **Collision band** now walks the whole plate within the rings (also across its oceanic cells)
   and marks the crust of continental origin, as the simulation's band. It is drawn only for
   converging collisions on the upper plate of the segment (documented).
6. **Combine rule** for a line between two cells: subducting side → trench with teeth on the
   other side; else an overriding side → trench with teeth on it; else the more important class
   of the two (`plates::Line::of`, tested).
7. **Frame format**: version bumped to 2 (16 B per plate); flag bits are constants on `CellInfo`
   with tests through `cell_infos` and the codec. Size impact measured (results.md).
8. **Cost** of `boundary_map` per update: measured (results.md).
9. **Colours**: saturated boundary colours with dark borders over muted plate colours; plate
   adjacency is computed only when an unseen plate id appears; colours are assigned in
   `receive` before the frame is recorded; unknown ids fall back to `id % 16`.
10. **Overlay/legend**: `RenderMode::overlay` is the one hook (its default draws `arrows()`);
   relative motion has its own arrow scale (reference 15 cm/yr); positions along boundaries are
   anchored to a fixed spatial grid (`overlay::anchored`) so they do not jump between frames.
11. The cell widget's "Plate" row shows the stable id (as the colours); `tangent_towards` moved
   into the tests.
