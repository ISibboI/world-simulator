# Code review: #173 lakes as open water (`e7d2a14..81a349d`, `src/`)

Independent review of the diff. Read-only; tests run: see the end.
Line numbers are in `81a349d`.

## Verdict

The change is correct in its core: the three-surface exchange conserves energy, the ocean
column's step is unchanged, and the switch-off path is bit-identical by construction. The
remaining issues are model consistency (lake thresholds, enthalpy at capacity changes) and stale
docs. Nothing blocks the PR; S1 and S2 are worth fixing or stating in the PR.

## Bit-identity with `open_water_lakes=0`, `min_sea_fraction=0.002` (checked)

- `planet.rs:477-489`: `lakes = None`, so `climate_elevations(…, None)` and
  `update(…, None, …)` behave as before; `is_lake` is all false (`temperature.rs:643`).
- `ocean_fraction = 1 − f − 0.0` is exactly `1 − f`. Every new term is `0.0 · finite`
  (`lake_fraction`, `to_lake`). `lake_surface` stays 15 °C and is never NaN, so the added terms
  are ±0 and leave every sum unchanged. Heat capacity, `mean`, band SST, gyre `ocean_area`,
  subduction and free-air weights all reduce to the old expressions.
- `WaterColumn::{albedo, step}` repeat the old ocean code in the same operation order:
  `(-H/L).max(0)` is the old `ice_thickness`, `melting = 0.0` replaces the literal `0.0`, and
  `self.freezing` is `params.freezing_temperature`.
- Climate: `monthly_lake_ice` is empty, so `.get()` returns `None` and the code falls back to
  `ice_free`. `lake_season`'s mean sums the same terms in the same order.
- `sea.rs:173`: `>` → `>=` only matters when `min_cells == cell_count`, i.e. fraction 1.0 (the
  new default). That early return gives the same empty result the loop would.
- The lake columns still integrate (virtual) with the switch on, but they cannot feed back while
  `lake_fraction == 0`. The test `without_lakes_the_lake_columns_change_nothing` covers this.

Energy: each cross term appears as `k·f_i·f_j·(T_j − T_i)` with opposite signs in the two
columns, so exchange conserves energy per cell up to the existing Gauss–Seidel lag (land, then
ocean, then lake). Water: the temperature model does not move water; the climate change only
reweights the season of already-removed lake evaporation, and the existing test checks that the
total still returns in full. Determinism: no new transcendental calls, no hash maps. NaN:
`monthly_lake_ice` is NaN off lakes by design and only reaches `climate::open_water`, which
handles it. `ice`, `monthly` and `annual_mean` are finite.

## Should

**S1. Lake definition differs from the rest of the code (`temperature.rs:643-648`,
`planet.rs:999-1001`, `planet.rs:689-691`).**
- The temperature model treats every cell with `lake_depth > 0` as a whole lake cell.
- The view, Köppen and `render::Water::of` count a lake only at `lake_depth >= LAKE_DEPTH`
  (1 m, `render.rs:40`).
- The partly covered last cell of every lake (plan-review S2, not addressed) also counts fully.
- So cells with 0 < depth < 1 m get the lake column's temperature and lake ice. Everything else
  shows them as land:
  - Köppen (`koppen.rs:386`) can call such a cell EF from its *lake* ice when the annual cover is
    ≥ `ICE_SHEET`.
  - The cell panel shows lake ice on "land".
  - Small lakes inflate `lake_fraction` by up to one cell.

Fix: use `crate::render::LAKE_DEPTH` (or a shared constant) in `is_lake` and in
`climate_elevations`, or else document the discrepancy. A cover fraction is the cleaner
long-term fix.

**S2. The lake enthalpy is kept when the column's heat capacity or freezing point changes
(`temperature.rs:586, 688-701`).** `T = freezing + H/C`, so changing `C` rescales the
temperature excess by `C_old/C_new`. Plan-review C3 calls this harmless, but the worst case is
larger than it says:
- A coarse cell's first lake turns the virtual column (`C` = ocean, 50 m, ~15 °C, `H ≈ 3e9 J/m²`)
  into a ~2.4 m column: `T − freezing ≈ 15·21 ≈ 300 K`.
- A first lake on a plateau also raises `freezing` by `Γ·z̄` (+13 K at 2 km).
- With damping ≈ 12–15 W/m²/K, `τ = C/D` is about 10 days for shallow lakes. The spike therefore
  dies within the first year of the 2-year update and does not reach the reported last-year
  fields directly.
- It does dump the virtual column's heat into the land and ocean through `to_lake`. The first
  year's `previous_annual_land` feeds next year's ice-sheet albedo and free-air excess.
- It recurs whenever a cell's mean lake depth changes; small changes give small jumps.

Fix: keep last update's `lake_heat_capacity` and `lake_freezing` per coarse cell and, for open
water (`H > 0`), convert `H_new = C_new·(freezing_old + H/C_old − freezing_new)` at the start of
`update`. Alternatively reset a column to the cell's mean air temperature when its
`lake_cells` goes from 0 to > 0. The fix is cheap and makes the column's state meaningful.

## Could

- **C1 `temperature.rs:1460` (`WaterColumn::step`).** Lake ice is capped at
  `max_ice_thickness` = 10 m, also on lakes shallower than that. The cap is an energy source
  (documented in the comment). Capping at `max(lake depth, …)`, or at a conduction-limited
  thickness, would be more physical; 10 m of ice needs ~3e9 J/m² to melt, which can make
  high-latitude lakes perennially frozen. Worth a sentence in the PR.
- **C2 `temperature.rs:1166-1170`.** A fine lake cell takes the interpolated sea-level lake
  temperature minus `Γ·z_fine`, while the column freezes at `Γ·z̄` of its own coarse cell.
  A lake higher than z̄, or a neighbour with a different z̄, can show open water below 0 °C or
  ice above it (plan-review S3, not clamped). This is fine for a 110 km grid, but state the
  choice.
- **C3 `temperature.rs:875`.** The comment "the area mean of its land and ocean surface" should
  mention lakes. The substep loop's `f` is now land only; fine.
- **C4 `viewer/modes/realistic.rs:437-439`.** The comment "not from the climate's `ice`, which
  is sea ice below sea level and the snowfall-limited ice sheet above it" is stale: `ice` on a
  lake is now the model's lake ice. The viewer still uses its own temperature heuristic
  (`lake_ice`) instead of the model's lake ice. That is the follow-up from plan-review S5;
  mention it in the PR.
- **C5 `viewer/modes/realistic.rs:665`.** With the switch off, lakes below sea level no longer
  take their neighbours' air. They now show the land-at-0-m temperature of `climate_elevations`.
  This is view only, not simulation, and acceptable.
- **C6 `planet.rs:773`, `presets.rs:431`, `planet.rs:843`.** Land statistics (precipitation,
  aridity) and `frozen_fraction` include lake cells, which now carry water temperatures and lake
  ice (plan-review S8). Compare runs with that in mind.
- **C7 `planet.rs:741`.** `lake_climate_contrast` reports 0.0 for empty sets; `main.rs` prints
  the cell count beside it, so this is readable. The rings exclude lakes and ocean (good).
  Cloning the whole `TemperatureModel` twice per snapshot is fine for a diagnostic flag.
- **C8 `planet.rs:999-1001`.** The water surface is `elevation_now + load_correction_old +
  depth_old`. After a tectonic step that is the old level plus the cell's elevation change,
  plus the sample-surface offset: tens of metres at most, < 0.5 K. OK.

## Tests

- Good:
  - `without_lakes_the_lake_columns_change_nothing` checks exact equality for on-without-lakes,
    for off-with-lakes, and against the old call.
  - The maritime-range test has a deep-versus-shallow contrast.
  - The winter-freeze test.
  - The climate test of ice cover versus ramp, with NaN fallback and water returned in full.
- Missing (could):
  - A unit test of `climate_elevations` with lakes: a lake below sea level ends at its water
    surface ≥ 0, and an ocean cell with a stale `lake_depth` is not raised.
  - A per-cell energy-closure test of the three-way exchange.
  - A test of `interpolate_lake` at a lake cell whose coarse neighbours have no lakes.
  - The test assertions use generous margins (e.g. `< 0.6×`); fine for a model test.

Tests run (`cargo test --release lake`, 81a349d): 21 passed, 0 failed (incl. the three new temperature tests and `climate::lake_ice_stops_evaporation`). The viewer test `dry_land_below_sea_level_takes_the_air_of_the_land_around` was not in the filter.
