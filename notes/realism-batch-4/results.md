# Batch 4 results: suture marks (#40), plume rifts far from their plume (#52)

_2026-11-21. Plan: [`plan.md`](plan.md), its review: [`review-plan.md`](review-plan.md).
Goal: an Earth-like planet, not Earth; effects of the ~110 km grid are accepted and named._

## Runs

8 seeds (0–7) × 1000 Myr each, default parameters, snapshots every 100 Myr.

| set | code | where |
|---|---|---|
| `main` | main at 4bcd243 (before PR #68) | local |
| `v1` | 4bcd243 + orogenic share (#40) + rift diagnostics | local |
| `v2` | `v1` + rift anchor (#52) | GitHub runners |
| `m68` | main at 74e80e3 (with PR #68) | GitHub runners |
| `m68v1` | 74e80e3 + orogenic share + diagnostics | GitHub runners |
| `m68v2` | this branch (merged with 74e80e3) | GitHub runners |

The branch was merged with main after PR #68 landed, so every comparison was repeated on the
new base; both are given. Runs diverge after the first changed event (chaotic), so sets are
compared as distributions: mean over seeds, seed range, Welch t. Scripts:
[`scripts/`](scripts/) (`marks.py`: marks in the cell dumps; `rifts.py`: the debug lines of
`events.rs`; `compare.py`: log metrics). Runtimes of sets on different machines are not
comparable; locally `main` 742 s vs `v1` 725 s per run (no measurable cost).

## #40: how much does the max rule over-spread? Little

Share of continental cells (mean of 8 seeds, %), by time:

| Myr | 200 | 400 | 600 | 800 | 1000 |
|---|---|---|---|---|---|
| `m68`: marked (max rule) | 11.9 | 23.9 | 32.9 | 40.8 | 47.2 |
| `m68v2`: cells with any orogenic share | 12.1 | 23.6 | 33.7 | 41.8 | 47.8 |
| `m68v2`: mean orogenic share | 11.2 | 22.0 | 31.1 | 38.3 | 44.1 |
| `m68v2`: cells with share ≥ 0.5 (= majority rule) | 11.3 | 22.1 | 31.2 | 38.6 | 44.3 |
| `m68v2`: cells with share ≥ 0.9 | 10.6 | 20.7 | 29.0 | 35.5 | 40.9 |

(Old base: marked 47.5 on `main`; `v1` any 48.5, mean 42.4, ≥ 0.5 42.8, ≥ 0.9 37.3.)

- The marked area grows by ~5–6% of the continental area per 100 Myr in every set. **That
  growth is not the mixing rule**: 92% (old base 87%) of all orogenic lithosphere sits in
  cells with a share ≥ 0.9, i.e. cells marked directly by a collision, suture or accretion.
  The premise of the issue (the mark spreads through repeated mixing) holds only for the
  difference between "any share" and the mean share: the max rule overstates the weakened
  area by 3.7 points of 44 (8% relative; old base 6 of 42, 14%) after 1000 Myr.
- The feared opposite, belts diluted until they no longer attract rifts (plan review, point
  1), does not happen either: only 1.7% (old base 2.7%) of the orogenic total is in cells
  with a share below 0.5.
- Mass-weighted share and majority rule give the same area (44.1 vs 44.3%). The share was
  implemented (option 2 of the issue): it needs no threshold, costs one `f64` per sample and
  no measurable runtime.
- Whether ~45% of the continental area being orogenic after 1000 Myr is plausible: on Earth
  roughly a third of the continental crust has a Phanerozoic (< 540 Myr) tectonothermal age
  (e.g. Artemieva 2006); the simulation marks ~29% in 500 Myr. Plausible for an Earth-like
  planet. The mark is permanent, so the share keeps rising towards saturation in longer
  runs, and the contrast between belts and cratons shrinks with it; that is a property of
  "weak for good" (`SUTURE_WEAKENING`), not of the mixing, and not changed here.

Rifts still follow the belts. Share of the continental cells on new rift boundaries that carry
a mark vs the share in the rifted plates (log statistic, unchanged definition, end of run,
mean of 8 seeds):

| | on rift boundaries | in the rifted plates | ratio |
|---|---|---|---|
| `main` | 53% | 27% | 1.9 |
| `v1` | 51% | 29% | 1.8 |
| `v2` | 52% | 26% | 2.0 |
| `m68` | 47% | 27% | 1.7 |
| `m68v1` | 49% | 29% | 1.7 |
| `m68v2` | 46% | 26% | 1.8 |

Per seed the first number ranges from 25 to 69%; the sets do not differ. A statistic using the
share instead of the mark (plan review, point 2) was not added: 92% of the orogenic
lithosphere is in cells with share ≥ 0.9, so it would show the same.

## #52: why 30% of the plume rifts ended far from their plume

Diagnostics on `v1`/`m68v1` (rifts before the fix): of 363 (235) plume rifts, 28% (29%) had
their closest boundary cell more than 500 km from the plume, as reported in the issue. Of
these far rifts:

| cause | `v1` | `m68v1` |
|---|---|---|
| the cut passed the plume (< 250 km), then pieces were moved to the other side | 97% | 96% |
| rigid crust assigned whole to one side moved the boundary away | 0% | 0% |
| the cut itself missed the plume | 3% | 4% |

The cause is the last step of `local_split_piece`: pieces of one side that are not connected
to that side's main piece are moved to the other side, and the main piece was "the largest".
When the cut left a small piece at the plume and a larger block of the same side elsewhere
(typical for plumes near a plate edge or under a continent held together as rigid crust), the
piece at the plume changed sides and the boundary at the plume vanished (median 41 cells
moved). This is bookkeeping, not physics: a break nucleates at the plume. Fix: for plume
rifts the two sides are anchored at the cells next to the plume.

| | plume rifts (8 runs) | farther than 500 km | median distance |
|---|---|---|---|
| `v1` (old base, before) | 363 | 28% | 111 km |
| `v2` (old base, after) | 296 | 4% | 0 km |
| `m68v1` (before) | 235 | 29% | 111 km |
| `m68v2` (after) | 188 | 4% | 0 km |

- The remaining 4% all have the plume within ~1.5 cells (≤ 150 km; two cases at 150–500 km)
  of the plate edge, where one of the two lines of cells that tie the break to the plume has
  no cells inside the plate. A plume at a plate boundary does not split its plate there on a
  real planet either (it sits on a ridge or a margin); at one cell from the edge this is at
  grid scale and left as it is.
- Fewer rifts in total (−18 to −20% in the debug-line count; in the log statistic 2.66 →
  2.35 per 100 Myr, t = −1.5, inside the seed range 1.6–3.3): splits whose plume-side piece is
  smaller than the minimum plate area now fail instead of being replaced by a break
  somewhere else in the plate. Rifts no longer appear far from any plume for this reason.
- Not done: the near-field angle metric the issue asks for. The angle measurement lives in
  the analysis of PR #50 (branch `agent/issue-30`), not on main; with the boundary now at
  the plume it can be added there.

## Guard rails

`m68` vs `m68v2`, 500–1000 Myr, 69 metrics: [`compare-main-branch.md`](compare-main-branch.md).
No metric differs by |t| ≥ 2 except the runtime (different machines, debug log). Examples:
trenches 112 → 110 thousand km, ridges 109 → 105, median sea-floor age 34.6 → 35.7 Myr,
continental area 33.1 → 32.7%, land 28.3 → 29.4%, highest point 4874 → 5295 m (t = +1.3),
global temperature 14.1 → 13.9 °C, initiations 7.3 → 6.6, sutures 0.97 → 0.88 per 100 Myr.

On the old base, `main` vs `v2` had four metrics at |t| 2.0–2.9 out of 69 (trenches +6%,
slow boundaries 9.6 → 6.5%, median sea-floor age 34.6 → 32.2 Myr, sea floor < 20 Myr
33.0 → 35.2%); none of them repeats on the new base, so they are read as chance.

## #53 and #51: recommendations

- **#53** (plume-head weakening has no memory and does not move with the plate): unchanged
  recommendation, low priority. With `rift_delay` 20 Myr and plates at ~5 cm/yr the heated
  lithosphere would trail up to ~1000 km behind the plume, about one head radius, so the
  field in the mantle frame misplaces the weak zone by up to its own size for fast plates.
  A decaying "plume heat" on the crust sample would cost what `orogenic` costs. It is worth
  doing together with the thermal-age question of the issue, not as part of this batch.
- **#51** (cut cost weighted by the plume's stress instead of the nucleation lines): the
  far rifts were not caused by the lines, and with the fix 96% of the rifts are at the plume.
  #51 remains a cleaner formulation of the same thing, not a realism problem: low priority.

## Overlap with other work

PR #68 (#43) is merged into this branch; no conflicts. Branches that add `Crust { … }`
literals (e.g. `agent/issue-42`) need the new field `orogenic` after this lands.
