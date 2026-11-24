# #103 Climate-classification render mode (Köppen–Geiger): results

Plan: [plan.md](plan.md). Builds on #105 (mode registry), #115 (frame history), #118
(category boundaries), #130 (options, coastlines), #131 (moisture data).

## What changed

- `src/koppen.rs` (new, pure): the 30 classes of Beck et al. (2018) with code, common name and
  main group; `classify(T[12], P[12])` with Beck's Table 1 rules (E before B, B before A/C/D;
  C/D at 0 °C; Pthreshold 2·MAT + 0/14/28 by > 70 % of the rain in the winter/summer half;
  summer = the warmer of AMJJAS/ONDJFM, so both hemispheres work; d only in D). When both s and
  w hold (Peel et al. give no rule), the drier half is the dry season (Kottek et al. 2006).
- **Months**: temperatures are the model's 12 monthly means. The water cycle solves only
  `climate.months` months (4 by default: Jan, Apr, Jul, Oct); the others are **interpolated
  linearly and periodically** (`monthly_precipitation`). This keeps the annual total exactly
  and invents no extremes (the driest/wettest months are solved months), but smooths the
  seasonality: dry seasons that peak between solved months are missed (biases Am/Aw → Af and
  s/w → f a little), and April and October decide much of the half-year split.
- Land with ice ≥ 50 % (as in the moisture mode) is **EF** whatever its temperatures. Ocean
  (and land below sea level, as everywhere in the viewer) has no class.
- Classified on the simulation side (`cell_infos`, the simulation thread / web worker): new
  `CellInfo::climate_class` (88 B/cell on the wire, +4) and one 1-byte channel in the frame
  history (40 B/cell raw, +1; codec version 5). Past frames show their own classes. Delta cost
  of the channel: ~0.95 kB per frame (`frame_sizes`, 5 subdivisions), about 1 % of a frame.
- Snapshot log: `climate classes of land: A …%, B …, C …, D …, E … | most common …`.
- Mode **Climate classes** (`--render-mode climate`, `?render-mode=climate`), unlit, coastlines.
  Options: *Show* = Classes (Beck's standard colours) / Main groups (Okabe–Ito colours, which
  stay apart for colour-blind eyes; the standard table does not: Cfa/Cfb/Cwa greens, Dfb/Dwb/Am
  blues, Csa/BSk yellows), *Borders* (lines between classes/groups on land). Legend: only the
  classes present, one row each ("Cfb — temperate oceanic (12 %)", share of land), ocean,
  coastline. New legend swatch shape `Entry` (a square on its own row) in the switcher.
- Cell widget: row *Climate* ("Aw — tropical savanna").
- **No change to simulation results**: state hashes of a 20 Myr run (5 subdivisions, seed 0)
  equal main's at every snapshot (fc84f72c…, ce25969a…, 7fd26710…).

## Plausibility (4 seeds, 6 subdivisions, 500 Myr, snapshots every 100 Myr)

Share of land (cells) per main group, mean of the snapshots 200–500 Myr:

| seed | A | B | C | D | E |
|---|---|---|---|---|---|
| 0 | 15.2 | 26.9 | 15.2 | 25.7 | 17.0 |
| 1 | 12.4 | 29.6 | 13.1 | 30.4 | 14.5 |
| 2 | 9.5 | 40.8 | 13.1 | 26.2 | 10.4 |
| 3 | 9.4 | 45.0 | 11.6 | 22.4 | 11.7 |
| **mean** | **11.6** | **35.6** | **13.2** | **26.2** | **13.4** |
| Earth (Beck et al. 2018, approx.) | ~19 | ~30 | ~13–15 | ~25 | ~13 |

Plausible for an Earth-like planet. The first 100 Myr are colder (E 20–40 %, much EF: the
initial highlands carry ice). Model weaknesses visible in the classes (for #132, not fixed here):

- **Too few tropical (A)**, and arid land dominated by **cold desert BWk** (12–28 % of land)
  rather than hot BWh: much desert is in mid-latitude continental interiors with MAT < 18 °C.
- In D, **hot-summer Dfa/Dwa** dominate over Dfb/Dfc, and **tundra ET is rare** (0–8 %): the
  known too-hot high-latitude summers (large continental seasonal range) push the warmest month
  over 22 °C and above 10 °C far poleward.
- Some Cs (dry-summer) appear where April is dry and October wet although July is the wettest
  month: a consequence of only 4 solved months and the AMJJAS/ONDJFM split (e.g. a Csb cell with
  200 / 1582 mm/yr in Jan / Jul).
- Coarse grid (~110 km; web ~220 km): classes are blocky, small highlands (Cwb, Dsc) and narrow
  coastal belts (Cfb strips, Mediterranean coasts) are one cell or missing. Accepted.

## Reviews

- Plan review (independent agent): rules and colour table confirmed; adopted: strict "> 70 %",
  months above 10 °C as T > 10, Kottek's s/w tie rule, documented mm conversion, interpolation
  caveats, EF for ice sheets and the treatment of the ocean.
- Code review (independent agent): no significant bugs (rules, units, interpolation, codec
  indices, web transport, determinism, colours checked). Minor, left as is: inland land below
  sea level counts as ocean (as everywhere in the viewer); ice-sheet EF overrides the
  temperature rule (by design).

## Checks

- Unit tests: 19 station climates (Singapore Af, Miami Am, Darwin Aw, Cairo BWh, Kashgar BWk,
  Niamey BSh, Denver BSk, Rome/Perth Csa, San Francisco Csb, Hong Kong Cwa, Buenos Aires Cfa,
  London Cfb, Tórshavn Cfc, Seoul Dwa, Moscow Dfb, Yakutsk Dfd, Utqiaġvik ET, Vostok EF), also
  shifted by half a year (other hemisphere); interpolation; codec round trip; mode colours,
  legend (every colour on the planet is in it) and overlay on a small planet; cell widget row.
- fmt, clippy (native all targets, wasm32), tests, doc with warnings denied.
- Desktop (virtual screen): classes, main groups with borders, a past frame
  ([desktop-classes.png](desktop-classes.png), [desktop-main-groups.png](desktop-main-groups.png)).
  Web at phone width (400 px): legend and cell widget
  ([web-phone-legend.png](web-phone-legend.png), [web-phone-cell.png](web-phone-cell.png)).
