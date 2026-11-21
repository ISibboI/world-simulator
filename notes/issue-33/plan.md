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

## Cause (from the code, `src/temperature.rs`; revised after the plan review, [`review-plan.md`](review-plan.md))

1. The energy balance runs on a coarse mesh (~440 km) at sea level. Fine cells get the land
   temperature of their coarse cell minus lapse rate × elevation.
2. Ice sheets start at an annual mean of −5 °C and are complete at −15 °C
   (`ice_sheet_cover`). The ice fraction of a coarse cell's land raises its albedo
   (0.32 → 0.62): in the tropics ~120 W/m² less sunlight per unit of ice fraction.
3. Heat transport is almost all between latitude **bands**; the exchange between neighbouring
   cells is deliberately weak, so that continents keep their seasons (diffusion length
   ~1000 km). High ground more than ~1000 km across that carries ice therefore cools itself
   by tens of K and keeps its ice: a second, self-sustaining state.
4. **The spin-up started in that state.** It began with a land temperature of 15 °C at sea
   level, which puts every plateau above ~3 km below the ice threshold in the first year.
   Where the iced state sustains itself, it stayed for the whole run, since the model keeps
   its state from step to step. Unit test: a 3.5 km plateau in the subtropics, well above
   freezing by the lapse rate, ends fully glaciated at −35 °C.

On a real planet the iced state of point 3 cannot exist in low latitudes. High ground stands
above the boundary layer in the free troposphere, which the tropical circulation keeps nearly
uniform along a latitude circle (weak temperature gradients where the Coriolis force is small;
Sobel et al. 2001), at a temperature set by convection over the sea. A plateau cannot stay tens
of K colder than the air around it.

## Change

1. **The spin-up starts without ice sheets** (`ICE_FREE_START`): ice forms where the land
   turns out cold enough, instead of being put on every plateau first.
2. **Low-latitude high ground is at least as warm as the free air at its height.** Annual mean
   of a land cell at elevation z and latitude φ:

   `T = T_local + w × max(0, T_free − T_local) − lapse rate × z`,
   `w = clamp((z − 1 km) / 2 km, 0, 1) × clamp((45° − |φ|) / 15°, 0, 1)`

   - `T_local`: the land temperature of the cell's coarse cell (as now).
   - `T_free`: the annual mean sea-surface temperature of the latitude band (the 18 bands of
     the energy balance, interpolated between band centres), blended smoothly with the mean of
     all surfaces of the band where it has little sea (review finding 4).
   - Only warming (finding 2): land hotter than the sea of its latitude keeps its own
     temperature; real plateaus are elevated heat sources.
   - Only in low latitudes (finding 1): full to 30°, none beyond 45°
     (`temperature.free_air_latitude`). A large cold plateau at high latitude makes its own
     climate under a surface inversion (the Antarctic plateau is ~30 K colder than the free
     air at its height), and its ice must keep its own feedback.
   - `temperature.free_air_height` (3000 m; 0 turns part 2 off) and
     `temperature.free_air_base` (1000 m). Boundary layers are 1–3 km deep: ground below 1 km
     lies inside one and keeps its own balance; ground above 3 km is in the free air all day.
     The first two versions let the weight grow from sea level. The results evaluation found
     that this warmed ordinary tropical uplands by 2–5 K (300 m to 2 km), because tropical
     lowland in the model is ~5 K colder than the sea of its latitude, a separate bias.
   - The shift is the same in every month: the seasonal cycle stays the local one.
   - The same shifted temperature decides the ice cover that sets the land albedo in the
     energy balance, and the ice field of the output. The energy balance itself is unchanged:
     where highland ice remains, its coarse cell still bears the whole deficit.

Tried first and dropped: a regional reference (air or sea-surface temperature smoothed over
1500 km). The coarse cells around an iced plateau are themselves cold, so a regional mean of
the air inherits the runaway it should break; and the nearest sea of a plateau inside a
continent can lie at another latitude (in the test: 14 °C from the subtropics instead of 25 °C
from the tropical ocean). Also dropped after the review: the first version applied the shift
at all latitudes and in both directions.

Not changed: the ice criterion itself. A summer-melt (degree-day) criterion would be more
physical for seasonal climates, but #24 found that it tips the model into a snowball. Between
45° and the poles, self-sustaining ice on large plateaus remains possible; from an ice-free
start it only forms where the land is cold enough without it.

## Tests

- `low_latitude_plateaus_are_not_colder_than_the_free_air_around_them`: an equatorial 4.5 km
  plateau and a subtropical 3.5 km one. Started glaciated without the free air, each stays
  glaciated, more than 10 K colder than the open ocean of its latitude (both at sea level).
  With the free air, the ice melts and the plateau is no colder than the ocean minus 2 K.
  From an ice-free start the free air never cools and never adds ice.
- `polar_plateaus_keep_their_own_climate`: a 3 km polar plateau is bit-identical with and
  without the free air, and fully ice-covered although the spin-up starts without ice.
- Existing temperature tests unchanged.

## Validation

Three sets, 4 seeds × 1000 Myr each (CPU is shared with two other tasks; 8 seeds if anything is
near significance): `main` (the first build of this branch with `temperature.free_air_height=0`,
bit-identical to `main`), "ice-free start only" (final build with
`temperature.free_air_height=0`), and the final build.

| Metric | main (triage, 8 seeds) | target |
|---|---|---|
| Tropical land > 3 km, reduced to sea level, minus tropical lowland (K) | −3.8 | within ±2 |
| Tropical land below −10 °C (% of tropical land) | 1.0 | ≤ 0.3 |

Guard rails (review): land below −10 °C poleward of 60° and at 45–60° (ice sheets must stay:
change below ~10% unless explained), temperature of polar land and of land above 2 km poleward
of 50°, ice and temperature of land above 3.5 km at 15–35°, global mean, equator and polar
temperatures, permanently frozen share, seasonal range of land at 55–70°, land precipitation
and arid share, runoff, sediment to the ocean, mean land and highest cell, runtime. No snowball
in any run. The abiotic start at t = 0 (#29 found −6.4 K against modern): report with and
without the change. The solar-constant sweep of the existing test suite still passes.
