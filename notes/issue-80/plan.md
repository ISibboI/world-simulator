# #80: status banner while the simulation starts or restarts

## What it does

`src/viewer/banner.rs` draws a banner across the full width of the view: white at 90 % opacity,
black text in egui's heading style (1.44 × body text, so it follows the browser's root font size
on the web like the rest of the UI), padded by 0.3 em above and below, so it is one text line
high. Long text (errors, narrow phones) wraps and the banner grows with it, up to 4 rows (then
"…"; the panel shows a failure in full); nothing overflows.

It shows, from the first paint of the page (web) or the window (desktop) until the first surface
update of the run:

| phase | text | how it is detected |
| --- | --- | --- |
| web only: page loaded, WebAssembly module loading | Loading the simulator… | HTML banner in `web/index.html` (before any Rust code runs) |
| view exists, simulation thread / Web Worker starting | Starting the simulation… | view created, no message from the simulation yet |
| initial planet being generated (mesh, plates, climate spin-up) | Generating the initial planet… | the simulation sends `SimulationEvent::Generating` (the worker posts `{ generating: true }`) before it builds the planet |
| Restart pressed | Restarting the simulation… | restart, no message from the new simulation yet |
| new planet being generated after a restart | Restarting: generating a new planet… | as above |
| the simulation failed (also a failed worker start on the web) | Simulation failed: <message> | `SimulationEvent::Failed` |

and disappears with the first surface update (t = 0, sent right after the planet is built). A
failure stays until the next restart. Errors before the view exists (bad URL options, no WebGL)
appear in the HTML banner. The decision is the pure function
`banner::simulation_text(restarted, generating, time, failure)` (unit tested).

## Placement

Centred vertically: where the eye goes first, and on start there is no planet yet. During a
restart it covers only a strip through the middle of the previous planet. It is painted into the
layer of the 3D view (the root `Ui`, after the central panel's paint callback), so it lies on
top of the planet but **below every egui window and area**: the parameter panel, its ⚙ button,
the cell widget of #72 and its button always stay visible and usable above it.

## Reuse (#78)

`banner::show(ui, text)` draws it for one frame; not calling it hides it. A speed hint can call it
(or a variant with another position) with its own text.

## Web page: seamless hand-over

The HTML `#status` element is the banner until the view's first frame: the same position (centred),
height (heading font size 1.44 rem, line height 1.05 = egui's Ubuntu font, padding 0.3 em), and
colours (white at 230/255 over the same background). The view hides it at the end of its first
frame (`run_web`'s `on_first_frame`), so the page change and the first canvas frame are shown
together: no frame without a banner. Measured: the two bars lie on the same pixel rows within
1 px (desktop 381–418 vs 380–418, phone 1209–1322 vs 1208–1323 physical px). Only the font differs
(the browser's sans-serif vs egui's Ubuntu Light), and the text changes at that moment anyway.

## Desktop

The window now opens before the initial planet is built: the options are validated first (wrong
options still end the program with an error before any window), then the simulation thread builds
the planet (as it already did on restarts) and runs it. Still one simulation thread; headless runs
are unchanged (`--web-math` is set in `build_planet`, now on the simulation thread, before anything
is computed; the view does not use `crate::math`).

## Panel

The panel keeps its status line (time, "Starting/Restarting the simulation…", or the failure in
red). It is the slot where the time is shown while running, and removing the starting/failed text
there would make the panel jump; the failure next to the parameters helps fixing them.

On narrow screens (< 600 pt, phones) the open panel covers nearly the whole view, so it **closes
when Restart is pressed** (the ⚙ button comes back) and the banner is seen (review finding).
