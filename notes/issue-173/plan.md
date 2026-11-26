# Plan: lakes as open water in the climate, then drop the 0.2% rule (#173)

_2026-11-26. Issue: [#173](https://github.com/ISibboI/world-simulator/issues/173). Background:
#126/#168 ([spill-results.md](../issue-126/spill-results.md), section "The 0.2% rule"). Goal: an
Earth-like planet, not Earth; grid effects (~110 km cells, ~440 km for the temperature model)
accepted (#64)._

## How water is represented today

**Temperature model** (`src/temperature.rs`, energy balance on a coarse icosphere, 4
subdivisions = 2562 cells ≈ 440 km). Every coarse cell has a *land surface* (heat capacity of the
air column, 1e7 J/m²/K) and an *ocean column* (Wagner & Eisenman 2015: enthalpy = mixed-layer heat
above the freezing point, 50 m = 2.1e8 J/m²/K, or latent heat of sea ice; ice surface temperature
from conduction; albedo `a + b P₂` for open water, sea-ice/melting-ice albedo with ice cover). The
two exchange heat through the air (`land_ocean_exchange` 10 W/m²/K, weighted by the other
surface's fraction); the air (area mean) drives the band-mean meridional transport and a weak local
diffusion. Ocean columns get ocean heat transport: Ekman/subtropical cells per latitude band
(spread over the band's ocean) and wind-driven gyres (cells with land fraction < 0.5). The land
fraction of a coarse cell is the share of its fine cells with elevation ≥ 0 in the *climate view*
(`planet::climate_elevations`: everything not connected to the ocean is raised to ≥ 0, so lakes and
closed depressions are land). Fine-cell temperatures: land cells interpolate the coarse land
temperature (sea-level equivalent) and apply the lapse rate (+ free-air shift); ocean cells
interpolate the ocean surface. `fields.ice` = sea ice (ocean cells) or ice sheets (land cells).

**Water cycle** (`src/climate.rs`, fine mesh). Ocean cells (view < 0) evaporate by the bulk formula
at the SST with marine air (RH 0.8, ΔT 1 K). Land cells get a potential evaporation (bulk formula
at the fine land temperature, with the actual air humidity) and Budyko ET. Lakes (#126):
`lake_potential = potential × ice_free(T)` (ramp −2…+2 °C on the monthly mean); the erosion's lake
balance evaporates at that rate, and the excess over ET (`water.lake_evaporation`) comes back to the
air as a moisture source in the following steps.

**Order within a step** (`Planet::step`): tectonics → ocean seeds/view → temperature update →
water cycle → erosion (lake balance with this step's climate) → `self.water` = lakes at the end of
the step. So the climate of step N sees the lakes of step N−1 (as the lake evaporation already does,
#126). One step = 1 Myr; the lag is harmless.

## Design

### 1. Lake columns in the temperature model (switch `temperature.open_water_lakes`, default on)

- **Which lakes:** all lake cells (`water.lake_depth > 0`) that the climate view does not see as
  ocean. No area threshold: physically every water surface has the water's heat capacity, and at
  the coarse resolution a lake's influence scales with its area share of the coarse cell anyway (a
  one-cell lake is ~6% of a coarse cell). Small lakes then get the temperature and ice of a water
  body (correct for a lake, if coarse), with little effect on the land around them; large lakes and
  cut-off seas dominate their coarse cells and behave like sea.
- **A third surface per coarse cell**, the *lake column*, with its own enthalpy and surface
  temperature, beside land and ocean. Fractions `f_land + f_ocean + f_lake = 1` (lake fine cells
  leave the land count). Same physics as the ocean column, via one shared function (the ocean
  column code is factored out, not duplicated):
  - heat capacity `ρc · mean(min(depth, H))` over the coarse cell's lake cells, with `H` the ocean
    mixed layer (50 m, from `ocean_heat_capacity`): shallow lakes have less inertia, deep lakes the
    ocean's (seasonal mixed layer; deeper water does not take part in the seasonal cycle);
  - no ocean heat transport (no Ekman share, no gyres; the gyre mask stays the ocean);
  - fresh water: freezes and melts at 0 °C at the lake surface. The coarse columns work in
    sea-level-equivalent temperatures (like land), so a lake column freezes at `Γ z̄` (z̄ = mean
    water-surface elevation of its lakes, Γ the lapse rate): a lake at 3000 m freezes when the
    sea-level temperature is +19.5 °C;
  - albedo: the ocean's open-water albedo (`a + b P₂`, planetary incl. clouds) and the sea-ice /
    melting-ice albedos with the same ice-cover ramp; same ice conductivity, latent heat, thickness
    cap;
  - heat exchange through the air with the other surfaces of the cell, each weighted by its
    fraction (`k Σ_j f_j (T_j − T_i)`, pairwise conservative; reduces exactly to today's land–ocean
    exchange when `f_lake = 0`); same meridional and local transport terms as the other surfaces.
  - The lake share enters the air temperature (area mean), the band heat capacity and hence the
    meridional transport, the thermal equator, and through them the maritime effect on the land of
    the same and neighbouring coarse cells. Ocean-only quantities (band SST for the Ekman heat
    flux, subduction temperature, ocean area for gyres, the free troposphere's "sea surface") stay
    ocean-only.
  - Coarse cells without lakes keep a virtual lake column (as every cell has a virtual ocean column)
    so that a lake appearing there starts from a sensible state; it uses H and z̄ = 0.
- **Fine lake cells** take the lake surface temperature interpolated from the coarse cells *that
  have lakes* (weights renormalised; the cell's own coarse cell always has one), lowered with the
  lapse rate to the water surface (and the free-air shift like land). Their annual `ice` is the
  lake ice cover (as sea ice for ocean cells), and a new field `monthly_lake_ice` (fine, NaN off
  lakes) gives the monthly mean ice cover for the water cycle.
- **Water-surface elevation:** with the switch on, `climate_elevations` puts lake cells at their
  water surface (`max(0, floor + load correction + lake depth)` from the last erosion step) instead
  of their floor, for the lapse rate and the saturation column over the lake.

### 2. Water cycle (same switch, through `monthly_lake_ice`)

- Lake cells' potential evaporation is the bulk formula at the *water* surface temperature (it
  follows automatically from the fine temperatures), with the actual air humidity (not the marine
  RH: a lake in a desert evaporates into dry air, Aral/Dead Sea).
- Lake ice: `lake_potential = potential × (1 − ice cover of the month)` from the lake column
  instead of the −2…+2 °C ramp, and the seasonal weights of the returned lake evaporation
  (`lake_season`) use the same open-water share. Cells that are not lakes yet (a lake may form in
  this step's erosion) keep the ramp at their land temperature. Ocean evaporation is unchanged (it
  is not reduced under sea ice today; noted, out of scope).

### 3. The 0.2% rule

`min_sea_fraction` stays a parameter (to reproduce old runs with `--set min_sea_fraction=0.002`),
default off (1.0, documented; `sea_seeds` returns early from 1.0 on instead of flooding every
basin). If the runs show a residual problem, report it with numbers and propose the minimal fix
rather than keeping the rule silently.

### Not changed

Viewer (it draws lake ice from its own heuristic; a follow-up could use the model's ice), the
erosion's lake balance, ocean evaporation under sea ice, salinity (cut-off seas are fresh for the
freezing point).

## Reproducibility

With `--set temperature.open_water_lakes=0 --set min_sea_fraction=0.002` the state hash must equal
main's (e7d2a14) for a few hundred Myr at 5–6 subdivisions; the new terms are written so that they
add exact zeros when `f_lake = 0`.

## Tests

- Temperature: a large lake on a continent at mid latitude has a smaller seasonal range than the
  land around it and than the same cells without the lake; land next to it gets a smaller range
  than with the switch off; a polar lake freezes in winter (monthly ice > 0) and thaws in summer;
  a shallow lake has a larger range than a deep one; with no lakes the fields equal the switch-off
  fields exactly.
- Water cycle: lake potential evaporation is zero in a month with full lake ice and equals the
  potential evaporation with none; the returned lake evaporation still sums exactly to the input.

## Measurement

8 seeds × 1000 Myr, 6 subdivisions, `--duration 1000 --snapshot-interval 50 --width 512
--cell-dump --lake-log-from 500`, window 500–1000 Myr, via `runs`:
- `main` (e7d2a14, rule on), `main-nosea` (`--set min_sea_fraction=1`);
- `lakes-rule` (this work, `--set min_sea_fraction=0.002`), `lakes` (this work, defaults = rule off).

Metrics (compare.py from #126/#168 plus new ones; the cell dump gets an `ice` column): endorheic
share, lake area, lake water (m of sea level; mean, largest snapshot) with seed scatter; lake
temperatures (annual, Jan/Jul seasonal range) and lake ice share vs land at the same latitudes;
land near large lakes (≥ 20 cells; rings of 1 and 2–3 cells): seasonal range, annual temperature,
precipitation, compared with land farther away in the same latitude band (difference of
differences between sets, since seeds diverge); global temperature, frozen fraction, aridity,
Köppen, hypsometry, water conservation (deficit after the sea-level solve), cut-off seas (largest
lake per snapshot). Runtime: paired local runs (seed 0, 200 Myr, 6 subdivisions, one core each).
Case study: the largest cut-off basin of the `lakes` set (temperature, ice, precipitation maps).

## After the plan review ([plan-review.md](plan-review.md))

- M1, M3, S1: as planned; the ocean share is `(1 − f_land) − f_lake`, every former `1 − f_land`
  use is ocean-only or includes the lakes as listed in the review; land ice sheets and the land
  albedo leave lake cells out. `climate_elevations` is unchanged with the switch off.
- M2: the water cycle uses the column's ice only where it is a number; elsewhere (NaN, also lake
  cells the current view sees as ocean or cells that were no lake last step) the ramp, and the
  monthly weights of the returned lake evaporation still average to one (exact return).
- S3: lake cells get the lapse rate only (no free-air shift), the same reduction their column's
  freezing point uses.
- S4: the mixed layer of shallow lakes is at least the land's heat capacity (~2.4 m of water).
- S5: the viewer's `lake_air` (air temperature of the land around for low cells) now applies to
  dry closed depressions only; lakes keep the model's temperatures. Lake ice in the viewer keeps
  its own heuristic (follow-up, noted in the results).
- S6: a paired diagnostic `--lake-climate-contrast`: at every snapshot the temperature model is
  cloned and run 10 updates (20 years) on the same state with lakes as open water and as land;
  logged for all lakes, lakes ≥ 20 cells, land next to them and 2–3 cells away.
- S7, criteria for the rule, written before the runs: drop the rule if without it (a) water stays
  conserved (deficit ≲ 1 m of sea level per snapshot), (b) large closed basins are open water in
  the climate (lake columns, ice where cold) with plausible temperatures (paired diagnostic), and
  (c) global temperature, frozen share, land precipitation, aridity and Köppen B of the land
  without lakes stay within the seed scatter of the rule-on sets. The lake column changes the
  climate of a cut-off basin, not whether it fills (its precipitation minus evaporation decides);
  a residual "how much water sits in cut-off basins" is reported with numbers either way.
- S8: land metrics are also reported for the land without lake cells.
- C2 (ice cap on lakes adds energy), C4 (no ocean heat transport, 50 m mixed layer for large
  cut-off basins), C5 (ocean evaporation not reduced under sea ice) are documented caveats.
