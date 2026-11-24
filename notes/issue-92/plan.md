# #92: photorealistic render mode — plan

Status (2026-11-24): Sebastian decided (§0); step 1 (the sun and the light options) is
implemented, see [sun.md](sun.md). Revised after an independent review (§12).

## 0. Sebastian's decisions (comment 7131174342, 2026-11-24) and the order of work

1. **Light:** the sun fixed in space by default, and a camera light as an option. The sun mimics
   the real sun including its evolution: for the simulated time frame the younger, fainter sun of
   the early planet with plates and oceans; later the time frame will be freely choosable, so the
   sun must represent any state from that early sun through today to a reasonable time into the
   future before the red-giant phase.
2. **Merges:** accept the one-time re-roll of detail (stage 6 not needed for now).
3. **Vegetation** from the continuous climate fields: yes.
4. **Season option:** yes.
5. **Zoom:** none for now, to limit the render detail.
6. **Relief:** realistic, no mountain inflation in this mode (instead of the global 40×).
7. **Clouds:** later, as an optional decoration.
8. **Rivers:** none, since they are invisible from space.
9. **Default detail levels:** decided later.

Also: render detail is generated on the fly, more detail for cells closer to the camera
(level of detail, LOD: view-dependent, for planets usually a chunked quadtree LOD such as CDLOD).
Stars only if they make sense (a short exposure of a sunlit planet shows none).

**Order of work:** each step is a separate PR that waits for Sebastian's merge before the next
starts.
1. **Sun and light options** (done, [sun.md](sun.md)): `src/sun.rs` (luminosity, temperature,
   radius, colour, apparent size and direction of the sun at any age from the zero-age main
   sequence to 10 Gyr); a view-wide Light option Camera | Sun | Sun + ambient for the shaded
   modes; the sun drawn in the background. No frame-format change.
2. **Stars and the realistic mode** (done, [stars.md](stars.md)): a procedural star field
   whose visibility follows a camera's exposure. A new render mode **Realistic**, first in the
   list (topographic surface as a placeholder), now holds the Light option (default Sun), the
   sun and the stars; the data modes are lit from the camera again (Sebastian on #148).
3. **Realistic surface, stages 0 + 1** ([surface.md](surface.md)): per-cell materials from the
   climate and crust, sun-lit in linear light, relief true to scale, sun glint on water, Season
   option, naga test for WebGL2. Stage 0's plate orientation moves to stage 2 (no use without
   noise); biosphere from the launch options; no frame change.
4. **Stage 2, sub-cell detail** ([detail.md](detail.md)): plate orientation in the frames
   (+16 B per plate, codec 11), de-indexed mesh, plate-anchored noise weighted to zero at cell
   centres (coasts, snow and tree lines, colour variation, bump relief), octaves limited by the
   pixel footprint (on-the-fly LOD), Detail option Low/Medium/High. Topographic stays the
   start-up mode (Sebastian on #157).
5. Then stage 3 onwards of §10 (with stage 0's plate frames), adapted to the decisions: no
   relief exaggeration in the realistic mode, LOD for the detail, no rivers, Season option,
   clouds later, stars only if they make sense.

Goal (from the issue): a render mode that shows the planet in space, lit by the sun (which is
drawn too), with procedurally generated detail inside each cell (relief, rock/soil/vegetation/ice
colours, coastlines, optionally clouds) that is deterministic, consistent with the cell data and
temporally stable (anchored to the crust, not to the fixed mesh), with a terminator, sun glint on
water, cheap limb scattering and optional stars, on desktop and web within the web budget.

Project goal applies: an Earth-*like* planet, judged by plausibility, not by matching Earth's
look. Inaccuracies from the coarse grid (~110 km cells, ~220 km on the web) are accepted; where
the mode makes them visible (e.g. one-cell-wide plate seams, see below), they are explained in
the legend text or the notes, not fixed.

## 1. What exists (main at 0f0a6fa, plus the open mode PRs)

**Renderer** (`src/viewer/renderer.rs`, `planet.wgsl`): one indexed mesh whose vertices are the
cells (icosphere vertices; 41k at 6 subdivisions, 10k at 5 on the web), triangles between
neighbouring cell centres. `Vertex = {position, normal, color}` (40 bytes), built on the CPU by
`surface_vertices` (land raised by `elevation × exaggeration`, default 40×; ocean at radius 1;
topographic colours). The fragment shader does a Lambert term with 30 % ambient, or nothing for
unlit modes. Light comes from the upper left *of the camera*, so the visible side is always lit;
there is no sun, no space background other than the panel fill (5, 5, 15), MSAA 4× on the
desktop only (eframe's web painter does not multisample). Drawn inside
egui's render pass via a paint callback (`prepare` uploads only what changed, `paint` draws planet,
overlay triangles, outline lines). Colours are written as sRGB values without conversion.
Camera: orbit (yaw/pitch) at a fixed distance of 4.5 radii, FOV 0.6 rad; **no zoom**. At typical
window sizes a cell is ~5–10 px wide (desktop, 6 subdivisions), ~10–15 px on the web.

**Modes** (`src/viewer/modes/`, #91): `RenderMode` gives per-cell colours on the CPU
(`colors(&ModeData)`), an overlay, `Shading::{Lit, Unlit}`, options (`Choice`/`Switch`, shown by
the switcher above the legend, touch-sized per #91) and a legend. Every mode shares the one
pipeline and vertex layout.

**Per-cell data** (`CellInfo`, 120 bytes live; frame history 48 bytes/cell quantised): elevation,
crust thickness, lithosphere age, sediment, orogenic share, time since orogeny, temperature
(annual/January/July), precipitation (annual/Jan/Jul), potential and actual evaporation, ice share,
river discharge, ocean currents, gyre streamfunction/heating, winds (annual/Jan/Jul), plate index,
flags (continental, oceanic, boundary class, deforming band). Plus per plate (`PlateInfo`, 16 B):
angular velocity and persistent id.

**Open mode PRs** (unmerged; this work stacks on #147):
- #142 crust (#99): flags widened to 16 bits in the history (felsic and metasediment bits) —
  rock colour can use them.
- #143 sediment (#100): no frame change.
- #145 lithosphere strength (#101): two share channels.
- #146 hydrology (#102): `lake_depth` and `drainage` (receiver slot, ocean/depression/lake bits)
  per cell — gives lakes and river directions for sub-cell water.
- #147 Köppen (#103): `climate_class` (30 classes) per cell, `src/koppen.rs`.
All of them touch `CellInfo`, the codec channel list and `VERSION`, and the mode list; they will
conflict with each other in those lines, so whatever lands later rebases.

**Anchoring.** The simulation is rigid-plate (`src/tectonics/mod.rs`): each plate stores its
crust in a plate-local frame indexed by mesh vertices; `Plate::rotation` (a `DQuat`, local →
world) is integrated every substep. A world cell maps to a local sample via
`Ownership {plate, local}`. So the crust frame of every plate is already known exactly — it is
just not sent to the view: `PlateInfo` has the angular velocity but not the orientation, and
neither the frame history nor `CellInfo` has any persistent sample id.
- *Splits* (`split_plate_inner`) give the new plate the parent's rotation: the local frame is
  continuous, no change in anchoring.
- *Merges* (`merge_plates_inner`: sutures, captures of tiny plates) move the absorbed crust into
  the frame of the receiving plate by position: the crust stays where it is, but its local
  coordinates change once.
- Cells change plate at convergent/divergent boundaries (overriding, new crust): real changes of
  the surface, so a change of detail there is correct.
- With the default `dt` of 1 Myr and plate speeds of 2–10 cm/yr, crust moves 20–100 km (¼ to 1
  cell) between frames. The per-cell data is resampled to the fixed mesh (nearest sample), so the
  coarse fields move in steps of a cell, while detail anchored to the plate frame moves smoothly.

## 2. Architecture

### Where it plugs in
- New mode `RenderModeId::Realistic` (`--render-mode realistic`, label "Realistic"), file
  `src/viewer/modes/realistic.rs` (CPU part) plus `src/viewer/realistic.wgsl` and
  `src/viewer/sky.wgsl` (GPU part).
- `RenderMode` gets one more optional method, e.g. `fn surface(&self, data) -> Option<Surface
  Style>` (default `None` = the existing pipeline with `colors()`), where the realistic mode
  returns its per-vertex material buffer and its lighting setup (sun direction, quality). The
  renderer keeps a second pipeline set for it; all other modes are unaffected. Overlays and the
  cell outline keep working on top (same depth buffer, same vertex positions).
- Vertex positions stay those of `surface_vertices` (same exaggeration), so picking, the cell
  widget, outlines and the time bar work unchanged. No month slider; the mode reads the existing
  annual/January/July fields through its own option (§6).

### CPU vs GPU split
- **Mesh:** the realistic pipeline draws a **de-indexed** copy of the mesh (3 own vertices per
  triangle: 246k vertices at 6 subdivisions, 61k at 5), built once per mesh. Each vertex carries
  its barycentric corner and, as three-component attributes, the plate indices and the values of
  all three corners of its triangle where a pixel must know them per corner (plate, land/sea
  class). Indexed sharing cannot give barycentrics (icosphere vertices have degree 5 and 6).
- **CPU (Rust, per displayed frame, unit-testable, deterministic):** for every cell, a compact
  material record, expanded to the de-indexed vertices (≈40 bytes/vertex, ~10 MB at 6
  subdivisions, ~2.5 MB at 5 on the web):
  - the cell's plate index (orientations of all plates in a small uniform array, 16 B each);
  - base albedo of the dry surface (rock/soil mix), vegetation cover and its tint, snow/ice
    cover, water depth (ocean or lake, signed elevation), relief roughness amplitude and style
    (rugged young orogen / old eroded / plain / ocean floor), wind speed (water roughness).
  Mapping functions from `CellInfo` (§4) live in Rust, so they are tested without a GPU and the
  shader only interpolates and adds detail. Cost: one pass over the cells plus the expansion, a
  few ms; rebuilt only when the frame or an option changes (like `colors()` today).
- **Data the view does not have yet:** the biosphere preset (`--biosphere`, for bare land
  without life) and the orbit (`OrbitParams`: obliquity, for the sun's declination) reach only
  the simulation; they must be plumbed into the view (`Launch`/`ModeData`), on the web too.
- **Uniforms:** today `{view_projection, light_direction, viewport}`. The realistic pipeline
  needs the eye position, the inverse view-projection (sky rays), the sun direction and
  colour, quality and the plate orientations; it gets its own bind group, so the overlay and
  outline pipelines keep their layout.
- **GPU (WGSL, per pixel):** plate-anchored 3D gradient noise (fBm, ridged for mountains) for
  colour variation, sub-cell coastlines/snowlines/treelines and bump normals; lighting (sun,
  terminator, ocean Fresnel + glint, night side); limb haze; background stars and sun.
- Nothing per frame on the CPU while only the camera moves; the view is static between
  simulation updates (egui repaints on interaction only), so the GPU cost is paid only while
  dragging or when a frame arrives.

### Shader constraints for WebGL2 and WebGPU
The web view runs on WebGL2 (via wgpu's GL backend, `downlevel_webgl2_defaults`) or WebGPU.
Design for the WebGL2 subset:
- No storage buffers, no compute: all per-cell data goes through **vertex attributes** (≤ 16
  attributes, stride ≤ 255 B; the tighter WebGL2 limit is 15 inter-stage variables; we need
  ~8–10) and plate orientations through a uniform array (16 KiB limit: hundreds of plates). Optional data
  textures read with `textureLoad` (works on WebGL2) only if needed later (clouds).
- Fragment derivatives (`dpdx`/`fwidth`) are available everywhere: used to clamp noise octaves
  to the pixel footprint (anti-aliasing and cost control). WGSL requires them in uniform
  control flow, so the footprint is computed at the top of `fs_main`, before any branch.
- No MSAA on the web: the octave clamp and analytic anti-aliasing of the limb, sun disc, stars,
  coastlines and glint carry all the anti-aliasing there.
- f32 everywhere (WebGL2 highp); noise frequencies up to ~1000 cycles per radius stay well within
  f32 precision on the unit sphere.
- Single render pass inside egui's callback: no offscreen targets or post-processing in the MVP
  (sun glow and atmosphere are analytic, no bloom pass). Light in linear space, tone-map, then
  encode to sRGB in the shader (the target format is checked once at start-up; today's shaders
  write sRGB values directly).
- A unit test validates the WGSL with naga (dev-dependency `naga = "=30.0.2"` with `wgsl-in`
  and `glsl-out`, wgpu's version) under downlevel capabilities and writes GLSL ES 3.00
  (`Version::Embedded { version: 300, is_webgl: true }`), so WebGL2 breakage shows in `cargo test`, not only in the browser.

### Draw order in the callback
1. Planet: realistic pipeline (depth write); includes the on-disc atmospheric haze, computed
   in linear space in this shader (blending a shell over the planet would blend in gamma space).
2. Sky after the planet (so it only shades uncovered pixels, which matters on phones): a
   full-screen triangle at the far plane (depth 1 in the DirectX-style projection, compare
   `LessEqual`, no depth write): stars (hash of the view direction on a fixed world-frame grid,
   deterministic), the sun disc and an analytic glow when it is in view.
3. Atmosphere shell, only for the off-disc limb ring: a sphere slightly above the highest
   vertex, additive blend, depth-tested against the planet (limb glow, backlit ring when the
   sun is behind the planet).
4. Existing overlay and outline pipelines, unchanged.

## 3. Temporal stability: anchoring the detail

**Recommendation: anchor to the plate frame.** Add the plate orientation (`Plate::rotation`, a
quaternion local → world, as f32) to `PlateInfo` as a new field `orientation` (the existing
`PlateInfo::rotation` is the angular velocity; rename it to `angular_velocity` on the way). The
shader takes the orientations as a uniform array; per pixel it picks the plate of the triangle
corner with the largest barycentric (i.e. of the nearest cell centre) and evaluates all noise at
`anchor = R_p⁻¹ · normalize(world position)`. Inside a plate the detail moves rigidly with the
crust from frame to frame and does not depend on the fixed mesh. (Interpolating a per-vertex
anchor instead was rejected in review: across plate boundaries the anchors of the corners can be
up to 180° apart, which smears the noise into bands tracing the plate outlines.)

- **Frame data cost:** +16 bytes per plate (`PLATE_BYTES` 16 → 32), i.e. a few hundred bytes per
  frame against ~2 MB raw cell planes (~0.02 %); codec `VERSION` bump. Nothing per cell. The live
  update path (threads and the web worker) carries `PlateInfo` as Pod bytes already.
- **Plate boundaries:** the detail switches plate on the Voronoi edge between two cells of
  different plates, a crisp seam where the surface changes anyway (ridges, trenches, orogens).
  If it looks too hard, the noise of the two plates can be blended in a narrow band around the
  edge (two evaluations only there).
- **Merges:** the absorbed plate's crust gets new local coordinates once, so its sub-cell detail
  re-rolls in one frame (statistically the same, coarse colours unchanged). Merges are not rare:
  besides sutures and captures, terrane accretion is a split plus a merge
  (`events.rs`, `Merge::Terrane`), and collision stacking moves crust between plate frames.
  Stage 0 counts these events per 100 Myr in a real run to judge how often a re-roll shows (it
  affects the absorbed piece only). Accept for the MVP; optional fix in stage 6: each crust sample carries a
  display-only "noise frame" tag (its plate of origin plus a constant correction rotation set at
  the merge), +1–2 bytes per cell in frames that almost never change (≈0 after delta + lz4).
- **Grid snapping:** the coarse fields move in one-cell steps (nearest-sample resampling) while
  the detail slides smoothly; between two frames detail and coarse colour may be offset by up to
  half a cell. Accepted grid artefact (anchoring to the sample's own position instead would make
  the detail jitter every frame, which is worse).
- **Determinism:** noise is a pure function of the anchor (integer hash, no time, no random
  state), so a frame always looks the same, live or from the history, on desktop and web up to
  GPU float differences.
- **Sub-cell features never contradict the cell:** noise that moves a threshold (coast, snow
  line, tree line) is multiplied by a weight that is zero at the vertices (e.g. `1 −
  smoothstep(0.6, 1, max barycentric)`, barycentrics from the de-indexed mesh), so every cell
  centre keeps exactly the class its data gives (land/sea, ice/no ice), and the zero-mean noise
  keeps the shares about right.

## 4. Surface appearance from the cell data

All colours are plausible for an Earth-like planet, chosen in linear space with albedos from the
usual ranges (ocean 0.06, forest 0.1–0.15, grassland 0.2, desert 0.3–0.4, fresh snow 0.8–0.9).
The mapping is continuous in the inputs (no hard class edges except where the data has them), so
interpolation across a triangle stays meaningful.

- **Ocean and lakes** (elevation < 0, or a lake from #146): colour by depth (shelf < 200 m
  turquoise-green tinted by sediment and warmth, deep water dark blue), Fresnel reflection of the
  sky colour, sun glint with a GGX-like lobe whose roughness follows the cell's wind speed (calm
  → sharp glint, trade winds → broad). Sea ice from `ice` with a noisy edge.
- **Vegetation** (if the biosphere is `modern`; none for `abiotic`): cover from moisture (aridity
  index P/PET) and warmth (growing-season temperature from the January/July means), tint from
  warmth and dryness (dark tropical green → temperate green → dark boreal green → olive/tan
  grassland). Köppen class (#147) is not used as input (discrete; one-cell-wide soft edges) but
  as a test oracle: e.g. B climates mostly bare, A/C/Df mostly vegetated. When #90 (life) exists,
  its fields replace this proxy.
- **Bare ground:** rock colour from crust (oceanic/basaltic dark grey, felsic/continental
  lighter, metasediment/sediment-rich warmer; flags from #142), soil/sand tint from sediment
  thickness and aridity (red-orange to pale sand in deserts), exposed rock on steep young orogens
  (orogenic share, time since orogeny).
- **Snow and ice:** permanent ice from `ice`; seasonal snow from the chosen season's temperature
  (option, §6) with a noisy snow line that follows the bump relief (more snow on high detail
  peaks).
- **Rivers:** at this scale real rivers are sub-pixel from space. Optional (off by default):
  large rivers (discharge above a threshold) as thin dark lines from cell to receiver (#146's
  drainage slot) with a noisy meander, as overlay geometry.

## 5. Relief, coastlines, lighting

- **Relief detail:** bump mapping (perturbed normal from the analytic gradient of the noise, no
  extra geometry); amplitude and style per vertex: ridged noise for young orogens (amplitude from
  elevation and orogenic share, decaying with time since orogeny), gentle fBm for old shields,
  almost flat for thick sediment plains and the abyssal plain; optional ridge orientation along
  the boundary (later). The coarse mesh normal (exaggerated geometry) stays the base. High
  quality may also displace a refined mesh (static CPU subdivision ×4 per triangle with vertex
  texture fetch), only if the limb silhouette looks too faceted — not planned for the MVP.
- **Water normal:** water uses `normalize(position)` as its base normal, not the mesh normal,
  which in coast triangles is tilted by the raised land vertex (glint would smear along coasts).
- **Coastlines:** land where `interpolated elevation + w · amplitude · noise > 0` (w as in §3),
  amplitude from local relief; gives fractal sub-cell coasts, islands and bays that keep each
  cell centre's land/sea class. Shallow-water band and surf-free soft edge from the same value.
- **Sun:** a directional light from a sun direction in the world frame (the planet does not spin
  in the simulation). Its latitude is the subsolar latitude of the chosen season (0° for the
  year, the declination of mid-January and mid-July, about −21°/+21°, from
  `OrbitParams::position(…).declination`); its longitude is fixed
  so that the default camera sees a three-quarter lit disc. Terminator from the Lambert term
  softened by a narrow twilight band and the atmosphere's reddening; the night side is dark with
  a tiny ambient term so the outline stays visible.
- **Atmosphere (cheap):** analytic single scattering in a thin shell: optical depth from the
  chord length through the shell (closed form for a sphere), Rayleigh-like blue and a
  Mie-like forward lobe, transmittance reddening near the terminator; the same terms give
  aerial-perspective haze on the surface towards the limb. The shell's thickness is exaggerated
  like the relief (true 100 km is 1.5 % of the radius, thinner than the 40× mountains), see
  decision 6.
- **Stars and sun:** a deterministic star field fixed in the world frame (it turns with the
  camera, not with time); the sun as a small bright disc with an analytic glow, visible when the
  camera looks past the planet towards it (with the fixed FOV, mostly when the planet is backlit).
- **Clouds (optional, stage 5):** not simulated, so they are decoration derived from the climate
  fields: cover from precipitation and latitude (ITCZ, storm tracks), stretched along the
  low-level wind; noise in the world frame (clouds do not ride on plates), static between frames
  of the same climate. Drawn in the atmosphere shell pass with shadows skipped. Off by default
  because they hide the surface the simulation produces.

## 6. UI

- One mode in the existing switcher; its options via `RenderMode::options` (rows of touch-sized
  buttons, as #91 built them; nothing new in the UI framework):
  - **Light:** Sun | Camera (Camera = today's light from the upper left of the viewer, for
    looking at the night side without dragging round);
  - **Season:** Year | January | July (sun latitude, snow, vegetation greenness, sea ice);
  - **Detail:** Low | Medium | High (quality levels, §7);
  - **Clouds:** switch, off (stage 5).
- Legend: a short description ("Colours derived from the simulated climate and crust; sub-cell
  detail is procedural") and a few swatches (forest, grassland, desert, rock, ice, shelf, deep
  ocean), not a colour scale.
- Cell widget, hover/selection outline, time bar and camera: unchanged. Phones: no new panels;
  the options sit in the collapsed switcher like the other modes; font follows the browser.
- Zoom does not exist: at the fixed camera distance most sub-cell detail is 2–3 octaves (a cell
  is 5–15 px). A zoom (scroll/pinch, camera distance ~1.3–4.5 radii) would make the detail worth
  much more; proposed as a separate issue (decision 5).

## 7. Performance budget and quality levels

The view is static between updates, so the budget applies to dragging: ≥ 30 fps on a mid-range
phone (WebGL2, DPR 2–3, the planet covering ~40 % of ~2.5 Mpx), ≥ 60 fps on desktop.
- Octaves are clamped by the pixel footprint, so the visible detail (and cost) is bounded by the
  screen size, not the noise settings: Low 2 octaves, no bump, haze only in the planet shader;
  Medium 4 octaves, bump, atmosphere shell; High 6 octaves, ridged mountains with two noise
  fields, glint with GGX, (clouds if on).
- Default: Medium on the web and with a coarse pointer, High on the desktop; the option overrides.
  If measured frame times on the test phones exceed the budget, the default drops to Low there.
- MSAA 4× stays on the desktop; the web has none, so the octave clamp and analytic edge
  anti-aliasing must suffice there (tested in chromium).
- CPU: one material pass per update (a few ms at 41k cells, measured in the debug log like
  "Display refreshed" today); GPU memory ~10 MB for the de-indexed material buffer at 6
  subdivisions (~2.5 MB at 5 on the web).

## 8. Testing

- **Unit tests (Rust):**
  - anchor stability: a synthetic plate rotated between two frames — the anchor of a crust point
    is unchanged after the rotation; split → no change; merge → only the absorbed plate changes;
  - frame codec round trip with plate orientations (and the version bump);
  - material mapping: determinism (same input → same bytes), monotonicity and plausibility
    checks (deeper → darker ocean, drier → less vegetation, colder → more snow, abiotic → no
    vegetation), and agreement with Köppen groups on a real planet state (e.g. ≥ 80 % of BW cells
    bare, of Af cells vegetated) once #147 is in;
  - the threshold weight is zero at vertices (cell centres keep their class);
  - WGSL validation with naga for WGSL and GLSL ES 3.00 (WebGL2) output.
- **Visual checks** (no egui_kittest smoke tests): desktop on `vscreen` (subdivisions 4, short
  duration, Read the screenshots); web in headless chromium with
  `--use-angle=swiftshader --enable-unsafe-swiftshader --ignore-gpu-blocklist`, desktop and a
  phone-sized window, each quality level; a series of consecutive frames from the time bar to
  check that detail moves with the plates and does not flicker; a merge frame to see the
  re-roll. Screenshots in `notes/issue-92/`.
- Frame-time numbers from the debug log on desktop and swiftshader (relative only).

## 9. Risks

- **Subjective look:** "photorealistic" from 110 km data with 5–15 px per cell will look like a
  plausible planet seen from far away, not like satellite imagery; without zoom much of the
  detail stays invisible.
- **WebGL2 limits / mobile GPUs:** shader size and precision on old phones; mitigated by the
  naga GLSL test, the Low level and test runs in swiftshader.
- **Aliasing and shimmer** while rotating: octave clamping; ridged noise is the risky part.
- **Plate-boundary seams and merge re-rolls** (§3): visible as grid artefacts; acceptable per
  the project rules, explained in the legend/notes.
- **Vertex count:** the de-indexed mesh has 6× the vertices (246k at 6 subdivisions); fine for
  the GPU, but the material expansion must stay a simple loop.
- **Conflicts with the five open mode PRs** in `CellInfo`, codec and mode list: implement stacked
  on #147 and rebase as they merge; the frame change here is only in `PlateInfo`.
- **Colour space:** today's pipeline writes sRGB values unconverted; the new one works in linear
  and must encode correctly for both the desktop and the web target formats.
- **Exaggeration 40×:** mountains reach 5 % of the radius, the true atmosphere 1.5 %; the shell
  and haze must be scaled to look right with exaggerated relief.

## 10. Stages (each a reviewable PR, MVP first)

| # | Stage | Content | Effort |
|---|---|---|---|
| 0 | Plate frames | `PlateInfo` + orientation, codec version bump, biosphere/orbit plumbed into the view, merge/terrane event count in a real run, tests | ¾ day |
| 1 | **MVP** | Mode + options skeleton, second pipeline with own uniforms, de-indexed mesh and material buffer, colours from the data (§4, no noise), sun light with terminator, ocean depth colour + Fresnel + glint, simple limb haze, black sky with sun disc, naga test, screenshots desktop/web | 2 days |
| 2 | Detail | Plate-anchored noise, sub-cell coastlines/snow/tree lines, colour variation, bump relief, octave clamp, quality levels, stability tests over consecutive frames | 2 days |
| 3 | Atmosphere and sky | Atmosphere shell with analytic scattering (sunset band, backlit ring, aerial haze), star field, sun glow | 1–1½ days |
| 4 | Water and ice details | Lakes from #146, optional river lines, seasonal snow and sea-ice edges, ridge orientation in orogens | 1–1½ days |
| 5 | Clouds (optional) | Climate-derived cloud layer, option | 1½ days |
| 6 | Merge-stable detail (optional) | Noise-frame tag per crust sample (simulation + frames) | 1 day |

Stages 0–2 give the issue's core (planet in space, sun, terminator, glint, stable sub-cell
detail); 3 completes the lighting list; 4–6 are refinements. Independent of these, a camera zoom
(separate issue, ~1 day) is what makes stage 2 pay off.

## 11. Decisions for Sebastian

1. **Light:** sun fixed in space (terminator visible; drag round to see the night side) as the
   default, with an option "Light: Sun | Camera"? (Recommended.)
2. **Merges:** accept a one-frame re-roll of sub-cell detail on the absorbed plate when plates
   merge (MVP), or do stage 6 (display-only tag on every crust sample)?
3. **Vegetation:** derived from continuous climate fields (aridity, warmth; bare for `abiotic`),
   with Köppen only as a test oracle? (Recommended; #90 would replace it later.)
4. **Season option** (Year/January/July: sun latitude, snow, greenness): wanted, or always the
   annual state?
5. **Zoom:** open a separate issue for camera zoom (scroll/pinch), which most of the sub-cell
   detail depends on?
6. **Exaggeration:** keep the global 40× relief and scale the atmosphere to match, or should the
   realistic mode default to a lower relief exaggeration (needs picking to follow it)?
7. **Clouds:** include as an optional, climate-derived decoration (stage 5, off by default), or
   leave them out because they are not simulated?
8. **Rivers:** leave them out (invisible from space at this scale) or draw large ones as thin
   lines behind an option?
9. **Quality default:** Medium on web/touch, High on desktop, user override — OK?

## 12. Independent review (2026-11-24)

A separate agent checked the plan against the code. Confirmed: splits copy the rotation,
merges rebase by position, only `move_plates` changes rotations; 1 Myr steps move crust ¼–1
cell; `PLATE_BYTES` 16 → 32; `textureLoad` and f32 precision on WebGL2. Corrections taken into
the plan above:
- barycentrics need a de-indexed mesh (shared icosphere vertices cannot carry them);
- per-pixel anchor from per-plate orientations (uniform array) instead of interpolated per-vertex
  anchors, which smear across plate boundaries;
- merges are frequent (terrane accretion = split + merge): count them before deciding on stage 6;
- the web has no MSAA;
- the obliquity is `OrbitParams::obliquity`, declination ±21° mid-month; biosphere and orbit are
  not in the view yet;
- uniforms need eye position, inverse view-projection and sun; own bind group;
- sky after the planet at the far plane; haze on the disc in the planet shader (linear), shell
  only off-disc;
- derivatives in uniform control flow; exact naga WebGL2 target and limits (15 inter-stage
  variables);
- water normal from the position, not the tilted mesh normal near coasts; name the new field
  `orientation` (`PlateInfo::rotation` is the angular velocity).
