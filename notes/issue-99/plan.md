# #99 Crust type and thickness render mode — plan

Issue: https://github.com/ISibboI/world-simulator/issues/99 (crust type — oceanic, continental,
arc, metamorphosed sediment — and thickness; highlight orogens > 50 km and thinned
margins/rifts; legend).

## Data (frame history)

The frame (`CellInfo`) already has `crust_thickness` (10 m steps), the `CONTINENTAL` flag, the
orogeny fields. Added, keeping frame growth at zero bytes:

- `CellInfo::OCEANIC` (bit 6), copied verbatim from open PR #140 (#98 crust age) so that both
  modes and the cell widget use the same oceanic / transitional / continental classes
  (`Crust::is_oceanic`, `Crust::is_continental`), and the two branches merge cleanly.
- `CellInfo::FELSIC` (bit 7, the last bit the 8-bit flags channel of the history keeps): the
  basement (`Crust::basement`, metasediment removed) is > 50 % felsic. Continental crust and
  continental arcs are felsic 1.0, intra-oceanic arc additions 0.4 on 0.0 oceanic crust, so
  this splits "arc/plateau on oceanic crust" from "continental crust" reliably.
- Metasediment: the simulation tracks a metasediment mass fraction. Shown only if it is
  common enough to matter (checked in a 4-seed run, see results); it would need one more flag
  bit, i.e. the flags channel widened from 1 to 2 bytes (+1 B/cell raw, ~0 after delta+lz4).

No simulation change; the simulation results are unchanged (only exported flags).

## Crust types (`cell_info::CrustType`, a refinement of the simulation's 3 classes)

| type | rule | colour (Okabe–Ito) |
|---|---|---|
| Oceanic | `is_oceanic` | blue |
| Arc, plateau (transitional) | neither, mafic basement | bluish green |
| Thinned continental (transitional) | neither, felsic basement | yellow |
| Mafic arc (continental) | continental, mafic basement | reddish purple |
| Continental | continental, felsic, ≤ 50 km | orange |
| Thick continental (> 50 km) | continental, felsic, > 50 km | vermillion |

Felsic family yellow → orange → vermillion follows thickness (thin margin → normal → orogen),
which covers "highlight thick orogens and thinned margins/rifts".

## Mode `crust` ("Crust")

Options: **Show** Type / Thickness / Both; **Isolines** Off / 10 km / 20 km; **Coastlines**
(on). Unlit.
- Type: categorical colours above; legend lists the types present in the shown frame.
- Thickness: viridis 0–80 km (perceptual, monotone lightness, CVD-safe), ticks every 10 km.
- Both: the type colour mixed with white (thinner) or black (thicker), on a log scale
  5–80 km (so oceanic 5–12 km variations show), at most 60 %; legend bar shows the shades of
  a grey.
- Isolines of the thickness via `overlay::isolines`, coastlines via `modes::coastlines`.
- Cell widget: the "Crust" row shows the refined type ("transitional (thinned continental),
  18.3 km").

## Checks

Unit tests (classification, scales monotone, options/legend, a planet in the mode), CI-clean
native + wasm32, desktop screenshots in vscreen, web version at desktop and phone width.
Plausibility of thickness/type shares for 4 seeds at 500 Myr from `--cell-dump`.
