# Review of notes/issue-154/plan.md (carbon cycle, barren planet)

Reviewer: independent agent, 2026-11-27. Based on the plan, `src/planet.rs`, `src/temperature.rs`,
`src/erosion.rs`, `src/presets.rs`, `src/tectonics/{ledger,mantle,boundaries}.rs` and the #190
default log (`~/tasks/issue-190/runs/new/seed-0/log.txt`). No code built or run. Literature
values marked "(mem.)" here are from my memory too.

Overall: the structure is sound (one conserved exogenic reservoir, sources tied to tectonic
rates, MAC-type land weathering, seafloor weathering, implicit solve). Stoichiometry,
partitioning numbers, the forcing fit and most literature values check out. The main problems
are the calibration on the first step, the stability argument (it ignores the EBM's lag), and
an expected regime (φ ≪ 1, supply-limited land, seafloor-dominated sink) that the plan does not
anticipate and that changes what the evaluation should look for.

## Findings

**1. Blocker: do not calibrate φ on the first step.**
At t = 0 the plates are at rest (log: mean speed 0.0, ridges 0 km). New sea floor appears only
when plates have opened a gap of about one cell (~110 km), i.e. after ~2 Myr at 5 cm/yr, so
`new_sea_floor` (and with it the ridge source, ~30% of D, and the whole seafloor sink, which is
∝ M_ridge) is ≈ 0 or quantised in step 1–2. Erosion is also in its initial transient (first 50
Myr: land mean 1024 → 473 m, denudation 18.9 Gt/yr with 61% reworked cover vs ~14–22 Gt/yr and
~35% later). φ fitted to step 1 would bake these artefacts in, and the seafloor sink switching
on in step 2–3 would make CO₂ drop at the start.
*Recommendation:* a calibration window: keep the forcing fixed (as with the switch off) for the
first K Myr (K ≈ 10–20), accumulate D and W(φ) per step, then solve Σ W(φ) = Σ D over the
window and start the carbon cycle at t = K with N from the current forcing. Log φ and the
window's D/W. Report the remaining start transient (erosion relaxation to ~50 Myr) as such.

**2. Major: the stability argument assumes the EBM equilibrates within one step; it does not.**
`years_per_update` = 2 with a 50 m mixed layer (C = 2.1 × 10⁸ J/m²/K, ~69% ocean): the global
relaxation time C/B is ~2–3 yr, ice albedo uses the previous year's mean. So one update
delivers only a ≈ 0.5–0.6 of the equilibrium response. With the lag, the error dynamics of the
plan's scheme are 2-D (error e and disequilibrium d = T_eq − T):
e' = (1 − a k) e − a d, d' = (1 − a) k e + (1 − a) d, k = λ_eq/λ.
Determinant 1 − a, trace 2 − a(1 + k): for a = 0.5, k = 1 the eigenvalues are complex with
modulus √(1−a) ≈ 0.7 and a period of ~8 steps. Stable (for k < (4 − 2a)/a), but **oscillatory
for any reasonable λ**: the claim "monotone for λ ≥ λ_true" is wrong. Matters in the fast
(modern, τ ≪ dt) regime; at epoch 0 the reservoir (τ_fb ~ 15 Myr) hides it.
*Recommendation (simplest robust):* predict the **equilibrium** temperature, not the next
update's: ΔT = λ (F(x) − F_now + ΔS_abs + N_toa), with N_toa the last update's global annual
mean top-of-atmosphere imbalance (absorbed − OLR, available inside the EBM). Then the solve
targets T_eq, the plan's (1 − λ_eq/λ) argument holds for T_eq, and T just relaxes toward it
monotonically. Alternative: more EBM years per update when the carbon cycle is on (cost; ice
memory still lags). Measure λ_eq and a (response after 1, 2, 5 updates to a ±4 W/m² step), and
make the toy-stability test a lagged 2-state climate (a = 0.5), not only λ_true = 1.5 λ. Log
predicted vs actual ΔT per step.

**3. Major: expect φ ≪ 1, supply-limited land and a seafloor-dominated sink at epoch 0.**
Rough numbers from the #190 log: bedrock denudation ~10–15 Gt/yr → land supply ceiling
~15–20 Tmol/yr (χ = 1.3, plus f_sed). Kinetic land at φ = 1: 0.25 (b) × 42 (r^0.5 at r = 1770)
× ~5 Tmol/yr ≈ 50 Tmol/yr, i.e. x = w_kin/w_sup ≈ 2–3, fully supply-limited. Seafloor at φ = 1:
c_kin = 0.023 × 5.6 ≈ 0.13 mol/kg vs c_max 0.26 → c ≈ 0.10 mol/kg × 80–140 Gt/yr ≈ 8–14
Tmol/yr, already ≥ D. W(φ = 1) ≈ 25–30 vs D ≈ 7–10 → **φ ≈ 0.15–0.3** (order of magnitude).
That is consistent with the plan's own remark that the extrapolated fit underestimates the
forcing (RCMs: 0.1–0.3 bar), i.e. the discrepancy is mostly in the forcing law, not the
weathering laws. Consequences: (a) the land sink's sensitivity is cut by the supply
saturation (d ln w/d ln w_kin = x e^{−x}/(1 − e^{−x}) ≈ 0.3 at x = 2); (b) when seafloor
dominates, both source and sink scale with M_ridge, so CO₂ hardly responds to spreading-rate
changes (only via the non-ridge sources and land) — an outcome to explain, not a bug.
*Recommendations:* (i) before coding, estimate W(φ = 1) offline from a #190 snapshot/log and
put the expected φ in the plan; (ii) offer Sebastian a third option: keep φ = 1 and calibrate
the **forcing**: solve r₀ from W(r₀) = D at the start climate, then F(r) = F_start +
F_fit(r) − F_fit(r₀) (continuous start, literature sinks, the uncertainty sits where it is
largest); (iii) cap φ far below 10⁴ (e.g. 10², warn): a huge φ saturates every sink and
removes the feedback entirely; with seafloor c_max · M ≈ 20–35 Tmol/yr > D the cap should never
be needed anyway; (iv) log the land/seafloor split and the supply-limited share of land
weathering.

**4. Major: arc return of slab carbon is not tied to seafloor uptake.**
The arc slab-crust coefficient (0.025 mol/kg, from today's altered crust ~0.1–0.2 wt% CO₂) is
today's steady state of seafloor weathering (c₀ = 0.023 mol/kg). At epoch 0 seafloor uptake is
c ≈ 0.05–0.1 mol/kg, but the return stays 0.025: the net seafloor sink is overstated and the
crust's carbon disappears. Krissansen-Totton et al. (2018) let the subducted crust carry what
seafloor weathering put into it.
*Recommendation:* arc slab-crust source = M_sub × (c_primary + f_ret · c̄), with c̄ the
seafloor uptake per kg averaged over the last ~mean subduction age (an exponential running
mean with ~60–80 Myr timescale, one extra state number) and f_ret ≈ 0.5 (fraction released
at arcs, rest to the mantle); choose c_primary so that today's numbers stay ~0.025 mol/kg.
Same logic (optional) for sediment carbonate: 3–10 wt% are consistent with burial today
(W ≈ 8 Tmol/yr ≈ 0.8 Gt CaCO₃/yr in ~15 Gt/yr sediment ≈ 5 wt%) — state this check in the plan.

**5. Major: the expected temperature response to the brightening sun is too optimistic; β =
0.5 at r ~ 10³ is questionable.**
Equilibrium with kinetic land only: 0.5 Δln r + 0.092 ΔT = 0 and ΔT = λ (ΔS + 11 Δln r); with
λ = 0.6 and ΔS ≈ 16 W/m² (the 7.6 K drift), ΔT ≈ +4 K — the thermostat removes only ~half of
the warming (less in the supply-limited regime). "The temperature stays roughly constant" is
the wrong expectation. GEOCARB's r^0.5 was fitted for a few–20 PAL; at 1000+ PAL the carbonic
acid effect enters through pH ([H⁺] ∝ p^0.5, silicate rates ∝ [H⁺]^~0.5 → ~p^0.25), close to
WHAK's 0.3 and the seafloor's 0.23.
*Recommendation:* β = 0.3 abiotic (WHAK; state range 0.2–0.5), and a small sensitivity set
(β 0.3/0.5, Ea 40/75, seafloor off; 2–4 seeds × 300 Myr). Write the expected ΔT per Gyr into
the plan so the evaluation has a target.

**6. Major: `--no-erosion` (and the web panel's erosion toggle) leaves the land sink at zero.**
`ErosionResult` does not exist then; w_sup = 0. Define it: either kinetic-only land weathering
without the supply limit (and say so in the log), or the carbon cycle is forced off with
`--no-erosion`. Test the combination.

**7. Minor: ocean partitioning exponent.** `p^{1/3}` holds when Ca²⁺ is set by alkalinity
(Ca ≈ alk/2). Today Ca²⁺ (10 mM) ≫ alk/2 (~1.2 mM), so at fixed Ca and calcite saturation
[CO₃²⁻] is fixed and DIC ∝ p^{1/2}; the regime moves toward p^{1/3} only when HCO₃⁻ exceeds
~2[Ca²⁺]. Effect: N_oc at epoch 0 between 2.4 × 10¹⁹ (p^{1/3}) and ~8 × 10¹⁹ (p^{1/2}), i.e. a
factor ≤ 1.5 in N and τ, nothing in equilibrium. Keep p^{1/3}, but describe it as a choice and
mention the range. Also at 0.5 bar CO₂ the mean molar mass of the air rises (M̄ ≈ 0.033);
negligible (12%).

**8. Minor: source noise in the fast regime.** With τ ≪ dt (modern) CO₂ follows D each step;
D is grid-quantised (ridge cells, stacking bursts: collision stacking 5–29 Gt/yr between
snapshots). ±20% D noise → ~±0.4 in ln r → ~±2 W/m² → ~±1 K step-to-step noise. Measure it;
if large, smooth D with a 3–5 Myr exponential mean (defensible: melt/slab transport delays) —
done identically in the off mode's logging.

**9. Minor: lithology.** Interpolating by bedrock density (2750–3000) treats arc crust (2800)
as felsic, though arcs are andesitic/basaltic and among the fastest-weathering terrains.
The crust carries a `felsic` fraction (in the state hash) — use L and χ from it instead.

**10. Minor: seafloor temperature clamp.** T_d = max(−1.8 °C, polar ocean surface mean): with
cold or ice-covered polar seas the seafloor sink loses all temperature sensitivity (only
r^0.23 remains). Fine physically (deep water cannot be colder), but it makes a cold, seafloor-
dominated state a possible stable equilibrium (see 14). Use the open-water/mixed-layer
temperature, not the sea-ice surface, if the field distinguishes them.

**11. Minor: runoff sensitivity ε.** The model's ocean evaporation is +5%/K, so its runoff
sensitivity may differ from 0.03/K. Measure ε together with λ from the same ±4 W/m² runs.

**12. Minor: bit-identity and determinism.**
- Changing `--co2` to the quadratic law changes runs with `--co2` > 1 (and `--set` paths using
  it) even with the switch off: "off is bit-identical" holds only for the default/forcing
  paths. Say so, or keep Myhre when off. Pinned hashes in tests will change for the default on.
- Off mode must not mutate anything (no `Simulation` accessor with side effects, no RNG use);
  add a test comparing state hashes of off vs the pre-change code path on a short run.
- Add N (or the forcing) to `state_hash` only when on, so off stays identical.
- Sums over cells in index order; `powf`/`exp`/`ln` via `crate::math`; `sqrt` is IEEE-exact.
  Cost: 60 bisection iterations × ~12k land cells × ~3 transcendental calls ≈ 2M calls/step,
  acceptable even with `--web-math`; bracketed Newton/secant or 40 iterations suffice.

**13. Minor: details to fix in the plan.**
- `bedrock_delta` is metres **per step**; divide by dt_years. Use only cells that are land at
  the step's start (wave/cliff erosion and submarine erosion are not continental weathering).
- Check the ledger/`TrenchSediment` counters are cumulative and never reset (statistics,
  snapshots, merges); `TrenchSediment` is kg/m² × cell area.
- Stacked sediment becomes metasediment; make sure later burial metamorphism does not count the
  same mass again (once decarbonated, not again).
- Bisection bracket: e.g. ln r ∈ [ln 10⁻³, ln 10⁵]; log when a bound is hit; never let the
  forcing exceed the EBM's sane range silently.
- Seafloor uptake is computed from today's M_ridge and conditions while real uptake happens
  over ~10–20 Myr of aging; fine at this level, mention it.
- The panel checkbox is UI: follow the UI guidelines (touch-sized targets, responsive panel).

**14. Literature flags (mem.)**
- c₀ (1.5 Tmol/yr modern seafloor uptake): Alt & Teagle (1999) is at the high end; Krissansen-
  Totton & Catling (2017) infer roughly 0.5 Tmol/yr (factor ~3 lower). φ partly absorbs it,
  but it sets the land/seafloor split and hence the feedback strength → sensitivity run.
- Gaillardet et al. (1999) 11.7 Tmol CO₂/yr includes Na/K silicate weathering (no carbonate
  burial); Ca + Mg part ~70–80%. Within the stated uncertainty.
- von Paris et al. (2008) argued for much lower CO₂ (mbar level) at the late Archean than
  Kasting/Haqq-Misra; don't cite it as supporting 0.1–0.3 bar.
- Others (Myhre 5.35; Byrne & Goldblatt fit; Ea 42–74 kJ/mol; GEOCARB f_E = 0.25, runoff 0.65,
  (2r/(1+r))^0.4; Dessert basalt factor; Kelemen & Manning arcs 18–43 Mt C/yr; Marty &
  Tolstikhin; Burton ~12 Tmol/yr; E_sf 92 kJ/mol; K₀ ≈ 0.04 at ~10 °C; DIC 2.0–2.25 mM) look
  right to within a factor 2. Atmospheric and ocean inventories (5 × 10¹⁶, 2.9 × 10¹⁸ mol) check.

**15. Evaluation: add these.**
- **Recovery/impulse test:** with φ fixed from the default run (`--set carbon.phi=…`), start
  at ±10 W/m² off the calibrated forcing (or inject a carbon pulse, e.g. a one-off
  `carbon.pulse`) and measure the e-folding return time vs τ_fb (§1) at epoch 0 and modern.
- **Snowball test:** start cold (−15…−20 W/m², φ fixed) and check whether CO₂ builds up and the
  planet escapes; log the seafloor share in the cold state (finding 10: seafloor weathering
  continuing under ice could pin a snowball).
- **Stability diagnostics:** predicted vs actual ΔT per step, sign changes of ΔF, step-to-step
  T noise on vs off, especially in the modern run.
- **Tectonic response:** lagged cross-correlation of ln r and T with D, M_ridge, land area,
  supply (bedrock denudation) and the largest-continent share; decompose Δln r into source
  vs sink drivers; CO₂ should lag D by ~τ_fb.
- **Budget closure** in the log: N_new − N_old = dt (D − W) each step (to rounding).
- **φ across seeds** (spread shows how start-dependent the calibration is), the land/seafloor
  split and the supply-limited share over time.
- The small sensitivity set from 5 (β, Ea, seafloor off, c₀ ÷ 3).
