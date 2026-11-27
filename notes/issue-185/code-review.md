# Code review (independent agent, given the diff only)

Verdict: no correctness bugs (codec channel order and indices, NaN handling, months, web
transfer, zeroed test defaults, thresholds and quantisation edges checked). Findings and what
was done:

1. Stale "55 bytes per cell" in `history/mod.rs` → fixed (57).
2. Cell info could say ", ice" (frozen most of the year) for a lake the July view draws open →
   fixed: the Surface row now names the months as drawn ("ice most of the year, open in Jul",
   "ice in Jan", "ice in Jan and Jul").
3. A lake without the model's lake ice (newer than the last temperature update) got ", ice" from
   the cell's land/sea `ice` → fixed: no ice claim there (the realistic mode draws it by its own
   fallback rule). The topographic mode's permanent-ice colour for such cells is unchanged
   (as before this PR; it lasts one step).
4. The planet test counted frozen/open over all seasons → fixed: it now requires a frozen and an
   open lake in January or July.
5. The codec test's lake-ice values did not change with time, so deltas were always zero → fixed.
6. `history::codec` widened to `pub(super)` for the planet test → kept (harmless).
