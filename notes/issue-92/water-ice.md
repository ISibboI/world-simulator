# #92 stage 4: water and ice details

Stage 4 of [plan.md](plan.md) §10, adapted to Sebastian's decisions (§0):

- **no rivers**, since they are invisible from space;
- relief true to scale, no zoom;
- level of detail by the pixel footprint, as in stage 2;
- Detail defaults to Medium; the Light option exists only in the Realistic mode;
- Topographic stays the start-up mode;
- nothing but the sun, its glow and its glint may over-expose.

Two independent agents reviewed the work:
- the **plan**, before the implementation (§6). It led to a mean-preserving sea-ice rule, a
  sign-free structure tensor for the strike, and several lake fixes;
- the **code**, afterwards (§6b). It found that an interpolated squeeze tensor cancels, so the
  ridges use per-corner noise fields.

**No frame change, no codec change.** Everything comes from fields the frames already hold:
- elevation, lake depth and the drainage bits;
- temperatures and precipitation of the year, January and July;
- the annual ice cover and the moisture.

The detail vertex grows from 52 to 64 bytes (+12 bytes: the strike, the squeeze and the ocean
flag of the triangle's three corners): 15.7 MB at 6 subdivisions (was 12.8), 3.9 MB at 5
(was 3.2).

## 1. What stages 1–3 did, and what was wrong

- **Lakes** had one flat colour whatever their depth, the ocean's roughness, and froze with the
  rule for land snow.
- **Sub-cell lakes on all lowlands:** the noise (±0.5) opened water wherever land corners
  were below ~500 m (coast value < 0.5), in deserts as well. These patches had the ocean's shelf
  colour.
- **Sea ice** was binary in the annual state (`ice > 0.5`). In January and July the permanent
  ice stayed fully frozen in summer. A month froze only below −1…−6 °C, but the model's melting
  ice sits at 0 °C. The edge noise rode on the oceanic plate under the ice.
- Snow did not follow the bump relief, and the relief had no orientation.

## 2. Lakes (`modes/realistic.rs`, `layers`, `detail_vertices`)

| Rule | Colour (linear reflectance) |
|---|---|
| Fresh lakes: turbid, greenish shallow water at ≤ 3 m to dark clear water at ≥ 300 m (logarithmic) | (0.030, 0.050, 0.035) → (0.009, 0.021, 0.028) |
| **Saline lakes:** a closed lake (the drainage, followed inside the lake, ends at a cell without a receiver, the exit of an endorheic lake) in a dry climate (moisture index 0.5 → 0.2), more so when shallow | → (0.06, 0.11, 0.10) |
| **Salt flat (playa):** the sink of a dry closed depression (no receiver: water collects there and evaporates) in a dry climate (MI 0.4 → 0.15). At most half the cell, since Earth's largest salt flats are smaller than a cell | → (0.50, 0.48, 0.44), plants cut by the same share |
| **Lake ice** (January/July): from −1 °C, fully frozen at −5 °C (shallow) to −10 °C (≥ 200 m deep, which stores the summer's heat); all year where the annual mean is −10 … −20 °C | snow-covered (0.60, 0.64, 0.70) where snow falls, bare grey ice (0.24, 0.28, 0.32) where it does not |

- **Calmer water:** lakes are fetch-limited, so their waves are those of half the wind speed
  (Cox & Munk). Their glint is sharper than the ocean's.
- **Air over low lakes:** the climate gives cells below sea level the ocean's surface
  temperature, also a lake's floor below sea level. A lake inside a frozen continent would then
  stay open in winter (seen in seed 2 at 130 Myr: a warm strip along a rift lake on the January
  map). For the ice, such lakes take the mean temperature of their neighbours above sea level,
  spread inwards over up to 8 rings (`lake_air`).
- **Shorelines** as at the coast (stage 2). A land corner's coast value now counts its height
  above the highest water surface next to it (a lake's level or the sea), not above sea level.
  A lake on a plateau gets its shore in the middle of the edge like a sea coast, not pushed into
  the lake.
- **Sub-cell lakes only in humid land:** the coast value of land has a floor that grows with
  aridity, 0.4 → 0.55 as the moisture index falls from 0.8 to 0.3. Above 0.5 the ±0.5 noise
  cannot open water, so dry land has no sub-cell lakes, while humid lowlands keep them (as on
  glaciated shields and wet plains). The water layer of land is now the shallow lake colour with
  the lake roughness, no longer the ocean's shelf.

The simulation has no salt. Saline lakes and salt flats are proxies from closure and aridity,
which is how they arise in nature. Lakes are rare in short runs: 0–1.4% of the land in most
frames of seeds 0–3, up to 5.5% (seed 2, 130 Myr). Dry closed depressions are rarer still.

## 3. Sea ice, snow and ice sheets

- **Annual state:** the ice cover sharpened around one half, `smoothstep(0.3, 0.7, ice)`, the
  topographic view's rule made continuous. A plain linear cover made the whole polar ocean
  (annual cover 0.5–0.8) a field of holes. Now the pack is closed, with a marginal zone of floes
  where the cover crosses one half. This is sub-cell: the edge lies where the interpolated cover
  crosses ½, not along the triangles.
- **January/July:** the frames hold only the annual cover. The month's temperature can't
  tell melting ice (0 °C in the model) from open water at its freezing point (−1.8 °C). So the
  hemisphere's winter month (the colder of the two) gets `min(1, 2·ice)` and the summer month
  `max(0, 2·ice − 1)`. The two average to the annual cover. Arctic-like ice with `ice = 0.8`
  keeps 60% in July: a loose summer pack.
- **Colour:** cold, snow-covered ice (0.68, 0.72, 0.78; albedo ≈ 0.72) to melting, wet ice
  with melt ponds (0.42, 0.48, 0.55; ≈ 0.47), from the season's temperature (−10 → 0 °C). These
  are surface albedos. The climate model's 0.55 / 0.45 are planetary albedos, which include the
  atmosphere.
- **Noise frame:** sea ice belongs to the climate, not to the crust under it. Its pattern is
  anchored to the **world frame**, not to the oceanic plate. Otherwise it would drift 10–100 km
  per Myr frame under an edge that stays where the climate puts it. The floes come from a
  pattern one octave finer than the cells'. The shader mixes it in by the interpolated ocean
  share (a flag per vertex), renormalised, only in pixels with partial cover. Leads show open
  water and glint; floes do not.
- **Ice sheets on land:** the same sharpened share (was `ice > 0.5`). The climate scales land
  ice by snowfall, so cold, dry deserts can show patchy ice where the share is near ½. Ice
  sheets also bury most of the bump relief (60% less).
- **Snow on the relief:** on rugged land the snow pattern mixes in the height that the bump
  relief implies: the noise on Medium, or −|noise| (recentred and rescaled) where High draws
  ridged crests. Snow then lies on the crests first and the valleys melt out, as with a lapse
  rate. The mix is renormalised to keep its spread.

## 4. Ridges along mountain belts

The frames suffice: a belt's strike is perpendicular to the slope of the surface across it.

- **CPU:** the gradient of the drawn surface height at every cell, by least squares over its
  neighbours (sea at 0). From it the **structure tensor** over the cell and its neighbours,
  `Σ ∇h ∇hᵀ`, in the cell's tangent plane. It is sign-free, so the two flanks of a range agree
  and its crest, where the gradient vanishes, takes their direction. The strike is
  perpendicular to the principal axis. Its strength is
  - `smoothstep(2, 8 m/km)` of the rms slope along the axis (gradients over ±1 cell are
    smoothed: a 4 km range 400 km wide measures ~5–10 m/km at 220 km cells),
  - times the coherence `(λ₁ − λ₂)/(λ₁ + λ₂)`,
  - times the ruggedness.
- **Vertex:** like the plates, every vertex carries the data of **all three corners** of its
  triangle, `ridges: [u32; 3]`. Per corner:
  - the strike `ŝ` as snorm8 x, y, z (world frame, sign-free);
  - the squeeze `σ = 1 − 1/k` in 7 bits (ridges up to k = 5 times as long as wide);
  - the ocean flag (for the sea-ice frame, §3).

  These pass to the fragment shader as one flat `vec3<u32>` varying, unpacked with shifts
  (GLSL ES 3.00 has no `unpackSnorm4x8`).
- **Shader** (Medium and High; Low has no bump): in a triangle with an oriented corner, each such
  corner `j` evaluates the relief fBm at `y − σ_j (y·ŝ_j) ŝ_j`, with gradient `M_j ∇n`. Here y is
  the point in the plate frame and `ŝ_j` the strike rotated into it. The other corners share the
  isotropic field. The fields are blended with the squared barycentric weights. A corner's field
  is fixed by its cell, so the blend is continuous across edges. Neighbouring corners of a belt
  have almost the same field, so the blend keeps its contrast there. The coastline keeps the
  isotropic noise. Cost: up to three extra fBm, only in orogen pixels.
- On a real state (seed 2, 130 Myr, 5 subdivisions), about 280 of ~2600 land cells line up:
  only the steep belts. The strength is
  - the slope term,
  - times the root of the coherence,
  - times the ruggedness (full from one half).

  The first version (k ≤ 4, linear in coherence and ruggedness) squeezed only weakly: a median
  σ of 0.03 and a 90th percentile of 0.37, barely visible. The final one is stronger.
- The effect is modest at the fixed camera distance: ridges are about one cell wide, a few
  pixels. Close-ups show the relief of belts streaked along the range instead of mottled.

## 5. Frame times

Median frame time while dragging (the method of stages 2–3), 1280×800, 5 subdivisions,
Realistic with the Sun light. Both renderers run on the CPU, so these are worst cases and only
the ratios matter; on real GPUs, phones included, this is far below a frame.

| | Low | Medium | High |
|---|---|---|---|
| Desktop, llvmpipe (2 threads, MSAA 4×), default view (seed 0, 10 Myr), before | 100.6 ms | 124.9 ms | 128.1 ms |
| after | 102.6 ms (+2%) | 127.4 ms (+2%) | 130.0 ms (+1.5%) |
| Desktop, orogen and ice in view (seed 2, 130 Myr), before | 97.6 ms | 121.6 ms | 123.5 ms |
| after | 100.4 ms (+3%) | 128.4 ms (+6%) | 131.5 ms (+6%) |
| Web, chromium with SwiftShader (WebGL2), default view, before | 395 ms | 482 ms | 492 ms |
| after | 445 ms (+13%) | 514 ms (+7%) | 534 ms (+9%) |

- The cost is the oriented relief in orogen pixels (up to three extra fBm on Medium/High), the
  world-frame floe pattern in pixels of partial sea ice, and the extra flat varying with its
  unpacking.
- SwiftShader pays relatively more, also on Low, which has no ridges: the default view has a
  broad partial sea-ice zone (an extra fBm there), and the integer varying and bit unpacking
  cost more there. Stage 3 added +23–33% on the same renderer.
- The web numbers need a local build with DEBUG logging and frames up to 2 s counted (the
  frame-time log drops intervals over 0.5 s as idle). Neither change is committed.
- On the CPU, the detail mesh now also gets the gradients, the structure tensor, the
  closed-lake walk and the lake air. The display refresh took 13–16 ms before and 14–17 ms
  after: about +1 ms at 5 subdivisions. It runs only when a new frame or option arrives, not
  while dragging.

## 6. Plan review (independent agent, before the implementation)

Adopted:
- the July sea ice vanished with a temperature rule, because melting ice sits at 0 °C. Now
  winter/summer from the annual cover;
- `Σ bᵢ nᵢ / sqrt(Σ bᵢ²)` over per-corner noise fields inflates the relief up to √3 where the
  fields agree. Now normalised by the sum of the weights, `Σ bᵢ² nᵢ / Σ bᵢ²`. The reviewer
  suggested an interpolated squeeze tensor instead. That was implemented first, but the code
  review (§6b) showed it cancels: each corner's strike is tangent at its own centre, so the
  interpolated squeeze of a coherent belt vanishes inside the triangle. Back to per-corner
  fields;
- the strike from one gradient cancels at the crest. Now the sign-free structure tensor over
  the neighbours, so crests get oriented;
- the snow height on High is −|n|, recentred;
- sub-cell lakes on land had the ocean's colour;
- lake ice came too early and was white without snow;
- lakes below sea level have the ocean's temperature and sea-ice field;
- playas were capped at half a cell;
- mixes of independent patterns are renormalised.

Not adopted from the plan review:
- a per-corner plate frame for the ridges, to remove the plate seam. The seam was accepted in
  stage 2 (`detail.md`). It still cuts across ridges where a belt is a plate boundary;
- depth-dependent freezing beyond the simple shift above.

## 6b. Code review (independent agent, after the implementation)

Fixed:
- **High:** the interpolated squeeze tensor cancelled inside every triangle of a coherent belt,
  while the gradient was still squeezed, so the shading and the snow disagreed. Replaced by
  per-corner fields (§4).
- snorm8 quantisation of the tensor leaked into a shift of up to 0.4 cycles, with a seam at
  the threshold. Gone with the per-corner fields: the strike is a direction, and a corner
  without a strike uses exactly the isotropic field.
- **Low:** its snow pattern is the negated relief noise, so mixing in the relief height
  cancelled it at mid ruggedness. Low no longer mixes the relief into the snow.
- `lake_air` now also gives dry depressions below sea level the air of the land around (salt
  flats, snow).
- Tests added: `closed_lakes` (closed against spilling lakes) and the shader's bit unpacking;
  the lake colour test is exact.

## 7. Grid limits and proxies (accepted)

- A lake is at least one cell: ~110 km, or ~220 km on the web. Sub-cell lakes in humid
  lowlands are decoration from the noise.
- Ice of a month: from the annual cover (sea ice) or the monthly mean temperature (lakes, snow),
  with ramps standing in for the freeze-up within the month. A summer pack is spread evenly over
  the cells that keep ice, so it looks loose everywhere rather than closed in the centre.
- Saline lakes and salt flats are proxies; the simulation tracks no salt.
- The strike over ±1 cell: belts narrower than ~2 cells have a noisy strike, and flat plateaus
  none. Plate seams (stage 2) cut through ridges where a belt is a plate boundary.

## 8. Checks

Unit tests (`modes/realistic.rs`):
- lakes: darker with depth; saline only when closed and dry; calmer than the ocean; lake ice in
  a cold month, not in a mild one or the annual state; deep lakes freeze later;
- salt flats only at dry sinks, at most half a cell;
- sea ice: annual cover sharpened; winter and summer average to the annual cover; Arctic-like
  ice stays in July; colder ice is brighter; all albedos below the exposure knee (0.86);
- the strike of a synthetic north–south range runs north–south on its flanks and crest, and
  none on the plains;
- lakes below sea level take the air of the land around and freeze;
- the coast floor of dry land and the ocean flag in the detail mesh;
- naga translates `realistic.wgsl` to GLSL ES 3.00 (WebGL2).

## 9. Screenshots ([water-ice/](water-ice/))

5 subdivisions; `desktop-*` with llvmpipe at 1280×800.

- `desktop-lakes.png`: seed 2 at 130 Myr, Sun, Detail Medium. A large rift lake (49 m deep in
  its middle) in the northern continent. It is dark like deep lakes on Earth, a little greener
  than the ocean, with stage 2's sub-cell shorelines.
- `desktop-lake-seasons.png`: the lake in the annual state, January and July (3× crops). It
  stays open in January: the air around it is 0 to +5 °C. To the west, land snow in January.
  At the top, a snow-capped range whose snow lies on the ridge crests.
- `desktop-lake-ice-jan-jul.png`: seed 1 at 120 Myr, a polar continent (Camera light).
  - In **January**, a lake at 74°N (7 m deep, January −33 °C) is frozen under snow and the
    ring of snow is closed.
  - In **July** (+10.6 °C) the lake is open (the dark patch in the right half of the ring),
    and the sea ice to the left breaks into floes.
  - The cold desert in the centre gets no snow (too dry). The grey blocks in the January
    snow are snow-covered forest, whose colour changes per cell. Both come from stage 1
    (unchanged, the same in the stage 3 build).
- `desktop-snow-sea-ice-jan-jul.png`: seed 0 at 300 Myr, looking down on the north pole
  (Camera light). In **January** the pack is closed, with a marginal zone of floes, and the
  land is under snow. In **July** a loose summer pack remains (the annual cover split
  winter/summer, §3), and the snow is gone.
- `desktop-sea-ice-edge.png`: the January ice edge (3×). The floes come from a world-frame
  pattern one octave finer than the cells, so the edge stays put between frames while the
  plates move.
- `desktop-north-january-sun.png`: the same view with the Sun in January: the Arctic in polar
  night.
- `desktop-orogen-before-after.png`: seed 2 at 130 Myr, Detail High, Camera light, 2.3×
  crops of the south-western belt.
  - Before (stage 3, left): mottled relief.
  - After (right): the relief is streaked along the range (NNE–SSW), and the ice cap is
    smoother (ice sheets bury 60% of the bump).

  At this camera distance a ridge is a few pixels wide, so the effect is modest.
- `web-desktop.png`, `web-phone.png`, `web-phone-legend.png`: the web version (chromium,
  SwiftShader WebGL2) at 1280×800 and at phone width (390×844), seed 2 at 130 Myr. On the
  phone the panels start collapsed. With the legend open, the four new swatches (sea ice,
  lake, saline lake, salt flat) wrap into the existing rows.
