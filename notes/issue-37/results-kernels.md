# #37 option 2: physical dynamic-topography kernels — results

Sebastian chose option 2 of the [feasibility notes](feasibility-2026-11-27.md): slab dynamic
topography from physical kernels of a layered viscous mantle, no calibration factor. Plan:
[plan-kernels.md](plan-kernels.md); review: [review-plan-kernels.md](review-plan-kernels.md).

## What changed (`src/tectonics/kernels.rs`, slab part of `src/tectonics/mantle.rs`)

- **Kernels computed at startup** (a few ms): Hager & O'Connell propagator for an
  incompressible, radially layered sphere, free slip at surface and CMB, Runge–Kutta in ln r
  with Gram–Schmidt re-orthonormalisation (stable to l = 100). Degrees 2–100 (degree 0: mass
  conservation; degree 1: frame-dependent with a fixed core).
- **Viscosity** relative to the upper mantle: lithosphere 0–100 km 100×, asthenosphere 100–300 km
  0.1×, 300–660 km 1×, lower mantle 30× (Hager & Richards 1989; Steinberger & Calderwood 2006;
  Mitrovica & Forte 2004 give 30–100× for the lower mantle).
- **Thermal expansivity** falls with depth, 3.1 → 2.0 (660 km) → 1.45 (1500 km) → 1.0 × 10⁻⁵ K⁻¹
  (CMB): approximate values from memory (±20%) after Katsura et al. 2009 and Steinberger &
  Calderwood 2006, consistent with the ~3× decrease over the mantle of Chopelas & Boehler 1992;
  not re-checked against the papers. A slab's heat deficit is
  conserved, so its excess mass scales with α/c_p; the kernels carry that factor, the tracked slab
  masses stay the conserved quantity.
- **Four depth stages** 100–660 / 660–1200 / 1200–2000 / 2000–2891 km (residence 13 / 45 / 67 /
  74 Myr). The new fourth stage replaces "below 2000 km the surface hardly feels them" by the
  kernel (stage 4 gives ~7% of the trench test's depression); what leaves it rests on the CMB,
  where the kernel is exactly zero.
- **Green's function per stage**, tabulated against chord length (2048 nodes), Gaussian load
  width 300 km (horizontal extent of the slab load). The slab grid moved from subdivision 3 to 4
  (= the evaluation grid) because its tent spreading over ~890 km cells widened the loads and cut
  the peak by 15–21%; each pair of cells is visited once.
- **Obsolete**: `SlabStage::{response, width}` (0.5/0.4/0.2 and 600/900/1500 km). New parameters
  `viscosity_layers`, `expansivity`, `cmb_depth`, `slab_load_width`, `kernel_degree`,
  `SlabStage::{top, bottom}`. `topography_response` stays as an on/off switch (1 = physical).
- Plume swells, slab bookkeeping (mass-conserving stage hand-over) and slab mass flux unchanged.

## Unit-test trench (5000 km, 5 cm/yr, old plate, steady state; `slab_field_sensitivity`)

Deflection on the slab line, dry:

| variant | depth |
|---|---|
| main (Gaussian stages, slab grid 3, its own unit test) | −1.37 km |
| kernels on main's slab grid 3 | −1.47 km (+8%) |
| kernels, default grids (slab = evaluation subdivision 4) | **−1.57 km** (+15%) |
| kernels, evaluated on the cells (converged reference, slab grid 5–6) | −1.71…−1.74 km |
| load width 200 / 450 km (on the cells) | −1.93 / −1.36 km |
| lower mantle 100× (on the cells) | −1.03 km |
| no weak asthenosphere (on the cells) | −2.03 km |
| stage shares (on the cells) 1 / 2 / 3 / 4 | −0.60 / −0.47 / −0.37 / −0.13 km |

The converged −1.7 km is at the low end of the expected −1.7…−2.4 km. The default grids lose
~10% of the peak (the evaluation grid's ~450 km spacing; a #64 grid artefact; subdivision 5
would recover 4% for 7× the cost).

## 8 seeds × 1000 Myr, branch 51898c7 vs main b3d43d6

`--duration 1000 --snapshot-interval 50 --cell-dump`, 11 snapshots per seed in 500–1000 Myr.
Script: [analyze_kernels.py](analyze_kernels.py) (plus
[analyze_check_2026-11-26.py](analyze_check_2026-11-26.py) for the slab flux). Field = dry
dynamic topography (`dyn`); under water the visible deflection is ×1.45.

| quantity (mean over snapshots and seeds) | main | kernels | change |
|---|---|---|---|
| slab mass flux (Gt/yr) | 28.8 | 28.9 | — (#32: ~1.6× Earth) |
| deepest point per snapshot (m, dry) | −1971 | −2317 | +18% |
| deepest point per seed (m, dry, mean of 8) | −2417 | −3028 | +25% |
| … per seed | −2152…−2774 | −2472…−3795 | |
| 1st percentile (m) | −1559 | −1742 | +12% |
| deepest under water, ×1.45 (m) | −2861 | −3364 | +18% |
| deepest on dry land (m) | −1687 | −2016 | +20% |
| highest (m, plume swells + return flow) | +1160 | +1343 | +16% |
| 99th percentile (m) | +786 | +1135 | +44% |
| RMS of the field (m) | 547 | 642 | +17% |
| mean \|h\| on continental crust (m) | 406 | 492 | +21% |
| area below −0.5 / −1 / −1.5 / −2 km (% of surface) | 18.1 / 6.2 / 1.5 / 0.2 | 20.1 / 8.3 / 2.8 / 0.8 | |
| … of it dry land (%) | 13.6 / 9.1 / 5.2 / 1.7 | 17.9 / 14.6 / 12.1 / 6.8 | |

Degree RMS of the field at 1000 Myr (m, mean over seeds):

| degrees | 1 | 2–3 | 4–8 | 9–16 | 17–30 |
|---|---|---|---|---|---|
| main | 239 | 378 | 313 | 114 | 27 |
| kernels | 15 | 520 | 359 | 127 | 53 |

Side effects (hypsometry, sea level):

| | main | kernels |
|---|---|---|
| sea level (dry offset, m) | −181 | −196 |
| land fraction (%) | 26.5 | 26.6 |
| flooded continental crust (%) | 30.7 | 29.5 |
| land elevation 10 / 50 / 90% (m) | 25 / 253 / 1804 | 26 / 285 / 1764 |
| ocean elevation 10 / 50 / 90% (m) | −5331 / −3534 / −552 | −5418 / −3509 / −596 |
| run time, GitHub runners only (seeds 4–7 vs main seeds 4–7) | 955 s | 1048 s (+10%, noisy) |

## Reading

- **As predicted, deeper**: +12…+25% depending on the measure, inside the feasibility estimate of
  10–50%. Four changes contribute: the kernel shapes (larger at long wavelengths, degree 2–3 RMS
  +38%, partly offset by the falling expansivity and the weak asthenosphere), the new stage 4, the
  finer slab grid (subdivision 3 → 4: narrower effective loads) and the 300 km load width. On the
  trench test the kernels on main's grid give +8% and the finer grid another +7%; the split in
  the full runs was not measured. The slab flux is unchanged; it is still ~1.6× Earth's (#32), and
  the field scales linearly with it, so an Earth-like flux would give roughly −1.4 km (snapshot
  minimum) and −1.9 km (seed minimum) dry.
- **Degree 1 left out** (239 → 15 m RMS, ~19% of main's variance): without self-gravitation the
  degree-1 response depends on how the core is held, i.e. on the frame. This is a modelling
  choice: with self-gravitation in the centre-of-mass frame a degree-1 response would exist.
- **Sharper flanks** (l 17–30 RMS doubled, as expected from both the narrower upper-mantle
  response and the finer slab grid). The deep lows themselves are broad (the evaluator's check of
  seeds 0, 2, 3, 6, 7: mean within 300 km within 3% of the minimum, 9–11 million km² deeper than
  half the minimum) and sit 190–550 km from long-lived trenches over slab graveyards; they are
  real features, not grid artefacts. More of them reach dry land (area below −1.5 km that is dry:
  5% → 12%): low-lying land (median 95 m) mostly within 700 km of a trench, like back-arc and
  foreland lowlands.
- **Seeds**: deepest point per seed +35 / +34 / +24 / +64% in seeds 3, 5, 6, 7, +15–17% in 0, 1, 4,
  −2% in seed 2. The runs are separate histories (different plate evolution), not paired, so these
  per-seed differences include run-to-run variation; only the 8-seed means are a comparison.
- **Hypsometry and sea level barely move**: sea level −15 m, land fraction unchanged, flooded
  continental crust −1 point, median land +32 m, all within the run-to-run spread (seed-to-seed
  standard deviation 49 / 67 m in sea level and 1.5 / 2.2 points in land fraction for main /
  kernels, i.e. ~20 m and ~0.7 points for an 8-seed mean). The field has zero mean,
  so it only redistributes.
- **Earth comparison** (orientation): observed residual topography is ±1 km under water
  (Hoggard et al. 2016), i.e. ~±0.7 km dry. The model's lows are ~3.3× that (snapshot minimum,
  dry and under water) and ~4.3× (seed minimum): about the ~2–3× gap of instantaneous-flow models
  of Earth (the "dynamic topography conundrum") times the model's ~1.6× slab flux.

Independent evaluation: an agent re-checked the numbers against the raw cell dumps, the
physics and the code (no bugs; deterministic; slab hand-over conserves mass); its corrections
(cause split, Earth ratio, RMS vs power, per-seed reading, degree 1, broad lows) are included above.

## Caveats

- **Load width** (300 km) is a physical but uncertain quantity; 200–450 km changes the trench
  peak by +24 / −13%. Not fitted.
- **Lower-mantle viscosity** 30× vs 100× changes the trench peak by −41%; literature spans both.
  30× is the end of the range with the strongest response; the 300 km load width, in turn, is
  the wide (weaker) end of the plausible ~150–300 km. Nothing was fitted.
- The load width is applied as the angle w/R at every depth; at depth it would be w/r, so deep
  loads are up to 1.65× too narrow in angle. Small effect, the deep kernels already suppress short
  wavelengths.
- #21's cover budget and #34's sediment result depend on the amplitude; not re-checked here.
- α profile approximate (±20%); no self-gravitation (~10% at low degrees), no 660 km
  phase-boundary compensation, no lateral viscosity (weak mantle wedges) — all named in the code.
- Grid (#64): the subdivision-4 evaluation grid cuts ~10% off narrow peaks.
- Overlap: PR #195 edits the plume-swell part of `dynamic_topography`; this change keeps to the
  slab part.
