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
