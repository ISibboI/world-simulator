# #100 Sediment thickness render mode — results

## What changed

- **Render mode `sediment`** (switcher label "Sediment"; `--render-mode sediment`,
  `?render-mode=sediment`), `src/viewer/modes/sediment.rs`:
  - Sediment thickness on land and sea floor on ColorBrewer's YlOrBr (sequential, colour-blind
    safe, lightness monotonic), logarithmic from 10 m (palest, also for bare rock) to 20 km
    (darkest); ticks 0.01, 0.1, 0.3, 1, 3, 10, 20 km. Unlit, so the colours match the legend.
  - Option **Thickness: Bulk | As rock**. Bulk (default) is the compacted column with its pores,
    what seismic isopach maps of Earth show; "as rock" is what the simulation stores (mass at
    2050 kg/m³). Bulk follows from it by the simulation's own compaction law
    (`isostasy::sediment_bulk_thickness`): ~1.5–1.9× the rock equivalent for thin, porous
    sediment, equal at ~5 km, less below (a compacted column is denser than 2050 kg/m³).
  - Option **Isopachs**: lines at 0.1, 1 and 10 km of the shown thickness (wider for thicker),
    drawn on the log axis so that they follow the colour steps.
  - Coastlines in blue with a thin white border (stand out on the yellows and the dark browns
    along coasts alike).
  - Legend with description, scale, coastline and isopach swatches.
- **Cell widget**: the Sediment row shows both, e.g. `774 m (512 m as rock)`, in km from 1 km
  (e.g. `3.4 km (3.1 km as rock)`), short enough for phones.
- **No frame growth**: every frame of the history already keeps the rock-equivalent sediment at
  1 m, so past frames show alike; no new channel, no flag bits. No simulation change.
- **Not done: deposition/erosion rates.** They are neither in the frame nor exported; a
  difference between frames at a cell mixes deposition with the advection of crust by the
  plates. Would need a new exported field and a frame channel (2 bytes/cell).

Tests: scale monotonic and clamped (0 m → palest, not missing), bulk/rock relation, options
→ legend swatches, a planet coloured and outlined per option (isopachs add lines), widget row.
fmt, clippy (native and wasm32), tests and doc pass with warnings denied.

## Screenshots

- Desktop, bulk: [desktop-bulk.png](desktop-bulk.png); with isopachs:
  [desktop-isopachs.png](desktop-isopachs.png) (5 subdivisions, seed 0, ~200 Myr).
- Web at phone width (400 px): [web-phone.png](web-phone.png) (panels collapsed),
  [web-phone-legend-cell.png](web-phone-legend-cell.png) (legend with options, cell widget). The
  option buttons and the checkbox are the switcher's existing widgets, touch-sized on coarse
  pointers.

## Plausibility (4 seeds, 500 Myr, 6 subdivisions, main at 72d612b)

Bulk thickness from `--cell-dump` (`bulk` column, the same function the mode uses); medians
in m; "bare" = < 10 m.

| Setting | seed 0 | seed 1 | seed 2 | seed 3 | Earth (orientation) |
| --- | --- | --- | --- | --- | --- |
| Ocean floor, mean | 712 | 691 | 324 | 366 | ~500 (bulk) |
| Ocean floor < 10 Myr: bare share | 81% | 89% | 94% | 84% | near zero at ridges |
| Ocean floor 10–100 Myr: mean | 534 | 363 | 203 | 129 | 100s of m, rising with age |
| Ocean floor > 100 Myr: median / p90 | 1094 / 9174 | 544 / 10211 | 30 / 8754 | 17 / 8568 | ~0.5–1 km pelagic, km-thick only at margins and fans |
| Ocean floor ≥ 200 Myr: area / mean | 2.0% / 7.5 km | 3.9% / 6.9 km | 1.6% / 5.8 km | 1.0% / 8.8 km | none older than ~180 Myr |
| Shelf (submerged continental, > −200 m): median / p90 | 1211 / 5907 | 2387 / 8219 | 3493 / 8858 | 2281 / 7249 | passive margins up to 10–15 km |
| Land: bare share / p90 | 52% / 3140 | 46% / 3711 | 43% / 4430 | 55% / 5143 | shields bare, platforms 1–5 km, basins more |
| Cells at the 11 km cap | 277 | 368 | 356 | 343 | — |

Reading:
- Plausible for an Earth-like planet: bare young ocean floor at the ridges, a few hundred metres
  on mid-aged ocean floor, km-thick shelves and margins, about half the land bare (shields,
  mountains) with basins of several km.
- **Maximum 11 km bulk everywhere** (300–370 cells per seed): sediment buried deeper than
  `metamorphic_depth` (11 km bulk) turns into felsic crust, so the thickest piles saturate
  there; Earth's thickest basins (Bengal, Gulf of Mexico, South Caspian) reach 15–20 km. The
  scale reaches 20 km so that the cap shows as dark brown rather than as the end of the bar.
- **Thick old ocean floor**: 11–16% of the ocean floor older than 100 Myr carries ≥ 8 km, and
  the ≥ 200 Myr ocean (1–4% of the area) holds 24–43% of all sediment on oceanic crust at
  ~6–9 km mean — sediment trapped in old, unsubducted basins, the known weakness of #14
  (commented there).
- Grid: at ~110 km cells (220 km on the web) narrow features (deltas, fans, accretionary
  wedges, shelves of tens of km) are averaged over a cell, so their peaks are lower and their
  outlines blockier than on Earth; accepted per the project goal.
