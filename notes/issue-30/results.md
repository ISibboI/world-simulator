# Results for #30: plume rifts with an emergent junction angle

Setup: 8 seeds (0–7) × 1000 Myr, default parameters, `--headless`. Means are over 500–1000 Myr; counts are at 1000 Myr. t is the Welch t of the branch against `main` (n = 8 each; |t| ≳ 2.4 ≈ p < 0.05).

**Baseline.** The final comparison uses `main` at #39 (`cd940a0`), labelled main2 → v3. Merging #44 (relief) came later. On top of #44 the branch was only sanity-checked (see the end).

## Variants
| label | code |
|---|---|
| main | `main` at #23 (`ce11e5d`): three prescribed arms 120° apart, lune blocks, bisector spines |
| v1 | plume-head weakening, plain reference circle, 6 orientations, **no** nucleation lines |
| v2 | v1 + nucleation lines (on the #23 `main`) |
| nohead | v2 without the head weakening (ablation) |
| main2 | `main` at #39 |
| v3 | the PR: v2 rebased onto main2 |

## Rift geometry
- The angle at the plume is measured between boundary points ~660 km along the new boundary, on both sides of the boundary's point nearest the plume.
- It is measured only for boundaries that pass within 500 km of the plume.
- The distance column is the closest new-boundary cell to the plume.

| | rifts (8 runs) | boundary–plume distance q25 / median / q75 (km) | within 500 km | angle q25 / median / q75 | angle share < 150° |
|---|---|---|---|---|---|
| main (#23) | 311 | 5 / 114 / 2770 | 65% | 87 / 122 / 154° | 0.68 |
| v1 (weakening only) | 350 | 461 / **1576** / 2211 | **25%** | 107 / 132 / 151° | 0.75 |
| v2 | 315 | 4 / 111 / 1298 | 72% | 111 / 139 / 158° | 0.64 |
| nohead | 349 | 4 / 111 / 1869 | 67% | 111 / 133 / 160° | — |
| main2 | 279 | 5 / 111 / 1811 | 70% | 92 / 122 / 154° | 0.70 |
| **v3** | 293 | 4 / 111 / 1737 | 70% | 106 / **132** / 160° | 0.68 |

The v3 angle histogram (40–200° in 20° bins) is 4 / 10 / 16 / 24 / 29 / 30 / 39. It is broad, with junction-like corners and nearly straight rifts. On Earth plume break-ups also range from Afar-type junctions to almost straight rifts (Deccan–Seychelles, Central Atlantic).

**This metric discriminates poorly.** Even with the angle imposed at 120°, main measured quartiles of 87–154° at ±660 km, because the arms bend after leaving the plume. The direct evidence that the angle is no longer set is the unit test `plume_rift_arms_follow_sutures_around_the_plume`. With old sutures leaving the plume at 90°, 120° or 150°, the break follows them at that angle (within 20°). Under #23 the arms were fixed at 120°.

## Guard rails: main2 → v3
| metric | main2 | v3 | t |
|---|---|---|---|
| plates / > 1% | 23.2 / 12.3 | 23.4 / 13.4 | 0.1 / 0.7 |
| trench / ridge / collision (10³ km) | 120.6 / 91.1 / 9.6 | 121.9 / 93.6 / 11.5 | 0.2 / 0.4 / 0.8 |
| mean sea-floor age (Myr) | 52.1 | 52.5 | 0.2 |
| sea floor ≥ 200 Myr (%) | 3.34 | 3.35 | 0.0 |
| rifts / initiations per run | 35.3 / 73.8 | 37.3 / 72.3 | 0.3 / −0.3 |
| captures / polarity reversals | 0.9 / 7.8 | 1.9 / 5.5 | 1.7 / −1.4 |
| great-circle splits | 0 | 0 | |
| rift boundaries on sutures (continental cells), vs the rifted plates | ~53% vs 25% | ~52% vs 26% | |
| runtime, mean of 8 runs run concurrently in pairs | 486 s | 490 s (+1%) | |

On the #23 `main`, the two variants with weakening raised the ≥ 200 Myr sea floor (v1 +0.7, t = 2.2; v2 +1.0, t = 2.6) and the initiations (v2 +9.6). The no-weakening ablation did not. On main2 (with #39) the difference is gone (3.34 → 3.35). This is weak evidence for a small effect of the weakening on initiation cuts, which #39 masks or removes.

## Findings
1. **Weakening alone does not make plume rifts.** With weakening alone (v1), only 25% of rift boundaries pass within 500 km of the plume, and the median distance is 1576 km.
   - The minimum cut sees where the lithosphere is weak, but not where the driving force acts.
   - A head weakened to 0.5× is no stronger an attractor than a suture, and much weaker than young sea floor or a rifted margin (0.05–0.25).
2. **The break nucleates at the plume.** The plume head pushes its two sides apart, so the break starts at the plume (Burke & Dewey 1973).
   - This is modelled with two lines of cells running from the plume, perpendicular to the reference circle, into the two parts. Six circle orientations are tried.
   - This fixes *where* the break goes (through the plume), not the angle. Arms may leave anywhere between the lines. Angles below ~30–45° are unreachable, because the orientations are 30° apart and the lines are ~2 cells wide.
3. **The head weakening contributes little to the geometry.** The ablation without it gives nearly the same distances and angles. The angle becomes emergent because the prescribed lune and arms are gone, not because of the thinning. The weakening is kept because it is real physics: lithosphere above a head is weaker, also for initiations.

## Caveats
- About 30% of plume rifts still have their boundary more than 500 km from the plume, as in `main` (#23 and #39 alike). Possible causes: lines cut short at plate edges, continents assigned whole to the side of their majority, and pieces moved after the cut. Not investigated.
- The nucleation lines are a kinematic stand-in for the driving stress, not a stress calculation.
- The 25 Myr rejuvenation may overstate the reheating of oceanic swells (Von Herzen et al. 1989).
- The head field is fixed in the mantle frame. It does not move with the plate and keeps no history.

## Follow-ups (not opened as issues)
- A cut cost weighted by the plume's tension, decaying with distance from the plume, instead of the nucleation lines.
- An anisotropic hoop-tension term on the dome, favouring radial cracks.
- Dome uplift (#21).
- Failed third arms (aulacogens) as weak zones.
- A near-field (±2–3 cell) angle metric.
- Why 30% of rifts miss the plume.

## Independent reviews
- **Plan:** approve with changes. All changes were adopted: a thickness-based taper ending at R, gating on head arrival, precomputed strengths, a quantitative junction test, distance distributions, acceptance criteria set before the runs, and the orientation search.
- **Results:** ready with caveats, no correctness bugs. The caveats above follow it.
