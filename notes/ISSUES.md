# Known physical-accuracy issues

From the independent physics review of commit fb5ffb6 (2026-11-15) and the shelf check. Earth
reference values in brackets. Numbered as in the review; **work starts with issue 1** (user
decision, 2026-11-15). Details and evidence: `sessions/2026-11-15.md`.

## Open

1. **Ocean floor too old, plates only die.** Mean ocean age 117–175 Myr [~64 Myr, max
   ~180–200]; 22% of the surface at the −6000…−6400 m asymptote [~1–2%]; plates 12 → 5–10.
   Cause: random kinematic plate motion, no slab pull, old ocean never starts subducting, plates
   never split or merge. Ideas: force/torque-based plate velocities (slab pull ∝ trench length ×
   slab age, ridge push, basal drag; possibly mantle flow), subduction initiation at old (>150–200
   Myr) ocean or passive margins, rifting along old sutures or of large/supercontinent plates,
   suturing (merging) after continental collision, GDH1 depth curve (asymptote 5651 m).
   **In progress:** plan in `plans/plate-dynamics.md` (torque balance + mantle flow, rifting,
   subduction initiation, suturing).
2. **Sea level fixed at 0 → ocean water volume not conserved** (1.75–2.35e9 km³ [1.335e9],
   drifting ~1 km sea-level equivalent). Solve sea level for constant water volume each step.
3. **Continental crust not conserved with erosion** (−36% seed 0, −60% seed 3 [~steady]):
   - eroded bedrock → sediment 1:1 by volume despite 2750 vs 2300 kg/m³ (16% mass loss);
   - sediment beyond the shelf is deleted (~7% of flux) → deep-sea fans instead;
   - sediment subduction 3.3 km³/yr [1–1.6] with no accretion → accrete/recycle 50–70%;
   - collision crust above the 75 km cap deleted in persistent collisions;
   - `apply_surface_changes` drops erosion changes of cells sharing a crust sample (~3000
     cells);
   - single-cell hole filling copies crust (creates volume, small).
4. **Thermal uplift on continental/arc crust with inherited young oceanic age** → thick young
   crust 1–1.5 km too high, highest cells 8.4–11.4 km [~5–6 km at 1° scale]. Track a thermal age
   that resets for continental crust; 70 km crust gives 6.3 km [Tibet ~5 km] → denser thick
   crust or a lithospheric-root term.
5. **New oceanic crust created at convergent boundaries** (28–30% of crust < 3 Myr): gap filling
   ignores relative plate motion. Create ridge crust only where plates diverge.
6. **Erosion inconsistencies:**
   - implicit stream-power solve ignores isostatic response → big rivers erode up to ~6× too
     little; use `e = F(z − z_r) / (1 + F·f)`;
   - runoff = precipitation although 50% of land rain is recycled [runoff ~35–40%];
   - no endorheic basins [~20% of land drains internally].
7. **Shelves are sediment piles on oceanic crust.** 94% of cells between −200 and 0 m carry
   > 500 m of sediment (mean 11 km, 2761 of 3111 cells > 6 km), only 364 are continental crust;
   bare they would lie at ~−5900 m [shelves: thinned continental crust + 1–5 km sediment]. Cause:
   each river mouth fills up to 256 cells to −120 m at once; no rifted (stretched) continental
   margins. Fix: stretch continental crust where plates diverge (β ≈ 2–4), limit progradation.
8. **Precipitation too low at mid-latitudes** (land 20–60°: 0.30–0.35 m/yr [~0.6]; half of land
   < 0.25 m/yr [~20% arid]); ocean moisture reset to local evaporation (no fetch, ΣP ≠ ΣE); polar
   front at 55° [storm tracks 40–50°]; no seasonal ITCZ migration.
9. **Temperature details:** poles ~10 K too warm [−25 °C poleward of 70°]; land heat capacity
   8e6 J/m²/K below the atmospheric column alone [use 2–3e7]; subtropical land seasonality too
   large.

Minor: statistics count land as `> 0` in one place and `>= 0` elsewhere.

## Acceptable simplifications (at ~110 km cells, 1 Myr steps)

Local Airy isostasy (no flexure, trenches, forebulges), constant densities, no compaction,
age-only ocean depth (no dynamic topography), nearest-neighbour resampling of rigid plates,
diffusive crustal flow, n = 1 stream power with calibrated K, no hillslope/glacial erosion, no
lithology, fixed three-cell winds without monsoons, single upwind neighbour, zonal energy
balance without ocean currents, Budyko annual-mean ice criterion, 50 m mixed layer.
