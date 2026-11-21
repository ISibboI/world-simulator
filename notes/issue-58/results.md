# #58 results: closed depressions below sea level are land, lakes or seas

_2026-11-22. Plan: [`plan.md`](plan.md), plan review: [`review-plan.md`](review-plan.md),
results evaluation: [`review-results.md`](review-results.md). `main` at 67ca0a2 (the run set of
the #57 agent, same commit and arguments) against the branch, 8 seeds (0–7) × 1000 Myr,
default parameters, `--material-budget --cell-dump --width 512`. Values are the mean over seeds
of the mean over the snapshots of 600–1000 Myr, seed range in brackets; counters per 100 Myr
over 500–1000 Myr. t is the Welch t against `main` (n = 8; |t| ≳ 2.4 ≈ p < 0.05; with ~70
metrics, 3–4 of them pass that by chance)._

Three versions of the branch were run:

- `b1`: connectivity as planned (ocean = water connected to the deepest cell of the largest
  body below sea level; everything else is land, dry or a lake).
- `b2`: `b1` plus: a sill on which the sea level stands connects the basin behind it (see
  below), lake water taken from the ocean volume, and the faster solve.
- `b3` (the branch): `b2` plus closed water bodies of ≥ 0.2% of the surface count as seas.

## What changed

1. **The ocean** (`src/sea.rs`) is the water below sea level connected, through cells that
   share an edge, to the largest body of water below sea level, or to another body of at least
   0.2% of the surface (`TectonicParams::min_sea_fraction`, `--set min_sea_fraction=…`).
   Everything else below sea level is land: a dry basin or a lake.
2. **Sea-level solve.** Each cell has a pass level (the lowest sea level at which the ocean
   reaches it, by a priority flood from the seeds); it joins the ocean when the sea level
   rises above it. The solve runs while flooding (cells leave the flood in the order of their
   passes) and stops at the level, so it needs no sort. A basin behind a pass joins at once;
   when the volume does not suffice to fill it, the sea level stands on the pass and the solve
   reports the rest as a deficit (the water that spills over the sill into the basin).
3. **Erosion.** The drainage floods from the ocean only; closed depressions below sea level go
   through the lake treatment of #44 (the lake rises until its evaporation balances its inflow,
   can be dry, traps sediment up to its pass). Rivers cut down to the floor of a dry
   depression, not only to sea level. Waves, the wave base, the coastal shelf target and
   diffusion act on the ocean only (land sediment spreads down into a depression).
4. **Water conservation.** The water of all lakes (by their area and depth) is taken from the
   ocean: `Simulation::set_lake_water` after each erosion step, a new term `lakes` in the
   sea-level budget. The ocean holds the rest exactly, except for the deficit of a sea level
   standing on a sill (then in the basin behind it).
5. **Climate and statistics.** The climate and temperature models see closed depressions as
   land at sea level (runoff, evapotranspiration and the potential evaporation that the lake
   balance needs; the lapse rate already ignored elevations below sea level). Land, flooded
   crust, sea floor, shelves, largest river, reservoirs and the cell dump (new column `ocean`)
   use the ocean mask. New log line `inland: …`.
6. Clippy on `main` fails since #78 and #87 landed together (`std::collections::HashMap` in
   `viewer/pace.rs`); fixed here in a separate commit (the #57 branch has the same fix).

## Why the two rules after `b1`

**Sills on which the sea level stands.** In `b1` the sea level stood on a pass into a basin it
could not fill in many snapshots, and the deficit (water in no reservoir) averaged 12.5 m of
sea level (up to 42 m). Physically the sea spills over such a sill and keeps the basin filled
from the ocean, like the Atlantic the Mediterranean; so the basin counts as ocean now, holding
the deficit, which is then conserved water in the basin. `b2`/`b3`: deficit 1.8 m / 0.02 m.

**Large closed bodies are seas.** In `b1`/`b2` closed ocean remnants of up to 2800 cells (7% of
the surface, 65–70% oceanic crust, mean depth 0.9–2.2 km, passes 70–230 m above sea level)
were land for 100–200 Myr. The climate sees them as land without ocean evaporation, so they
became deserts (0.04–0.55 m/yr), mostly dry, and their catchments endorheic: endorheic land
16 → 24% of land (seed 6: up to 61%). Two model limits make this extreme: lakes are no
moisture source in the climate, and no reflooding of a dry basin over its sill exists. On
Earth every water body of that size below sea level is connected to the ocean, mostly by
straits far narrower than a cell (Mediterranean, Black Sea, Red Sea, Baltic: 1–40 km against
~110 km cells); the largest closed one, the Caspian Sea (3.7 × 10⁵ km², ~30 cells), has a
third of the threshold (10⁶ km², ~80 cells). So bodies above the threshold are treated as seas
connected by sub-grid straits; this is a grid artefact (#64) made explicit, and the threshold is
a decision for Sebastian (see the end).

## Targets and inland water

| Metric | main | b3 (branch) | t | b2 (no size rule) | t |
|---|---|---|---|---|---|
| River mouths on inland water bodies | 21.7 bodies of 1–3 cells, 25.7 in all (1.1% of the surface) are sea | **0**: closed depressions are lakes | | 0 | |
| Terminal fans and deltas (km·cells /100 Myr) | 10990 (5196…16549) | 9252 (3431…14846) | −0.9 | 8206 (2663…11038) | −1.6 |
| Sediment trapped in lakes (km³/yr) | 0.32 (0.18…0.44) | 0.79 (0.26…1.13) | +4.2 | 0.89 | +4.0 |
| Sediment to the ocean (km³/yr) | 4.67 (3.82…5.58) | 4.61 (3.36…5.94) | −0.2 | 4.04 | −1.4 |
| Closed depressions below sea level (cells) | — (as sea: ~450) | 136 (99…193) | | 407 | |
| … of them under lake water (cells) | — | 84 (59…118) | | 182 | |
| Depressions reaching below sea level, of them dry | — | 47.8, 11.9 dry (3…22) | | 44.3, 9.8 dry | |
| Lakes (% of land) | 2.92 (1.88…3.48) | 4.13 (2.64…5.00) | +3.7 | 4.21 | +3.3 |
| Endorheic (% of land) | 15.9 (13.4…21.2) | 18.0 (13.1…28.2) | +1.1 | 24.1 (13.5…35.7) | +3.1 |
| Endorheic into single-cell pits (% of land) | 7.16 | 6.60 | −0.7 | 8.00 | +1.0 |
| Lake evaporation (km³/yr) | 3033 | 3938 | +2.8 | 4640 | +3.0 |
| Lake water (m of sea level) | — (not counted) | 3.11 (1.31…5.10) | | 5.44 | |
| Sea level on a sill: deficit (m of sea level) | — | 0.02 (0.00…0.05) | | 1.82 (0…6.3) | |
| Sea level (m, dry offset) | −169 (−242…−93) | −139 (−212…−49) | +1.0 | −135 | +1.6 |
| Ocean (% of surface) | — | 70.5 (below sea level 70.8) | | 70.1 (71.0) | |
| Land (% of surface) | 29.7 | 29.5 | −0.4 | 29.9 | +0.3 |
| Mean land (m) | 824 (675…1023) | 782 (527…1029) | −0.5 | 700 | −1.9 |
| Highest point (m) | 5027 | 5000 | −0.3 | 4980 | −0.2 |
| Runtime (s, GitHub runners) | 595 (544…655) | 585 (397…723) | −0.2 | 681 | +3.5 |

- **The refused sediment.** By construction no river ends in an inland water body any more,
  so none of the sediment that finds no room comes from there. The total of terminal fans and
  deltas fell by 16% (not significant: the seeds spread by a factor of 3–4). The 39–47% of
  the issue were measured on the overfill of the routing before the terminal fans (batch 1),
  with seeds 0–1 only; most of what is left is refused on full shelf seas (batch 1 results).
  What rivers used to dump into inland seas now fills lakes: sediment trapped in lakes rose
  from 0.32 to 0.79 km³/yr; sediment to the ocean did not change.
- **Lakes and endorheic land.** Lakes cover 4.1% of land against 2.9% (the closed depressions
  below sea level hold lakes on 84 cells, 0.7% of land, and their catchments feed them);
  Earth's lakes cover ~2% of land (with the Caspian), so this is high but plausible for a
  planet with more closed basins. Endorheic land 18% (Earth ~18–20%) is not significantly above `main`. A quarter
  of the depressions that reach below sea level are dry (Qattara, Turpan, Death Valley type).
- **Lake water** 3.1 m of sea level (1.3–5.1). Earth's lakes hold ~0.5 m (the Caspian 0.2 m);
  the planet's are larger because closed basins below sea level up to 0.2% of the surface are
  lakes and partly km deep. It is now taken from the ocean, so the planet's water is conserved.
- **Paired runtime** (local, seed 2, 200 Myr, both at once): 127 s `main`, 133 s branch
  (+4.5%; erosion +26 ms per step: three solves per step with flood, and the masks).

## Small depressions by their pass (`b3`, per snapshot, bodies / cells)

| size | pass 0–10 m | 10–50 | 50–100 | 100–200 | 200–500 | > 500 m |
|---|---|---|---|---|---|---|
| 1–3 cells | 5.2 / 6 | 2.9 / 4 | 0.8 / 1 | 1.2 / 1 | 2.7 / 4 | 15.7 / 21 |
| 4–49 cells | 0.5 / 6 | 0.8 / 13 | 0.2 / 4 | 0.2 / 3 | 0.6 / 6 | 1.0 / 6 |
| 50–81 cells | 0.1 / 8 | 0.1 / 6 | 0.1 / 6 | 0.0 / 2 | 0.1 / 9 | 0 |

Most closed depressions below sea level are single cells more than 500 m below their rim:
deep single-cell pits, most of which #57 traces to crust samples shown by two cells (see
"Interaction with #57").

## Hysteresis and flicker

No hysteresis: the classification follows from the topography and the volume at each solve,
and the volume is monotone in the sea level, so one solve has one answer. The area of closed
depressions changes by 2957 cells per 100 Myr (b2: 8041), i.e. ~30 cells per step against 136
present: depressions come and go every few steps. Most are the single-cell pits above, which
appear and vanish with the resampling of plates, not basins flipping between sea and lake; the
sea-level effect of a flip is that basin's volume over the ocean area (≤ 0.1 m for these). The
size rule can flip a body between sea and lake when it crosses the threshold; that was not
seen as a problem in the runs but is not measured separately.

## Side effects and guard rails (`main` → `b3`)

Significant:
- Lakes, lake evaporation and lake sediment, as above (the point of the change).
- **Sea-level budget: mantle term −73 → −44 m per 100 Myr (t +4.0; b1 +4.4, b2 +2.5), sharing
  29 → 38 (t +2.7; b2 −0.9), new lakes term +4.5 m per 100 Myr.** The terms are differences of
  consecutive solves; with connectivity a solve can connect or cut off basins, and that change
  is booked in whichever term's solve sees it. The lakes term in particular returns, one step
  later, the lake water of a basin that the sea has just flooded (its water is still taken
  from the ocean in the step it connects), so it drifts positive while lake water stays
  1–5 m. The sea level itself did not change significantly (−169 → −139 m, t +1.0), and the
  budget still closes (unexplained 0.0000 m). Dynamic topography is not the cause: on `main`
  the inland seas sat on average dynamic topography (+46 m against +49 m on land).
- Plates > 1%: 15.4 → 17.9 (t +2.5; b2 +0.6). Tectonics has no path from this change other
  than the sea level; with b2 at +0.6 this is likely one of the chance hits.

Not significant: sediment part of the sea level 230 → 219 m (t −1.4), its slope 5.9 → 11.0 m per
100 Myr (t +0.9), flooded continental crust 22.5 → 24.5%, land below 10 m 2.5 → 2.9% of land
(t +1.8), shelf area 5.1 → 5.6%, felsic + sediment net −1.93 → −1.93 Gt/yr, trench sediment,
burial metamorphism, runoff, temperatures, frozen area, sea-floor age, ≥ 200 Myr sea floor.

## Interaction with #57

#57 (agent/issue-57) gives duplicated crust samples the mean of their cells' surface changes,
which should remove most single-cell pits. That will shrink the 1–3-cell depressions above
(15.7 per snapshot behind passes > 500 m) and their flicker, and change the pit, lake and
endorheic numbers of both branches. Whichever lands second rebases and re-measures briefly.

## Caveats

- **Grid.** Straits narrower than a cell are not resolved: a closed body below 0.2% of the
  surface behind a land cell is a lake even where a real coast would have a narrow strait, and
  a body above it is a sea even where it would be closed. The 0.2% follows Earth's sample of
  basins, not physics.
- **Lakes are no moisture source in the climate** (as before #58): a large lake does not
  moisten its region. This is what made the large closed basins of `b2` deserts.
- **Water loading of dry depressions.** Surface elevations below sea level carry the water load
  (×1.45 below sea level) everywhere; a dry depression would be up to 31% shallower without
  it. Not changed (it would need the mask inside the isostasy of every cell).
- **Render and 3D view** still colour every cell below sea level as sea (a dry depression
  looks like a small sea in the PNGs); a follow-up if wanted.
- **Lake water at the moment a basin connects** is still taken from the ocean for one step
  (see the budget above).

## Decisions for Sebastian

1. **The size rule** (`min_sea_fraction`, default 0.2% ≈ 10⁶ km²): keep it, change it, or
   drop it (`--set min_sea_fraction=1`, which is `b2`: large closed ocean remnants become
   mostly dry, endorheic land 24%). Dropping it would call for lakes as moisture sources in
   the climate first.
2. Whether the render/3D view should draw closed depressions as land or lakes.
