# #95 Moisture render mode: results

Plan: [plan.md](plan.md) (with the review's changes below). Stacked on #130 (temperature mode,
mode options, isolines, coastlines).

## What changed

- New render mode **Moisture** (`--render-mode moisture`, `?render-mode=moisture`;
  `src/viewer/modes/moisture.rs`), unlit with coastlines, options:
  - **Show**: *Precipitation* / *Aridity*.
  - **Season**: *Year* / *January* / *July* (precipitation; the aridity index is always annual).
  - **Desert edge**: a red line (white border) on ice-free land where the aridity index is 0.2.
- Precipitation colours: ColorBrewer **YlGnBu** (sequential, colour-blind safe, monotone
  lightness) on a **logarithmic** axis, 30 … 5000 mm/yr (clamped), ticks 50 … 5000. One scale
  for all seasons, land and ocean alike.
- Aridity: UNEP **aridity index AI = P / PET** of land, ColorBrewer **BrBG** (diverging,
  colour-blind safe) on a log axis, near-white at AI = 1 (P = PET), brown dry, teal wet,
  0.02 … 10, ticks at the UNEP class limits (0.05, 0.2, 0.5) and 1, 3, 10. Ocean blue-grey,
  ice sheets (ice ≥ 50 %) lilac, land without PET dark grey ("No data").
- Cell widget: new rows *Precip. Jan / Jul* and *Aridity index* (value and UNEP class, land
  without ice sheet).
- Data: `CellInfo` gains `precipitation_january`, `precipitation_july` (from the water cycle's
  solved months; slot = month × months / 12, NaN if the month is not solved, e.g. no July with
  `climate.months` 1 or 3) and `potential_evaporation` (annual, land; 0 over the ocean). Frame
  codec: three more 1 mm/yr channels, version 2 → 3.

## Size impact (`frame_sizes` test, 5 subdivisions, 10 242 cells, 100 steps)

| | before | after | change |
|---|---|---|---|
| `CellInfo` per update (wire, web worker → page) | 56 B/cell, 574 kB | 68 B/cell, 696 kB | +21 % |
| quantised frame (raw) | 25 B/cell, 256 kB | 31 B/cell, 318 kB | +24 % |
| keyframe (lz4), second half | 136 kB | 177 kB | +30 % |
| delta frame, second half | 79.7 kB | 91.7 kB | +15 % |
| **stored per frame** (1 key in 25) | **82.0 kB** | **95.2 kB** | **+16 %** |

Per-channel deltas: January 3.4 kB, July 3.7 kB (each month is refreshed every 4th update),
PET 5.3 kB per frame. Encode 0.39 → 0.52 ms, decode 0.80 → 1.17 ms per delta frame.

## Simulated ranges (fresh planet, 5 subdivisions; `moisture_distribution` test)

| | 1 % | 5 % | 50 % | 95 % | 99 % | max |
|---|---|---|---|---|---|---|
| P annual, land (mm/yr) | 24 | 49 | 451 | 1767 | 4049 | 7101 |
| P January, land | 10 | 27 | 397 | 2317 | 4197 | 9584 |
| P annual, ocean | 63 | 199 | 896 | 1959 | 2343 | 3520 |
| PET, land (mm/yr) | 28 | 34 | 449 | 1494 | 2583 | 5948 |
| AI, ice-free land | 0.033 | 0.078 | 0.91 | 3.2 | 4.6 | 10.9 |

UNEP classes of ice-free land: hyper-arid 2 %, arid 12 %, semi-arid 16 %, dry sub-humid 8 %,
humid 62 % (Earth, by UNEP's PET: roughly 7.5 / 12 / 18 / 10 / 53 %) — plausible for an
Earth-like planet. The scales cover 1 %–99 % of both.

## Review of the plan (independent agent) and what was done

- PET lags on changing coastlines (the water cycle refreshes one of its months per update, and
  only land cells have PET): **land with PET = 0 is "No data"** instead of maximally humid;
  newly risen land may show a too-wet index for up to a few updates (one cell wide, a coarse-grid
  effect). Accepted, explained here.
- The model's PET is aerodynamic only (bulk formula at the land temperature, no radiation term),
  unlike the Penman–Monteith PET the UNEP limits were set with, so **class limits are nominal**
  (module docs). The resulting class shares above are nevertheless Earth-like.
- **Ice sheets masked** (their PET ≈ 0 would read as humid).
- Ocean colour darker/bluer than BrBG's near-white middle; desert-edge line computed on ice-free
  land only (ends at coasts and ice).
- Monthly slots mapped to months properly; with no July solved the map shows "No data" (the option
  framework has no disabled options).

## Checks

- Unit tests: scales monotone in lightness (precipitation darker with more rain, aridity darker
  away from 1 on both arms, brown dry/teal wet); aridity index and ground classes of cells;
  options → legend; a planet in the mode (colours match the scales in all three settings,
  overlay has coastlines, the desert line adds pieces); codec round trip of the new channels;
  cell widget rows.
- fmt, clippy (native and wasm32, all targets/features), tests, doc with warnings denied.
- GUI (vscreen): desktop, 5 subdivisions — [annual precipitation](desktop-annual.png),
  [aridity with desert edge](desktop-aridity-desert-edge.png), [a past frame (20 of 30 Myr)
  with the cell widget's new rows](desktop-past-frame-aridity.png); web at phone width (390 px,
  4 subdivisions) — [July precipitation](web-phone-july.png), [aridity](web-phone-aridity.png).
  Screenshots predate two label changes ("Precipitation of" → "Season", which no longer wraps
  on phones; "Desert edge (aridity 0.2)" → "Desert edge (0.2)").

## Observations

- In the screenshots the deserts lie on the western, lee side of the large continent in the
  subtropics, its east side and the convergence zone are wet, and the wet tropical band moves
  between January and July with the ITCZ.
- Polar land counts as humid by this index (little PET), as in UNEP's definition; ice sheets are
  shown separately.

## Decisions for Sebastian

- Size: +16 % stored per frame, +21 % per update on the wire, for seasonal precipitation and PET.
  Dropping January/July would save ~7 kB/frame of the 13 kB.
- Not done: P − E (would need evaporation in the frames, +2 B/cell); a month slider (needs all
  solved months: by default only 4 are solved).
