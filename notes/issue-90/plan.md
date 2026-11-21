# #90 Plan: a second phase for the evolution and spread of life

Design plan only, no code yet. Sources and the code survey are in [research.md](research.md);
the independent review is in [plan-review.md](plan-review.md) (19 findings, 6 major); all
major and most minor points are folded in below (marked "(review)").

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
| What marks the switch? | **Colonisation of land by plants** (with complex marine life on shelves) — the first point where life changes the physics the model already resolves (albedo, evapotranspiration, root cohesion, weathering). On Earth ~0.5–0.47 Ga after ~4 Gyr. The model has no O₂ cycle and (with the default compensating greenhouse) the planet's age barely changes its climate, so the trigger is a **run-time parameter** (`--life-start`, e.g. 300 Myr: after the tectonic/climate spin-up), gated by "no snowball"; the planet age is a label until CO₂ is modelled (review). The date is a choice, not a prediction (research §1). |
| Freeze the continents? | **No — keep the existing 1-Myr step and run the biosphere inside it.** Vegetation equilibrates with climate in ≪ 1 Myr and speciation takes ~0.1–few Myr, so kyr substeps buy nothing while orbital cycles are not modelled. Freezing would remove the main driver of diversification (changing geography) and costs barely less once the biosphere runs at 1 Myr. Offer `--freeze-tectonics` only as a cheap option for experiments. |
| Default or opt-in? | **Opt-in first** (`--life-start`): a two-phase run needs `--biosphere abiotic` land in phase 1, which changes every default run (2.1 K colder, 35% less land rain). Default once validated (review). |
| Level of detail | **MVP = level 2 (ecosystem function)**: plant functional types with bioclimatic limits, fractional cover, NPP, biomes as a display classification, coupled to land albedo, Budyko ω and erodibility per cell — the generalisation of today's global `--biosphere` preset. **Level 3 (abstract lineages à la gen3sis/SPLIT)** on top as stage 2; **carbon cycle (weathering ↔ CO₂)** as stage 3. |
| Runtime | Measured: 0.38 s per 1-Myr step early in a run (tectonics 22%, climate 47%, erosion 31%), 0.36–0.65 s later (#78). Level 2: +5–20 ms per step (+1–5%). Level 3: ~130–260 ns per occupied (cell, lineage) pair: 0.5 M pairs ≈ +0.1 s, 1.5 M pairs ≈ +0.2–0.4 s per step; caps set from a prototype measurement. Freezing would save only ~half a step. See §4. |
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

A run is one continuous time axis. The CLI gains `--life-start <Myr of the run>`; the run's
`--duration` covers both phases; the web gets the same as a query parameter. `--life-start`
implies the abiotic land parameters before the switch (phase 1 cannot be `Modern`); without
`--life-start` runs stay exactly as today (opt-in; review). The `Modern` preset remains as the
"fully vegetated, no dynamics" shortcut.

### 2.2 The trigger

Options considered:

| Option | Pros | Cons |
| --- | --- | --- |
| A. Fixed run time (e.g. after phase-1 duration) | trivial, reproducible | arbitrary |
| B. Planet age (e.g. 4.0 Gyr, tied to `--epoch`) | ties to the faint Sun and to a meaningful "how old is the planet" knob; Earth-calibrated default with a stated plausible range (~3–5 Gyr) | still a choice; physically almost inert here (the default greenhouse compensates the faint Sun) and `--epoch 0.55` would fire in the first step, during the cold transient (review) |
| C. Physical gate: no snowball (frozen fraction < ~50%) and habitable shelf area > threshold | uses what the model resolves; mirrors the post-Cryogenian window | thresholds are guesses; a gate alone can fire at once |
| D. O₂ / P box model (COPSE-like) predicting the oxygenation | most "mechanistic" | large new model, poorly constrained, still needs biology timing parameters; out of scope |
| E. Stochastic hazard from habitable area × time | "life finds a way" feel, seed-dependent | pseudo-mechanism, tuning knob in disguise |

**Recommendation: A, with a "no snowball" gate (review).** Phase 2 starts at
`--life-start` Myr of run time, if the frozen fraction is below a threshold (e.g. 50%);
otherwise it waits and logs why. Default suggestion 200–500 Myr: after the transient of the
initial conditions (the abiotic planet is 3–9 K colder in its first ~100 Myr) and once the
tectonic state is "mature". Why not B (planet age): with the default greenhouse forcing, which
exactly compensates the faint Sun, the epoch changes almost nothing in this model, and a planet
age of 4.0 Gyr with `--epoch 0.55` would fire in the first step, during that cold transient. The
planet age stays a label (start epoch + run time) and becomes physical only with an explicit
`--co2` or the carbon cycle (stage 3). The shelf gate of C is dropped for land plants (it
belongs to marine life, stage M7). Advancing the epoch with run time needs a setter on
`TemperatureModel` (its parameters are fixed at construction) and only matters with `--co2` or
CO₂ from the carbon cycle.

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

Colonisation speed does not need substeps either: plants migrate 100–1000 km per kyr
(Clark et al. 1998), so within one Myr step any connected, suitable region is reachable (§3.3).
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
| **2. Ecosystem function** | ~6–8 PFTs (e.g. bryophyte/pioneer crust, herb/grass, tropical evergreen tree, temperate deciduous tree, boreal needleleaf tree, desert shrub, tundra), bioclimatic limits (coldest-month T, GDD, moisture index P/PET), damped fractional cover, flood-fill spread through suitable cells, NPP (Miami/Del Grosso style); biome = display classification | ~32–48 bytes | ~5–20 ms | per-cell land albedo (planetary, half the surface contrast), Budyko ω, erodibility K and sediment diffusivity; NPP → weathering (stage 3) | the physical effect of life, colonisation front, biome maps |
| 3. Evolving lineages | abstract lineages (clades, not species) with traits (thermal optimum/tolerance, aridity tolerance, dispersal, realm), range = set of cells, speciation by range fragmentation, dispersal, competition through a cell carrying capacity ∝ NPP, extinction | sparse occupancy, ~8 bytes × occupied (cell, lineage) | ∝ Σ range sizes (§4) | via PFTs: a lineage belongs to a PFT; innovations (trees, seeds) appear as lineage traits | diversity maps, radiations, extinctions, phylogeny |

**Recommendation:** level 2 as the base (it is what changes the planet and it generalises the
existing preset), level 3 on top for the "evolution" part. Level 3 without level 2 would be
decorative (no feedback); level 2 without level 3 has colonisation and biomes but no evolution.

### 3.3 Level 2 in more detail (MVP)

Per land cell and step, using last step's climate (as the ice sheets already do):

1. Climate drivers from existing fields: annual mean T, coldest/warmest month, growing degree
   days (> 5 °C from `TemperatureFields::monthly`), annual P and PET (water cycle), ice cover.
   Moisture index **P/PET** (or AET/PET computed with a *fixed* ω): the model's AET comes from
   the Budyko curve with ω, and ω will depend on cover, so AET/PET would make bare cells look
   drier than their climate and could block colonisation for no physical reason (review).
2. **Potential cover** of each PFT from bioclimatic limits (BIOME-style, Prentice et al. 1992)
   with smooth edges, competition by a fixed dominance order (trees > shrubs/herbs > crusts where
   allowed) and total cover limited by water (P/PET).
3. **Available PFTs**: in the MVP a timed sequence after the transition as a simple stand-in for
   evolution (pioneers at once, vascular herbs after ~30 Myr, trees after ~60–80 Myr; each delay a
   parameter, Earth: 0.47 → 0.43 → 0.385 Ga). Stage 2 replaces the schedule by lineages
   acquiring the traits.
4. **Spread**: each step a PFT fills, by flood fill from the cells where it exists, every
   *connected* cell where it is suitable — plant migration (100–1000 km/kyr) makes any connected
   region reachable within a Myr; a limit of N rings per step would make the greening time a pure
   function of a tuning knob (review). Barriers (deserts, mountains, ice, ocean) stop it;
   crossing an ocean gap or a barrier happens with a small, distance-dependent probability per
   step from the biosphere RNG. The visible front then comes from PFT availability (the schedule,
   later evolution) and from barriers, not from a speed parameter. This also makes vegetation on
   cells (while plates move) a non-issue.
5. **Cover** moves part-way towards the potential each step (damped, e.g. half the gap) where
   the PFT is present; the damping counters the multi-step climate lag (item 8).
6. **NPP** = Miami-style min(f(T), f(P)) × cover, converted from dry matter to carbon (× ~0.45–0.5)
   and checked per vegetated area (review); **biomass/soil carbon** optional (only needed for the
   carbon cycle).
7. **Coupling back** (replacing the global preset scalars by per-cell values):
   - land albedo: the model's `land_albedo` is a **planetary** (cloud-including) albedo per coarse
     EBM cell, and the presets map a surface contrast of ~0.1 to +0.05 (Donohoe & Battisti 2011).
     The same half rule per PFT: bare 0.37, desert shrub ~0.35, grass ~0.30, forest ~0.26–0.28,
     checked so that a Modern-like cover averages ~0.32 (review: the draft mixed in surface
     values, ~4× too strong). Implemented as "preset value + Σ cover × delta" so that unvegetated
     runs stay bit-identical. No snow masking in the MVP (seasonal snow is not modelled);
   - Budyko (Fu) ω from 1.6 (bare) to ~2.6–3.5 (forest) by cover and rooting depth (Zhang et al.
     2004; Donohue et al. 2012, as in `presets.rs`). Since `erosion.runoff_discharge` is on by
     default, ω also changes river discharge and hence erosion (review; the `presets.rs` remark
     that discharge follows precipitation is stale and should be fixed in M0);
   - erodibility K ×2 (bare) → ×1 (full cover), sediment diffusivity 10⁴ → 5×10³ m²/yr (the
     diffusion substep count must follow the *maximum* per-cell diffusivity);
   - marine: biogenic pelagic sediment switches on with marine productivity (stage 2/3).
8. **Stability**: vegetation and rain feed back on each other (Charney 1975; Kleidon et al. 2000;
   Brovkin et al. 1998). The model's climate itself lags: the water cycle re-solves one of its 4
   months per step and ET uses last year's precipitation, the EBM runs 2 years per step, so the
   loop closes over ~1–4 steps (review). Expect **bistability** (regions locked bare or green,
   as in the Sahara/Sahel and forest/savanna literature; Hirota et al. 2011) more than
   oscillation — that is plausible behaviour, not a bug, if it is not an artefact of the lag.
   Mitigation: damped cover updates (item 5), a log of cells that flip back and forth, and a test
   that the equilibrium does not depend on the start (bare vs. fully green initial cover). An
   "extra climate pass" is not a cheap fallback: 177 ms is the whole climate step and one more
   partial pass does not reach a fixed point. M1 first measures how the 177 ms split between the
   EBM and the water cycle.

The Modern preset then becomes "the equilibrium of phase 2 with all PFTs", and Abiotic "phase 2
before colonisation" — a consistency check: a long phase 2 with all PFTs should come close to
the Modern preset's statistics (global and land temperature, land precipitation and runoff,
arid fraction, sediment flux; 4 seeds, 200 Myr after full greening), within the seed scatter or
with the difference explained (vegetation will be patchier than the uniform preset).

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
  divergence time τ become daughter lineages (gen3sis' divergence counter, merged back when they
  reconnect first). τ of several steps (~2–5 Myr, optionally shorter in warm climates; Allen et
  al. 2002), so that a coastline cell flipping for one step does not split a lineage (review);
  (5) trait
  drift per daughter (small random change) — adaptation by selection emerges from local
  extinction of ill-fitting populations; (6) global extinction when the range is empty.
- **Innovations**: rare trait gains (e.g. "tree", "seed": drought tolerance) unlock PFTs for the
  lineage's descendants — the stage-2 replacement for the timed PFT schedule.
- **Budget**: the cost driver is the number of occupied (cell, lineage) pairs, not the number of
  lineages (review); cap Σ ranges (e.g. ~1 M pairs desktop, ~0.25 M web, final values from the
  M4 prototype measurement). Near the cap the carrying capacity tightens (more competition)
  rather than truncating, so the dynamics stay smooth.
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
- Determinism (#79, #87): a separate `ChaCha8Rng` stream for the biosphere with a seed *derived*
  from the run seed (e.g. hashed with a constant; not the tectonics seed itself), so adding the
  biosphere does not perturb tectonics' RNG; iterate lineages by id and cells by index; daughter
  lineage ids assigned in a defined order (by smallest cell index of the component); float sorts
  with `total_cmp`; no hash-map iteration; transcendental math through `src/math.rs`;
  single-threaded. `state_hash()` adds per-cell cover/PFT and NPP, lineage count and Σ range (and
  CO₂ in stage 3) **only when phase 2 is enabled**, so all existing logged hashes stay valid.
- Cell mapping: vegetation lives on the fixed mesh (cells), not on crust samples; when a plate
  moves, a cell's vegetation does not travel with the crust — fine at Myr steps because
  vegetation re-equilibrates every step; lineage ranges likewise live on cells (a lineage on a
  drifting continent "moves" by re-dispersal each step into the cells the continent now covers;
  with the per-step flood fill of §3.3 and lineage dispersal of ≥ 1 ring per step this covers
  plate speeds up to ~11 cm/yr (review: settled).

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

Baseline below: 0.38 s; later in a run 0.36–0.65 s per step (#78), so 500 Myr of phase 1 or 2
take ~3–5.5 min plus the biosphere. Per occupied (cell, lineage) pair, level 3 needs per step
~13 memory/neighbour operations (suitability 1, dispersal and connected components 6 neighbours
each) at ~10–20 ns → **~130–260 ns per pair** (corrected after review: the draft's per-pair
count was inconsistent).

| Variant | Extra per step | Phase-2 step | 500 Myr of phase 2 |
| --- | --- | --- | --- |
| Level 2, lagged coupling (no extra climate pass) | PFT limits for ~13k land cells × 8 PFTs × 12 months ≈ 1–2 M cheap ops + spread over neighbours ≈ **5–20 ms** | ~0.39 s (+3–5%) | ~3.3 min |
| Level 2 + one extra climate pass per step (only if the lag oscillates) | +177 ms | ~0.56 s (+50%) | ~4.7 min |
| Level 3, 0.4 M occupied pairs (e.g. 2 000 lineages × 200 cells) | **50–100 ms** | ~0.45 s | ~4 min |
| Level 3, 1.5 M pairs (e.g. 5 000 × 300) | **0.2–0.4 s** | ~0.6–0.8 s | ~5–7 min |
| Level 3, 6 M pairs (uncapped growth) | ~0.8–1.6 s | ~1.2–2 s | 10–17 min |
| Carbon cycle (stage 3) | weathering sum over land + 1D root find (~20 evaluations of a cheap sum) ≈ 1–5 ms, **unless** the CO₂ must be iterated with the climate (each iteration +177 ms; the quasi-steady solve uses last step's climate instead) | | |
| Freeze (`--freeze-tectonics`) | − tectonics − erosion ≈ −200 ms | ~0.18 s + biosphere | |

Web: ~2× slower per cell than native and 4× fewer cells at 5 subdivisions; a cap of ~0.25 M
pairs keeps the phase-2 step near today's web step (~0.4 s, #78). Memory: §3.6 (≤ ~50 MB
worst case native, a few MB on the web). The level-3 numbers are estimates from operation counts
(no prototype); a small timing prototype at the start of M4 (synthetic ranges on the real mesh)
must measure them before the caps are fixed.

So freezing saves ~half of a step, not an order of magnitude: the climate, which phase 2 needs
anyway, is the largest part. This is the quantitative reason not to freeze by default.

## 5. Visualisation

- **Viewer and web: a map-layer selector** (today there is one colour scheme). A row of large
  toggle buttons (touch-friendly, ≥ 44 px, per #72) or an egui combo: *Relief* (today), *Biome*,
  *Vegetation cover / NPP*, *Diversity* (lineage richness, stage 2), *Habitability* (phase 1).
  Vertex colours are built today in the simulation thread / Web Worker
  (`viewer::surface_vertices`), which takes no messages back from the UI, so a layer switch must
  recolour on the UI side from the per-cell data it already receives (`CellInfo`) — the layer
  fields must be in `CellInfo` or a second compact per-cell buffer (review). The relief stays as
  the geometry; no new shader work.
- **Relief palette**: today's palette already paints lowlands green (a "modern" look). With
  phase 2, land colour should follow the vegetation (bare: tan/grey rock; grass: olive; forest:
  dark green; desert: sand) and the elevation be shown by shading/hypsometric tint only — a small
  decision for Sebastian, since it changes the look of phase-1 runs too (bare land everywhere).
- **Cell info** (#72): rows for biome, cover by PFT, NPP, number of lineages, dominant lineage;
  `CellInfo` is 56 bytes today (2.3 MB per update at 41k cells); packed biosphere fields
  (dominant PFT + biome as u8s, total cover and NPP as f32 or u16, lineage count, dominant
  lineage id) add ~16 bytes (→ 72 bytes, 2.9 MB); cover *per* PFT and lineage lists are sent on
  demand for the selected cell instead of for every cell (review).
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
| M0 | Phase plumbing | `--life-start` (opt-in), snowball gate, phase in logs/status; per-cell land albedo, ω, K and diffusivity as "preset + delta" in temperature/climate/erosion (zero delta → bit-identical); fix the stale `runoff_discharge` remark in `presets.rs`; measure EBM vs water-cycle time | 1–2 | `state_hash` unchanged for runs without `--life-start`; Abiotic/Modern presets reproduced via per-cell fields |
| M1 | Level 2 MVP | PFTs, potential cover (P/PET), timed PFT schedule, flood-fill spread, damped cover, NPP (carbon), coupling; biome classification; statistics, flip log | 2–3 | colonisation front visible; all-PFT equilibrium close to the Modern preset (§3.3 test, 4 seeds); land NPP 30–80 Pg C/yr; same equilibrium from bare and from green starts (or bistability explained) |
| M2 | Visualisation | layer selector (desktop + web), vegetation tint, cell-info rows, PNG layers | 1–2 | works on phone width and desktop; screenshots in notes |
| M3 | Phase-1 handover layers | shelf/productivity/upwelling/pioneer maps, optional habitability history | 1 | maps plausible (upwelling at eastern boundaries, productive deltas) |
| M4 | Level 3 lineages | timing prototype first; lineage engine, speciation by fragmentation, dispersal, competition, pair cap; diversity layer, lineage view, stats | 4–6 | diversity gradients emerge (tropics > poles, mountains/archipelagos as cradles); runtime within budget |
| M5 | Innovations | trait gains unlock PFTs (replace the timed schedule) | 1–2 | land greening timing emerges with plausible spread |
| M6 | Carbon cycle | weathering–outgassing CO₂ balance, continuity at the switch, snowball guard, epoch advancing with run time (setter on `TemperatureModel`) | 2–4 | greening cools the planet by a plausible few K; no runaway; sensitivity documented |
| M7 | Marine life | shelf/upwelling productivity, reefs, biogenic pelagic sediment as an outcome | 2–3 | later; optional |

M0–M2 make a useful first PR series ("the planet greens"); M4 is the evolution part.

## 7. Risks

- **Feedback instability / bistability**: vegetation ↔ rain ↔ albedo can tip or lock in
  (Charney; Brovkin et al. 1998); the climate's own multi-step lag can turn this into flicker;
  damped cover and a flip log (§3.3 item 8).
- **Default-run change**: two-phase runs need abiotic land in phase 1; kept opt-in until
  validated.
- **Snowball via the carbon cycle** (M6): the EBM is ice-albedo sensitive; the abiotic preset was
  already 2 K colder with tropical ice on high plateaus early on.
- **Cost creep of level 3**: Σ range sizes grow with diversity; the pair cap and carrying
  capacity bound it, the web needs a smaller cap; the per-pair cost is an estimate until the M4
  prototype.
- **False precision**: the timed PFT schedule and the trigger are choices; present them as such
  in the UI and logs.
- **Calibration temptation**: many new parameters (PFT limits, τ, barrier-crossing
  probability); stick to literature values and check only soundness (project convention).
- **Cell-vs-sample mapping**: vegetation and ranges on cells while crust moves; acceptable at Myr
  steps, but lineages on fast plates may need ≥ 1 ring/Myr dispersal to keep up.
- **Climate model limits** shape the biomes: no seasonal snow (boreal snow-masking effect
  missing), simple moisture transport, the tropical-lowland cold bias (#65) shifts biomes; the
  biome map will inherit these.

## 8. Decisions for Sebastian

1. **Transition**: land colonisation, started at a **run time** (`--life-start`, e.g. 300 Myr,
   after the spin-up) and gated by "no snowball" — with the planet age only a label until CO₂ is
   modelled. Or do you prefer a planet-age knob anyway (physically inert today)?
2. **Opt-in or default**: two-phase runs need bare (abiotic) land in phase 1, which changes
   every default run (colder, drier). Recommended: opt-in first, default after validation.
3. **Freeze vs common step**: recommended a common 1-Myr step with tectonics running (freezing
   saves only ~half a step and removes the geography changes that drive diversification), plus
   `--freeze-tectonics` as an experiment option. Or freezing as the default?
4. **Detail level / MVP**: level 2 (PFTs, cover, NPP, coupling) first, lineages second — agreed?
   Lineage budget to be set from a measurement after M1/M4 (order 1 M occupied cell–lineage
   pairs desktop, 0.25 M web).
5. **Coupling**: albedo + ω + erodibility in the MVP; carbon cycle (CO₂ from weathering replacing
   the prescribed forcing, which lets greening cool the planet) as stage 3 — or not at all?
6. **Visualisation**: layer selector + land coloured by vegetation (which also changes the look
   of phase 1: bare rock instead of green lowlands); which layers matter most (biome, NPP,
   diversity, lineage ranges)?
7. **Marine life**: shelves/reefs as part of level 3, or land only at first?
