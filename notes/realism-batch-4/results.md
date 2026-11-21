# Batch 4 results: suture marks (#40), plume rifts far from their plume (#52)

_2026-11-21. Plan: [`plan.md`](plan.md), its review: [`review-plan.md`](review-plan.md).
Independent review of these results: [`review-results.md`](review-results.md) (written against
the first version of this file; its findings are worked in here, see the end).
Goal: an Earth-like planet, not Earth; effects of the ~110 km grid are accepted and named._

## What changed

1. **#40**: crust samples carry the share of their lithosphere that was deformed in an orogeny
   (`Crust::orogenic`, 0–1), mixed by crust mass wherever samples are mixed; the strength of
   the lithosphere falls in proportion to it (`1 − 0.5 · share`). Before, a sample was weak
   as a whole if any contributor was marked ("max rule").
2. **#52**: for plume rifts the two parts of the plate are anchored at the cells next to the
   plume (before: at each side's largest piece), and a division whose break ends more than
   half a plume-head radius (500 km) from the plume is not a rift.

## Runs

8 seeds (0–7) × 1000 Myr each, default parameters, snapshots every 100 Myr. Main moved twice
during the work (PR #68, PR #73); the branch was merged with it each time and the comparison
repeated. The last base is the one that counts; the earlier ones are independent repeats.

| set | code |
|---|---|
| `main`, `v1`, `v2` | main at 4bcd243; + orogenic share and diagnostics; + rift anchor |
| `m68`, `m68v1`, `m68v2`, `m68v3` | main at 74e80e3 (with #68); + share; + anchor; + rejection of far breaks |
| `m73`, `m73v3` | **main at d3127cb (with #73); this branch** |

Runs diverge after the first changed event, so sets are compared as distributions: mean over
seeds (seed range), Welch t. Scripts: [`scripts/`](scripts/) (`marks.py`: cell dumps;
`rifts.py`, `rift_counts.py`: debug lines of `events.rs`; `compare.py`: log metrics).
Runtime: the only clean pair is local, `main` 742 s vs `v1` 725 s per run (no measurable
cost of the share). The other sets ran on a mix of GitHub runners and this machine, with
debug logging on the branch; their runtimes (e.g. `m73` 506 s vs `m73v3` 555 s) are not
comparable. The code added for #52 is one pass over the cells per split attempt.

## #40: how much does the max rule over-spread? Little

Share of continental cells (mean of 8 seeds, %), `m73v3`. The branch still keeps the time of
the last orogeny with the max rule, so the old mark and the new share can be compared **in
the same runs**:

| Myr | 200 | 400 | 600 | 800 | 1000 |
|---|---|---|---|---|---|
| marked by the max rule (= cells with any share) | 10.9 | 23.7 | 35.7 | 45.3 | 51.4 |
| mean orogenic share | 10.1 | 22.0 | 32.8 | 41.7 | 47.4 |
| cells with a majority share (≥ 0.5) | 10.1 | 22.0 | 32.9 | 41.9 | 47.7 |
| cells with share ≥ 0.9 | 9.6 | 20.7 | 30.6 | 38.7 | 43.9 |
| `m73` (main): marked | 10.9 | 24.5 | 35.4 | 42.9 | 48.5 |

Earlier bases at 1000 Myr, any share / mean share: `v1` 48.5 / 42.4, `m68v2` 47.8 / 44.1.

- Mixing by mass conserves the orogenic share × crust mass in every mixing of samples, so
  the mean share can only grow where a collision, suture or accretion marks crust. The max
  rule's surplus is the difference between "any share" and the mean share: **4.0 points of
  47 after 1000 Myr** (8%; earlier bases 3.7 of 44 and 6.1 of 42). That is the answer to the
  issue's question: the max rule over-spreads, but by little.
- The growth of the weak area, 5–6 points of the continental area per 100 Myr in the first
  500 Myr and 3–4 afterwards, comes from the marking itself: a collision marks the crust it
  thickens beyond 35 km within its stacking profile (up to 4 cell rings, ~330 km), for as
  long as it lasts. Orogens of that width are real; the area marked scales with the length
  and duration of collision boundaries.
- The feared opposite (plan review, point 1), belts diluted until they no longer attract
  rifts, does not happen: 1.7% of the orogenic total is in cells with a share below 0.5, 92%
  in cells with a share ≥ 0.9.
- The area with a majority share equals the mean share (47.7 vs 47.4%). This is not a
  measurement of what the majority rule (option 1 of the issue) would give, since that rule
  rounds at every mixing. The share was implemented (option 2): no threshold, one `f64` per
  sample, no measurable runtime.

Rifts still follow the belts. Share of the continental cells on new rift boundaries that carry
a mark (max-rule mark, the unchanged log statistic; on the branch this is "any share") vs the
share in the rifted plates, end of run, mean of 8 seeds:

| | on rift boundaries | in the rifted plates | ratio |
|---|---|---|---|
| `main` / `v1` / `v2` | 53% / 51% / 52% | 27% / 29% / 27% | 1.9 / 1.8 / 1.9 |
| `m68` / `m68v1` / `m68v2` / `m68v3` | 47% / 49% / 46% / 44% | 27% / 29% / 26% / 27% | 1.7 / 1.7 / 1.8 / 1.6 |
| `m73` / `m73v3` | 41% / 52% | 22% / 26% | 1.8 / 2.0 |

Per seed the first number ranges from 22 to 73%; the sets do not differ.

### Caveat: the weak area has no upper limit (new issue proposed)

After 1000 Myr half of the continental cells carry some share and 44% are orogenic as a
whole; the area still grows by ~3 points per 100 Myr and nothing removes the mark
(`SUTURE_WEAKENING` is "for good"). In runs of 2–3 Gyr the contrast between belts and cratons,
which is what makes rifts follow belts, disappears. That is a consequence of permanent
weakness, not of the mixing rule, and it is not changed here. For scale: roughly a third of
Earth's continental crust has a Phanerozoic (< 540 Myr) tectonothermal age (order of
magnitude from global maps, e.g. Artemieva 2006; not re-derived), the simulation marks ~28%
in 500 Myr, so the rate is plausible for an Earth-like planet; but on Earth old belts regain
strength as they cool, and cratons are strong because of their thick, depleted keels.
Options for a follow-up: let the weakness heal with thermal age, or give cratons a strength
term of their own.

## #52: why 30% of the plume rifts ended far from their plume

Share of plume rifts whose closest boundary cell is more than 500 km from the plume, on
main (log counter): `main` 35% (116/336), `m68` 28% (59/213), `m73` 31% (67/213), as reported
in the issue. Diagnostics on `v1` / `m68v1` (share change only, 363 / 235 rifts, 28% / 29%
far). Of the far rifts:

| cause | `v1` | `m68v1` |
|---|---|---|
| the cut passed the plume (< 250 km), then pieces were moved to the other side | 97% | 96% |
| rigid crust assigned whole to one side moved the boundary away | 0% | 0% |
| the cut itself missed the plume | 3% | 4% |

The cause is the last step of `local_split_piece`: pieces of one side that are not connected
to that side's main piece are moved to the other side, and the main piece was "the largest".
When the cut left a small piece at the plume and a larger block of the same side elsewhere,
the piece at the plume changed sides and the boundary at the plume vanished (median 41 / 24
cells moved). A plate edge near the plume raises the risk (69% / 78% of the rifts with the
plume ≤ 150 km from the edge ended far, 18% / 13% of those more than 500 km inside), but
45% / 28% of the far rifts had the plume more than 500 km inside the plate. This is
bookkeeping, not physics: the break nucleates at the plume. Fix 1: anchor the two sides at
the plume.

The remaining far rifts (4% in `v2` and `m68v2`, 17 of 19 with the plume within 150 km of
the plate edge) were divisions whose cut never passed the plume; probable reason (from the
code, not measured): at the edge, one of the two lines of cells that tie the break to the
plume has no cells inside the plate. A plume at the edge of a plate does not break that
plate somewhere else. Fix 2: such a division is not a rift.

| | plume rifts (8 runs) | break within one cell of the plume | farther than 500 km |
|---|---|---|---|
| `v1` (before) | 363 | 71% | 28% |
| `v2` (anchor) | 296 | 95% | 4% |
| `m68v1` (before) | 235 | 70% | 29% |
| `m68v2` (anchor) | 188 | 94% | 4% |
| `m68v3` (anchor + rejection) | 190 | 99% | 0% |
| `m73` (main) | 213 | – | 31% |
| `m73v3` (**this branch**) | 221 | 99% | 0% |

- Rift count. With the anchor, more splits fail because the part on the plume's side is
  smaller than the minimum plate area (rifts that failed for good: 35 → 93 in `v1` → `v2`,
  22 → 55 in `m68v1` → `m68v2`, counted by the reviewer from the log lines; a plume does not
  try again). Before, such a split was replaced by a break elsewhere in the plate. In the log
  statistic (0–1000 Myr, per 100 Myr): `main` 4.20, `v1` 4.54, `v2` 3.70; `m68` 2.66,
  `m68v1` 2.94, `m68v2` 2.35, `m68v3` 2.38; `m73` 2.66, `m73v3` 2.76 (t = +0.3). The seed
  range is wide (1.4–3.6 on the last base), and no step is beyond |t| = 1.5: a drop of
  ~10% against main on two bases, none on the third.
- Not done: the near-field angle metric the issue asks for. The angle measurement lives in
  the analysis of PR #50 (branch `agent/issue-30`), not on main.

## Guard rails

`m73` vs `m73v3`, 500–1000 Myr, 69 metrics: [`compare-main-branch.md`](compare-main-branch.md).
One metric at |t| ≥ 2 besides the runtime: ocean heat transport at 20°N 1.19 → 1.29 PW
(t = +2.2), which was +0.7 and +0.0 on the earlier bases. The largest others (|t| 1.3–1.8,
not significant): sediment part of sea level 244 → 230 m, sea floor 20–40 Myr 22.6 → 23.5%,
median sea-floor age 34.8 → 32.9 Myr, highest point 4981 → 5384 m. Trenches 111 → 117 and
ridges 103 → 109 thousand km (t ≈ +1.2), continental area 33.5 → 32.7%, land 29.2 → 28.4%,
global temperature 14.2 → 13.8 °C, initiations 6.1 → 5.7 and sutures 0.95 → 0.88 per 100 Myr.

Earlier bases: `m68` vs `m68v2` no metric at |t| ≥ 2 (largest: sea floor ≥ 200 Myr 3.2 →
2.5%, t = −1.7; flooded continental crust 27.7 → 23.4%, t = −1.4); `m68` vs `m68v3` shelf
sediment 3.8 → 4.3 km (t = +2.2). `main` vs `v2` showed one coherent signal at |t| 2.0–2.9
(more boundaries and spreading, median sea-floor age 34.6 → 32.2 Myr, slow boundaries 9.6 →
6.5%), which is absent on `m68` and weak with the same sign on `m73` (median age t = −1.4).
Reading: no reproducible shift; a real effect on the sea-floor age, if any, is below ~5%.

## #53 and #51: recommendations

- **#53** (plume-head weakening has no memory and does not move with the plate): low
  priority, unchanged. With `rift_delay` 20 Myr and plates at ~5 cm/yr the heated
  lithosphere would trail up to ~1000 km behind the plume, about one head radius, so the
  field in the mantle frame misplaces the weak zone by up to its own size for fast plates.
  A decaying "plume heat" on the crust sample would cost what `orogenic` costs; it belongs
  with the healing of weak zones proposed above.
- **#51** (cut cost weighted by the plume's stress instead of the nucleation lines): the far
  rifts were not caused by the lines, and all rifts are now at the plume. #51 remains a
  cleaner formulation, not a realism problem: low priority.

## Overlap with other work

PR #68 (#43) and PR #73 (#42) are merged into this branch; the only adaptation was the new
field in three `Crust { … }` literals of their tests.

## Review findings and what was done

Verdict of [`review-results.md`](review-results.md): approve with changes; code sound, the
numbers reproduce. Done: the explanation of the cause no longer blames rigid continents (1);
the rift-count change is counted, with window (2); the remaining far breaks are rejected in
code and the run repeated (3); the #40 argument rests on conservation and the within-run
comparison, and names the collision marking as the driver (4); growth rate corrected (5);
saturation stated with numbers, issue proposed (6); "majority rule" claim reduced (7);
"within one cell" instead of the median (8); table corrected and its mark named (9); guard
rails reworded (10); runtime (11); tests: the edge test uses the 500 km criterion and counts
the rifts that succeed (21 of 24 set-ups, 15 without the anchor), and a collision test for
the share (12). Not done: a separate test "piece too small gives no rift" (the edge test
covers "a rift at the plume or none"); a debug line for divisions without result (13).
Commits 1e22f88 and three notes commits carry a wrong co-author line (14); history was not
rewritten.
