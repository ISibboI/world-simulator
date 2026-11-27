# Review 1 of #190 (barren planet at epoch 0 as the default run)

Reviewed: `notes/issue-190/plan.md`, `git diff ae6c203 HEAD` (f5b0570, 6a1afef), the life-
dependent parameters in `src/` (presets, temperature, climate, erosion, initial_sediment,
realistic mode, koppen, viewer, web/query).

Overall: the change is small and correct. The CLI defaults, the `#[default]` move and the
docs agree; the parameter panel and the web page take their defaults from clap, so they follow
without code changes; `Parameters::new(.., Modern, Epoch::PRESENT)` still equals the struct
defaults (Modern is a no-op, `Epoch::apply` returns early for PRESENT, and the struct defaults
were not touched; the test `presets.rs:288` covers it). The sun view uses `cli.epoch`
(`viewer/mod.rs:676/745/1537`), the Realistic mode switches vegetation off from
`cli.biosphere` (`viewer/mod.rs:673/742`), and the start log line prints biosphere, epoch, Ga,
solar constant and forcing (`main.rs:570`). No real bugs found. `cargo test --release` (2 cores, `agent-scratch/target-review`): 520
passed, 0 failed, 5 ignored.

## Findings

1. **Doc issue (stale, misdescribes the abiotic erosion) — `src/presets.rs:91-93`.** "Since the
   discharge follows the precipitation rather than the runoff (#7), the drier interiors of an
   abiotic planet erode less here, where on Earth the larger runoff fraction would erode more:
   the model understates abiotic erosion." Since #7 the rivers carry the runoff
   (`ErosionParams::runoff_discharge: true`, `erosion.rs:137/204`, module doc `erosion.rs:5`).
   With ω 2.6 → 1.6 the Budyko ET falls, so runoff and discharge *rise* on the abiotic planet,
   on top of K ×2. Fix: replace the sentence with e.g. "Rivers carry the runoff (#7), which the
   lower ω of the abiotic preset raises, so the abiotic planet erodes more through both K and
   the discharge." Since this is now the default run, the doc should be right. (Pre-existing,
   but in the file this PR edits and in the plan's inventory row "Hillslope/channel erosion".)

2. **Doc issue (dangling reference) — `src/presets.rs:54-55`.** `Biosphere::Abiotic` points to
   `notes/issue-190/results.md`, which does not exist on the branch yet. Fix: make sure the
   results file is committed before the PR (or drop the reference).

3. **Suggestion (inventory completeness, plan §1).** The table is complete for the parameters
   the model has; a few life-related items could be named in the "not represented" list, so
   Sebastian sees them:
   - Coastal vegetation (salt marsh, mangroves) that slows cliff/shore retreat and stabilises
     deltas: `erosion.cliff_retreat_*`, `delta_top` are unchanged by the preset. Minor; fine to
     leave, but list it.
   - Snow masking by forests: not applicable, since the model has no seasonal snow albedo on
     land (land ice uses the annual mean) — worth one line so nobody looks for it.
   - Platform cover of the initial continents (`initial_sediment.platform_thickness` 3 km,
     shield fraction 0.4) and the passive-margin prisms are present-day stocks; on Earth a large
     share of platform cover is biogenic carbonate. For an abiotic planet chemical/microbial
     carbonate is plausible, so the composition is fine; the *amount* is an epoch issue (see 5).
   - `carbonate_rate`/`ccd_depth` are unused when `biogenic_pelagic = false` (correct; could be
     stated).
   - The abiotic land albedo is not part of the faint-sun compensation (see 7).

4. **Suggestion — `src/viewer/modes/realistic.rs:52-56`.** The Season option's hover text says
   it changes "the greenness of the vegetation"; on the (now default) abiotic planet there is
   none. The legend already drops the vegetation swatches (`realistic.rs:196`) and the
   description says "none without life". Fix: "… seasonal snow, lake and sea ice, and the
   greenness of the vegetation (if any)" — a const string, so a neutral wording is enough.

5. **Suggestion / caveat for the PR (plan §1, §3) — epoch 0 changes only the sun and the
   greenhouse forcing.** The default run now claims to be the planet "at 4.0 Ga", but the
   initial state is unchanged: 40% continental crust with ~3 km platform cover and a
   present-day ocean-age and sediment distribution, today's ocean volume, a 24 h day
   (`EARTH_ROTATION` in climate.rs:68, ocean.rs:28, temperature.rs:379; the early Archean day
   was ~13–18 h, which would narrow the circulation cells and weaken meridional transport), and
   today's mantle heat flow and plate driving forces. None of this is life, so it is out of
   scope for #190, but the plan's "not covered" list should say that the epoch is "sun +
   greenhouse only", and the README sentence "0 is the earliest start (4.0 Ga, once the planet
   has cooled enough for plates and oceans)" (README.md:20-21) reads as if the initial planet
   were Hadean/Eoarchean. Suggest a follow-up issue (epoch-dependent initial conditions: less
   continental crust, thin cover, faster rotation, hotter mantle) rather than more work here.

6. **Suggestion — `src/viewer/parameters.rs:685/705`.** The round-trip test uses
   `--epoch=-0`, which is now the default value spelled differently (it was the default before
   #150 too). It still tests that a non-default spelling survives, but no longer that a
   non-default *epoch* round-trips. Fix: `--epoch=0.5` (and keep `-0` in a second assertion if
   the spelling case matters). Also the test helper `ModeData { abiotic: false, .. }` in the
   mode tests (`viewer/modes/climate.rs:390` etc.) now runs on a planet built with the abiotic
   default; harmless, but `abiotic: true` would match the planet.

7. **Physics (question 3) — the prescribed forcing is a reasonable default; two caveats to
   state.**
   - Size: (1 − 0.741) × 1361 × 0.7 / 4 = 61.7 W/m² — matches the plan's 61.8. In 1-D
     radiative–convective models this is of the order of 0.1–0.3 bar CO₂ without methane (the
     Myhre log fit, `5.35 ln C`, would need ~10⁵ × PAL and is invalid there; the code says so
     for `--co2`). For an **abiotic** planet the methane route that the doc comment of
     `Epoch::greenhouse_forcing` cites ("0.01–0.03 bar with ~0.1–0.5% of methane") is not
     available (biogenic CH₄), so the CO₂-only number is the relevant one. Estimates of CO₂ at
     ~4 Ga from the carbon cycle (e.g. Kadoya et al., 2020; Krissansen-Totton et al., 2018) are
     ~0.1–several bar, and silicate weathering is an abiotic feedback (it works slower on a bare
     planet, which favours *higher* CO₂). So a forcing that keeps the planet near today's mean
     temperature is plausible for a barren Earth-like planet at 4 Ga. Suggest adding one
     sentence to the `greenhouse_forcing` doc / README that on the abiotic default the forcing
     stands for CO₂ alone (~0.1–0.3 bar), not CO₂ + biogenic methane.
   - It compensates only the faint sun, with Earth's Bond albedo 0.3. The abiotic preset raises
     the land albedo by 0.05 (≈ +0.015 planetary at ~30% land, ≈ −5 W/m²), and the ~1.5 km
     initial highlands sit near the land-ice threshold; that is why the first ~100 Myr of an
     abiotic run are 3–9 K colder (doc of `Biosphere::Abiotic`), and why the Köppen test found
     no tropical (A) climate at t = 0 on the 3-subdivision mesh (`koppen.rs:684`). Physically
     that is acceptable (a brighter barren planet *is* colder at equal CO₂), but for a default
     that people look at first, the PR should state it, and the results should check that the
     default does not drift into a snowball on some seeds (permanent-ice share per seed, not only
     the 4-seed mean). If it does, the consistent fix is to compensate the absorbed-flux
     deficit with the preset's albedo rather than 0.3, not to tune.
   - Keeping the forcing fixed while the sun brightens (#149) warms the default by +12 W/m²
     over 1000 Myr (checked: S/S₀ 0.741 → 0.792 under Gough). That is inconsistent with the
     very assumption that sets the start value (the weathering feedback holds the climate), but
     it is the agreed interim until #154; the plan states it. Fine.

8. **Doc nit — `src/main.rs:184-185`.** The re-wrapped `--epoch` help has a line of ~120
   characters ("… 4 today, up to 9.43 (the Sun before it leaves the main sequence). Sets");
   rustfmt does not wrap doc comments, so CI passes, but the rest of the file wraps at 100.
   Re-wrap.

9. **Doc nit — `src/temperature.rs:136-138`.** `TemperatureParams::epoch` says "today … by
   default". True for the struct default, but next to the new default run it reads as the run
   default. Suggest "today in the struct default; a run starts at epoch 0 (`--epoch`, #190)".

Checked and fine: `sun::EARLIEST_EPOCH` used as the clap default; README (both paragraphs) and
the CLI help agree; `presets.rs` module doc; the panel/URL defaults come from
`arg.get_default_values()` (`viewer/parameters.rs:195`), so an empty URL means the barren
default and `defaults_give_no_arguments` still holds; `query.rs` tests only parse strings
(`epoch=-0.5` is not validated there, as before); `web/tests/browser_test.py` uses
`seed=0&subdivisions=3&duration=50` and does not depend on the biosphere; MOIST_GREENHOUSE check
unaffected by the default; the plan's climate numbers (+12 vs +23 W/m²) are right.
