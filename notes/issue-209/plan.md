# #209: global statistics in the viewer — plan

Stacked on #217 (`agent/issue-213`), which added `GlobalStatus` (the ramp-up phase) to the
parameter panel. This grows that record into the planet's global statistics, carried with every
state and every frame of the history, so that the panel follows the time bar.

## What is shown (only values the model has now)

| Group | Row | Source |
| --- | --- | --- |
| Time | epoch (Gyr) and time before present (Ga) | view: `--epoch` + shown time (`sun::age_of_run`) |
| | Sun: solar constant of the climate (W/m²), % of today's luminosity | record: `TemperatureFields::solar_constant`; view: `Sun::of_run` (as the Realistic legend) |
| | day length 24 h, labelled "fixed" | constant (`EARTH_ROTATION` in climate/ocean/temperature) |
| | ramp-up phase (#217's lines) | record (end time, settled, CO₂ held) |
| Climate | mean surface temperature, equator (<10°), poles (>70°) | record: annual means over cells, like `PlanetStatistics` |
| | CO₂ (× pre-industrial 280 ppm and ppm), greenhouse forcing (W/m²) | record: `CarbonState::ratio`, `forcing` |
| | ice cover (share of surface with ice > 0.5) | record, like `frozen_fraction` |
| | mean land precipitation (m/yr) | record: mean over non-ocean cells |
| Surface | land fraction, continental-crust fraction | record: cells (`is_ocean`, `is_continental`) |
| | sea level change since the start (m) | record: tectonics `sea_level − initial_sea_level` (new read-only accessor) |
| | lakes: area (10⁶ km²), volume (10³ km³) | record: `Hydrology::lake_cells × cell area`, `lake_volume` |
| Tectonics | plates (all owning cells, and > 1 % of the surface), mean surface speed (cm/yr) | record: from cells and `PlateInfo` rotations |

Not modelled yet, left out (pending #208): radiogenic heat production, mantle temperature /
heat flow, a changing day length. Continental fraction is shown as simulated state.

## Data path

- `GlobalStatus` moves to `src/viewer/global.rs` and grows to the fields above (f32 values,
  plate counts u16, a flag byte). `GlobalStatus::of(planet, cells, plates)` is computed in the
  simulation thread/worker where `cell_infos`/`plate_infos` are already built (one pass over
  the cells, read-only: no effect on the simulation or its state hash).
- Fixed binary form `to_bytes`/`from_bytes` (~65 bytes, little endian, NaN allowed).
  - Desktop: `SurfaceUpdate::global` as now.
  - Web: the worker message carries it as one `global` byte array (replacing #217's three keys).
  - History: the codec puts the record **uncompressed in the frame header** (version bump to
    14, after main's 13). `codec::header` returns it, so `History::record` keeps it in the
    frame's `Entry` without decoding: the panel and sparklines can read any frame's statistics
    at once. Frame growth: +65 bytes per frame (frames are tens of kB).
- Viewer: `Latest` and `show()` carry the record; `poll_history` takes it from the history
  entry. `shown_global` is what the panel shows, so the values follow the time bar.

## UI

- In the parameter panel under the time line (where #217's ramp-up lines are): a
  `CollapsingHeader` "Planet" with grouped `Grid`s (label, value with unit), collapsed by
  default on narrow screens (< 600 pt, like the panel itself), open on wide ones. Shared egui
  code; touch sizes come from `UiOptions` (header row gets the touch interact size).
- Sparklines (cheap: painter polyline over the in-memory history entries, ≤ ~200 points after
  decimation, a marker at the shown time) for mean temperature, CO₂ and sea level, ~100×18 pt
  next to/below the value. Dropped if they do not read well on the phone screenshot.
- No new kittest tests (per instructions). Unit tests: record bytes round trip, codec header
  round trip carries the record, `GlobalStatus::of` on a tiny planet gives fractions in 0..1.

## Verification

- State hash: a short `--headless` run on this branch and on the base, same seed, compare the
  logged state hashes.
- Local CI with `CARGO_BUILD_WARNINGS=deny`: fmt, clippy, test, doc, wasm check/clippy (incl.
  tests); `trunk build --release`.
- Screenshots (vscreen): desktop 1280×800, web 390×844; in `notes/issue-209/`.


After the review (see `plan-review.md`) the record also carries the escape's water loss and O₂ (#212, after the base was merged with main): 73 bytes. The results are in `results.md`.
