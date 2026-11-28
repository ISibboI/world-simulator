# Volcanism, first PR: phases 0–2 (issue #106)

Plan for the first implementation step of #106, after the design answer in
[design-2026-11-26.md](design-2026-11-26.md) and after #196 (the carbon cycle, #154) reached
`main`. Earth-like planet, not Earth: the numbers below are plausibility ranges, not targets.
Errors of the ~110 km grid are accepted (#64).

## Scope of this PR

| Phase | In this PR | What |
| --- | --- | --- |
| 0. Magmatic flux diagnostics | yes | Per-snapshot magmatic volume rates by source (ridges, arcs, LIPs, plume tails), the LIP count and a log line per LIP. Two new ledger terms. |
| 1. Plume-head LIPs | yes | One LIP per plume when its head arrives. Log-normal volume, scaled by a lid factor (melt column under the lithosphere). Oceanic plateaus and continental flood basalts as added mafic crust. |
| 2. Hotspot tracks | yes | Continuous tail flux at the cell above each plume after its head, scaled by `Plume::flux` and the same lid factor. Plume-fed ridges get thicker crust automatically (the tail lands on whatever crust is above the plume). |
| 3. Climate ties beyond CO₂ | no, follow-up | Surface lithology (a fresh-basalt surface layer) for weathering; thermogenic CO₂ of LIPs into sediment; optional LIP aerosol forcing (#204). |
| 4. Activity view | no, #110 | The LIP record and the per-source rates this PR adds are its inputs. |

Phases 1 and 2 share one emplacement routine (volume over a Gaussian footprint), so delivering
both costs little more than phase 1 and gives the two relief features the design found
resolvable: oceanic plateaus and hotspot chains.

## Model

### Trigger (phase 1)

A plume emplaces its LIP in the step in which its age crosses `lip_age` = 10 Myr, when
`Plume::head_growth` reaches 1 (the head has arrived and spread). With the default
`plume_interval` of 15 Myr this is about one LIP per 15 Myr, the rate the design argued is
Earth-like (Earth: ~1 per 20–30 Myr for ≥ 10⁶ km³, ~1 per 10 Myr including ~10⁵ km³ events).
Plumes that exist at the start older than `lip_age` (#217 primes the mantle with plumes of
random ages) count as old: they get no LIP, only their tail. No separate random clock.

### Volume (phase 1)

`V = V_ref · (peak_flux / mean_peak_flux) · L · exp(σ ε)`, ε ~ N(0, 1) (Box–Muller from the
simulation's RNG, through `math`), clamped to 10⁵–10⁸ km³.
- `V_ref` = 2 × 10⁷ km³: the head's melt under no lid (a ridge).
- `σ` = ln(10)/2 ≈ 1.15: a decade spans ±2σ.
- **Lid factor** `L = ((Z − z_L) / Z)²`, clamped to ≥ 0: decompression melting of the hot head
  starts at depth `Z` ≈ 180 km (a plume ~200–300 K hotter than ambient mantle crosses the dry
  solidus there) and stops at the base of the lithosphere `z_L`. Both the column height and the
  mean melt fraction grow with `Z − z_L`, so the melt volume goes about with its square (White
  & McKenzie 1989; Ellam 1992).
  - Oceanic and transitional lithosphere: `z_L = min(2.32 √(κ t), 95 km)` with κ = 10⁻⁶ m²/s and
    `t` the lithosphere's age (`Crust::age`, not the plume-reset strength age): 30 km at 2 Myr,
    65 km at 25 Myr, 95 km beyond ~55 Myr. `L` = 0.69 / 0.40 / 0.22.
  - Lithosphere of continental origin (`Crust::is_continental_origin`): `z_L` = 130 km (a
    non-cratonic mean; the model has no cratons), `L` = 0.077.
  - Judged at the cell above the plume.
- Resulting medians (mean peak flux): ~1.5 × 10⁶ km³ under continents, ~4–8 × 10⁶ km³ under
  typical ocean, ~1.4 × 10⁷ near ridges; the log-normal spreads these over 10⁵–10⁸. Earth:
  median ~10⁶ km³, oceanic plateaus 10⁷–10⁸ (Ontong Java 4–6 × 10⁷).

### Footprint and crust (phases 1 and 2)

The volume is spread as a radial Gaussian around the plume over the **visible crust samples**
(each counted once, at its nearest cell, as the arc additions do since #57), normalised
discretely so that the emplaced volume is exactly `V` (before the thickness cap):
- LIP: σ = 400 km, cut at 1000 km (`PLUME_HEAD_RADIUS`). A 10⁷ km³ plateau gets ~10 km at the
  centre (σ² 2π ≈ 10⁶ km²); 5 × 10⁷ km³ gets ~50 km → clamped by the max crust thickness only in
  extreme cases. On continents a 1.5 × 10⁶ km³ province adds ~1.5 km at the centre.
- Tail: σ = 150 km, cut at 400 km (1–3 cells at 6 subdivisions, ~1 cell at 5).
- The crust is added with `Crust::add_crust` as mafic crust: density 2950 kg/m³ (plateau crust
  is 2950–3050 with its cumulates; normal oceanic crust in the model is 2900), `felsic` 0,
  `metasediment` 0, capped at `max_crust_thickness` like other additions. On land the column
  does not separate lava from underplate (both are mafic crust of the column); the sediment
  stays on top.
- Isostasy then gives: oceanic plateau of +10–25 km → 1.4–3.5 km shallower sea floor; flood
  basalt of +1.5 km on continental crust → ~+0.2 km. Thick plateaus (≳ 19 km at 2950 kg/m³) reach
  `SUBDUCTION_RESISTANCE_BUOYANCY` and accrete at trenches through the existing logic.
- Ledger: two new terms, `lips` and `plume tails` (actual mass change, so the ledger closes
  also where the cap binds).

### Tail (phase 2)

Each step, every plume older than `lip_age` adds `Q = q · flux() · L · dt` at its current
position, with `q` = 10⁸ km³/Myr per m²/s at `L` = 1. A typical plume (flux 4 × 10⁻³ m²/s) on
old ocean (`L` ≈ 0.22) gives ~9 × 10⁴ km³/Myr (0.09 km³/yr, magma including intrusions and
underplate). Earth: Hawaii 0.01–0.04 km³/yr for most of its history, 0.2 recently (edifices
only); Iceland ~0.1–0.2. With ~9 plumes the model's tail total is ~0.5–1 km³/yr, the
intraplate share of Earth's magmatism (Crisp 1984; White et al. 2006: ~1–2.5 km³/yr
intraplate of ~25–30 total).
- Plates move over the fixed plume: a column passes the 300 km footprint in ~6 Myr at 5 cm/yr
  and gains ~1–3 km, a +0.2–0.5 km sea-floor track; slow plates pile more (Hawaii-type
  +1–2 km). Under a ridge the new crust is thickened (Iceland), without special code.
- Under continents (`L` = 0.077) the tail is weak (~3 × 10⁴ km³/Myr), as at Yellowstone and
  Hoggar.

### Carbon (no double counting with #196)

#196's degassing sources are ridges (per kg of new sea floor), arcs, plumes (`plume_carbon ×
Σ flux`) and metamorphism.
- **Ridges**: unchanged. Tail crust on a ridge is not new sea floor (it is added to existing
  samples), so it is not counted there.
- **Plume tails**: their CO₂ is already #196's plume term (the same `flux()` drives both). No new
  carbon. The tail crust makes visible what that term books.
- **LIPs (new)**: the head's melt is genuinely new magmatism, not in #196. A new driver
  `TectonicDrivers::lip_crust` (cumulative kg, from the ledger term) and a parameter
  `carbon.lip_carbon` = 0.07 mol/kg (0.3 wt% CO₂ of the emplaced magma, erupted and intruded;
  basaltic magmas carry 0.1–0.5 wt%, e.g. Self et al. 2006 used ~0.5 wt% for the Deccan). Booked
  into the `plumes` source (`Sources` keeps its four fields, so the log and statistics stay as
  they are), unsmoothed: the pulse lands in the step of the LIP. A 10⁶ km³ continental LIP gives
  ~0.2 Tmol/yr in its step (~3% of background), a 10⁷ km³ plateau ~2 Tmol/yr.
- The head's ramp of `flux()` over the first 10 Myr (the plume term) is the plume's tail
  starting, not the head; there is no overlap with the LIP term.
- Thermogenic CO₂ (sills in sediment) and lithology-dependent weathering of fresh basalt are
  phase 3. #196 already weathers mafic crust 5× faster by `felsic`, but a 1 km flood basalt on
  35 km of crust lowers the column's felsic fraction by only ~3%: the fresh-surface effect needs
  a surface-lithology tracer (follow-up).

### Climate ties not built here

- SO₂ aerosols: average out over a 1 Myr step (design §4). #204 (volcanic sulfate as cloud
  nuclei) can read the per-step LIP and tail volumes this PR exposes.
- #202 (abiotic CH₄/H₂ from outgassing) can scale with the same magmatic rates.
- #110 (activity view): `Simulation::volcanism()` gives the LIP record (time, position, volume,
  lid factor, setting) and the totals.

## Code

- `src/tectonics/volcanism.rs` (new): `VolcanismParams` (in `TectonicParams`, settable as
  `volcanism.*`, `volcanism.enabled=0` for ablations), `Volcanism` (record and totals), the
  lid factor, the emplacement routine and `Simulation::volcanism_step(dt)`.
- `tectonics/mod.rs`: call it in `end_step` after `mantle.step` (plumes aged, born) and before
  the sea level is solved; `carbon_drivers` gets `lip_crust`.
- `tectonics/ledger.rs`: terms `Lips`, `PlumeTails`.
- `carbon.rs`: `lip_crust` driver, `lip_carbon` parameter, booked into `plumes`.
- `statistics.rs`, `main.rs`: totals in the statistics; a snapshot log line "magmatism
  (km³/yr): ridges, arcs, LIPs (n), plume tails".
- No change to the frame codec, the viewer or the web UI.

## Tests

1. Ledger closes with volcanism (residual ~0) and the new terms are positive.
2. Emplacement conserves volume: Σ Δthickness × cell area = V (no cap), each sample once.
3. Lid factor: decreasing with age, smallest under continents, in [0, 1].
4. A large oceanic LIP makes plateau crust that resists subduction; a small continental one
   only thickens mildly.
5. A plume crossing `lip_age` gives exactly one LIP; an old plume gives none and only a tail.
6. Carbon: the LIP driver adds `lip_carbon × rate` to the plume source; zero LIP crust leaves
   the sources unchanged.
7. Determinism (existing test) with volcanism.

## Evaluation

Default barren preset, 8 seeds × 1000 Myr, this branch against `main` (same arguments), via
`runs`. Report: crust volume added by source and its distribution (ocean/continent, plateau
thickness), LIP count and interval vs ~1 per 15 Myr (model) and ~1 per 20–30 Myr (Earth,
large ones), relief (hypsometry, plateau and track elevations), CO₂ and temperature against
main, runtime. Independent evaluation by a second agent.

## Risks

- Plateaus ≳ 28 km at 2950 kg/m³ reach `CONTINENTAL_BUOYANCY` and count as continental in the
  statistics (and lose part of the oceanic thermal uplift); they stay non-felsic, so not of
  continental origin. Watch the continental fraction.
- Extra crust displaces ocean water: the sea level rises a little (plateaus ~10⁷ km³ each).
- The extra RNG draws change every run against main (seed-for-seed comparison is statistical).

## Response to the plan review ([plan-review.md](plan-review.md))

- **Thickness cap (must-fix):** done. Plume crust is added only up to the cap
  (`min(h, max − thickness)`), so the cap never thins existing felsic crust; a test checks the
  felsic mass.
- **RNG (must-fix):** volcanism has its own ChaCha8 stream derived from the seed, so it changes
  no other random draw; `volcanism.enabled=0` is checked to give main's state hash. Box–Muller
  uses `math::ln`/`math::cos`, with `u ∈ (0, 1]`.
- **Plateaus > 12 km are "transitional" (must-fix, decided):** accepted as physical. Plateaus
  are not normal oceanic crust: Earth's plateaus are not sites of subduction initiation, they
  are shallow (sediment zones of a shelf/slope are plausible on their tops), and they add no
  ridge push away from ridges. Watched in the evaluation (oceanic area, continental fraction).
- **Pile-up under slow plates (should):** not limited in code; the evaluation reports the
  thickest plume-fed columns. Crustal flow above 40 km spreads thick crust as for orogens.
- **CO₂ per kg differs between head and tail (should):** stated in the results: #196's plume
  term is per plume flux, independent of the lid; it implies more CO₂ per kg of tail crust than
  the LIP's 0.07 mol/kg. Not double counting; a follow-up could tie the plume term to the tail
  volume.
- **Calibration bias (should):** accepted (~3% for a rare huge plateau in the window).
- **Plateau shape (should):** done: σ = max(400 km, √(V / (2π · 25 km))); chord distances.
- **Continental budget (should):** the evaluation reports felsic/continental changes against
  main and the accretion of plateaus.
- Nice-to-haves not done: a separate sea-level budget term (volcanism is in "tectonics"),
  rift-thinned continental lid.

## Changes after the evaluation runs (see [results.md](results.md))

- Tail flux 10⁸ → 3 × 10⁷ km³/Myr per m²/s (all tails ~0.35 km³/yr, Earth's oceanic intraplate
  rate; 10⁸ gave ~1.2 km³/yr and piled up mafic crust).
- LIP volume spread ln(10)/2 → ln(10)/3 within a setting (the setting's lid factor gives the
  rest of Earth's three decades; the wider spread made continental LIPs of up to 6 × 10⁷ km³).
- New limit: plume crust only up to a 40 km mafic column (eclogite founders), after the
  independent evaluation found 47–75 km basalt piles.
