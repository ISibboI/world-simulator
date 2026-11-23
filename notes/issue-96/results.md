# #96 Ocean-current render mode: results

Stacked on #131 (#95). Plan and its review: [plan.md](plan.md).

## What the mode shows

**Render mode `ocean-currents`** (switcher label "Ocean currents"):

- **Arrows**:
  - Placement: one at each cell of the climate model's coarse mesh (icosphere 4, about 440 km apart), the resolution of the gyre model.
  - What they show: the depth-integrated volume transport.
  - Length: grows with the square root of the transport; full length at 1 Sv per 100 km of width (10 m²/s).
- **Arrow colour** (switch "Warm / cold", on by default):
  - Red where the gyres warm the mixed layer by more than 10 W/m² (annual mean), blue where they cool it by as much, white otherwise.
  - This is the model's own gyre heat convergence (`ρ c_p γ Q T` upstream, i.e. `−u·∇T`), not a temperature proxy. It is the "warm current / cold current" of the model: water that comes from warmer sea than it flows on to.
- **Background** (choice):
  - Transport: ColorBrewer Purples, logarithmic, 0.02–2 Sv per 100 km.
  - Gyres: the streamfunction ψ in PuOr, ±15 Sv; orange is clockwise, purple anticlockwise.
  - Sea temp.: the annual mean, on the temperature mode's scale.
  - Land is flat beige; sea that the gyre model leaves out is grey (see below). Coastlines are drawn as lines; the view is unlit.
- **Streamlines** (switch, off by default): isolines of ψ at a round interval (1/2/5 × 10ⁿ Sv, about 6 per sign). The same transport flows between every two of them.
- **Legend**: a description, including what is not modelled, the colour scale, swatches, and an arrow scale in Sv per 100 km.
- **Cell widget**: a new row "Ocean current", e.g. "0.12 Sv/100 km to E, −2 W/m²".

## What the model has, and what it does not

The model in `src/ocean.rs` is Stommel's vorticity balance solved for ψ on the coarse mesh. It has a Sverdrup interior and western boundary currents one coarse cell wide. It is driven by a fixed analytic annual-mean wind-stress profile (`climate::surface_wind_stress`).

- **Depth-integrated, no depth structure.** There are no speeds, only transport per width. As a guide, 1 Sv per 100 km is 1 cm/s in a 1 km deep layer.
- **Not modelled:**
  - Deep and thermohaline circulation.
  - Upwelling and Ekman pumping. Real eastern-boundary currents are cold partly because of upwelling.
  - Seasons.
  - Flow through straits and around islands: every coast is a wall with ψ = 0, so there is no net transport between land masses.
  - Circumpolar currents: in a zonally unbounded band only the friction sets the flow.
- **The overturning cells under the trade winds** carry heat in the model only as a zonal mean. They are not shown.
- **Coarse grid (#64).** Currents and heating are drawn per coarse cell, so the background shows ~440 km hexagons. Fine sea cells whose coarse cell is mostly land are not part of the gyre model: they are grey ("Sea without currents in the model") and form a thin fringe along many coasts.

## Magnitudes (default settings, 6 subdivisions, seeds 0–2, start of the run)

| | seed 0 | seed 1 | seed 2 |
|---|---|---|---|
| largest \|ψ\| (Sv) | 8.6 | 11.3 | 14.2 |
| transport p50 / p99 / max (Sv per 100 km) | 0.22 / 1.09 / 1.31 | 0.20 / 0.89 / 1.61 | 0.22 / 1.02 / 1.64 |
| gyre heating p1 / p99 (W/m²) | −31 / +71 | −42 / +78 | −36 / +54 |

The gyres are **weak**: Earth's subtropical gyres carry about 30 Sv, and 100–150 Sv with their recirculations. Follow-up: #134 (part of #132).

The patterns themselves are plausible (see the screenshots):
- clockwise subtropical gyres in the north, anticlockwise subpolar ones;
- western intensification, with poleward boundary currents that warm the sea and cool equatorward flow in the east;
- westward flow in the trade-wind belt.

## Data and frame size

- **`TemperatureFields`** gains three fields on the fine mesh:
  - `ocean_current`: east and north, m²/s, the value of the coarse cell;
  - `gyre_streamfunction`: Sv, interpolated;
  - `gyre_heating`: annual mean, W/m², the value of the coarse cell.

  They are NaN where the gyre model has no ocean. They are not part of the state hash; the simulation is unchanged.
- **Transport per coarse cell:** least squares of `U·(r̂ × e) = q` over the cell's Voronoi edges `e`, with edges to land as walls (`q = 0`). It is exact for a linear ψ (test `transport_of_a_zonal_flow`).
- **`CellInfo`**: +4 f32, 68 → 84 B per cell on the wire (+24 %).
- **Frame codec**: 4 `Signed` channels, VERSION 4:
  - currents 0.05 m²/s, ψ 0.05 Sv, heating 1 W/m²;
  - 31 → 39 B per cell raw.
  The first try with 10× finer steps cost +29 % per frame for no visible gain.

`frame_sizes` test, 5 subdivisions, 100 steps, second half:

| | before | after | change |
|---|---|---|---|
| quantised frame (raw) | 318 kB | 400 kB | +26 % |
| keyframe (lz4) | 177 kB | 225 kB | +27 % |
| delta frame | 89.9 kB | 106.6 kB | +19 % |
| **stored per frame** (1 key in 25) | **93.4 kB** | **111.3 kB** | **+19 %** |

Per-channel deltas: current east/north 4.5 / 4.2 kB, ψ 5.0 kB, heating 4.6 kB per frame. The gyres change with every climate update as the land moves.

## Checks

- `CARGO_BUILD_WARNINGS=deny`:
  - fmt;
  - clippy (all targets, native and wasm32);
  - doc with `-D warnings`;
  - all tests: 353 pass, including new tests for the transport reconstruction, the mode (colours, arrows, warm/cold, streamlines, legend on a planet), the cell-widget row and the codec round trip.
- **Desktop** (vscreen):
  - [transport on a past frame](desktop-past-frame-transport.png);
  - [gyres with streamlines](desktop-gyres-streamlines.png);
  - [sea temperature](desktop-sea-temperature.png);
  - [cell widget row](desktop-cell-row.png).
- **Web at phone width** (390 px), trunk release build:
  - [planet](web-phone.png): panels collapsed;
  - [legend](web-phone-legend.png): scrolls; touch-sized buttons from the shared UI.
  - At phone size the arrows are small until one zooms in.
