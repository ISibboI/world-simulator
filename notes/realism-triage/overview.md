# Realism triage: the open issues against an Earth-like planet

_2026-11-20, `main` at 3527833 (after #50). Request: "verify that the open issues are making the
simulation less realistic. The goal is to simulate an Earth-like planet, not Earth exactly. Then
fix the issues, ideally in large batches."_

## Method

- All 27 open issues were read with their comments, linked notes and the merged PRs
  #22–#50.
- Baseline: `main`, 8 seeds (0–7) × 1000 Myr, default parameters, `--headless --material-budget
  --cell-dump`, snapshots every 100 Myr. Numbers below are the mean over seeds of the mean over
  the snapshots of 500–1000 Myr, with the seed range in brackets, unless noted.
- Two short prototypes on seeds 0–3 tested the claimed cause of #32/#42 (see "Plate boundaries").
- Scripts (agent task directory, not in the repository): `compare.py` (log metrics, Welch t),
  `triage.py` (cell-dump metrics per issue).

**Criterion.** An issue is a realism problem if the model shows physically implausible
behaviour, violates a conservation law, shows a numerical artefact, or produces a value that an
Earth-like planet could not plausibly have. A value that differs from Earth's present-day number
but lies inside the range of Earth's own past, or of a planet with a slightly different mantle
temperature, ocean volume or continent arrangement, is not a problem by itself.

Verdicts: **(a)** real realism problem · **(b)** Earth-exact target only · **(c)** fixed or
obsolete · **(d)** diagnostic or feature.

## Verdicts

| # | Issue | Verdict | Severity | Evidence on `main` | Recommendation |
|---|---|---|---|---|---|
| 47 | Single-cell spikes on capped sediment columns | (a) numerical artefact | high | Cause found: sediment that finds no room at a river mouth is piled on **one** land cell (`erode`, "overfill"). 17% of the river sediment ends this way (4400 km·cells per 100 Myr ≈ 0.55 km³/yr of 3.75). A large river puts tens of km on one cell in one step; metamorphism turns it into crust. Every spike > 2 km above its neighbours sits on the capped column (12.9 km): 4 in 48 snapshots, up to 5.2 km high | batch 1 |
| 14 | Ancient ocean persists and traps sediment | (a) implausible | high | Oceanic crust ≥ 200 Myr: 4.6% of the oceanic crust (3.3% of the sea floor by the log's measure). The oldest oceanic crust is **880 Myr** (830–915): part of the initial sea floor never subducts. It holds 45% (35–55) of all sediment on oceanic crust, 8.5 km on average, and 36% of it is filled to above −200 m. Earth's oldest in-place sea floor is ~180 Myr (340 Myr in the eastern Mediterranean) | batch 2 |
| 42 | Every converging oceanic contact becomes a trench | (a) missing physics, **but not the main cause of #32** | medium | The rule is as described. Measured: trenches without a self-sustaining slab (< 150 km) are only 5–8% of the trench length; the other 92–95% are mature subduction zones with a mean slab of ~510 km. See "Plate boundaries" | batch 2 |
| 32 | Sea-floor production ~1.7× Earth's | (a) for the age structure, (b) for the number | medium | Production 5.97 km²/yr (5.2–6.5). 6 km²/yr alone is plausible for an Earth-like planet (Earth's own Cretaceous estimates reach 1.5–2× today's). Implausible is how it is consumed: 36% of the sea floor is younger than 20 Myr (Earth 21%), a third of new sea floor is subducted before it is 20 Myr old, and nothing resists the subduction of young, buoyant lithosphere | batch 2; reframe the target as "age–area distribution and young-crust consumption", not 3.4 km²/yr |
| 2 | Ocean floor too old, plates only die | (c) obsolete umbrella | — | The title no longer holds (mean sea-floor age 52 Myr, plate births ≈ deaths). Its open items are #32/#42/#43 (relative speeds, young-crust consumption) and #14 | close; superseded by #14, #32, #42, #43 |
| 43 | Too few slow strike-slip and diffuse continental boundaries | (a) partly, (b) partly | medium | Relative speed median 6.8 cm/yr; 6% of the boundary slower than 2 cm/yr (Earth 39%); strike-slip 14%. Plates with random poles give ~22% strike-slip, so the missing class is the slow boundaries between plates without slabs, not transforms as such | batch 2 (measured outcome of the force balance) |
| 6 | New oceanic crust at convergent boundaries | (a) numerical artefact | low | Created sea floor exceeds the kinematic opening by 12% (6.69 vs 5.97 km²/yr) | batch 2 |
| 37 | Slab dynamic-topography depressions too deep | (a) downstream | low–medium | Minimum −1.9 km dry (−1.6…−2.4); 5% of the surface below −1 km; rms 510 m. Mantle-flow models of Earth give lows of ~−1 to −1.5 km and an rms of 300–500 m. The subduction flux that feeds it is ~2× Earth's | re-measure after batch 2 |
| 45 | Sediment part of the sea level fills more slowly with lakes | (b)/(c) | low | Slope 500–1000 Myr: 8.9 m/100 Myr (2.2–15.8), mean inside the ≤ 10 target; the level at 1000 Myr was unchanged by #44. A slower approach to the same level is not unrealistic | re-measure after batch 1 (the overfill fix changes where 17% of the sediment goes), then close |
| 46 | Endorheic share and lake area; single-cell pits | (a) for the pits, (b) for the shares | low | Endorheic 11.3% of land, of which 5.5 points drain into single-cell pits (resampling noise: 1.5% of land cells are pits, only 0.08% deeper than 100 m). Lakes 2.9% of land and endorheic 6–11% are plausible for an Earth-like planet (Earth: lakes ~2–4% of unglaciated land, endorheic 18–20%) | batch 1 (pits); drop the Earth-exact targets |
| 7 | Erosion inconsistencies | (c) fixed by #44 | — | Isostatic incision, runoff (37,700 km³/yr; Earth ~40,000) and lakes are in. The rest is #46 | close |
| 48 | Sediment flux to the ocean ~half of Earth's | (b) | — | 3.75 km³/yr (2.6–4.7) ≈ 7.7 Gt/yr. Earth's pre-human estimate is 14–16 Gt/yr, but that is today's high-relief, glaciated Earth; Phanerozoic averages from preserved sediment are ~5–10 Gt/yr. The flux is limited by uplift, not by K (#29: doubling K lowered the relief, not the flux) | close, or reframe as "glacial and hillslope erosion" (feature) |
| 49 | Land too low and too flat below the highest peaks | (b) | — | Mean land 570 m (400–690), p90 1.5 km, p99 3.3 km, highest cell 4.6 km. Earth today: ~800 m, p99 ~4.5 km, with the Tibetan plateau, an unusual feature. All inside a plausible range | close; re-measure after batches 1 and 2 |
| 8 | Shelves are sediment piles on oceanic crust | (c) fixed | — | 94.5% of shelf cells are on continental-class crust, 2.0% on oceanic. Shelf sediment 3.6 km on average, 25% of shelf cells above 6 km (Earth's passive margins carry 2–15 km) | close |
| 4 | Continental crust not conserved with erosion | (c) fixed | — | Felsic + sediment net −0.36 Gt/yr (−1.4…+0.6), inside Earth's 0…−1.5. The ledger closes (sea-level budget unexplained 0.00 m) | close; the trapped ancient basins stay in #14 |
| 33 | Land ice on tropical highlands | (a) | low–medium | Tropical land above 3 km is 4 K colder than the lapse rate explains (sea-level equivalent 18.2 °C vs 22.0 °C in the lowlands), and half of it (1.0% of tropical land, up to 2.9%) is below the −10 °C ice-sheet criterion, which a 6.5 K/km lapse rate reaches only above ~5 km. The ice criterion uses the annual mean; real ice sheets need the summer to stay below melting | batch 3 |
| 25 | Poles too warm | (b), mostly (c) | — | Sea-ice thermodynamics and land/ocean surfaces are in (#24). Polar ocean > 75°N: −27 °C in January, 0 °C in July. Annual mean poleward of 70°: −13.4 °C (Earth −18…−25). Earth itself had far warmer poles for most of the Phanerozoic | close |
| 26 | Ocean currents | (c) implemented, rest (b) | — | Gyres and the Ekman overturning are in (#24). Ocean heat transport at 20° 1.2 PW (Earth 1.5–2), gyres 13 Sv (Earth 30–100; limited by the 440 km grid) | close |
| 27 | Monsoon, storm-track latitude | (c) mostly, rest (d) | — | Summer/winter rain ratio on land at 10–30°: 5.1 (target ≥ 3); land rain at 20–35°: 0.42 m/yr; storm-track maximum at 48°. Arid land 27% (23–33). No seasonal wind reversal (thermal winds exist but are off) | close, or reframe as the feature "seasonal wind reversal" |
| 40 | Suture mark mixing (max rule) | (a) numerical artefact | low | Marked share of continental cells grows ~4 points per 100 Myr and reaches 44% at 1000 Myr (Earth's Phanerozoic belts are ~30–40% of the continents). How much of that is over-spreading by the max rule is not measured yet | batch 4 |
| 52 | 30% of plume rifts end > 500 km from their plume | (a) artefact | low | From PR #50 (8 seeds): 30% on `main` and on the PR alike. Not re-measured here | batch 4 |
| 53 | Plume-head weakening fixed in the mantle frame | (a) inconsistency | low | By code: the weakening is a field at the plume's mantle position and does not move with the plate. PR #50's ablation showed it changes the rift geometry little | batch 4 |
| 51 | Plume rifts: plume-stress-weighted cut cost | (d) refinement | — | Replaces a kinematic stand-in by a force model; no measured defect beyond #52 | batch 4 if #52 needs it |
| 54 | Failed rift arms as inherited weak zones | (d) feature | — | — | later |
| 36 | Persistent negative mantle sea-level term | (d) diagnostic | — | −74 m per 100 Myr in every seed. The dynamic-topography field has zero mean and does not grow; the term is one-signed because the budget attributes each step's field change before erosion levels it (the "coastal" term carries the other half). No drift of the sea level itself | close, or fold into #35 |
| 35 | Continental cover diagnostics | (d) diagnostic | — | — | later |
| 13 | Asteroid impacts | (d) feature | — | — | later |

Counts (27 issues): 12 real problems (a) — #6, #14, #32, #33, #37, #40, #42, #43, #46, #47, #52,
#53, of which #32, #43 and #46 only in part; 4 Earth-exact only (b) — #25, #45, #48, #49; 6 fixed
or obsolete (c) — #2, #4, #7, #8, #26, #27; 5 diagnostics or features (d) — #13, #35, #36, #51,
#54.

## Plate boundaries: what the prototypes show

#32's investigation named "every converging oceanic contact becomes a trench" (#42) as the main
cause of the high production. Two prototypes on seeds 0–3 × 1000 Myr tested this (500–1000 Myr,
Welch t against the same seeds of `main`):

1. **Subduction-initiation resistance.** A trench whose slab is shorter than 150 km resists
   convergence with a plastic strength of 3 × 10¹² N/m (Gurnis et al. 2004; Toth & Gurnis 1998),
   fading as the slab grows. Run to 400 Myr only: trench length and production did not move.
   Reason: such incipient trenches are only 7–11 thousand km of the ~125 thousand km of trench.
2. **The same plus net slab buoyancy.** Lithosphere younger than 10 Myr is buoyant (Cloos 1993):
   slab pull scales with `√age − √10 Myr`, and young slabs resist.

| | main | prototype 2 | t |
|---|---|---|---|
| Sea-floor production (km²/yr) | 6.01 | 5.58 | −1.8 |
| Trench / ridge length (10³ km) | 127 / 101 | 133 / 108 | +0.7 / +1.0 |
| Incipient trenches (10³ km) | — | 12.2, converging at 2.0 cm/yr (others 4.1) | |
| Relative speed, median (cm/yr) | 6.69 | 5.81 | −3.9 |
| Boundary slower than 2 cm/yr (%) | 6.7 | 5.8 | −1.0 |
| Sea floor younger than 20 Myr (%) | 35.9 | 33.6 | −1.5 |
| Mean sea-floor age (Myr) | 52.8 | 52.4 | −0.2 |
| Slab pull (10²¹ N) | 1.38 | 1.12 | −4.4 |

**Conclusion.** Both processes are real physics and move the numbers in the right direction, but
by ~10%. #42 is not the main cause of #32. More than 90% of the trench length is mature
subduction with full-length slabs. What differs from Earth is that nearly every plate carries
slabs, so almost no pair of neighbours moves slowly against each other (Earth's slow boundaries
lie between its slab-free plates: Africa, Antarctica, Eurasia, the Americas). Batch 2 therefore
starts with a measurement, not with a fix: slab-attached share of each plate's boundary, trench
lifetimes and what ends them, and where the 880 Myr old sea floor of #14 sits relative to the
trenches. The prototype code is kept as a patch for that batch.

## Batches

Ordered by verified impact, by how certain the cause is, and by dependencies. One PR per batch.

1. **Sediment routing artefacts: #47, #46 (pits), then re-measure #45, #48, #49.**
   The cause of #47 is known and it misroutes 17% of the river sediment, so it also touches the
   sediment budget (#45), the relief (#49) and the flux to the ocean (#48). Independent of the
   plate dynamics.
2. **Plate boundaries and sea floor: #14, #32, #42, #43, #6, then re-measure #37.**
   Highest impact, but the cause is not established (see above); starts with the measurement.
   Every other tectonic metric depends on it, so it comes before the weak-zone batch.
3. **Highland climate: #33.** The summer-melt criterion for ice sheets, which #24 could not
   switch on without a snowball; small area, contained in the temperature model.
4. **Weak zones and plume rifts: #40, #52, #53 (+ #51 if #52 needs it).** Low severity;
   after batch 2, because plate-boundary changes shift where rifts and sutures form.

Not planned: #13, #35, #54 (features and diagnostics), and the issues recommended for closing.

## Close recommendations

- **Fixed or obsolete:** #2 (superseded by #14, #32, #42, #43), #4, #7, #8.
- **Earth-exact targets inside the plausible range:** #25, #26, #27 (or reframe as the feature
  "seasonal wind reversal"), #48, #49.
- **Diagnostic without a defect:** #36.
- **After its batch has been re-measured:** #45.

## Baseline tables

Log metrics (`compare.py`), 8 seeds, 500–1000 Myr:

window 500-1000 Myr; seeds: main: 8
| metric | main |
|---|---|
| plates | 25.23 (18.80…29.60) |
| plates > 1% | 14.90 (10.20…18.60) |
| mean speed (cm/yr) | 3.86 (3.62…4.04) |
| speed 99% (cm/yr) | 9.47 (8.90…10.18) |
| trenches (10^3 km) | 125 (100…140) |
| ridges (10^3 km) | 99.15 (79.41…109.38) |
| collisions (10^3 km) | 14.70 (7.46…20.20) |
| slab pull (1e21 N) | 1.39 (1.27…1.51) |
| relative speed median (cm/yr) | 6.77 (6.38…7.22) |
| boundary slower than 2 cm/yr (%) | 6.08 (4.40…8.60) |
| opening/closing (%) | 61.35 (58.40…70.60) |
| strike-slip (%) | 13.88 (8.60…15.60) |
| sea-floor production kinematic (km2/yr) | 5.97 (5.18…6.51) |
| sea-floor production created (km2/yr) | 6.69 (5.69…7.25) |
| mean sea-floor age (Myr) | 52.31 (47.58…55.58) |
| ocean age median (Myr) | 32.10 (28.80…34.80) |
| sea floor >= 200 Myr (%) | 3.32 (2.26…4.46) |
| sea floor < 20 Myr (% of ocean) | 35.55 (31.60…38.20) |
| sea floor 20-40 Myr (%) | 22.62 (21.40…24.20) |
| ocean deeper than 6 km (%) | 3.92 (2.86…6.42) |
| continental (% of surface) | 35.27 (33.76…38.18) |
| land (% of surface) | 29.22 (27.76…30.84) |
| mean land (m) | 573 (397…713) |
| highest point (m) | 4605 (3514…5288) |
| flooded continental crust (%) | 32.30 (27.00…35.60) |
| crust > 50 km (% of continental) | 1.65 (0.40…3.20) |
| sediment part of sea level (m) | 269 (252…292) |
| land lower than 10 m (% of land) | 2.31 (1.92…2.74) |
| shelf area (% of surface) | 6.28 (5.54…7.22) |
| shelf mean sediment (km) | 3.61 (3.14…4.06) |
| dynamic topography min (m) | -1928 (-2475…-1590) |
| dynamic topography max (m) | 1117 (971…1287) |
| sea-level budget mantle (m/100 Myr) | -73.61 (-84.48…-58.50) |
| felsic+sediment net (Gt/yr) | -0.36 (-1.36…0.56) |
| trench sediment (Gt/yr) | 3.67 (2.66…4.84) |
| collision stacking (Gt/yr) | 13.61 (5.48…17.42) |
| land precipitation (m/yr) | 0.75 (0.66…0.80) |
| runoff (km3/yr) | 37658 (32653…45060) |
| largest river (m3/s) | 79344 (52712…132353) |
| sediment to ocean (km3/yr) | 3.75 (2.56…4.66) |
| endorheic (% of land) | 11.29 (8.28…15.82) |
| endorheic into single-cell pits (% of land) | 5.51 (4.20…7.50) |
| lakes (% of land) | 2.91 (2.42…4.00) |
| lake evaporation (km3/yr) | 2904 (1850…3619) |
| global temperature (C) | 14.21 (13.82…14.60) |
| equator (C) | 25.94 (25.42…26.36) |
| poles (C) | -13.42 (-14.42…-12.46) |
| permanently frozen (%) | 11.18 (9.58…11.96) |
| ocean heat transport 20N (PW) | 1.21 (0.98…1.32) |
| gyres (Sv) | 12.65 (10.80…14.40) |
| rifts /100 Myr | 4.45 (3.00…5.40) |
| initiations /100 Myr | 8.57 (5.20…12.00) |
| sutures /100 Myr | 3.30 (1.40…4.20) |
| plates born /100 Myr | 14.07 (8.40…17.40) |
| segments created /100 Myr | 515 (412…575) |
| overfill (km cells /100 Myr) | 4373 (3168…6007) |
| sediment-part slope (m/100 Myr) | 8.85 (2.16…15.82) |
| highest point, max over all snapshots (m) | 5614 (5210…5960) |
| runtime (s) | 592 (560…633) |

Cell-dump metrics (`triage.py`), 8 seeds, 500–1000 Myr:

| metric | mean (seed range) |
|---|---|
| #47 spikes (cells > 2 km above all neighbours) per snapshot | 0.08  (0.00 .. 0.33) |
| #47 spikes on sediment > 10 km | 0.08  (0.00 .. 0.33) |
| #46 single-cell land pits (% of land cells) | 1.46  (1.29 .. 1.66) |
| #46 pits deeper than 100 m (% of land cells) | 0.08  (0.05 .. 0.11) |
| #49 mean land (m) | 568.87  (406.92 .. 686.98) |
| #49 land p50 (m) | 323.16  (241.02 .. 357.39) |
| #49 land p90 (m) | 1468.17  (926.12 .. 1996.74) |
| #49 land p99 (m) | 3326.26  (2546.34 .. 3996.33) |
| #49 land max (m) | 4624.79  (3786.10 .. 5322.38) |
| #49 land above 2 km (% of land) | 6.34  (2.73 .. 10.36) |
| land (% of surface) | 29.18  (27.96 .. 30.27) |
| #33 tropical land (|lat|<25) with t_annual < -10 C (% of tropical land) | 0.97  (0.00 .. 2.88) |
| #33 tropical land with t_annual < 0 C (%) | 2.99  (0.50 .. 7.03) |
| #33 tropical land above 3 km (%) | 2.01  (0.29 .. 4.65) |
| #33 tropical land > 3 km: t_annual + 6.5 K/km * elev (C) | 18.21  (15.11 .. 22.49) |
| #33 tropical lowland (< 500 m) t_annual (C) | 22.02  (21.15 .. 22.73) |
| #33 land with t_annual < -10 C (% of land) | 10.93  (5.41 .. 16.79) |
| #33 share of ice-criterion land at |lat| < 40 (%) | 7.98  (1.29 .. 14.28) |
| #25 annual mean T poleward of 70 (C) | -13.38  (-14.19 .. -12.44) |
| #25 ... ocean only (C) | -12.29  (-12.68 .. -11.84) |
| #25 ocean > 75N Jan (C) | -26.80  (-29.16 .. -25.69) |
| #25 ocean > 75N Jul (C) | 0.20  (-0.05 .. 0.80) |
| #25 land 55-70 seasonal range (K) | 41.57  (36.09 .. 46.11) |
| #25 land 55-70 summer (C) | 14.85  (9.72 .. 20.54) |
| global mean T (C) | 14.17  (13.89 .. 14.57) |
| equator (|lat|<5) T (C) | 25.57  (24.95 .. 26.04) |
| #27 land precipitation (m/yr) | 0.75  (0.69 .. 0.82) |
| #27 land P 20-35 (m/yr) | 0.42  (0.36 .. 0.46) |
| #27 land 10-30 summer/winter P ratio | 5.09  (4.61 .. 5.94) |
| #27 arid land (< 0.25 m/yr, % of land) | 26.82  (23.22 .. 32.76) |
| #27 extratropical P maximum latitude (deg) | 48.44  (47.08 .. 50.42) |
| #14 oceanic crust >= 200 Myr (% of oceanic crust) | 4.57  (3.45 .. 5.86) |
| #14 mean sediment on >= 200 Myr oceanic crust (km) | 8.54  (7.99 .. 9.38) |
| #14 share of oceanic sediment on >= 200 Myr crust (%) | 44.62  (35.28 .. 55.23) |
| #14 >= 200 Myr oceanic crust above -200 m (% of it) | 35.72  (27.39 .. 41.02) |
| #14 max oceanic age (Myr) | 880.27  (829.47 .. 914.90) |
| mean oceanic crust age (Myr) | 58.54  (53.24 .. 62.43) |
| #2 oceanic crust < 20 Myr (% of oceanic) | 35.48  (31.41 .. 38.00) |
| #2 ocean deeper than 6 km (% of surface) | 3.87  (2.84 .. 6.40) |
| #8 shelf (0..-200 m) on continental class (%) | 94.54  (93.40 .. 95.80) |
| #8 shelf on oceanic class (%) | 1.99  (1.48 .. 2.85) |
| #8 shelf mean sediment (km) | 3.57  (3.12 .. 3.96) |
| #8 shelf cells with sediment > 6 km (%) | 25.02  (20.59 .. 29.02) |
| #37 dynamic topography min (m, dry) | -1902.33  (-2358.77 .. -1605.72) |
| #37 dynamic topography p1 (m) | -1482.49  (-1909.77 .. -1208.95) |
| #37 dynamic topography p99 (m) | 730.06  (687.07 .. 820.60) |
| #37 dynamic topography rms (m) | 511.90  (443.70 .. 640.02) |
| #37 cells with dyn < -1000 m (% of surface) | 5.17  (2.57 .. 10.07) |
| continental crust thickness p50 (km) | 29.13  (28.73 .. 29.43) |
| continental crust > 50 km (% of continental) | 1.72  (0.54 .. 3.04) |
| flooded continental crust (%) | 32.02  (26.71 .. 34.88) |
| #40 continental cells with suture mark (%) | 36.10  (30.32 .. 40.35) |

Spikes (seed, time, cell, elevation, height above the highest neighbour, sediment, crust thickness, class, coastal, latitude):

```
  s0   800   1045    3400    2679   12914   30979 0 1.0  -29.0
  s1   700  29226    5210    4390   12914   44716 0 0.0   -5.7
  s3   600   5366    3168    2584   12914   34938 0 0.0   15.9
  s3   800   7645    3747    2688   12914   38401 0 0.0  -23.0
```
