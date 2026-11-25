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
