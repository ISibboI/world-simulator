# #33 plan: tropical highlands follow the free troposphere

_2026-11-21. Triage: [`../realism-triage/overview.md`](../realism-triage/overview.md) (batch 3)._

## What is wrong

Measured in the triage (`main`, 8 seeds × 1000 Myr, 500–1000 Myr):

- Tropical land (|latitude| < 25°) above 3 km is 4 K too cold for its height: reduced to sea
  level with the model's own lapse rate it is at 18.2 °C (seeds 15.1…22.5), the lowland around
  it at 22.0 °C.
- Half of it (1.0% of the tropical land, up to 2.9% in a seed) is below −10 °C in the annual
  mean, the ice-sheet criterion. By the lapse rate alone (6.5 K/km from ~22–26 °C) that needs
  more than 5 km; the highest cells are ~4.6 km.

## Cause (from the code, `src/temperature.rs`)

1. The energy balance runs on a coarse mesh (~440 km) at sea level. Fine cells get the land
   temperature of their coarse cell minus lapse rate × elevation.
2. Ice sheets start at an annual mean of −5 °C and are complete at −15 °C
   (`ice_sheet_cover`). A plateau of ~4.5 km in the tropics reaches −5 °C by the lapse rate.
3. The ice fraction of a coarse cell's land raises its albedo (0.32 → 0.62). In the tropics
   that is ~400 W/m² × 0.3 = 120 W/m² less sunlight per unit of ice fraction.
4. Nothing spreads this loss. Heat transport is almost all between latitude **bands**; the
   exchange between neighbouring cells is deliberately weak, so that continents keep their
   seasons. So the coarse cell cools by itself, its fine cells get colder, more of them cross
   −5 °C: a local ice–albedo runaway on one coarse cell.

On a real planet step 4 fails. High ground stands above the boundary layer in the free
troposphere, which the circulation mixes over the Rossby radius of deformation (~1000 km in
mid-latitudes, more towards the equator; in the tropics the free troposphere is nearly uniform:
the weak-temperature-gradient balance, Sobel et al. 2001). The surface of a mountain or plateau
has about the temperature of the free air at its height (Pepin & Seidel 2005), not that of a
local radiative balance. Its ice does reflect sunlight, but the deficit is shared by the region.

## Change

Annual mean temperature of a land cell at elevation z:

`T = T_local + w(z) × (T_regional − T_local) − lapse rate × z`, `w = min(z / 3 km, 1)`

- `T_local`: the land temperature of the cell's coarse cell (as now).
- `T_regional`: the annual mean air temperature (land and ocean surfaces of the coarse cells by
  area), smoothed over 1500 km on the coarse mesh.
- New parameters `temperature.free_air_height` (3000 m; 0 turns the change off and reproduces
  `main`) and `temperature.free_air_radius` (1.5 × 10⁶ m).
- The shift is the same in every month: the seasonal cycle stays the local one (plateaus are
  strong seasonal heat sources and sinks).
- The same shifted temperature decides the ice cover that sets the land albedo in the energy
  balance, and the ice field of the output.
- The energy balance itself is not changed: an iced highland still reflects, and its coarse
  cell still cools at sea level. Only the highland no longer inherits all of that cooling.

Why 3 km: boundary layers are 1–3 km deep; ground above that is in the free troposphere all
day. Why linear: no better knowledge; the weight is 0 at sea level, where the local balance
certainly holds.

Not changed: the ice criterion itself. A summer-melt (degree-day) criterion would be more
physical for seasonal climates, but #24 found that it tips the model into a snowball; it does
not address this issue, where the tropical highlands have no summer.

## Tests

- A tropical continent with a 4.5 km plateau: without the change the plateau is several K
  colder than the lowland around it at the same level and carries ice; with it, the difference
  is below 3 K and the ice cover below 0.3.
- Existing temperature tests unchanged (sea level and low land are not touched).

## Validation

`main` behaviour = the same binary with `--set temperature.free_air_height=0` (the shift is
then exactly zero). 4 seeds × 1000 Myr each first; 8 if anything is near significance. CPU is
shared with two other tasks, so runs are on 2 cores.

| Metric | main | target |
|---|---|---|
| Tropical land > 3 km, reduced to sea level, minus tropical lowland (K) | −3.8 | within ±2 |
| Tropical land below −10 °C (% of tropical land) | 1.0 | ≤ 0.3, and only above ~5 km |

Guard rails: global mean, equator and polar temperatures, permanently frozen share, land with
annual mean below −10 °C, seasonal range of land at 55–70°, land precipitation and arid share,
runoff, sediment to the ocean, mean land and highest cell (ice and temperature feed erosion
through the water cycle), runtime. No snowball in any run. Abiotic start at t = 0 (#29 found
−6.4 K against modern from ice on the initial plateaus): report the difference with the change.
