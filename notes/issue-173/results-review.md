# Independent evaluation of notes/issue-173/results.md

Date: 2026-11-26. Checked against `agent-scratch/runs/{main,main-nosea,lakes2-rule,lakes2}`, `table.md`,
and reruns of `compare.py` (also with `main-nosea` as the base), `cases.py` (on lakes2 and main-nosea)
and `lakeocean.py`. Also read `Planet::lake_climate_contrast` and `koppen::classes`.

## Numbers

Every number I checked in results.md matches the data or table.md. That covers the summary table
(all rows, ranges and t values), the paired diagnostic table and the text derived from it
(43 → 20 K, −15 K and −13 K for the land rings, ice 12.9 / 6.2 %), the six case-study rows, the
main-nosea counter-examples (seed 7 800 Myr +48.8 / −21.2 °C, P 2.50 / E 2.63; seed 5 1000 Myr), the
tropical-lake numbers (−5.0 K mean, p90 +12.9 K, 36–41 °C, 6.2–8.2 m/yr), the criterion (a) deficits,
and the claim "arid +5 points, t +2.0 against main-nosea" (rerun: +2.0).
Minor issues:
- "Polar cut-off seas have a 15–25 K seasonal cycle": the four cases span 13.5–26.8 K.
- compare.py counts lake cells as `lake > 0`, but the text says "≥ 1 m". The model and the paired
  diagnostic use ≥ 1 m (`LAKE_DEPTH`). The difference is negligible, but the text should not say both.
- The case table is headed "snapshots with > 10 m lake water", but it shows only 6 of the 26 such
  snapshots in `lakes`. It should say "selected". The left-out snapshots include the three
  largest-water ones not shown (seed 0 600 Myr with 226 m, seed 3 900 Myr with 329 m, seed 7
  500 Myr with 73 m).

## Findings

### Must

**M1. The explanation of why cut-off basins hold more water does not fit the data, and it credits
the new climate with what is mainly the effect of the rule being off.**
- *The new climate adds little that can be measured.* main → main-nosea (rule off, old climate)
  already gives 2.4 → 12.6 m of lake water. The largest snapshot goes 12 → 92 m. Against
  main-nosea, `lakes` is not significantly different: mean 24.2 vs 12.6 m (t +1.2), largest
  snapshot 138 vs 92 m (t +0.7). Only the count of snapshots above 10 m differs (3.25 vs 1.75,
  t +2.9). The t +2.6 that results.md quotes is against `main`, so it measures mainly the rule.
  With the rule on, lakes-rule holds no more water than main (2.0 vs 2.4 m).
- *The large values are abrupt cut-offs of deep ocean basins, not basins filling up.* Per-snapshot
  lake water in `lakes`:
  - seed 3: 1 → 388 → 329 → 23 m (850–950 Myr);
  - seed 1: 1 → 220 → 22 m;
  - seed 0: 6 → 226 → 146 → 78 → 33 m.

  These basins have floors at a median of −2.2 to −3.1 km, depths of 2.2–3.0 km and 1600–3800
  cells. At the largest, that is about 9 % of the planet's surface (≈ 48 Mkm²): oceanic basins,
  20–45 times the rule's 0.2 % threshold (≈ 82 cells). Their water is ocean water trapped when the
  sill rose. After that it declines over 1–3 snapshots; it does not "fill from its own rain and
  rivers to its sill".
- *The largest cases are not icy polar seas.* The largest snapshot of all (seed 3, 850 Myr,
  388 m) is a subtropical sea with no ice and E > P (4.06 > 3.57 m/yr). The second (seed 1,
  850 Myr, 220 m) is equatorial and **sea-fed**: 170 m of its water is sea water poured over the
  sill, and its evaporation beyond its own inflow is 36,600 km³/yr. Seed 7, 500 Myr (73 m, +27°)
  is sea-fed too (67.5 m sea water). Over the set, sea water in basins averages 2.8 m of the 24 m,
  and up to 170 m in one snapshot. results.md never mentions sea-fed basins.
- *The ice story is only part of it.* The polar story does hold for some snapshots (seed 0 at
  650–700 Myr, seed 4 at 750 Myr, seed 5 at 500 Myr), and it explains why those basins dry
  slowly. It is one factor, not "the main reason".
- *main-nosea had the same kind of basin.* Seed 7 at 800 Myr held 314 m and seed 5 at 1000 Myr held
  274 m with P ≈ E, so land temperatures did not make such basins dry out either.

Suggested rewrite:
- Most of the water comes from rare, abrupt cut-offs of deep ocean basins at grid resolution.
  The rule-off sets have them with or without lake columns.
- Lake columns slow the drying of polar ones (ice, low evaporation). Some warm ones are kept
  full by sea water over the sill.
- Present the effect of the climate change on storage as not significant with 8 seeds.

**M2. The tropical-lake caveat is wrong where it says "Their balance is negative, so they do not
fill".** Two of the hot cases are refilled from the ocean every step: seed 1 at 850 Myr (36 °C,
E 6.24 m/yr, sea-fed) and seed 2 at 800 Myr (35 °C, sea-fed). The unphysical evaporation therefore
does not just empty a lake. It draws ocean water through the basin at up to 36,600 km³/yr, about
0.1 m of sea level per year, back into the air, and 45 % of the land drains into that basin in that
snapshot.

How severe is it? 6–8 m/yr needs roughly 470–640 W/m² of latent heat, several times the energy
available at the surface. results.md says "more than the energy available", which is right but
understated: this is an energy-conservation violation, not a tuning issue. The water budget stays
closed, so it is still a caveat rather than a blocker. It belongs in the follow-up issue with
these numbers.

### Should

**S1. Present the decision as a choice between three options, with lakes-rule as a real option.**
The pre-registered criteria are met as written:
- (a) holds;
- (b) holds by the paired diagnostic;
- (c) is within the scatter against main.

But the criteria do not limit storage in cut-off basins, which is exactly what the rule targeted.
The plan only promised to report it. lakes-rule keeps most of the climate gain at the old
storage:
- large-lake range 26.9 vs 40.4 K as land (`lakes`: 20.4 vs 43.0 K);
- land next to large lakes 30.8 vs 39.6 K;
- lake water 2.0 m, endorheic land 6.4 %.

Sebastian's choice is therefore:
1. Rule off: rare 100–400 m ocean-basin cut-offs, some sea-fed and overheated.
2. Rule on with lake columns.
3. A sub-grid strait (#168).

The heading "verdict: dropped" and the phrase "by design, not a defect" pre-empt that choice.
Basins of 1600–3800 cells with oceanic floors are most plausibly a coarse-grid artefact, a strait
narrower than a cell, which results.md itself says. That should be stated plainly rather than
calling it physical.

**S2. Statistical caveats are missing.**
- With 8 chaotic seeds and wide scatter, criterion (c) "within the seed scatter" has little power.
  Arid land has a per-seed SD of about 6 points, so only differences of more than ~8 points could
  be detected. The result shows "no large change", not "no change".
- About 70 metrics are tested, so a few |t| ≈ 2 values arise by chance. Examples are arid land
  (+2.0) and Köppen B (+2.0, not mentioned) against main-nosea.
- Lake water is extremely skewed: 2–5 snapshots make up most of the mean. A mean with a Welch t is
  a poor summary; use per-seed medians, a rank test or the count of events.
- The 11 snapshots per seed are autocorrelated.

**S3. The reason given for the drier land against main-nosea is not supported in aggregate.**
results.md says the old lakes "were a hot-summer moisture source". But the total lake evaporation
into the air is *higher* in `lakes` than in main-nosea (8,550 vs 6,740 km³/yr), because there is
more lake area. Only the local precipitation anomaly next to large lakes supports it (+0.28 vs
+0.09 m/yr, t −2.3). Land precipitation also varies non-monotonically across the sets: main-nosea
is the wet one, and arid land in lakes-rule (17.7 %) equals main. Call it "possibly, partly noise".

**S4. The cut-off events distort land statistics, and this should be said.**
- In single snapshots, lake cells reach 4000 cells: in seed 3, land is 38 % of the surface
  instead of 28 %.
- "Endorheic % of land" includes land draining into giant cut-off or sea-fed seas (26–45 % of land
  in single snapshots). The comparison to Earth's ~18–20 % therefore partly measures this artefact.
- Köppen excludes lakes (checked in `koppen::classes`), and so do the "land without lakes" rows.
  The `% of land` rows from compare.py (arid land, lake shares) include lakes.

**S5. The one-step lag matters more than "seen in one snapshot" suggests.** Each cut-off is a
single abrupt event. The step in which it happens is exactly when the basin holds the most water,
and for the climate the basin is land then. Seed 0 at 600 Myr has a 2317-cell sea at 45 °C in
January and −23 °C in July with no ice, holding 226 m. That step's lake balance also evaporates at
land temperatures. Mention that this is the first step after every cut-off, and say whether it
affects the means (it is one of the 88 snapshots in `lakes`).

**S6. Part of the size of the maritime effect is set by the grid, not only its reach.**
- Land within 1–3 fine cells (≤ 330 km) usually lies in the same 440 km coarse cell as the lake.
  It is coupled to the lake through `k Σ f_j (T_j − T_i)` with a land heat capacity of
  1e7 J/m²/K, so it relaxes to the lake temperature within about two weeks when f_lake is large.
  The 15 K reduction is thus largely the coarse-cell mean.
- This is the same treatment coastal land gets at the ocean, so it is consistent. For seas of
  Caspian to Mediterranean size, a strong maritime effect is plausible.
- Say that both the size and the reach follow the coarse cell.

### Could

- The cases' "ice" column is the share of cells with annual ice > 0.5, not winter ice cover. That
  contradicts "ice in winter" for seed 4 (22 %).
- "Twice as often as land ice sheets": lake ice and ice sheets are different processes (ice sheets
  need accumulation). Comparing with sea ice at the same latitude would be more meaningful.
- The Earth comparison ("large mid-latitude lakes 20–25 K") is set against a mean over all
  latitudes, including tropical lakes with ranges of about 5 K. Comparing mid-latitude large lakes
  only would be fairer.
- The summary table uses |Jul − Jan| and the paired diagnostic uses warmest − coldest month. Water
  lags the season, so |Jul − Jan| slightly understates water ranges. The difference here is small
  (20.6 vs 22.3 K).
- The paired diagnostic runs 20 years from a state that was spun up with lakes. That is enough for
  the land and a 50 m mixed layer, but should be stated.
