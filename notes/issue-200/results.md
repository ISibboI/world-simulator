# #200 results: inorganic carbonate sediment

Model: [plan.md](plan.md) (with the changes after the [plan review](review-plan.md), §9).
Code at `0e614bd` (branch `agent/issue-200`, stacked on #224). Runs: default barren, 8 seeds ×
1000 Myr, against #224's head (`792a095`, same code as `2c41507`; the #197 baseline runs), and
`--biosphere modern --epoch 4`, 4 seeds, against a baseline of `2c41507`. Means of 200–1000 Myr
per seed, then mean ± SD over seeds. Numbers from [`analyse.py`](analyse.py). An earlier run of
`eece481` (same physics, slower bookkeeping) gave the same numbers within the seed scatter.

Project goal: an Earth-like planet, not Earth; Earth values are orientation only.

## 1. Barren planet (default)

### Climate and carbon cycle

| quantity | #224 | #200 | change |
|---|---|---|---|
| pCO₂ (bar) | 0.449 ± 0.068 | **0.378 ± 0.034** | −16% |
| pCO₂ at 1000 Myr (bar) | 0.341 ± 0.047 | 0.299 ± 0.036 | |
| global temperature (°C) | 16.5 ± 0.4 | **15.1 ± 0.8** | −1.4 K |
| φ | 1.37 ± 0.16 | **1.49 ± 0.15** (1.25–1.68) | inside 0.3–3 |
| sources (Tmol/yr) | 12.29 | 11.07 | −1.2 |
| – ridges / plumes | 3.35 / 0.94 | 3.33 / 0.94 | |
| – **arcs** | 7.25 ± 0.57 | **6.43 ± 0.76** | −0.8 |
| – **metamorphic** | 0.76 ± 0.22 | **0.38 ± 0.12** | −0.4 |
| sinks: land / sea floor (Tmol/yr) | 2.81 / 9.54 | 2.56 / 8.56 | |
| land mean elevation (m, snapshots) | 723 ± 372 | 801 ± 193 | scatter |

### Arc and metamorphic CO₂

- **Arcs.** The old term was 0.15 mol C per kg of subducted sediment (3 wt% CaCO₃ assumed,
  half returning): 0.15 × 5.8 Gt/yr ≈ **0.87 Tmol/yr**. The sediment that actually subducts
  carries **0.089 ± 0.028 Gt/yr of carbonate, 1.5 wt%** of 5.8 Gt/yr: the pelagic sediment of a
  barren ocean is clay, and the carbonate sits on shelves of continental crust, which does not
  subduct; what subducts is trench fill, rise sediment and forearc sediment (subduction
  erosion). Half returns: **0.44 Tmol/yr**, i.e. −0.43 Tmol/yr. The rest of the arc drop follows
  the lower CO₂ (the altered-crust return scales with the seafloor uptake).
- **Metamorphic.** The old term was 0.5 mol/kg of metamorphosed and stacked sediment (10 wt%
  CaCO₃, half decarbonated). The actual carbonate in that sediment is less (the deeply buried
  basins and the stacked margins are mostly clastic), and half of it decarbonates: 0.38 instead
  of 0.76 Tmol/yr.
- With ~1.2 Tmol/yr less degassing, φ rises 8% to balance at the start's CO₂; after the ramp-up
  the CO₂ settles ~16% lower, 1.4 K cooler.

### Where the carbonate precipitates

| | |
|---|---|
| precipitated (Gt/yr CaCO₃) | **0.90 ± 0.16**, of which new (land burial 2.56 Tmol/yr) 0.26, recycled 0.64 |
| depth of the sea floor < 20 / 20–200 / 200–1000 / 1000–3000 / ≥ 3000 m | 5 / **69** / 15 / 9 / 2% |
| latitude \|φ\| < 20 / 20–40 / 40–60 / ≥ 60° | **50** / 36 / 12 / 2% |
| on crust of continental origin (shelves, platforms) | **75 ± 7%** |
| saturation Ω at the sea floor: mass-weighted mean / max | **3.0** / **4.2** (level S 3.8) |
| dissolved load taken from the clastic deposits | 0.81 Gt/yr (unmatched −1e-15 Gt/yr) |
| overflow (no room up to the wave base) | 0 |

- Warm, shallow shelves take most of it; deep water is undersaturated or barely saturated (2%
  below 3 km: at S ≈ 3.8 the saturation horizon lies at ~3.5–4 km under tropical water and
  shallower under cold water, and the rate (Ω − 1)² is small near it). Ω of the warmest shelves 4.2 is inside the
  Precambrian range 3–10 named in the plan; since the level is solved to take the burial flux,
  this checks the assumed rate constant k₀ rather than being an independent result.
- At 1000 Myr (cell dumps, carbonate = fraction × sediment per cell, summed over the 8 seeds)
  the shelves shallower than 200 m hold carbonate platforms of ~840 m rock-equivalent on average
  (520–1530 m per seed, ~18% of their sediment); 42% of all carbonate lies on land (15–65% per
  seed: platform cover uplifted or exposed by sea-level change), the rest mostly on the shelves
  and slopes, ~2% below 3 km.
- **Recycling** (a lower bound of carbonate weathering: only erosion dissolves it): 0.64 Gt/yr of carbonate eroded (rivers, coasts, waves) is dissolved and
  precipitated again, 2.5× the new burial. This is carbonate weathering of the continental cover
  (Earth: ~12 Tmol/yr ≈ 1.2 Gt/yr; Gaillardet et al. 1999, *mem.*), the same order. The wave
  reworking of the platforms themselves is kept small by the wave-base cap (review M3).

### Carbonate in the sediment and the carbon reservoirs

| | 100 Myr | 200–1000 Myr | 1000 Myr |
|---|---|---|---|
| carbonate share of all sediment | 8.4% | 11.2% | 12.3% |
| … of the continental cover (start 10%) | 21% | 20% | 19% |
| … transitional / oceanic crust | | 7.1% / 2.6% | |

The cover's share rises from the initial 10% to ~20% within 100 Myr (the precipitation goes onto
the flooded continental margins) and then stays there: the planet's own steady state is about
twice the 10% the start assumed. The carbon in the crust as a whole (sediment carbonate,
marble, altered sea floor) still grows by ~1.3 Tmol/yr, the net degassing of the mantle: the
exogenic carbon falls slowly while the crust takes up carbon, plausible for a planet whose
crustal carbonate inventory is still building up.

Reservoirs at 1000 Myr (10¹⁸ mol C, mean of 8 seeds): exogenic 98 ± 10, **sediment carbonate
1380 ± 311** (745 at 100 Myr), altered sea floor −2 ± 130 (−259 to +161 per seed, see caveats), **locked in crystalline crust
(marble, wedges) 582 ± 142**, source buffers 5. Since the start: 5981 from the mantle (ridges,
wedge, plumes), 4688 into the mantle. Closure residual **≤ 1.3e-14** of the reservoirs (modern runs ≤ 3.5e-13); carbonate
lost with removed crust (merges, removed plates) 0.016 × 10¹⁸ mol (0.001%).

## 2. Modern biosphere (`--biosphere modern --epoch 4`)

| quantity | #224 | #200 |
|---|---|---|
| pCO₂ (ppm, 200–1000 Myr) | ~730 | ~1070 |
| temperature (°C) | 23.6 ± 0.7 | 24.6 ± 1.0 |
| φ | 3.0 (clamped) | 3.0 (clamped) |
| arcs (Tmol/yr) | 4.60 | **6.92** |
| metamorphic | 0.62 | 0.30 |
| sinks land / sea floor | 4.43 / 4.91 | 4.90 / 6.67 |

- The initial pelagic ooze and the ongoing pelagic share (50% above the 4.5 km CCD) make the
  subducting sediment carbonate-rich: **0.37 Gt/yr of 4.1 Gt/yr, 9.1 wt%** CaCO₃ (Earth's
  GLOSS ~7 wt%, Plank & Langmuir 1998: pass). Half returns: 1.86 Tmol/yr instead of 0.61.
- Deposition: 45% at 20–200 m, 27% below 3 km (ooze), 52% on continental crust; Ω mean 1.7,
  max 3.5.
- φ stays clamped at 3 (as before #200), so the extra arc CO₂ raises the CO₂ (~+45%) and the
  temperature (+1 K) instead of being calibrated away.

## 3. Runtime

The barren runs mixed local and GitHub machines (4 local seeds ~980 s, 4 GitHub ~1280 s;
baseline 1158 ± 43 s on GitHub) and say little. The modern runs were all on GitHub: **1018 ± 11
s vs 819 ± 11 s, +24%**. Locally, the same runs with the switch on and off: +9% over 60 Myr
barren (54.4 vs 49.9 s), +11% over 40 Myr modern; per step the precipitation and the exchange
take ~50–70 ms (sample grouping, wave bases, the saturation solve, the sea-level re-solve) and
the erosion phase ~60 ms more (larger state differences later in the run may add to it in the
modern preset).

## 4. Checks

- **Switch off** (`--set carbon.carbonate_sediment=0`): state hashes identical to #224's head over
  30 Myr (held CO₂) and 20 Myr with `carbon.phi=1` (active cycle).
- Unit tests: tracer conservation (`add_sediment`, `remove_sediment_to`, `remove_clastic`),
  saturation distribution and capped fills exact, burial decarbonation removes exactly the CO₂
  mass (ledger closed), the exchange changes the mass by carbonate − clastic (ledger closed), the
  carbon buffers conserve, the planet's carbon closes over 8 steps held and active, off leaves no
  carbonate.

## 5. Caveats

- **Scope of "the arc CO₂ follows the carbonate".** Only the sediment part does: 0.44 of the
  6.43 Tmol/yr barren arc CO₂ (1.86 of 6.92 modern). Most of the arc source is the returning
  altered sea floor, and most of the burial (8.6 of 11.1 Tmol/yr barren) goes into the altered
  sea floor, which is still booked with a fixed mean composition, not carried by the plates.

- **Marble and wedge carbonate is a one-way sink** (review M2, option a): 0.58 × 10²¹ mol after
  1000 Myr (0.5–0.7 Tmol/yr in every seed, ~30% of all crustal carbonate), as large as the new
  carbonate fluxes. When that crust erodes it is treated as silicate rock
  (it even feeds the silicate supply), and subducted metasediment carries no carbonate. A
  `Crust::marble` tracer would close this (follow-up).
- **The altered sea floor is only booked**, not carried by the plates: its reservoir (uptake less
  the 100-Myr-mean slab carbon of the subducted crust) goes well below zero in single seeds
  (−259 × 10¹⁸ mol in seed 0 for ~200 Myr, 2.5× the exogenic carbon): there the arcs return
  more altered-crust carbon than the sea floor took up, an unphysical source of ~0.3–0.5 Tmol/yr,
  as large as the new carbonate arc term. It cancels over the seeds (−2 ± 130) and predates #200
  (#196's fixed slab composition); #200 only makes it visible. Tracking it per
  sample is a follow-up.
- Carbonate weathering in place (karst, chemical denudation without erosion) is not modelled;
  only eroded carbonate dissolves. All eroded carbonate dissolves (no detrital carbonate).
- 0.1 kg/mol (calcite) overstates the mass by ≲ 10% where Mg carbonate forms. The precipitation
  law has no nucleation threshold (a sea-floor cement/ooid law).
- While the CO₂ is held (ramp-up), the burial is the degassing split in the proportion of the
  sinks at φ = 1 (only the first ~150 Myr).
- Grid (#64): a sample shown at several cells takes the mean of their rates and room; carbonate
  platforms are ~110 km wide at least.
- The initial 10% carbonate of the barren cover is half the planet's own ~20%; a start at 20%
  would raise the early metamorphic CO₂ a little.

## 6. Decisions for Sebastian

1. **Modern preset warmer**: φ is clamped at 3, so the extra carbonate arc CO₂ goes into the CO₂
   (+45%, 24.6 °C). Accept, raise the clamp for that preset, or lower its slab carbonate return.
   (The modern subducted carbonate flux, 3.7 Tmol/yr, is at the high end of Earth estimates.)
2. **Marble tracer** (`Crust::marble`, carbonate in crystalline crust, eroded as carbonate and
   subducted with the slab): follow-up issue or extend this PR.
3. **Altered sea floor per sample** (carry the seafloor uptake on the young crust, so the arc
   return follows it and cannot go negative): follow-up issue.
4. **Initial carbonate of the barren cover**: 10% now, the planet settles at ~20%; raise the
   default to 20%?
