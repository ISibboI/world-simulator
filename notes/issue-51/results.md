# Results: plume rifts from physics only (#51) and extension fabric / failed arms (#54)

_2026-11-27. Plan: [`plan.md`](plan.md) (revised after [`review-plan.md`](review-plan.md)).
Independent evaluation: [`review-results.md`](review-results.md) (its corrections are adopted
below; the two-arm plume sections were added after it). Goal: an Earth-like planet, not Earth;
effects of the ~110 km grid are accepted (#64)._

## Short answer

* **With literature values, the physics alone does not rift plates at plumes.** Rift-type
  splits (#182 breakups and sheet splits) within ~500 km of a plume are *rarer* than within
  500 km of the plumes' antipodes (the chance null), in every window and arm. #182 searches
  sections through every plume, straight and with two arms 90–150° apart; the best one was
  overloaded (S ≥ 1) in 0.9% of 11 200 analyses of plates with a plume, and 2 plates in
  8 × 1000 Myr broke along one. The plume's swell push and magma act only on the head's
  ~2000 km chord of a section that must cross the whole plate; full magma weakening lasts only
  ~10 Myr (the head arrives over 10 Myr, the head stage ends at 20 Myr). The plan review
  estimated that this lowers a whole section's strength by only ~1.3–2× (an estimate, not
  measured here).
* **Consequence:** fewer and larger plates. 500–1000 Myr: plates 27 → 19, largest plate
  19% → 37% (up to 68% on 2 of 8 seeds; main ≤ 36%), births 10.9 → 6.5 per 100 Myr (|t| ≥ 3).
  Neither arm disperses a supercontinent within 500 Myr; main splits a large piece off it on 2
  of 4 seeds, the branch on 1.
* **#54:** stretching leaves an extension fabric that weakens the lithosphere and heals over
  ~200 Myr. New boundaries run on fabric about twice as often as chance (25% of their
  continental cells vs 13% of the split plates'), but much of that is lithosphere stretched in
  the same episode that then breaks (the fabric's age was not tracked), and fabric overlaps
  orogenic belts. Whether fabric deep inside continents behaves like failed rifts/aulacogens
  (reactivated long after) is therefore **not shown**. Weighted by shortening, 20% of the sheet's
  shortening falls on fabric (inversion, partly zones alternating between stretching and
  shortening). The fabric forms broad patches over stretched regions, not narrow arms (#64).

**Decision for Sebastian** (the threshold fixed in the plan, ≥ 1 plume rift per 100 Myr above
chance, was missed): see [Decision](#decision).

## What changed

1. **Swell load** (`sheet.rs::cell_forces`, `MantleFlow::plume_swell_gradient`): the potential
   energy of the plume swells, `ρ_m g h L` with the dry swell height `h` (≤ 650 m) and the
   lithosphere's thickness `L` (100 km × its thermal term), pushes the lithosphere away from the
   swell's crest, in the sheet and in #182's loads. Peak ~2 × 10¹² N/m, like ridge push.
2. **Magma-assisted opening** (`weak_zones::magma_weakening`): where a plume supplies melt now
   (the head's footprint during its first 20 Myr, the tail's later, scaled by the flux), the
   lithosphere's strength in **opening** is × 0.1 (Buck 2004, 2006; Bialas et al. 2010), floored
   at the ridge-axis strength 0.05. #182's limit analysis has separate strengths per section edge
   for opening, sliding and closing (per-edge IRLS weights; convex; tested). The sheet's viscosity
   gets the same factor (isotropic; capped by its viscosity floor).
3. **Sections through plumes** (`breakup.rs::plume_sections`): for each active plume above a
   plate, sections from the plume along two arms in 12 directions, 90°, 120°, 150° apart or
   straight (42 per plume); the part between the arms is connected to the plume; the best is
   analysed fully and refined along the weakest lithosphere like the level sections.
4. **Plume-rift rule removed:** `rift_above_plumes`, `SplitAnchor::Rift`, the nucleation lines
   (`SPINE_LENGTH`, `RIFT_ORIENTATIONS`), `rift_delay`, `min_oceanic_rift_area`,
   `Plume::has_rifted` and the rift-only statistics and log lines. `--set events.rift_delay=…`
   and `events.min_oceanic_rift_area` no longer exist.
5. **Extension fabric** (`Crust::rifted`, #54): grows with the extensional strain of continental
   lithosphere (sheet stretching at ≥ 10⁻¹⁵ s⁻¹, so creep noise does not add up; boundary rifts
   that stretch margins, `stretch_rift`), full at a strain of 1; mixed by mass like the orogenic
   weakening (continental lithosphere only), heals with it (200 Myr). Weakens the strength by up
   to × 0.5 (`RIFT_FABRIC_WEAKENING`: the mechanical part of a rift's weakness; the thermal part
   and the thinning are separate terms) in `lithosphere_strength` (#182, weak-zone cuts, view) and
   in the sheet's strength. Strength view: new "Rift fabric" term and "Rifted margin, failed
   rift" weakness; frame history format version 12 (one more 8-bit channel); `--cell-dump`
   column `rifted`.
6. **Diagnostics** (also on the baseline, state unchanged): per new boundary its cause, distance
   to the nearest plume and to the nearest plume antipode (chance null), and the belts and
   fabric on it; per snapshot the fabric area (near margins / deep inside continents) and the
   share of the sheet's shortening on fabric; #182's plume-section overloads. `births()` now
   counts #182 and sheet splits (it missed them on main).
7. Ablation switches (physical defaults, for attribution): `--set swell_load=0`,
   `magma_weakening=1`, `rift_fabric=0`.

## Runs

* `base`: main (1418167) + the diagnostics only (the tree of this branch's first commit c77fad9;
  same state hash as main, checked on seed 0 at 0/50/100 Myr).
* `straight`: the branch with straight plume sections only (42f8aab); `arms`: the final branch
  with two-arm sections (bc8322e); `nofabric`: `straight` with `--set rift_fabric=0` (4 seeds).
* 8 seeds × 1000 Myr, 6 subdivisions, main's default flags (dispersed start, no `--epoch`),
  `--snapshot-interval 10`. Supercontinent arms: `--initial supercontinent`, 4 seeds × 500 Myr
  (`arms` and `straight` were identical there). Step 0: the branch before the removal with the
  rule switched off, 4 seeds × 400 Myr.
* Means over snapshots in the window (rates: differences of the cumulative counts), then over
  seeds (range in brackets), Welch t against `base`; with ~40 metrics, treat |t| < 3 as
  suggestive. Script: [`scripts/compare.py`](scripts/compare.py); map:
  [`scripts/fabric_map.py`](scripts/fabric_map.py).

### Splits and plates, 500–1000 Myr (per 100 Myr)

| | base (main) | straight | arms (final) | nofabric |
|---|---|---|---|---|
| plume-rule rifts | 2.25 [1.6–2.8] | 0 | 0 | 0 |
| #182 breakups by opening | 1.88 | 1.80 (t −0.2) | 1.67 (t −0.4) | 1.20 |
| #182 by sliding/closing | 0.65 | 0.33 | 0.23 (t −2.4) | 0.00 |
| sheet splits | 0.68 | 0.45 | 0.28 | 0.20 |
| subduction initiations | 4.80 | 4.33 | 4.08 (t −1.0) | 3.75 |
| all splits | 10.25 | 6.90 (t −4.6) | 6.25 (t −4.1) | 5.15 |
| births / deaths | 10.9 / 11.1 | 7.2 / 7.6 | 6.5 / 7.2 (t −4.1 / −3.4) | 5.3 / 5.2 |
| plates | 27.2 [20.7–35.8] | 20.4 | 19.2 [9.1–23.9] (t −3.2) | 15.0 |
| largest plate (% of surface) | 19.3 | 35.9 | 36.8 [19.9–57.4] (t +4.0) | 40.8 |
| largest plate, maximum over the run per seed | 24–36 | 31–68 | 31–68 | |

0–500 Myr (base → arms): plume-rule rifts 2.52 → 0; all splits 11.3 → 9.1 (t −1.9); plates
25.4 → 21.7 (t −2.1); largest plate 18.8 → 24.0% (t +2.6). The effect grows with time.

**Splits near plumes, by cause** (sums over 8 seeds; within ~500 / ~1000 km of a plume vs of a
plume's antipode):

| 500–1000 Myr | base | straight | arms |
|---|---|---|---|
| plume rule | 90 / 90 vs 13 / 17 | — | — |
| #182 opening | 6 / 13 vs 16 / 20 | 2 / 6 vs 5 / 15 | 1 / 4 vs 6 / 18 |
| #182 other | 1 / 2 vs 3 / 6 | 0 / 1 vs 2 / 3 | 0 / 1 vs 2 / 2 |
| sheet | 5 / 7 vs 7 / 11 | 6 / 9 vs 8 / 9 | 6 / 7 vs 6 / 6 |
| initiations | 11 / 20 vs 14 / 19 | 23 / 32 vs 9 / 15 | 20 / 28 vs 9 / 14 |

0–500 Myr, arms: #182 opening 6 / 11 vs 8 / 14, sheet 0 / 0 vs 0 / 0, initiations 29 / 44 vs
17 / 35 (base: 9 / 13 vs 24 / 33). Rift-type splits are not more frequent near plumes than near
their antipodes. The antipode null is itself biased (plumes rise away from slabs, so their
antipodes lie nearer the trenches where initiations happen), and initiations swing both ways
between windows (base 0–500: fewer near plumes; 500–1000 in the branch: more); initiations are
not driven by the plumes in either version (they follow old margins), so their excess is
taken as noise of the null.

**Plume-section overloads** (the best section through a plume, per analysed plate with a plume):

| | analyses | < 0.25 | 0.25–0.5 | 0.5–1 | ≥ 1 | breakups along one |
|---|---|---|---|---|---|---|
| step 0 (straight, 4 × 400 Myr) | 2 448 | 91.5% | 6.0% | 2.0% | 0.5% | 0 |
| straight (8 × 1000 Myr) | 11 544 | 87.2% | 8.6% | 3.8% | 0.5% | 2 |
| arms (8 × 1000 Myr) | 11 223 | 85.2% | 9.8% | 4.1% | 0.9% | 2 |

### Continents, land and deformation, 500–1000 Myr

| | base | straight | arms | nofabric |
|---|---|---|---|---|
| continental crust (% of surface) | 34.6 | 35.9 (t +1.4) | 35.7 (t +1.2) | 36.7 |
| crust of continental origin | 37.9 | 40.3 | 39.9 (t +1.5) | 39.4 |
| land | 29.6 | 29.1 | 29.3 (t −0.5) | 32.0 |
| mean land elevation (m) | 804 | 589 | 622 (t −1.1) | 555 |
| flooded continental crust (%) | 23.9 | 30.0 | 28.9 (t +1.7) | 21.1 |
| continental crust > 50 km (%) | 2.2 | 1.3 | 1.9 (t −0.2) | 0.2 |
| largest continental mass (% of continental crust) | 91.0 | 91.3 | 91.5 (t +0.2) | 98.7 |
| land masses (without specks) | 15.0 | 18.1 | 17.9 (t +1.6) | 13.3 |
| deforming area (≥ 10⁻¹⁶ s⁻¹, % of surface) | 8.2 | 5.5 | 4.9 (t −2.0) | 2.5 |

0–500 Myr: no difference beyond |t| = 1 in continental area, land, elevation, flooding, thick
crust or deformation.

### Supercontinent

The default start is dispersed; in all arms the continents assemble within a few hundred Myr
into one mass of ~90% of the continental crust (≥ 98% at some time after 100 Myr on every seed)
and stay mostly together: the largest mass fell below 35% on 2 of 8 base seeds (at 80 and
130 Myr, reassembled ~100 Myr later) and on 1 of 8 branch seeds (briefly, at 250–290 Myr). From
a supercontinent start (4 seeds × 500 Myr) neither main nor the branch disperses it; the largest
mass every 50 Myr:

| seed | base | branch (straight = arms) |
|---|---|---|
| 0 | 100 100 72 71 69 69 69 99 99 100 99 | 100 100 77 76 76 76 99 99 98 98 98 |
| 1 | 100 93 93 86 86 94 93 99 99 99 99 | 100 93 88 88 91 92 100 100 100 100 100 |
| 2 | 100 100 100 59 59 61 92 93 92 92 99 | 100 100 100 99 99 99 99 99 99 99 99 |
| 3 | 100 100 99 98 92 92 94 95 95 94 94 | 100 100 92 99 100 100 99 100 100 99 100 |

Plates 23.0 → 15.9 (t −5.1), largest plate 20 → 36% (t +3.2) over 0–500 Myr; on seed 2 the
largest plate covers 50–61% of the surface for ~300 Myr. Main's supercontinent cycle is weak
already (the archive notes slow breakup); without the plume rule the supercontinent's plate
mostly stays whole. (The "largest mass" counts continental crust touching across plate
boundaries as one.)

### Extension fabric (#54)

Arms, 500–1000 Myr (0–500 Myr in brackets):

* Mean fabric on continental crust 0.09 (0.06); continental cells with fabric ≥ 0.25: 3.2%
  near margins, 8.7% deep inside continents (3.8%, 4.4%).
* New boundaries: continental cells on fabric 25% vs 13% of the split plates (18% vs 6%); fabric
  deep inside continents 15% vs 10% (7% vs 3%). By cause (500–1000): #182 opening 36% vs 23%,
  sheet splits 27% vs 9%, initiations 14% vs 7%. Belts: 38% vs 26% (base 58% vs 39%).
* **Caveat (review):** fabric is counted at the split. Much of it is lithosphere stretched in
  the same deforming episode that then becomes a boundary, which is not the reactivation of an
  old failed arm; the fabric's age was not tracked, and fabric overlaps orogenic belts (which
  boundaries follow anyway). So the ×2 is an upper bound for reactivation.
* Inversion: weighted by shortening, 20% of the sheet's shortening over 1000 Myr falls on fabric
  (per seed 1–96%; the high seeds have little shortening, alternating stretching and shortening of
  the same zones).
* Without the fabric (`nofabric`) there are still fewer splits (5.2 per 100 Myr: #182 opening
  breakups 1.2 instead of 1.8) and plates (15): the fabric gives #182 and initiations weak zones
  to break along.
* The sheet stretches plume heads only a little (oceanic heads stay rigid; continental ones are
  capped by the viscosity floor), so the fabric comes mostly from stretching in deforming
  continental interiors and from boundary rifts. It forms broad patches, not narrow arms:
  radial push on a nearly uniform sheet stretches a disc, and a 50–100 km graben is 1–3 cells
  (#64). Map (straight, seed 0, 600 Myr; plate boundaries black):

  ![](img/fabric-seed0-600.png)

### Runtime

Paired local runs, one core, 200 Myr, seed 0: base 119.2 s, straight 119.8 s (+0.5%; section
analysis 4.2 → 5.7 s), arms 131.4 s (section analysis 7.7 s; the rest of the difference is a
different history: the runs diverge once a split differs). Seed runs on GitHub: base
867–1129 s, arms 710–884 s.

## Decision

The plan fixed the threshold in advance (≥ 1 plume rift per 100 Myr above chance). It was missed:
with the swell push, magma-assisted opening at Buck's factor and sections through the plumes
(straight and with two arms), plates do not break at plumes. The plume acts on ~2000 km of a
section that crosses the whole plate, and a rigid-plastic analysis needs the whole section to
yield. In nature plume rifts *propagate*: a magma-filled rift tip concentrates stress and cuts the
plate segment by segment (Buck 2006; Afar, Red Sea), which a limit analysis of whole sections
does not represent. The outcome also depends on #182's strength (5 × 10¹³ N/m, settled on #182).

Options:

1. **Accept physics only** (this PR as it is): no plume rifts; fewer plates (19 vs 27) and a
   largest plate of ~37% on average, up to 68% on some seeds and 50–61% for ~300 Myr from a
   supercontinent start, which is implausible for an Earth-like planet; supercontinents stay
   together longer.
2. **Keep the plume rule as well:** revert the removal commit (42f8aab); the swell load, magma
   weakening, plume sections and fabric stay. Plume rifts as on main, still by the nucleation
   lines; #51 would stay open. **Not run** in this combination: plate counts would be expected
   near main's plus the fabric's effect, but that is untested.
3. **Model rift propagation** as a stated physical assumption, e.g. magma channelled along an
   opening section from the plume (the magma factor along the whole propagating rift instead of
   only over the head). A new model with its own parameter (how far melt propagates), not a
   literature constant; it needs its own plan.

No knob was tuned: `magma_weakening` 0.1, `swell_load` 1 and `rift_fabric` 1 are physical
defaults; the switches exist for the ablation.
