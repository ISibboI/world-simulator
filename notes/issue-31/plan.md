**Plan** (reviewed by an independent agent)

The mark reaches oceanic crust in two ways:
1. **It is set by the column test.** Collision thickening (`boundaries.rs`) and `mark_suture` (`events.rs`) test `is_continental_origin_column()`, which counts metasediment. A metamorphosed accretionary wedge on oceanic basement passes that test and is marked, but its lithosphere is oceanic. The cell dump and `weak_zones` judge by the basement. In two existing 1000 Myr dumps from `main` + #23, about half of the marked oceanic cells have a column felsic fraction above 0.5.
2. **Mixing takes the maximum.** `spread_crust` (merges) and `heal_gap` (gap fills) give the mixed sample the latest mark of any contributor, even when the mixture is oceanic.

Changes:
- **Set the mark only on continental lithosphere:** `is_continental_origin()`, judged on the basement, at both setting sites.
- **New `Crust::mix_orogeny`:** after mixing, a sample keeps the latest mark only if it is still continental lithosphere. Otherwise the mark is cleared: oceanic or transitional crust starts unweakened.
- **Where it applies:** `spread_crust`, `heal_gap` and `stretch_rift`. In `stretch_rift` this changes nothing for real continental donors.
- **Unit tests** for the setting test and for both mixing paths.
- **Validation:** 4 seeds × 1000 Myr, `main` against the branch, with `--cell-dump`:
  - marked share of oceanic, transitional and continental cells;
  - rift boundaries on sutures;
  - orogenic share of continental crust;
  - guard rails.

Out of scope: the `max` rule still marks a whole continental sample even when only a small share of the marked material is mixed into it. That spreads marks inside continents and will be listed as a follow-up in the PR.

## Independent plan review (summary)
Verdict: approve with changes. Main points, all adopted:
- The mark is set with the column test (`is_continental_origin_column`, metasediment included) but read with the basement test (cell dump, rift statistics). A metamorphosed accretionary wedge on oceanic basement can therefore be marked directly, without any mixing. Erosion (`thin_from_top`) then removes the metasediment first and leaves marked bare oceanic crust.
  - Check on two existing `main` + #23 dumps at 1000 Myr: 58 and 47 marked oceanic cells, of which 44 and 35 carry metasediment, and 30 and 21 have a column felsic fraction above 0.5.
  - Change: the setting sites and `mix_orogeny` use `is_continental_origin()`, judged on the basement.
- Apply `mix_orogeny` in `stretch_rift` as well, for consistency.
- A marked cell that later falls below the continental test, by thinning from below or by crustal flow, keeps its mark. It stays felsic, so it becomes transitional, never oceanic.
- Follow-up, out of scope: the `max` rule marks a whole continental receiver even when only a small marked share is mixed into it.
