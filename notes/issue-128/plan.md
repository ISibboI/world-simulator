# Plan: draw closed depressions as lakes (≥ 1 m water) or land, not sea (#128)

_2026-11-25. Stacked on #168 (agent/issue-126, d04ba2a). A viewer/rendering change: no
simulation result may change (state hash identical)._

## What the chain already has

- `CellInfo::drainage` carries `OCEAN`, `DEPRESSION` and `LAKE` (≥ `LAKE_DEPTH` = 1 m, a bit
  of its own) and `CellInfo::lake_depth`; `CellInfo::water_body()` → Ocean / Lake(depth) /
  DryDepression / Land (#146). The frame history keeps the low 8 drainage bits and the lake
  depth, so past frames already know ocean/lake/dry — **no codec change needed**.
- Using it already: the hydrology mode, the drainage tracing, the cell widget (ocean / lake with
  depth / dry depression).
- `CellInfo::elevation` is `Planet::surface_elevation` (#127): lake floors pressed down by
  their water, dry depressions below sea level unloaded. Lake water level = floor + depth.

## Still deciding "ocean" by `elevation < 0` (to fix)

| place | now | change |
|---|---|---|
| `viewer::surface_vertices` (3D relief, all lit modes) | height `elevation.max(0)` | ocean at radius 1; lakes at their water level (`elevation + lake_depth`); land at its elevation, also below sea level (dry depressions dip below the sea surface) |
| `render::surface_color` (topographic vertex colours) | `elevation ≥ 0` land | takes the water body: ocean bathymetric colours, lakes a lake blue by depth, land (also below sea level) land colours; rivers only on land; permanent ice on lakes as water ice |
| PNG `render_elevation`, `render_temperature` (coast), `render_precipitation` | `surface_elevation < 0` | `Planet::surface_water()` ocean mask + lake depth ≥ 1 m, same colours as the 3D view |
| `modes::coastlines`, sediment mode's coast | `elevation ≥ 0` | boundaries between ocean, lake and land (3 categories): lake shores drawn too |
| realistic mode (`layers`, coast heights) | land = `elevation ≥ 0`, lake = `lake_depth > 0` | ocean / lake (≥ 1 m) / land by `water_body()`; lake coast height by depth |
| Köppen mode colour and legend counts | ocean = `elevation < 0` | `is_ocean()` |
| moisture mode ground, statistics; `CellInfo::aridity_index` | `elevation < 0` / `≥ 0` | `is_ocean()` |
| temperature mode ice line (land only) | `elevation ≥ 0` | `!is_ocean()` |
| ocean currents mode (ocean only) | `elevation < 0` | `is_ocean()` |
| `koppen::classes` (Köppen of the cell widget, the mode and the log's shares) | `surface_elevation < 0` → none | ocean mask: dry depressions and lakes below sea level get a class (diagnostic only; the log's Köppen shares change slightly, the state hash not) |

Rule for the data modes (Köppen, aridity, temperature, precipitation…): lakes and dry
depressions are land of the climate and keep their data colour; the coastline overlay outlines
the lakes. Surface modes (topographic, realistic, the PNG elevation map) draw lakes as water.

`LAKE_DEPTH` moves to `render` (used by the PNG maps without the viewer), re-exported in
`cell_info`. Topographic legend: swatches "Lake" and "Land below sea level" (a darker lowland
green, so dry depressions stand out from the sea).

## Checks

- Unit tests: `surface_color` per water body; `surface_vertices` heights (lake at water level,
  dry depression below radius 1, ocean at 1); coastlines with a lake; realistic layers of a dry
  depression below sea level (land) and of a 0.5 m lake (land); Köppen colour of a dry
  depression; PNG elevation of a lake below sea level.
- State hash identical before/after (seed 0, 100 Myr, `--web-math`).
- Visual: desktop, web (headless chromium, swiftshader) and phone 390×844 on vscreen, a seed and
  time with a lake below sea level and a dry depression below sea level; before/after
  screenshots in `notes/issue-128/`.
- CI: fmt, clippy native + wasm32, tests (incl. the naga shader test), doc.

## Plan review (independent agent) and revision

- **Required, done — mask vs elevation timing**: the drainage bits (ocean, depression, lake)
  come from the last erosion step's water state, the displayed elevation from after its
  surface changes (deposition, waves, sea-level change), so coastal cells can cross sea level
  between them (deltas built to +2 m in the mask, coasts eroded just below 0 outside it). Rule
  now (`render::Water::of`, `CellInfo::water_body`): in a closed depression a lake (≥ 1 m) or
  land; elsewhere ocean below sea level and land above it. Only the depression/lake bits
  override the elevation's sign; no dark-green fringe along coasts. The PNG maps use the same
  rule (`Water::of_cell`: depression and not ocean, lake depth ≥ 1 m, as `drainage_bits`).
- **Required, done — missed sites**: ocean currents `color()`; realistic mode ocean depth,
  relief (lakes no longer get land relief), lake ice (water ice, not an ice sheet), coast
  height (lakes at least 0.5 so the coast noise does not erase small lakes); picking comment
  (a ray ending on the radius-1 sphere picks the dry depression under it); tests with ocean
  fixtures (consistent with the new rule: negative elevation outside depressions is ocean).
- **Data modes (decision for Sebastian)**: Köppen and aridity colour the ocean as water, so
  they now draw lakes (≥ 1 m) as water too ("Ocean and lakes" swatch) and leave them out of
  the class shares of the Köppen legend; dry depressions get their class. Temperature,
  precipitation and the other field modes keep their data colour on lakes; the coastline
  overlay outlines the lakes.
- Checked OK: frame history sufficient (drainage low 8 bits, lake depth at 0.1%), state hash
  untouched (Köppen only in the log), no separate ocean sphere (dry depressions below radius 1
  are safe).

## Code review (independent agent)

No blocking bugs; planet side (`Water::of_cell`) and viewer side (`CellInfo::water_body`)
agree, the state hash is untouched. Fixed: the cell widget's land rows used the drainage ocean
bit (now `is_sea`); the Köppen log counted lakes while the mode's legend left them out — now
`koppen::classes` gives lakes no class (water, like the ocean), so log, legend, mode and widget
agree. Accepted edge cases of the sign rule (rare, a cell at most): a rim cell less than 1 m
below its depression's spill level carries no depression flag, so if it is also just below
sea level it is drawn as ocean; a cell flagged as a depression in the last step that the step
opened to the sea is drawn as land or lake beside the ocean until the next step.
