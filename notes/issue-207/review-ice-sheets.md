# Review: ice sheets from the snow pack, merge 97a9268 (#226)

This review covers `git diff 97a9268^1 97a9268 -- src`, mostly `src/temperature.rs`, compared against main
(`97a9268^2`). I read the code only. I did not build it or run anything.

**Verdict: correct. Nothing must be fixed.** A few should-fix and minor items follow.

## What I checked and found correct

- **Albedo path.** With the snow pack on, `land_ice` stays 0, so `land_albedo` is the snow-free value
  `open = land_albedo + land_change`. Snow adds its brightening on top of that
  (`albedo = land_albedo + snow_brightening`). The old `(1 − ice)` factor has been removed from
  `snow_brightening`, so the ice is not counted twice. The cap `clamp(0, ice_albedo − open)` keeps every
  fine cell, and so every coarse mean, at or below `ice_albedo`. Defaults: 0.5 (0.8 − 0.17) = 0.315 is
  above the cap of about 0.30, so fresh snow on modern land reaches exactly 0.62. On bare land the
  brightening is 0.25 and the cap is 0.25 − `land_change`.
- **`fields.ice` on land.** It is `perennial_snow` when both `snow_on` and `perennial_snow_ice` are on.
  Otherwise it is `ice_sheet` (the old rule), which matches the decision. Sea ice and lake ice are
  unchanged.
- **Snow off is bit-identical to main.**
  - `land_ice` uses the same expression as main, in the same order.
  - `land_albedo` is the same `open + (ice_albedo − open)·ice`.
  - `snow_brightening` and `snow_latent` stay zero, and adding `+ 0.0` changes no value.
  - `ice_sheet[fine]` is main's land expression.
  - The new test only shows that the snow parameters have no effect when snow is off. It does not compare
    against main (see minor item 4).
- **#204 cloud correction.** `land_change` is applied once, in `open`. The snow term uses
  `aerosol.land_surface_albedo`, the same surface that `land_change` is computed for, and the old
  `land_surface_albedo` parameter is gone, so there is one source for it. `lake_albedo` and the sea-ice
  `WaterColumn::albedo` do not touch the snow. `reflected` and `planetary_albedo` include the snow.
- **Water budget.** The budget closes: start + snowfall − melt − overflow − dropped = end (there is a
  test). Overflow is counted as glacier runoff. The ice sheets carry no mass of their own: the stored
  snow is capped at `max_snow`.
- **Free-air shift.** It still uses `previous_annual_land` and `previous_free_air_excess` from the year
  before, as before the merge. `snow_offset` comes from the same `shift`.
- **Edge cases of the perennial rule.**
  - First update, or no precipitation: snow is 0 at the start and 0 at the end, so 0 ≥ 0 passes, but
    `least_cover` = 0, so `perennial` = 0. This is harmless.
  - A cell held at the cap: snow ends at `max_snow` exactly as it started, so the equality test makes it
    perennial, as intended.
  - A cell that becomes ocean or lake: its snow goes into `dropped` and it leaves `land_cells_list`.
  - `least_cover` and `year_start_snow` cover only the last year, which is consistent when
    `years_per_update > 1`.

## Must-fix

None.

## Should-fix

1. **The cloud correction is not faded under partial snow.** The old ice path,
   `open + (ice_albedo − open)·ice`, removed `land_change` in proportion to the ice cover. The snow path
   gives `min(land_albedo + land_change + change, ice_albedo)`, so `land_change` stays in full under
   aged or forest-masked snow until the cap is reached. On the abiotic preset (fewer continental nuclei,
   so `land_change` < 0) snowy land comes out darker by |`land_change`| than the old ice path would
   give.
   - The effect is small: it is at most |`land_change`|, and clouds over a bright surface matter less in
     any case.
   - It would be more consistent to scale the term per cell by `(1 − cover · change / (ice_albedo − open))`,
     or by `(1 − cover)` alone, for example `open_cell = land_albedo + land_change · (1 − cover)` inside
     `snow_brightening`.
   - If you keep it as it is, note it in `notes/issue-207/results.md`.
2. **Stale docs.**
   - `update()` doc: "It brightens the ice-free part of the land". There is no ice-free part any more.
   - Comment above `let ice =` (about line 1633): "ice sheets by temperature and snowfall, and snow that
     lasts all year". It should say: perennial snow with the snow pack, the old rule otherwise.
   - `update()` doc: "`precipitation` limits where ice sheets can grow". With snow on, it sets the
     snowfall.

## Minor

1. In the `land_ice` loop, `t` is computed for every land cell even when snow is on, where only the
   `!snow_on` branch uses it. Moving it into that branch saves one `interpolate` per cell per year. The
   result is unchanged.
2. Melting perennial snow is darker than the old ice sheet: aged snow (0.6) gives 0.17 + 0.5·0.43 ≈ 0.385
   added brightening, so about 0.53 instead of 0.62 in summer. This is physically defensible, since
   ablation zones are dark, but it is a calibration shift relative to the old rule. It should be
   mentioned when the results are compared. The snow-free surface reference also moved from 0.20 to
   0.17, which brightens every snowy cell by +0.015 before the cap.
3. A cell that turns from ocean into land inherits a stale `warmest_season` (40 °C by default, or
   whatever it had when it was last land). Its forest share is wrong for one year. This is harmless.
4. Missing tests:
   - Snowy land never exceeds `ice_albedo`, including with `land_change` ≠ 0.
   - With snow off, `fields.ice` equals the old rule (a value check, not only parameter independence).
5. `planet.rs` (about line 848) has an orphan comment, "Ice sheets depend on last step's snowfall", above
   unrelated code. It comes from main and is not part of this merge.
