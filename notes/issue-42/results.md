# #42 follow-up results: diffuse convergent zones

_2026-11-21. Plan: [`plan.md`](plan.md) (with its revisions after the [plan review](review-plan.md)),
results evaluation of the first measurement: [`review-results.md`](review-results.md).
Final measurement: `main` at e53b5f3 (#63 and #68 merged) against the branch rebased onto it,
8 seeds (0–7) × 1000 Myr, default parameters, `--headless --cell-dump --width 256
--snapshot-interval 50`, both sets run at the same time on the same machine. Mean over seeds (seed
range) of the means over the snapshots of 500–1000 Myr; counters per 100 Myr; Welch t, n = 8
(|t| ≳ 2.1 ≈ p < 0.05 for one pre-named metric; with ~100 metrics a few |t| > 2 occur by chance).
Scripts on tktie: `~/tasks/issue-42/scripts/` (`compare.py` log metrics, `contacts2.py` contact
classes, `plates.py` plate structure, `tlist.py`), `~/tasks/issue-42/tmp/eval/why.py` (from the
results evaluation)._

## Result in one sentence

The new boundary state works as intended (slab-free, mass-conserving shortening at 0.3–0.5 cm/yr
that localises into a trench when it yields), and it takes over most of the contacts that `main`
locks; but it is **~1% of the converging oceanic contacts**, the trench length does not change,
and on the current `main` (after #68) the "non-trench" share of converging ocean–ocean contacts is
already 21%, so #42's literal question was largely answered by #68.

## Contacts (headline)

Rows = boundary cells seen from their plate, with the smoothed normals. "A" is the metric of the
reply on PR #63. "B" classifies by boundary state the rows where oceanic crust is the lower plate
of a segment (or has no segment and converges), locked rows (|v| ≤ 0.1 cm/yr) included.

| | main | branch | t |
|---|---|---|---|
| A. ocean–ocean converging rows: part of a trench (%) | 78.7 (76.4…81.9) | 80.7 (77.5…83.8) | +2.1 |
| A. … no trench (%) (`main` before #68: 12.9) | **21.3** | 19.3 | −2.1 |
| B. ocean–ocean lower side: trench (%) | 76.7 | 80.0 | +3.3 |
| B. … **diffuse zone** (%) | 0 | **1.00** (0.54…1.70) | |
| B. … locked (%) | 5.2 | **3.0** | −9.1 |
| B. … other (locked or diverging rows of segments, no segment) (%) | 23.3 | 19.0 | −4.1 |
| B. … rows per snapshot: all / trench | 514 / 394 | 496 / 397 | −0.3 |
| B. ocean–ocean + ocean–continent: diffuse zone (%) | 0 | 0.77 (0.18…1.57) | |
| Diffuse-zone length (log, 10³ km; Earth's OCB 17.4) | — | 1.1 (0.1…3.2) | |
| Convergence at diffuse zones (log, smoothed normals; cm/yr; Earth's OCB median 0.67) | — | 0.33 | |
| Shortening per diffuse segment, mean / max (km) | — | 57 / 246 | |
| Crust stacked by shortening (Gt/yr; subducted oceanic crust 132) | — | 0.33 | |
| Localisations (diffuse → trench) per 100 Myr | — | 70 | |

- **The trench rows stay the same in number** (394 → 397 per snapshot); the trench *share* rises
  because the other lower-side rows fall by a fifth, mostly the locked ones: a contact that
  `main` locks below 0.1 cm/yr now creeps (up to `diffuse_convergence`) and either stays a
  diffuse zone or yields and localises. That is the intended physics; it does not create more
  trench length (112 → 111 × 10³ km, t −0.2).
- Diffuse rows converge at 0.3–0.5 cm/yr, like Earth's oceanic convergent boundaries (OCB, median
  0.67 cm/yr), but the speed is bounded by `diffuse_convergence` = 1 cm/yr by construction.
- Diffuse segments with a mean rate ≤ 0.1 cm/yr are classified "other", so the diffuse *state*
  covers somewhat more than the 1% of rows (results evaluation, finding 5: ~0.7 points more in
  the first measurement).
- Grid: the zones are one or a few cells wide (~110 km), while Earth's diffuse zones are
  hundreds to a thousand km wide. The shortening is stacked over the collision profile
  (4 rings), not distributed over the true width. Accepted as a grid effect.

### Where diffuse zones occur

Converging or locked oceanic lower-side rows by slab attachment of the two plates (own mature
trench on ≥ 10% of the plate's boundary), branch:

| plate pair | share of the rows | trench | diffuse | other | share of all diffuse rows |
|---|---|---|---|---|---|
| both slab-attached | 25% | 87% | 0.5% | 12% | 15% |
| own attached, other free | 63% | 94% | 0.3% | 6% | 23% |
| own free, other attached | 5% | 74% | 1.0% | 25% | 6% |
| **both slab-free** | **7%** | 69% | **6.0%** | 25% | **55%** |

Diffuse zones form where neither plate is pulled by its own slabs, which is where the force
across a contact can stay below the strength. In the model such pairs carry only 7% of these
rows (and 17–19% of all boundary length, Earth 48%).

## Plate dynamics and guard rails

| | main | branch | t |
|---|---|---|---|
| Trench length (10³ km) | 112 | 111 | −0.2 |
| Incipient trenches (slab < 150 km; 10³ km) | 13.1 | 11.5 | −0.9 |
| … convergence there (cm/yr) | 2.05 | **2.48** | +5.5 |
| Slab pull (10²¹ N) | 1.09 | 1.11 | +0.6 |
| Sea-floor production, kinematic (km²/yr) | 6.02 | 6.10 | +0.5 |
| Median relative speed (cm/yr) | 5.97 | 6.31 | +1.4 |
| Boundary slower than 2 cm/yr (%) | 13.6 | 11.7 | −0.7 |
| Ridge flank rows opening slower than 2 cm/yr (%) | 19.1 | 15.3 | −2.2 |
| Surface on slab-attached plates (%) | 43.4 | 44.7 | +0.6 |
| Boundary between two slab-free plates (%) | 19.1 | 17.1 | −0.6 |
| Subduction flux of crust younger than 20 Myr (%) | 22.6 | 22.4 | −0.1 |
| Initiation segments / of them grown to 300 km (per 100 Myr) | 25.1 / 17.3 | 20.9 / 14.5 | −1.2 / −1.1 |
| Mean sea-floor age (Myr) | 53.6 | 51.0 | −1.5 |
| **Sea floor ≥ 200 Myr (%)** (#14) | 3.22 | **2.35** | −2.3 |
| Ancient sea floor: within 300 km of its plate's continent / shallower than 2 km / sediment (%, %, km) | 71 / 46 / 7.0 | 82 / 61 / 9.0 | +3.0 / +4.0 / +5.2 |
| Mean land / highest point (m) | 809 / 4910 | 805 / 5048 | −0.0 / +0.4 |
| Crust thicker than 50 km (% of continental) | 4.3 | 4.5 | +0.2 |
| Collision length (10³ km) / collision stacking (Gt/yr) | 22.1 / 17.9 | 22.1 / 19.5 | −0.0 / +0.3 |
| Sediment part of the sea level (m) / slope (m per 100 Myr) | 226 / 8.4 | 242 / 13.8 | +1.6 / +1.2 |
| Burial metamorphism on continental crust (Gt/yr) | 0.11 | 0.15 | +2.3 |
| Permanently frozen (%) / global temperature (°C) | 11.3 / 14.1 | 11.1 / 14.1 | −0.2 / −0.0 |
| Dynamic topography minimum (m; #37) | −2031 | −2223 | −1.3 |
| Oceanic-origin crust (felsic < 0.3) thicker than 12 / 17 km (% of surface) | 0.19 / 0.00 | 0.26 / 0.02 | +2.2 / +3.9 |
| Unsettled velocity solves | 0 | 0 | |
| Runtime, 8 seeds run concurrently (s) | 730 | 682 | −1.4 |

- The incipient trenches that remain converge faster: slow slab-free contacts are now diffuse
  zones, and re-segmented pieces next to a trench inherit its thrust (results evaluation,
  finding 3) and so feel no strength.
- The old sea floor shrinks by a quarter and what remains is more clearly passive-margin
  basement (near continents, shallow, under thick sediment), the #14 picture.
- No relief guard rail moves. Thickened oceanic crust stays rare (0.26% of the surface above
  12 km against 0.19%; 0.02% above 17 km). The sediment part of the sea level is 16 m higher
  (t +1.6), within the seed scatter.
- Runtime: no cost (the two sets ran side by side; the unpaired difference is −7%).

## History: the first measurement and what it changed

The first measurement (branch before the rebase against `main` before #68; 8 seeds) found the same
small diffuse share (1.5% of ocean–ocean lower-side rows) but also a shift of the relief guard
rails (collision length −25%, highest point −460 m, crust > 50 km −50%, t ≈ −2). The independent
results evaluation ([`review-results.md`](review-results.md)) traced candidates; an ablation
(3 seeds) showed that the cause was a side change, the full initiation strength on the oceanic
side of segment-less ocean–continent contacts (it slowed oceans converging on continents and so
continental collisions). It was outside #42's scope and was reverted. The evaluation also found:
- a bug: a new segment inherited its neighbour's thrust even with the opposite polarity → fixed
  (and tested);
- stacking of oceanic crust onto continental margins at ocean–continent diffuse zones → the crust
  now goes onto the oceanic side (tested);
- missing counters → the log now reports the crust subducted at diffuse zones not yet seen by a
  velocity solve and the number of thrusts with displacement ≥ 150 km but a shorter slab.

Sensitivities of the first version (seeds 0–2, compared with the branch on the same seeds; the
seeds are high-production seeds of `main`, so all variants look lower than `main` there):

| | branch | events resisted (`initiation_displacement` = 0) | `diffuse_convergence` = 0.5 | strength 10¹³ N/m |
|---|---|---|---|---|
| ocean–ocean diffuse (% of lower-side rows) | 1.5 | 1.9 | 0.8 | 3.8 |
| ocean–ocean no trench (A, %) | 15.7 | 16.2 | 14.4 | 18.7 |
| Initiation segments grown to 300 km per 100 Myr | 22.2 | 18.7 | 22.5 | 21.7 |

- Resisting the events (option 1b of the plan review) costs ~15% of the successful initiations
  and adds little diffuse length; it would need the hazards reinterpreted as attempt rates and
  failed passive-margin splits merged back. Not recommended; the default keeps events as
  completed initiations.
- A slower creep (0.5 cm/yr) halves the diffuse share; a 3× stronger plate (the upper end for
  intact lithosphere, Mueller & Phillips 1991) gives 3.8%. Even then the model stays far from
  Earth's OCB share. The strength is a calibration lever, shown only to answer how far this
  mechanism can go.

## Why so little, and the comparison with Earth

Rigid plates transmit the slab pull of every attached plate to all its contacts, which then exceed
the strength of 3 × 10¹² N/m and localise. Only contacts between plates without that forcing
creep, and those are rare in the model (table above). On Earth, diffuse zones form where the force
transmitted across the zone stays below the strength, often inside a composite plate
(India–Australia–Capricorn, Central Indian Basin) or between slowly converging slab-free plates
(Gorringe, Nubia–Eurasia). PB2002's OCB is 25% of oceanic convergent length (OCB/(OCB+SUB), a
quarter of it inside Bird's orogens); in the model ~1% of the converging oceanic lower-side rows
are diffuse zones and another ~10–19% are rows of segments that are neither trench nor diffuse
(oblique, locked or diverging parts of segments since #68). The remaining gap is the plate
organisation of #32/#43 and the rigid-plate approximation (deformable margins and non-rigid plates
are separate follow-up issues), not the boundary state machine.

## Relation to #68 (merged)

#68 decides per segment whether it subducts (smoothed-normal convergence, not nearly along
strike); this change adds whether it has a thrust at all. The two combine as: a segment is a
trench where it is localised and subducting, a diffuse zone where it is not localised but
converges (`subducting`), and slabs and the displacement on the thrust grow only on localised,
subducting segments. The yield test uses #68's per-segment motion.
