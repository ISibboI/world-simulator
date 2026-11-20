# #31 results: suture mark kept on continental lithosphere

## Validation before the rebase onto #21 (8 seeds × 1000 Myr)
`main` at ce11e5d (#23) against this branch at the same base, seeds 0–7. Marked shares are per cell class from `--cell-dump` (the class is judged on the basement, `main.rs` `write_cell_dump`), pooled over the snapshots from 500 to 1000 Myr.

| Marked share of cells | main | branch |
|---|---|---|
| Oceanic, 500–1000 Myr (per-seed range) | 0.26% (0.17…0.37) | **0.000%** in every seed and snapshot |
| Oceanic at 1000 Myr | 0.40% (0.12…0.73) | **0.000%** |
| Transitional | 23.1% | 8.4% |
| Continental origin | 38.7% | 39.1% |

What main's marked transitional cells are, from the independent evaluation, pooled over snapshots:
- 77.5% thinned felsic margins;
- 11.3% metasediment wedges on oceanic basement;
- 11.1% mafic arc or plateau crust.

On the branch they are 97% margins (median felsic fraction 0.93, 12.5 km thick, no metasediment). These are belt lithosphere that was marked while it was continental and was thinned later, which is physical. The marks that were dropped sit in mixtures with sea-floor material: the marked share of margins with a felsic fraction of 0.5–0.7 falls from 29% to 2%, while for 0.95–1.0 it stays at 45% → 43%.

Guard rails: per-seed means over 500–1000 Myr, Welch t (n = 8).

| | main | branch | t |
|---|---|---|---|
| Rift boundary cells on sutures (%) | 53.5 (44…65) | 52 (46…61) | −0.5 |
| … vs all continental cells of the rifted plates (%) | 29.9 | 28.8 | −0.6 |
| Mean sea-floor age (Myr) | 52.0 | 52.6 | +0.3 |
| Sea floor ≥ 200 Myr (%) | 3.24 | 3.37 | +0.3 |
| Trench length (10³ km) | 125.7 | 127.8 | +0.5 |
| Ridge length (10³ km) | 96.0 (91.5…103) | 101.5 (95.6…107) | +2.9 |
| Collision length (10³ km) | 12.5 | 14.5 | +1.4 |
| Plates > 1% | 13.9 | 14.7 | +1.7 |
| Continental-origin crust (% of surface) | 40.5 | 40.6 | +0.1 |
| Flooded continental crust (%) | 33.4 | 36.9 | +0.6 |
| Sea-floor production (km²/yr) | 6.72 | 6.84 | +0.9 |
| Rifts / initiations / sutures per run | 39.6 / 74 / 32 | 40.9 / 82 / 32.4 | +0.5 / +1.2 / +0.2 |
| Runtime (s) | 588 | 598 | +0.4 |

**Ridge length rises by 6%.** In an exact permutation test p = 0.013, but with a Bonferroni correction over ~15 metrics p ≈ 0.2, so it is not significant.
- It follows the plate count (correlation 0.69 across runs).
- Ridge length per plate is unchanged: 6.94 vs 6.92 thousand km.
- Before 450 Myr, while few cells are marked, ridge length is 88.9 vs 89.6 thousand km.
- The only plausible mechanism is that weak-zone cuts take slightly different routes now that oceanic cells are no longer at half strength.

**Rifts still follow sutures:** 1.79× the background share on main and 1.81× on the branch.

## After the rebase onto current `main` (#21), 4 seeds × 1000 Myr
`main` at fa8da42 against this branch rebased onto it, seeds 0–3.

| Marked share of cells, 500–1000 Myr | main | branch |
|---|---|---|
| Oceanic | 0.26% (0.16…0.31), 0.40% at 1000 Myr | **0.000%** in every seed and snapshot |
| Transitional | 21.9% | 8.2% |
| Continental origin | 39.1% | 36.5% |

| Guard rails, per-seed means over 500–1000 Myr | main | branch | t |
|---|---|---|---|
| Rift boundary cells on sutures (%) | 54.5 | 54.5 | 0.0 |
| … vs all continental cells of the rifted plates (%) | 27.5 | 25.3 | −0.6 |
| Mean sea-floor age (Myr) | 49.4 | 50.4 | +0.3 |
| Sea floor ≥ 200 Myr (%) | 3.13 | 2.92 | −0.3 |
| Ridge length (10³ km) | 99.8 | 92.8 | −0.6 |
| Trench / collision length (10³ km) | 130.1 / 13.4 | 124.6 / 10.2 | −0.4 / −1.5 |
| Plates > 1% | 15.1 | 12.6 | −1.0 |
| Continental-origin crust (% of surface) | 40.4 | 40.6 | +0.2 |
| Sea-floor production (km²/yr) | 6.91 | 6.51 | −0.7 |
| Runtime (s) | 643 | 625 | −1.6 |

**The ridge-length shift of the first batch reverses sign here** (−7%, n.s.), so it is seed scatter and not an effect of the change. No metric differs significantly.

## Follow-ups (not done here)
- **The `max` rule.** Mixing a small marked share into an unmarked continental sample (`spread_crust`, `heal_gap`, `stretch_rift`) marks the whole sample. That spreads marks inside continents by a cell per mixing event. A mass-weighted rule, keeping the mark only if the marked material makes up at least about half of the mixture, would be more faithful.
- **Marks that persist after thinning.** Lithosphere that was marked while continental keeps its mark after it is thinned to transitional crust (~8% of transitional cells). This is physical (hyper-extended margins of a belt). Weak-zone strength applies both the thinning factor and the ×0.5 suture factor to it.
