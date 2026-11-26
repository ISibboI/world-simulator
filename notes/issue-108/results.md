# Issue #108: plate-motion arrows in plate colours

## Change
- `Plate motion` draws every arrow in the colour of the plate that owns the arrow's cell. The
  colour comes from the same function as the cells of the `Plates` mode
  (`plates::plate_color`: plate index → plate id → `PlateColors`), so a plate keeps one colour
  for its life in both modes, in live and in history frames.
- The surface underneath stays topographic: plate colours there would hide the arrows of the
  same colour. The arrows keep their black border, which keeps the pastel plate colours legible
  over ocean blue, land green/brown and white ice (see screenshots).
- The `Plates` mode itself is unchanged: its plate-motion arrows stay yellow, since they sit on
  plate-coloured cells.
- Legend: the description says arrows take their plate's colour; a "Plates" palette swatch as in
  the `Plates` mode; the example arrows are neutral light grey instead of yellow.
- Unit test `arrows_have_the_colours_of_their_plates_in_the_plates_mode`: every arrow's colour
  equals its cell's colour in the `Plates` mode (exactly on continental cells, which that mode
  does not darken), several colours occur and none is the old default yellow.

## Screenshots (seed 0, 4 subdivisions)
The runs are different instances (times differ), so the plates are not the same in before/after.
- [before-desktop.png](before-desktop.png): before, all arrows yellow.
- [after-desktop.png](after-desktop.png): after, desktop.
- [after-desktop-plates-mode.png](after-desktop-plates-mode.png): the same frame in the `Plates`
  mode, same colours.
- [after-web-1280.png](after-web-1280.png): web, 1280×800.
- [after-web-390.png](after-web-390.png), [after-web-390-legend.png](after-web-390-legend.png):
  web, 390 px wide with touch input, legend open.
