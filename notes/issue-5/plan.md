# Plan: relief — mountain heights (#5) and erosion inconsistencies (#7)

_2026-11-20, step 4a of the ordered plan. One PR for both issues. Earlier plan for #5:
[`../plans/issue-4-mountain-heights.md`](../plans/issue-4-mountain-heights.md) (thermal uplift
fades on continental crust, dense lower crust below 50 km, 75 km cap → ≤ ~5.2 km from crust).
Open in #5: "peaks of 8–9.7 km cannot come from crust"._

## 1. Erosion: three inconsistencies (#7)

### 1a. Isostasy in the implicit stream-power solve

`erode()` solves `E = K Q^m S` implicitly per cell from the ocean upstream:
`cut = F (z − z_r) / (1 + F)`, `F = K Q^m dt / L`. That assumes the surface drops by the
removed thickness. It then applies the Airy rebound (`add_layer(…, −cut, ρ_iso)`), so the surface
drops only by `f · cut` with `f = (ρm − ρ)/ρm ≈ 0.15` (crust) — the slope at the end of the step
is higher than the one the solve used, i.e. the scheme is not implicit in the true state. In a
steady state (uplift = erosion) the inconsistent solve needs a slope `(1 + F)/(1 + F f)` times too
high: ~1× for headwaters (F ≈ 0.1) but ~3× for F ≈ 5 and ~6× for big rivers. The continuous
law (dt → 0) does not depend on `f`; this is a time-discretisation error.

**Fix:** solve `h = F (z(h) − z_r)` with `z(h)` the surface after removing `h` with rebound:
- bedrock (linear): `h = F (z − z_r) / (1 + F f)`, `f = dry_response(ρ_iso)` (dense root aware);
- sediment (compaction, nonlinear): bisection on the bulk cut; if the cut goes through all of it,
  the time spent is `bulk / (K_s Q^m (z_bare − z_r)/L)` (rate at the end, implicit), the rest of
  the step cuts bedrock with the linear formula.
- The surface never drops below the base level (by construction).

### 1b. Discharge from runoff, not precipitation

Discharge accumulates `P · A`. On land ~60% of precipitation is evapotranspired (the climate
model already has Budyko ET; land runoff ~0.24 m/yr vs P ~0.82 m/yr; Earth ~0.27–0.30 m/yr).
**Fix:** the climate exports annual potential evaporation (PET) and runoff
`R = P − ET_Budyko(P, PET, ω)` for land cells (the same Fu curve the climate uses for ET, with
the current annual P); discharge accumulates `R · A`. Effect: `Q^0.5` falls by ~0.55 on average,
more in arid land. **K is not re-tuned** (fix physics, don't tune); the effect on relief is
measured and reported.

### 1c. Lakes and endorheic basins

Today every depression fills (by the priority flood) and spills, so all rivers reach the ocean;
Earth has ~18–20% of its land draining internally (Caspian, Aral, Tarim, Great Basin, Chad,
Lake Eyre, Altiplano). A closed lake rises until its evaporation balances its inflow
(Mason et al. 1994; the lake-area / catchment ratio `A_L/A_C = R / (E_L − P)`); it only spills
if it reaches its pass first.

**Fix (per depression of the priority flood, a subtree rooted at its exit cell):**
- Depression cells: land with spill level > floor + 1 m (thinner "lakes" are flats).
- Inflow = runoff accumulated at the exit (catchment + lake cells' own runoff).
- Open water evaporates `PET − ET` more than the land it covers (PET from the climate, the same
  bulk formula at the land temperature). Fill from the lowest floor up: if the cumulative excess
  evaporation reaches the inflow before the spill level, the lake is **endorheic**: its water
  surface lies there, nothing spills, its catchment is endorheic. Otherwise it spills
  `inflow − evaporation` (lake evaporation also reduces spilling outflow, as on Earth).
- **Sediment:** lakes trap what reaches them (trap efficiency of large lakes ~1; Brune 1953): it
  levels the lake floor from the lowest cell up (bisection on a common level, with sediment
  loading via `Seabed::room`), at most to the spill level; the rest passes the outlet. This
  replaces filling each lake cell on the river's path up to the spill level (towers).
- **Base level:** rivers cannot cut below the water surface they flow into (sea or lake surface,
  the endorheic level or the spill level); lake and basin floors are not incised.
- Lake evaporation is not fed back into the atmosphere's water budget (the climate assumes
  runoff returns to the sea): endorheic runoff is a few % of land runoff; noted as a caveat.
- Land below sea level stays ocean by definition (`elevation < 0`), so the Caspian / Dead Sea
  type is not represented; out of scope.

### Diagnostics
Log line per snapshot: runoff, water to the ocean, lake evaporation (km³/yr), endorheic share of
land, lake share of land, sediment trapped in lakes (km³/yr). Earth: runoff ~40,000 km³/yr
(~0.27 m/yr), to the ocean ~37,000–40,000, endorheic ~18–20% of land (incl. deserts), lakes
~2–3% of land.

## 2. Mountain heights (#5)

_To be filled after the main-branch diagnosis below._

## 3. Validation

8 seeds × 1000 Myr, main vs branch (means 500–1000 Myr, Welch t, n = 8):
- hypsometry: land p50/p90/p99/max, highest cells (target ≲ 6–6.5 km at ~120 km cells; Earth at
  1°: max ~5.5–6 km, mean land ~800 m incl. ice), cells > 4 / 6 km; local relief;
- sediment flux to the ocean (Earth 15–20 Gt/yr ≈ 7–10 km³/yr rock), largest river;
- endorheic area (Earth ~18–20%), lakes; runoff and water to the ocean;
- flooded continental crust (25–40%), land fraction;
- sediment-part sea-level slope (must not regress from #34's 2.2 m/100 Myr; target ≤ 10);
- felsic + sediment net, trench sediment, runtime (≤ +10%).

Unit tests: implicit incision balance with rebound (bedrock, sediment, mixed; never below base);
lake balance (partial / spilling); humid basin spills, arid basin keeps its water and sediment,
water budget closes (runoff = to ocean + lake evaporation).
