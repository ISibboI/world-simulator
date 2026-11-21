# #90 Plan: a second phase for the evolution and spread of life

Design plan only, no code yet. Sources and the code survey are in [research.md](research.md);
the independent review is in [plan-review.md](plan-review.md) and is folded in below (marked
"(review)").

**Goal of the project, applied here:** an **Earth-like planet, not Earth**. Phase 2 should be
physically and biologically *plausible* (processes, conservation, plausible ranges: land NPP of
order 50 Pg C/yr, forests cooling the tropics and warming the boreal zone, vegetation raising land
rain by tens of percent, diversity highest in warm, wet, topographically varied, long-stable
regions), not a replay of Earth's history. Grid effects (~110 km cells, ~220 km on the web) are
accepted (#64): no microhabitats, islands smaller than a cell disappear, speciation can only be
allopatric at ≥ ~100 km scale.

## 1. Summary of the recommendation

| Question | Recommendation |
| --- | --- |
| What marks the switch? | **Colonisation of land by plants** (with complex marine life on shelves) — the first point where life changes the physics the model already resolves (albedo, evapotranspiration, root cohesion, weathering). On Earth ~0.5–0.47 Ga; here a **planet-age parameter** (default: planet age ≈ 4.0 Gyr, i.e. `--epoch` 0.55), **gated by "no snowball"** and enough habitable shelf. No O₂ model: the date is a choice, not a prediction (research §1). |
| Freeze the continents? | **No — keep the existing 1-Myr step and run the biosphere inside it.** Vegetation equilibrates with climate in ≪ 1 Myr and speciation takes ~0.1–few Myr, so kyr substeps buy nothing while orbital cycles are not modelled. Freezing would remove the main driver of diversification (changing geography) and costs barely less once the biosphere runs at 1 Myr. Offer `--freeze-tectonics` only as a cheap option for experiments. |
| Level of detail | **MVP = level 2 (ecosystem function)**: plant functional types with bioclimatic limits, fractional cover, NPP, biomes as a display classification, coupled to land albedo, Budyko ω and erodibility per cell — the generalisation of today's global `--biosphere` preset. **Level 3 (abstract lineages à la gen3sis/SPLIT)** on top as stage 2; **carbon cycle (weathering ↔ CO₂)** as stage 3. |
| Runtime | Measured today: 0.38 s per 1-Myr step (tectonics 22%, climate 47%, erosion 31%). Level 2: +3–5% per step (vegetation ~5–20 ms; +50% only if an extra climate pass is needed). Level 3 with a capped ~5 000 lineages: +0.1–0.4 s per step, i.e. phase-2 steps ~1.3–2× the phase-1 cost. See §4. |
| Visualisation | A **map-layer selector** in the 3D view and the web (relief, biome, NPP/cover, diversity, a selected lineage's range), extra cell-info rows, PNG snapshots for biome, NPP and diversity. |

## 2. Phases and the transition

### 2.1 What the phases are

- **Phase 1** (as today): tectonics, climate, erosion, from the start of the run. Life is
  microbial and acts in bulk: `Biosphere::Abiotic` land parameters (bare land, braided rivers,
  no biogenic ooze), prescribed greenhouse forcing of the epoch. Phase 1 additionally *diagnoses*
  where early life lives (§2.3) — cheap, display-only, no feedback.
- **Phase 2**: everything of phase 1 keeps running; a `Biosphere` module is added to
  `Planet::step`. Land starts bare and is colonised from wet coasts and river margins; the land
  parameters move cell by cell from the abiotic to vegetated values as cover grows.

A run is one continuous time axis. The CLI gains e.g. `--life-start <planet age in Gyr or Myr of
the run>` and the run's `--duration` covers both phases (alternatively `--phase2-duration`); the
web gets the same as query parameters.

### 2.2 The trigger

Options considered:

| Option | Pros | Cons |
| --- | --- | --- |
| A. Fixed run time (e.g. after phase-1 duration) | trivial, reproducible | arbitrary |
| B. Planet age (e.g. 4.0 Gyr, tied to `--epoch`) | ties to the faint Sun and to a meaningful "how old is the planet" knob; Earth-calibrated default with a stated plausible range (~3–5 Gyr) | still a choice |
| C. Physical gate: no snowball (frozen fraction < ~50%) and habitable shelf area > threshold | uses what the model resolves; mirrors the post-Cryogenian window | thresholds are guesses; a gate alone can fire at once |
| D. O₂ / P box model (COPSE-like) predicting the oxygenation | most "mechanistic" | large new model, poorly constrained, still needs biology timing parameters; out of scope |
| E. Stochastic hazard from habitable area × time | "life finds a way" feel, seed-dependent | pseudo-mechanism, tuning knob in disguise |

**Recommendation: B with the C gate.** Phase 2 starts at the configured planet age if the
climate is not in a snowball and there is shallow, ice-free shelf; otherwise it waits until it is
(and logs why). The epoch then advances with simulated time in phase 2 (solar constant rising by
~1% per 100 Myr, Gough 1981), which matters over a few hundred Myr; phase 1 keeps its fixed epoch
as today unless the epoch is made time-dependent too (small, separate change).

Why land colonisation and not earlier (eukaryotes, Ediacara) or later (forests): earlier stages
change nothing the model can resolve except as bulk parameters (already there); later stages are
*outcomes* phase 2 should produce (trees as a PFT that appears by evolution or by a timed
"innovation", stage 2). Starting at land colonisation lets phase 2 show the most visible
transition in a planet's history: bare continents greening over ~50–100 Myr, rivers changing
style, rain spreading inland, the planet cooling as weathering rises (stage 3).

### 2.3 What phase 1 hands over

At the transition (and as optional display layers during phase 1):

- **Marine habitat**: shelf cells (0 to −200 m), sea-surface temperature, ice-free;
  **productivity proxy** = nutrient input from rivers (sediment and discharge per mouth, already
  computed by erosion) + upwelling (Ekman divergence from `surface_wind_stress` and coast
  orientation: eastern boundaries in the trades, equatorial divergence, polar fronts).
- **Land habitat for pioneers**: ice-free cells with runoff > ~0.1 m/yr or on a river with
  discharge above the river threshold, near the coast (land plants descend from freshwater algae:
  colonisation starts at wet margins, deltas, river banks).
- **Initial lineages** (stage 2): a few founder lineages seeded in the most productive shelf
  regions (marine) and the pioneer land cells (land), each with traits drawn around the local
  climate.
- Optional **habitability history**: time-integral of the habitability during phase 1 ("where
  early life persisted"), shown as a layer. No feedback.

## 3. Design options

### 3.1 Freezing vs a common step

| | Freeze continents (Sebastian's idea) | Decoupled steps (tectonics 1 Myr, biosphere kyr) | **Common 1-Myr step (recommended)** |
| --- | --- | --- | --- |
| Physics | fine for ≲ 10–20 Myr (plates move ~5 cm/yr → 500 km = 4–5 cells per 10 Myr); wrong for 100+ Myr: no new barriers, no mountain uplift, climate static | most general | adequate: vegetation equilibrium ≪ 1 Myr; speciation ~Myr; weathering–CO₂ balance ~0.1–1 Myr |
| Evolution | static landscape → quick speciation–extinction equilibrium, little structure (research §2, finding c) | — | changing geography drives vicariance, as in gen3sis/SPLIT/Leprieur |
| Cost per Myr | climate + biosphere (~45–60% of today's step) | climate × N substeps unless climate is frozen too; kyr steps of a lineage model ×1000 | today + biosphere |
| Code | a flag to skip tectonics and erosion | a second scheduler, interpolation of fields | biosphere call in `Planet::step` |

Kyr substeps would only matter with kyr-scale forcing (Milankovitch cycles, not modelled) or
for transient vegetation dynamics (succession after disturbance, decades–centuries), neither of
which the 1-Myr-averaged climate represents. If Milankovitch forcing is ever added, the
biosphere can get substeps then.

A frozen mode stays useful as `--freeze-tectonics` (skip tectonics and erosion, keep climate and
biosphere): faster experiments with the biosphere on a fixed map, and Sebastian's original
picture is still available.

### 3.2 Detail levels

| Level | What | State per cell | Cost | Coupling | Value |
| --- | --- | --- | --- | --- | --- |
| 1. Biomes from climate | Whittaker/Köppen lookup from T, P | 1 byte | ~1 ms | albedo/ω per biome | maps; no dynamics, no colonisation |
| **2. Ecosystem function** | ~6–8 PFTs (e.g. bryophyte/pioneer crust, herb/grass, tropical evergreen tree, temperate deciduous tree, boreal needleleaf tree, desert shrub, tundra), bioclimatic limits (coldest-month T, GDD, moisture α = AET/PET), fractional cover relaxing towards equilibrium with a spread rate, NPP (Miami/Del Grosso style); biome = display classification | ~32–48 bytes | ~5–20 ms | per-cell land albedo (incl. snow masking by trees), Budyko ω, erodibility K and sediment diffusivity; NPP → weathering (stage 3) | the physical effect of life, colonisation front, biome maps |
| 3. Evolving lineages | abstract lineages (clades, not species) with traits (thermal optimum/tolerance, aridity tolerance, dispersal, realm), range = set of cells, speciation by range fragmentation, dispersal, competition through a cell carrying capacity ∝ NPP, extinction | sparse occupancy, ~8 bytes × occupied (cell, lineage) | ∝ Σ range sizes (§4) | via PFTs: a lineage belongs to a PFT; innovations (trees, seeds) appear as lineage traits | diversity maps, radiations, extinctions, phylogeny |

**Recommendation:** level 2 as the base (it is what changes the planet and it generalises the
existing preset), level 3 on top for the "evolution" part. Level 3 without level 2 would be
decorative (no feedback); level 2 without level 3 has colonisation and biomes but no evolution.

### 3.3 Level 2 in more detail (MVP)

Per land cell and step, using last step's climate (as the ice sheets already do):

1. Climate drivers from existing fields: annual mean T, coldest/warmest month, growing degree
   days (> 5 °C from `TemperatureFields::monthly`), annual P, PET and AET (water cycle), moisture
   index α = AET/PET, ice cover.
2. **Potential cover** of each PFT from bioclimatic limits (BIOME-style, Prentice et al. 1992)
   with smooth edges, competition by a fixed dominance order (trees > shrubs/herbs > crusts where
   allowed) and total cover limited by water (α).
3. **Available PFTs**: in the MVP a timed sequence after the transition as a simple stand-in for
   evolution (pioneers at once, vascular herbs after ~30 Myr, trees after ~60–80 Myr; each delay a
   parameter, Earth: 0.47 → 0.43 → 0.385 Ga). Stage 2 replaces the schedule by lineages
   acquiring the traits.
4. **Spread**: a PFT colonises a cell only from a neighbour where it exists, with a spread rate
   (cells per Myr; plant migration is km/yr, so in practice limited by the step: up to N rings per
   step, N a parameter ~3–10). This produces the visible colonisation front and the role of
   barriers (deserts, mountains, oceans: transoceanic dispersal with a small probability per step).
5. **Cover** relaxes towards the potential within one step (equilibrium) where the PFT is
   present.
6. **NPP** = Miami-style min(f(T), f(P)) × cover; **biomass/soil carbon** optional (only needed
   for the carbon cycle).
7. **Coupling back** (replacing the global preset scalars by per-cell values):
   - land albedo = mix of bare (0.37), vegetated by PFT (forest 0.13–0.17, grass 0.2, desert shrub
     0.3) and snow-covered: forests mask snow (seasonal snow is not modelled — only ice sheets —
     so this is limited to the ice-sheet margin; keep simple);
   - Budyko ω from 1.6 (bare) to ~2.6–3.5 (forest) by cover and rooting depth (Zhang et al. 2004;
     Donohue et al. 2012, as in `presets.rs`);
   - erodibility K ×2 (bare) → ×1 (full cover), sediment diffusivity 10⁴ → 5×10³ m²/yr;
   - marine: biogenic pelagic sediment switches on with marine productivity (stage 2/3).
8. **Stability**: vegetation and rain feed back on each other (Charney/Kleidon). Using last step's
   climate for this step's vegetation (lagged by one Myr step) is stable and cheap; a within-step
   fixed-point iteration (vegetation ↔ water cycle, 2 passes) only if the lag shows oscillations.

The Modern preset then becomes "the equilibrium of phase 2 with all PFTs", and Abiotic "phase 2
before colonisation" — a consistency check: a long phase 2 with all PFTs should reproduce the
Modern preset's climate statistics within the scatter (§6).

### 3.4 Level 3 in more detail (stage 2)

Following gen3sis (Hagen et al. 2021) and SPLIT (Descombes et al. 2018), adapted for Myr steps and
a fixed budget:

- **Lineage** = abstract clade-level unit (think "a genus-group"), not a species: Earth's millions
  of species are neither affordable nor resolvable at 110 km. Record: id, parent, birth time,
  realm (marine shelf / land), PFT (for land plants) or "fauna", traits (thermal optimum and
  breadth, aridity tolerance, dispersal rings), and per occupied cell an abundance (u8/u16) and a
  cluster id.
- **Per step**: (1) suitability of each occupied and neighbouring cell from traits and climate;
  (2) dispersal: grow the range into suitable cells within the dispersal distance (deterministic
  rings + rare long-distance jumps from the seeded RNG); (3) local extinction where unsuitable or
  outcompeted; cell carrying capacity ∝ NPP (land) or productivity (shelf) shared among lineages
  (diversity-dependence, keeps the count bounded); (4) **speciation**: connected components of
  the range (cells within dispersal distance); components separated for longer than a
  divergence time τ (~1–3 Myr) become daughter lineages (gen3sis' divergence counter); (5) trait
  drift per daughter (small random change) — adaptation by selection emerges from local
  extinction of ill-fitting populations; (6) global extinction when the range is empty.
- **Innovations**: rare trait gains (e.g. "tree", "seed": drought tolerance) unlock PFTs for the
  lineage's descendants — the stage-2 replacement for the timed PFT schedule.
- **Budget**: hard caps on live lineages (e.g. 5 000 desktop, 2 000 web) and on Σ ranges; when
  exceeded, the carrying capacity tightens (more competition) rather than truncation, so the
  dynamics stay smooth.
- **Mass extinctions**: emerge from rapid climate change (snowball, CO₂ swings); later hook for
  #13 (impacts).

### 3.5 Carbon cycle (stage 3)

Global CO₂ box in quasi-steady state per step (weathering ↔ outgassing rebalance in ~0.1–1 Myr):
silicate weathering W = W₀ · f(runoff) · exp(ΔT/T_e) · f(erosion supply) · f_plant (GEOCARB/
COPSE form; f_plant 0.25 bare → 1 vegetated, Moulton & Berner; Lenton et al. 2012), outgassing
from ridge production and subduction rate (both in the tectonic statistics). Solve the CO₂ for
which W = outgassing each step and set `greenhouse_forcing` = 5.35 ln(C/C₀). This replaces the
prescribed epoch forcing and makes the planet cool when it greens — the Ordovician/Devonian
glaciations as an emergent, not scripted, outcome. Risk: it can push the planet into a snowball
(the EBM's ice-albedo feedback is strong); needs a guard and a sensitivity study. It also makes
phase 1 dependent on calibrating W₀ against the prescribed forcing at the transition (continuity
at the switch: choose W₀ so that the CO₂ at the transition equals the prescribed forcing).

### 3.6 State, memory, determinism

- Level 2: ~40 bytes per cell → 1.6 MB at 41k cells (0.4 MB at 10k web cells).
- Level 3: occupancy as a flat `Vec<(lineage: u32, abundance: u16, cluster: u16)>` per cell or
  per lineage (CSR-style arrays rebuilt each step), ~8 bytes × occupied pairs: 1–2 M pairs →
  8–16 MB; lineage table + phylogeny of all lineages ever (~10⁵–10⁶ × ~32 bytes) → ≤ 32 MB, prune
  extinct subtrees without living descendants for display, keep counts.
- Determinism (#79, #87): a separate `ChaCha8Rng` stream for the biosphere seeded from the run
  seed (so adding the biosphere does not perturb tectonics' RNG); iterate lineages by id and cells
  by index; no hash-map iteration; transcendental math through `src/math.rs`; single-threaded.
  `state_hash()` adds per-cell cover/PFT and NPP, and lineage count and Σ range (and CO₂ in
  stage 3), so web and `--web-math` runs stay comparable.
- Cell mapping: vegetation lives on the fixed mesh (cells), not on crust samples; when a plate
  moves, a cell's vegetation does not travel with the crust — fine at Myr steps because
  vegetation re-equilibrates every step; lineage ranges likewise live on cells (a lineage on a
  drifting continent "moves" by re-dispersal each step into the cells the continent now covers;
  a dispersal distance ≥ 1 ring per Myr covers plate speeds up to ~11 cm/yr). (review: check)

## 4. Runtime estimates

Measured phase-1 cost (this task, see §4.1) and estimated phase-2 additions, native, 6
subdivisions (41k cells, ~12–15k land cells, ~3–5k shelf cells); web ~2× slower at 5 subdivisions
(4× fewer cells).

### 4.1 Measured phase-1 cost

`main` at 67ca0a2, release build, seed 1, 6 subdivisions, `--headless`, steps 6–30 of a 30-Myr
run (`RUST_LOG=world_simulator=debug`, phase timings of `Planet::step`), one core on tktie with
other jobs running:

| Part | ms per 1-Myr step | share |
| --- | --- | --- |
| tectonics | 82 | 22% |
| climate (temperature EBM + water cycle) | 177 | 47% |
| erosion | 117 | 31% |
| **total** | **376** | |

(Later in a run, with more plates/sediment, steps are 0.36–0.65 s; #78.) At that time the planet
had ~2.6k shelf cells (6.4% of the surface) and roughly 12k land cells.

### 4.2 Estimates for phase 2 (native, 6 subdivisions)

| Variant | Extra per step | Phase-2 step | 500 Myr of phase 2 |
| --- | --- | --- | --- |
| Level 2, lagged coupling (no extra climate pass) | PFT limits for ~13k land cells × 8 PFTs × 12 months ≈ 1–2 M cheap ops + spread over neighbours ≈ **5–20 ms** | ~0.39 s (+3–5%) | ~3.3 min |
| Level 2 + one extra climate pass per step (only if the lag oscillates) | +177 ms | ~0.56 s (+50%) | ~4.7 min |
| Level 3, 2 000 lineages, mean range 200 cells (0.4 M occupied pairs) | suitability + dispersal (6 neighbours) + connected components per lineage with a reused 41k marker array ≈ 10–20 ops × 15 ns per pair-neighbour ≈ **40–120 ms** | ~0.45 s | ~4 min |
| Level 3, 5 000 lineages, mean range 300 cells (1.5 M pairs) | **150–400 ms** | ~0.55–0.8 s | ~5–7 min |
| Level 3, 20 000 lineages (uncapped growth) | ~0.6–1.6 s | ~1–2 s | 8–17 min |
| Carbon cycle (stage 3) | weathering sum over land + 1D root find (~20 evaluations of a cheap sum) ≈ 1–5 ms, **unless** the CO₂ must be iterated with the climate (each iteration +177 ms; the quasi-steady solve uses last step's climate instead) | | |
| Freeze (`--freeze-tectonics`) | − tectonics − erosion ≈ −200 ms | ~0.18 s + biosphere | |

Web: ~2× slower per cell than native and 4× fewer cells at 5 subdivisions; level-3 caps of ~2 000
lineages keep the phase-2 step near today's web step (~0.4 s, #78). Memory: §3.6 (≤ ~50 MB
worst case native, a few MB on the web). The level-3 numbers are estimates from operation counts
(no prototype); M4 must measure them early and set the caps from the measurement.

So freezing saves ~half of a step, not an order of magnitude: the climate, which phase 2 needs
anyway, is the largest part. This is the quantitative reason not to freeze by default.

## 5. Visualisation

- **Viewer and web: a map-layer selector** (today there is one colour scheme). A row of large
  toggle buttons (touch-friendly, ≥ 44 px, per #72) or an egui combo: *Relief* (today), *Biome*,
  *Vegetation cover / NPP*, *Diversity* (lineage richness, stage 2), *Habitability* (phase 1).
  Surface colour comes from a per-cell value + palette chosen in the vertex build
  (`viewer::surface_vertices`), so layers need no new shader work; the relief stays as the
  geometry. In *Relief* mode, vegetation tints the land (bare: tan/grey rock; grass: olive;
  forest: dark green) — that alone makes colonisation visible without switching layers.
- **Cell info** (#72): rows for biome, cover by PFT, NPP, number of lineages, dominant lineage;
  `CellInfo` grows by ~16 bytes (56 → 72 bytes, 2.3 → 2.9 MB per update at 41k cells).
- **Lineage view** (stage 2): tapping a cell lists its lineages (by abundance); selecting one
  highlights its range and its relatives' ranges (sister clades in a second colour); a small
  diversity-through-time plot (egui_plot or hand-drawn lines) in the panel.
- **Phase banner**: the status line shows the phase, planet age and the transition time.
- **Snapshots**: `biome_*.png`, `npp_*.png`, `diversity_*.png` (equirectangular, same renderer);
  diversity-through-time and per-step biosphere statistics in the log (land cover, NPP total,
  lineages alive/born/died); stage 2 optionally a Newick dump of the phylogeny.

## 6. Milestones (rough effort, agent-days of work including review and evaluation)

| # | Stage | Content | Effort | Done when |
| --- | --- | --- | --- | --- |
| M0 | Phase plumbing | `--life-start`, trigger + gate, phase in logs/status, epoch advancing in phase 2; per-cell land albedo, ω, K fields in temperature/climate/erosion (equal to the global presets → bit-identical results before the switch) | 1–2 | `state_hash` unchanged for runs without phase 2; Abiotic/Modern presets reproduced via per-cell fields |
| M1 | Level 2 MVP | PFTs, potential cover, timed PFT schedule, spread, NPP, coupling; biome classification; statistics | 2–3 | colonisation front visible; all-PFT equilibrium reproduces the Modern preset statistics within seed scatter (4 seeds); land NPP 30–80 Pg C/yr |
| M2 | Visualisation | layer selector (desktop + web), vegetation tint, cell-info rows, PNG layers | 1–2 | works on phone width and desktop; screenshots in notes |
| M3 | Phase-1 handover layers | shelf/productivity/upwelling/pioneer maps, optional habitability history | 1 | maps plausible (upwelling at eastern boundaries, productive deltas) |
| M4 | Level 3 lineages | lineage engine, speciation by fragmentation, dispersal, competition, caps; diversity layer, lineage view, stats | 4–6 | diversity gradients emerge (tropics > poles, mountains/archipelagos as cradles); runtime within budget |
| M5 | Innovations | trait gains unlock PFTs (replace the timed schedule) | 1–2 | land greening timing emerges with plausible spread |
| M6 | Carbon cycle | weathering–outgassing CO₂ balance, continuity at the switch, snowball guard | 2–4 | greening cools the planet by a plausible few K; no runaway; sensitivity documented |
| M7 | Marine life | shelf/upwelling productivity, reefs, biogenic pelagic sediment as an outcome | 2–3 | later; optional |

M0–M2 make a useful first PR series ("the planet greens"); M4 is the evolution part.

## 7. Risks

- **Feedback instability**: vegetation ↔ rain ↔ albedo can oscillate or tip (Charney); the
  one-step lag damps it; watch for two-state flicker between steps.
- **Snowball via the carbon cycle** (M6): the EBM is ice-albedo sensitive; the abiotic preset was
  already 2 K colder with tropical ice on high plateaus early on.
- **Cost creep of level 3**: Σ range sizes grow with diversity; caps and carrying capacity bound
  it, but the web needs smaller caps.
- **False precision**: the timed PFT schedule and the trigger are choices; present them as such
  in the UI and logs.
- **Calibration temptation**: many new parameters (PFT limits, spread, τ); stick to literature
  values and check only soundness (project convention).
- **Cell-vs-sample mapping**: vegetation and ranges on cells while crust moves; acceptable at Myr
  steps, but lineages on fast plates may need ≥ 1 ring/Myr dispersal to keep up.
- **Climate model limits** shape the biomes: no seasonal snow (boreal snow-masking effect
  missing), simple moisture transport, the tropical-lowland cold bias (#65) shifts biomes; the
  biome map will inherit these.

## 8. Decisions for Sebastian

1. **Transition**: land colonisation at a planet age (default ~4.0 Gyr, gated by no snowball) —
   or a simple "after N Myr of phase 1"? Should phase 1 start at a young planet age so that the
   epoch (faint Sun) advances through the run?
2. **Freeze vs common step**: recommended common 1-Myr step with tectonics running, and
   `--freeze-tectonics` as an experiment option. Or do you want freezing as the default
   (cheaper, but static geography for evolution)?
3. **Detail level / MVP**: level 2 (PFTs, cover, NPP, coupling) first, lineages second — agreed?
   Lineage caps acceptable (≈5 000 desktop, ≈2 000 web)?
4. **Coupling**: albedo + ω + erodibility in the MVP (yes/no); carbon cycle (CO₂ from weathering
   replacing the prescribed forcing) as stage 3 — or not at all?
5. **Visualisation**: layer selector + vegetation tint in the relief view; which layers matter
   most to you (biome, NPP, diversity, lineage ranges)?
6. **Marine life**: shelves/reefs as part of level 3, or land only at first?
