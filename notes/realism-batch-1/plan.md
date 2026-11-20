# Batch 1 plan: sediment routing artefacts (#47, #46)

_2026-11-20. Triage: [`../realism-triage/overview.md`](../realism-triage/overview.md), tracking
issue #55. `main` at 3527833._

## Problems and causes

### #47: single-cell spikes on capped sediment columns

`erosion::erode` routes the sediment of each river mouth into the sea: margin cells within reach
(shelf, slope, rise, up to their depth targets and thickness caps), then deep-sea fans
(`fill_fan`), then small enclosed seas and a delta plain one ring wide. **What is left is put on
the river's last land cell** (`seabed.deposit(source_cell, remaining)`, counted as "overfill").

Measured on `main` (8 seeds, 500–1000 Myr): overfill is 4400 km·cells per 100 Myr, i.e.
~0.55 km³/yr or 17% of the sediment the rivers bring to the sea (3.75 km³/yr). A diagnostic run
(seed 0, first ~150 Myr, 9341 piles) shows:

- 77% of the piled sediment arrives in piles of more than 1 km on one cell in one step, 35% in
  piles of more than 5 km. The next tectonic step turns everything below 11 km into crust
  (burial metamorphism), so the cell becomes a mountain of metasediment: every spike more than
  2 km above its neighbours in the baseline sits on the capped column (12.9 km).
- 53% of the leftover comes from mouths whose turbidity currents reach more than 10 deep-sea fan
  cells (hundreds, down to −5500 m): the room is there, but fans are capped at
  1.5 km × exp(−distance / 1000 km).
- 42% comes from mouths that reach no fan cell: narrow basins, where every oceanic cell is
  within 500 km of margin crust and is therefore slope or rise, capped at 8 and 5 km and at
  −2000 and −3500 m.
- 25% comes from mouths on water bodies of at most 3 cells.

So the spikes are not a missing hillslope process. Sediment is refused by the sea although the
sea has room, and is then put in a place where no river would leave it.

### #46: single-cell pits as closed basins

Every depression deeper than 1 m (`LAKE_MIN_DEPTH`) is a lake with a water balance. 1.5% of the
land cells are single-cell pits. Their median depth is 3.7 m, 75% are shallower than 12 m, 94%
shallower than 100 m; half sit on cells that show a duplicated crust sample (resampling), and
fewer than 1% are still pits 100 Myr later. They are noise of the cell elevations, but 5.5% of
the land drains into them as endorheic basins (of 11.3% endorheic in total).

## Physics (revised after the plan review, see [`review-plan.md`](review-plan.md))

1. **Fans aggrade to a graded surface** (`fill_fan`, switch `erosion.graded_fans`). A deep-sea
   fan has no thickness limit; it has a surface gradient. Large fans fall from their head to the
   abyssal plain with 1–5 m/km on average, steeper on the upper fan (Bengal ~1.2, Indus ~2,
   Mississippi ~3.5, Amazon ~5 m/km; Curray et al. 2003; Bouma et al. 1985; Damuth & Flood
   1985).
   - The first pass is unchanged (thin lobes: cap falling with distance).
   - New second pass for what is left: the cells on the same downhill paths, nearest first,
     fill up to the concave surface `slope_depth − fan_relief × (1 − exp(−distance /
     fan_length))`, without a thickness cap. `fan_relief` is 3000 m: ~3 m/km at the head,
     below 1 m/km beyond 1500 km (review finding 10). The head is at the slope depth
     (−2000 m; the Bengal fan's is at ~−1400 m).
   - Slope and rise cells on the path take part: a narrow basin fills like a fan. Trench cells
     on the path take part too (trench fill is real: Makran, Cascadia).
   - Burial metamorphism below 11 km is unchanged.
2. **What the sea refuses builds a terminal fan or a prograding delta**
   (`build_terminal_fan`, switch `erosion.terminal_fans`), instead of the planned aggradation
   along the river's trunk. The review showed that the trunk holds little (most rivers are
   steeper than the grade line) and that the model would cut it out again in the next step
   (findings 1, 2). The diagnostic also showed that 40–47% of the leftover comes from mouths on
   water bodies of 1–3 cells: depressions below sea level inside continents, which the model
   treats as sea.
   - The leftover of a mouth becomes a cone around the mouth whose surface falls with
     `alluvial_slope` (2 × 10⁻⁴, ~22 m per cell; megafans and delta plains fall by 10⁻⁴–10⁻³:
     Kosi, Okavango, Taquari, Pilcomayo; Leier et al. 2005; Hartley et al. 2010).
   - The cone spreads through the cells it covers, land and sea (a full shelf sea becomes
     land: the delta progrades). Its height is the one that holds the amount, with the
     isostatic response of each column. (As first implemented it stopped at deep water and
     covered lows behind higher ground; both were changed after the results evaluation, see
     [`results.md`](results.md).)
   - Nothing is piled on one cell any more. The overfill counter stays, for the case that a
     cone exceeds its search radius (64 rings).
   - Not done: a transport threshold for incision. Sediment of a cone is eroded again like any
     sediment; in a filled basin the next step's drainage then carries it on to the next sea.
3. **Dropped after measuring it (see [`results.md`](results.md)).** Depressions that a river
   crosses at its transport slope are not basins
   (`erosion.lake_min_depth`, 20 m ≈ `alluvial_slope` × cell spacing). A cell's elevation is a
   mean over ~110 km. Review finding 3: such depressions must keep filling with sediment and
   must keep holding the base level of their rivers, or they would persist and widen. So they
   stay in `Lakes` (trapping, base level) and only lose the water balance: they spill, and are
   neither lakes nor endorheic basins in the statistics.

Not done: no slope limit or hillslope diffusion for existing peaks (the cause is removed
instead), no retuning of K, no change to shelf targets, first-pass caps or the delta reach.
Review finding 4 (a crust sample shown by two cells gets both cells' deposits) is a tectonics
bookkeeping question; it is measured in the results, not changed here.

## Tests

- `full_fans_aggrade_to_their_graded_surface`: a load far beyond the lobe caps leaves no
  overfill; the fan is thicker than the cap and nowhere above its surface; with the switches
  off the same load overfills.
- `sediment_without_room_builds_a_cone_instead_of_a_pile`: full shelf, no room beyond; no pile,
  no cell more than a few falls above its lowest neighbour, deep water not crossed, rock mass
  conserved.
- `shallow_depressions_are_not_basins_but_still_trap_sediment`: a 5 m pit spills and collects
  sediment; a 200 m pit is an endorheic lake.
- The tests of the old routing keep running with the switches off.

## Validation (8 seeds × 1000 Myr, `main` vs branch, 500–1000 Myr, Welch t)

Targets, set before the runs:

| Metric | main | target |
|---|---|---|
| Overfill (km·cells per 100 Myr) | 4400 | ≤ 100 |
| Sediment in terminal fans and deltas (km·cells per 100 Myr) | — | report; expected well below 4400 once fans take their share |
| Spikes > 2 km above all neighbours, per snapshot | 0.08 (all on the capped column) | 0 |
| Land draining into single-cell pits (% of land) | 5.5 | ≤ 1.5 |
| Runtime | 592 s | ≤ +10% |

Expected side effects (review finding 6; rough magnitudes, written before the runs): up to
0.55 km³/yr more sediment stays in the sea, so the sediment part of the sea level at 1000 Myr
may end 10–20% higher (269 → ~300–320 m) and its 500–1000 Myr slope may rise (8.9 → up to
~15 m/100 Myr); flooded continental crust up by a few points; burial metamorphism moves from
continental crust to fan heads on oceanic crust; trench sediment up.

Guard rails (report mean, seed range, t; explain every |t| > 2.4): sediment-part slope
500–1000 Myr, sediment part at 1000 Myr, sediment on oceanic crust ≥ 200 Myr and the thickest
sediment on oceanic crust (review finding 7), sea floor ≥ 200 Myr, mean sea-floor age,
production, hypsometry (highest cell, p99, mean land), land below 10 m (2.3% of land; the
aggradation must not bring back wide plains at sea level), shelf area, flooded continental
crust, runoff, sediment to the ocean, endorheic and lake shares, felsic + sediment net, burial
metamorphism, trench sediment.

Attribution on seeds 0–3: each change switched off alone (`erosion.graded_fans=0`,
`erosion.terminal_fans=0`, `erosion.lake_min_depth=1`).
