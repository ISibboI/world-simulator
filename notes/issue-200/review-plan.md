# Review of notes/issue-200/plan.md (independent, read-only)

Reviewed against the worktree at 2c41507 (+ uncommitted start of `Cohort::carbonate` in
crust.rs/boundaries.rs). Overall: the design is sound and fits the code well (the Cohort tracer
really does reach every lateral sediment path: sheet mixing, gap fills, rift fills, crustal flow
in topology.rs:99, diffuse-shortening additions via `CrustAddition::cohort`). The findings are
mostly about closing the carbon books exactly and about two places where the label would drift.

## Major

M1. **Smoothing is not a conserving buffer; `C_transit` as "difference" can go negative.**
`CarbonCycle::step` smooths *rates* by exponential relaxation (`old.towards(raw, w)`, w = 1−e^{−dt/τ}),
and `end_hold` resets the smoothed rates to the calibration-window mean. Σ smoothed·dt ≠ Σ raw·dt,
and the jump at `end_hold` creates/destroys carbon. A residual "in transit" can therefore become
negative (mantle-supplied carbon) and is not a reservoir. Change: for the carbonate-derived arc and
metamorphic sources use a real reservoir per process: `B += returned carbonate (mol)` each step,
degas `B·(1−e^{−dt/τ})`, `B −= that`. Exactly conserving, never negative, same 3 Myr smoothing.
During the hold the same buffer still drains (sources are computed, just not applied to N). Keep
rate smoothing only for the switch-off path (bit-identity).

M2. **Marble and accreted-wedge carbonate (`C_crust`) is a one-way sink, not a reservoir.**
Burial metamorphism (`metamorphose_sediment`), collision stacking (boundaries.rs ~388, sediment →
crust at the same mass) and accretion (Accretion addition, `sediment: 0`) turn sediment into
crystalline crust, which has no carbonate label. Later that crust is eroded as bedrock (treated as
silicate: it even feeds the silicate land sink with `felsic_cations`), subducted (the lower
plate's `metasediment_mass()` goes down at subduction, subduction erosion thins from below), or
stacked again — none of which can draw from a global `C_crust`. With ~0.5–1 Tmol/yr going in it
reaches ~10²⁰–10²¹ mol per Gyr, ≫ the exogenic reservoir. Change (pick one, say which in the plan):
(a) minimal: call it what it is, a sink "carbonate locked in crystalline crust", reported in the
partition, and list re-exhumation/subduction of marble as a known gap/follow-up; or (b) add a
`Crust::marble` mass-fraction tracer of the crystalline crust next to `metasediment` (mixed by mass
wherever `metasediment` is; all CrustAddition sites already carry `metasediment`), so bedrock
erosion of marble is recycled carbonate (and excluded from the silicate supply) and subducted marble
feeds `slab_carbonate_return`. (b) is the physically consistent one but touches every Crust
constructor; (a) is acceptable for this issue if stated in the conservation equation.

M3. **Wave reworking will churn platform carbonate and move the label off the slope.**
Storm wave base is −30…−100 m open / −5…−30 m sheltered (erosion.rs:99–106), but precipitation
fills room up to −5 m. Everything built between the wave base and −5 m is reworked next step:
removed with its carbonate fraction (counted as recycled, redissolved) and redeposited downslope as
*clastic*, while the same mass reprecipitates on the platform top. Mass/carbon stay closed, but
(i) F_recycled becomes a large self-induced flux, (ii) the label piles up on the shallowest warm
cells and slope/rise aprons (the part that actually reaches trenches) stay unlabelled, biasing the
arc CO₂ low. Change: cap `room` at the cell's wave base (`erosion::wave_bases`, which the planet
already computes), not at a fixed −5 m; log F_recycled/F_new and the wave-reworked carbonate
separately. Optional (larger): carry a carbonate fraction through `erode()`'s routing with a
dissolution fraction, so detrital carbonate keeps its label.

M4. **Recycled-carbonate measurement must exclude metamorphism.** `apply_surface_changes` ends
with `metamorphose_sediment()` (mod.rs ~1912). A before/after inventory around the call (as
planet.rs does for `cover_cohort_eroded`) would count burial decarbonation/marble as eroded
carbonate (recycled to the sea, CO₂-neutral) — double counting with the metamorphic source. Change:
have `apply_surface_changes` return/accumulate the carbonate removed by its own surface part
(before metamorphism), or split the metamorphism call out; counter for metamorphism separately.

M5. **Solve S over the samples, not the cells.** §2c says `Σ_cells min(R·area·years, room) = F`,
but then applies per sample with one cell of area each (as `apply_surface_changes` books it).
Those two sums differ wherever samples are shown at k > 1 cells, so F would not be met. Change:
build the list of shown samples (mean of their cells' g = e^{γ(T−T₀)−z/z_p}, A(T), room) and do
the bisection on that list; then "deposited == F" is exact by construction. Precompute g and A
per sample so the bisection loop has no transcendental calls (fixed ~60 iterations, cheap).

## Minor

m1. **Hold rule uses the φ = 1 split.** During the hold `sinks` run at φ = 1; land is
supply-limited (saturating `exp_m1`) and seafloor is capped, so `land/(land+seafloor)` at φ = 1 is
not the split at the calibrated φ. Harmless (only the hold), but state it; or after `end_hold` log
the split at the calibrated φ for comparison.

m2. **Collision decarbonation needs its own ledger term.** "inside collision consumed/stacked"
breaks the consumed ↔ stacked mass pairing in the ledger and statistics. Use one
`Term::Decarbonation` for both burial and collision (recorded where the CrustAddition is built).

m3. **Count carbonate losses explicitly where cheap.** Merges (`merge_plates`), `remove_plate`,
`AdditionsLost`, the thickness cap in `apply_additions` and `clamped` in surface changes all drop
sediment. Rather than one residual, record carbonate at the same `ledger.record` sites (a small
parallel carbonate ledger keyed by the same `Term`); the residual then really is rounding.

m4. **Remove the clamps from the carbonate bookkeeping.** The in-progress `add_sediment`,
`from_amounts`, gap/rift fills clamp the fraction to [0,1]; a silent clamp hides conservation bugs.
Use `debug_assert!` on the range and clamp only in `remove_clastic` (fraction after removing
clastic can round above 1).

m5. **Regolith vs. well-mixed label.** §2a says regolith is "the non-carbonate bottom part", but
`remove_sediment_to` removes carbonate in proportion, so eroding regolith (excluded from
`sediment_erosion` in planet.rs ~930) still removes carbonate that is counted as recycled. Either
drop the claim (regolith is part of the well-mixed column) or clamp regolith ≤ clastic and route
regolith removal through `remove_clastic`. The first is simpler and consistent.

m6. **Which deposits lose the dissolved load.** "In proportion to the step's positive sediment
changes" includes land deposits (alluvium, lake fill, terminal fans). The dissolved load ends in
the sea: take it from ocean-cell deposits first (`result.water.ocean`), land only as fallback.

m7. **Subducted carbonate counter.** Today `subducted_sediment` excludes subduction erosion (that
is `Term::SubductionErosion`). Make the carbonate driver = (sediment − accreted_sediment)·f of the
lower plate + `from_sediment`·f of the eroded forearc (boundaries.rs ~535), and say so; also note
that the lower plate's metasediment (marble, M2) is not counted.

m8. **Mg carbonate.** Land sink is Ca+Mg; MgCO₃ 0.084 kg/mol, dolomite 0.092 per C. 0.1 slightly
overstates mass (≲ 10%); fine as a parameter, mention it.

m9. **State hash.** `Planet::state_hash` does not include the Cohort, so the carbonate field is
invisible to the web/native identity check when on. Hash `cohort.carbonate` only when
`carbonate_sediment` is on (keeps the switch-off hash identical to #224).

m10. **Switch-off bit-identity — watch the details.** Adding a 0.0 tracer is exact, but: no new
calls that mutate sediment (e.g. `remove_clastic(0)`, extra `solve_sea_level`) on the off path; new
ledger/budget terms must not enter any existing summed total that feeds the state (they don't
today: ledger is diagnostic), and `-0.0` vs `+0.0` sums in logs are fine. Add a test: off mode
gives the same state hash as without the feature for a few steps.

m11. **Evaluation orientation.** Add Earth's GLOSS (Plank & Langmuir 1998: ~7 wt% CaCO₃ in
subducting sediment) for the modern run, and for barren an expected range of the solved Ω on the
warmest shelves (Precambrian ~3–10) — both are already near the plan, just make them pass/fail
checks. Report F_recycled/F_new (M3).

m12. **Literature** citations check out by title/venue (Ridgwell & Zeebe 2005 EPSL 234;
Higgins et al. 2009 EPSL 284; Grotzinger & James 2000 SEPM SP 67; Zhong & Mucci 1989; Burton &
Walter 1987; Milliman 1993 GBC; Kelemen & Manning 2015 PNAS; Dasgupta & Hirschmann 2010).
Parameter checks: γ = 0.03/K gives 1.5→4 over ~33 K (ok); z_p = 5800 m gives ×2 at ~4 km (ok);
k₀ = 0.2 with n = 2 gives 0.8–3 mol/m²/yr at Ω = 3–5 (matches GBB ~1.5). Note that homogeneous
abiotic calcite nucleation needs Ω ≳ 10–20 (Mg inhibition), so k₀(Ω−1)² without threshold is a
heterogeneous/seafloor-cement law — consistent with Higgins et al., worth one sentence.

## Simpler alternatives considered

- The Ω model with a solved S is already cheap and gives a saturation horizon for free; a pure
  weighting (∝ A·g with caps) would be marginally simpler but loses the threshold. Keep it.
- For M1, the reservoir buffer is *simpler* than reconciling smoothed rates with a residual.
- For M2, option (a) is the simple path; take (b) only if the evaluation shows marble ≫ C_sed.
