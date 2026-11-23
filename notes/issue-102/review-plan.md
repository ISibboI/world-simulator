# Review of notes/issue-102/plan.md (independent, 2026-11-24)

Verdict: sound. Exporting from `erode` is the right call. Fix the points below before implementing.

## 1. Is the export correct and complete?
- **Endorheic-exit cut: correct.** `Lakes::new` makes every lake a receiver subtree rooted at its
  exit, and the exit's receiver is never a lake member (it is processed first and would otherwise
  have absorbed the exit). So "the path ends at a depression cell with no receiver" gives exactly
  the sim's `terminal` set (src/erosion.rs:519-537). Build the cut receivers in a **separate vec**.
  Never touch `receiver`, which incision and sediment routing use further down.
- **Lake chains: fine.** A spilling lake's exit drains through ≥1 non-member cell into the next
  lake. If that lake is endorheic, the walk ends at its exit, the same as the sim.
- **Spilling vs closed per lake is derivable, but the plan doesn't say how.** Lake identity is
  "follow receivers while the depression bit is set". The last such cell is the exit, and the lake
  is closed iff that exit has no receiver. Write this down and unit-test it, including two lakes
  in a chain and a spilling lake that feeds a closed one.
- **Partly covered last cell:** depth = level − floor is right for the #128 rule. Also note:
  (a) cells are in floor order, so the partial cell can have < 1 m and draws as dry. That is
  consistent with the rule, but the view's "lake share" (count of cells ≥ 1 m) will not equal
  `hydrology.lake_cells` (fractional coverage, ~3.6 %). Compare like with like in the
  plausibility check, or say in the legend which one is shown. (b) Spilling lakes:
  `spill = filled[exit]`, and members are > 1 m below their own `filled`, so their depth is ≈ ≥ 1 m.
  Fine.
- **Ocean cells:** receiver None by construction, so OK. **No-ocean planet:** the flood root (the
  lowest cell) is land with no receiver and no depression bit. The view must treat it as a
  terminal (endorheic) and not crash or call it a mouth. Add a test for it.
- **Time lag (main gap):** receivers, the ocean mask and depth refer to the elevation *before*
  erosion/sea-level change, while the frame stores the elevation *after* `apply_surface_changes`.
  Discharge already has this lag, so it is acceptable, but:
  - Store depth (not level), as planned. Never recompute depth against the frame elevation
    (lake infill would make it wrong).
  - All other modes and `format_rows` use `elevation < 0` as ocean. The hydrology mode will
    show slightly different coastlines (deltas, pre-step mask). That is OK, but the cell widget
    must use one source in this mode (the bit). Say so for #128.
- **Erosion disabled (`self.erosion == None`):** the plan keeps "the ocean mask of the initial
  topography", which then goes stale as plates move. Recompute the mask every step from the
  `seeds` already computed in `Planet::step` (`sea::ocean_from`), or mark the data as
  unavailable and hide the mode's content.

## 2. Risk to simulation results
Low. `state_hash` covers discharge, not the new data. Keep it a pure read-out after the existing
loops. No new HashMap; the receiver slot comes from `mesh.neighbors(cell).iter().position(..)`
(deterministic). Do not reorder or merge the existing loops when adding the collection (FP
summation order in `hydrology.*`). Check the hash on 2 seeds, also with erosion off.

## 3. Codec
- **Integer8 is hard-wired to `flags`** (codec.rs:208/259). The second Integer8 channel needs a
  per-channel field mapping. Small, but plan for it.
- **Threshold flip:** with `Logarithmic(1/1000)`, a live depth of 1.0004 m can decode below 1 m.
  The same frame would then draw as lake live and as land in history. Put "lake (≥ 1 m)" as
  **bit 5** in the drainage byte (bits 5-7 are free) and classify on the bit. Then depth is only
  used for colour and the widget, so 1 byte is enough: e.g. Logarithmic(~0.035) covers 0–20 km
  at ~3.5 % relative, which is plenty for a log colour scale. Result: 41 instead of 42 B/cell.
- Bit 3 (ocean) is redundant-ish: receiver none ∧ ¬depression ⇒ ocean, except the no-ocean
  root. Keep it anyway; it is free and explicit.
- Bump VERSION to 5 and keep the delta (wrapping_sub) path. The round-trip test must cover
  slot 7 (none) and all bits.

## 4. Simpler / smaller things
- `CellInfo.drainage` can be `u8` (+ `lake_depth: f32`). It is mostly padding either way.
- Rivers overlay: skip segments whose cell and receiver are both lake cells (bit 5). Otherwise
  "rivers" get drawn across lake surfaces. End lines at the lake shore, as you already do at
  the coast.
- Basin colour by hash of the outlet: mouths move with deltas, so colours will flicker between
  steps in playback. That is acceptable. Hashing the outlet's *basin-largest* cell is no better.
  Keep it simple.
- Phone UI: three options plus a legend with live numbers. Keep the options in the collapsible
  panel with touch-sized controls (shared egui code; ~44–48 px on coarse pointers).
