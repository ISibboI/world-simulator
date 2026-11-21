# #80: status banner while the simulation starts or restarts

## What it does

`src/viewer/banner.rs` draws a banner across the full width of the view: white at 90 % opacity,
black text in egui's heading style (1.44 × body text, so it follows the browser's root font size
on the web like the rest of the UI), padded by 0.3 em above and below, so it is one text line
high. Long text (errors, narrow phones) wraps and the banner grows with it, up to 4 rows (then
"…"; the panel shows a failure in full); nothing overflows.

It shows

| state | text |
| --- | --- |
| view opened, no surface update yet | Starting the simulation… |
| Restart pressed, no update of the new run yet | Restarting the simulation… |
| the simulation failed (also a failed worker start on the web) | Simulation failed: <message> |

and disappears with the first surface update of the run. A failure stays until the next restart.
The decision is the pure function `banner::simulation_text(restarted, time, failure)` (unit
tested); `Viewer` passes `runs > 1` (number of started simulations), the time of the latest
update of the current run and its failure.

## Placement

Centred vertically: where the eye goes first, and on start there is no planet yet. During a
restart it covers only a strip through the middle of the previous planet. It is painted into the
layer of the 3D view (the root `Ui`, after the central panel's paint callback), so it lies on
top of the planet but **below every egui window and area**: the parameter panel, its ⚙ button,
the cell widget of #72 and its button always stay visible and usable above it.

## Reuse (#78)

`banner::show(ui, text)` draws it for one frame; not calling it hides it. A speed hint can call it
(or a variant with another position) with its own text.

## Web page

The HTML `#status` element (shown while the WebAssembly module loads, and for errors before the
view exists) is styled like the banner, so "Loading…" turns into "Starting the simulation…" in
the same place. It is hidden as soon as the view starts its first simulation; it no longer shows
the start text or simulation failures (the banner does).

## Panel

The panel keeps its status line (time, "Starting/Restarting the simulation…", or the failure in
red). It is the slot where the time is shown while running, and removing the starting/failed text
there would make the panel jump; the failure next to the parameters helps fixing them.

On narrow screens (< 600 pt, phones) the open panel covers nearly the whole view, so it **closes
when Restart is pressed** (the ⚙ button comes back) and the banner is seen (review finding).
