# #92 stage 4: water and ice details — plan

Status: plan (2026-11-26), before review. Stage 4 of [plan.md](plan.md) §10, adapted to
Sebastian's decisions (§0): **no rivers** (invisible from space); relief true to scale; no zoom;
LOD by the pixel footprint as in stage 2; Detail default Medium; Light option only in the
Realistic mode; Topographic stays the start-up mode; nothing but the sun (its glow and glint) may
over-expose.

Project goal: an Earth-*like* planet, judged by physical plausibility; the ~110 km grid
(~220 km on the web) is accepted and explained where it shows.

## 1. What stages 1–3 already do

- **Lakes** are water cells (`CellInfo::water_body`, #128: a closed depression with ≥ 1 m of
  water) with one flat colour `LAKE` (0.012, 0.03, 0.04) whatever their depth, the ocean's
  wind roughness (Cox & Munk) and so the same glint, and a coast value
  `−(0.5 + 0.5 smoothstep(0, 200 m, depth))`, so the noise (±0.5) never erases a lake.
  In January/July a lake freezes with the land rule `smoothstep(−1, −6 °C)` and shows `SNOW`.
- **Dry depressions** (closed depressions with < 1 m of water) are ordinary land.
- **Sub-cell "lakes" on all lowlands:** land corners have the coast value
  `0.4 + 0.6 smoothstep(0, 2000 m, elevation)`; the noise reaches ±0.5, so wherever the land is
  below ~500 m the noise opens small water patches — in deserts as well as in humid lowlands —
  and they take the water layer of the cell (shelf colour, glint). On land next to a high lake
  the coast value counts the height above sea level, not above the lake, so the shoreline is
  pushed into the lake.
- **Sea ice:** annual state: binary, `ice > 0.5` → full `SEA_ICE`. January/July: the binary
  permanent ice, or `smoothstep(−1, −6 °C)` of the month's temperature. So in July the annual
  ice (> 0.5) stays fully frozen, and a cell with annual cover 0.4 shows open water in January
  unless its month is cold. The edge noise is the plate-anchored pattern, i.e. it rides on the
  oceanic plate under the ice.
- **Snow:** a cover per cell (month freezing × precipitation); the shader roughens it with an
  independent pattern (`3.2 w c(1−c) n₂`) and sharpens it around ½. The snow does not follow the
  bump relief.
- **Bump relief:** isotropic fBm (ridged on High); no orientation.

## 2. Changes

### 2.1 Lakes

1. **Colour by depth and salinity** (CPU, `layers`):
   - fresh water from a turbid, greenish shallow colour at ≤ 3 m to clear dark water at
     ≥ 150 m (logarithmic, like the ocean's shelf→deep): `LAKE_SHALLOW` ≈ (0.030, 0.050, 0.035)
     (albedo ~0.045), `LAKE_DEEP` ≈ (0.008, 0.020, 0.032) (~0.018);
   - **saline lakes:** endorheic lakes (the lake's water does not leave: following the
     receivers inside the lake ends at a cell without a receiver) in an arid climate (moisture
     index P/PET of the lake cell < 0.4…0.2) are tinted towards a pale, shallow saline colour
     `SALINE` ≈ (0.06, 0.11, 0.10) (Aral, Great Salt Lake, Caspian shallows), weighted by
     aridity and shallowness. The simulation has no salinity; this is a proxy from closure and
     aridity, as in nature (closed lakes in dry climates are saline).
2. **Calmer water:** lakes are fetch-limited; their roughness uses Cox & Munk with half the
   wind speed, so their glint is sharper than the ocean's at the same wind.
3. **Lake ice** with the simulation's own rule: the frozen share of the chosen month is
   `1 − ice_free(T) = clamp((2 − T)/4, 0, 1)` (`climate::ice_free`, which also stops lake
   evaporation); colour `LAKE_ICE` ≈ (0.60, 0.64, 0.70), snow-covered lake ice, darker than
   fresh snow. Annual state: the permanent ice share only. Noise anchored to the plate (the
   lake sits on the crust).
4. **Shorelines:** the coast value of a land corner counts its height above the highest water
   surface among its water neighbours (a lake's level `elevation + depth`, or the sea), not above
   sea level. Lakes on plateaus get shorelines in the same place as sea coasts.
5. **No sub-cell lakes in dry land:** the coast value of land corners gets a floor that grows
   with aridity, `0.4 → 0.55` as the moisture index falls from 0.8 to 0.3. At ≥ 0.5 the ±0.5
   noise cannot open water inside the land, so only humid lowlands keep sub-cell lakes (as on
   glaciated shields and wet plains on Earth). Shorelines next to water move by a few per cent of
   an edge.
6. **Playas / salt flats:** a dry depression's sink (a cell of a closed depression without a
   receiver: water reaches it and evaporates) in an arid climate shows a salt flat
   `SALT_FLAT` ≈ (0.50, 0.48, 0.44) (albedo ~0.48; playas 0.3–0.5, salt pans higher), blended
   by aridity (MI 0.4 → 0.15); its plant cover is cut accordingly. Only the sink, not the whole
   basin, since only the lowest part floods and dries.

### 2.2 Sea ice

1. **Continuous cover**, so the edge falls between cells where the interpolated cover crosses ½
   (sub-cell, not in cell steps):
   - annual state: the annual mean cover `ice` (0–1) of the simulation as it is;
   - January/July: the month's cover from the month's surface temperature, `smoothstep(−1, −4 °C)`
     (open water cannot cool below −1.8 °C in the model, so a mean below that means ice),
     clamped to what the annual mean allows: at least `12·ice − 11` (ice nearly all year
     stays), at most `12·ice` (ice that is rare in the annual mean cannot cover a whole month).
2. **Colour by temperature:** cold, snow-covered sea ice (≈ 0.72) to melting, wet ice with melt
   ponds (≈ 0.47), from the season's temperature (−10 °C → 0 °C), like the model's own
   `sea_ice_albedo` / `melting_ice_albedo` switch at 0 °C.
3. **Noise frame:** sea ice is part of the climate, not of the crust: its edge pattern is
   anchored to the **world frame** (`up`), not to the oceanic plate below. Otherwise the pattern
   would drift ~10–100 km per Myr frame under an ice edge that stays where the climate puts it.
   Per corner the vertex carries an "ocean" flag; the shader mixes the plate pattern and a
   world-frame pattern by the interpolated ocean share, and evaluates the world pattern only in
   pixels where the snow/ice layer is partial (in branch, after the derivatives).
   Leads in the marginal ice zone show open water and glint; the floes do not.
4. Ice sheets on land: continuous share as well (`ice` instead of `ice > 0.5`), merged with the
   seasonal snow by `max`.

### 2.3 Seasonal snow lines

- The snow line follows the relief: on rugged land the snow noise is mixed towards the implied
  height of the bump relief (the same fBm that tilts the normals, its ridged form on High), so
  the snow lies on the bump peaks and ridges and the valleys melt out, as with a lapse rate.
  `n_snow = mix(n_pattern, height, ruggedness)`.
- The cell covers stay as stage 1 (month below freezing × the month's precipitation); with the
  continuous ice share (2.2.4) the edge between an iced cell and a snow-free one is sub-cell.

### 2.4 Ridge orientation in orogens

Data: no frame change needed. The strike of a mountain belt is perpendicular to the slope of the
surface across it, and the frames hold every cell's elevation.

- **CPU:** per land cell, the gradient of the drawn surface height (`surface_height`, sea at 0)
  by least squares over its neighbours in the tangent plane; strike `s = normalize(up × ∇h)`;
  anisotropy `a = smoothstep(3, 15 m/km, |∇h|) × ruggedness` (0 on water, plains and flat
  crests). Thresholds checked against a real state (a 3–4 km belt over 200–300 km is 10–20 m/km).
- **Vertex data:** like the plates, each vertex carries the strike of **all three corners**:
  `ridges: [u32; 3]`, per corner `s·a` as snorm8 xyz plus a flag byte (bit 0: ocean corner,
  for 2.2.3). Passed as one flat `vec3<u32>` varying; unpacked with shifts (no
  `unpack4x8snorm`, which GLSL ES 3.00 lacks). `DetailVertex` 52 → 64 bytes (+23%; 15.7 MB at
  6 subdivisions, 3.9 MB at 5).
- **Shader (Medium/High; Low has no bump):** where a corner of the triangle has `a > 0`, each
  such corner `j` with `b_j > 0` evaluates the fBm in a stretched frame:
  `y = base·anchor`, `y_j = y − (1 − 1/k_j)(y·ŝ_j)ŝ_j`, `k_j = 1 + 3a_j` (ridges up to 4× as long
  as wide), `ŝ_j` the strike rotated into the plate frame. The gradient is `M_j ∇n`. Isotropic
  corners use the existing `shape`. The results are blended with the barycentric weights and
  renormalised (`Σ b_j n_j / sqrt(Σ b_j²)`, isotropic corners merged first since they share one
  field), so the variance stays the same. Each `ŝ_j` is constant per cell, so there is no swirl
  (a strike interpolated across the triangle would shear the noise by |y|·Δθ ≈ several cycles
  per cell); the blend is continuous across edges because a shared edge has the same two
  corners. The oriented field replaces `shape` in the bump and the snow height; the coastline
  keeps the isotropic `shape`.
- **Cost:** up to 3 extra fBm per pixel, but only in orogen triangles (a few % of the disc).

### 2.5 Legend and text

Swatches: Lake, Saline lake, Salt flat, Sea ice (with Ice, snow). Description: lakes and their
ice, salt flats, sea ice of the season; the hover of *Season* mentions lake ice.

## 3. Grid limits (accepted, to explain)

- A lake is at least one cell (~110 km; ~220 km on the web). Small lakes do not exist in the
  data; humid lowlands get sub-cell water patches from the noise only (decoration).
- Lake ice and sea ice of a month come from monthly mean temperatures, with ramps standing in for
  the freeze-up within the month; sea ice per month is a proxy bounded by the simulated annual
  cover (the frames do not hold the monthly ice).
- Saline lakes and salt flats are proxies (closure + aridity); the simulation has no salt.
- Strike from the surface gradient over one ring of cells: belts narrower than ~2 cells have a
  noisy strike; a flat plateau has none (isotropic).

## 4. Tests

- Lakes: deeper → darker; saline tint only for closed lakes in arid climate; lake roughness
  below the ocean's at the same wind; lake ice follows `ice_free` (open at +2 °C, frozen at
  −2 °C), none in the annual state without permanent ice.
- Playa: arid dry sink → salt flat (brighter, albedo 0.3–0.55); humid sink or non-sink → not.
- Coast values: arid lowland ≥ 0.5 (no noise lakes), humid lowland 0.4; land beside a high
  lake counts from the lake's level.
- Sea ice: annual = `ice`; July with `ice = 1` stays frozen; `ice = 0` never freezes; a cold
  January with intermediate `ice` freezes; colder ice brighter; all albedos below the knee (0.86).
- Strike: a synthetic north–south ridge gives a north–south strike with `a > 0`; flat land
  `a = 0`; water 0; the ocean flag set on ocean corners only; vertex packing round-trips.
- naga: `realistic.wgsl` still translates to GLSL ES 3.00.

## 5. Measurements and screenshots

- Frame times as stage 3 (median while dragging, 5 subdivisions, 1280×800, Low/Medium/High),
  llvmpipe and SwiftShader WebGL2, before/after.
- Screenshots in [water-ice/](water-ice/): lakes, lake ice (January), snow lines January vs
  July, a sea-ice edge, an orogen close-up; desktop, web, phone 390×844. A longer run (more
  orogens and lakes than at 10 Myr) if a 10 Myr state shows too few.
