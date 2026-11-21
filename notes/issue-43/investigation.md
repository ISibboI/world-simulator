# Issue #43: which plate boundaries dominate, model vs Earth — independent investigation

_2026-11-21. Requested on PR #63 ("What kind of boundaries dominate in the simulation? How does
the distribution look on earth?"). Investigation only; no simulation behaviour changed.
Classification and Earth evaluation re-derived from scratch (scripts in this folder); the
earlier notes of #32 and PR #63 were read only for context. An independent review of the draft
was incorporated (its corrections: overriding-side trench rule, Earth chord lengths, the
orientation judgement, the force-balance normals, the suture/terrane mechanism)._

## Answer in short

| boundary type (length, 10³ km, and share) | model (`main` before #63, 8 seeds, 500–1000 Myr) | Earth, PB2002 (Bird 2003) | Earth seen at the model's cell size (~110 km chords) |
|---|---|---|---|
| subduction (trench) | **122 (48%)** (seeds 111–138) | 51 (20%) | 51 (21%) |
| … of it intra-oceanic (overriding plate oceanic) | **77** (64–95) | ~17–20 (≈40% of subduction; literature) | |
| oceanic divergent (ridge) | 88 (35%) | 67 (26%) | 73–85 (30–33%) |
| oceanic transform | 16 (6%) | 48 (18%) | 30–31 (12%) |
| continental rift | 7 (3%) | 28 (11%) | 27–28 (11%) |
| continental transform | 6 (2%) | 26 (10%) | 24–25 (10%) |
| continental convergent without slab (collision, diffuse) | 10 (4%) | 23 (9%) | 23 (9%) |
| oceanic convergent without slab | 3 (1%) | 17 (7%) | 17 (7%) |
| **total** | 252 | 260 | 245–260 |
| median relative speed (cm/yr) / length slower than 2 cm/yr | 6.8 / **5%** | 3.0 / **39%** | 3.0 / 39–40% |

(Earth at 110 km: the range spans weighting by the original step length and by chord length,
see section 2.)

- **The model is dominated by trenches (48% of the length, 2.4× Earth's) and ridges (35%).**
  Ridges are only mildly over-represented: seen at the model's resolution, Earth shows 30–33%
  ridge, because transform offsets shorter than a cell merge into the ridge. The real anomalies are
  1. **intra-oceanic subduction**, ~4× Earth's;
  2. the near-absence of **slow continental boundaries** (rift, transform, collision: 9% vs 30%);
  3. too few **transforms** (9% vs 22% at equal resolution, both oceanic and continental);
  4. **almost no slow boundaries** (5% vs 39% below 2 cm/yr).
- **On Earth**, half the boundary length is non-subducting and slow: continental rifts,
  continental transforms, collision belts and diffuse oceanic convergence move at 1–2 cm/yr
  (median), ridge–transform systems at ~3 cm/yr. Gordon (1998) estimates that diffuse boundaries
  cover ~15% of Earth's surface.
- **Trenches are not selective.** 15% of the model's trench length moves within 20° of strike
  (Earth 4%; 5–8% even with 20° of noise added to Earth's azimuths): contacts that would be
  transforms keep a slab and pull.
- Resolution is not the cause: a coarser grid (subdivisions 5) gives the same mix.

## 1. Method

### Model data
- `main` at d399ee5 (before PR #63) plus the boundary dump of PR #63 (commit a878720,
  cherry-picked, output only), release build, `--headless --cell-dump --snapshot-interval 50`,
  seeds 0–7 × 1000 Myr, default resolution (subdivisions 6, 40 962 cells, ~110 km spacing).
  Statistics over the 11 snapshots of 500–1000 Myr of each seed (88 snapshots).
- Resolution check: subdivisions 5 (~220 km), seeds 0–1 × 1000 Myr, same window.
- `main` after PR #63: section 6.

### Classification (`sim_classes.py`)
Every dump line is a boundary cell of one plate facing one other plate; both sides are dumped,
so each line counts half a cell length (R × mean edge angle / 1.1, the model's own zig-zag
correction; the total, 252 thousand km, matches Earth's 260).

1. **Subduction** if the plate subducts there (`kind = t`), or it is the overriding side
   (`kind = o`, not a collision or ridge cell) of a segment that has a slab and subducting cells,
   and the plates do not diverge faster than 0.1 cm/yr (the model's own trench rule in
   `Simulation::boundary_cells`). Split by the crust of the overriding side: *intra-oceanic* if
   not of continental origin, else *under continent*. Thick felsic arcs count as continental, so
   "intra-oceanic" is a lower bound.
2. Otherwise by the **angle α between relative velocity and boundary strike**: α < 20° is a
   **transform** (Bird's PB2002 rule: velocity azimuth within 20° of the step azimuth); else
   **divergent** if the boundary-normal component opens, **convergent** if it closes.
3. **Continental** if the cell or the other plate's adjacent cells are of continental origin
   (Bird: everything not entirely oceanic counts as continental).
4. Speeds are full relative speeds |v_A − v_B|; "slow" = below 2 cm/yr (Gordon's diffuse
   boundaries move at ~0.2–1.5 cm/yr).

The boundary direction is the model's smoothed normal (as in the dump). As a check it was
recomputed as the principal axis of all boundary cells of the same plate pair within 300 km
(`--pca-km 300`) and, by the reviewer, 600 km: class lengths change by < 1.5 thousand km and the
angle distributions are unchanged, so the result does not depend on how the normal is estimated.

### Earth data (`pb2002_classes.py`)
PB2002 (Bird 2003), `PB2002_steps.dat` (5819 steps of ~45 km, each with length, azimuth,
relative velocity, class), from [peterbird.name/oldFTP/PB2002](http://peterbird.name/oldFTP/PB2002/);
class definitions from its [readme](http://peterbird.name/oldFTP/PB2002/2001GC000252_readme.txt)
(read in full). Evaluated (a) with Bird's own classes, (b) with the model classification
recomputed from the step azimuths, and (c) after **re-digitising** every boundary with chords of
~110, 220 and 440 km (velocity = length-weighted mean vector of the steps, crust/subduction
attribute by majority), to see what a boundary drawn on ~110 km cells could show. Re-digitised
chords are weighted either by the original step lengths or (`--chord-length`) by the chord
length, which is how the model measures a straight boundary. (a) and (b) agree exactly, which
validates the script against Bird's classification.

## 2. Earth

### PB2002 by Bird's classes
| class | length (10³ km) | share | median speed (cm/yr) | slower than 2 cm/yr |
|---|---|---|---|---|
| OSR oceanic spreading ridge | 67.3 | 26% | 3.4 | 24% |
| SUB subduction | 51.3 | 20% | 6.4 | 15% |
| OTF oceanic transform | 47.8 | 18% | 3.1 | 33% |
| CRB continental rift | 27.5 | 11% | 1.1 | 79% |
| CTF continental transform | 26.1 | 10% | 1.6 | 60% |
| CCB continental convergent | 23.0 | 9% | 2.1 | 49% |
| OCB oceanic convergent | 17.4 | 7% | 0.9 | 73% |
| all | 260.5 | | 3.0 | 39% |

52.5 thousand km (20%) of the steps lie in Bird's "orogens", zones of distributed deformation
that the plate model only approximates. Without them: OSR 30%, OTF 21%, SUB 19%, continental
classes 24%.

- Slow boundaries (< 2 cm/yr, 101 thousand km) are 60% continental or OCB, 30% slow ridges and
  transforms.
- Angle from strike (10° bins, 0 = pure strike-slip), at 45 km steps: non-subducting
  **26 10 6 6 5 5 7 10 25 %** (two peaks), subduction 1 3 4 6 9 11 17 24 24 % (head-on).
- Intra-oceanic subduction ~17 000 km, ≈40% of the convergent margins (a chapter abstract in
  *Subduction Zone Geodynamics*, Springer 2009,
  [doi:10.1007/978-3-540-88558-0_2](https://dx.doi.org/10.1007/978-3-540-88558-0_2), read only as
  snippet; ≈40% also in the publisher's description of Larter & Leat (eds.) 2003,
  [Intra-oceanic subduction systems](https://www.geolsoc.org.uk/bookshop/products/intra-oceanic-subduction-systems/)).
- Diffuse plate boundaries cover ~15% of Earth's surface, some > 1000 km wide, with relative
  speeds of ~0.2–1.5 cm/yr (Gordon 1998,
  [Annu. Rev. Earth Planet. Sci. 26, 615](https://doi.org/10.1146/annurev.earth.26.1.615);
  abstract only).

### Earth at coarser resolution (step-length weighted; chord-length weighted in brackets)
| | 45 km steps | 110 km chords | 220 km | 440 km |
|---|---|---|---|---|
| oceanic divergent | 67.3 (26%) | 84.9 (33%) [73.4, 30%] | 90.2 (35%) | 93.5 (36%) |
| oceanic transform | 47.8 (18%) | 31.1 (12%) [30.4, 12%] | 25.5 (10%) | 22.2 (9%) |
| continental transform | 26.1 (10%) | 24.8 (10%) [24.3, 10%] | 26.8 (10%) | 28.7 (11%) |
| SUB, CRB, CCB, OCB | | unchanged within ±2 | | |
| non-SUB angle from strike | 26 10 6 6 5 5 7 10 25 | 17 9 9 9 8 9 10 11 18 [19 10 9 8 8 9 9 10 18] | 15 10 11 8 9 7 12 13 15 | 12 12 10 9 9 9 13 12 14 |

A third of the oceanic transform length (58% at 220 km) is in offsets shorter than a model
cell: at 110 km they merge with the ridge into oblique "ridge" segments, and the angle
distribution flattens. So **a 110-km model can show at most ~12% oceanic transform, and ~30–33%
ridge.** Continental transforms are long, straight faults (San Andreas, Alpine, North Anatolian,
Dead Sea) and survive coarsening.

### Earth-like, not Earth
Earth today is one snapshot. Trench length has varied: tomography-based estimates reach
~75 000 km at ~170 Ma, up to ~2× today's ([van der Meer et al. 2014, PNAS](https://www.geologist.nl/wp-content/uploads/2016/11/2014_vanderMeer_PNAS.pdf);
abstract and snippets). The plausible range for an Earth-like planet is therefore wide, roughly
20–30% of the boundary as subduction, perhaps more during the dispersal of a supercontinent,
but 122 thousand km is above the reconstructed maximum, and intra-oceanic subduction dominating
is not seen. Slow boundaries are a robust feature of a planet whose largest plates carry
continents and no slabs.

## 3. Model

Means of 8 seeds, 500–1000 Myr, smoothed normal:

| class | length (10³ km) | share | median speed (cm/yr) | slower than 2 cm/yr | Earth median speed |
|---|---|---|---|---|---|
| divergent, ridge | 88.1 | 35% | 7.6 | 2% | 3.4 (OSR) |
| subduction, intra-oceanic | 76.8 | 30% | 6.9 | 3% | 6.4 (all SUB) |
| subduction, under continent | 45.0 | 18% | 6.6 | 3% | |
| transform, oceanic | 15.7 | 6% | 6.9 | 2% | 3.1 (OTF) |
| divergent, continental rift | 6.6 | 3% | 2.9 | 36% | 1.1 (CRB) |
| convergent, continent–continent | 6.3 | 3% | 2.1 | 46% | 2.1 (CCB) |
| transform, continental | 6.2 | 2% | 3.9 | 27% | 1.6 (CTF) |
| convergent, ocean–continent, no slab | 4.2 | 2% | 5.3 | 13% | |
| convergent, ocean–ocean, no slab | 3.1 | 1% | 6.6 | 8% | 0.9 (OCB) |
| all | 252 | | 6.8 | 5% | 3.0 |

- Seed ranges (10³ km): subduction 111–138, of it intra-oceanic 64–95; ridge 78–99; transforms
  17–28; continental rift 4–10; convergent without slab 9.5–21.
- Relative speed bands 0–1 / 1–2 / 2–5 / 5–8 / 8–12 / > 12 cm/yr: 2 / 3 / 19 / 43 / 27 / 6 %.
- Angle from strike, non-subducting: **9 8 8 9 10 12 13 15 16 %** (Earth at 110 km: 17–19 … 18);
  subduction: **7 8 9 10 11 13 14 14 15 %** (Earth: 1 3 3 6 11 12 16 23 25). The model has half
  of Earth's strike-slip share at equal resolution and no clear transform peak; its trenches are
  far less head-on than Earth's.
- Resolution check, subdivisions 5 (~220 km, 2 seeds): ridge 34%, subduction 45%, transforms 9%,
  continental rift 4%, same angle distributions. The mix is a property of the model's rules, not
  of the grid (this shows nothing about a finer grid, where Earth's transform share would rise
  from 12% towards 18%).

### Speeds
Trenches converge at Earth-like rates (6.6–6.9 vs 6.4 cm/yr), but ridges and oceanic transforms
move ~2.2× faster than Earth's, continental rifts and transforms ~2.5×. Ridges plus trenches
(the fast types) make up 83% of the length (Earth 46%), which is why the median relative speed
is 6.8 instead of 3.0 cm/yr and sea-floor production is ~1.8× Earth's (#32).

## 4. Physically implausible vs merely different

| finding | judgement |
|---|---|
| total boundary length 252 vs 260 thousand km; plate count; trench convergence rates | Earth-like |
| ridge share 35% | slightly above Earth at this resolution (30–33%); fine |
| ridges spreading at 7.6 cm/yr median (Earth 3.4) | different; follows from the plate driving below; on its own still within the range of fast-spreading Earth |
| subduction 48% of the boundary, 122 thousand km | **implausible**: above any reconstructed Earth value (≤ ~75 thousand km); it doubles slab pull per plate and the fast boundary share |
| intra-oceanic subduction 77 thousand km (Earth ~17–20) | **implausible** in amount: oceanic convergence on Earth is often slow and slab-less (OCB, 17 thousand km at 0.9 cm/yr); in the model every converging oceanic contact gets a slab |
| trenches with strike-slip motion (15% of trench length within 20° of strike) keep their slab and slab pull | **implausible**: strongly oblique convergence partitions or stops subduction; such contacts are transforms on Earth |
| transforms 9% (Earth at 110 km: 22%) | **weaker than Earth at equal resolution**; partly the trenches above, partly few slab-free continental pairs (continental transforms 2% vs 10%) |
| continental boundaries 9% (Earth 30%); hardly any slow boundary (5% vs 39%) | **implausible** for an Earth-like planet with large continental plates; partly a consequence of rigid plates (no diffuse zones) |

## 5. Mechanisms in the code

1. **Every converging contact becomes a trench, regardless of obliquity, crust age contrast or
   rate** (`Simulation::boundary_cells`, `src/tectonics/dynamics.rs`; `Polarities::contact`,
   `src/tectonics/polarity.rs`). A polarity segment is created at the first overlap (polarity by
   buoyancy), a cell is a trench as soon as its raw per-cell convergence exceeds
   `min_trench_convergence` = 0.1 cm/yr, and once a slab exists it stays a trench until the plates
   diverge (hysteresis). There is no slow, slab-less convergent class for ocean–ocean contacts:
   the model has 3 thousand km of it, Earth 17.
2. **Trench test, slab growth and slab pull use the raw per-cell normal** (the summed directions
   to the neighbours, which scatters by ~20° on the zig-zag mesh; only shear coupling and
   collision resistance use the smoothed normal). Slab growth (`update_slab_lengths`) averages
   only the trench cells that converge, and the slab decays only when none of them does. On a
   near-strike-slip boundary some cells always see spurious convergence, so the segment keeps its
   slab: 15% of the trench length is strike-slip in motion, and 7.5% of the trench cells actually
   diverge along the smoothed normal. (PR #63 lists the normal mismatch as a known gap.)
3. **Boundary geometry is inherited, not adapted to the motion.** New boundaries are drawn by
   events (plume rifts along weak zones or great circles, initiation along coasts and fracture
   zones, `events.rs`) and afterwards only grow by gap filling (`boundaries.rs`, layer-by-layer
   fill: a new cell goes to whichever adjacent plate reaches it first, so the ridge keeps the shape
   of the old boundary) or shrink by consumption. Plate velocities come from the torque balance
   and change whenever trenches appear or vanish, so the relative motion rotates against a fixed
   boundary shape. `opens_away` refuses to spread at very oblique motion (< ~15° from strike),
   which is right, but nothing builds transform offsets. At the model's resolution this matters
   less than it seems: Earth's ridge–transform staircase is mostly finer than a cell (the
   transform-forming instabilities of Gerya 2010 / Püthe & Gerya 2014 make offsets of tens of km).
4. **Continental boundaries are removed quickly.** A colliding pair with ≥ 10 colliding cells and
   at least as many colliding as trench cells sutures after `events.suture_time` = 30 Myr
   (`suture_colliding_plates`, `events.rs`); pairs that cannot suture accrete the smaller
   colliding piece as a terrane after `terrane_accretion_time` = 12 Myr. Earth's
   Alpine–Himalayan belt (~44% of Earth's CCB length) has been active for 50+ Myr. Continental
   rifts turn into ridges as soon as the crust reaches the breakup thickness. Plates are rigid,
   so diffuse intraplate zones (Gordon 1998) cannot exist.
5. **Slab-free plates are rare and not slow.** On average 25.6 of 26 plates have trench cells
   (mechanisms 1–2). Continental plates follow the mantle flow closely (`continental_drag_factor`
   = 4, flow towards slab graveyards, `mantle.rs`), which can drive or brake them. On Earth the
   slow boundaries are between large slab-free plates (Africa, Eurasia, Americas, Antarctica);
   here such pairs are rare (PR #63 measured 9% vs 48% of the boundary).

Mechanisms 1, 2 and 5 produce the subduction excess and the high speeds; 2 and 5 most of the
missing transforms (oceanic and continental); 4 and 5 the missing slow continental boundaries;
3 a smaller part of the oceanic transform deficit.

## 6. After PR #63, and the normal-estimate check

**After PR #63** (`main` 288f9cf: buoyant young slabs, resistance to starting subduction).
Same settings, seeds 0–1 (runs of the parallel #42 task, read only), against the same seeds before
#63, 500–1000 Myr:

| class (10³ km) | before #63 | after #63 |
|---|---|---|
| subduction (intra-oceanic) | 126 (80) | 126 (83) |
| ridge | 92.6 | 92.3 |
| transforms (oceanic + continental) | 24.8 | 25.0 |
| continental rift | 7.9 | 7.4 |
| convergent without slab | 18.0 | 16.2 |
| median relative speed (cm/yr), slower than 2 cm/yr | 6.5, 7% | 6.6, 7% |
| trench length within 20° of strike | 15% | 15% |

PR #63 does not change the boundary mix materially (2 seeds only; differences are within seed
scatter). Everything in this note applies to the current `main`.

**Normal estimate.** With the principal-axis normal over 300 km instead of the model's smoothed
normal (8 seeds, first version of the classification): ridge 87.4 vs 88.1, oceanic transform 15.3
vs 14.2, continental transform 5.7 vs 5.4 thousand km, angle distributions within ±1 point. The
reviewer found the same at 600 km. The near-flat angle distribution is not an artefact of noisy
normals.

## 7. Recommendations (not implemented)

Ordered by expected effect per effort. Run-time cost is negligible unless stated: the smoothed
normals and the segment bookkeeping exist already.

1. **Trenches need convergence across the smoothed boundary, and lose their slab under
   strike-slip motion.** Use the smoothed normal for the trench test, slab growth and slab pull;
   count a segment as converging only if the length-weighted convergence of its cells is
   positive and the motion is more than ~20–30° from strike; otherwise decay the slab (break-off
   within ~10 Myr). Expected: up to ~18 thousand km of strike-slip "trenches" become transforms
   (upper estimate, ignoring feedback), slab pull drops on the plates concerned, relative speeds
   fall. Small change in `boundary_cells` / `update_slab_lengths`.
2. **A slab-less convergent class for slow or young ocean–ocean contacts.** Converging oceanic
   contacts below ~1–2 cm/yr, or without enough age contrast, stay a diffuse convergent boundary
   (resisted like a weak collision) instead of opening a polarity segment with a slab; only
   sustained convergence (~100–150 km of shortening, the induced-initiation threshold of Gurnis et
   al. 2004) creates a slab. PR #63 added a resistance to starting subduction; this keeps such
   contacts slab-free. Expected: intra-oceanic subduction down by tens of thousand km, an OCB-like
   class appears, more slab-free plates and so more slow boundaries. This is #42's territory; the
   numbers here say it is the largest single excess.
3. **Keep collisions as boundaries longer.** Suture on stalled convergence (e.g. < 0.5 cm/yr over
   ~20 Myr) instead of after a fixed 30 Myr. Expected: continental convergent length up a few
   times (Earth 23 thousand km). Affects collision stacking and topography calibration, so it needs
   the usual guard rails.
4. **Plate driving of slab-free plates** (continental drag, return flow), the candidate PR #63 left
   open. It sets the speed of the slow boundaries; test it after 1–2, which remove most of the
   extra slabs that make every plate fast.

**Out of scope at this resolution: ridges that orient themselves** (decided on #43). In the gap fill, assign a new cell to the
plate on whose side of the local spreading axis it lies (perpendicular to the relative
velocity, per segment of a few cells) instead of to whichever plate reaches it first, so that
oblique ridges turn into orthogonal segments with offsets of ≥ 2 cells. Ceiling: about +6
points of oceanic transform (6% → 12%) at this resolution; it does nothing for the continental
deficit. Moderate effort in `boundaries.rs`, one projection per filled cell.
It was first ranked last because the other items address larger implausibilities, item 1 alone converts
more length (up to ~18 thousand km of oblique trench) than this can add, and it is the most
complex change (fill rule plus stable segmentation over time). It gains weight on a finer grid,
which resolves Earth's short offsets.
At ~110 km cells most real ridge offsets are sub-grid; only the few offsets of several hundred km (Romanche, Eltanin, Charlie-Gibbs) span cells, worth a few points of transform at most. Revisit if the grid gets finer, or once items 1–4 are done.

Not recommended: a coarser grid changes nothing, and a finer one (4× cost per level) could show
more short transforms but not fix 1–4; tuning speeds or ridge push keeps the partitioning (#32).

## References

- Bird, P. (2003), An updated digital model of plate boundaries, G³ 4(3), 1027,
  [doi:10.1029/2001GC000252](https://doi.org/10.1029/2001GC000252); data and readme at
  [peterbird.name/oldFTP/PB2002](http://peterbird.name/oldFTP/PB2002/) (data evaluated directly).
- Gordon, R. G. (1998), The plate tectonic approximation: plate nonrigidity, diffuse plate
  boundaries, and global plate reconstructions, Annu. Rev. Earth Planet. Sci. 26, 615–642
  (abstract only).
- Gerya, T. (2010), Dynamical instability produces transform faults at mid-ocean ridges, Science
  329, 1047–1050; Püthe, C., Gerya, T. (2014), Dependence of mid-ocean ridge morphology on
  spreading rate in numerical 3-D models, Gondwana Res. 25, 270–283 (both from abstracts/snippets).
- Larter, R. D., Leat, P. T. (eds.) (2003), Intra-oceanic subduction systems, Geol. Soc. Spec.
  Publ. 219 (publisher description only).
- Gurnis, M., Hall, C., Lavier, L. (2004), Evolving force balance during incipient subduction, G³ 5.
- van der Meer, D. G. et al. (2014), Plate tectonic controls on atmospheric CO₂ levels since the
  Triassic, PNAS 111, 4380–4385 (abstract and snippets).
- Mallard, C. et al. (2016), Subduction controls the distribution and fragmentation of Earth's
  tectonic plates, Nature 535, 140–143 (summary only): in self-consistent convection models, slab
  spacing sets the layout of large plates.
