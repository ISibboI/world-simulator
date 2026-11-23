# #58: independent evaluation of the results and the diff

_2026-11-22, by an independent reviewer agent (read-only; diff at the `b3` commit, runs main,
b1–b3). Condensed; [`results.md`](results.md) says what was changed after it._

1. **Main finding: the size rule made the solve depend on its reference level.** The solve
   chose the bodies to seed by `dry < reference` (the previous sea level), the masks by the
   surface at the new level. A basin of 82 cells below S0 is seeded and filled, the sea level
   drops to S1 where it has 78 cells; at S1 the mask does not seed it (erosion, climate and
   statistics treat it as land and a lake, `set_lake_water` takes its water a second time), and
   the next solve does not seed it either, so it can flip back. Evidence: budget terms
   main → b3 (m/100 Myr) metamorphism −4.2 → −15.7 (t −5.6, not reported), sharing +28.8 →
   +37.7, lakes +4.0, while b2 (no size rule) has −2.3, +27.3, −7.8. Fix: a size that does not
   depend on the sea level (e.g. the area at the basin's own spill level), the same seeds in
   the mask, and a test at two reference levels.
2. Smaller code points: in the deficit case the mask includes the basin and the surface shows
   it filled to the sill, while it holds only the deficit (negligible in b3, state it); a cut-off
   basin's sea water goes back to the ocean at once and the lake balance refills it at once
   (fits the steady-state lakes of #44, say so); lake volume uses unloaded depths while floors
   carry the water load (name with the loading caveat). Checked correct: the flooding solve with
   tie groups and deficit, pass B `reached`, the incision base, the diffusion floor, waves and
   wave base, drainage seeding.
3. Results support the main targets (no mouths on inland water, lake sediment 0.32 → 0.79,
   sediment to the ocean unchanged, deficit ≈ 0 in b3). The budget explanation is incomplete
   (metamorphism and coastal missing; the lakes-term story cannot explain b2's negative sign;
   the dynamic-topography check does not test sills). Lakes +1.2 points of land: only 0.7 are
   the depression cells. Plates > 1%, rifts (+2.3), ridges, slab pull move together in b3 only:
   report, and do not call them chance until item 1 is fixed.
4. The sill rule is sound. The size rule is an honest stopgap; a more physical alternative is
   sub-grid relief at the pass; the root of b2's deserts is that lakes are no moisture source.
   Closed seas drying quickly is physical (Messinian), only 100–200 Myr without reflooding is not.
5. Missing: per budget term, cells changing ocean membership per solve; how often bodies cross
   the threshold and solve and mask seed different bodies; all budget terms and rifts in the
   table; the over-displayed water in the deficit case; a brief re-measurement after the fix.
