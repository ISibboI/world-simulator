# #210 decision 1: physical O₂ sinks (the abiotic O₂ budget of #201)

Sebastian on PR #212: decision 1 "yes": the O₂ sink should follow the model's tectonic and
weathering rates instead of the constant placeholder of 3 Tmol O₂/yr. This is the **abiotic O₂
budget part of #201** (O₂/ozone): the sources are the escape's O₂ (and later photosynthesis), the
sinks below. #201 keeps the rest (ozone, the climate seeing O₂, the biosphere's O₂).

Still a global budget, like #196 and #210. The planet: Earth-like, not Earth; Earth's numbers
only give the orientation.

## The sinks (mol O₂/yr)

The O₂ in the air `N` (mol, `p = N M g / A`) changes by `dN/dt = P − G − F − W(p)`:

1. **Reduced volcanic gases, `G = ρ_gas · D`** (as reviewed: D = mantle CO₂ of arcs and plumes only, see below). `D` is #196's degassing
   (mol C/yr: ridges, arcs, plumes, metamorphic; smoothed like the carbon cycle's). Volcanic
   gases from a mantle near the QFM buffer carry H₂, CO and H₂S besides H₂O, CO₂ and SO₂; all of
   them (and SO₂ to sulfate) take up O₂. Holland (2002) puts today's volcanic O₂ demand
   somewhat below ~0.2 mol O₂ per mol CO₂, the share of carbon a living planet buries as organic
   matter (which is why O₂ could rise at all); Catling & Kasting (2017, ch. 7 and 10) give
   Earth's reductant flux of volcanic plus metamorphic gases as 1–5 Tmol O₂-eq/yr on ~6–10 Tmol
   C/yr. **`ρ_gas` = 0.15** mol O₂ per mol C. Metamorphic gases of a barren planet lack the
   organic carbon that makes Earth's reduced (CH₄); they get the same ratio for simplicity (the
   model's metamorphic CO₂ is a small share, 1–10%).
   Model: 14–20 Tmol C/yr → ~2–3 Tmol O₂/yr.
2. **Oxidation of new oceanic crust, `F = c_sf · Ṁ_crust`.** `Ṁ_crust` is the new sea floor
   (kg/yr, smoothed) that #196's seafloor weathering uses. Hydrothermal alteration raises
   Fe³⁺/ΣFe of the upper, permeable crust from ~0.15 to ~0.4 and oxidises some sulfide
   (Lécuyer & Ricard, 1999; Hayes & Waldbauer, 2006). The upper ~0.6 km of ~7 km of crust,
   ~1.45 mol Fe/kg (10.4 wt% FeO), +0.25 Fe³⁺/ΣFe → 0.36 mol Fe³⁺/kg of altered rock → 0.09 mol
   O₂/kg altered → **`c_sf` = 0.008** mol O₂ per kg of new crust. Earth: ~6e13 kg/yr → 0.5 Tmol
   O₂/yr (Lécuyer & Ricard: 0.3–1 Tmol/yr for the crust's Fe³⁺ gain; Hayes & Waldbauer:
   ~0.5–1). Model (~1.6e14 kg/yr): ~1.3 Tmol/yr. The crust's oxidation is set by its
   iron and the circulating water, not by the O₂ (an anoxic ocean oxidises it with water and
   sulfate and releases H₂ that would meet the O₂), so it is a fixed capacity.
3. **Oxidative weathering on land, `W(p) = s(p) · W_max`.** `W_max` is the Fe²⁺ and sulfide
   exposed by erosion: per land cell (the cells #196's weathering uses, without permanent ice)
   `area · (e_bed ρ_bed c(mafic) + k_sed e_sed ρ_sed c(0))`, with the bedrock and sediment
   erosion (m/yr) of the step. The content `c` (mol O₂/kg) interpolates felsic and basaltic
   rock by the mafic share:
   - felsic (upper continental crust; Rudnick & Gao, 2003): Fe²⁺ 0.42 mol/kg (5.0 wt% FeOᵀ,
     60% ferrous), S 0.019 mol/kg (620 ppm);
   - basalt (MORB): Fe²⁺ 1.23 mol/kg (10.4 wt% FeOᵀ, Fe³⁺/ΣFe 0.15; Cottrell & Kelley, 2011),
     S 0.034 mol/kg (1100 ppm);
   - O₂ per Fe²⁺ → Fe³⁺: 1/4; per sulfide S → sulfate: 15/8 (pyrite);
   - share of the Fe²⁺ that weathering oxidises: **0.5** (Fe³⁺/ΣFe rises from ~0.3–0.4 in
     igneous rock to ~0.6–0.7 in shales; much Fe²⁺ leaves in detrital silicates), sulfide fully;
   - felsic 0.089, basalt 0.22 mol O₂/kg; sediment `k_sed` = 0.3 of fresh felsic rock (as
     #196's `sediment_weatherable`: recycled sediment of an anoxic planet keeps part of its
     Fe²⁺ and pyrite).

   The **O₂ dependence** `s(p) = 1 − exp(−√(p/p_ox))`: pyrite oxidises with a rate ∝ [O₂]^0.5
   (Williamson & Rimstidt, 1994), and the supply limit makes it saturate. Detrital pyrite and
   uraninite survive in the Archean (pO₂ ≲ 1e-5 PAL) and are gone after the Great Oxidation
   (≳ 1e-3 PAL; Holland, 2006; Johnson et al., 2014): **`p_ox` = 2e-5 bar** (1e-4 PAL) gives
   27% at 1e-5 PAL, 96% at 1e-3 PAL. Earth-like planet: ~15–20 Gt/yr of bedrock erosion → ~1.5
   Tmol O₂/yr at full oxidation (Earth's pyrite plus Fe²⁺ weathering: ~1–3 Tmol O₂/yr with
   biogenic pyrite; Holland, 2002; Catling & Kasting, 2017).

Expected total for the model's Earth-like tectonics: ~4–6 Tmol O₂/yr, of which ~1.5 from
weathering and ~4 fixed, against the placeholder's 3. The fixed part (gases, sea floor) alone
exceeds the escape's O₂ of a temperate planet (0.01 Tmol/yr) by ~400×, so a temperate barren
planet keeps 0 O₂ as before.

## Step and conservation

Per step (years `y`, O₂ `N₀` before, `P` produced): if `N₀ + P ≤ y (G + F)`, all O₂ goes to the
fixed sinks in proportion to their capacities and `N = 0` (the rest of the reductants stays
unused: as H₂ it would escape or build up, #202). Otherwise `N` solves the implicit (backward
Euler) equation `N + y (G + F + W(p(N))) = N₀ + P` (increasing in `N`, bisection; stable at the 1
Myr step also when `s` is steep). The sinks take `y G`, `y F`, `y W(p(N))`, and `N = N₀ + P −`
their sum, so the budget closes by construction. Cumulative counters per sink; a unit test checks
produced = Σ sinks + in the air (to rounding) over many steps with changing inputs, that the
sinks never take more than there is, and the limits (no O₂ when the fixed sinks exceed the
source; `s` → 1 at high O₂).

## Code

- `EscapeParams`: `oxygen_sink` goes; new `volcanic_reductants` (0.15), `seafloor_oxidation`
  (0.008), `felsic_ferrous`, `basalt_ferrous`, `felsic_sulfide`, `basalt_sulfide`,
  `ferrous_oxidised` (0.5), `sediment_reduced` (0.3), `oxidation_pressure` (2e-5 bar); all
  settable as `escape.NAME`.
- `escape::StepInput` gets the degassing (mol C/yr) and new crust (kg/yr) from the carbon
  cycle's state (`CarbonState` gains the smoothed new-crust rate) and the land cells (#196's
  `carbon::LandCell`, built once per step for both).
- `EscapeState`: per-sink rates and cumulative totals, logged on the escape line.
- `--no-escape` stays bit-identical to #196 (the carbon cycle only gains a reported field).

## Evaluation

Same runs as before at the new head: default barren 8 seeds × 1000 Myr, `--epoch 7` 4 seeds,
`--epoch 8.4` 4 seeds, `--biosphere modern --epoch 4` 4 seeds. Report O₂ over time, each sink,
water lost, the ocean loss time at late epochs, and whether the temperate default ends with 0
O₂. Expectation: more sink → the late loss rises towards the energy limit (epoch 8.4) and
epoch 7's O₂ falls.

## Plan review (independent agent) and what changed

1. **ρ_gas was applied to recycled carbon too** (subducted sediment carbonate, the slab's
   seafloor carbon, metamorphic carbon), which brings no mantle reductants. *Changed:* the
   volcanic sink scales with the mantle CO₂ of arcs (the wedge's own carbon) and plumes only
   (`CarbonState::arc_plume_mantle_degassing`).
2. **G and F could count the ridges twice** (hydrothermal H₂, H₂S, Fe²⁺ of the ridges are the
   crust's oxidation seen from the other side). *Changed:* ridges are in F only; G has no ridge
   term.
3. **Sediment was charged again every time it is moved**, though under lasting O₂ it is
   oxidised already. *Changed:* eroded sediment counts as oxidised (`k_sed` = 0, no parameter);
   only bedrock is charged. Under an anoxic atmosphere this changes nothing, as `W(0)` = 0.
4. **With `--biosphere modern`, `s(p)` sees only the escape's O₂**, not the background 0.21 bar.
   *Kept, stated:* these sinks act on the escape's O₂ budget; the modern planet's own O₂ cycle
   (photosynthesis, burial) is #201's. The fixed sinks take all of the escape's O₂ there anyway.
5. **Pyrrhotite** in basalt (9/4 O₂ per S) instead of pyrite. *Changed:* the content is now one
   parameter per rock, felsic 0.089 and basalt 0.23 mol O₂/kg (0.154 Fe²⁺ + 0.077 sulfide).
6. **Startup:** the tectonics starts at rest, so in the first steps G and F are ~0 and a trace of
   O₂ goes to the weathering. *Accepted:* checked only after the start (the air holds 0 O₂ at the
   first snapshot).
7. The implicit step and its conservation are sound (one root, branches meet at N = 0).
8. No overlap with the carbon cycle (it tracks carbon only); dissolved O₂ is a reservoir of ~1%,
   neglected.
