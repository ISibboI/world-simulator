# #92 stages 0 + 1: the realistic surface (MVP)

The Realistic mode used to show the topographic map as a placeholder. It now shows a surface
derived from the cell data: reflectances lit in linear light by the simulation's sun, the relief
true to scale, a sun glint on the water, and a thin haze towards the limb. Sebastian's decisions
from [plan.md](plan.md) §0 apply:

- realistic relief, without exaggeration;
- the season option belongs to the realistic mode;
- no rivers, since they are invisible from space;
- no zoom;
- clouds come later;
- the detail re-roll on plate merges is accepted;
- distance-based LOD comes later.

## Stage 0: deferred to stage 2

Stage 0 had two parts in the plan. The **plate orientation** in `PlateInfo` and the frames
anchors the sub-cell noise to the crust. Stage 1 has no noise, so the orientation would be
unused. It moves to stage 2, where the noise needs it. The other part, the **biosphere and
orbit for the view**, is needed now and comes from the launch options, like the epoch in step 1.
`--biosphere abiotic` gives bare land. The orbit is the simulation's default, since `--set`
cannot change it. **Frame growth: 0 bytes. The codec is unchanged.**

## Materials (`src/viewer/modes/realistic.rs`, `material`)

Each cell gets a reflectance in linear RGB on the CPU, from its data and the season. Each
mapping is continuous in its inputs, except where the data itself has hard edges: permanent
ice, land against sea, lakes.

| Surface | Rule | Reflectance (linear RGB) |
|---|---|---|
| Ocean | shelf colour at ≤ 100 m depth, deep-ocean colour at ≥ 3000 m, logarithmic in between | shelf (0.018, 0.06, 0.07), deep (0.006, 0.016, 0.042) |
| Lake (`lake_depth > 0`) | dark water | (0.012, 0.03, 0.04) |
| Permanent ice (`ice > 0.5`) | ice sheet on land, sea ice on the ocean | (0.78, 0.81, 0.86) / (0.55, 0.60, 0.66) |
| Bare ground | rock/soil, blended to sand by aridity (moisture index MI = P/PET from 0.4 down to 0.1) | rock (0.15, 0.135, 0.12), soil (0.17, 0.13, 0.09), sand (0.45, 0.33, 0.20) |
| Vegetation (not when abiotic) | cover = smoothstep(MI 0.08 → 0.5) × smoothstep(warmest month 0 → 6 °C); trees = smoothstep(MI 0.5 → 1) × smoothstep(warmest month 6 → 12 °C, near the tree line); grasses from steppe to grassland with MI 0.25 → 0.6 | steppe (0.20, 0.17, 0.09), grassland (0.075, 0.11, 0.035), forest (0.03, 0.06, 0.025) |
| Tundra | where the warmest month is below 8 °C, fully below 2 °C | (0.11, 0.105, 0.075) |
| High mountains | blend to bare rock from 2500 to 4500 m | rock |
| January / July | seasonal snow where the month is below −1 to −6 °C and the month has precipitation (forests hide most of it); sea ice on freezing ocean cells; grasses brown in a cold or dry month | snow (0.80, 0.82, 0.85), snowy forest (0.22, 0.24, 0.26) |

The colours were checked against the usual albedos (unit tests):

| Surface | Usual albedo |
|---|---|
| Ocean | < 0.03, plus the glint |
| Forest | 0.04–0.1 |
| Grassland | 0.08–0.2 |
| Desert | 0.3–0.42 |
| Ice | 0.7–0.9 |

The vegetation is a proxy from the climate fields, to be replaced by #90 (life). The Köppen
classes are not an input; using them as a test oracle is left for the stage with sub-cell detail.

**Water** carries its roughness (GGX α) in the alpha channel of the vertex colour; land has 1.
The roughness follows the wind speed of the season through the Cox & Munk (1954) mean-square
slope, `α = √(0.003 + 0.00512 U)`: 0.055 in calm air, 0.2 at 7 m/s, capped at 0.45.

## Lighting (`planet.wgsl`, `realistic`)

The realistic branch decodes the vertex colours to linear reflectance, lights them, and encodes
the result to sRGB. The data modes keep their old path, unchanged.

- **Diffuse:** `albedo × (ambient + sun × n·l)`. The sun's colour and the exposure gain of the
  eye adaptation (#156) come from `light_color`, decoded to linear light. The ambient share of
  **Sun + ambient** is decoded the same way, so the night side has the same sRGB brightness as
  before.
- **Water:** the normal is the sphere's normal, not the mesh normal tilted by a coast. The
  glint uses a GGX distribution, Schlick's Fresnel term with F0 = 0.02 (water) and Smith–Schlick
  shadowing. The water body is seen through the surface, which reflects the share F. The sky
  reflected in the water is black space, so only the sun is reflected. Water is blended over half
  a coast triangle (alpha 0.6 → 0.4).
- **Haze:** a cheap stand-in for the atmosphere, (1 − up·v)³ on the day side, in Rayleigh blue.
  Stage 3 replaces it with the atmosphere shell.
- **Relief:** true to scale (exaggeration 1 in this mode), so the planet is almost a sphere and
  mountains shade only faintly. The surface is rebuilt when switching between the realistic mode
  and a data mode. Picking and the cell outline follow the shown vertices.
- **Season** (Year | January | July): the sun's declination and distance for mid-January and
  mid-July (`lighting::season_fraction`; the months start at perihelion), plus the materials of
  that month. Year is an equinox at the mean distance, with the annual materials.
- **Validation:** a unit test (`renderer::tests::shaders_translate_to_webgl2`) parses and
  validates every shader of the view with naga (dev-dependency, wgpu's version 30.0.2). It also
  translates each entry point to GLSL ES 3.00 for WebGL2, so a shader the web cannot run fails in
  `cargo test`.

## Grid artefacts and limits (accepted)

- Colours change in steps of a cell, about 110 km at 6 subdivisions and 220 km on the web.
  Coastlines and the edges of ice and deserts follow the triangles of the mesh. Sub-cell
  coastlines and detail come in stage 2.
- The haze is not an atmosphere: there is no glow beyond the limb and no reddening at the
  terminator.
- There are no clouds, so the planet looks clearer than Earth, which is ~2/3 cloud-covered.
- Many deserts in short runs: the colours follow the simulated moisture index, and in the
  first Myr of a run large parts of the land are arid.

## Screenshots ([surface/](surface/))

5 subdivisions, 10 Myr, seed 0, Sun light:

- `desktop-year.png`: the annual state; glint on the ocean, haze at the limb, deserts, green
  coasts, polar ice.
- `desktop-january.png` / `desktop-july.png`: the sun's declination moves the polar night; the
  materials of the month.
- `desktop-terminator.png`: the terminator, with the glint near a coast.
- `desktop-abiotic.png`: `--biosphere abiotic`, bare land with no vegetation.
- `web.png` and `phone-legend.png`: chromium with SwiftShader at 1280×800, and at phone width
  (390×844) with the legend open. The Light and Season rows wrap.
