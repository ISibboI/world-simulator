> **Archive as of 2026-11-19.** Issues are tracked on GitHub (https://github.com/ISibboI/world-simulator/issues); per-issue notes live in `notes/issue-<N>/`; see `CLAUDE.md`. Do not extend this file.

# Known physical-accuracy issues

**The issues now live on GitHub:** https://github.com/ISibboI/world-simulator/issues (moved
2026-11-19). Record new issues and progress there, not here. Each GitHub issue holds the full
text it had here and its plans from `plans/` as comments (the plan files stay in the repository).

Older notes, plan file names and session logs use the old numbers below. Cite issues as "issue
13 (#14)" (old number, GitHub number) where the old number is still in use.

| Notes issue | GitHub | Title |
|---|---|---|
| 1 (incl. findings 1a–1r) | [#2](https://github.com/ISibboI/world-simulator/issues/2) | Ocean floor too old, plates only die (plate dynamics) |
| 2 | [#3](https://github.com/ISibboI/world-simulator/issues/3) | Sea level fixed at 0: ocean water volume not conserved |
| 3 | [#4](https://github.com/ISibboI/world-simulator/issues/4) | Continental crust not conserved with erosion |
| 4 | [#5](https://github.com/ISibboI/world-simulator/issues/5) | Mountains too high: thermal uplift on continental/arc crust with inherited young oceanic age |
| 5 | [#6](https://github.com/ISibboI/world-simulator/issues/6) | New oceanic crust created at convergent boundaries |
| 6 | [#7](https://github.com/ISibboI/world-simulator/issues/7) | Erosion inconsistencies (isostasy in stream power, runoff, endorheic basins) |
| 7 | [#8](https://github.com/ISibboI/world-simulator/issues/8) | Shelves are sediment piles on oceanic crust |
| 8 | [#9](https://github.com/ISibboI/world-simulator/issues/9) | Precipitation too low at mid-latitudes |
| 9 | [#10](https://github.com/ISibboI/world-simulator/issues/10) | Temperature details: poles too warm, land heat capacity, subtropical seasonality |
| 10 | [#11](https://github.com/ISibboI/world-simulator/issues/11) | Plates break apart along straight lines |
| 11 | [#12](https://github.com/ISibboI/world-simulator/issues/12) | Ocean wave (coastal) erosion |
| 12 | [#13](https://github.com/ISibboI/world-simulator/issues/13) | Asteroid impacts |
| 13 | [#14](https://github.com/ISibboI/world-simulator/issues/14) | Ancient ocean (≥ 200 Myr) persists and traps sediment |
| 14 | [#15](https://github.com/ISibboI/world-simulator/issues/15) | Initial continents and plates are hexagonal |

Plans by issue: 1 (#2): `plate-dynamics`, `issue-1-soundness`, `issue-1-tuning`,
`per-segment-polarity`, `boundary-friction`, `mass-conserving-gaps`, `issue-1r-thickening`,
`local-splits`, `terrane-accretion`; 2 (#3): `issue-2-sea-level`; 3 (#4):
`issue-3-mass-conservation`, `issue-3-continental-mass-balance`, `sediment-routing`,
`trench-sediment`, `accretionary-margins`, `intra-oceanic-arcs`; 4 (#5):
`issue-4-mountain-heights`; 5 (#6): `issue-5-gap-filling`; 7 (#8): `margin-stretching`;
13 (#14): `issue-13-ancient-ocean`; 14 (#15): `issue-14-initial-conditions`.

## History

The list started from the independent physics review of commit fb5ffb6 (2026-11-15) and the
shelf check (details: `sessions/2026-11-15.md`; feasibility research per issue:
`research/feasibility-2026-11-15.md`). Order of work until the move (user, 2026-11-15:
calibrate issue 1 only as far as needed to confirm the implementation, then prefer issues that
also help issue 1, then work through the rest autonomously, with an independent plan review
before each issue): 1 (mechanisms) ✓ → 5 ✓ (partly) → issue 1 soundness fixes ✓ → 3 ✓ (ledger;
artifacts remain) → 4 ✓ (peaks still too high) → runtime optimisation review ✓ → issue 1 tuning
✓ → per-segment polarity ✓ → boundary length and collision thickening (1o a/b) → 2 ✓ → 1r
(mass-conserving gap healing, stretching/collapse) → issue 3 continental mass balance → 1q
relative-speed tuning (later, user) → 7 + 6 → 9 → 8. Issues 13 and 14 were added on 2026-11-19
(14 done the same day). The current order of work is the "Current plan" in `SUMMARY.md`.

Minor: statistics count land as `> 0` in one place and `>= 0` elsewhere.

## Acceptable simplifications (at ~110 km cells, 1 Myr steps)

Local Airy isostasy (no flexure, trenches, forebulges), constant densities, no compaction,
age-only ocean depth (no dynamic topography), nearest-neighbour resampling of rigid plates,
diffusive crustal flow, n = 1 stream power with calibrated K, no hillslope/glacial erosion, no
lithology, fixed three-cell winds without monsoons, single upwind neighbour, zonal energy
balance without ocean currents, Budyko annual-mean ice criterion, 50 m mixed layer.
