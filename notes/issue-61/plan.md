# Issue #61: parameter panel: plan

Goal (issue #61 and Sebastian's comment): an egui panel in the shared viewer to choose the
simulation parameters and restart the simulation with them, on desktop and web. It sits in a
corner and can be hidden; hidden, only a small button to show it again remains. Initial values
come from the CLI (desktop) or the URL query (web), as before.

Later requirements from Sebastian (via the coordinator): the same UI on web and desktop, but on
the web text sized from the browser's root font size, touch-friendly targets on touch screens,
and a responsive panel on phones (hidden by default on narrow screens, never off-screen).

## Design

- **Fields come from clap.** `ParameterPanel` (`src/viewer/parameters.rs`) walks
  `Cli::command().get_arguments()` and makes one field per option: a checkbox for flags
  (`SetTrue`), a drop-down for value enums (possible values), a multi-line box for the
  repeatable `--set`, a text field otherwise. Name, help text (tooltip) and default come from
  clap. Initial values are read from the `ArgMatches` of the start-up arguments. A new option
  in `Cli` therefore shows up in the panel without any change.
- **Validation = the CLI's.** The fields become arguments again (`--name=value` only for values
  that differ from the default) and are parsed with `Cli::try_parse_from`, then checked with
  `Cli::validate`: the `ensure!` checks of `build_planet` (moved into `Cli::check`) and the
  parameter setup with presets and `--set` overrides (moved into `Cli::parameters`). This runs
  whenever a field changes; the error is shown in the panel and disables "Restart".
- **Shown options:** all, except `--headless` (and on the web the file options rejected by the
  URL parser: `output`, `width`, `cell-dump`). Main group at the top: seed, subdivisions,
  plates, continental-fraction, initial, duration, dt, biosphere, epoch, no-erosion. All others
  under "More options".
- **Restart.** The viewer owns a `StartSimulation` launcher (`FnMut(Launch, Sender, wake_up)`)
  and calls it at start-up and on every restart, each time with a new channel. The view keeps
  the previous planet until the first surface of the new simulation arrives. The first update
  of a simulation carries the mesh triangles, so the renderer can switch meshes (another
  `--subdivisions`).
  - Desktop: one thread per simulation. The old simulation stops after its current step
    because sending to its dropped channel fails (`simulate`'s callback returns
    `ControlFlow::Break`, no final snapshot). The new thread first joins the old one, so only
    one simulation runs at a time and the UI never blocks.
  - Web: the old worker is terminated, and a new one starts with the new query string. The
    page URL gets the same query (`history.replaceState`), so the setting can be shared as a
    link.
- **Unchanged:** headless runs (the callback always continues), the desktop start without
  using the panel, the URL syntax. `query.rs` holds the query ↔ arguments conversion (now
  pure Rust, unit-tested natively, used by the web version).
- **Device adaptation (`UiOptions`):** on the web, body text at the root font size of the
  page (`getComputedStyle(document.documentElement).fontSize`, scaled from egui's 12.5 pt;
  other text styles scale with it). egui's `pixels_per_point` (devicePixelRatio, browser zoom)
  is untouched. With `(pointer: coarse)`, interact size 44 × 44, larger button padding,
  spacing and checkboxes. The panel starts hidden on screens narrower than 600 points and is
  capped to the screen size (it scrolls). The desktop keeps egui's default style.
