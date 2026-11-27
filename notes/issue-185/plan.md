# Issue #185: the viewer draws lake ice from the temperature model

## Starting point

Since #173 / PR #180 the temperature model has a lake column per coarse cell (fresh water,
freezing at 0 °C at the water surface, `temperature.open_water_lakes`, on by default). It
reports, on the fine cells that are lakes in the model (not ocean, ≥ `LAKE_DEPTH` = 1 m of lake
water):

- `TemperatureFields::ice`: the annual mean lake-ice cover (0–1; as for sea ice: ice thickness
  over `ice_albedo_thickness`, capped at 1);
- `TemperatureFields::monthly_lake_ice[month]`: the mean cover of every month (NaN elsewhere).

The viewer already gets the annual value (`CellInfo::ice`, used by the "Permanent ice" row, the
topographic colours via `render::surface_color` and the moisture mode: ice where it is > 0.5,
so those already agree with the model). But the **Realistic** mode ignores it for lakes: it
draws lake ice from its own rule (annual: annual mean below −10…−20 °C; January/July: the
month's temperature below −1…−10 °C by depth, or a warmest month below 0…−5 °C). The months'
lake ice is not in the frames at all.

## Plan

1. `CellInfo` gets `lake_ice_january` and `lake_ice_july` (f32, the model's mean lake-ice cover
   of month 0 and month 6, the months of `temperature_january/july`; NaN where the model has no
   lake). 148 instead of 140 bytes per cell in memory.
2. Frame codec: two 1-byte `Share` channels (steps of 1/254, 255 = NaN), codec version 11 → 12.
   Off the lakes the value is a constant NaN, so the delta is zero there and lz4 removes it;
   the cost is in the lake cells only. Measure with the `frame_sizes` test before/after.
3. Realistic mode, lakes (`WaterBody::Lake`, i.e. ≥ 1 m of water; dry depressions stay land):
   - where the model has lake ice for the cell: the ice layer's cover is the model's cover:
     the month's mean for January/July, and for the annual state the annual mean cover
     sharpened around one half (as for sea ice and for the permanent ice of the other modes:
     frozen more than half the year). The shader draws ice where the cover exceeds one half, so
     the lake is drawn frozen exactly where the model's cover is > 0.5.
   - where it has none (lakes off with `--set temperature.open_water_lakes=0`, or a lake that
     appeared in the erosion step after the temperature update, which saw the lakes of the
     step before): the old rule as a fallback.
   - snowy vs bare ice as before (by the month's precipitation).
4. Legend: a "Lake ice" swatch; the description says lake ice comes from the temperature model.
   Cell info: the "Permanent ice" row shows for lakes also the January / July lake ice.
5. Lakes below sea level keep the model's temperatures (`lake_air` is unchanged: it only fills
   dry depressions below sea level).
6. Tests: a planet test that the viewer's lake ice (after the frame codec) equals the model's
   in every lake cell and period; unit tests of the layers with the model's cover (frozen in
   a mild month if the model says so, open in a cold one if it says so) and of the fallback.
7. Screenshots (vscreen, Realistic, January and July) before/after, a phone-size web shot if
   the web UI changes (it only gains a legend swatch and text).
