# Issue #161: time text overlapped the Live button on phones

On narrow screens the time bar put the time text right-aligned beside the buttons. With touch-sized
buttons (4 × 48 points) a 320–412-point phone has too little room for a past-frame text such as
"163 Myr · live 190", so the text was drawn over the Live button.

Fix (`src/viewer/timeline.rs`, `rows`): the bar measures the buttons and the text and picks a layout:

- one row (buttons, slider, time) on wide screens when a slider of at least 120 points still fits;
- two rows (buttons + time, then slider) when the time fits beside the buttons;
- three rows (buttons, then time, then slider) otherwise.

Screenshots (Chromium with touch emulation, web build, `subdivisions=4`, viewing a past frame
while the simulation runs on):

- `before-after-narrow.png`: before at 320 px (overlap), then after at 320, 360, 390, 412 px.
- `before-412.png`: before at 412 px (text touching the Live button).
- `after-desktop.png`: 1280 px, unchanged one-row layout.

## Collapsible time bar (PR #164 review)

The bar has a collapse button (chevron, first button). Collapsed, it is a flat strip in the bottom
left corner with only the expand chevron, pause/resume and the time (and a ⚠ with the message as
tooltip if a frame could not be read). Keys (space, ←, →, End) still work. Like the parameter
panel and the cell widget it starts collapsed on screens narrower than 600 points (phones) and
expanded otherwise, and keeps the user's choice for the session (not across reloads).

Height: on touch screens the strip is one 44-point button row plus a 3-point margin (≈ 50 CSS px,
vs ≈ 140 for the expanded three-row bar on a 390-px phone); on desktop ≈ 30 px, vs ≈ 42 px for
the one-row bar — and only as wide as its contents.

- `collapse-phone.png`: 390 × 844 touch emulation, collapsed (default) and expanded.
- `collapse-desktop.png`: 1280 px desktop, the bottom of the window expanded (top) and collapsed.
