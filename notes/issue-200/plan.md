# #200: inorganic carbonate sediment from the carbon cycle's burial

Issue #200. Builds on the carbon cycle of #154/#196 ([plan](../issue-154/plan.md), caveat in
[results](../issue-154/results.md#caveats): "carbon buried as carbonate is not added to the
sediment, and the plates do not carry it") and on the regolith of #197 (PR #224, stacked base).

Project goal: an Earth-like planet, not Earth. Earth numbers are orientation only; the coarse
grid (~110 km cells) is accepted (#64). Values marked *(mem.)* are from memory, not checked
against the source while writing; ranges are wide on purpose.

## 1. What is missing today

- The land sink of #196 (silicate weathering, mol Ca+Mg = mol C) is buried as carbonate only on
  paper: the carbon leaves the exogenic reservoir and goes nowhere. Its mass (~100 g per mol)
  never reaches the sediment.
- The degassing of sediment carbon uses fixed concentrations: 0.15 mol C per kg of subducted
  sediment (3 wt% CaCO₃, half returns at arcs) and 0.5 mol/kg of metamorphosed and collided
  sediment (10 wt% CaCO₃, half decarbonated). Nothing follows the carbonate that is actually
  in the sediment that subducts or metamorphoses.
- Eroded sediment counts as weatherable silicate in the land sink, also where it would be
  carbonate.
- `initial_sediment.carbonate_rate`/`ccd_depth` only shape the initial pelagic ooze of the
  modern preset; nothing is labelled carbonate.

## 2. The model

### 2a. A carbonate label carried by the plates

Each crust sample gets the **mass fraction of its sediment that is carbonate**
(`Cohort::carbonate`, next to the cover-cohort tracers of #34/#35, which already travel through
every path that moves sediment: transport, mixing in the deformation sheet, gap fills, merges,
trench additions, crustal flow). Like the well-mixed cohort tracer it mixes by mass where
sediment is added and keeps its fraction where sediment is removed (a removed slice of a
well-mixed column).

Why not a per-cell label like #197's regolith: regolith re-equilibrates within ~0.1 Myr, so a
label that stays with the cells is good enough. Carbonate lives for 100s of Myr and must travel
with the plates to trenches and orogens; a cell label would detach from its crust within a few
steps. Same idea (a label on part of the sediment layer, no new mass reservoir), but on the
sample. Regolith is part of the well-mixed column.

The carbonate has the sediment's rock-equivalent density (2050 kg/m³ of rock equivalent, the
same compaction): it is part of the sediment layer for isostasy, sea level, erosion and the
cover budget, no new mass reservoir. Erodibility as sediment (2 K).

Molar mass of the carbonate: **0.1 kg/mol** (CaCO₃; dolomite would be 0.092 per C). Each mol
is 0.044 kg of CO₂ from the exogenic reservoir plus 0.056 kg of cation oxide from the rock.

### 2b. Where the carbonate comes from each step

After the carbon cycle's step (which gives the land sink) the planet precipitates
`F = F_new + F_recycled` mol of carbonate:

- `F_new`: the land sink of the step (mol/yr × years) when the cycle is active. While the CO₂ is
  held (ramp-up, #213) or the cycle is off, the sinks are not applied to the exogenic carbon;
  then the carbonate buried is the land share of the sources, `D · land/(land + seafloor)`, so
  that the held CO₂ is consistent with a balanced budget and carbon still closes (§4).
- `F_recycled`: carbonate that the surface processes removed in the step (rivers, coasts, waves,
  diffusion; measured on the samples, before − after of the carbonate inventory). Eroded
  carbonate dissolves (carbonate weathering is fast, supply-limited) and reprecipitates in the
  sea: CaCO₃ + CO₂ + H₂O → Ca²⁺ + 2 HCO₃⁻ → CaCO₃ + CO₂ + H₂O, **CO₂-neutral**. The erosion moved
  its mass downstream as (unlabelled) clastic sediment.

Mass: the carbonate deposited has mass `F · 0.1 kg`. Its cation oxide (and all of the recycled
carbonate) was routed by the erosion as clastic sediment, so the same mass,
`F_new · 0.056 + F_recycled · 0.1` kg, is **taken back from the step's clastic deposits**
(the dissolved load, in proportion to the step's positive sediment changes; non-carbonate
sediment only). The rock gains exactly the CO₂ of `F_new` (0.044 kg/mol), which left the
exogenic reservoir. If the step's deposits cannot supply it (no erosion: then also no land
sink), it is taken from the non-carbonate sediment of the deposition samples; what still lacks
is counted (`unmatched`, expected 0) and logged.

The carbonate fraction of eroded sediment is removed from the silicate weathering supply: the
sediment term of the land sink uses `sediment_erosion · (1 − carbonate)`.

### 2c. Where it precipitates: a saturation state

An abiotic ocean has no pelagic calcifiers: no carbonate ooze, no CCD in today's sense. Inorganic
carbonate forms where the water is supersaturated enough to overcome the kinetic barrier: warm,
shallow shelves and platforms (ooids, aragonite needles/whitings, seafloor cements, tidal-flat
crusts), the "Neritan" ocean of Ridgwell & Zeebe (2005, EPSL 234) whose saturation is set by the
shelf area that takes the burial flux; Precambrian platforms with abundant seafloor-precipitated
aragonite and calcite (Grotzinger & James 2000, SEPM SP 67; Grotzinger & Kasting 1993, *mem.*).
Higgins, Fischer & Schrag (2009, EPSL 284) argue that before oxygenation the sea floor itself
was a carbonate factory: without organic matter respired in the sediment, carbonate precipitated
at the sediment–water interface is not redissolved, so the sea floor (not only the water column)
is the precipitation site (*mem.*). Deep water is colder and under pressure, hence less
saturated: it can stay undersaturated (a saturation horizon) unless the shelves are too small for
the flux.

Per ocean cell (lakes excluded; evaporitic lake carbonate is not modelled), at sea-floor depth
`z` and bottom temperature `T_b`:

- `T_b = T_deep + (T_s − T_deep) e^{−z/z_t}`, `T_s` the cell's annual mean surface temperature
  (≥ −1.8 °C), `T_deep` the carbon cycle's bottom-water temperature, `z_t` = **1000 m**
  (thermocline). Shelves see the surface temperature.
- Saturation `Ω = S · e^{γ (T_b − T₀) − z/z_p}` with the global level `S` (unknown, solved),
  T₀ = 25 °C:
  - **γ = 0.03 /K**: surface Ω_aragonite of today's ocean rises ~2.5× from polar (~1.5) to
    tropical (~4) water (CO₂ solubility and the carbonate equilibria; Feely et al. 2009, *mem.*).
  - **z_p = 5800 m**: the solubility product of calcite rises ~2× at 400 bar (partial molar
    volume change ~−40 cm³/mol near 2 °C; Millero 1995, *mem.*), the main reason deep water is
    undersaturated.
- Precipitation rate (mol/m²/yr) `R = k₀ A(T_b) max(Ω − 1, 0)^n`, **n = 2** (calcite and
  aragonite in seawater: n ≈ 1.7–2.9; Zhong & Mucci 1989, Burton & Walter 1987, *mem.*),
  `A = exp(E_a/R (1/T₀ − 1/T_b))`, **E_a = 40 kJ/mol** (rates rise ~3× from 5 to 25 °C;
  Burton & Walter 1987, *mem.*).
- `k₀` = **0.2 mol/m²/yr** at Ω − 1 = 1 and 25 °C: Earth's neritic carbonate accumulation is
  ~1–2 mol/m²/yr on tropical banks at Ω_aragonite ≈ 3–5 (Great Bahama Bank mud ~0.1 m/kyr;
  Milliman 1993, *mem.*). k₀ sets how concentrated the deposits are: large k₀ keeps Ω close
  to 1 and puts everything on the warmest shallow cells, small k₀ raises Ω and spreads the
  carbonate deeper. The solved Ω of the warmest shelves is logged as a plausibility check
  (Precambrian estimates Ω ~ 3–10, *mem.*; > 20 would say k₀ is too low).
- **Accommodation**: a cell takes carbonate only up to `platform_depth` = **−5 m** (the
  sediment that raises it there, with isostasy and compaction, `isostasy::sediment_for_rise`):
  platforms aggrade to sea level and then build outward. Waves rework what lies above the wave
  base; that carbonate is recycled (§2b), so the platform top is shaped by the existing wave
  erosion.
- `S` is solved by bisection so that `Σ_cells min(R · area · years, room) = F` (monotone in S).
  If all room is full, the rest goes in proportion to R above the cap (counted, expected 0).

The precipitation is applied per **crust sample** (a sample shown at k cells gets the mean of its
cells' rates and room, so the deposits do not pile k-fold on one sample), which keeps the total
mass exact: the books treat every sample as one cell of area. The same holds for the clastic
removal.

**Biogenic planet** (`--biosphere modern`, `initial_sediment.biogenic_pelagic`): calcifying
plankton export carbonate to the open ocean. A share `pelagic_share` = **0.5** of F (pelagic vs
neritic accumulation of today's ocean are of the same order; Milliman 1993, *mem.*) settles as
ooze on sea floor shallower than the CCD (`ccd_depth`, 4500 m as the initial sediment; the CCD
itself is not modelled), uniformly per area; the rest follows the saturation rule (reefs, banks).
Barren: 0.

### 2d. What the plates do with it

All conversions are counted per process in kg of carbonate (rock-equivalent sediment × the
sample's carbonate fraction), so the carbon cycle uses the actual carbonate:

| process (existing code path) | carbonate goes | CO₂ |
|---|---|---|
| subducted with the slab (below the décollement) and removed by subduction erosion | mantle; **`slab_carbonate_return` = 0.5** returns at the arc | arc source = 0.5 × subducted carbonate (replaces 0.15 mol/kg × subducted sediment) |
| scraped off into accretionary wedges | stays in the crust (low-grade metasediment) | none |
| burial metamorphism (`metamorphose_sediment`) and collision stacking | metasediment crust; **`metamorphic_decarbonation` = 0.5** decarbonates (calc-silicate reactions), the rest stays as marble | metamorphic source = decarbonated carbonate (replaces 0.5 mol/kg × metamorphosed + stacked sediment) |
| eroded on land / reworked in the sea | dissolved and reprecipitated (§2b) | neutral |
| distributed shortening, transport, merges | moves with the sediment (tracer) | — |

Sources: Kelemen & Manning (2015, PNAS): a large part of subducted carbon is removed from the
slab, but much may stay in the mantle lithosphere/crust; Dasgupta & Hirschmann (2010): 20–80%
returns (*mem.*). Metamorphic decarbonation needs silicate reactants and fluid infiltration;
pure limestones survive as marble (Kerrick & Caldeira 1998, *mem.*). 0.5 keeps the old
assumptions (#154 plan §2: "about half returns", "half decarbonated"), now applied to the
actual carbonate.

**Decarbonation changes mass**: the metasediment of burial metamorphism and collision stacking
is lighter by the CO₂ it releases (0.044 kg per mol decarbonated), booked as a new mass-ledger
term `decarbonation` (burial) or inside collision consumed/stacked. Exact; zero without
carbonate.

The arc and metamorphic rates are smoothed like the other tectonic rates (3 Myr). The difference
between what left the sediment and what was degassed is a small "in transit" reservoir (§4).

The fixed-concentration terms stay for the switch-off mode.

**Expected change of the arc/metamorphic CO₂.** Barren: the pelagic sediment is clay, the
carbonate sits on shelves of continental crust, which does not subduct; subducted sediment is
mostly trench fill and rise sediment. The arc's sediment term (today 0.15 mol/kg × ~2–3 Gt/yr
≈ 0.3–0.5 Tmol/yr) likely drops several-fold; the arc's large terms (mantle wedge, altered
crust) are unchanged. The metamorphic term follows the carbonate fraction of the continental
cover: with 10% at the start it stays about as today (~1–1.5 Tmol/yr), then drifts with the
cover's fraction. φ recalibrates at the end of the ramp-up.

### 2e. Initial carbonate

The generated start has 150 Myr–Gyr old cover, so it carries carbonate already:

- continental cover (platform and shelf prisms): `platform_carbonate` = **0.1** barren (the
  carbonate share of the burial flux, ~10% of the sediment flux on Earth; Precambrian
  successions are ~10–20% carbonate, *mem.*), **0.2** modern (Phanerozoic platforms ~20–25%
  carbonate; Ronov 1980, *mem.*);
- pelagic sediment: barren clay 0; modern: the ooze part above the CCD,
  `(carbonate_rate − clay_rate) · min(age, ccd_age)` of the uncapped column;
- rises (terrigenous): 0.

The drift of the cover's carbonate fraction over 1000 Myr shows whether these are near the
planet's own steady state.

## 3. Seafloor weathering

The seafloor sink (#196) is carbonate in the altered basalt (veins, amygdales), not sediment: it
stays in the crust as now. It is booked as a global reservoir (altered crust carbon: + uptake,
− the slab carbon of the subducted crust, of which `slab_return` degasses at arcs as now), only
for the reservoir partition. Tracking it per sample would need the uptake on the young crust
samples; not in this issue.

## 4. Conservation

Carbon (mol), per step and cumulative, logged with each snapshot:

`N + C_sed + C_aoc + C_crust + C_transit = start + mantle degassing (ridges, wedge, plumes) −
mantle uptake (unreturned slab carbonate, unreturned altered crust, carbonate lost with crust)`

- `N` exogenic (atmosphere + ocean), `C_sed` carbonate in the sediment of all samples (hidden
  ones included), `C_aoc` altered crust, `C_crust` marble and wedge carbonate in the crust,
  `C_transit` the smoothing buffer.
- The sediment inventory is compared with the counted fluxes (like the mass ledger's residual);
  carbonate lost where hidden crust is removed (merges, removed plates, cap) appears there and is
  booked to the mantle; the residual is reported (expected ≪ 1%).

Mass: the exchange of §2b changes the rock mass by `+0.044 kg × F_new` exactly; decarbonation
by `−0.044 kg × decarbonated`. Unit tests: deposition + clastic removal conserve mass to
rounding; the deposited carbonate equals F; tracer mixing conserves carbonate in sheet mixing,
`add_sediment`, `remove_sediment_to`; burial metamorphism and collisions remove exactly the
decarbonated CO₂; the switch off gives zero carbonate everywhere and the old sources.

## 5. Switch and parameters

`--set carbon.carbonate_sediment=0` turns it all off: no initial carbonate, no precipitation, no
clastic exchange, the old fixed-concentration sources. With it off, state hashes match #224's
head. New parameters (`--set carbon.NAME`): `carbonate_sediment`, `slab_carbonate_return`,
`metamorphic_decarbonation` (also in the tectonics, which removes the mass),
`carbonate_molar_mass`, `carbonate_rate_constant` (k₀), `carbonate_reaction_order` (n),
`carbonate_activation_energy`, `carbonate_saturation_temperature` (γ),
`carbonate_pressure_depth` (z_p), `thermocline_depth` (z_t), `carbonate_platform_depth`,
`carbonate_pelagic_share`, `carbonate_ccd`; `initial_sediment.platform_carbonate`.

Transcendental math through `crate::math`; deterministic (no parallelism, fixed iteration
counts).

## 6. Code

- `src/tectonics/crust.rs`: `Cohort::carbonate`, `add_sediment`/`remove_sediment_to` and
  `remove_clastic`, `Crust::carbonate()` (rock-equivalent m); `sheet.rs` Mixture,
  `boundaries.rs` shares and trench counters (`TrenchSediment::carbonate_*`), `cover.rs` keeps
  the carbonate when the cohort starts, `initial_sediment.rs` initial fractions.
- `src/tectonics/mod.rs`: burial decarbonation, carbonate counters in `carbon_drivers`,
  `carbonate_inventory()`, `exchange_carbonate()` (deposit per sample + clastic removal, ledger,
  material and cover budgets, sea-level re-solve with a `carbonate` sea-level budget term).
- `src/carbonate.rs` (new): parameters live in `CarbonParams`; the saturation solve and the
  per-sample distribution as pure functions (unit-tested).
- `src/carbon.rs`: drivers and smoothed rates for carbonate; sources from the actual carbonate
  when on; `LandCell::carbonate`; the land share for the hold; reservoir bookkeeping.
- `src/planet.rs`: carbonate fraction of eroded sediment, inventory before/after the surface
  changes, the exchange after the carbon step, statistics.
- `src/main.rs`: a snapshot log line (deposition by depth/latitude/crust class, Ω, fractions of
  the sediment, reservoir partition and closure), cell-dump columns `carbonate` (fraction) and
  `carb_dep` (m/Myr).
- No render mode or cell-info row (frame size); possible follow-up.

## 7. Evaluation

Commit, then through `runs`: default barren 8 seeds × 1000 Myr against #224's head (baseline
runs), and `--biosphere modern --epoch 4`, 4 seeds. Report: carbonate deposition rate and its
place (depth bins, latitude, shelf on continental crust vs deep), the carbonate fraction of the
sediment (all, continental cover, sea floor, subducted), the reservoir partition and closure,
arc/metamorphic CO₂ before/after, pCO₂, temperature, φ (0.3–3?), relief/shelves if the extra
sediment changes them, runtime. Independent evaluation. Results to `notes/issue-200/results.md`.

## 8. Overlaps

PR #221 (volcanism) adds LIP CO₂ to the plume source and #223 (H₂/CH₄) adds sources to
`carbon.rs`; #220 touches subduction erosion and crustal flow in `boundaries.rs`. Conflicts are
textual (`Sources`, `TectonicDrivers`, the subduction-erosion loop), resolved when they merge.

## 9. Changes after the review ([`review-plan.md`](review-plan.md))

| finding | change |
|---|---|
| M1 rate smoothing does not conserve carbon; a "transit" difference can go negative | The carbonate sources get **real buffers**: per process (subducted, decarbonated) `B += carbonate leaving the sediment`, degassing `B (1 − e^{−dt/τ})`, τ = `degassing_smoothing` (3 Myr). Exactly conserving, never negative, also through the hold. Rate smoothing stays for the other sources and the switch-off path. |
| M2 marble and wedge carbonate is a one-way sink | Option (a): booked openly as "carbonate locked in crystalline crust", a sink in the partition. Its later erosion (as silicate bedrock), subduction or restacking does not draw on it; a `Crust::marble` tracer is a follow-up issue, to be taken up if the evaluation shows the sink large against the sediment carbonate. |
| M3 waves rework carbonate above the wave base | The room of a cell is capped at its **storm wave base** (`erosion::wave_bases`, open and sheltered), not a fixed −5 m (that is the fallback without erosion). `F_recycled / F_new` and the wave-reworked carbonate are logged. |
| M4 recycled carbonate must exclude metamorphism | `apply_surface_changes` counts the carbonate its surface part removed (inventory before/after the sharing, before `metamorphose_sediment`), which records its own counter. |
| M5 solve over samples | Done so: the bisection runs over the shown samples (mean rate and room of their cells), so the deposit equals F by construction. |
| m1 hold split at φ = 1 | Stated (only during the ramp-up). |
| m2 collision decarbonation | One ledger term `Decarbonation` for burial and collisions. |
| m3 losses at the ledger sites | Losses with removed crust stay in the sediment-carbonate residual (inventory − counted terms), reported; booked to the mantle. Counted at the sites if the residual is not small. |
| m4 silent clamps | `debug_assert!` on the fraction range; clamping only in `remove_clastic` (rounding). |
| m5 regolith "non-carbonate bottom" | Dropped: regolith is part of the well-mixed column (carbonate removed with it is recycled). |
| m6 where the dissolved load is taken | From ocean-cell deposits first, other deposits as fallback. |
| m7 subducted carbonate | = lower plate's (sediment − accreted sediment) × fraction + forearc sediment removed by subduction erosion × fraction. Subducted metasediment (marble) is not counted (M2). |
| m8 Mg carbonate | 0.1 kg/mol (calcite) overstates the mass by ≲ 10% where Mg carbonate forms; stated. |
| m9 state hash | The carbonate fraction is hashed only when the switch is on. |
| m10 bit-identity | Test: switch off gives zero carbonate and no new mutating calls; state hashes against #224 checked on a short run. |
| m11 evaluation | Pass/fail orientation: modern subducting sediment vs GLOSS ~7 wt% CaCO₃; barren Ω of the warmest shelves 3–10 (≲ 20). F_recycled/F_new reported. |
| m12 nucleation | The law is a heterogeneous (sea-floor cement, ooid) law without a nucleation threshold; homogeneous nucleation would need Ω ≳ 10–20 (Mg inhibition). Consistent with Higgins et al. 2009. |
