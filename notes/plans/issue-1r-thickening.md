# Plan: continental thickening (issue 1r)

_2026-11-16. User: thickening next (relative-speed tuning of 1q later)._

## Problem

Visible continental crust thickens from 35–37 km to 42–47 km mean in 500 Myr; 10–35% of it is
thicker than 50 km [Earth: a few %]; it never thins (10th percentile stays at the initial
margin thickness). With the sea level (issue 2) the freeboard (mean elevation of continental
crust) rises 0.4 → 1.3–1.6 km and flooded continental crust falls from ~27% to 2–24%
[Earth ~25–30%]. Isostasy is fine (~150 m per km of crust). Causes (evaluation of 3af40dc):
collisions keep converging and stack crust (20–40 Gt/yr); orogenic collapse is far too slow
(`crustal_flow`: thickness diffusion ≈ 600 km²/Myr, only within a plate, only above 40 km);
nothing stretches continental crust (rifts and divergent gaps get new oceanic crust right at
the continental edge).

## 1. Orogenic collapse (gravitational spreading by lower-crustal channel flow)

Thick crust is hot at its base; its weak lower crust flows down the gradient of surface
elevation (Clark & Royden 2000; Bird 1991): flux per unit width
`q = −(ρ_c g h_ch³ / (3 η)) ∇z`, i.e. a diffusion of elevation with
`κ = ρ_c g h_ch³ / (3 η) · (1 − ρ_c/ρ_m)⁻¹`-scaled for crust thickness; for a channel of
10–15 km and η = 10¹⁹–10²⁰ Pa·s this is ~10³–10⁴ km²/Myr.

- Replace the pairwise thickness diffusion by a flux driven by the **dry-elevation difference**
  between neighbouring cells, converted to crust thickness with the source's isostatic
  response (`dry_response(ρ)`), with mobility `κ(h)` = `collapse_diffusivity` × ramp of the
  thicker crust's thickness from `crustal_flow_threshold` (40 km) over `crustal_flow_range`
  (20 km). Only downhill, only from crust above the threshold (normal crust and oceanic crust
  do not flow); the receiving cell may be any crust.
- **Across plate boundaries** too: orogens spread onto their foreland and over adjacent plates
  (crust moves from one plate's sample to the other's; booked in the ledger as crustal flow,
  mass conserved).
- Discrete Laplacian on the (near-)hexagonal mesh: per neighbour pair
  `Δ = κ dt · 2/(3 d²) · (h-equivalent difference)`, explicitly sub-cycled so that
  `κ dt_sub / d² ≤ 0.2` (stability), d = mean edge length. Resolution independent by
  construction (κ in km²/Myr).
- Calibrate `collapse_diffusivity` within 10³–3·10⁴ km²/Myr on: fraction of continental crust
  > 50 km (few %), mean continental thickness (~35–40 km), freeboard and flooded fraction.

## 2. Rift stretching (thinned passive margins)

Where plates diverge at continental crust, the lithosphere stretches before it breaks up;
passive margins are 100–300 km of crust thinned by β = 2–4 before the continent–ocean
boundary (McKenzie 1978).

- In the gap filling (`resolve_ownership`, `boundaries.rs`), when a spreading gap cell is filled
  by a plate whose adjacent sample (the cell the fill comes from) is continental-density crust
  thicker than `rift_breakup_thickness` (20 km), the gap gets **half of that sample's crust**
  (thickness, sediment; same density, birth) and the sample keeps the other half: mass
  conserving stretching by β = 2 per opened cell. Thinner (stretched) crust is no longer split:
  the next gap gets new oceanic crust (breakup). A 35 km margin thus becomes ~2 cells of
  ~17 km transitional crust, then ocean.
- Ledger: no new mass (booked as zero change); `new_crust_cells` (production) not counted for
  stretched cells; a new counter `stretched_cells`.
- Thermal state: stretched crust keeps the continental birth (no oceanic thermal uplift beyond
  what `thermal_weight` already gives thin crust) — check elevations of stretched margins.

## Verification

- Unit tests: collapse conserves mass, reduces the thickness of an isolated thick block and
  spreads it over neighbours (also across a plate boundary), leaves crust below the threshold
  and flat thick plateaus untouched, is stable for large κ dt (sub-cycling) and resolution
  independent (s4 vs s5 spreading width); rift stretching conserves mass, halves the source,
  stops below the breakup thickness, and creates oceanic crust beyond it.
- Runs (seeds 0–3, 500 Myr): mean continental thickness and its distribution (> 50 km
  fraction, 10th percentile), freeboard, flooded continental fraction, land fraction and mean
  land elevation, continental area and mass, sea level, max elevation; runtime.
- Diagnostics: continental thickness percentiles (10/50/90%) and fraction > 50 km; stretched
  cells; collapse flux (Gt/yr).

## Not in scope

Collision resistance / convergence rates (1q tuning), delamination beyond the 75 km cap,
erosion changes, plate splitting geometry (issue 10).

---

# Revised plan v2 (2026-11-17)

_User: issue 1r before the sediment routing (margins need rift thinning and thermal subsidence
for room; see `sediment-routing.md` and its review). Incorporates the v1 review above, the gap
and mass-balance work since (gap copies removed, issue 3), and the felsic tracer
(`Crust::felsic`)._

## Current state (seeds 0–3, 500 Myr, `agent-scratch/t1/log-trc-*`)

Mean land 1.24–1.36 km [0.84]; continental crust mean ~41 km at 500 Myr with ~20% > 50 km and
~11% > 60 km [a few %]; flooded continental crust falls from ~27% to 7–19% [25–30%]; collision
stacking 46–63 Gt/yr (doubled by the subduction-resistance predicate); passive margins are
short-lived and have no room for sediment (no thinning, no thermal subsidence); continental
crust never thins (10th percentile ~31 km).

## A. Rift stretching with thermal subsidence (passive margins)

- **Where:** a gap cell that opens (`opens_away`) next to (the fill parent) crust of continental
  origin (`felsic > 0.5`) thicker than `breakup_thickness` (15 km): no new sea floor; instead
  the plate's stretching zone fills it. Slow or oblique continental divergence (below
  `min_spreading_rate`, currently shared gaps) is stretched the same way as long as the plates
  open (not transforms), so rifts stretch before they break up.
- **How (resolution independent, tapered):** donors = the plate's samples of continental origin
  within `rift_width` W (200 km) of the gap cell in the plate frame (N samples). Uniform
  stretching by β = (N+1)/N per opened cell: each donor gives 1/(N+1) of its crystalline crust
  and sediment, the new sample receives the sum (mass, density and felsic tracer mixed by
  mass). Cells nearest the rift take part in every opening, so the margin tapers over W.
  When the parent is thinner than `breakup_thickness`, the gap gets new oceanic crust
  (continental breakup).
- **Thermal subsidence (McKenzie 1978):** stretching heats the lithosphere: with U the current
  fraction of ridge uplift of a sample (from its age; 0 for old continents), after stretching by
  β_step its fraction is U' = 1 − (1 − U)/β_step; its formation time is set to the age whose
  GDH1 uplift is U' of the ridge uplift (inverse by bisection). It then subsides with the GDH1
  curve as it ages: syn-rift uplift and post-rift thermal subsidence create the margin's room
  for sediment. The new sample takes the mass-weighted U' of its donors. `thermal_weight`
  already gives thin crust the full oceanic thermal behaviour.
- **Classes:** stretched crust (15–20 km) is transitional by buoyancy but remains of
  continental origin and resists subduction (≥ 2000 m). Passive-margin initiation looks for
  oceanic crust next to crust of continental origin (not only `is_continental`); statistics
  report continental area both by class and by origin.

## B. Orogenic collapse (lower-crustal channel flow)

- Flux driven by the **dry-elevation** difference between neighbours, converted to crystalline
  thickness with the source's isostatic response; thickness diffusivity `collapse_diffusivity`
  κ (km²/Myr; channel flow K·a with a ≈ 0.15: ~3.7·10³ at η = 10¹⁹ Pa·s, ~3.7·10⁴ at 10¹⁸ for a
  15 km channel; Clark & Royden 2000; Bird 1991) with a **cubic** mobility in the source
  thickness above `crustal_flow_threshold` (40 km) over `crustal_flow_range` (20 km); only
  downhill; per pair capped at the amount that equalises the pair.
- Discretisation: per neighbour pair Δh = κ·m·dt_sub·2/(3d²)·Δz/a_src, explicit sub-cycles with
  κ_max dt_sub/d² ≤ 0.2 (real edge length), computed on the active set (cells above the
  threshold and their neighbours) in dense arrays, written back once per step.
- Receivers: crust of continental origin only, also on another plate (orogens spread over the
  foreland across a collision); never onto oceanic crust (a trench would subduct it). Moves
  crystalline crust only (density, felsic tracer mixed by mass); booked as crustal flow
  (mass-conserving).
- Replaces the current pairwise thickness diffusion (`crustal_flow_rate`).

## C. Less collision stacking

- Collisions subduct part of the lower plate's buoyant crust: at a stacking contact only the
  upper `accreted_fraction` (0.5–0.6; about half of India's crust subducted, Ingalls et al.
  2016, Capitanio et al. 2010; arcs lose their lower crust, Stern & Scholl 2010) is stacked,
  the rest subducts (booked as subducted crust; felsic tracer gives the felsic loss). A physical
  sink besides erosion and the 75 km cap.

## Calibration and verification

- `collapse_diffusivity` within 10³–3·10⁴ on the thickness distribution (> 50 km a few %,
  90th percentile, plateau widths); `rift_width`, `breakup_thickness`, `accreted_fraction` from
  the literature, not tuned.
- Diagnostics: continental thickness percentiles (10/50/90%) and fraction > 50 km over samples
  of continental origin; flooded continental fraction (by origin); stretched cells; collapse
  flux; felsic budget (`--material-budget`); margin widths (cells of stretched crust).
- Targets (seeds 0–3, 500 Myr): mean land → ~0.84 km, flooded continental crust → 25–30%,
  > 50 km → a few %, felsic + sediment within ±10%, continental area not shrinking, sediment
  on margins (re-measure trench sediment input), ocean ages/plates unchanged in range;
  `--no-erosion` run once (collapse must not depend on erosion).
- Tests: stretching conserves mass and felsic mass, tapers over W, stops below the breakup
  thickness (then oceanic crust), raises the thermal uplift by (1 − 1/β) and lets it subside;
  collapse conserves mass, spreads an isolated thick block (also across a collision), never
  flows onto oceanic crust, is stable for large κ dt and resolution independent (s4 vs s5);
  partial accretion subducts the expected fraction; ledger closes; incremental vs full matching
  identical.

## Review of plan v2 (independent agent, 2026-11-17): sound with changes — adopted

Physics: U' = 1 − (1 − U)/β with the GDH1 inverse is a fair stand-in for McKenzie (35 → 17.5 km:
+570 m → ≈ −1.5 km syn-rift → ≈ −3.0 km; GDH1's time constant ~36 Myr vs McKenzie's 62.8 Myr,
so margins subside ~2× too fast — accepted, noted). Uniform stretching over a disc of radius W
is resolution independent (β ≈ 1.5 per opened row at s6, margins ~400 km, mean ~25 km).
Collapse only redistributes (lowers the > 50 km share, not the mean). Flooded 25–30% by origin
is uncertain (~20 rifts per 500 Myr, short-lived margins) — do not tune for it. With this
isostasy, mean land 0.84 km corresponds to ~36–37 km mean continental crust.

Required:
1. C as written removes 16–28 Gt/yr of felsic crust at today's churn (Earth ~0.4–1 km³/yr):
   first measure the collision churn split by real normal convergence vs strike-slip/resampling
   overlaps, add "stack only while converging", do C last with a felsic budget prediction;
   prefer accreting the upper ~15–20 km plus sediment and subducting the rest; own ledger term.
2. Stretching below `min_spreading_rate` only above a small positive threshold (0.1–0.2 cm/yr)
   plus the obliquity test; diagnostic stretched area vs kinematic continental opening.
3. A hidden sample in the gap's slot is revealed, not stretched; consumed slots not stretched.
4. Donors: same plate, continental origin, thicker than `breakup_thickness`; deterministic ring
   order (hidden samples may donate).
5. Collapse per sample (duplicated samples), outflow limited per sample, active set growing
   between sub-cycles.
6. No cross-plate collapse at converging contacts (spread onto the upper plate's own side).
7. Replace every class check by "continental origin" = felsic > 0.5 and resists subduction:
   events (split normal, `seed_margin_subduction`, candidates, rift `continent_nearby`),
   statistics (flooded, freeboard, old passive margins).
8. Closed-form GDH1 inverse (both branches meet at 20 Myr); U' → 0 keeps the old birth; U from
   age alone; stretched cells not in production; stretching booked as zero change; felsic
   budget in the substeps phase.
Optional: stretched young crust gets ridge push and loses the continental drag (check opening
rates); stage A → measure → B → measure → C; `felsic` in the matching state comparison; test
margin profile shape rather than strict halving. Tests: rigid rotation/transform/convergence
do not stretch; hidden slot revealed; donors same plate above breakup; β = 2 profile ≈ −1.5 km
syn-rift then ≈ −3 km after 100 Myr; GDH1 inverse round trip; collapse conserves mass with
duplicated samples, no cross-plate flow at converging contacts.
