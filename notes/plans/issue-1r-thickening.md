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
