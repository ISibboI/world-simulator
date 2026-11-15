# Known physical-accuracy issues

From the independent physics review of commit fb5ffb6 (2026-11-15) and the shelf check. Earth
reference values in brackets. Numbered as in the review; **work started with issue 1** (user
decision, 2026-11-15). Details and evidence: `sessions/2026-11-15.md`. Feasibility research
with approaches and references per issue: `research/feasibility-2026-11-15.md`.

**Order of work** (user, 2026-11-15: calibrate issue 1 only as far as needed to confirm the
implementation, then prefer issues that also help issue 1, then work through the rest
autonomously, with an independent plan review before each issue): 1 (mechanisms) ✓ → 5 ✓
(partly) → issue 1 soundness fixes ✓ → 3 ✓ (ledger; artifacts remain) → 4 ✓ (peaks still too
high) → **runtime optimisation review (user, 2026-11-15: before issue 1 tuning; the user
decides which optimisations to do)** → issue 1 tuning (plan reviewed) → 2 → 7 + 6 → 9 → 8.

## Open

1. **Ocean floor too old, plates only die.** Mean ocean age 117–175 Myr [~64 Myr, max
   ~180–200]; 22% of the surface at the −6000…−6400 m asymptote [~1–2%]; plates 12 → 5–10.
   Cause: random kinematic plate motion, no slab pull, old ocean never starts subducting, plates
   never split or merge. Ideas: force/torque-based plate velocities (slab pull ∝ trench length ×
   slab age, ridge push, basal drag; possibly mantle flow), subduction initiation at old (>150–200
   Myr) ocean or passive margins, rifting along old sutures or of large/supercontinent plates,
   suturing (merging) after continental collision, GDH1 depth curve (asymptote 5651 m).
   **Mechanisms implemented** (plan `plans/plate-dynamics.md`): torque balance (slab pull,
   ridge push, basal drag, collision resistance), mantle flow (slab downwellings + plumes),
   rifting above plumes, subduction initiation at old passive margins and old ocean–ocean
   boundaries, suturing, GDH1, crust recycling by subduction erosion. Calibrated only as far as
   needed to show they work (seed 0, 500 Myr: mean ocean age 44 Myr, median 25, 6% > 150 Myr,
   22 plates with 81 births / 71 deaths, 7.3 cm/yr). **Open:** final calibration after issues
   5, 3, 4 (continental area still declines 30% → 20% and land rises to ~2 km; recycling vs arc
   growth is very sensitive); trench propagation and age-hazard initiation per the research;
   tiny plates reach 42 cm/yr [Earth max ~15–20], which forces many substeps (run 960 s).
   **Independent evaluation of c449e2f (2026-11-15): does not solve issue 1 yet — overshoots.**
   Plate count climbs to the hard cap of 30; after 200 Myr the ocean is too young (mean 33–37,
   median 18–20 Myr [~64 / ~50–55], 33–36% younger than 10 Myr [~11%], exponential instead of
   triangular area–age distribution); crust production 4× Earth kinematically, 6–7× actually;
   continental area still drifts (30% → 10–19% over 900 Myr with erosion). Ranked findings
   (outputs in `agent-scratch/issue1-review/`):
   1a. Over-production: many small plates each move as a block → half the boundary length opens.
       Calibrate on production/age, fewer births, merge tiny plates, transform friction.
   1b. Numerical over-production by boundary flicker (cells alternate gap ↔ overlap; worse with
       more substeps); new crust exceeds kinematic opening even with fixed motions.
   1c. **Bug:** polarity per plate pair → every boundary cell of a pair with a polarity is a
       trench (`dynamics.rs` classification, before the ridge check), even where diverging; 47–49%
       of boundary cells are "trench", ~half of them diverging; slab age at trenches 16–20 Myr;
       global slab pull ~2.2e21 N [0.5–1e21]; it also blocks ocean–ocean initiation. Fix:
       per-segment subduction state, require sustained convergence, slab pull ∝ slab length.
   1d. **Bug:** plume rifts through ≥ 160 Myr ocean become trenches in the same step
       (`plate_events` order) — most oceanic initiations are this artifact; rifts cut random great
       circles through whole plates.
   1e. **Bug:** split plates inherit `collision_time` → instant sutures of new plates.
   1f. Passive-margin initiation cuts a great circle across the whole plate; suturing counts
       continental contact regardless of motion → some splits re-sutured after 30 Myr.
   1g. **Bug:** thinned continental crust (below the buoyancy threshold) keeps its 1000 Myr age →
       counts as old ocean (max age 1100–1750), dominates initiation weights.
   1h. **Bug:** mantle kernel 1/(2π sin θ) is symmetric about 90° → every source has a phantom
       sink at its antipode; plume and slab fluxes unbalanced. Use Q·cot(θ/2)/(4πR) (source +
       uniform return flow).
   1i. Tiny plates (< 0.5%) reach 20–57 cm/yr (drives substeps): merge plates < ~0.3% into the
       neighbour with the longest shared boundary.
   1j. Continental area drift: collisional thickening without extensional collapse.
   Minor: active set stops after 4 iterations without convergence check; ridge push only at
   ridge cells; useless regularisation; `mod.rs` docs still mention random reorganisation;
   merges drop hidden samples. Determinism and index remapping verified OK.
   **Soundness fixes done (2026-11-15, plan + review `plans/issue-1-soundness.md`):** trenches
   only where converging or with an existing slab (hysteresis); slab pull ∝ slab length (grows
   with convergence to 600 km, decays τ = 10 Myr, reset on polarity flip, 150 km on initiation,
   600 km for initial zones); ocean–ocean initiation removed (converging pairs become trenches
   by polarity); plume rifting needs continental crust within 0.17 rad or a plate > 15%; split
   resets collision time; suturing counts non-diverging contact, timer decays; `is_oceanic`
   (< 12 km and > 2850 kg/m³) for ocean statistics and initiation; mantle kernel cot(θ/2)/(4π);
   tiny plates (< 0.3%) captured by the most similarly moving neighbour; active set ≤ 20
   iterations; regularisation scaled by drag; births/deaths by mechanism, trench/ridge length,
   slab pull and ridge push logged. 38 tests.
   Verification (500 Myr, seed 0 at s6, seeds 1–2 at s5): slab pull 0.55–1.05e21 N ✓ (was
   2.2e21); trench 150–190k km vs ridge 250–360k km (ratio plausible, absolute ~3× Earth:
   too many plates); plates 25/30/25 (seed 1 at the cap; births 52–76 vs deaths 34–63, mostly
   passive-margin initiations every ~10 Myr); max speed 28–54 cm/yr ✗; mean ocean age 31–37 Myr
   ✗ (too young); continental area 30 → 25–26%; merge loss up to 67,000 km·cells (seed 0).
   **Remaining = calibration, deferred to the issue 1 tuning step after issues 3 and 4:** birth
   rate (initiation interval), small-plate speeds, ocean production, trench propagation and
   age-hazard initiation per the research.
2. **Sea level fixed at 0 → ocean water volume not conserved** (1.75–2.35e9 km³ [1.335e9],
   drifting ~1 km sea-level equivalent). Solve sea level for constant water volume each step.
3. **Continental crust not conserved with erosion** (−36% seed 0, −60% seed 3 [~steady]):
   - eroded bedrock → sediment 1:1 by volume despite 2750 vs 2300 kg/m³ (16% mass loss);
   - sediment beyond the shelf is deleted (~7% of flux) → deep-sea fans instead;
   - sediment subduction 3.3 km³/yr [1–1.6] with no accretion → accrete/recycle 50–70%;
   - collision crust above the 75 km cap deleted in persistent collisions;
   - `apply_surface_changes` drops erosion changes of cells sharing a crust sample (~3000
     cells);
   - single-cell hole filling copies crust (creates volume, small).
   **Implemented (2026-11-15, plan + review `plans/issue-3-mass-conservation.md`):** crust mass
   ledger (`src/tectonics/ledger.rs`; every operation books its actual change; residual from the
   total of all samples incl. hidden ones; closes to round-off, test `crust_mass_ledger_closes`);
   sediment counted as rock mass (2050 kg/m³; bulk 2300 for isostasy); erosion converts bedrock to
   sediment at equal mass; deep-sea fans (up to 2048 cells) before deltas, the rest booked as
   lost; shared samples get the summed erosion change (excess sediment removal from bedrock by
   mass); 20% frontal accretion of subducting sediment as metasediment crust; collision stacking
   converts sediment at constant mass; per-snapshot budget in Gt/yr and masses by class.
   Seed 0, 500 Myr: continental crust mass −15% with erosion (was −36%), +40% without.
   Budget (Gt/yr; Earth): new sea floor 280–340 (~70, issue 1), arcs 20–25 (~7–8), subduction
   erosion 7–9 (~4), subducted sediment 10–16 (~3), **gap filling +27…+51 (artificial,
   resampling)**, **merges −2…−18 (hidden crust lost)**, sediment lost 0.4–4.3. Remaining leaks
   are resampling artifacts (gap copies, merges) and rate calibration (issue 1); hidden mass
   ~1.5–2.3e9 Gt (phantom crust on subducting plates).
4. **Thermal uplift on continental/arc crust with inherited young oceanic age** → thick young
   crust 1–1.5 km too high, highest cells 8.4–11.4 km [~5–6 km at 1° scale]. Track a thermal age
   that resets for continental crust; 70 km crust gives 6.3 km [Tibet ~5 km] → denser thick
   crust or a lithospheric-root term.
   **Implemented (2026-11-15, plan + review `plans/issue-4-mountain-heights.md`):** the GDH1
   thermal uplift fades out between the continental threshold (3000 m crustal buoyancy) and
   normal 35 km continental crust (arcs keep it); crust deeper than 50 km densifies to
   2950 kg/m³ at constant mass (70 km → ~4.85 km, 75 km cap → ~5.2 km); erosion gets a separate
   per-cell isostatic density for removal from the top. Tests: monotonic elevation at all ages,
   young arcs still override young ocean, no thermal uplift on continents, erosion response =
   formula. Seed 0, 500 Myr: max elevation per snapshot 4.0–9.7 km (before 6.8–9.6), mean land
   still rises to ~1.9 km. **Open:** peaks of 8–9.7 km cannot come from crust (cap → 5.2 km);
   diagnose (sediment in high lake basins? transitional crust on young lithosphere?).
5. **New oceanic crust created at convergent boundaries** (28–30% of crust < 3 Myr): gap filling
   ignores relative plate motion. Create ridge crust only where plates diverge.
   **Partly fixed (2026-11-15, plan `plans/issue-5-gap-filling.md`):** overridden samples are
   consumed only once invisible (no freed cells; arcs/erosion/collision counted once per
   sample); new sea floor only where the receiving plate opens away from a nearby plate (smoothed
   normals, ≥ 0.5 cm/yr, obliquity limit); other gaps healed with copies unless the slot's crust
   was consumed (then boundary slivers share the adjacent sample, interior areas become sea
   floor). Seed 0, 200 Myr: young crust at convergent boundaries 21–26% → 8–10% (target < 5%;
   rest likely issue 1c), continental area 24.6% → 27.7%. Copies still create ~330,000 km·cells
   per 200 Myr (→ issue 3 ledger). Variants tried: plain copies inflated continents (30 → 35%,
   copies of consumed collision crust were stacked again — a mass pump); pure sharing left 12% of
   the surface as persistent shared cells.
6. **Erosion inconsistencies:**
   - implicit stream-power solve ignores isostatic response → big rivers erode up to ~6× too
     little; use `e = F(z − z_r) / (1 + F·f)`;
   - runoff = precipitation although 50% of land rain is recycled [runoff ~35–40%];
   - no endorheic basins [~20% of land drains internally].
7. **Shelves are sediment piles on oceanic crust.** 94% of cells between −200 and 0 m carry
   > 500 m of sediment (mean 11 km, 2761 of 3111 cells > 6 km), only 364 are continental crust;
   bare they would lie at ~−5900 m [shelves: thinned continental crust + 1–5 km sediment]. Cause:
   each river mouth fills up to 256 cells to −120 m at once; no rifted (stretched) continental
   margins. Fix: stretch continental crust where plates diverge (β ≈ 2–4), limit progradation.
8. **Precipitation too low at mid-latitudes** (land 20–60°: 0.30–0.35 m/yr [~0.6]; half of land
   < 0.25 m/yr [~20% arid]); ocean moisture reset to local evaporation (no fetch, ΣP ≠ ΣE); polar
   front at 55° [storm tracks 40–50°]; no seasonal ITCZ migration.
9. **Temperature details:** poles ~10 K too warm [−25 °C poleward of 70°]; land heat capacity
   8e6 J/m²/K below the atmospheric column alone [use 2–3e7]; subtropical land seasonality too
   large.

10. **Plates break apart along straight lines** (user, 2026-11-15): rifting above plumes and
    subduction initiation at passive margins split plates along a full great circle, which looks
    odd. Look for a physics-based way to choose the break (e.g. along zones of weakness such as
    old sutures or thinned/young lithosphere, following the stress field, or growing a rift from
    the plume towards existing boundaries). Related: issue 1 findings 1d/1f and the deferred
    recommendation S1 in `plans/issue-1-soundness.md`.
11. **Ocean wave (coastal) erosion** (user question, 2026-11-15): check whether wave erosion of
    coasts is simulated; currently only fluvial erosion and sediment deposition are modelled
    (`src/erosion.rs`). Not answered yet.

12. **Asteroid impacts** (user, 2026-11-15): include impacts in the model at a later point
    (craters, ejecta, and their effects on climate and life). Not a defect of the current
    model; a future feature.

Minor: statistics count land as `> 0` in one place and `>= 0` elsewhere.

## Acceptable simplifications (at ~110 km cells, 1 Myr steps)

Local Airy isostasy (no flexure, trenches, forebulges), constant densities, no compaction,
age-only ocean depth (no dynamic topography), nearest-neighbour resampling of rigid plates,
diffusive crustal flow, n = 1 stream power with calibrated K, no hillslope/glacial erosion, no
lithology, fixed three-cell winds without monsoons, single upwind neighbour, zonal energy
balance without ocean currents, Budyko annual-mean ice criterion, 50 m mixed layer.
