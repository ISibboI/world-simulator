# #42 follow-up results: diffuse convergent zones

_2026-11-21. Plan: [`plan.md`](plan.md) (with its revisions after the [plan review](review-plan.md)).
`main` (288f9cf, = #63 merged) against the branch, 8 seeds (0–7) × 1000 Myr, default parameters,
`--headless --cell-dump --width 256 --snapshot-interval 50`. Mean over seeds (seed range) of the
means over the snapshots of 500–1000 Myr; counters per 100 Myr over 500–1000 Myr; Welch t, n = 8
(|t| ≳ 2.1 ≈ p < 0.05 for one pre-named metric; with ~100 metrics, a few |t| > 2 are expected by
chance). Scripts: `~/tasks/issue-42/scripts/` on tktie (`compare.py`, `contacts2.py`,
`plates.py`, `tlist.py`). The `main` runs reproduce batch 2's final numbers exactly (production
5.68 km²/yr, slab pull 1.28–1.29 × 10²¹ N) and the PR #63 reply's contact table (87.1 / 12.9%)._

## Result in one sentence

The new boundary state exists and behaves as intended (slow, slab-free, mass-conserving
shortening that localises into a trench when it yields), but it covers **~1.5% of the converging
oceanic contacts**, not Earth's ~25%: almost all oceanic contacts in the model are driven above
the strength of the plate, and that is not something the state machine changes.

## The contacts (headline)

Rows = boundary cells seen from their plate, smoothed normals. "A" is the metric of the reply on
PR #63; "B" classifies by boundary state the rows where oceanic crust is the lower plate (or has no
segment), locked rows (|v| ≤ 0.1 cm/yr) included.

| | main | branch | t |
|---|---|---|---|
| A. ocean–ocean, converging > 0.1 cm/yr: part of a trench (%) | 87.1 (86.1…88.5) | **84.6** (83.4…85.8) | −5.8 |
| A. … no trench (%) | 12.9 | **15.4** | +5.8 |
| A. ocean–ocean + ocean–continent: no trench (%) | 13.6 | 15.2 | +2.4 |
| B. ocean–ocean lower side: trench (%) | 86.1 | 83.6 | −4.2 |
| B. … **diffuse zone** (%) | 0 | **1.50** (0.94…2.18) | |
| B. … other / no segment (%) | 13.9 | 14.9 | +1.7 |
| B. … locked (%) | 3.8 | 2.9 | −3.3 |
| B. ocean–ocean + ocean–continent: diffuse zone (%) | 0 | 1.72 (0.53…2.79) | |
| Diffuse-zone length (log, 10³ km; Earth's OCB 17.4) | — | 2.1 (0.7…3.2) | |
| Convergence of diffuse rows, quartiles (cm/yr; Earth's OCB 0.38 / 0.67 / 1.38) | — | 0.28 / 0.47 / 0.69 | |
| Shortening per diffuse segment, mean / max (km) | — | 26 / 89 | |
| Crust stacked by shortening (Gt/yr; subducted oceanic crust 125–129) | — | 0.53 | |
| Localisations (diffuse → trench) per 100 Myr | — | 71 | |
| … crust stacked by segments that localised within 2 Myr of creation (Gt/yr) | — | 0.05 | |

- The "other / no segment" rows (~14%) are on `main` too. They are lower-side rows of trench
  segments whose own raw cell normal does not converge (the zig-zag artefact of batch 2 and #43),
  untouched here.
- Diffuse rows converge at 0.3–0.7 cm/yr, like Earth's oceanic convergent boundaries (OCB), but
  the speed is bounded by `diffuse_convergence` = 1 cm/yr by construction and validates nothing
  above it.
- 9% of the stacked crust is transient (zones that localise within 2 Myr), the rest belongs to
  zones that shorten for longer.

## Plate dynamics

| | main | branch | t |
|---|---|---|---|
| Incipient trenches (slab < 150 km; 10³ km) | 11.1 | **8.1** | −3.0 |
| … convergence there (log, raw normals; cm/yr) | 1.86 | 2.19 | +5.8 |
| Trench length with slab < 150 km (%, dump) | 9.0 | 7.0 | −2.7 |
| Trench convergence slower than 2 cm/yr (% of trench) | 22.6 | 19.2 | −2.7 |
| Trench length (10³ km) | 122 (109…132) | 114 (105…135) | −1.7 |
| Slab pull (10²¹ N) | 1.28 | 1.23 | −1.4 |
| Sea-floor production, kinematic (km²/yr) | 5.68 (5.10…6.21) | 5.48 (4.99…6.04) | −1.2 |
| Mean sea-floor age (Myr) | 54.1 | 56.4 | +1.1 |
| Median relative speed (cm/yr) | 6.50 | 6.46 | −0.3 |
| Boundary slower than 2 cm/yr (%) | 8.2 | 7.2 | −1.6 |
| Subduction flux of crust younger than 20 Myr (%) | 23.8 | 24.1 | +0.2 |
| Surface on slab-attached plates (own mature trench ≥ 10% of boundary; %) | 48.3 | **41.9** | −3.3 |
| Boundary between two slab-attached plates (%) | 38.0 | 31.9 | −2.4 |
| Boundary between two slab-free plates (%; Earth 48) | 11.8 | 12.9 | +1.0 |
| Continental crust on slab-attached plates (%) | 15.3 | 7.2 | −2.2 |
| Initiation segments / of them grown to 300 km (per 100 Myr) | 28.8 / 22.7 | 24.9 / 20.2 | −1.4 / −1.1 |

The slow incipient trenches of `main` (slab-free contacts that converged a little and so counted
as trenches) are now diffuse zones; the ones that remain converge faster. Fewer plates count as
slab-attached. Everything else moves by less than its seed scatter.

## Guard rails

| | main | branch | t |
|---|---|---|---|
| Sea floor ≥ 200 Myr (%) | 3.5 | 4.0 | +1.3 |
| Sediment part of the sea level (m) / slope (m per 100 Myr) | 265 / 11.9 | 267 / 9.7 | +0.2 / −0.5 |
| Mean land (m) | 545 | 464 | −1.8 |
| Highest point (m) | 4567 | 4108 | **−2.1** |
| Crust thicker than 50 km (% of continental) | 2.0 | 1.0 | **−2.2** |
| Collision length (10³ km) | 13.7 | 10.3 | **−2.0** |
| Collision stacking (Gt/yr) | 15.0 | 10.7 | −1.8 |
| Flooded continental crust (%) | 33.7 | 35.6 | +0.7 |
| Continental area (%) | 35.1 | 36.0 | +2.1 |
| Permanently frozen (%) / global temperature (°C) | 11.4 / 14.5 | 9.8 / 15.2 | −2.8 / +1.9 |
| Dynamic topography minimum (m; #37) | −2014 | −1928 | +0.7 |
| Oceanic-origin crust (felsic < 0.3) thicker than 12 / 17 km (% of surface) | 0.22 / 0.00 | 0.27 / 0.02 | +1.8 / +4.2 |
| Mass ledger residual | below the log's 0.005 Gt/yr | same | |
| Unsettled velocity solves | 0 | 0 | |
| Plates ≥ 1% / plates born per 100 Myr | 14.0 / 13.3 | 13.4 / 11.7 | −0.6 / −1.6 |

**The relief guard rails move:** less collision length, less thick crust, lower peaks, and
(through the lower relief) less permanent ice and a slightly warmer planet. TODO: attribution
(ablation below).

## Sensitivities (seeds 0–2; compared with the same seeds of the branch)

| | main | branch | events resisted (`initiation_displacement` = 0) | `diffuse_convergence` = 0.5 | strength 10¹³ N/m |
|---|---|---|---|---|---|
| ocean–ocean diffuse (% of lower-side rows) | 0 | 1.5 | 1.9 | 0.8 | **3.8** |
| ocean–ocean no trench (A, %) | 13.1 | 15.7 | 16.2 | 14.4 | 18.7 |
| Trench length (10³ km) | 129 | 107 | 118 | 120 | 111 |
| Production (km²/yr) | 5.99 | 5.15 | 5.57 | 5.53 | 5.22 |
| Initiation segments grown to 300 km per 100 Myr | 21.9 | 22.2 | 18.7 | 22.5 | 21.7 |
| Highest point (m) / crust > 50 km (%) | 4892 / 3.1 | 3937 / 0.5 | 4270 / 0.9 | 4525 / 1.8 | 4412 / 1.4 |

- Seeds 0–2 react more strongly than the 8-seed mean (production −14% there against −3.5% over
  8 seeds); compare the columns with each other, not with the 8-seed table.
- **Resisting the events** (option 1b of the plan review) lowers successful initiations by ~15%
  (n.s. on 3 seeds) and adds little diffuse length. It would need the hazards reinterpreted as
  attempt rates, and failed passive-margin splits are not merged back. Not recommended.
- **A slower creep** (0.5 cm/yr, the Central Indian Basin's rate) halves the diffuse share: more
  contacts yield.
- **A stronger plate** (10¹³ N/m, the upper end for intact lithosphere; Mueller & Phillips 1991)
  gives 3.8%. Even then the model stays far from Earth's ~25% OCB share. The strength is a
  calibration lever, not a fix; it is shown only to answer how far this mechanism can go.

## Runtime

TODO (paired timing).

## Why so little

In the force balance almost every converging oceanic contact is pushed harder than
3 × 10¹² N/m: slab pull of 2 × 10¹³ N/m per metre of mature trench, transmitted by rigid plates,
overwhelms the strength of a contact as soon as the plate on its other side is slab-attached.
Only contacts between plates without that forcing creep. The diffuse state therefore takes over
what #63 locked (locked share −0.9 points) plus the slowest incipient trenches. Earth's diffuse
zones are mostly *inside* intact lithosphere or between slab-free plates (Central Indian Basin,
Gorringe, Macquarie); the model has few slab-free plate pairs (13% of the boundary against 48%)
and every contact is a plate boundary. So the remaining gap is the plate organisation of #32/#43,
which the #43 agent is now working on (smoothed-normal trench test, slab break-off under
strike-slip, driving of slab-free plates). Once fewer plates are slab-attached, more contacts
should fall below the strength and become diffuse zones with this change.
