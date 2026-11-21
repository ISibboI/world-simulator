# Review of notes/issue-43/investigation.md (draft, 2026-11-21)

Reviewer: independent subagent. No repository files changed. Checks were single-threaded Python on
the existing dumps and PB2002 data. Helper scripts and outputs are in `~/tasks/issue-43/tmp/review/`
(`pb_tests.py`, `pb_checks.py`, `dump_checks.py`, `sim_fixed.py`, `fixed_all.txt`) and
`~/tasks/issue-43/tmp/review_pca600.txt`.

Overall: the numbers in sections 2 and 3 reproduce exactly from the scripts, and the PB2002 parsing
is correct. The subduction excess and the oblique-trench finding hold up under every check I ran.
The main problem is the **orientation judgement**. The draft's own coarsening table shows that
Earth, digitised at the model's resolution, also has a nearly flat angle distribution. The claim
"boundaries hardly follow the plate motion / implausible" is therefore overstated, and
recommendation 3 is sized wrongly. A few code statements in section 5 also need correcting.

## Findings, by importance

### 1. The orientation judgement ignores the draft's own equal-resolution comparison (major)
- Earth's non-SUB angle distribution (10° bins, 0 = strike-slip):
  - 45 km steps: 26 10 6 6 5 5 7 10 25
  - 110 km chords: **17 9 9 9 8 9 10 11 18**
  - 220 km: 15 10 11 8 9 7 12 13 15
  - 440 km: 12 12 10 9 9 9 13 12 14
- Model: 8 8 8 9 10 12 14 15 16.
- At equal resolution the difference is a factor ~2 in the 0–10° bin (8 vs 17%). The model's
  distribution rises steadily towards head-on, while Earth's at 110 km is a weak U. It is not
  "two peaks vs random".
- The summary bullet ("hardly follows the plate motion … on Earth two peaks at 0–10° and 80–90°"),
  §3 ("nearly the distribution of randomly oriented boundaries") and the §4 row ("implausible")
  compare the model with Earth at 45 km.
- Noise test (Gaussian azimuth noise added to PB2002):
  - 20° on the 45 km steps flattens Earth to 15 13 10 9 7 9 10 12 15.
  - 20° on the 110 km chords flattens it to 14 11 9 9 10 10 12 11 14.
  - So a noise-free model normal is needed for the comparison to mean anything.
- The model's normals are not that noisy. A PCA normal over 600 km (`--pca-km 600`, s0+s1) gives
  non-SUB 8 8 8 9 10 11 13 15 17 and SUB 7 8 9 9 11 12 14 15 15, the same as the smoothed normal
  (s0+s1 smoothed ≈ 8 8.5 8.5 9.5 10.5 11.5 13 14.5 16). The model's lack of a strike-slip peak is
  therefore real, but modest.
- Gerya 2010 and Püthe & Gerya 2014 describe ridge–transform patterns with offsets of tens of km.
  - In PB2002, 35% of oceanic transform length is in runs < 110 km and 58% in runs < 220 km
    (`pb_checks.py`).
  - So those papers do not show that a 110 km model must show strong orientation.
- **Fix:** compare at 110 km in the summary, §3 and §4:
  - "0–10° bin 8% vs 17%; transforms 8% vs 22% (12% oceanic + 10% continental)".
  - Downgrade the §4 row from "implausible" to "weaker than Earth at equal resolution; the deficit
    is mainly in continental transforms (2% vs 10%) and partly in oceanic transforms (6% vs 12%)".
  - State that the Gerya results are sub-grid.

### 2. Recommendation 3 (self-orienting ridges) is aimed at the smaller part of the transform deficit (major for priorities)
- At 110 km the transform deficit is 14 pp. Of this, 6 pp are oceanic (6% vs 12%) and 8 pp are
  continental (2% vs 10%). Most of Earth's oceanic offsets are sub-grid (finding 1).
- Rec 3 can gain at most ~6 pp, and only for offsets ≥ 2 cells. Under "offset ≥ 2 cells", that
  is the 42% of OTF length in runs ≥ 220 km.
- The continental transform deficit belongs to mechanism 5 (few slab-free continental plate
  pairs), not to ridge geometry.
- **Fix:** state the ceiling (~+6 pp oceanic transform at best). Consider ranking rec 3 below
  recs 4 and 5, or tie it to the continental-transform deficit too.

### 3. §5 mechanism 2: "the force balance uses smoothed normals" is only partly true (medium)
- `boundary_force` (dynamics.rs:310–326) applies slab pull and ridge push along the raw
  `boundary.normal`. `slab_resistance` also uses the raw normal (dynamics.rs:185–193).
- Only the boundary shear coupling (dynamics.rs:209–227) and the collision resistance
  (dynamics.rs:229–237) use `smoothed_normals`.
- Trench classification in `boundary_cells` (dynamics.rs:106–121) also uses the raw per-cell
  convergence.
- The rest of mechanism 2 is correct (dynamics.rs:404–470):
  - Growth sums only trench cells with raw convergence > 0 and averages over those cells only, so
    the mean is biased upward.
  - `slab_length *= decay` runs only when no trench cell of the segment converges.
  - The "~20° scatter" figure is the code comment at dynamics.rs:232. `smoothed_normals` says
    "±30° or more" (dynamics.rs:332).
- Supporting number from the dumps (seeds 0–3): 7.5% of trench (`t`) cells diverge along the
  smoothed normal (3 908 / 52 110), and 7.1% diverge faster than 0.1 cm/yr. They are trenches only
  through the raw normal plus hysteresis.
- **Fix:**
  - Reword mechanism 2 to "slab growth, trench classification, slab pull and slab resistance use
    the raw normal; only shear coupling and collision resistance use the smoothed one".
  - In rec 1, replace "(as the force balance does)" with "(as shear coupling and collision
    resistance do)".
  - Recommend switching the trench test in `boundary_cells` to the smoothed normal as well.

### 4. The overriding-side subduction rule in sim_classes.py over-counts subduction a little (medium-minor)
- `sim_classes.py:104` counts an overriding-plate cell as subduction whenever its segment has a
  slab and the smoothed conv is > −0.1, without checking the cell's own kind.
- In seeds 0–3 there are 57 152 such overriding lines vs 52 110 `t` lines. Among them:
  - 2 553 are kind `c`: the model itself classes them as **collision**, because both crusts resist
    subduction (dynamics.rs:118).
  - 1 860 are kind `r` (ridge).
  - 2 609 lie in segments with no trench cell at all in that snapshot.
- With a corrected rule (own kind not `c`/`r`, and the segment has `t` cells; `review/sim_fixed.py`,
  8 seeds):
  - subduction 128.2 → **121.8** thousand km (51% → 48%)
  - intra-oceanic 80.3 → 76.8
  - continent–continent convergent 4.2 → 6.3
  - other classes shift by under 2 thousand km
- The conclusions are unchanged.
- The crust used for the split is right in both branches:
  - `t` branch: overriding = other plate, tested with `other_cont` from the other plate's adjacent
    cells.
  - Overriding branch: own `cont[cell]`.
- The continental flag is `is_continental_origin` (felsic > 0.5 and resists subduction,
  crust.rs:198–207). Thick felsic arcs therefore count as continental, which makes "intra-oceanic"
  a lower bound.
- **Fix:** apply the corrected rule, or mention the ±6 thousand km ambiguity.

### 5. The Earth coarsening counts original step lengths, which is inconsistent with the model's length measure (medium-minor)
- The model's length per cell is `R·edge/1.1` (`BOUNDARY_ZIGZAG`, mod.rs:486–488). It is
  calibrated so that a *straight* boundary gets its true length, which is the length of the smooth
  line, i.e. the chord.
- `pb2002_classes.py:138` instead gives each chord the summed step length. For ridge–transform
  staircases this is longer than the chord. `gc_dist` is defined (line 55) but never used.
- With chord lengths at 110 km (`pb_tests.py`):
  - total 245 thousand km
  - oceanic divergent **73.4 (30%)** instead of 84.9 (33%)
  - other classes within ±1
  - angle distribution 19 10 9 8 8 9 9 10 18
- Ridges are still "not the anomaly" (model 35%), but "Earth at 110 km: 33% ridge" should read
  ~30%.
- **Fix:** weight coarsened Earth by chord length, or show both and say which one matches the
  model's measure.

### 6. §5 mechanism 4: suture and collision removal is described incompletely (minor)
- "Suture after 30 Myr" is correct (`suture_time: 30.0`, events.rs:125), but it is conditional.
  `collision_time` grows only while the pair has ≥ 10 colliding cells and at least as many
  colliding as trench cells (`suture_trench_ratio` 1.0, events.rs:1653–1662). Otherwise it
  counts down.
- Missed: pairs that cannot suture get **terrane accretion** after 12 Myr of converging contact
  (`terrane_accretion_time`, events.rs:1693–1712). This is a second, faster remover of collision
  boundaries.
- The statement that the Alpine–Himalayan belt "makes up most of Earth's 23 thousand km of CCB"
  is overstated. Only 10.1 thousand km (44%) of CCB lies in a 10°W–125°E, 20–50°N box. "About
  half" fits better.
- **Fix:** add terrane accretion, mention the suture condition, and change "most" to "about half".

### 7. §5 mechanisms 1, 3 and 5: confirmed, with small nuances
- **Mechanism 1** matches the code:
  - `Polarities::contact` (polarity.rs:237–278) creates a segment on first overlap, with polarity
    from inheritance or buoyancy and no age-contrast test. `initiation_age_contrast` applies only
    to induced initiation.
  - The trench test is at dynamics.rs:110–115: raw conv > 0.1 cm/yr, or slab > 0 and conv > −0.1.
- **Mechanism 3** gap fill (boundaries.rs:460–600):
  - It is a FIFO BFS from all owned cells, so a cell goes to whichever plate's front reaches it
    first. The ridge sits on the mid-line of the gap and keeps the old boundary shape. Correct.
  - `opens_away` needs opening > 0.25·|v_rel| (boundaries.rs:853 and 903), i.e. at least ~14.5°
    from strike. Correct.
- **Mechanism 5**:
  - `continental_drag_factor` = 4 (mod.rs:282) is right. Note that it couples continents more
    strongly to the mantle both ways: it drives them where the flow is fast and brakes them where
    it is not, so "dragged" needs that nuance.
  - "Nearly every plate carries a trench" is confirmed: on average 25.6 of 26 plates have `t`
    cells (seeds 0–3, 500–1000 Myr).

### 8. Smaller wording and number issues
- §3 "The fast types make up 86% of the length (Earth 46%)": these are ridges plus trenches only
  (35 + 51 vs 26 + 20). With oceanic transforms included it would be 91% vs 64%. Name the classes.
- §2 "without orogens the shares barely change (SUB 19%, OSR 30%, OTF 21%)": OSR goes 26 → 30% and
  OTF 18 → 21%. That is a modest change, not "barely".
- Seed range "convergent without slab 8–15": s3 and s4 give 7.5 and 7.6.
- Summary "Resolution is not the cause: halving the resolution…": subdivisions 5 is *coarser*
  (220 km). It shows the mix is stable between 110 and 220 km. It does not show what a finer grid
  would do, and Earth's own OTF share rises 12 → 18% from 110 to 45 km. Reword the "not
  recommended: higher resolution" line accordingly: finer resolution alone is not expected to
  help without rule changes.
- §4 "Earth's Cretaceous ridges were faster": this is debated (e.g. Rowley 2002 argues near-constant
  sea-floor production). Soften it to "possibly".
- §1 "(a) and (b) agree exactly, which validates the script": (b) reuses Bird's own div/rl,
  crust and SUB attributes. The agreement validates the 20° rule and the sign handling, not the
  velocities. That is fine, but say so.
- Rec 1 "transforms 8% → ~15%": the arithmetic (16% of 128 ≈ 20.5 thousand km) is right. It
  assumes no dynamic feedback, so label it as an upper estimate. With the corrected rule from
  finding 4, 15% of ~122 ≈ 18 thousand km.

## Verified correct (no change needed)
- **PB2002 column parsing** (pb2002_classes.py:21–40) matches the readme and Table 2: seq, `:`pair,
  lon1 lat1 lon2 lat2, length, step azimuth, v (mm/yr), v azimuth, divergence, right-lateral,
  elevation, age, class with `:` and `*` (orogen).
  - Spot check, step 3: div = 13.2·sin(142−47) = 13.1 and rl = 13.2·cos(47−142) = −1.15, both
    matching the file.
  - The coarsening uses the same formulas.
  - The CRLF line endings are harmless, because `split()` handles them.
- **20° rule:** `atan2(|div|,|rl|) < 20°` is Bird's "velocity azimuth within 20° of the step
  azimuth", in either direction. Bird's classes reproduce exactly.
- **Dump sign conventions:**
  - The normal points from plate to other.
  - `conv = ((ω_p − ω_o)×r)·n` is positive when converging (statistics.rs:309–327).
  - `strike` is the component along r×n; only |strike| is used.
- **Angle** `atan2(|conv|,|strike|)` is the angle from strike, and 11%/bin is the right uniform
  reference.
- **Half-cell weighting:** both sides are dumped, and the total (252) is consistent with the
  model's `BOUNDARY_ZIGZAG`.
- **PCA normal** (sim_classes.py:80–94): it uses the same plate's cells of the pair within the
  radius, takes the tangent-plane principal axis as strike, and converts the speed correctly
  (rad/Myr·km → cm/yr). It falls back to the smoothed values below 3 cells. The sign is copied from
  the model's conv, which is negligible except when conv = 0 exactly.
- **Numbers re-checked:**
  - all §2/§3 tables
  - seed ranges
  - the subdivision-5 claims (ridge 34–35%, subduction 48–49%, transforms 7–8%, rift 4%)
  - "a third of OTF in offsets < 110 km" (35%)
  - slow Earth boundaries (60% continental or OCB, 31% slow OSR/OTF)
  - 16% / 25% of trench length within 20° / 30° of strike
- **The oblique-trench finding is robust.**
  - Model: 15–16% within 20° with the smoothed, PCA-300 and PCA-600 normals.
  - Earth: 4% at 45 km and at 110 km, and still only 5–8% with 20° of added azimuth noise.
- **Literature citations** (Bird 2003, Gordon 1998, Gerya 2010, Püthe & Gerya 2014, Gurnis et
  al. 2004, van der Meer 2014, Mallard 2016, Leat & Larter 2003) have correct bibliographic
  details as far as I can check from memory. The ~17 000 km / ≈40% intra-oceanic figure is the
  commonly quoted Leat & Larter value. The van der Meer "~75 000 km" figure is from snippets, as
  the draft says.
