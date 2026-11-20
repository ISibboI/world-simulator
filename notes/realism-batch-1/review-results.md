# Review of the batch 1 results (#47, #46, #45)

_2026-11-20, independent evaluator. Own parsing of the logs and cell dumps (scripts in
`~/tasks/realism-batch-1/tmp/eval/`); no simulations run._

**Verdict: ready with caveats for #47. Not ready to close #46.** The #47 fix does what it claims
(overfill 0, no sediment spikes, mass conserved, no new sediment artefact found in the dumps).
The #46 conclusion and the explanation of the side effects do not follow from the data.

## Findings, most important first

1. **#46: single-cell pits are mostly a resampling artefact, not arid playas.** Measured
   (600–1000 Myr, 8 seeds; a pit is a land cell > 1 m below all neighbours): 57% (main) and
   59% (branch) of the pits sit on a duplicated crust sample (`dup` > 1), against 15–16% of all
   land cells. That is 3.7× more than chance. Pits are not drier than other land: 26–28% of
   pits get < 0.25 m/yr against 25–27% of land, and mean pit precipitation is 0.94–0.99× the
   land mean. The `lake20` run does not test single-cell pits. It changed every depression
   shallower than 20 m. Endorheic land fell by 8 points (11.0 → 3.0), but only ~3.4 points of
   that drained into single-cell pits (4.5 → 1.05). Inference: the doubled deposits and erosion
   on shared samples (plan review, finding 4) are the likely source of the pits. The review
   asked for a tectonics issue if shared samples dominate, and they do for pits. Change: do
   not close #46. Open that tectonics issue, rewrite "single-cell pits are how the mesh resolves
   arid interior drainage" in results.md, and keep the 20 m variant as the reason why a global
   threshold is the wrong tool.
2. **The terminal-fan cone crosses drainage divides.** Code: `build_terminal_fan` searches
   through every land cell, however high, and one cone surface fills every cell under it within
   64 rings. A basin or lake behind a ridge 5–30 cells away gets sediment that could not reach
   it. Measured: lakes fall from 2.91% to 2.08% of land in all 8 seeds (paired t −4.7). Inference:
   this side effect is partly this non-physical fill. Change: grow the cone only through cells
   below the current cone surface (a flood fill under the surface, redone per ring doubling), or
   open a follow-up issue.
3. **The "tectonic" mountain islands are not at main's rate.** Measured, 0–1000 Myr, 88
   snapshots each: single-cell land islands more than 2 km high occur 7 times on the branch, 0
   on main, 2 in `nograded` and 1 in `noterminal` (44 snapshots each). All are sediment-free
   (0–46 m) with 42–59 km crust, mostly continental cells set in oceanic crust. Main's two
   sediment-free spikes (seeds 2 and 5) are not islands: they have 4–6 land neighbours. The
   eighth branch spike (seed 5, 900 Myr, 67 km) has 2 land neighbours. Islands above 1 km
   occur at similar rates (main 23, branch 30). Zero against 10 across all branch variants has
   p ≈ 0.02, and the mechanism is unknown. Change: say "not sediment, not #47; rate higher than
   on main, cause unknown". Do not say they are the same kind as main's.
4. **The explanation of the side effects is a hypothesis, not a result.** Measured, branch minus
   main per seed: land −7.4, −2.7, +1.3, −2.7, +3.1, −4.8, +1.0, −1.7 points (paired t −1.4;
   without seed 0, −0.9). Flooded continental crust −7…+16 (t +1.3). These two changes are
   not robust. Land below 10 m (+1.0, t +4.2), shelf (+1.2, t +2.9) and lakes (−0.9, t −4.7)
   are robust. Burial metamorphism on continental crust falls in every seed but one (−0.07 Gt/yr).
   Its per-seed change does not track the land change (r = +0.24; seed 1: +0.02 with land
   −2.7, seed 4: −0.08 with land +3.1). Hypsometry of continental crust (% of surface): 300–1000 m
   10.4 → 8.5, 1–2 km 2.5 → 2.1; −200…0 m 5.9 → 7.1, 10–100 m 4.4 → 5.7. So high land
   becomes low plains and shallow shelf. That is consistent with the explanation but does not
   prove it. Change: report land and flooded crust as "not significant; seed 0 carries half",
   and give the hypsometry instead of the metamorphism argument.
5. **Acceptability (#47 side effects).** Land 27.5%, flooded continental crust 36%, lakes 2.0%
   of land and endorheic 10% are inside an Earth-like range. Land below 10 m is 3.3%
   (2.2–4.6%), above #22's ≲ 3% target. Acceptable with a note in the PR. Lakes are at the low
   edge, and finding 2 may cost them.
6. **#45: closing is defensible, but not on the slope test.** The slope over 500–1000 Myr is
   8.9 (main) and 10.0 (branch) m/100 Myr, with seeds from −1.5 to 22.6. The data cannot tell
   apart a target of ≤ 10 from one of 15. The level at 1000 Myr is unchanged (284 vs 291 m,
   t +0.4). Close #45 as "Earth-exact only, level unchanged", not as "target met".
7. **Code: no mass bug, two small points.** (a) The bisection keeps `room(low) ≤ amount`, and
   the deposits repeat the same sums, so `deposited == room(low)` and `.min(amount)` never
   triggers. The cone cannot over-deposit. It leaves `amount − room(low)`, bounded by the
   resolution: (high−low)/2⁵⁰ in height × cells under the cone, below 10⁻⁶ m. When that is
   > 1e-9 it is still piled on `source_cell` and counted as overfill (invisible at `{:.0}`
   km·cells). Put it on the apex instead. (b) With `terminal_fans=0`, a remainder in (0, 1e-9]
   is now dropped (a mass leak of ~10 m³, negligible). Marks are reset on every path
   (`fill_fan` and the cone reset all pushed cells, and only pushed cells are marked).
   `reaches_rim` is correct (no cells at ring `rings` means a closed search). Apex on a
   slope cell surrounded by deep water: only the apex and land or shelf cells get the cone. No
   such single-cell bathymetric piles show in the dumps (cells above −200 m next to < −2500 m
   with > 3 km sediment: 177 vs 176). The statistics plumbing is correct, but the terminal fans
   are missing from the per-zone split of "sediment to ocean", as the overfill was.
8. **The window label is wrong.** The snapshot means use 600–1000 Myr (5 snapshots,
   `T0 < t`). The cumulative counters and the slope use 500–1000. results.md says 500–1000 for
   all. Fix the label.

## Re-derived numbers (own parser; all match results.md within rounding)

Overfill 4373 → 0. Terminal fans 3982 (1304–6174), on seeds 0–3: b1 3562, nograded 5697,
noterminal overfill 3907. Sediment spikes (> 2 km, sea = 0 m), 48 snapshots: main 4 (all
12.9 km), branch 0. Cells > 500 m above all neighbours: 0.14% → 0.09% of land. Land 29.22 →
27.48. Flooded 32.30 → 36.20. Land < 10 m 2.31 → 3.29. Shelf 6.28 → 7.52. Lakes 2.91 → 2.02.
Endorheic 11.29 → 10.24, into pits 5.5 → 5.5. Sediment part: mean 269 → 269, at 1000 Myr 284
→ 291, slope 8.85 → 10.02. Burial metamorphism on continental crust 0.20 → 0.13. Runtime 592 →
529 s. lake20 at 100–400 Myr: endorheic 11.0 → 3.0, lakes 2.8 → 0.9, evaporation 3210 → 717.
Oceanic sediment on crust ≥ 200 Myr: 45.9 → 51.4% (results: 44.6 → 49.9, other definition),
8.5 → 9.1 km.

No new sediment artefact in the dumps: land cells with > 8 km sediment are 944 vs 916 (22%
coastal in both). The sediment-thickness distribution on land is unchanged (p90 6.6 vs 6.3 km;
3.8 vs 4.1% of land at the 12.9 km cap, an old feature of both). There is no peak at the cone
fall (22 m) in neighbour differences, and flat sediment plains are unchanged. Single-cell islands
with > 1 km sediment: 9.8 → 10.9 per snapshot (t +1.1; t +3.8 on seeds 0–3), small.
