# Seismically and volcanically active regions (issue #110): plan

Earthquakes and eruptions happen on time scales of seconds to centuries, far below the 1 Myr
step, so they cannot emerge from the simulation. What the simulation does have are the
processes that drive them: plate-boundary types and relative velocities, slabs and the arcs
above them, the deformation of the lithosphere (#182's sheet), spreading ridges, rifts and
mantle plumes. The view estimates **long-term rates** from them with published relations:
a seismic moment rate and a magma supply rate per area, each with its dominant source.

Earth-like planet, not Earth: the relations are calibrated on Earth, and Earth's totals are
plausibility checks, not targets. Errors of the ~110 km grid are accepted (#64).

Display data only: computed read-only from the state at every update; state hashes stay as
on the base branch (checked with a short run).

## 1. Seismic moment rate

### Plate boundaries (Kostrov 1974; Bird & Kagan 2004)

The long-term seismic moment rate of a plate-boundary fault of length L is

    Ṁ = μ · (c z) · L · v_slip · W/z

with μ the shear modulus, c z the *coupled seismogenic thickness* (seismogenic thickness z
times seismic coupling c; aseismic creep is the rest) and v_slip the slip rate. For a dipping
fault (dip δ) taking a horizontal relative velocity v (heave), the slip is v / cos δ and the
down-dip width z / sin δ, so

    Ṁ / L = μ · (c z) · v / (sin δ cos δ)        (dip-slip)
    Ṁ / L = μ · (c z) · v                         (strike-slip, δ = 90°)

Bird & Kagan (2004, BSSA 94, 2380) fitted c z per boundary class from PB2002 and the CMT
catalogue (values from their abstract; dips as in their analysis, rounded):

| Class (B&K) | c z (km) | dip | used for (model class) |
| --- | --- | --- | --- |
| SUB subduction | 18 | 14° | Subducting / Overriding: convergence |
| CCB continental convergent | 18 | 20° | Collision; continental Diffuse and OtherConvergent |
| OCB oceanic convergent | 3.8 | 20° | oceanic Diffuse and OtherConvergent |
| CTF continental transform | 8.6 | 90° | strike-slip at continental boundaries |
| OTF oceanic transform | 13 / 1.8 / 1.6 (slow < 4, medium < 6.8, fast cm/yr) | 90° | strike-slip at oceanic boundaries |
| OSR spreading ridge, normal | 0.13 | 55° | Ridge; oceanic OtherDivergent: opening |
| OSR spreading ridge, strike-slip | 0.40 | 90° | strike-slip at ridges |
| CRB continental rift | 3.0 | 55° | Rift; continental OtherDivergent: opening |

μ = 49 GPa for subduction (the megathrust reaches the mantle wedge), 27.7 GPa elsewhere
(B&K used these two values).

Relative motion: at each boundary cell, the relative velocity of the material (plate rotation
plus #182's deformation velocity, as `Simulation::material_relative_velocity`) is split into
its convergence v_n (normal to the boundary, signed) and strike-slip v_t. The class decides
which row takes v_n; v_t always goes to the strike-slip row of the crust (OTF/CTF, or OSR
strike-slip at ridges). Inactive boundaries (slower than `min_trench_convergence`) get none.
Continental = crust of continental origin (`Crust::is_continental_origin_column`).

Per cell: a boundary is a line between cells of two plates; the cells on each side share it,
so each boundary cell gets half of Ṁ/L times one cell spacing s (mean edge length), divided
by the cell area A:

    ṁ_cell = ½ · (Ṁ/L) · s / A          [N·m yr⁻¹ km⁻²]

A boundary that zig-zags across the hexagonal cells has ~0–15 % more cells per length than a
straight one; accepted (grid, #64).

Expected values: subduction at 6 cm/yr: Ṁ/L = 2.3 × 10¹⁴ N·m yr⁻¹ m⁻¹, ~10¹⁵ N·m yr⁻¹ km⁻² per
cell, global ≈ 10²² N·m/yr for ~50 000 km of trenches (Earth's long-term rate is
~(0.7–1) × 10²² N·m/yr, dominated by subduction, Kagan 2002). Ridges ~2 × 10¹², slow oceanic
transforms ~10¹³–10¹⁴, collision belts ~10¹⁴.

### Distributed deformation (Kostrov 1974; Savage & Simpson 1997)

Inside plates, #182's sheet gives the strain rate ε̇ (second invariant) where the lithosphere
deforms (collision bands, rift zones, diffuse zones, weak margins). The seismic moment rate
per area of a deforming region is

    ṁ = 2 μ (c z) ε̇

with c z = 9 km for lithosphere of continental origin (between CTF 8.6 and B&K's CCB) and
3.8 km (OCB) for oceanic lithosphere, μ = 27.7 GPa. A Tibet-like strain rate of 10⁻¹⁵ s⁻¹
gives 2 · 27.7 GPa · 9 km · 3.2 × 10⁻⁸ yr⁻¹ = 1.6 × 10¹³ N·m yr⁻¹ km⁻², the order of the Himalaya–Tibet belt's distributed seismicity. Boundary and
distributed contributions add: the sheet's strain is the deformation inside the plate, the
boundary rate the motion across the plate contact. Stable plate interiors have no strain in the
sheet and so no seismicity (Earth's intraplate seismicity is ~1 % of the moment rate; not
modelled).

Intermediate and deep earthquakes in the slabs (a few % of the moment) are left out: the map
shows shallow seismicity, as B&K's.

### Shown as

The colour is log₁₀ of ṁ, 10⁹ to 10¹⁶ N·m yr⁻¹ km⁻², with classes per decade. The cell widget
also gives the magnitude of a single earthquake that would release a century of the cell's
moment, Mw = ⅔ (log₁₀(ṁ A · 100 yr) − 9.1) (Hanks & Kanamori 1979), as an intuition aid (Mw ≈ 8
at a subduction cell, ≈ 6 at a fast ridge); per 100 × 100 km, about one cell at the default
resolution.

## 2. Magma supply (volcanism)

A volumetric rate per area, shown as a layer thickness per year (mm/yr = 10³ m³ yr⁻¹ km⁻²).
It includes intrusions: eruption (extrusive) shares are ~10–20 % for ridges and arcs (White et
al. 2006), which the legend states.

1. **Mid-ocean ridges:** crust production v_open · h, h = the model's new oceanic crust
   (`OCEANIC_THICKNESS`, 7 km), at Ridge and oceanic OtherDivergent cells (opening velocity
   v_open = −v_n), half per side as above. 5 cm/yr → ~1.6 mm/yr per cell. Earth: ~20 km³/yr
   (Crisp 1984; White et al. 2006 ~21).
2. **Arcs:** the model grows arc crust behind the trench: `arc_crust_per_subduction` (500 m)
   per subducted cell, spread over `arc_profile` (rings 1 and 2 behind the trench, 110–220 km,
   the arc above the slab's ~100 km depth contour: arc–trench gaps of 150–250 km, slab depth
   below arcs 65–130 km, England et al. 2004; Syracuse & Abers 2006). The view uses the same:
   supply per trench length v_n · `arc_crust_per_subduction` (5 cm/yr → 25 km³ km⁻¹ Myr⁻¹; Earth
   20–40, Reymer & Schubert 1984; up to ~90 for intra-oceanic arcs, Dimalanta et al. 2002),
   spread from each Overriding boundary cell over the rings of the overriding plate with the
   `arc_profile` weights (normalised), not across a neighbouring reversed segment (approximated:
   only cells of the overriding plate). Not at diffuse (non-localised) zones or collisions,
   where the model grows no arc either. ~0.15 mm/yr per arc cell. Earth: ~3–8 km³/yr.
3. **Hotspots (plumes):** every plume of `MantleFlow::plumes` supplies
   Q = q · flux · Λ, q = 10⁸ km³ Myr⁻¹ per m² s⁻¹, with the lid factor Λ = ((Z − z_L)/Z)²,
   Z = 180 km, z_L = min(2.32 √(κ t), 95 km) for oceanic lithosphere of age t, 130 km for
   lithosphere of continental origin (decompression melting stops at the lid; White & McKenzie
   1989; Ellam 1992), spread as a Gaussian of σ = 150 km cut at 400 km, normalised over the
   cells. These are the constants of #106's plan for its plume tails, so that the view matches
   what #106 will emplace. ~0.1 km³/yr per plume under old ocean (Hawaii 0.01–0.2, Iceland
   0.1–0.2). Earth's intraplate total ~1–2.5 km³/yr.
4. **Continental rifts:** v_open · h · f_rift at Rift and continental OtherDivergent cells, f_rift
   = 0.1: rifting continental lithosphere melts far less than a ridge until it has thinned by a
   factor of ~2–3 (McKenzie & Bickle 1988); the Kenya rift erupted ~10⁵ km³ over ~30 Myr and
   ~1000 km (~3 km³ km⁻¹ Myr⁻¹ against 35 for a ridge opening at its rate). ~0.02 mm/yr.

The dominant source per cell is the largest of the four. LIPs (plume heads) are #106's: it
adds a record of LIPs (time, position, volume), and a later change can add a fifth source,
"large igneous province", that fades over a few Myr after each LIP, and replace the plume
term by #106's actual tail volumes. Nothing in this PR depends on #106.

#196's degassing is global (mol/yr per source, from global tectonic totals), not per cell; it is
not used.

## 3. Data and frames

- `tectonics/activity.rs`: `Simulation::activity(&BoundaryMap) -> Activity` with per cell
  ṁ (N·m yr⁻¹ km⁻²), the magma supply (mm/yr) and their dominant sources. `BoundaryMap` keeps
  the convergence and strike-slip rates (cm/yr) of the boundary that decided each cell's class.
- `CellInfo` gets `seismicity: f32`, `magma_supply: f32` and `activity: u32` (sources: 4 bits
  seismic, 4 bits volcanic): 164 bytes per cell.
- Codec v15: two new 1-byte logarithmic channels (a new `Code::Decades { low, decades }`: 0 =
  zero or below `low`, 1–254 log-spaced, 255 NaN; ṁ 10⁶–10¹⁸ (0.047 decades per step),
  magma 10⁻⁶–10² mm/yr (0.032)) and one `Integer8` for the sources: **+3 bytes per cell**
  (58 → 61), and less after the delta and lz4 (nonzero only at boundaries and plumes).
- `GlobalStatus`: the global seismic moment rate (N·m/yr) and magma supply (km³/yr), shown in
  the "Simulation and planet" panel next to the plate statistics (outputs go there, not into the
  parameter panel; #218). +8 bytes per frame header.

## 4. Render mode "Tectonic activity" (`--render-mode tectonic-activity`)

- Option "Show": **Earthquakes** (default) / **Volcanism** / **Both**; switch "Plate boundaries"
  (the Plates mode's lines, default off).
- Earthquakes: ṁ on a continuous sequential log scale (YlOrRd: pale yellow → orange → red →
  dark red), ticks every two decades. Cells without activity: neutral pale greys, darker for the
  ocean than for land, so the geography stays readable; coastlines drawn. Unlit, so the colours
  match the legend.
- Volcanism: magma supply on its own log scale (PuRd: pale lilac → magenta → dark red, apart from
  the earthquake colours), 10⁻³–10 mm/yr.
- Both: the earthquake colours, plus volcano markers (small triangles with a dark border) at
  volcanic cells above 10⁻² mm/yr, thinned to one per ~1.5 arrow spacings, coloured by the
  dominant volcanic source (Okabe–Ito: arc, ridge, hotspot, rift). Legend lists both.
- Legend: description (what is estimated, from what; "long-term rates, not single events"),
  the colour scale with unit, the source swatches.
- Cell widget: two new rows, "Earthquakes" (e.g. "subduction, 1.2·10¹⁵ N·m/yr/km², ≈ Mw 8.1 per
  century") and "Volcanism" (e.g. "arc, 0.15 mm/yr").
- Follows the mode registry and the UI rules: same UI for desktop and web, the switcher's touch
  sizes and fonts as for the other modes, nothing new on the parameter panel, plate colours
  untouched, no month slider, no egui_kittest tests.

## 5. Tests

- Unit: B&K formula numbers (subduction 6 cm/yr per length), class mapping (two-plate splits as in
  `display.rs` tests: converging oceanic → convergent, sliding → transform with OTF coupling,
  opening → ridge); plume Gaussian conserves Q; codec round trip of the new channels (zero stays
  zero, relative error ≤ 6 %); global sums; arc supply sits behind the trench on the overriding
  plate.
- Hash: a short headless run on base and branch, identical state hashes.
- Visual: screenshots desktop and web phone-size (390×844) of the three options and the
  widget, compared qualitatively with Earth's seismicity (Ring of Fire, Alpine–Himalayan belt,
  ridges) and volcano maps.

## 6. Literature

- Bird, P. & Kagan, Y. Y. (2004): Plate-tectonic analysis of shallow seismicity. BSSA 94, 2380.
- Kostrov, V. V. (1974): Seismic moment and energy of earthquakes, and seismic flow of rock.
- Savage, J. C. & Simpson, R. W. (1997): Surface strain accumulation and the seismic moment
  tensor. BSSA 87, 1345.
- Kagan, Y. Y. (2002): Seismic moment distribution revisited. GJI 148/149.
- Hanks, T. C. & Kanamori, H. (1979): A moment magnitude scale. JGR 84, 2348.
- Crisp, J. A. (1984): Rates of magma emplacement and volcanic output. JVGR 20, 177.
- White, S. M., Crisp, J. A. & Spera, F. J. (2006): Long-term volumetric eruption rates and
  magma budgets. G³ 7, Q03010.
- Reymer, A. & Schubert, G. (1984): Phanerozoic addition rates to the continental crust.
  Tectonics 3, 63. Dimalanta, C. et al. (2002): New rates of western Pacific island arc magmatism.
  Tectonophysics 352.
- England, P., Engdahl, R. & Thatcher, W. (2004): Systematic variation in the depths of slabs
  beneath arc volcanoes. GJI 156, 377. Syracuse, E. M. & Abers, G. A. (2006): G³ 7, Q05017.
- White, R. & McKenzie, D. (1989): Magmatism at rift zones. JGR 94, 7685. McKenzie, D. & Bickle,
  M. J. (1988): J. Petrol. 29, 625. Ellam, R. M. (1992): Geology 20, 153.

## Response to the plan review ([plan-review.md](plan-review.md))

- **Ridge numbers (must-fix):** corrected above (~2 × 10¹² N·m yr⁻¹ km⁻², Mw ≈ 6).
- **Signs (must-fix):** convergent classes (Subducting, Overriding, Collision, Diffuse,
  OtherConvergent) whose plates move slightly apart (slab hysteresis) get no opening faults and
  no magma; moment rates use |v|, magma max(−v_n, 0), so nothing goes negative.
- **Boundary vs distributed (must-fix):** done as suggested: each strained cell is classified by
  its area change (shrinking by more than half the strain rate: shortening; growing by as much:
  stretching; else shear) and uses that style's c z and dip factor, ṁ = μ (c z) ε̇ g(δ), with ε̇
  the sheet's strain-rate invariant standing in for the largest principal rate (within ~√2;
  Savage & Simpson's measure). A band taking up a convergence v over its width W then releases
  what the boundary would (μ c z v g(δ) per length).
- **Strain-rate floor (should):** 10⁻¹⁷ s⁻¹; the colour scale starts at 10¹⁰ N·m yr⁻¹ km⁻².
- **Arcs at diffuse zones (should):** left out. The model adds arc crust there only for overlaps
  that the last velocity solve did not see converging (new segments, resampling noise), which the
  simulation counts separately as `diffuse_subducted`; they are not a standing arc.
- **Model arc total (should):** ~2–2.5 km³/yr in a first check (Earth 3–8 km³/yr, incl.
  intrusions); stated in the results.
- **Ridges in "Both" (should):** left out of the volcano markers; ridges show in the colours.
- **Transform speed classes (should):** B&K's c z for oceanic transforms falls 7× between their
  slow and medium classes (4 cm/yr); kept as published, noted as a caveat (a sharp step in the
  map at 4 cm/yr).
- **Several boundaries per cell (should):** not done: a cell at a triple junction keeps the
  motion of the boundary that decides its class. Sub-grid (#64); stated as a caveat.
- **Lid factor at the plume centre (should):** done (the cell nearest the plume).
- **Sizes (should):** `CellInfo` 152 → 164 bytes; frames 58 → 61 bytes per cell (the base had 58,
  not 57). Global totals are summed in f64 from the unquantised cells.
- **Nice-to-haves:** the existing `boundary_map()` of `cell_infos` is passed to `activity()`;
  Mw is labelled per century per 100 × 100 km; marker size follows the arrow spacing (zoom) as the
  other overlays. The reversed-segment rule for arcs is approximated by staying on the overriding
  plate.

## Addendum: eruption rate and LIPs (#222 review, Sebastian: "an additional eruption rate view and number")

Decided: the magma supply (intrusions included) stays; an **eruption rate** is added next to it.

- Eruption rate = magma supply × the erupted share of its setting, 1 / (1 + I/E) with the
  intrusive-to-extrusive ratio I/E (Crisp 1984; White, Crisp & Spera 2006, G³ 7, Q03010):
  ridges I/E ≈ 6 (oceanic crust is ~1 km of lavas over ~6 km of dikes and gabbro: 1/7), arcs
  I/E ≈ 5 (1/6), oceanic hotspots I/E ≈ 2 (1/3), continental settings (hotspots under
  continents, rifts) I/E ≈ 10 (1/11), large igneous provinces mostly extrusive, I/E ≈ 1 assumed
  for a province with its sills and underplate (1/2). Earth: ~20 km³/yr at ridges and ~5 at arcs
  give ~3.7 km³/yr with the hotspots, within Earth's 3–4 km³/yr of eruptions (most of it unseen
  on the sea floor).
- Plumes now read #221's volcanism (in main): the tail of every plume past `lip_age` with
  `volcanism.tail_volume_flux`, `tail_width` and its `lid_factor` (the same numbers the model
  emplaces; nothing when `volcanism.enabled=0`), and a fifth source, **large igneous
  province**, from the LIP record: each LIP's volume over a pulse of 3 Myr after its time
  (LIPs emplace ≥ 75 % of their volume within 1–5 Myr, Bryan & Ernst 2008), on its emplacement
  footprint (σ = max(lip_width, √(V / (2π · lip_plateau_thickness)))).
- Frames: one more 1-byte logarithmic channel (eruptions) and the source field grows to 16 bits
  (12 used: the eruption source differs from the magma source where a hotspot sits on a ridge):
  61 → 63 bytes per cell; `CellInfo` 164 → 168 bytes; the global record +4 bytes.
- UI: "Eruptions" in the mode's Show choice (Earthquakes / Volcanism / Eruptions / Both), on
  the magma colours shifted one decade down; an "Eruptions" row in the cell widget; a global
  "Eruptions" number with a sparkline in the Simulation and planet panel; LIP markers in "Both".
