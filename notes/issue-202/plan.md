# #202 plan: abiotic H₂ and CH₄ from serpentinization and outgassing

Issue #202. A barren planet only (biogenic CH₄ is #203). Builds on the carbon cycle (#196,
`notes/issue-154/`) and the escape with its O₂ budget (#212, `notes/issue-210/`). Earth-like,
not Earth: Earth's fluxes are orientation. Values marked *(mem.)* are from memory.

## 1. What exists and what changes

- #212's O₂ budget has a fixed **volcanic** sink `0.15 × D_mantle` (mol O₂ per mol of arc +
  plume mantle CO₂): the reduced gases (H₂, CO, H₂S) that would meet O₂. Under an anoxic
  atmosphere the part of it that is not used "stays unused: as H₂ it would escape or build up,
  #202". This issue makes that part explicit: H₂ and CO become gas fluxes that build up in the
  air, escape, or meet O₂; only the sulfur part stays a fixed O₂ capacity. At the default redox
  state the volcanic O₂ capacity stays ≈ 0.15 per mol C, so #212's O₂ results do not move.
- The escape (#210) counts hydrogen from H₂O only. Now the H₂-equivalent mixing ratio at the
  homopause is `f_T = f_H₂O + f_H₂ + 2 f_CH₄` (Hunten, 1973), and H₂ and CH₄ escape at their own
  diffusion limits.
- The greenhouse forcing is CO₂ only; CH₄ (and H₂–N₂ CIA) are added.

## 2. Sources (mol/yr)

### 2a. Volcanic H₂ and CO (mantle redox)

Per mol of mantle CO₂ of arcs and plumes (#212's `arc_plume_mantle_degassing`; the ridges'
reductants are part of the seafloor oxidation and of the serpentinization, as #212 decided):

- H₂: `w · x_H/(1 + x_H)`, `x_H = r_H · 10^(−ΔFMQ/2)`; CO: `x_C/(1 + x_C)` with
  `x_C = r_C · 10^(−ΔFMQ/2)` (both ratios ∝ fO₂^−½).
- `r_H` = H₂/H₂O = 0.01 and `r_C` = CO/CO₂ = 0.03 at the FMQ buffer and ~1200 °C (Holland,
  2002; Catling & Kasting, 2017; *mem.*).
- `w` = 10 mol H₂O per mol mantle C: plume/MORB gases have H₂O/CO₂ ~3 (MORB ~0.15 wt% H₂O,
  ~0.13 wt% CO₂), arc gases are ~90 mol% H₂O with most of it from the slab (*mem.*). The water
  is not tracked (juvenile water is not in the model); only its H₂ share is.
- **`ΔFMQ`** (`reduced.mantle_redox`, log units, default 0): the planet's mantle redox state.
  −2 gives ×10 the H₂/H₂O; −4 (near IW) about 50% H₂ and CO: the "reduced planet" of the issue.
- CO counts as H₂-equivalent: in the air it becomes CO₂ + H₂ (CO + H₂O → CO₂ + H₂ by OH from
  water photolysis), and its carbon is already part of #196's degassing.
- The volcanic O₂ sink of #212 becomes `ρ_S · D_mantle` (sulfur gases, `reduced.volcanic_sulfur`
  = 0.085) plus the O₂ that the H₂/CO actually meet. At FMQ: ½ (10 · 0.0099 + 0.029) = 0.064, total
  0.149 ≈ #212's 0.15.

Model (~3–4 Tmol C/yr of arc + plume mantle CO₂): ~0.45 Tmol H₂-eq/yr.

### 2b. Serpentinization H₂ at slow ridges

Peridotite is exposed and serpentinized where spreading is slow (detachment faults, amagmatic
segments; Cannat et al., 2010; Escartín et al., 2008); fast ridges build a full magmatic crust.

- The tectonics records the new sea floor per **full spreading rate** (the opening rate between
  the two plates at the new cell, 1 cm/yr bins up to 10, one above): the step's mass-weighted
  exposure `s̄ = Σ m_b s(u_b)/Σ m_b`.
- `s(u)` = mass of serpentinized peridotite per kg of new crust: `s₀` = 0.08 at u ≤ 2 cm/yr,
  falling linearly to 0 at 8 cm/yr (Worman et al., 2016: an inverse relation of serpentinizing
  thickness and spreading rate).
- `Y` = 0.25 mol H₂ per kg of serpentinite (complete serpentinization of harzburgite gives
  ~0.2–0.35; McCollom & Bach, 2009; Klein et al., 2013; *mem.*).
- `H₂ = Ṁ_crust · s̄ · Y`, `Ṁ_crust` the carbon cycle's smoothed new crust.
- Earth (65 Gt/yr; ~5% of crust production at ultraslow, ~25% slow, ~30% intermediate
  ridges, *mem.*): s̄ ≈ 0.29 s₀ → **0.4 Tmol H₂/yr**. Literature: 0.1–0.4 (Cannat et al., 2010),
  0.4–1.5 at ridges plus 0.6–1.1 at transforms (Worman et al., 2016; Rüpke & Hasenclever, 2017).
  Mid-range.
- The water whose O goes into magnetite is taken from the ocean (one H₂O per H₂). The hydration
  water of serpentine (~13 wt%) returns at subduction and is not tracked.
- Forearc serpentinization (by slab fluids) keeps its H₂ at depth: left out.

### 2c. Abiotic CH₄ (Fischer–Tropsch-type)

`CH₄ = χ · H₂_serp`, `χ` = 0.01 mol CH₄ per mol serpentinization H₂: CO₂ + 4 H₂ → CH₄ + 2 H₂O,
so the serpentinization's net H₂ is `(1 − 4χ)` of it, the C comes from the exogenic carbon and
the water goes back to the ocean. Earth: ~0.004 Tmol/yr, inside 0.001–0.01 (Etiope & Sherwood
Lollar, 2013). Upper bound of Guzmán-Marmolejo et al. (2013): ~0.2 Tmol/yr. Magmatic CH₄ is
negligible at magmatic temperatures (CH₄/CO₂ ≪ 10⁻⁴ at FMQ) and is left out, also for reduced
mantles (a known limitation: very reduced, cool outgassing could give CH₄).

## 3. Sinks and the step

Per step of `y` years, with the gas in the air `N_H₂`, `N_CH₄` (mol) at its start:

1. **Pools**: `H = N_H₂ + y S_H₂` (volcanic H₂ + CO, net serpentinization H₂), `M = N_CH₄ + y S_CH₄`.
2. **O₂ titration** (#212's `oxygen_step`): the gases join the fixed capacities with ½ O₂ per H₂
   and 2 O₂ per CH₄, next to the sulfur and seafloor sinks. When O₂ is short, all of it is used
   and the sinks share it in proportion (as in #212); the unused share `(1 − s)` of H and M is
   free gas. When O₂ is left, all gas is oxidised (lifetime ~years against the step) and the
   weathering's implicit step follows as before.
3. **CH₄** (free `M_f`): backward Euler, `N_CH₄ = M_f / (1 + y (k_ph + k_ox + k_esc))`:
   - `k_ph = 1/τ_an`, `τ_an` = 10⁴ yr: photolysis and oxidation by OH from water in an O₂-free
     atmosphere; products CO₂ + 4 H₂ (CH₄ + 2 H₂O → CO₂ + 4 H₂). Check: Pavlov et al. (2000)
     and Kasting (2005, *mem.*) find ~1000 ppmv for today's biogenic flux (~30 Tmol/yr) in an
     anoxic atmosphere, a total lifetime of ~6 kyr; with the escape below ours is ~7 kyr.
   - `k_ox = (1/τ_ox) (p_O₂/0.21 bar)`, `τ_ox` = 10 yr: OH oxidation in an oxic atmosphere
     (today ~9–12 yr), with `p_O₂` the background O₂ of the biosphere (`reduced.background_oxygen`:
     0.21 bar modern, 0 barren; the escape's O₂ has titrated the gas already). Products
     CO₂ + 2 H₂O; the O₂ is the biosphere's (#201), counted but not taken from #212's budget.
   - `k_esc`: diffusion-limited escape of its H: `(b/H_a)/n_col · c`, with `n_col` the column of
     air (mol/m², 1 bar) and `c = min(1, Φ_EL/Φ_DL)/(1 + f_T)` of the last step; products
     CO₂ + 2 H₂ + 4 H↑ (CH₄ + 2 H₂O; the H₂ joins the H₂ pool).
4. **H₂** (free `H_f` + the H₂ from CH₄): `N_H₂ = (H_f + H₂ from CH₄)/(1 + y (k_esc + k_ox,H))`,
   escape coefficient as for CH₄ per H₂, oxic lifetime 2 yr (OH; today's 2 yr incl. soils).
   Lifetime by escape at 1 bar: ~27 kyr, so `f_H₂ ≈ S/(b/H_a A)`: 1 Tmol/yr → ~150 ppm
   (Kasting's ~0.1% volcanic H₂ for a few times more outgassing).
5. **Water escape**: the water's share of the diffusion limit, `2 (b/H_a) f_H₂O/(1 + f_T)`, but at
   most `Φ_EL` minus the gases' H of the last step (they compete for the XUV energy). Its O₂ is
   #212's source. Lagged by one step only in the energy-limited regime with gas present (only
   with a moist greenhouse and an anoxic atmosphere at once); the books close exactly anyway.
6. **Carbon**: FTT takes C from the exogenic carbon, CH₄ destruction returns it as CO₂;
   `CarbonCycle::exchange` books the net into its `N` after each step.
7. **Water**: net water used by the chemistry (serpentinization +1 per H₂, CO +1, FTT −2 per
   CH₄, CH₄ destroyed anoxically or by escape +2, oxidation by O₂ −1 per H₂ and −2 per CH₄) is
   taken from (or returned to) the ocean with the escaped water.

Closed budgets, with cumulative counters and unit tests over many steps with changing inputs:
- **Redox** (H₂-equivalents, O₂ = −2): volcanic + serpentinization = escaped H/2 from gases +
  2 × O₂ titrated (escape O₂ and background O₂) + `N_H₂ + 4 N_CH₄`;
- **C**: CH₄ made − CH₄ destroyed = `N_CH₄` (and = what the carbon cycle got back, negative);
- **H**: 2 volcanic H₂ (juvenile) + 2 × net water used = 2 N_H₂ + 4 N_CH₄ + escaped H from gases;
- #212's O₂ budget keeps closing (produced = sinks + air).

## 4. Forcing

- **CH₄**: Myhre et al. (1998), `0.036 (√M − √M₀)` W/m², M in ppbv, relative to the
  pre-industrial `M₀` = 700 ppbv, the reference of the energy balance model's zero forcing (like
  CO₂'s 280 ppm). N₂O overlap left out (no N₂O on a barren planet). Without CH₄: −0.95 W/m².
  Valid to a few ppmv; above ~100 ppmv the shortwave absorption makes the net forcing grow far
  more slowly (Byrne & Goldblatt, 2014; ~1.4 W/m² SW at 100 ppmv, *secondary source*), so the
  fit is capped at 100 ppmv with a note. Abiotic CH₄ stays at ppbv–ppmv, so the cap is moot by
  default.
- **Biotic background**: `--biosphere modern` keeps today's CH₄ (`reduced.background_methane` =
  700 ppbv, held until #203), barren 0. Forcing uses background + abiotic, so a modern run
  changes by ≈ 0.
- **H₂–N₂ CIA**: `a f_H₂ (p_N₂/1 bar)²`, `a` = 40 W/m² (Wordsworth & Pierrehumbert, 2013: at most
  2–3 K for 10% H₂ in 1 bar N₂, i.e. ~4 W/m²). Negligible below ~1% H₂.
- **Start continuity**: the epoch's forcing is the total. The carbon cycle starts from the CO₂
  that makes up the total with the reduced gases of the start (none), and **during the ramp-up
  hold (#213) the CO₂ is held at the total** (`CarbonCycle::hold_forcing`): it makes up any change
  of the gas forcing, so the start stays continuous and φ is calibrated at the gases of the
  ramp-up's end. Afterwards the gas forcing changes freely and the carbon cycle responds.
  With `--no-carbon-cycle` the CO₂ forcing stays at the start's and the gas forcing adds its
  change.
- The **organic haze** threshold (CH₄/CO₂ ≳ 0.1; Trainer et al., 2006; Haqq-Misra et al., 2008)
  is a caveat only (#203): abiotic CH₄/CO₂ is ~10⁻⁶ here. Logged ratio.

## 5. Code

- `src/reduced.rs`: `ReducedParams` (`enabled`, `mantle_redox`, `hydrogen_ratio`,
  `monoxide_ratio`, `mantle_water`, `volcanic_sulfur`, `serpentinite`, `slow_rate`,
  `fast_rate`, `serpentinite_hydrogen`, `methane_share`, `methane_anoxic_lifetime`,
  `methane_oxic_lifetime`, `hydrogen_oxic_lifetime`, `background_oxygen`,
  `background_methane`, `cia_coefficient`; `set` with the `reduced.` prefix), the sources, the
  gas step, the forcing, `ReducedState` (mixing ratios, sources, sinks, forcing, cumulative
  budgets) with a log line.
- `escape.rs`: the gas step runs inside `Escape::step` (it shares the O₂ titration and the
  limits); `oxygen_step` gets the gases as further capacities. With the gases off the code path
  is the old one, bit for bit.
- Tectonics: `opens_away` returns the opening rate; `Simulation` keeps the new sea floor per
  rate bin (cumulative, not in the state hash, no effect on the state).
- `carbon.rs`: `exchange(mol)`, `hold_forcing(forcing, ocean_mass)`.
- `Planet`: greenhouse = CO₂ forcing + gas forcing; statistics and snapshot log line.
- CLI `--no-reduced-gases` (under "More options"); `--no-escape` also turns the gases off (they
  need the escape as their sink), with a note in the help. `Biosphere::Modern` sets the
  background O₂ and CH₄. `remove_water` accepts a negative volume (water returned).
- Math through `crate::math` (`sqrt` is IEEE-exact and allowed, `powf`/`exp`).

## 6. Evaluation

`runs` from one commit: default barren 8 seeds × 1000 Myr, main for comparison (the same seeds);
`--biosphere modern --epoch 4` 4 seeds × 1000 Myr. A short off run (`--no-reduced-gases`)
against main for identical state hashes. Report CH₄ and H₂ over time, sources and sinks,
forcing change, effect on O₂ and escape, temperature change, runtime; plus a sensitivity run
`--set reduced.mantle_redox=-4` (4 seeds) for a reduced planet. An independent reviewer
evaluates the results.

## 7. Plan review (independent agent) and what changed

1. **Unbounded H₂ in the energy-limited regime** (reduced mantle, late sun: sources above the
   XUV limit). *Changed:* the H₂ step is implicit with the escape `min(diffusion limit of its
   own mixing ratio, what the energy limit leaves after the water's share)` (bisection); the
   mixing ratios are against the whole column (air + H₂ + CH₄), so they stay below 1; a unit
   test with sources above the energy limit. The water and the gases share the XUV in
   proportion to their H₂-equivalents (the water's share from the last step's gases), so the
   total stays at the limit. H₂'s own pressure is not added to the surface pressure (reported).
2. **The off switch must be the old code path.** *Done:* with `--no-reduced-gases` (or
   `--no-escape`) the volcanic capacity is the literal `0.15 × D`, no gas forcing is added and
   the CO₂ starts from the full forcing; the spreading-rate bins only add up masses that nothing
   reads. Hash checks also with `--biosphere modern` and `--no-carbon-cycle`.
3. **The CO₂ hold keeps carbon and ratio consistent.** *Done:* `hold_forcing` recomputes the
   exogenic carbon (and the state) from the new ratio.
4. **Serpentinization vs. #212's seafloor sink.** Kept as separate processes: #212's 0.008 mol
   O₂/kg is the Fe³⁺ gain of the upper ~0.6 km of *basalt*; serpentinization oxidises
   *peridotite* exposed at slow ridges. Basalt-hosted vents carry orders of magnitude less H₂
   than ultramafic ones (e.g. Lost City / Rainbow vs. basaltic vents, *mem.*), so the basalt's
   oxidation stays a fixed O₂ capacity under anoxia (its H₂ is small) and is not turned into H₂.
   When O₂ is present, the serpentinization H₂ and CH₄ add ~0.2–0.5 Tmol O₂/yr of new capacity
   to #212's sinks (stated in the results).
5. **Volcanic H₂ at the low end.** ~0.45 Tmol H₂-eq/yr against the 1–8 Tmol/yr of photochemical
   models (Kasting; partly metamorphic and from a hotter mantle). Consistent with #212's 0.15
   restricted to arc and plume mantle CO₂; `w` = 10 is the main uncertainty.
6. **Redox wording.** At ΔFMQ −4 (≈ IW, which is FMQ −3.5 to −4): H₂ is 50% of the water, CO
   75% of the carbon. At reduced redox the sulfur leaves as H₂S (more O₂ demand), but
   `volcanic_sulfur` stays fixed: a stated simplification.
7. **The carbon cycle's prediction ignores the gas forcing change.** Kept: a change of the gas
   forcing reaches the carbon cycle one step later through `toa_imbalance`. By default the gas
   forcing changes by ≪ 0.1 W/m² per step after the start.
8. **CIA law.** *Changed* to `a p_H₂ p_N₂` (bar²), `a` = 40 W/m²/bar², held at 10 W/m² (the
   window saturates). Wordsworth & Pierrehumbert (2013): ≤ 2–3 K at today's N₂ with 10% H₂,
   10–15 K at 2–3 bar N₂, consistent with `p_H₂ p_N₂` scaling. N₂ is the background pressure less
   the biosphere's O₂.
9. **Lifetimes.** The linear O₂ scaling is only meant for the end states (anoxic, today);
   Goldblatt et al. (2006) find it non-monotonic in between. The anoxic lifetime itself depends
   on the CH₄ and H₂ fluxes. Caveats.
10. **CO assumed to oxidise.** Large CO fluxes against the OH supply can build up CO (a "CO
   runaway"; Kharecha et al., 2005; Zahnle et al., 2006): a caveat for the reduced-mantle run.
11. Budgets and lags: confirmed closed.
12. The spreading-rate bins were kept (already implemented, no effect on the state).
13. Evaluation additions: an energy-limited case (the reduced mantle), H₂ in bar and as a share
   of the surface pressure, and a `--no-carbon-cycle` run.
