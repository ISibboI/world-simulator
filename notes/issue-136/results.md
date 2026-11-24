# #136 Moisture mode: P − E — results

Plan: [plan.md](plan.md).

## What changed

- Moisture mode, **Show: P − E** (third choice next to Precipitation and Aridity), with the
  Season option (Year / January / July). BrBG diverging scale (colour-blind safe; brown = the
  surface loses water, blue-green = gains, white = 0) on a signed logarithmic axis
  `sign(x)·log10(1 + |x| / 10 mm/yr)` up to ±3000 mm/yr, ticks −1000, −100, −20, 0, +20, +100,
  +1000 (fewer ticks so the labels fit at phone width). Legend text per season.
- Cell widget: new row **P − E**, annual, with the evaporation: "+75 mm/yr (evap. 298)".
- Data: `CellInfo` gets `evaporation`, `evaporation_january`, `evaporation_july` (the water
  cycle's actual evaporation: bulk formula over the ocean, Budyko evapotranspiration on land).
  `WaterCycle::monthly_evaporation` exports the solved months' evaporation (export only; the
  simulation is unchanged).
- History codec version 4 → 5: the three fields as 1-byte channels on a square-root axis up to
  8 m/yr (new `Code::Root`).

## Sanity check: global P − E

Area-weighted means from the frame values (5 subdivisions, seed 0; `moisture_distribution`
test):

| time | P (mm/yr) | E (mm/yr) | P − E year | Jan | Jul |
|---|---|---|---|---|---|
| start | 894.84 | 894.88 | −0.037 | −0.019 | −0.024 |
| 50 Myr | 1025.16 | 1025.20 | −0.039 | −0.017 | −0.027 |

The model's own (f64) global means are equal to all digits (894.854 / 894.854 and
1025.203 / 1025.203; no clipped precipitation): the water cycle closes. The −0.04 mm/yr is the
f32/flat-triangle area weighting of the check, 4 · 10⁻⁵ of P.

## Distributions (50 Myr, m/yr)

| | 1 % | 5 % | 25 % | 50 % | 75 % | 95 % | 99 % |
|---|---|---|---|---|---|---|---|
| P − E year, ocean | −1.22 | −0.92 | −0.39 | −0.06 | +0.24 | +0.52 | +0.68 |
| P − E year, land | −0.33 | −0.00 | +0.04 | +0.18 | +0.38 | +0.82 | +1.59 |
| P − E Jan, land | −0.69 | −0.39 | −0.07 | +0.11 | +0.46 | +1.17 | +2.16 |
| P − E Jul, land | −0.94 | −0.51 | −0.10 | +0.07 | +0.42 | +1.34 | +2.31 |
| E year, ocean | 0.12 | 0.16 | 0.60 | 1.41 | 1.84 | 2.06 | 2.15 |
| E Jan, land | 0.001 | 0.007 | 0.05 | 0.21 | 0.78 | 2.80 | 4.97 |

Over the ocean the pattern is as expected: net evaporation in the subtropics (down to about
−1.5 m/yr), gain in the convergence zone and the storm tracks (up to +0.7 m/yr, +2.8 locally);
the ocean as a whole loses what the land runs off. Seasons: the winter storm track gains most
(January in the north, July in the south).

### Caveats (model properties, not of the view)

- **Annual land P − E is slightly negative on ~5 % of land cells, below −0.3 m/yr on ~1 %**
  (at the start: minimum −37 mm/yr). Likely cause: the water cycle refreshes one month per
  update, and each month keeps the evapotranspiration computed with the annual precipitation
  (and land mask) of the update when it was solved; where precipitation shifted (or land rose
  from the sea, keeping ocean evaporation in the older months), the annual E no longer matches
  the Budyko curve of the current P. The rivers use the current Budyko runoff (≥ 0), so the view
  shows the water cycle's E honestly and differs from the runoff the rivers get there.
- **Monthly land evapotranspiration reaches ~6 m/yr rates** (about 16 mm/day) in summer:
  the annual Budyko ET is spread over the months in proportion to PET, so highly seasonal
  continents concentrate it in summer; real summer ET rarely exceeds ~6–8 mm/day. Hence monthly
  land P − E down to −1.5 m/yr in the dry season (the view's text says this is stored water).
- **Lakes** evaporate more than the land they cover in the river model, which takes it from the
  runoff and does not return it to the air; there is no per-cell field for it, so it is not in E.
  The legend says so.

These three are reported on #132 (climate-model weaknesses), not changed here.

## Frame size (`frame_sizes`, 5 subdivisions, 10 242 cells, 40 steps)

| | main | this branch |
|---|---|---|
| quantised bytes per cell | 39 | 42 |
| CellInfo (live frames) | 84 B/cell | 96 B/cell |
| keyframe (2nd half) | 224.0 kB | 254.9 kB |
| delta (2nd half) | 106.6 kB | 114.3 kB |
| stored per frame (1 key in 25) | 111.3 kB | 120.0 kB (+7.8 %) |

Channels: annual E 5.0 kB/delta, January and July E 1.8 kB each. With 2-byte 1 mm/yr channels
the stored frame would have been 129.2 kB (+16 %): the square-root byte halves the cost.
Quantisation steps: 4 mm/yr at 30 mm/yr, 11 at 0.25 m/yr, 22 at 1 m/yr, 31 at 2 m/yr; on the
P − E scale that is a fraction of one colour step everywhere (past frames look like live ones),
but the cell widget of a past frame can show a desert's runoff a few mm/yr off.

## GUI

- Desktop (vscreen, 5 subdivisions): [annual](desktop-annual.png) with a land cell,
  [July](desktop-july.png), [January on a past frame (20 of 30 Myr)](desktop-past-frame-january.png).
  These were taken with an earlier tick set (±500, ±2000 instead of ±1000).
- Web at phone width (390 px): [legend](web-phone.png); the third choice wraps to its own line.

The coarse grid shows: hexagonal steps in the high-latitude bands and sharp coastal contrasts
(one cell of ocean evaporation next to land runoff) are the ~220 km cells of the web/test
resolution.

## Possible conflicts

PR #138 (#97 wind) also bumps the codec version to 5, adds CellInfo fields and channels and
moves `Period` into `modes/mod.rs`. Whichever merges second needs version 6, and the
`Period::evaporation`/`balance` helpers move with `Period`.
