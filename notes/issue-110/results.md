# Seismically and volcanically active regions (#110): results

Method: [plan.md](plan.md) (with the review response at its end; review in
[plan-review.md](plan-review.md)). Code: `src/tectonics/activity.rs` (the estimate),
`src/viewer/modes/tectonic_activity.rs` (the render mode).

## What is estimated

- **Earthquakes:** the long-term seismic moment rate per area (N·m yr⁻¹ km⁻²). At plate
  boundaries from the relative motion of the material on either side (plate rotations plus
  #182's deformation), split into convergence/opening and strike-slip, each with Bird & Kagan's
  (2004) coupled seismogenic thickness and fault dip of its class (subduction 18 km at 14°,
  collision 18 km, oceanic convergence 3.8 km, continental transforms 8.6 km, oceanic transforms
  13/1.8/1.6 km by speed, ridges 0.13 and 0.40 km, rifts 3 km). Inside plates from #182's strain
  rate with the coupling of its style (shortening, stretching, shear, from the area change).
- **Volcanism:** the long-term magma supply per area (mm/yr as a layer; intrusions included):
  ridges by spreading rate × 7 km, arcs as the model grows them (`arc_crust_per_subduction` per
  subducted length, 1–2 rings behind the trench), hotspots above every plume by its flux and the
  lithosphere's lid (#106's constants), continental rifts at 10 % of a ridge.
- Each cell also names the dominant source of both.

## Numbers (seed 0, 6 subdivisions, 300 Myr, default parameters)

| Quantity | Model | Earth (orientation) |
| --- | --- | --- |
| Seismic moment rate | 2.8 × 10²² N·m/yr | ~(0.7–1) × 10²² (Kagan 2002) |
| of it subduction | 90 % (2.6 × 10²²) | ~90 % |
| distributed deformation | 7 % (2.0 × 10²¹) | a few % (Asia's diffuse belts) |
| collision / other convergence / strike-slip / ridges / rifts | 0.9 / 0.8 / 0.6 / 0.3 / 0.04 % | ridges ≲ 1 % |
| Magma supply | 52 km³/yr | ~25–30 (Crisp 1984; White et al. 2006) |
| ridges | 47.5 km³/yr | ~20 |
| arcs | 3.3 km³/yr | 3–8 |
| hotspots | 1.3 km³/yr | 1–2.5 (intraplate) |
| rifts | 0.05 km³/yr | small |

The ridge estimate (47.5 km³/yr) matches the model's own production of new sea floor over the
last 10 Myr of the run (53 km³/yr of 7 km crust, from the mass ledger), so the estimate is
faithful to the model. The model turns over ~2× Earth's sea floor at that time: it makes
~6.8 km²/yr at its ridges and, by the subduction moment (2.6 × 10²² N·m/yr at 18 km coupling and
14° dip), consumes about as much at its trenches (Earth ~3.4 km²/yr). Its mean plate speed is
Earth-like (3.4 cm/yr), so the cause is more active boundary length (ridges and trenches) rather
than faster plates; the global moment rate and magma supply follow from that turnover. Applied to
Earth's rates the method itself gives ~1.2 × 10²² N·m/yr, at the top of the observed range (the
megathrust coupling of Bird & Kagan includes great earthquakes). A property of the tectonics, not
of this view. At 5 subdivisions (60 Myr, the first check) the totals were 2.0 × 10²² N·m/yr and
36 km³/yr.

Cells: at 300 Myr, 22 % of the cells have any seismicity; the strongest 1 % exceed
1.4 × 10¹⁵ N·m yr⁻¹ km⁻² (subduction; ≈ Mw 8 per century in 100 × 100 km), the median active cell
is ~10¹² (ridges, deforming continents).

State hashes: identical to the base branch (seed 3, 5 subdivisions, 40 Myr: `348fb443…`,
`dbd8f15f…` on both); the estimate only reads the state.

## How it looks

Desktop (5 subdivisions, seed 3, 150 Myr):
- [Earthquakes with the cell widget](desktop-earthquakes.png): subduction zones dark red,
  collision and deforming belts red, ridges and transforms orange, quiet plate interiors grey.
  The pattern is Earth's at plate boundaries: a "Ring of Fire" of the trenches around the closing
  ocean, belts of distributed seismicity through deforming continental crust (as the
  Alpine–Himalayan belt), and lines of weak seismicity along the ridges. Unlike Earth, much of
  the continental area shows moderate seismicity, especially at the web's 4 subdivisions (see
  the caveats).
- [Volcanism](desktop-volcanism.png): ridges brightest (most magma, but almost all of it at
  depth and under water), arcs as narrower bands behind the trenches, hotspots as round spots.
- [Both, with the planet panel](desktop-both.png): arc volcanoes 1–2 cells behind the
  trenches on the overriding plate, hotspot and rift markers; the planet panel's Plates group
  shows the global moment rate and magma supply with sparklines.

Phone (web, 390 × 844, 4 subdivisions, light theme):
- [Earthquakes with the legend](phone-earthquakes-legend.png), [Both](phone-both.png),
  [cell widget](phone-cell.png). The switcher's three choices fit one and a half rows; the
  cell widget's activity rows fit the phone width (the magnitude got a row of its own,
  "Century quake", so that no row is cut off). The mode adds no new kind of widget: its options
  are the switcher's choice buttons and switch, which get touch-sized targets on coarse pointers
  like every other mode's (the screenshots are from a fine-pointer browser).

## Caveats (also on the PR)

- Grid (#64): boundaries are one cell wide on each side, so the moment and magma of a boundary
  are spread over ~2 × 110 km (4 subdivisions on the web: 2 × 440 km); real megathrusts are
  ~100–200 km wide down dip, ridges and arcs ~10–50 km. Densities per area are therefore lower
  and wider than Earth's, totals are right. Zig-zag boundaries count ~0–15 % more cells per
  length. A cell at a triple junction keeps only the motion of the boundary that decides its
  class.
- Oceanic transforms: Bird & Kagan's coupling drops 7× at 4 cm/yr; the map shows that step.
- Distributed deformation: much of the continental area shows moderate seismicity
  (10¹²–10¹³ N·m yr⁻¹ km⁻², Tibet-like), most at the web's 4 subdivisions, because #182's sheet
  deforms it at 10⁻¹⁶–10⁻¹⁵ s⁻¹. The view shows that faithfully; whether the sheet's background
  strain is too high is a question for the sheet, not for this view. Strain rates below
  10⁻¹⁷ s⁻¹ are left out (stable interiors; no intraplate seismicity is estimated).
- "Century quake" (one earthquake that would release a century of moment in 100 × 100 km) is an
  intuition aid, not a forecast; with coarser cells the boundary moment spreads wider and it
  reads ~0.4 lower at 4 subdivisions.
- Arc magma also reaches the trench cells themselves: the arc profile skips the trench cell of
  each contact (ring 0), but its neighbours along the trench are one ring away. The model grows
  its arc crust the same way (the view copies its spreading), so about a third of the arc's
  magma sits on the trench line ([cell widget](desktop-earthquakes.png): "arc, 0.04 mm/yr" at an
  overriding trench cell). Sub-grid (#64).
- Oblique trenches: their strike-slip part uses the transform coupling of the crust (13 km on
  slow oceanic plates), as Bird & Kagan split oblique motion. In "Both", a hotspot whose cells a
  ridge dominates gets no marker.
- No intermediate and deep (intraslab) earthquakes; no back-arc spreading; no arcs at diffuse
  convergent zones (the model only grows arc crust there for overlaps the velocity solve did
  not see converging).
- Hotspots use #106's plume-tail constants; once #106 is in, the plume term can read its tail
  volumes and a "large igneous province" source can show recent LIPs fading over a few Myr
  (one more code in the 4-bit source field, no frame change).
- Magma supply includes intrusions; only ~10–20 % erupts, and at ridges almost all of it under
  water, so the volcanism scale is not a map of volcanoes; "Both" therefore marks arcs,
  hotspots and rifts, not ridges.

## Independent evaluation

[evaluation.md](evaluation.md): no must-fix items. Its "should" items are addressed above (the
cause of the high moment rate, the continental seismicity, the touch sizes) and by a fresh
desktop screenshot of the cell widget (a subduction cell: 3.4·10¹⁴ N·m/yr/km², Mw 7.6 per
100 × 100 km).

## Frames

`CellInfo` 152 → 164 bytes (live updates); frame history 58 → 61 bytes per cell before lz4
(two 1-byte logarithmic channels and the sources), codec version 15; the global record +8 bytes
per frame.
