# Issue #43: results of recommendations 1, 3 and 4

_2026-11-21. Plan: [`plan.md`](plan.md) (review: [`review-plan.md`](review-plan.md)). Results
review: [`review-results-2.md`](review-results-2.md) (of the first version, v1; its findings are
fixed here). Investigation: [`investigation.md`](investigation.md). Earth-like planet, not Earth:
PB2002 numbers are plausibility checks._

## What changed

1. **One boundary normal** (`dynamics.rs::boundary_cells`): every boundary cell carries the smoothed
   normal (3-ring centroids; the raw one only on slivers where the two point apart). Trench test,
   ridge test, slab pull, ridge push, slab resistance and everything downstream use it. Forces and
   the subduction flux to the mantle are per true boundary length (`boundary_length_per_cell`,
   ÷ 1.1 zig-zag factor; with smoothed normals the forces no longer cancel partly along the
   zig-zag, which would have raised them by ~16%).
2. **Segments subduct only where they converge, and not along strike**
   (`update_segment_motion`): per segment, the mean normal convergence and the mean strike-slip
   over its lower plate's boundary cells (diverging cells included). A segment subducts if it
   converges faster than `min_trench_convergence` (0.1 cm/yr) and at least
   `min_trench_obliquity` = 20° from strike. Only then does its slab grow (at that mean rate) and
   are its slab-free converging cells trenches. Otherwise the slab decays (e-folding 10 Myr,
   break-off below 20 km: ~34 Myr for a 600 km slab, during which the remnant still pulls).
3. **Sutures need a stalled collision** (`events.rs::suture_colliding_plates`): a colliding pair
   sutures after `suture_time` (30 Myr, unchanged) during which its colliding cells converged on
   average slower than `events.suture_convergence` = 0.5 cm/yr. A collision that keeps
   converging stays a plate boundary and can accrete terranes. (v1 used "fewer than 10 cells
   faster than 0.5 cm/yr"; that absolute count kept long, creeping belts from ever suturing and
   thickened the crust; see "v1" below.)
4. **Driving of slab-free plates**: measured, no change (plan §3). After 1–3, boundaries between
   slab-free plates move at a median 1.9 cm/yr (Earth 1.5); the revisit criterion (> 2.5 cm/yr)
   is not met.

Declared side effects:
- A segment stays in contact (does not expire) as long as its lower plate has boundary cells there,
  also when it does not subduct; segments without slab persist along ridges and transforms
  (polarity memory).
- Slab growth uses the mean over all lower-plate cells of a segment, diverging ones included, so
  slabs grow more slowly on rotating segments; initiation segments that reach 300 km drop from 81%
  to ~67%.
- Kinematic sea-floor production uses the smoothed normals (the raw normals' spurious opening is
  gone); "created" is the unbiased number.

Self-orienting ridges are out of scope for the project (Sebastian, 2026-11-21). Recommendation 2
(slab-less slow ocean–ocean convergence) is #42's.

## Data
- `main` = 288f9cf (#63 merged), 8 seeds × 1000 Myr (runs of the #42 task with identical flags,
  read only); branch = a1c4298 (v2), same seeds and flags (`--headless --cell-dump --width 256
  --snapshot-interval 50`). Means over the snapshots of 500–1000 Myr per seed; Welch t over the
  8 seed means (|t| > 2.1 ≈ 5% for one metric; with ~50 metrics a few such values are expected by
  chance).
- Sensitivity, seeds 0–2 (on v1): old suture rule (`events.suture_convergence` = 10⁹: every
  collision is stalled) = recommendation 1 alone; no obliquity gate (`min_trench_obliquity` = 0).
- Scripts: `sim_classes.py` (boundary classes, as in the investigation), `compare.py` (logs and
  classes, Welch t).

## Plate boundaries (8 seeds)

| | main | branch | t | Earth PB2002 (at ~110 km) |
|---|---|---|---|---|
| subduction (10³ km) | 121 | **110** | −2.1 | 51 |
| … intra-oceanic (10³ km) | 78.7 | **62.6** | −2.5 | ~17–20 |
| ridge (10³ km) | 89.8 | 97.5 | +1.8 | 67 (73–85) |
| transform (10³ km) | 24.6 | **32.6** | +2.2 | 74 (55) |
| continental rift (10³ km) | 8.6 | 7.6 | −0.8 | 28 |
| convergent without slab (10³ km) | 15.8 | 18.0 | +0.6 | 40 |
| total boundary length (10³ km) | 260 | 266 | +0.3 | 260 |
| subduction / ridge / transform share | 47 / 35 / 9% | **42 / 37 / 12%** | −3.7 / +2.5 / +3.4 | 20 / 26 (30–33) / 28 (22)% |
| continental boundaries (rift + transform + convergent) | 10.9% | 11.3% | +0.2 | 30% |
| slower than 2 cm/yr | 8.1% | 10.5% | +1.5 | 39% |
| median relative speed (cm/yr) | 6.5 | 6.3 | −1.2 | 3.0 |
| **trench within 20° of strike** | 15.5% | **9.4%** | −7.7 | 4% |
| trench 70–90° from strike (head-on) | 28.6% | 32.6% | +3.5 | 48% |
| boundary between slab-free plates (trench share < 10%) | 7.4% | 13.9% | | 48% (slab ≥ 150 km) |
| … its median relative speed (cm/yr) | 2.1 | 1.9 | | 1.5 |

Subduction angle histogram (10° bins from strike): main 7 8 9 10 11 12 13 14 15, branch
4 6 8 10 12 13 15 16 17, Earth 1 3 3 6 11 12 16 23 25. Median speeds of the continental classes:
convergent 1.5 cm/yr (Earth 2.1), transform 2.1 (1.6), rift 1.9 (1.1).

## Guard rails (8 seeds)

| | main | branch | t |
|---|---|---|---|
| slab pull (10²¹ N); ridge push (10²⁰ N) | 1.29; 6.35 | **1.11**; 6.20 | −5.2; −0.5 |
| sea-floor production kinematic / created (km²/yr) | 5.67 / 6.41 | 5.94 / 6.49 | +1.2 / +0.3 |
| mean ocean age / sea floor mean age (Myr) | 60.4 / 53.7 | 59.6 / 53.3 | −0.3 / −0.2 |
| sea floor ≥ 200 Myr (%) | 3.4 | 3.0 | −1.0 |
| mean plate speed (cm/yr) | 3.87 | 4.04 | +1.2 |
| plates; largest plate (%) | 24.4; 33.7 | 24.0; 26.7 | −0.2; −1.9 |
| mean land (m) | 547 | 856 | +3.0 |
| highest cell (m) | 4531 | 5050 | +1.6 |
| continental crust flooded (%) | 33.5 | 24.7 | −3.9 |
| continental crust thicker than 50 km (%, 500–1000 Myr) | 1.9 (0.5–4.1) | 5.4 (1.0–13.2) | |
| collision stacking (Gt/yr) | 14.5 | 17.0 | +0.6 |
| sea level, dry offset (m) | −98 | −213 | |
| largest continental mass (% of continental crust) | 82 | 88 | |
| sutures / terrane accretions per 100 Myr | 3.3 / 5.9 | **0.6** / 9.4 | −6.3 / +1.7 |
| rifts / initiations / plates born per 100 Myr | 4.5 / 8.2 / 13.3 | 2.4 / 6.2 / 9.4 | −4.6 / −2.4 / −3.5 |
| dynamic topography minimum (m) | −2011 | −2119 | −0.8 |
| sediment part of the sea level (m) | 262 | 232 | −3.0 |
| segments that resumed subducting (per Myr, once per step) | — | 2.1 | |
| runtime (s, unpaired: `main` ran on a fuller machine) | 687 | 517 | |

Hypsometry: mean land 0.86 km (Earth ~0.84), crust > 50 km 5.4% (the calibration band of issue
1r, 3–6%), stacking in its band (13–23 Gt/yr). Continental crust floods less (25% vs 34%),
because land is higher and the dry sea-level offset lower. Fewer sutures go with fewer rifts and
fewer plate births: plates that do not merge carry their continents apart less often (plume rifts
need a continent on a large plate).

## Which change does what (seeds 0–2, v1 code)

| | main | rec 1 only (old suture rule) | rec 1 + 3 (v1) | v1 without obliquity gate |
|---|---|---|---|---|
| subduction (10³ km) | 127 | **112** | 111 | 129 |
| intra-oceanic subduction (10³ km) | 84 | **66** | 65 | 79 |
| transform (%) | 9.8 | 11.4 | 13.5 | 9.6 |
| trench within 20° of strike (%) | 15.7 | **9.5** | 9.0 | 12.8 |
| slab pull (10²¹ N) | 1.33 | **1.08** | 1.09 | 1.27 |
| slower than 2 cm/yr (%) | 7.7 | 7.9 | 20.9 | 15.5 |
| mean land (m) | 625 | 471 | 996 | 967 |

- Recommendation 1 does what it targets: fewer and less oblique trenches, less intra-oceanic
  subduction, less slab pull. The obliquity gate is needed for that; smoothed normals alone leave
  the trenches as long as on `main`. It does not make boundaries slower. (3 seeds; its effect on
  mean land is not significant.)
- Recommendation 3 as first written (v1, absolute fast-cell count) made the slow boundaries
  (21% slower than 2 cm/yr, continental boundaries 19%, slab-free plates bordering each other
  on 19% of the boundary) but only through belts that creep at ~0.3 cm/yr and never suture:
  crust > 50 km 8%, mean land 1.04 km, stacking 26 Gt/yr. With the corrected criterion (v2)
  those belts suture again and hypsometry stays in range, and the slow share rises only a little
  (8 → 10.5%, not significant).

## Assessment
- Physically better: trenches only where plates converge across them, not along strike
  (strike-slip trenches 15.5 → 9.4%, Earth 4%), 9% less subduction, 21% less intra-oceanic
  subduction, 14% less slab pull, 33% more transform length; sutures only when collisions have
  stopped. Hypsometry, sea-floor age and production stay in range.
- Not reached: Earth's slow boundaries. Continental boundaries stay at 11% (Earth 30%), the slow
  share at 10.5% (Earth 39%). In this model a slow continental boundary needs a collision belt
  that creeps without suturing, and with rigid plates such a belt only stacks crust (v1). Earth's
  slow belts also deform by extrusion and collapse over wide zones, which rigid plates cannot.
- Intra-oceanic subduction stays ~3× Earth's: #42's diffuse zones target it.
- Production is not lower (6.49 vs 6.41 km²/yr created): #32 is not solved by this.

## Caveats
- `main` runs from another task (same commit and flags), unpaired runtime.
- Sensitivity runs are 3 seeds on v1.
- Earth is one snapshot; the model numbers are 500–1000 Myr means.
