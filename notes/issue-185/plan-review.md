# Plan review (independent agent) and how it was taken up

Findings of the reviewer (condensed), with the response:

1. **The model's cover is not "frozen".** Lake cover = ice thickness / `ice_albedo_thickness`
   (0.5 m), capped at 1. "Cover > 0.5" means a monthly mean thickness above 0.25 m; a lake under
   10–20 cm of ice all January would be drawn open. → **Taken up:** the months are drawn frozen
   where the month's mean cover exceeds 0.1 (≈ 5 cm mean ice); the annual state keeps the
   permanent-ice rule of the other modes and the "Permanent ice" row (annual cover > 0.5,
   frozen most of the year; same smoothstep(0.3, 0.7) as sea ice, crossing 0.5 at 0.5).
2. **Model lakes and viewer lakes come from different steps** (the temperature update sees the
   previous erosion step's lakes; `Planet::new` has none). → Gate on `lake_ice_january` being
   finite, never on `ice`; the planet test compares cells that are lakes in both and reports
   the fallbacks.
3. **New fields default to NaN** in test literals, so fallback tests do not pass for the wrong
   reason. → Done (test helpers set NaN explicitly where the fallback is tested).
4. **Hard-coded sizes:** `size_of::<CellInfo>() == 140`, "140 bytes" docs, "55 bytes per cell",
   `VERSION = 11`, the codec round-trip test. → All updated.
5. **Quantisation** (1/254 steps) → the test allows half a step.
6. Months 0 and 6 are right. 7. No other mode draws lake ice (topographic and moisture use the
   model's annual `ice` already).
8. **January/July lake ice is seasonal, not permanent** → shown in the "Surface" row ("lake,
   35 m deep, ice in Jan"), not under "Permanent ice".
9. **Coarse-grid artefact:** all lakes of one coarse cell of the temperature model (~440 km,
   16 fine cells at the default 6 subdivisions) share one lake column with their mean depth, so a deep and a shallow lake close
   together freeze alike, unlike the old depth-dependent rule. → Explained in the results
   (accepted grid effect, #64).
