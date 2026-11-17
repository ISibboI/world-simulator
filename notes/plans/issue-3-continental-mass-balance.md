# Plan: continental mass balance (issue 3, after mass-conserving gap healing)

_2026-11-17. User: fix the gaps, then the continental mass balance; collapse/stretching only if
still needed._

## Problem

Since gap healing no longer creates crust (b497f53), continental crust mass falls ~48% in
500 Myr (1.80 → 0.93–1.01e10 Gt, seeds 0–3), continental area 40% → 22–24% of the cells. Earth's
continental crust is roughly in steady state over the last ~2 Gyr (arc production ≈ losses by
sediment subduction, subduction erosion and delamination; Scholl & von Huene 2007/2009,
Clift et al. 2009).

## Budget (scratch instrumentation `agent-scratch/dbg6`, seeds 0/1, Gt/yr over 500 Myr)

Felsic crust = crystalline crust with density < 2850 kg/m³ (continental, arc, metasediment),
hidden samples included; sediment counted separately (rock mass).

| Process | Felsic crust | Sediment | Earth |
|---|---|---|---|
| Erosion of bedrock | −12.5 / −12.5 | +10.3 / +10.4 | ~20 Gt/yr to the oceans |
| Sediment deleted ("lost" beyond fans) | | ≈ −2 | 0 |
| Plate events (merges drop hidden samples) | −5.3 / −5.1 | −0.5 | 0 (artefact) |
| Arcs onto felsic / onto other crust | +3.3 / +2.6, +4.4 / +4.7 | | ~8 total |
| Frontal accretion (metasediment) | +1.3 / +1.4 | | |
| Subducted felsic crust (mostly 10–20 km thick) | −2.9 / −3.9 | | small (buoyant arcs accrete) |
| Subduction erosion of felsic crust | −1.8 / −1.4 | | ~4 |
| Subducted sediment | | −6.9 / −7.4 | ~3–4 |
| Net | ≈ −14 | ≈ +2 (stock grows) | ≈ 0 |

## Changes

1. **Merges keep hidden crust** (artefact, −5 Gt/yr): `merge_plates` maps the absorbed plate's
   *samples* (also hidden ones) into the frame of the receiving plate instead of only the
   visible cells; where the receiving plate already has a sample at the target vertex, stack
   continental crust (as now) or keep the denser/older one hidden-merged by mass (to define).
2. **Buoyant crust accretes instead of subducting** (Cloos 1993: crust thicker than ~15–20 km of
   arc/plateau/continental composition is too buoyant to subduct): at an overlap, a lower-plate
   sample that is felsic (density < 2850) and thicker than `accretion_thickness` (≈ 15 km) is
   stacked onto the overriding plate like continental crust (terrane accretion), not
   subducted.
3. **No sediment deletion:** sediment beyond the fans' reach is spread over the deep ocean
   floor (pelagic/turbidite apron, e.g. evenly over the ocean cells of the basin or within a
   larger radius) instead of being deleted; it then displaces water (sea level) and is
   subducted or accreted with its crust.
4. **Sediment fate at trenches:** frontal accretion 20% → calibrate against Earth's sediment
   subduction (~1.6 km³/yr ≈ 3–4 Gt/yr; Clift & Vannucchi 2004, Scholl & von Huene 2007) only
   after 1–3, since 3 adds sediment to the ocean; consider restricting shelf building
   (−120 m fill) to continental/transitional crust so that shelves stay on margins (the known
   "shelves are sediment piles on oceanic crust" issue), with the rest to fans.

Order: 1 (pure artefact) → 2 → 3 → re-measure → 4 if still needed. Each step measured with the
budget (permanent diagnostic: felsic and sediment budget per process in the log).

## Verification

- Tests: a merge conserves total mass including hidden samples; a thick felsic sample arriving
  at a trench is stacked onto the overriding plate (mass conserved, booked as accretion);
  sediment no longer disappears (ledger term "sediment lost" zero, mass conserved by surface
  processes); ledger closes.
- Runs (seeds 0–3, 500 Myr): felsic and continental mass trend (target: within ±10% per
  500 Myr), continental area (cells and samples), the budget table above vs Earth, sea level,
  mean land, production, plates; runtime.

## Plan review (independent agent, 2026-11-17): sound with changes — adopted

1. Budget: erosion is a transfer; conserve **felsic crust + sediment**. Net ≈ −12 Gt/yr; the
   largest real sink is net sediment subduction (5.6 average, 9.4 at the end; Earth 2.6–4.5),
   then merges (~5), felsic subduction (~3), subduction erosion (~1.8), deleted sediment (2–4,
   ledger "surface processes" −4.1 at the end). The density cutoff (< 2850) is biased (arc crust
   on oceanic samples invisible until the whole sample crosses it): use a linear unmixing,
   felsic volume fraction ≈ clamp((2900 − ρ)/100, 0, 1). Report interval rates too.
2. Merges lose visible crust as well (two absorbed samples mapping to one target vertex
   overwrite or stack: 0.5–3% of visible mass per merge, +28–36 samples > 60 km per merge —
   part of issue 1r), the stacking path turns sediment into crust (+37% mass) and overwrites
   the receiving plate's hidden samples. Rule: map every absorbed sample (visible or hidden)
   by world position; nearest free vertex within 1–2 rings; else spread its mass over the
   target and its neighbours; never stack same-plate samples; clear `consumed`. Test after
   motion: mass conserved incl. hidden, max thickness not increased, visible cells unchanged.
3. Accretion by one buoyancy predicate `resists_subduction()` (dry crustal buoyancy ≥ X <
   `CONTINENTAL_BUOYANCY`; Cloos 1993) applied at the stacking test (also under an oceanic
   winner), in the polarity arrival/flip logic and in the `Collision` boundary kind (else slab
   pull drives arcs into the trench). Main case: arcs on the old upper plate when a continent
   arrives. Consider partial accretion (upper ~60%). First measure where thin felsic crust
   comes from (arcs on ocean, thinned margins, subduction erosion floor at 7 km).
4. Sediment: losses are probably in small isolated basins (every cell below sea level is
   "ocean"); diagnose first; simplest fix: deposit the rest above `delta_top` (becomes land,
   next step's flood moves it). No budget gain expected (deep-sea sediment subducts).
5. Sediment at trenches is the main lever, not optional: accrete trench fill above a
   subduction-channel thickness (~0.5–1 km; Clift & Vannucchi 2004, von Huene & Scholl 1991)
   instead of a flat 20%. Do not restrict shelves to continental crust.
6. Earth values in km³/yr: arcs 2.5–3 (7–8 Gt), subduction erosion 1.3–1.4 (3.5–4 Gt),
   sediment subduction 1.0–1.65 (2.6–4.5 Gt), felsic crust lost in collisions ~0.4 (~1 Gt),
   erosion to the oceans ~14 Gt/yr pre-human (Syvitski et al. 2005).
7. Success metric: felsic + sediment within ±10% per 500 Myr; per-flux targets above; tests:
   merge after motion, arc–continent collision accretes the arc, buoyant sample under an
   oceanic winner is stacked, sediment into an isolated basin is not lost.
Order: diagnostic → 1 (merges) → measure → 2 (accretion predicate) → 3 (sediment, after
diagnosis) → 4 (trench accretion).
