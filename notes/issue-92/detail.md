# #92 stage 2: sub-cell detail anchored to the plates

Sebastian, on #157 (2026-11-24): "1. No, Topo should stay default. 2. Go ahead." Topographic
therefore stays the mode at start-up. This stage adds procedural detail inside the cells of the
Realistic mode: coastlines, snow and ice edges, patchy plant cover, colour variation and bump
relief. The detail is generated on the fly: there are more octaves where a pixel covers less
ground, and it is anchored to the plates, so it moves with the crust.

## Stage 0: the plate orientation in the frames

- `PlateInfo` gains `orientation`, the quaternion from the plate's local frame (in which the
  simulation stores its crust) into the world (`Simulation::plate_rotation`).
- The frame history carries it: `PLATE_BYTES` goes from 16 to 32 and the codec `VERSION` from 10
  to 11. **Frame growth: +16 bytes per plate per frame**, a few hundred bytes for ~12–40 plates,
  against ~2 MB of raw cell planes (≈ 0.02%). The web worker sends `PlateInfo` as plain bytes, so
  it needs no change.

**Merges re-roll the detail of the crust they move.** A merge gives the absorbed crust new
coordinates in its new plate's frame. I counted the events that move crust between frames in two
default runs (seeds 0 and 1, 300 Myr, 6 subdivisions):

| Seed | Sutures | Captured pieces (cells) | Terranes (cells) | Cells per Myr |
|---|---|---|---|---|
| 0 | 0 | 766 (5862) | 29 (2244) | ~27 |
| 1 | 2 | 504 (2164) | 22 (2493) | ~16 |

About 16–27 of 41k cells re-roll per 1 Myr frame (0.04–0.07% of the surface), mostly small
detached pieces. Only those cells change: the coarse colours stay, and the detail changes in
place. As decided, this is accepted. Cells that change plate at boundaries, through subduction or
new sea floor, change their detail as well, which is correct, since the surface really changes
there.

## The mesh and the shader

- **De-indexed mesh** (`renderer::DetailVertex`, 52 bytes; built by
  `modes::realistic::detail_vertices`): each triangle has its own three vertices. Each vertex
  holds its corner (giving barycentric coordinates in the shader) and the plates of all three
  corners. It also carries its cell's layers:
  - bare ground and plants with the plant cover;
  - snow or ice with its cover;
  - open water with its roughness;
  - relief ruggedness;
  - a coast value: positive on land, negative on water, 0.4–1 in size, growing with height and
    depth.
- Water cells take the land layers of their land neighbours, and land cells the water of their
  water neighbours, so a pixel the noise turns into land shows the land beside it.
- **Memory:** 12.8 MB at 6 subdivisions (246k vertices) and 3.2 MB at 5 (61k, the web default).
- **CPU:** rebuilt with every new frame or option change, 12.6 ms at 5 subdivisions. The camera
  moving costs no CPU work.
- **Plate frames:** a second uniform buffer holds the orientations of up to 256 plates (runs
  have tens) and the quality settings.
- **Pipeline:** `realistic.wgsl` has its own pipeline, which shares the bind group layout. The
  data modes keep `planet.wgsl` unchanged, apart from the removal of the stage-1 realistic
  branch.

Per pixel (`realistic.wgsl`):

1. **Derivatives first**, in uniform control flow: the pixel footprint on the sphere (the
   longer of the screen derivatives of the unit position) and that of the coast value.
2. **Anchor:** the plate of the nearest corner, i.e. the largest barycentric coordinate. The
   point is rotated into that plate's frame, `q⁻¹ · p`. Within a plate the noise therefore
   moves rigidly with the crust. The plate changes on the Voronoi-like edges between cells of
   different plates, which gives a seam where the surface changes anyway (ridges, trenches,
   orogens).
3. **Weight:** `w = 1 − smoothstep(0.6, 1, max barycentric)`, zero at the corners. Noise that
   moves a boundary is multiplied by `w`, so **every cell centre keeps the class its data
   gives** (land or water, ice or no ice).
4. **Level of detail:** octave `i` has `base · 2^i` cycles per radius, where `base` is one cycle
   per cell spacing. Only octaves below the pixel's Nyquist limit are summed (`frequency ×
   footprint < 0.5`), and the last one fades in fractionally. At most the quality's octaves are
   used. This is the distance-based, on-the-fly level of detail Sebastian asked for: the footprint
   grows towards the limb, where the surface is seen at a slant, and on small screens. More
   pixels per cell (a larger window or a high-DPI screen) give more octaves. It also anti-aliases
   the noise without MSAA, which the web lacks.
5. **Noise:** gradient noise with analytic derivatives (Quílez) and an integer hash of the
   lattice point (the same on every GPU), summed as fBm. The sum is scaled by 2.7 to a standard
   deviation of about 0.4; the raw sum's is about 0.15, as the review measured by Monte Carlo. A
   second, independent fBm gives the pattern for snow, plants and colour. The Low level reuses
   the first.
6. **Features:**
   - **coast:** `coast + 0.5 w n₁` (n clamped to ±1), anti-aliased over a pixel. This gives
     fractal coastlines and bays, and islands and lakes where the land is low or the water
     shallow. In the deep ocean (coast value near −1) no island can appear, since the noise
     reaches at most ±0.5;
   - **snow and ice:** the cover plus `3.2 w s(1−s) n₂`, sharpened around one half. Ice and snow
     edges are crisp, while full and absent cover stay as they are;
   - **plants:** the cover plus `2.4 w c(1−c) (n₂ − n₁) 0.7`. Partial cover breaks into patches
     and the tree line becomes ragged;
   - **colour variation:** ±15% brightness from both patterns at |n| = 1 (≈ ±6% rms),
     everywhere. It changes no class;
   - **bump relief** (Medium and High): the noise gradient, rotated back into the world and made
     tangential, tilts the normal of rugged land by `0.2 × ruggedness / √octaves`. Every octave
     adds as much slope, so this normalisation keeps the relief equally rough on large screens.
     The gradient is taken with respect to the noise coordinates (cycles per cell spacing), so
     the implied heights scale with the cell size: a few km rms on fully rugged land. Ruggedness comes from
     the elevation (0.3–3 km) and the orogenic share. High uses ridged noise for mountains (the
     gradient sign flips with the noise sign). Water lies flat on the sphere.
7. **Lighting:** as in stage 1 (sun, ambient, Fresnel/GGX glint on open water, haze at the limb).
   The reflectances are decoded with the exact sRGB curve, the same one the CPU encodes them
   with. A polynomial fit was tried and measured no faster, and it was 10–27% off for the
   dark water.

**Quality option** of the Realistic mode (Detail: Medium | Low | High; Medium is the default
until the defaults are decided):

| Level | Most octaves | Bump | Ridged | Patterns |
|---|---|---|---|---|
| Low | 2 | no | no | one, reused |
| Medium | 4 | yes | no | two |
| High | 6 | yes | yes | two |

At the fixed camera distance a cell is only 5–15 pixels, so the footprint usually allows 1–3
octaves. Medium and High then differ mainly in the ridged relief, and more on large or high-DPI
screens.

## Frame times (median while dragging; 5 subdivisions, 1280×800)

| | Topographic | Low | Medium | High |
|---|---|---|---|---|
| Desktop, llvmpipe (software), MSAA 4× | 23 ms | 46 ms | 61 ms | 61 ms |
| Web, chromium with SwiftShader (software WebGL2) | 125 ms | 327 ms | 420 ms | 415 ms |

Both renderers run on the CPU, so these are worst cases and only the ratios matter.

- The realistic surface costs 2–3.4× the topographic one. The level hardly matters above Low,
  because at this window size the footprint, not the level, limits the octaves; Low saves the
  second pattern and the relief. About a third of that is the
  de-indexed mesh with its 8 varyings; a flat colour without any shading measured 31 ms against
  23 ms on llvmpipe. Shading and noise make up the rest.
- On real GPUs, including phones, the pixel work is far below one frame. Mobile GPUs were not
  available for testing.
- The view only redraws while it is dragged, while a frame arrives, or while the eye adaptation
  converges.

## Checks

- **Unit tests:**
  - the detail mesh: every corner's layers, plates and corner index; coast signs; coasts exist;
    uniforms;
  - anchor stability under a plate rotation;
  - quality settings;
  - the codec round-trips the orientation;
  - the naga test translates `realistic.wgsl` to GLSL ES 3.00 for WebGL2.
- **Temporal stability:** three consecutive frames (8, 9 and 10 Myr; `detail/frames-8-9-10.png`)
  keep the same coastline and patterns, shifted with the crust. The lakes change with the lake
  data itself.
- **Visual:** desktop at each level, web (SwiftShader) and phone width (390×844). On the phone
  the planet is small, so the footprint allows fewer octaves and the detail is coarser.

**Memory left as it is:**
- In the realistic mode the indexed planet mesh is still built and uploaded, although only the
  detail mesh is drawn. Picking and the outline use its positions.
- The detail buffer stays allocated after switching to a data mode, about 13 MB at 6
  subdivisions, 3 MB at 5.

## Grid artefacts and limits (accepted)

- **Seams:** the detail switches plate where the barycentric coordinates of two corners of
  different plates are equal, in the middle of the triangle where the noise has full weight. The
  result is a crisp seam along plate boundaries, also in coastlines on active margins. It is
  consistent across shared edges. A cross-fade of the two plates' noise near the seam could
  soften it later.
- **Ice edges** between a fully iced cell and an ice-free one still follow the triangles of the
  mesh in steps of about a cell, made ragged by the noise.
- **Snow sharpening:** the cell centre of a partly snowy cell (a cover between 0.2 and 0.8) shows
  less or more snow than the cell's share, because snow and ice edges are drawn crisp.
- **No zoom:** most of the octaves stay invisible at the fixed camera distance.
- **Merges** re-roll the detail of the absorbed crust, about 0.05% of the surface per Myr (see
  above).

## Screenshots ([detail/](detail/))

5 subdivisions, 10 Myr, seed 0:

- `desktop-medium.png`, `desktop-low.png`, `desktop-high.png`;
- `zoom-low.png` / `zoom-high.png`: a coast with sub-cell lakes, and the polar ice edge;
- `frames-8-9-10.png`: three consecutive frames (8, 9 and 10 Myr);
- `web-medium.png`: the web version (chromium with SwiftShader);
- `phone.png`: phone width with the legend open. The Detail row fits, and the open ocean shows
  no specks.
