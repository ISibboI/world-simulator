# #113: crustal flow and subduction erosion act once per sample — results

Plan: [plan.md](plan.md); plan review: [review-plan.md](review-plan.md) (its findings were
acted on, see the plan's last section); independent results review:
[review-results.md](review-results.md). Full tables: [compare.txt](compare.txt) (log metrics)
and [orogens.txt](orogens.txt) (land elevation from the cell dumps); scripts in
[scripts/](scripts/).

## What changed

A crust sample can be shown at k > 1 cells (about 11% of the cells since #112). The books count
it as one cell of area. #104 made surface processes and profile additions count such a sample
once; this does the same for the last two paths that still acted once per cell.

1. **Crustal flow** (`crustal_flow_inner`): every transfer between two cells is divided by the
   larger number of cells showing its source or its receiver sample.
   - A duplicated sample now exchanges the mean of its cells' exchanges instead of their sum.
   - The partner gets exactly what the source gives, so mass stays conserved locally and nothing
     has to go to hidden samples.
   - The partner's edge to a duplicated sample conducts 1/k. That fits the geometry: the
     duplicated sample stands for a larger area whose centre lies further away.
2. **Subduction erosion** (`erode_overriding_samples`):
   - It now runs after the final assignment of samples to cells (after distinct resampling and
     gap filling) and erodes the sample each event's trench cell finally shows.
   - A sample shown at k cells with n events is thinned once by n/k × 300 m. The rest, (1 − 1/k)
     n × 300 m, is taken in equal parts from its neighbours on the plate (the adjacent forearc).
   - So the erosion per subducted sample stays 300 m. It stays in balance with arc growth, which
     `apply_additions` adds in full per event. Only the distribution changes: one column is no
     longer thinned k×.
3. **A log line per snapshot** ("duplicated samples act once so far") reports the effective rate
   factors.

Unit tests:
- A source shown at two cells loses exactly 10/12 of its single-cell loss; the old code took
  20/12.
- A receiver shown twice takes half.
- A twin-hidden sample takes no part.
- Mass is conserved to 1e-12.
- Erosion: a sample at two trench cells is thinned once and its neighbours give the rest, and the
  total equals that of a single cell with two events. One event over two cells takes sediment
  first, and the ledger matches the mass removed.

Determinism: only integer counts and sums in a fixed order (cell order, sorted samples).

## Effective rates (branch, 16 seeds, cumulative to 1000 Myr)

| | |
|---|---|
| crustal flow applied / per-cell request | **78.8%** (76.8…81.4) |
| subduction erosion on the eroded samples / events | 90.2% |
| subduction erosion moved to their neighbours | 9.8% |
| subduction erosion, crust budget (Gt/yr, 500–1000 Myr) | −2.78 (main −2.76) |

- **Crustal flow −21%.** Duplicated samples are over-represented in orogens: 17–19% of the cells
  with crust > 40 km or land > 2 km show one, against 9% overall (results review).
  - In terms of the old per-cell counting, the effective `collapse_diffusivity` goes from 1e4 to
    about 7.9e3 km²/Myr.
  - That is still inside the physical range of lower-crustal channel flow (3.7e3–3.7e4, Clark &
    Royden 2000).
  - The value was chosen physically when collapse was introduced (73ea6c7), not fitted against
    the double counting, so it is kept.
- **Subduction erosion: total flux unchanged by design.** About 10% of it now lands on the
  neighbouring forearc samples instead of on a duplicated one.

## Calibration check

16 seeds (0–15) × 1000 Myr, default parameters (`--duration 1000 --cell-dump
--snapshot-interval 100`), branch 714df0f against main cf409e1. Values are means over the
500–1000 Myr snapshots: mean (seed range), Welch t.

| metric | main | branch | t |
|---|---|---|---|
| crust > 50 km (% of continental) | 1.64 (0…6.2) | 1.01 (0…4.2) | −1.2 |
| mean land (m) | 594 (309…1152) | 495 (197…907) | −1.3 |
| land (% of surface) | 26.9 | 28.4 | +1.3 |
| continental (% of surface) | 33.1 | 33.6 | +0.6 |
| flooded continental crust (%) | 31.3 | 28.9 | −0.7 |
| felsic + sediment change (Gt/yr) | −1.78 | −1.39 | +0.7 |
| continental crust change (Gt/yr) | −2.75 | −3.70 | −1.7 |
| collision stacking (Gt/yr) | 21.1 (4…40) | 16.5 (1…36) | −1.3 |
| highest point (m) | 4141 | 3832 | −1.7 |
| land p90 / p99 (m, cell dumps 600–1000 Myr) | 1811 / 3022 | 1540 / 2625 | −1.1 / −1.7 |
| land crust p99 (km) | 50.1 | 47.7 | −2.1 |
| ocean deeper than 6 km (%) | 2.85 | 4.61 | +3.6 |
| sea floor ≥ 200 Myr (%) | 5.4 | 3.3 | −3.3 |

**Noise.** Plate tectonics diverges chaotically after the first changed step. Seeds stop tracking
their main counterpart by 300–500 Myr, and collision activity is bimodal per seed: either none or
15–60 thousand km.
- The two halves of main (seeds 0–7 against 8–15, same code) already differ by t −1.9 on old sea
  floor and −1.1 on crust > 50 km.
- Over all 8-vs-8 splits of main, 95% of |t| lie below 2.0–2.3 (results review).

### Diagnostic variants (seeds 0–7)

Only the crustal-flow change, and only the erosion change:
- **flow-only:** orogens lower (land p99 2158 m, t −3.1), collisions halved (t −2.6).
- **erosion-only:** within noise (|t| ≤ 1.7 on these).

The results review looked at them in detail and concluded that flow-only's drop most likely comes
from seed regimes:
- Its direction is wrong for less collapse, which should keep orogens higher.
- It is explained by fewer active collisions. Seeds 1–3 end without collisions, and seed 3 does
  so in main too.
- The full branch, with the same flow change, does not reproduce it. Its orogen drop sits
  entirely in seeds 0–7, and seeds 8–15 against main 8–15 give t −0.6 to +0.5.

A modest real reduction cannot be fully excluded from 8 seeds.

### Reading

- **No metric left its plausible range** for an Earth-like planet. All branch means lie inside
  main's seed ranges.
  - Crust > 50 km is 1.0% (Earth roughly 2–3%; strongly regime-dependent).
  - Mean land is 495 m, a bit low against Earth's 840 m, as main already is.
  - Land is 28% and continents cover 34%.
  - The continental balance is unchanged within noise.
  - The highest point averages 3.8 km (5.3 km at most). Cells are ~110 km, so peaks are
    averaged.
- **Orogens slightly lower** (t −1 to −2): probably noise. Most likely they are unchanged or at
  most a few hundred metres lower.
- **Ocean deeper than 6 km** (+1.75 points, t +3.6): robust against main, but not explained by
  either change.
  - It appears only after 600 Myr, when main's seeds 0–7 have a late collision phase.
  - It is not additive (flow-only +1.7, erosion-only +1.2, both +1.8).
  - Neither change acts on the deep ocean: flow moves continental crust only, and erosion acts on
    the overriding plate.
  - Main's 16 runs being an unusually shallow set (P ≈ 4e-5 for a random 16 of the 48 runs) fits
    best. 4.6% is still plausible.
- **Old sea floor ≥ 200 Myr down** (t −3.3): attributed to the erosion change by the variants, but
  it shows only at 700–800 Myr, and the main split alone gives t −1.9. The direction is harmless
  (less ancient ocean is a long-standing goal, #14).
- **No re-tuning.** `collapse_diffusivity` stays physical at its new effective rate, and the
  `subduction_erosion` flux is unchanged.
- **Runtime unchanged:** a paired local 150 Myr run (seed 0) took 154 s on main and 148 s on the
  branch. The GitHub runtimes differ by runner machines.

## Caveats

- The flow damping applies to the hole filling of #129 as well. A one-cell hole fed by a
  duplicated sample fills more slowly; it is still decided every step.
- Where both sides of an edge are duplicated, the side with the smaller k exchanges less than its
  own mean. This is the price of a symmetric, conservative factor.
- If more certainty on the orogens or the deep ocean is wanted, more seeds of main and branch
  (16–31) would settle it. More diagnostic variants would not.
