# #96 Ocean-current render mode: plan

Stacked on #131 (#95, moisture), which also changes the frame format.

## What the model has (`src/ocean.rs`, `src/temperature.rs`)

- **Wind-driven gyres** (Stommel 1948 / Sverdrup) on the coarse mesh of the energy-balance model
  (icosphere 4, 2562 cells, ~440 km): a **streamfunction ψ** (m³/s) per coarse cell and the
  **volume flux** across every Voronoi edge between ocean cells (from ψ at the edge's corners).
  Depth-integrated: there is no depth structure, hence no velocity, only transport per unit width
  (m²/s). Steady: the wind stress is a fixed annual-mean zonal profile, so the gyres change only
  when the land (coarse cells with < 50 % land are ocean) changes. Coasts are walls with ψ = 0.
- **Subtropical overturning cells** (poleward Ekman transport under the trades, return in the
  thermocline): only a zonal-mean heat transport across latitude bands, no per-cell flow.
- Not modelled: thermohaline / deep circulation, seasonal currents, flow through straits and
  around islands (every coast has ψ = 0, so no net transport between two land masses), equatorial
  (under)currents beyond what Sverdrup gives.

## Data (simulation → view → frame history)

- `ocean.rs`: `Gyres::transport` — per coarse cell, the transport vector (east, north) in m²/s,
  least-squares fit to the fluxes across its edges (`q_j ≈ L U·n_j`, edges to land are walls with
  q = 0; `L` = Voronoi edge length `d/√3`). Flux computation factored out so a test can feed a
  synthetic ψ (uniform zonal flow) and check the reconstruction.
- `temperature.rs`: `TemperatureFields` gains, on the fine mesh, `ocean_current` (east, north;
  m²/s; interpolated with the existing coarse→fine weights renormalised over the coarse *ocean*
  cells, NaN for land and where no ocean cell is near) and `streamfunction` (Sv, interpolated with
  ψ = 0 at land cells, NaN on land). Not part of the state hash.
- `CellInfo`: `current_east`, `current_north`, `streamfunction` (f32) — 68 → 80 B per cell on the
  wire. Frame codec: three `Signed` channels (0.01 m²/s → ±327 m²/s; 0.01 Sv → ±327 Sv), 31 → 37
  B/cell raw, frame VERSION 4. Measured with the `frame_sizes` test before/after.

## View: render mode `ocean-currents`

- Arrows at the fine cells nearest the vertices of the coarse mesh (icosphere 4: the model's own
  resolution, as the issue asks), ocean cells only (current elevation < 0 and a finite value).
  Length ∝ sqrt(transport), fixed reference (so frames compare), legend arrow scale in
  "Sv per 100 km" (= 10 m²/s) with a note how that converts to a speed for a given layer depth.
- Arrow colour (switch "Warm / cold", default on): along-flow sea-surface temperature difference
  (annual mean SST of the fine cells one coarse spacing upstream and downstream): water from
  ≥ 1 K/1000 km warmer → warm current (red), colder → cold current (blue), else neutral. This is
  the sign of `−u·∇T`, the heat convergence the model's gyres deliver.
- Background (choice): transport magnitude (sequential), gyres (ψ, diverging: clockwise /
  anticlockwise), or annual SST (the temperature mode's scale). Land flat grey, unlit; coastlines.
- Streamlines (switch): isolines of ψ at a "nice" interval (1/2/5 × 10ⁿ Sv, ~6 per sign).
- Cell widget: row "Ocean current" ("2.3 Sv/100 km to NE").
- Everything comes from `CellInfo`, so past frames work alike. No new `Layer`.
- Unit tests: transport reconstruction, interpolation masks, codec round trip, mode
  colours/arrows/legend on a small planet.

## Checks

fmt, clippy native + wasm32, tests, doc with warnings denied; desktop and web (phone width)
screenshots via vscreen; frame size before/after.
