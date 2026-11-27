# Results: plume rifts that propagate across the plate (#51, option 3)

_2026-11-27. Plan: [`plan-propagation.md`](plan-propagation.md) (revised after
[`review-plan-propagation.md`](review-plan-propagation.md)). Independent evaluation:
[`review-results-propagation.md`](review-results-propagation.md). Goal: an Earth-like planet,
not Earth; effects of the ~110 km grid are accepted (#64)._

## Short answer

**The propagation model works as designed, but with literature values it gives almost no plume
rifts:** 2 in 8 seeds × 1000 Myr (0.03 per 100 Myr; main has 2.2 by its rule), none from a
supercontinent start. Plates and the largest plate stay where the PR was before (20 plates vs
main's 30; largest plate 35% vs 17%, 500–1000 Myr). The success threshold fixed in the plan
(≥ 1 plume rift per 100 Myr) is missed by a factor of ~30.

The limit is not the propagation speed: a magma-fed rift along a whole section through a plume
can be opened by the plate's loads (`S_rift ≥ 1`) in only ~4.5% of the analyses of plates with a
plume, and rarely for long enough to cross the plate (40–50 Myr at 5 cm/yr for ~2000 km). Even at
the fastest published propagation (14 cm/yr) there are only 0.3 rifts per 100 Myr; limiting
the magma to 2000 km from the plume changes nothing. The reason: the plume weakens only the
*opening* strength, and the loads of real plates fail sections by mixed mechanisms (opening with
sliding and bending), so a magma factor of 0.1 raises the overload of a plate-wide section only
~2–3× (unit test: 2.3×), not 10×.

## What changed (on top of the PR)

* `breakup.rs`: for each active plume above a plate, the sections through it (two arms, as in
  the PR) are also analysed **as a magma-fed rift**: the strength in opening along the whole
  section is lowered by the plume's magma factor (0.1 at full supply), sliding and closing keep
  their strength. If the best such section has `S_rift ≥ 1`, the plate's rift timer runs; the
  plate splits along it (cause "plume rift") once the timer reaches the time the rift needs to
  run from the edge of the plume's magma footprint to the far end of the section at
  `breakup.propagation`, and at least #182's 5 Myr. Below 1 the timer runs back as fast (dykes
  freeze). The timer follows the rift's plume through splits (the part above the plume keeps it)
  and merges; a rift from another plume starts anew. Refined along the weakest lithosphere like
  the other sections.
* **One new parameter, `breakup.propagation` = 50 km/Myr (5 cm/yr)**, the log-middle of published
  rift propagation rates (~1.5 cm/yr in 3D models, a few cm/yr for spreading in the Gulf of Aden,
  ~10 cm/yr South Atlantic, ~14 cm/yr Woodlark). Not tuned. `breakup.rift_reach` (default 0 =
  whole rift) exists only for the sensitivity run.
* Statistics: plume rifts (by the plume's age when the rift started), `S_rift` distribution,
  plates with a propagating rift; `plate_sections.csv` gets `rift_overload`, `rift_timer`,
  `rift_reach_km`. `births()` counts plume rifts.
* Main merged into the branch (#191 step split, epoch-0 barren default, lake ice, …); the frame
  history format is now version 13 (the PR's fabric channel and main's lake-ice channels both took
  12). `mantle.rs` is not touched (PR #211 edits its slab part).

Checks: `--set breakup.propagation=0` gives the PR head's state hashes exactly (seed 3, 0/40/80
Myr); two runs give the same hashes, also with `--web-math`; local CI clean.

## Runs

* **main**: current main (96ed0f8) + the PR's diagnostics commit (674bf4b; no state change).
* **before**: the PR head merged with main (e844fdf), i.e. option 1.
* **now**: this model (c320fea).
* 8 seeds × 1000 Myr, `--duration 1000 --snapshot-interval 10` (main's defaults).
  Sensitivity (4 seeds): `propagation` 15 and 140 km/Myr, `rift_reach` 2000 km,
  `magma_weakening` 0.05 (the ridge-axis floor; see Decision). Supercontinent: 4 seeds × 500 Myr,
  `--initial supercontinent`, all three arms. Pre-check: 2 seeds × 400 Myr with
  `--plate-log`.
* Means over snapshots in the window, then over seeds [range], Welch t against main. Script:
  [`scripts/compare.py`](scripts/compare.py).

### 500–1000 Myr

| | main | before | now | v = 15 | v = 140 | reach 2000 km |
|---|---|---|---|---|---|---|
| seeds | 8 | 8 | 8 | 4 | 4 | 4 |
| plume rifts /100 Myr | 2.20 (rule) | 0 | 0.03 | 0.05 | 0.30 | 0.05 |
| `S_rift ≥ 1` (% of analyses) | | | 4.5 | 6.3 | 5.9 | 1.9 |
| `S_rift` 0.5–1 (%) | | | 9.7 | 8.6 | 8.8 | 3.9 |
| all splits /100 Myr | 11.3 | 7.1 (t −4.4) | 6.4 (t −5.5) | 7.5 | 7.1 | 6.5 |
| births /100 Myr | 11.9 | 7.4 | 6.6 (t −5.6) | 7.7 | 7.6 | 6.7 |
| splits ≤ 500 km of a plume / of an antipode | 2.8 / 1.0 | 0.65 / 0.50 | 0.57 / 0.38 | 0.85 / 0.40 | 1.15 / 0.50 | 0.60 / 0.30 |
| plates | 29.9 [26.5–32.6] | 20.4 [12.1–26.6] | 19.6 [12.1–26.6] (t −5.7) | 21.0 | 20.9 | 19.4 |
| largest plate (%) | 17.3 [13.1–19.9] | 32.8 [17.6–47.1] | 34.8 [17.6–47.1] (t +4.8) | 32.0 | 27.0 | 35.1 |
| largest plate, max in window (%) | 24.6 [16.7–39.2] | 43.0 [22.8–58.7] | 44.6 [22.8–58.7] | 41.8 | 38.4 | 44.0 |
| continental crust (% of surface) | 33.0 | 34.2 | 34.4 | 34.0 | 33.5 | 34.4 |
| land (%) | 27.1 | 28.0 | 28.4 | 29.1 | 29.7 | 29.6 |
| mean land elevation (m) | 572 | 394 | 366 (t −3.4) | 431 | 587 | 374 |
| flooded continental crust (%) | 30.1 | 31.8 | 31.1 | 27.5 | 23.3 | 27.1 |
| deforming area (%) | 7.1 | 4.8 | 4.5 | 5.0 | 6.6 | 4.3 |

0–500 Myr (main → before → now): plates 25.1 → 21.3 → 20.9, largest plate 18.2 → 23.9 → 24.0%,
births 12.1 → 9.0 → 9.0 per 100 Myr, plume rifts 0 → 0 → 0.03 per 100 Myr.

**The plume rifts that happened** (all arms): `now` 2 (seeds 0 and 1, both from plumes in their
head stage, < 20 Myr old at onset), v = 15: 2, v = 140: 8 (3 from heads, 5 from tails 20 to
> 100 Myr old), reach 2000: 2. With so few, the effect on plate counts is within seed scatter;
`now` and `before` are identical on 6 of 8 seeds.

**Pre-check** (2 seeds × 400 Myr, plate log): `S_rift ≥ 1` in 4.7% / 4.3% of the analyses; the
longest propagating rift ran 38 Myr (needed 46 Myr for its 2300 km) and then the plate's loads
dropped below the rift's strength. Letting stalled rifts keep their length instead of retreating
would not have changed this materially (offline replay: 1 more rift in 800 Myr).

**Supercontinent** (`--initial supercontinent`, 4 seeds × 500 Myr): `now` is identical to
`before` on all 4 seeds (no plume rift). The largest continental mass stays at 91–100% in both;
main splits off a large piece on 2 of 4 seeds (down to 70–71%). Largest plate at 500 Myr
(seeds 0–3): main 23/17/18/14%, before = now 52/27/56/22%.

**#54 fabric** (500–1000 Myr, now vs before): mean fabric on continental crust 0.07 vs 0.08;
continental cells with fabric ≥ 0.25: 3.8% near margins, 6.1% deep inside continents (before
3.9%, 6.5%); new boundaries run on fabric 24% vs 9% of the split plates' continental cells
(before 25% vs 9%). Same as the PR before, as expected with 2 extra splits.

**Runtime:** paired local runs, seed 0, 200 Myr, one core: before 205.5 s, now 203.4 s (the runs
diverge after a split); the section analysis takes 12.0 → 14.2 s (+1% of the run). GitHub
seed runs: before 764–964 s, now 895–975 s (different runners).

## Why so few: the physics, not a bug

* The rift criterion is the energy balance of opening the whole section as a magma-fed rift. The
  plume lowers only the opening strength; most overloaded sections of real plates fail with
  sliding and bending components, which the magma does not help, so the overload grows only
  ~2–3× (opening share of the mechanism ~0.6), and 0.1 × #182's strength (5 × 10¹³ N/m) is
  5 × 10¹² N/m, above Buck's ~10¹² N/m for magma-assisted rifting.
* The loads on plates with plumes are modest: swell push ~2 × 10¹² N/m over the head, and drag
  of the continents (×4) resists. `S_rift ≥ 1` episodes last a few Myr to a few tens of Myr,
  shorter than the crossing time at any published speed.
* The plume-rift rule on main rifted every qualifying plume after 20 Myr regardless of the
  loads; with the stress-based model the loads must be there, and on this planet they rarely are.

## Decision

The plan's threshold is missed. Options for Sebastian (none implemented as default):

1. **Keep option 3 as it is** (a physical model, rarely active): plates ~20, largest plate ~35%,
   up to 59% on some seeds.
2. **Magma strength in absolute terms (sensitivity run below):** Buck's argument gives an
   absolute force for magma-assisted rifting (~10¹² N/m), not a ratio; against #182's
   5 × 10¹³ N/m that is a factor ~0.02, floored at the ridge axis (0.05). `magma_weakening=0.05`
   changes an existing PR parameter, so it is a decision, not tuning.
3. **Plume rule back as well** (option 2 of the PR, revert 42f8aab; untested in combination).
4. **Lower #182's strength** (2 × 10¹³ N/m raised the plate count by two thirds on #123): a
   global change outside #51.
