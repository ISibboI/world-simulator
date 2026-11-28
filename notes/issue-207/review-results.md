# #207 independent review of results.md and the code (branch at 4bd839e)

Read-only review: logs in `~/tasks/issue-207/runs2` (on) and `runs` (off) were re-parsed with
`analysis/parse.py`, the sweep outputs in `sweep/` and `sweep/v2/` were read, and
`git diff e55bf1d..HEAD -- src` was reviewed. No builds or runs.

**Verdict:** the numbers are reproduced correctly and the model is sound. It is suitable for a
PR once three conclusions are reworded: the attribution of the late cooling, the "second
stable state", and the latent-heat result. None of these needs new code. A short hold run at
100% S₀ would settle the bistability claim.

## 1. Numbers versus the logs

All the climate, snow and #107 table entries I re-parsed match within rounding. These include
the late/t0/coldest/most-frozen values for every group, the per-seed ranges, CO₂, forcing, φ,
NH/SH cover, the perennial share, W/m², storage, and the sensitivity runs (−5.9/51%,
13.5/15%; −6.8/53%, 10.4/21%). "CO₂ lower in 7 of 8 seeds" is correct (seed 4 is the
exception). The sweep rows match `sweep/*.txt` (off) and `sweep/v2/*.txt` (on), and so do the
seed-1 figures.

## 2. Conclusions

### Must-fix

- **M1: about half of the barren late cooling comes from the φ calibration, not the snow
  albedo.** Late forcing is 59.6 W/m² with snow against 61.0 W/m² without, i.e. −1.4 W/m² of
  greenhouse forcing. The snow itself reflects 1.45 W/m². So the 1.5 K is roughly half a
  CO₂/φ effect. That effect is an artefact of calibrating φ at a glacial start (1.40 vs 1.18).
  A real weathering thermostat would *offset* the snow cooling over the long term, not double
  it. results.md explains the mechanism, but the headline bullet ("Barren default: 1.5 K
  colder late") presents the whole 1.5 K as a snow effect. State the split in the bullet and
  in the PR. Also flag the calibration-at-a-cold-start issue as a follow-up: snow makes it
  worse, but it is a carbon-cycle design question.
- **M2: "A second stable state" is not shown.** The sweep holds each step for 40 years, and
  the return leg is still warming steeply (barren 98→100%: −3.0 → 2.0 °C; modern −1.7 → 3.6 °C).
  It shows hysteresis over 40 years, not two equilibria. In addition:
  - Precipitation is fixed from a warm-climate dump. A cold state would get less snowfall
    (Clausius–Clapeyron), which weakens the snow branch.
  - There is no sublimation.
  - The gap is very large: 7–8 K global and 18 points of frozen surface at the same S₀. That is
    far beyond the narrow bistability window of Calov & Ganopolski (2005), which comes from ice
    sheets with elevation feedback over millennia, not from a pack capped at 1 m.

  Either hold at 100% S₀ for a few hundred years (e.g. 5–10 × 40 yr) and show that it stays
  cold, or rename it "slow recovery/hysteresis within 40 yr". In either case drop or qualify
  "physical in kind" and say that it is probably exaggerated by the fixed precipitation. Also
  say whether the full runs could be path-dependent, i.e. whether part of the late barren/cold
  cooling persists from the glacial t0. Cold-on is still 2.5 K colder and 4.4 points more
  frozen at 1000 Myr.
- **M3: the 1.1 K warming from latent heat is likely a lag artefact.** In `SnowSeason::step`
  the latent heat of a season's snowfall/melt is applied in the *next* season. So melt energy
  is withdrawn after the snow is gone (early summer), not during melt, and the degree-day melt
  is never energy-limited. With that lag the latent heat cannot "delay spring". The mechanism
  results.md gives ("releases the snowfall's heat in winter and takes it back with the melt")
  nets to zero over a year, so it does not explain a *warming*. Present the 1.1 K as a
  side-effect of the one-season lag (or test it with the heat applied in the same season), not
  as a physical result. The `update` doc comment still says "The melt takes no latent heat
  from the land (a degree-day model)", which contradicts the default `snow_latent_heat: true`.

### Should-fix

- **S1: "permanently frozen" changes definition with snow on.** Perennial snow counts as ice
  in `fields.ice`, so on/off frozen fractions are not like for like. The perennial-only land
  is 4.9% of land at barren t0 and 7.9% in cold late, which adds about 1–2.5 points of surface.
  This affects the "+17–30 points more ice" sweep claim and the frozen rows. Say so, and
  ideally give the snow-on frozen fraction without perennial snow.
- **S2: #107 comparison uses different seed sets.** `barren-noperice` is seeds 0–3 and
  `barren-on` is seeds 0–7. On the same seeds 0–3: on 29% versus no-perennial 43% (off 45%), so
  the conclusion holds, but use matched seeds. The "…% of it" figure is an unweighted mean of
  per-snapshot ratios. Pooled (weighted by frozen-summer land) it is 15% on, 34% without
  perennial→ice and 25% off. Report the pooled value or say which one is used.
- **S3: φ comparison with noperice.** Its φ per seed is identical to barren-on (1.39, 1.38,
  1.14, 1.20). So perennial→ice does not touch the calibration. Anyone comparing the mean 1.28
  with 1.40 would wrongly infer that it does. Mention this or omit it.
- **S4: Latent-heat bookkeeping gaps.** These are small but should be listed as caveats:
  - `snow_latent` is a local in `update()`, so the last season's latent heat of every update
    is never applied (1/48 of the flux; late December is snowfall-dominated, so a slight
    cooling bias).
  - Snow `dropped` on flooded cells returns no heat.

  The unit test closes the water budget, not the energy budget.

## 3. Code review (no bias-relevant bugs found beyond the above)

- The degree-day formula (σφ(T/σ) + TΦ(T/σ)), the erfc snow share and the periodic
  interpolation of monthly weights (centres at k·12/m + 0.5) are correct. Season totals sum to
  the annual precipitation.
- Albedo combination: brightening × (1 − ice-sheet cover), so there is no double counting on
  ice sheets. Perennial snow keeps the snow albedo and does not also get `ice_albedo`. The
  caps work out (0.32 + 0.5·0.6 = 0.62 and 0.37 + 0.5·0.5 = 0.62, both equal to `ice_albedo`).
- Perennial rule: `end ≥ year-start` together with the least monthly cover is sensible. The
  cap correctly keeps capped cells perennial.
- Minor: the first `update` in `Planet::new` has no precipitation, so the snow gets 1 m/yr
  everywhere. That inflates the early pack in dry cold regions, which never loses it without
  sublimation. The bias probably fades by t0 but is worth a line.
- Minor: the statistics' land mask (`!ocean && lake < LAKE_DEPTH`) differs from the
  temperature model's (view elevation ≥ 0 and not lake). Dry basins below sea level count as
  land with zero snow.
- Minor: on the first year of a modern run, `warmest_season` starts at `ICE_FREE_START`, so the
  forest mask is at its maximum.
- Minor: the snow-reflection W/m² excludes ice-sheet area (× (1 − ice)). That explains why
  cold-on t0 shows only 1.37 W/m² at 90% cover. Worth one sentence if t0 W/m² is ever quoted.

## 4. Plausibility

- Seasonal cover is plausible for a warm (15–22 °C) planet with little high-latitude land:
  NH Jan ≈ 25% and NH Jul ≈ 2–3% of land late, with SH winter 17–27%.
- The global albedo effect of 0.9–1.5 W/m² is of Earth's order. Flanner et al. (2011) is
  quoted correctly as NH snow plus sea ice.
- The summer-surviving snow at the glacial starts (47–90% of land perennial) is plausible as a
  glacial state. It is an upper bound (no sublimation).
- "No snowball down to 90% S₀" is supported by the sweep, which keeps open tropical ocean.
  The comparison with Voigt & Marotzke (≈91%) is fair.
- Coarse-grid effects (one land temperature per ~440 km cell, no slope or aspect) are
  correctly declared as accepted (#64).

## 5. Missing or misleading for the PR

- The PR summary should carry M1 and M2 ("about half of the late cooling is the carbon
  calibration"; "hysteresis over 40 yr, not proven bistable").
- Water storage of 0.01–0.4 m of sea level not taken from the ocean is stated. Fine as long as
  it is mentioned.
- The runtime claim (+8%) is a single local pair and could not be verified here. Label it as
  one measurement.
