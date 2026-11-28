# #207 plan review (independent)

Reviewed: `notes/issue-207/plan.md` against `src/temperature.rs` (land albedo block ~l. 885–905,
land update ~l. 1010–1030, `ice_sheet_cover`/`blend`), `src/presets.rs` (abiotic land albedo),
`src/carbon.rs` (`weathering_cells`: `1 − ice`). The overall design is sound: a mass-based pack
(snowfall needed, PDD melt), a smooth rain/snow split, temperature ageing, forest masking only
with vegetation, and bit-identity with snow off. The issues below are about energy, the internal
consistency of the albedos, the coupling into `fields.ice`, and whether the evaluation can tell
a physical snowball from an artefact.

## Must fix

1. **Latent heat of melt is missing, so energy is not conserved (and the bias is large here).**
   Land heat capacity is the air column, 1e7 J/m²/K. Melting 0.3 m w.e. takes 1e8 J/m², which is
   ~10 K of land temperature, or ~25 W/m² over a 45-day melt. Without the sink the land warms
   too early in spring, the PDDs grow, and snow disappears too fast. This shortens the snow
   season, so the snowball test is biased *optimistic*. It also creates energy, which the project
   rules forbid. Take `L_f ρ_w · melt / season_length` out of the land energy balance of the
   coarse cell (an area-weighted mean over its fine land cells), applied in the next season, the
   same lag as the albedo. Snowfall freezing does not need a source term, because the
   precipitation's latent heat is not in the EBM. Otherwise show with a diagnostic that it is
   small. As written, "caveat" is not enough.

2. **Snow can be brighter than an ice sheet.** Barren: 0.37 + 0.5·(0.85 − 0.30) = 0.645;
   modern: 0.32 + 0.5·(0.85 − 0.20) = 0.645. Both are above `ice_albedo` 0.62. The implied
   ice-sheet surface albedo is (0.62 − 0.37)/0.5 + 0.30 = 0.80 (the same 0.80 for modern). Ice
   sheets are snow surfaces too, so treat them in one consistent way: derive both from one
   surface-to-planetary conversion (fresh snow 0.80–0.85 surface ⇒ ice ≈ snow_fresh), or cap the
   snow at `ice_albedo`. Otherwise land under perennial snow outside the Budyko ice sheet is
   brighter than the ice sheet beside it.

3. **Provenance of the snow-free `land_albedo` 0.32.** #190's table calls it "vegetated land".
   If 0.32, the land heat capacity ("1.3e7 snowballs") or the 91%/94% snowball thresholds in the
   module header were ever tuned against Earth's temperatures, they already hold Earth's snow
   implicitly, and adding snow counts it twice. The plan must say where 0.32 comes from. Snow-free
   TOA albedo over land (CERES, roughly 0.25–0.35 depending on cloud and desert share) supports
   it physically. A global cooling in the snow-on runs must then not be "compensated" by
   retuning.

4. **The perennial-snow → `fields.ice` criterion is too loose, and its effect is entangled.**
   "Minimum cover over the year's seasons" with `c = tanh(S/0.01 m)` makes a cell fully
   ice-covered once ~3 cm w.e. survives one summer. That switches weathering off (`1 − ice` in
   `carbon.rs`), which shifts CO₂. Use a mass-balance criterion instead: snow left at the end of
   the melt season in both years of the update and a non-negative annual net balance, or
   overflow > 0. Put the `ice` coupling behind its own switch, so that runs can separate the
   albedo effect from the weathering/CO₂ effect. Without sublimation (see should-fix 3) this
   criterion over-counts cold deserts. That is the right direction for #107, but report how much.
   Also state that two ice criteria now coexist (Budyko annual mean −5…−15 °C scaled by P/0.3,
   and snow mass balance), and that they can disagree. Example: annual −12 °C with a +10 °C
   summer counts as Budyko ice even though the snow melts every summer. Report the disagreement
   (share of land, each direction) and leave unifying the two to a follow-up issue.

5. **The evaluation cannot yet tell a physical snowball from an artefact.** Add:
   - **Onset diagnostics** for any freezing run: the year-by-year land snowline, perennial snow
     share and sea-ice edge. These show whether the runaway starts on land (snow) or at sea, and
     whether it starts at the season/albedo lag.
   - **A sensitivity set on the failing (or the coldest surviving) case**:
     `surface_albedo_share` 0.35, `snow_fresh_albedo` 0.80, latent heat on/off, and snow updated
     per substep instead of per season. If the snowball disappears under one of these plausible
     variants, it is model-sensitive, not robust.
   - **The solar sweep must exercise snow realistically.** The `solar_sweep` test calls
     `update(&elevation, Some(&precipitation), None, 0.0)` without monthly precipitation, so the
     seasons are uniform. Pass seasonal precipitation or say so. Report the threshold shift ΔS
     against snow off. Comprehensive GCMs include seasonal snow and freeze with modern
     continents at ~91% (stay ice-free at ~94%, Voigt & Marotzke 2010). A snow-on threshold above
     ~95% S₀ for Earth-like continents means the model is too sensitive.
   - **Earth orientation numbers for the modern case:** NH snow cover in January about 45–50% of
     NH land (~45–47 M km², Rutgers/NOAA climate record); in August only a few % outside
     Greenland. Global land-snow shortwave effect of order −1 W/m² (Flanner et al. 2011 give
     −2.2 to −4.6 W/m² NH mean for the whole NH cryosphere, snow and sea ice; please check the
     exact split). Large departures need an explanation (coarse grid, continents) before any
     snowball conclusion.

## Should fix

1. **`∂α_p/∂α_s = 0.5` is probably at the high end for snow regions.** High latitudes are cloudy
   (70–80%) with long slant paths. Qu & Hall (2005) values for snow regions are mostly at the
   lower part of the range cited. Donohoe & Battisti (2011) give an atmospheric attenuation from
   which ~0.5 is a global-mean estimate, not a high-latitude value. Overestimating it pushes
   toward snowball. At least run the sensitivity (must-fix 5) and quote the high-latitude value
   in the notes.
2. **Albedo lag and once-per-season updates.** The albedo of season s uses the snow at the end of
   s − 1. In spring a pack that melts during s keeps full albedo for the whole season (~15 days
   of extra brightening). The cost estimate (0.3 M ops/yr) leaves room to update per substep
   (×8), using the substep temperature, which the PDD σ already handles. Or use the mean of the
   start and end cover of s − 1. At least quantify it once.
3. **No sublimation.** In cold dry interiors this is 10–30%+ of snowfall. It decides whether
   thin packs survive summer (must-fix 4). Add a simple term, or state that the perennial share
   is an upper bound.
4. **Cover formula.** ECHAM's form (Roesch et al. 2001) is `0.95·tanh(100·S)`. Either cite the
   0.95 factor and drop it on purpose, or keep it.
5. **Temperature ageing (Robock 1980).** It is fine, but the 0.60 "aged" value applies at season
   means ≥ 0 °C, while the pack may be mostly gone by then. Check that spring albedo is not
   dominated by fresh-snow values on packs that are actively melting under σ = 5 K days.
6. **Water budget.** The closure test is good. One sentence is off: overflow is not "already
   contained" in the runoff in any physical sense, because the climate simply does not see it.
   Say that seasonal storage and the 1 m cap leave the annual P − ET unchanged, and give the
   magnitude (1 m w.e. on ~10% of the surface ≈ 0.05 × 10¹⁵ m³ ≈ 0.1 m sea level). Note also that
   the cap means ~1 m w.e. is stored per perennial cell; it persists across Myr steps, which is
   fine.

## Optional

- Forest masking: the treeline at warmest month ~10 °C (Köppen) and Betts & Ball (1997, snowy
  boreal forest ~0.15–0.3) are fine. Applying it only to `--biosphere modern` is correct.
- Snow on lake and sea ice: leave it out, as planned.
- Expose `monthly_snow` in the view later (a UI follow-up; it would need the shared desktop/web
  egui touch/responsive rules).
- Log the #107 number with snow on/off, as planned. Add the same split by Köppen EF.

## Literature check

- Fine: Jennings et al. 2018 (~1 °C NH median, 50% threshold); σ = 5 K for PDD (Reeh 1991); the
  Calov & Greve 2005 PDD formula (correct as written); Braithwaite 1995 (~3 mm/K/d snow); Hock
  2003 (2.5–11.6 mm/K/d snow); Voigt & Marotzke 2010 (91%/94%); Robock 1980 ageing; Betts &
  Ball 1997.
- Needs care: Donohoe & Battisti 2011 do not state "∂α_p/∂α_s ≈ 0.5 over snow". That value is a
  derived global estimate (should-fix 1). "EBMs 2–10% below S₀" is plausible as a range, but
  name the source.
