# Results: plume rifts that propagate across the plate (#51, option 3)

_2026-11-27. Plan: [`plan-propagation.md`](plan-propagation.md) (revised after
[`review-plan-propagation.md`](review-plan-propagation.md)). Independent evaluation of a first
version of these results: [`review-results-propagation.md`](review-results-propagation.md); its
corrections and its code finding (the rift's distance was a moving target) are adopted below and
the main arm was rerun. Goal: an Earth-like planet, not Earth; effects of the ~110 km grid are
accepted (#64)._

## Short answer

**The propagation model works as designed, but with literature values it gives almost no plume
rifts:** 4 in 8 seeds × 1000 Myr (0.05 per 100 Myr; main has 2.2–2.4 by its rule), none from a
supercontinent start. Plates and the largest plate stay where the PR was before (500–1000 Myr:
19.5 plates vs main's 29.9; largest plate 34% vs 17%, up to 59% on one seed). The threshold fixed
in the plan (≥ 1 plume rift per 100 Myr) is missed by a factor of ~20.

What limits it: a magma-fed rift along a whole section through a plume can be opened by the
plate's loads (`S_rift ≥ 1`) in only ~2.5–5% of the analyses of plates with a plume, in short
episodes (pre-check: 7 episodes in 800 seed-Myr, 4 of them ≥ 5 Myr). Even at infinite propagation
speed that caps plume rifts at ~0.5 per 100 Myr; at the fastest published speed (14 cm/yr) there
are 0.2 per 100 Myr (6 of 8 on one seed). The plume weakens only the *opening* strength, and the
loads of real plates fail plate-wide sections by mixed mechanisms (opening with sliding and
bending), so magma at 0.1 raises a section's overload by a median factor of 1.8 (plate logs; 10×
only for pure tension). In most episodes the plate's ordinary overload was itself near 1: the
magma-fed rift is viable only on plates that are close to breaking anyway. The deficit in plates
(20 vs 30) therefore comes from the plates' loads against #182's strength, which option 3 does not
change.

## What changed (on top of the PR)

* `breakup.rs`: for each active plume above a plate, the sections through it (two arms, as in
  the PR) are also analysed **as a magma-fed rift**: the strength in opening along the whole
  section is lowered by the plume's magma factor (0.1 at full supply), sliding and closing keep
  their strength. While the best such section has `S_rift ≥ 1`, the plate's rift timer runs; the
  plate splits along it (cause "plume rift") once the timer reaches the time the rift needs to
  run from the edge of the plume's magma footprint to the far end of the section at
  `breakup.propagation` (that distance is set when the rift starts), and at least #182's 5 Myr.
  Below 1 the timer runs back as fast (the dykes freeze). Through splits by other causes and
  merges the timer stays with the part above the rift's plume; a rift from another plume starts
  anew. The rift section is refined along the weakest lithosphere like the other sections.
* **One new parameter, `breakup.propagation` = 50 km/Myr (5 cm/yr)**, the log-middle of published
  rift propagation rates (~1.5 cm/yr in 3D models, a few cm/yr for spreading in the Gulf of Aden,
  ~10 cm/yr for the South Atlantic, ~14 cm/yr for the Woodlark basin). Not tuned.
  `breakup.rift_reach` (default 0 = the whole rift) exists only for the sensitivity run.
* Statistics: plume rifts (by the plume's age when the rift started), the `S_rift` distribution,
  plates with a propagating rift; `plate_sections.csv` gets `rift_overload`, `rift_timer`,
  `rift_reach_km`. `births()` counts plume rifts.
* Main merged into the branch (#191 step split, epoch-0 barren default, lake ice, …). The frame
  history format is now version 13 (the PR's fabric channel and main's lake-ice channels both took
  12). `mantle.rs` is not touched (PR #211 edits its slab part).

Checks: `--set breakup.propagation=0` gives the PR head's state hashes exactly (seed 3, 0/40/80
Myr); two runs give the same hashes, also with `--web-math`; local CI clean.

## Runs

* **main**: current main (96ed0f8) + the PR's diagnostics commit (674bf4b; no state change).
* **before**: the PR head merged with main (e844fdf), i.e. option 1.
* **now**: this model (cc250ec). A first version (c320fea) re-measured the rift's distance on
  every analysis; the sensitivity arms below use it (with fewer rifts than cc250ec, if anything).
* 8 seeds × 1000 Myr, `--duration 1000 --snapshot-interval 10` (main's defaults).
  Sensitivity (4 seeds × 1000 Myr, c320fea): `propagation` 15 and 140 km/Myr, `rift_reach`
  2000 km, `magma_weakening` 0.05. Supercontinent: 4 seeds × 500 Myr, `--initial
  supercontinent`. Pre-check: 2 seeds × 400 Myr with `--plate-log`.
* Means over snapshots in the window, then over seeds [range], Welch t against main. Script:
  [`scripts/compare.py`](scripts/compare.py).

### 500–1000 Myr

| | main | before | now |
|---|---|---|---|
| plume rifts /100 Myr | 2.20 (rule) | 0 | 0 (0.10 in 0–500 Myr) |
| `S_rift ≥ 1` / 0.5–1 (% of analyses) | | | 4.9 / 10.8 |
| all splits /100 Myr | 11.3 | 7.1 (t −4.4) | 6.4 (t −5.3) |
| births /100 Myr | 11.9 | 7.4 | 6.7 (t −5.2) |
| splits ≤ 500 km of a plume / of an antipode | 2.8 / 1.0 | 0.65 / 0.50 | 0.67 / 0.48 |
| plates | 29.9 [26.5–32.6] | 20.4 [12.1–26.6] | 19.5 [12.1–26.6] (t −5.8) |
| largest plate (%) | 17.3 [13.1–19.9] | 32.8 [17.6–47.1] | 34.1 [17.6–47.1] (t +4.2) |
| largest plate, max in window (%) | 24.6 [16.7–39.2] | 43.0 [22.8–58.7] | 42.4 [22.8–58.7] |
| continental crust (% of surface) | 33.0 | 34.2 | 34.0 |
| land (%) | 27.1 | 28.0 | 29.1 |
| mean land elevation (m) | 572 | 394 | 419 (t −1.9) |
| flooded continental crust (%) | 30.1 | 31.8 | 28.5 |
| deforming area (%) | 7.1 | 4.8 | 5.1 |

0–500 Myr (main → before → now): plume rifts 2.4 (rule) → 0 → 0.10 per 100 Myr; plates
25.1 → 21.3 → 21.0; largest plate 18.2 → 23.9 → 23.9%; births 12.1 → 9.0 → 9.2 per 100 Myr.

The 4 plume rifts of `now`: seeds 0 and 1 (plumes in their head stage, < 20 Myr old when the rift
started) and 2 on seed 7 (plume 50–100 Myr old). `now` ends with the same state as `before` on
5 of 8 seeds.

**Sensitivity** (4 seeds × 1000 Myr, first version; plume rifts in the whole run, then plates and
largest plate in 500–1000 Myr):

| | plume rifts (4 seeds) | per 100 Myr | `S_rift ≥ 1` (%) | plates | largest plate (%) |
|---|---|---|---|---|---|
| v = 15 km/Myr | 2 | 0.05 | 6.3 | 21.0 | 32.0 |
| v = 50 (default) | 2 (8 seeds) | 0.03 | 4.5 | 19.6 | 34.8 |
| v = 140 | 8 (6 on seed 1) | 0.20 | 5.9 | 20.9 | 27.0 |
| magma only within 2000 km | 2 | 0.05 | 1.9 | 19.4 | 35.1 |
| `magma_weakening` 0.05 | 3 | 0.08 | 2.4 | 16.6 | 34.8 |

`magma_weakening` 0.05 (Buck's ~10¹² N/m for magma-assisted rifting in absolute terms against
#182's 5 × 10¹³, floored at the ridge axis) gives no more rifts: it also weakens the plates above
plumes in the sheet and in #182, which changes the plates (fewer).

**Pre-check** (2 seeds × 400 Myr, plate log, first version): `S_rift ≥ 1` in 4.7% / 4.3% of the
analyses; the longest rift propagated for 38 Myr while its section's distance grew from 1320 to
2300 km as the plate drifted over the plume (the moving target fixed in cc250ec); with the fix
the same seeds give 2 rifts instead of 1. Letting stalled rifts keep their length instead of
running back would add nothing (replay of the plate logs).

**Supercontinent** (`--initial supercontinent`, 4 seeds × 500 Myr): `now` is identical to
`before` on all 4 seeds (no plume rift). The largest continental mass stays ≥ 91% in both. Main
does not disperse it either: seed 1 held 70–71% at 140–200 Myr and 58% at 210 Myr, seed 2 dipped
to 68–80% at 210–240 Myr, seed 0 had one snapshot at 71%; all re-assemble to 95–99% by 500 Myr.
Largest plate at 500 Myr (seeds 0–3): main 23/17/18/14%, before = now 52/27/56/22%.

**#54 fabric** (500–1000 Myr, now vs before): continental cells with fabric ≥ 0.25: 3.6% near
margins, 7.5% deep inside continents (before 3.9%, 6.5%); new boundaries run on fabric in 25% of
their continental cells vs 9% of the split plates' (before 25% vs 9%). Unchanged, as expected.

**Runtime:** paired local runs, seed 0, 200 Myr, one core (first version): before 205.5 s, now
203.4 s (the runs diverge after a split); the section analysis 12.0 → 14.2 s (+1% of the run).
GitHub seed runs: before 764–964 s, now 802–973 s (different runners).

## Why so few: physics, not a bug

* The rift criterion is the energy balance of opening the whole section as a magma-fed rift.
  Magma lowers only the opening strength; overloaded plate-wide sections fail with sliding and
  bending components, so the overload grows by a median 1.8× (p10 1.1, p90 2.2).
* `S_rift` is usually far below 1 (median 0.15–0.21); `S_rift ≥ 1` episodes are few and mostly on
  plates whose ordinary sections are near 1 too.
* Main's rule rifted every qualifying plume after 20 Myr regardless of the loads; with a
  stress-based criterion the loads must be there, and on this planet they rarely are, against
  #182's 5 × 10¹³ N/m.

## Decision

The plan's threshold is missed; option 3 cannot restore main's plate statistics by itself,
because the deficit comes from the loads against #182's strength. Options for Sebastian (none
changed as default):

1. **Keep option 3 as it is** (recommended by the evaluation): physical, rarely active, +1%
   runtime, the same plates as before (~20, largest ~34%). Open a separate issue for the plate
   count and the largest plate (#182's strength or the loads).
2. **Lower #182's strength** (2 × 10¹³ N/m raised the plate count by two thirds on #123): a global
   change outside #51; option 3 would then also rift more.
3. **Plume rule back as well** (revert 42f8aab): restores main's statistics with a rule that
   ignores the loads; untested in combination.
4. Stronger magma weakening: no effect at 0.05 (above); not recommended.
