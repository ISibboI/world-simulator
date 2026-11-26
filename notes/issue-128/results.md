# Results: closed depressions drawn as lakes (≥ 1 m water) or land (#128)

_2026-11-25. Stacked on #168 (agent/issue-126). Plan, plan review and code review:
[plan.md](plan.md)._

## What was already there

The chain already knew the water of every cell (#146): ocean, closed depression and lake
(≥ 1 m) bits and the lake depth in `CellInfo`, also in the frame history. The hydrology mode
and the cell widget used them. Everything else (the 3D relief and topographic colours, the
coastlines, the realistic, Köppen, aridity, temperature, ocean-current and sediment modes, the
PNG maps and the Köppen classes) still decided "sea" by `elevation < 0`.

## What changed (viewer and output only)

- **One rule everywhere** (`render::Water::of`, `CellInfo::water_body`). In a closed
  depression, a cell is a lake if it holds at least 1 m of water, otherwise land, also below
  sea level. Elsewhere, a cell is ocean below sea level and land above it. The views and the
  PNG maps use the same rule.
  - Outside depressions the elevation's sign decides, because the drainage mask is from before
    the last step's deposition and wave erosion. Otherwise coastal cells that crossed sea level
    in that step would be drawn wrong.
- **3D view**: ocean at the sea surface, lakes at their water level (floor + depth), land at its
  ground; dry depressions below sea level dip below the sea surface.
- **Topographic colours**:
  - lakes in turquoise, by depth;
  - land below sea level in a darker lowland green;
  - rivers only on land;
  - permanent lake ice drawn as water ice.
  - The legend has new "Lake" and "Land below sea level" swatches.
- **Coastlines** (unlit modes, sediment mode) are drawn between ocean, lakes and land, so lake
  shores are outlined too.
- **Realistic mode**: lakes of at least 1 m are water, and dry depressions are land.
  - Lakes lose the land relief, and their permanent ice is water ice.
  - Small lakes are protected against the coast noise.
- **Köppen and aridity modes** (land classifications that colour the ocean as water): lakes are
  water too ("Ocean and lakes"); dry depressions get their class or index.
  - `koppen::classes` gives lakes no class, so the cell widget, the mode's legend and the
    Köppen shares in the snapshot log agree.
- **Temperature, ocean currents, moisture statistics, the aridity index and the cell widget's
  land rows** now take land and ocean from the same rule. Other field modes keep their data
  colours on lakes.
- **PNG maps**: the elevation map draws lakes and dry depressions like the 3D view. The
  coastlines of the temperature map and the ocean of the precipitation map follow the ocean
  mask.
- **Frame history**: no change needed. The drainage bits (ocean, depression, lake) and the
  lake depth are already kept, and a round-trip test now checks that past frames give the same
  ocean, lake and land.

## Checks

- **State hash identical**: seed 0, 5 subdivisions, `--web-math`, 100 Myr, all three
  snapshots. The logs are identical apart from timings.
- **Tests**: 450 pass, 5 of them new:
  - `Water::of` and `surface_color` per water body;
  - vertex heights (lake at its water level, dry depression below radius 1, ocean at 1);
  - shore categories;
  - realistic layers of a dry depression and a lake;
  - Köppen colours of a dry depression and a lake.
  - In addition, existing tests got new assertions: the cell widget's rule and sign edge
    cases, the planet-side vs viewer-side agreement, and the codec round trip.
- **Visual check**: seed 1, 5 subdivisions, 80 Myr. This frame has 21 lake cells and 5 dry
  depression cells below sea level, among them a polar lake ~1 km deep whose floor is at −230 m.
  - [desktop before](desktop-before.png) vs [after](desktop-after.png), same camera. Before,
    the polar lake was drawn as shallow sea and the lake cells as river blue. After, the lakes
    are turquoise at their water level, and the cell widget says "lake, 41.7 m deep".
  - [dry depression at −3 m](desktop-after-dry.png): drawn as dark lowland green, and the
    widget says "land, dry depression".
  - [realistic mode](realistic-after.png).
  - Web (headless Chromium, SwiftShader): [live frame](web-after.png) and
    [past frame 77 Myr](web-after-past-frame.png); lakes look the same in past frames.
  - [phone 390×844](phone-after.png): the legend with its new swatches fits the width.
  - PNG elevation map, cropped around the lakes: [before](map-before.png) and
    [after](map-after.png).

## Caveats

- Grid-scale edge cases of the sign rule affect a cell at most, and rarely:
  - A rim cell less than 1 m below its depression's spill level carries no depression flag. If
    it is also just below sea level, it is drawn as ocean.
  - A depression that the last step opened to the sea is drawn as land or lake until the next
    step.
- A lake next to the ocean is told apart only by colour and the coastline overlay. In the
  realistic mode both are water, with no shore between them.
- Lake ice is shown with the "Sea ice" swatch.
