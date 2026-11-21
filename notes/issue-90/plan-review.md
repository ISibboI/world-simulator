# #90 Plan review: the biosphere phase

Independent review of [plan.md](plan.md) and [research.md](research.md) at cec3536. I checked
the code claims against `main` at 67ca0a2 (`src/presets.rs`, `src/planet.rs`,
`src/temperature.rs`, `src/climate.rs`, `src/erosion.rs`, `src/viewer/cell_info.rs`,
`src/viewer/mod.rs`, `src/render.rs`, `src/main.rs`, `src/web.rs`). I did not run builds or
simulations, and only spot-checked the literature. As the project asks, I judge plausibility for
an Earth-like planet, not a match to Earth.

## Verdict

**The direction is sound. Revise it before implementation.** The main choices hold up:
land colonisation as the switch, no freeze, a shared 1-Myr step, and level 2 (ecosystem function)
as the MVP with lineages on top. The treatment of freezing is fair and quantitatively right.
Six problems should be fixed in the plan first:

- the albedo coupling uses the wrong kind of albedo, about 4× too strong;
- the moisture index forms a circular loop with ω;
- the trigger's "planet age" is almost physically inert in this model, and the time model is
  undefined;
- the change to the default phase 1 (Modern → Abiotic) is not raised as a decision;
- the colonisation front is driven by a disguised tuning knob;
- the stability argument assumes a one-step climate lag, but the code has a multi-step lag.

Most of the code claims are correct (details in finding 9).

## Findings

### 1. Land albedo: the model uses planetary albedo, the plan proposes surface values (major)

`TemperatureParams::land_albedo` is a **planetary albedo including clouds** (doc comment at
`temperature.rs:182`). It is applied per **coarse** cell (subdivision 4, ~440 km;
`temperature.rs:673-678`). `presets.rs` turns the surface contrast of ~0.1 between vegetated
and bare land into **+0.05** of model albedo ("the top of the atmosphere sees about half";
Donohoe & Battisti 2011): Modern 0.32 → Abiotic 0.37. Plan §3.3 item 7 mixes "bare (0.37)", which
is a model value, with "forest 0.13–0.17, grass 0.2, desert shrub 0.3", which are surface values.
Forest would then sit 0.22 below bare land in the EBM, about 4× the contrast the presets use.
Densely forested coarse cells would warm by many kelvin, and the Modern-preset consistency check
of M1 could not pass: the land mean would fall far below 0.32.

*Change:* define the PFT albedos as planetary values with the presets' rule
`α_model = 0.37 − 0.5·(α_bare,surf − α_PFT,surf)`. That gives about 0.26–0.28 for forest,
~0.30 for grass and ~0.35 for desert shrub. Check that a modern-like cover gives a land mean of
≈ 0.32. Compute the coarse-cell albedo as the mean over fine land cells, like `land_ice`, and
keep the ice blend on top. Note that the colonisation front's climate effect is smoothed over
~440 km.

### 2. α = AET/PET is circular with the vegetation-dependent Budyko ω (major)

The plan limits cover by the moisture index α = AET/PET (§3.3 items 1–2), and makes ω depend on
cover (item 7). In this model, AET is computed from Fu's curve with ω
(`climate.rs:612-622`, `budyko_runoff`). Bare land (ω = 1.6) therefore has a low AET/PET because
it is bare. That restricts cover, which keeps ω low, and the reverse holds for vegetated land. This
loop is a per-cell algebraic feedback that involves no atmosphere at all. It would make bare
cells look "drier" than their climate is and could block colonisation of semi-arid land for
spurious reasons. (BIOME computes α from a bucket model whose soil water capacity does not depend
on the vegetation being predicted.)

*Change:* drive the PFT limits with a vegetation-independent index. Either use the aridity index
P/PET (UNEP; both fields exist per cell), or evaluate α with a fixed reference ω (e.g. the
Modern 2.6). Use the vegetation-dependent ω only for the water cycle.

### 3. The trigger: "planet age" is nearly inert here, and the time model is undefined (major)

- `Epoch::greenhouse_forcing` without `--co2` is defined to **compensate** the faint Sun
  (`presets.rs`, "climate-compensated forcing"). The planet age therefore changes almost nothing
  physically, only the latitudinal distribution, by a small amount. As a result:
  - option B's advantage ("ties to the faint Sun") largely disappears;
  - the claim in §2.2 that an epoch advancing in phase 2 "matters over a few hundred Myr" is wrong
    for the default configuration: the higher solar constant would be offset exactly by a smaller
    forcing. It matters only with `--co2` or with the carbon cycle (stage 3).
- The plan does not say what planet age the run **starts** at. Today `--epoch` is fixed for the
  whole run. If the default is "life at planet age 4.0 Gyr" and a run is started with
  `--epoch 0.55` (planet age 4.02 Gyr at t = 0), phase 2 fires in the first step. The planet would
  still carry the artificial initial tectonic state, and the Abiotic preset's documented cold
  transient of the first ~100 Myr (ice on tropical plateaus) would still be under way.
- The tectonic initial state does not represent a ~3.5–4 Gyr-old planet anyway, so "planet age"
  in phase 1 is a label.
- Under the compensated forcing, the C gate (no snowball) will essentially always pass at once,
  so it is decorative. That is fine, but say so. The "habitable shelf" condition does not apply to
  land plants, which descend from freshwater algae.

*Change:* state the time model explicitly:

- run start age A₀ (from `--epoch`), with displayed planet age A₀ + t;
- a life start given as **run time**, defaulting to a value past the spin-up transient (e.g.
  200–500 Myr; the 4-seed abiotic statistics used 200–1000 Myr);
- optionally a planet-age form that is converted to run time and refused if it lies before the
  run start or within the spin-up.

Recommend A + the no-snowball gate. Keep planet age as a label until `--co2` or the carbon cycle
makes it physical. Drop the "habitable shelf" gate for land colonisation, or move it to marine
life (M7). Make the time-dependent epoch part of M6, not M0.

### 4. Phase 1 must be Abiotic, but the default is Modern from t = 0: a missing decision (major)

Today a default run uses `Biosphere::Modern` parameters from the first step: vegetated albedo,
ω = 2.6, modern K. A consistent phase 1 is Abiotic. Per the preset's own 4-seed numbers that is
2.1 K colder, with 35% less land rain, ~80 m lower land and a much colder first ~100 Myr. Making
phase 2 part of default runs therefore changes **phase 1** of every default run, and with it the
basis of the calibrations and results of earlier issues. The plan implies this (§2.1, "Abiotic
land parameters") but does not raise it.

*Change:* add to §8:

> Is the two-phase run (Abiotic → life) the new default, or opt-in via `--life-start` while
> default runs stay Modern throughout?

Recommendation: opt-in at first, and reconsider the default once M1 is evaluated. Define how
`--biosphere` interacts with it: `--biosphere` sets phase 1; refuse `modern` together with
`--life-start`.

### 5. The spread rate in rings per Myr is a tuning knob in disguise (major)

Plant migration runs at ~0.1–1 km/yr (Holocene tree migration; Clark et al. 1998, "Reid's
paradox"), i.e. 100–1000 km per **kyr**. Within one 1-Myr step, any connected, climatically
suitable region is reachable many times over. A limit of "N = 3–10 rings per step" (330–1100 km
per Myr) makes the greening time of a continent (tens of Myr) a pure function of N. That is the
kind of pseudo-mechanism the plan itself rejects in option E. On Earth, the slow greening of the
interiors was paced by **evolution**: desiccation tolerance, vascular tissue, roots, seeds, soil
formation. Dispersal was not the limit.

*Change:*

- Within a step, let a PFT colonise every suitable cell connected to an occupied cell through
  suitable cells (one BFS/flood fill per PFT, O(cells)).
- Cross barriers (open sea, unsuitable bands wider than a few cells) with a small per-step
  probability.
- The visible front then comes from PFT availability (the timed schedule in M1, innovations in M5)
  and from barriers and climate. Keep a spread limit only as an optional parameter, documented as
  non-physical.

This also settles the "(review: check)" in §3.6. With within-step reach, plate speed no longer
constrains anything, and vegetation on cells while the crust moves is fine. For lineages, use two
distinct distances: the colonisation reach per step (large, through suitable cells) and the
gene-flow distance that defines clusters for speciation (small, 1–2 cells across unsuitable
cells; gen3sis uses the same split).

### 6. The climate response is a multi-step lag, not one step: revisit the stability argument (major)

The plan argues stability from "last step's climate for this step's vegetation" (§3.3 item 8,
§7). The code has more lag than that:

- the water cycle solves **4 months** (`months: 4`) and refreshes **1 per step**
  (`months_per_update: 1`, at most 6 Newton iterations). A given month's precipitation is
  therefore up to 4 steps old;
- the evapotranspiration uses the annual precipitation of the previous update
  (`set_evapotranspiration`);
- the EBM runs 2 years per step from the previous state;
- ice sheets use last step's precipitation.

The vegetation → ω → ET → rain loop therefore closes over ~1–4 steps. Delayed feedback with a
strong gain is exactly where a fixed-point iteration starts to oscillate. Separately, the
vegetation feedbacks (ET recycling, albedo) are mostly **positive**, so the more likely failure is
bistability and path dependence rather than flicker. Starting from bare land, marginal regions can
lock into the desert branch (cf. Brovkin et al. 1998; Claussen 1999; Hirota et al. 2011). That is
physically legitimate but must be recognised as such.

The "+177 ms extra climate pass" fallback is also ill-defined:

- 177 ms is the whole climate time, mostly the EBM plus one water-cycle month;
- a pass that again refreshes one month does not reach a fixed point;
- a full re-solve of 4 months × several Newton iterations costs more.

*Change:*

- Under-relax the cover, `c ← c + λ (c* − c)` with λ ≈ 0.3–0.5. This is a numerical damping, not a
  time-scale claim.
- Log a per-step flip counter (cells whose dominant PFT changes back and forth).
- Measure the EBM vs water-cycle split before quoting any extra-pass cost.
- In M1, test the transition response: how many steps until land rain settles after the switch.
- Mention hysteresis and bistability in §7 as an expected, explainable outcome.

### 7. Runtime estimates: level 2 is plausible, level 3 needs fixes (minor)

- **Level 2 at 5–20 ms is plausible.** 13k land cells × 8 PFTs × 12 monthly temperatures is about
  1 M cheap operations, flood fills are O(cells), and coarse albedo averaging is trivial. The
  hidden cost is that changing ω every step can raise the water-cycle iterations and the
  `clipped` fraction (capped at 6 Newton iterations per update, so the cost is bounded, but watch
  convergence).
- **Wrong baseline.** The 0.376 s baseline comes from steps 6–30. Phase 2 starts hundreds of Myr
  in, where steps take 0.36–0.65 s (#78). Quote 500 Myr of phase 2 as ~3–5.5 min for level 2,
  not 3.3 min.
- **Ambiguous level-3 operation count.** "10–20 ops × 15 ns per pair-neighbour" for 1.5 M pairs
  × 7 visits gives 1.6–3.2 s, not 150–400 ms. The quoted range implies ~15–40 ns per
  pair-neighbour visit in total. That is plausible, since the per-cell marker and climate arrays
  fit in L2/L3. State it that way, and add two costs:
  - rebuilding the CSR (cell → lineages) with a counting sort each step (~10–30 ms at 1.5 M
    pairs);
  - the per-cell competition pass.
- **Range sizes.** With within-step dispersal (finding 5), mean ranges will be larger than
  200–300 cells. The cost scales with Σ ranges, not with the number of lineages. Cap Σ occupied
  pairs (the real cost driver) and let the lineage count follow from it.
- **Lineage caps in §8.3 are premature.** Rather than asking Sebastian to approve caps (5 000 /
  2 000) before any measurement, schedule a small level-3 timing spike (synthetic ranges on the
  real mesh, ~0.5 day) right after M1, and set the caps from it.

### 8. Bit-identity and the state hash in M0 (minor)

- **Albedo averaging.** If per-fine-cell albedos are averaged onto coarse cells, a sum of N equal
  values divided by N is not always bit-equal to the value, so "bit-identical before the switch"
  would fail. Use `land_albedo + Σ cover·(α_PFT − land_albedo) / n` (exactly `land_albedo` when
  all covers are 0), or keep the scalar path while the biosphere is inactive.
- **Diffusion substeps.** `diffuse_sediment` derives its substep count from the global diffusivity
  (`erosion.rs:1619-1621`). With per-cell values, derive it from the maximum (identical when the
  field is uniform). The scheme stays conservative: transfers are donor-based.
- **State hash.** Add the new fields to `state_hash()` **only when phase 2 is enabled** (or once it
  has started). Otherwise every hash changes, and comparisons with logged hashes from #79/#87
  break. That conflicts with M0's "state_hash unchanged".

### 9. Code claims (minor; mostly confirmed)

Confirmed:

- `Planet::step` order: tectonics → temperature (with last step's precipitation) → water cycle →
  erosion.
- `state_hash` contents (FNV-1a over time, per-cell elevation, crust fields, precipitation,
  discharge, annual mean temperature, owner).
- `budyko_omega` is used in exactly two places (`climate.rs:456` runoff, `:621` ET), so per-cell
  ω is a local change.
- `land_albedo` per coarse cell is a local change, but it needs a new per-fine-cell input to
  `TemperatureModel::update`, since `params` are fixed inside the model.
- Per-cell K is trivial: `incise` takes a per-cell `(sediment, bedrock)` rate pair. Research §5's
  wording "already handles sediment vs bedrock erodibility per cell" is slightly off, since the
  factor is global, but the conclusion holds.
- `CellInfo` is 56 bytes (12 × f32 + 2 × u32, asserted in a test).

Not mentioned in the plan:

- `runoff_discharge` is **true** by default (`erosion.rs:177`). Rivers therefore carry the
  Budyko runoff, and per-cell ω also changes discharge and stream-power incision, partly offsetting
  the K change. (The comment in `presets.rs` that "the discharge follows the precipitation rather
  than the runoff (#7)" is stale. Fix it in passing.)
- Changing the solar constant and forcing during a run (§2.2) needs a setter on `TemperatureModel`,
  because its params are private and fixed.

*Change:* add these two facts to §3.3 and §6 (M0).

### 10. Visualisation: colours are built in the simulation thread, so the layer selector must recolour on the UI side (minor)

`surface_vertices` (colours included) runs in the simulation thread or the **Web Worker**
(`main.rs:303`, `web.rs:464`). The worker loop "needs no messages from the page" and blocks.
Choosing the layer in the vertex build would therefore show a new layer only with the next step,
and never once a run has finished.

*Change:*

- Recolour on the UI thread from per-cell values in `CellInfo` (41k cells, negligible), keeping
  positions and normals from the worker.
- `CellInfo` then has to carry the layer fields (e.g. cover by PFT as 8 × u8, NPP f32, biome and
  richness as u16s: 16 bytes, keeping the struct Pod and u32-aligned). The plan's "cover by PFT"
  plus NPP plus lineage fields do not fit "+16 bytes" otherwise.
- Today's relief palette already paints lowlands green (`ELEVATION_STOPS`, 0 m = green), so a
  vegetation tint in *Relief* mode makes the colour ambiguous. Either add a separate "Surface"
  (natural colour) layer or switch land in *Relief* to an elevation-only palette once vegetation
  exists. That is a small decision for Sebastian.

### 11. Speciation robustness on a jittery mesh (minor)

τ = 1–3 Myr is only 1–3 steps. Coastline and shelf cells that flip land/ocean between steps
(sea-level changes, sample jitter) can split and rejoin ranges and produce spurious speciation.

*Change:*

- Require a separation gap of at least the gene-flow distance in unsuitable cells for at least
  τ ≥ 3–5 steps.
- Use gen3sis' merge-on-reconnection (divergence decays while in contact).
- In M3, measure the per-step land/ocean flip rate of coastal cells.

As an option, make the divergence rate temperature-dependent ("evolutionary speed", metabolic
theory; Allen et al. 2002). Gen3sis' latitudinal-diversity case study and Saupe et al. (2019)
explore this. Without it, a tropics > poles gradient can emerge only from area, productivity and
stability.

### 12. Determinism details (minor)

- **Seed the biosphere RNG from a derived seed**, e.g. SplitMix64(seed ^ "BIOS") or similar,
  never the same `seed_from_u64(seed)` as tectonics; otherwise the streams are identical.
- Consider keyed draws (an RNG seeded from (seed, step, lineage id)), so that caps or debug
  features do not reshuffle every later draw.
- Assign daughter ids in a defined order (e.g. by the smallest cell index of each component).
- Sort floats with `total_cmp`.
- Sum NPP and statistics in cell or id order.
- Use no hash maps for occupancy. The plan's CSR arrays are right.
- Count PFT and lineage cells as integers wherever possible.

### 13. NPP units and targets (minor)

The Miami model gives **dry matter** (g/m²/yr). Multiply by ~0.45–0.5 for carbon. The planet's
land fraction differs from Earth's, so check NPP **per vegetated area** (mean ~300–700 g C/m²/yr;
tropical forest 800–1200, tundra and desert < 100) rather than the absolute 30–80 Pg C/yr in M1,
or scale that range by land area.

### 14. Missing literature (minor; from background knowledge, not checked online)

- Le Hir et al. (2011), "The climate change caused by the land plant invasion in the Devonian",
  *EPSL* 310, 203–212: plant albedo, hydrology and weathering effects in a GCM plus geochemistry.
  This is the closest analogue to the planned coupling.
- Brovkin et al. (1998, *JGR*) and Claussen (1999): vegetation–climate multiple equilibria.
- Hirota et al. (2011, *Science* 334, 232–235): forest/savanna bistability.
- Clark et al. (1998, *BioScience* 48, 13–24): plant migration rates (finding 5).
- Allen, Brown & Gillooly (2002, *Science* 297, 1545–1548) and Saupe et al. (2019, *Nature
  Ecology & Evolution*): temperature, climate change and diversity gradients.

### 15. Snow masking (nit)

The model has no seasonal snow, and trees do not grow on ice sheets. Drop "forests mask snow" from
the MVP instead of listing it with a caveat, and keep the gap in §7.

### 16. Fu's ω values (nit)

Make sure the forest ω of 2.6–3.5 is Fu's ω (as in Zhang et al. 2004), not the `w` of Zhang et al.
(2001), whose different formula gives forest 2.0 and grass 0.5. With per-cell ω, check that the
land mean under modern-like cover stays near the Modern 2.6 (M1 consistency check).

### 17. Modern-equivalence test (nit)

M1's "all-PFT equilibrium reproduces the Modern preset" needs a defined configuration: all PFTs
available from the start, ≥ 200 Myr. Also compare the land means of the per-cell albedo and ω with
0.32 and 2.6 directly. That is a cheap, sharper check than the climate statistics.

### 18. Decisions for Sebastian (minor)

- Add decision (4): default vs opt-in two-phase runs, and the phase-1 preset.
- Reframe decision 1 per finding 3: the life start in run time; planet age as a label until
  CO₂ is physical.
- Replace the lineage cap numbers in decision 3 with "caps set from a measurement after M1".
- Add the small relief-palette choice from finding 10.

The other decisions (freeze vs common step, the coupling set, stage 3 yes or no, land first and
marine later) are the right ones to ask.

### 19. Housekeeping (nit)

The plan's header already says that this review "is folded in below (marked '(review)')", but
only one "(review: check)" marker exists. Update the header and the markers when the findings are
folded in.

---

## Disposition (plan author, after the review)

All six major findings were folded into [plan.md](plan.md): planetary albedo with the half rule
(forest ~0.26–0.28, grass ~0.30, preset + delta form); moisture index P/PET instead of AET/PET;
the trigger is a run time (`--life-start`) with a snowball gate, the planet age only a label
until CO₂ is modelled; opt-in first, with abiotic phase-1 land, added as a decision; per-step
flood-fill spread instead of an N-ring speed; the multi-step climate lag, damping, flip log and
expected bistability. Minor points adopted: corrected per-pair level-3 cost and a cap on occupied
pairs set from an M4 prototype; later-run baseline (#78); bit-identity via preset + delta,
diffusion substeps from the maximum diffusivity, hash fields only with phase 2; stale
`runoff_discharge` remark; the `TemperatureModel` setter for an advancing epoch (stage 3);
UI-side recolouring from `CellInfo` and per-cell data budget; relief-palette decision; longer τ
with merge on reconnection; derived biosphere seed, `total_cmp`, ordered daughter ids; NPP in
carbon; the Modern-equivalence test defined; snow masking dropped from the MVP; the missing
literature added to research.md §7 (marked as not re-read).
