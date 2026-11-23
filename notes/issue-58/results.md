# #58 results: closed depressions below sea level are land, lakes or seas

_2026-11-23. Plan: [`plan.md`](plan.md), plan review: [`review-plan.md`](review-plan.md),
results evaluation: [`review-results.md`](review-results.md). Final measurement: `main` at
a253fca (after #57 and #91 landed) against the branch (`b7`, b24b9e9), 8 seeds (0–7) ×
1000 Myr, default parameters, `--material-budget --cell-dump --width 512`. Values are the mean
over seeds of the mean over the snapshots of 600–1000 Myr, seed range in brackets; counters per
100 Myr over 500–1000 Myr. t is the Welch t against `main` (n = 8; |t| ≳ 2.4 ≈ p < 0.05; with
~80 metrics, 3–4 pass that by chance). Full tables: [`compare-main-b7.txt`](compare-main-b7.txt)
(also `b6`, the same rules before the last speed-up), earlier versions against the old `main`:
[`compare-before-57.txt`](compare-before-57.txt). Scripts: [`scripts/`](scripts/)._

## What changed

1. **The ocean** (`src/sea.rs`) is the water below sea level connected, through cells sharing
   an edge, to the largest body of water below sea level, or to another closed body whose basin,
   filled to its pass towards the ocean, covers at least 0.2% of the surface
   (`TectonicParams::min_sea_fraction`, `--set min_sea_fraction=…`). A sill on which the sea
   level stands connects the basin behind it. Everything else below sea level is land: a dry
   basin or a lake.
2. **Sea-level solve.** Each cell has a pass level (the lowest sea level at which the ocean
   reaches it, from a priority flood from the seeds) and joins the ocean when the sea level
   rises above it. The solve runs along the flood (cells leave it in the order of their passes),
   without a sort. A basin behind a pass joins at once; when the volume does not suffice to fill
   it, the sea level stands on the pass and the rest of the volume (the deficit) is the water
   that spilled over the sill into the basin, which then counts as ocean.
3. **Erosion.** The drainage floods from the ocean only; closed depressions below sea level go
   through the lake treatment of #44 (the lake rises until its evaporation balances its inflow,
   can be dry, traps sediment up to its pass). Rivers cut down to the floor of a dry
   depression, not only to sea level. Waves, the wave base, the coastal shelf target and
   diffusion act on the ocean only (land sediment spreads down into a depression).
4. **Water conservation.** The water of all lakes (their area times depth) is taken from the
   ocean: `Simulation::set_lake_water` after each erosion step, with a new term `lakes` in the
   sea-level budget. The ocean holds the rest exactly; in the deficit case the deficit is in the
   basin behind the sill.
5. **Climate and statistics.** The planet finds the ocean's seeds once per step; the climate and
   temperature models see closed depressions as land at sea level (runoff, evapotranspiration,
   and the potential evaporation that the lake balance needs; the lapse rate already ignored
   elevations below sea level), and the erosion takes the ocean as the water connected to those
   seeds. Land, flooded crust, sea floor, shelves, largest river, reservoirs and the cell dump
   (new column `ocean`) use the ocean mask. New log line `inland: …`.

## How the rules came about (b1 → b7)

| version | rule | closed depressions below sea level (cells) | lake water (m) | deficit (m) | endorheic (% of land) |
|---|---|---|---|---|---|
| old `main` | everything below sea level is sea | — (~450 cells are inland "seas") | — | — | 15.9 |
| `b1` | connected to the largest body only | 844 | 12.9 | 12.5 (≤ 42) | 30.0 |
| `b2` | + a sill the sea stands on connects; lake water from the ocean | 407 | 5.4 | 1.8 | 24.1 |
| `b3` | + bodies ≥ 0.2% of the surface below sea level are seas | 136 | 3.1 | 0.02 | 18.0 |
| `b4` | size by the basin at its *lowest* pass | 475 | 7.3 | 6.4 (≤ 43) | 27.5 |
| `b5` | size by the basin at its pass *towards the ocean* | 96 | 2.6 | 0.00 | 16.5 |
| `b7` vs new `main` (after #57) | `b5` + seas found once per step | 99 | 3.4 | 0.02 | 19.5 (main 12.9) |

- **Sills (`b1` → `b2`).** In `b1` the sea level often stood on a pass into a basin it could
  not fill, and the deficit (water in no reservoir) averaged 12.5 m of sea level. Physically the
  sea spills over such a sill and feeds the basin from the ocean (the Atlantic and the
  Mediterranean); so the basin counts as ocean, and the deficit is the water in it.
- **Large closed bodies are seas (`b2` → `b3`).** In `b1`/`b2` closed ocean remnants of up to
  2800 cells (7% of the surface; 65–70% oceanic crust, 0.9–2.2 km deep, passes 70–230 m above
  sea level) were land for 100–200 Myr. The climate sees them as land without ocean
  evaporation, so they became deserts (0.04–0.55 m/yr), mostly dry, with endorheic catchments
  (endorheic land up to 61% in seed 6). Two model limits make this extreme: lakes are no
  moisture source in the climate, and a dry basin is never reflooded over its sill. A closed sea
  drying out is physical (the Mediterranean in the Messinian, 5.96–5.33 Ma), but not for
  200 Myr next to the ocean. On Earth every water body of 10⁶ km² below sea level is connected to the
  ocean, mostly by straits far narrower than a cell (Mediterranean, Black Sea, Red Sea, Baltic:
  1–40 km against ~110 km cells); the largest closed one, the Caspian Sea (3.7 × 10⁵ km²,
  ~30 cells), has a third of that. So bodies above 0.2% of the surface (≈ 10⁶ km², ~80 cells)
  are treated as seas connected by sub-grid straits: a grid artefact (#64) made explicit.
- **The size must not depend on the sea level (`b3` → `b5`).** The results evaluation found
  that `b3` measured a body's size below the sea level, the previous one in the solve and the
  new one in the masks: a body near the threshold could be sea in the solve and a lake in the
  erosion, and flip back. `b4` used the basin filled to its lowest pass, which is sea-level
  independent but stops at sills inside large remnants (so they became lakes) and flips when
  sediment fills a sub-basin (the fluvial and coastal budget terms swung by up to 45 m per
  step). `b5` uses the basin filled to its pass towards the ocean: independent of the sea level
  and of inner sills; tested at two sea levels and with an inner sill.
- **Speed (`b5` → `b7`).** The extra flood of the size rule made the branch ~15% slower.
  Finding the seas once per step for the climate and the erosion (the erosion's own solves
  flood from those seeds and stop at the level) brings it to +9% (paired, local).

## Targets and inland water (`main` a253fca → `b7`)

| Metric | main | b7 (branch) | t |
|---|---|---|---|
| River mouths on inland water bodies | 21.4 bodies (17.4 of 1–3 cells), 1.1% of the surface, are sea | **0**: closed depressions are lakes | |
| Terminal fans and deltas (km·cells /100 Myr) | 10101 (5027…15250) | 8654 (4461…12447) | −1.1 |
| Sediment trapped in lakes (km³/yr) | 0.24 (0.15…0.33) | 0.55 (0.23…0.82) | +3.7 |
| Sediment to the ocean (km³/yr) | 4.73 (3.66…5.38) | 4.30 (2.26…5.08) | −1.1 |
| Closed depressions below sea level (cells) | — (as sea: ~450) | 99 (66…151) | |
| … of them under lake water (cells) | — | 68 (46…101) | |
| Depressions reaching below sea level, of them dry | — | 44.1, 8.8 dry (3.8…16) | |
| Lakes (% of land) | 2.63 (2.22…2.94) | 3.56 (2.62…4.88) | +3.3 |
| Endorheic (% of land) | 12.9 (9.2…17.4) | 19.5 (14.1…28.5) | +3.2 |
| Endorheic into single-cell pits (% of land) | 6.58 | 8.49 | +2.2 |
| Lake evaporation (km³/yr) | 2642 | 3568 | +3.2 |
| Lake water (m of sea level) | — (not counted) | 3.37 (0.35…5.57) | |
| Sea level on a sill: deficit (m of sea level) | — | 0.02 (0.00…0.08) | |
| Sea level (m, dry offset) | −126 (−199…4) | −153 (−241…−63) | −0.9 |
| Ocean (% of surface) | (below sea level) | 71.2 (below sea level 71.4) | |
| Land (% of surface) | 28.3 | 28.8 | +0.6 |
| Mean land (m) | 764 (501…989) | 740 (460…938) | −0.3 |
| Highest point (m) | 4891 | 4687 | −0.9 |
| Runtime, paired local, seeds 2–5 × 100 Myr (s, user CPU) | 45.2 | 49.3 | **+9%** |

- **The refused sediment.** No river ends in an inland water body any more, so none of the
  sediment that finds no room comes from there. The total of terminal fans and deltas fell by
  14% (not significant; the seeds spread by a factor of 3). The 39–47% of the issue were
  measured on the overfill of the routing before the terminal fans (batch 1, seeds 0–1); most
  of what is left is refused on full shelf seas. What rivers used to dump into inland seas now
  fills lakes: sediment trapped in lakes rose from 0.24 to 0.55 km³/yr.
- **Lakes and endorheic land.** Lakes cover 3.6% of land against 2.6%; Earth's lakes cover ~2%
  (with the Caspian), so this is high but plausible for a planet with more closed basins.
  Endorheic land rose from 12.9 to 19.5% of land (t +3.2; `b6` 16.8%, t +1.9), which is
  Earth's ~18–20%: catchments of closed depressions below sea level that used to drain into
  "seas". A fifth of the depressions that reach below sea level are dry (Qattara, Turpan,
  Death Valley type), the rest hold lakes.
- **Lake water** 3.4 m of sea level (0.4–5.6); Earth's lakes hold ~0.5 m (the Caspian 0.2 m).
  It is now taken from the ocean, so the planet's water is conserved.
- **Runtime** on GitHub runners varies by ±15% between batches (here `main` 788 s, `b7` 646 s);
  the paired local timing is the reliable one: +9%.

## Depressions by their pass (`b7`, per snapshot: bodies / cells)

| size | pass 0–10 m | 10–50 | 50–100 | 100–200 | 200–500 | > 500 m |
|---|---|---|---|---|---|---|
| 1–3 cells | 4.8 / 6 | 3.2 / 4 | 0.9 / 1 | 1.1 / 2 | 2.1 / 3 | 15.8 / 21 |
| 4–49 cells | 0.5 / 5 | 0.6 / 8 | 0.1 / 3 | 0.3 / 3 | 0.4 / 5 | 1.2 / 7 |
| ≥ 50 cells | 0.0 / 1 | 0.0 / 2 | 0.0 / 2 | 0 | 0 | 0 |

Most closed depressions below sea level are single cells more than 500 m below their rim, even
after #57: deep single-cell pits (endorheic into single-cell pits 6.6% of land on `main`, 8.5%
here). They are a separate artefact, now drained into lakes instead of "seas".

## Hysteresis and flicker

No hysteresis: the classification follows from the topography and the volume at each solve, the
volume is monotone in the sea level, and the size rule does not depend on the sea level. The
area of closed depressions changes by ~2000 cells per 100 Myr, ~20 cells per step against ~100
present: depressions come and go every few steps. Most are the single-cell pits above, which
appear and vanish with the resampling of plates, not basins flipping between sea and lake; a
flip moves the sea level by that basin's volume over the ocean area (≤ 0.1 m for these). A body
whose basin crosses the size threshold (its pass rises or falls) switches between sea and lake;
with the pass-based size that happens only when the pass itself changes and was not visible in
the budget of `b5`–`b7`.

## Side effects and guard rails (`main` → `b7`; `b6` for comparison)

Significant:
- Lakes, lake evaporation, lake sediment, depressions, endorheic land, as above (the point of
  the change).
- **Sea-level budget terms move, the sea level does not.** Coastal 208 → 170 m per 100 Myr
  (t −2.8; `b6` −2.5), tectonics −226 → −178 (t +2.0), sharing −6.6 → −12.4 (t −3.1; `b6` −4.4,
  t +3.3, so not robust), new lakes term −1.2. The terms are differences of consecutive solves;
  basins joining or leaving the ocean are booked where the solve sees them, and the coastal
  processes (delta plains, diffusion, waves) no longer work in inland "seas". The total
  (−2.8 → +2.4 m per 100 Myr, t +0.6), the sea level (t −0.9) and the residual (0.0000 m) are
  unchanged. Against the old `main` (before #57), `b5` moved the mantle term (−73 → −52,
  t +2.8) and coastal (t −3.3); against the new `main` the mantle term is not significant
  (t +0.8).
- Endorheic into single-cell pits 6.6 → 8.5% (t +2.2), largest river +2.3, burial
  metamorphism on continental crust +2.3, convergence at mature trenches +2.4: not in `b6`
  (|t| ≤ 0.9); likely chance at this number of metrics.
- Sediment part of the sea level 237 → 226 m (t −1.6; `b6` 213 m, t −4.2): less sediment
  reaches the sea (it stays in lakes), so it displaces less water. Direction consistent in all
  versions; size within the seed spread.

Not significant: land, mean land, highest point, flooded continental crust, shelf area and
sediment, land below 10 m, felsic + sediment net, trench sediment, runoff, temperatures, frozen
area, sea-floor age, ≥ 200 Myr sea floor, rifts, plates.

## Interaction with #57

#57 landed first (duplicated samples get the mean of their cells' surface changes). The branch
is merged with it and re-measured against the new `main` (above); the results before #57 are in
[`compare-before-57.txt`](compare-before-57.txt). The single-cell pits behind passes > 500 m
are still there after #57 (15.8 per snapshot).

## Caveats

- **Grid.** Straits narrower than a cell are not resolved: a closed body smaller than 0.2% of
  the surface behind a land cell is a lake even where a real coast would have a narrow strait,
  and a larger one is a sea even where it would be closed. The 0.2% follows Earth's sample of
  basins, not physics.
- **Lakes are no moisture source in the climate** (as before): a large lake does not moisten
  its region. This is what made the large closed basins of `b1`/`b2` deserts.
- **Water loading of dry depressions.** Surface elevations below sea level carry the water load
  (×1.45) everywhere, so a dry depression is drawn up to 31% deeper than without it, and lake
  volumes are taken from these depths without the load; the two errors partly compensate. Not
  changed (it would need the mask inside the isostasy of every cell).
- **A basin the sea spills into** is ocean in the mask and shown filled to the sill, while it
  holds only the deficit (0.02 m of sea level on average).
- **Steady-state lakes (#44):** when a basin is cut off, its sea water returns to the ocean at
  once and the lake balance sets its level in the same step; lake water of a basin the sea has
  just flooded is still taken from the ocean for one step.
- **Render and 3D view** still colour every cell below sea level as sea (a dry depression looks
  like a small sea in the PNGs); a follow-up if wanted.

## Decisions for Sebastian

1. **The size rule** (`min_sea_fraction`, default 0.2% ≈ 10⁶ km²): keep, change, or drop it
   (`--set min_sea_fraction=1`, as `b2`: large closed ocean remnants become mostly dry and
   endorheic land rises to ~24%). Dropping it would call for lakes as moisture sources in the
   climate first.
2. Whether the render/3D view should draw closed depressions as land or lakes.
