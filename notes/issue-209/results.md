# #209: global statistics in the viewer — results

## What the viewer shows

After Sebastian's review on #218 (the parameter panel is for inputs only), the outputs have their
own panel, **Simulation and planet**, in the bottom right corner above the time bar:

- the run's time and pace (`t = X of Y Myr · s per step`), or why it failed, or what it does
  before its first state, and the step-time hint (all moved out of the parameter panel);
- #217's ramp-up line (moved out of the parameter panel);
- the global statistics below.

It is open by default on wide screens and hidden on phones (< 600 pt), where a button
"Planet · 30 Myr (ramp-up)" remains (with the step-time hint above it), so nothing is lost.
On a phone the open panel sits above the cell widget's button, and the sparklines get rows of
their own when the panel is narrower than 25 × the body text size. The parameter panel holds
only the options of the simulation and the restart buttons; it keeps clear of the new panel.
The time bar is unchanged.

Every value belongs to the state on screen: live, or the past frame the time bar shows.

| Group | Rows |
| --- | --- |
| Time | epoch (Gyr after the earliest start), star luminosity (% of today's Sun), solar constant the climate uses (W/m²), day 24 h (fixed) |
| Climate | mean temperature (sparkline), equator / poles, CO₂ (ppm, or bar when high, × PAL; sparkline), greenhouse forcing (W/m²), ice cover, land precipitation, water lost to space (m global layer) and O₂ (bar) of the escape (#212; — when it is off) |
| Surface | land, continental crust, sea level since start (sparkline), lakes (area, volume) |
| Plates | plates (and those > 1 % of the surface), mean surface speed (cm/yr) |

Every row explains itself on hover. Sparklines run over all recorded frames (thinned to ≤ 160
points) with a dot at the displayed frame; hover shows their range.

Not shown, because the model does not have them yet (#208): radiogenic heat production, mantle
temperature and heat flow, a day length that changes (the climate uses a fixed rotation).

## Data path and frame size

- `viewer::GlobalStatus` (`src/viewer/global.rs`): 17 `f32`, 2 `u16`, 1 flag byte = **73 bytes**,
  computed with every state from the planet and the cells/plates the view gets anyway (one pass
  over the cells; read only).
- Desktop: in `SurfaceUpdate`. Web: one `global` `Uint8Array` in the worker's message.
- History: uncompressed in the frame header (codec version **14**; header 18 → 91 bytes), so
  `History` keeps it per frame in memory without decoding (+73 bytes per frame; frames are tens
  of kB at 5–6 subdivisions).

## Checks

- State hashes unchanged: seed 3, 4 subdivisions, 30 Myr, `--headless`, this branch vs. the base
  (760ca4b): identical hashes at both snapshots.
- Local CI with `CARGO_BUILD_WARNINGS=deny`: fmt, clippy (desktop and wasm32, all targets), doc,
  tests; `trunk build --release`.
- Screenshots (virtual screen):
  - [desktop-past-frame.png](desktop-past-frame.png): the view at 42 Myr while the run is at
    60 Myr — the panel shows the 42-Myr values, the sparkline dots sit at 42 Myr;
  - [desktop-live.png](desktop-live.png): live at 60 Myr;
  - [web-phone-collapsed.png](web-phone-collapsed.png): web at 390×844, both panels hidden;
  - [web-phone-planet.png](web-phone-planet.png): the same with the planet panel open; the
    sparklines sit under their values so that the panel fits;
  - [web-phone-parameters.png](web-phone-parameters.png): the parameter panel, inputs only.

## Notes on the values

- Early runs (epoch 0, 4.0 Ga) show CO₂ of about 0.5 bar (1760× PAL) and +62 W/m² forcing:
  the carbon cycle's start for a faint young Sun (#196), held during the ramp-up phase.
- The sea level "since start" rises by a few hundred metres early in a run at 4 subdivisions;
  it is the tectonic/sediment sea level of the model, measured against the land.
- Lakes come from the last erosion step, one step behind the tectonics.
