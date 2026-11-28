# Review of notes/issue-106/plan.md (volcanism phases 0–2)

Reviewer: independent agent, 2026-11-27. Code read at f32d0e0 (+ uncommitted work in the worktree, not reviewed).
Literature values are **from memory**. Overall: sound plan, numbers mostly within a factor ~2 of what I
recall; the main risks are the thickness cap, piling under slow plates, the 12 km "oceanic" cutoff and RNG.

## Must-fix
1. **Clamp the addition, don't add-then-cap.** `Crust::add_crust` thins to `max_thickness` *from below with
   the mean composition* (`thin_from_below`), so where the cap binds on continental crust, felsic basement is
   destroyed and the ledger term "actual mass change" hides it (the felsic material budget shows a leak outside
   any phase). Add `h' = min(h, max − thickness)` instead; then the term equals what was emplaced, felsic mass is
   exactly conserved, and test 2 can assert exact volume even with the cap (report the clipped volume).
2. **Separate RNG stream for the LIP draw.** Drawing ε from `self.rng` after `mantle.step` shifts every later
   plume birth/split, so branch vs main is only statistically comparable *and* `volcanism.enabled=0` is not an
   exact ablation unless the draw is skipped. Use a dedicated `ChaCha8Rng` derived from the seed (or a hash of
   seed + plume birth time/index) so plume births, rifts etc. stay identical to main. Then add the test
   "enabled=0 ⇒ same state hash as without the module". Box–Muller: guard u = 0 (`ln`), use `math::ln/cos/exp`.
3. **Plateaus become "transitional" at > 12 km** (`MAX_OCEANIC_THICKNESS`, `is_oceanic_column`). Not just a
   statistics label: `erosion.rs::zones` treats every non-oceanic cell as ring 0 (shelf), so plateaus far out
   become margins that shift slope/rise/fan zones around them; `events.rs` excludes them from subduction
   initiation; `sheet.rs` ridge push only on oceanic; `planet.rs::crust_class`. Decide per use (probably fine for
   initiation, wrong for sediment zones) and list it in Risks; check one seed's zone map around a plateau.

## Should
4. **Piling under slow/stationary plates.** Tail at fixed position × 150 Myr lifetime: on a near-stationary
   continent 3×10⁴ km³/Myr ⇒ ~4.5×10⁶ km³ under one ~1.4×10⁵ km² footprint ⇒ ~30 km extra mafic crust (+3–4 km
   elevation, felsic dilution toward the 0.5 `is_continental_origin` threshold). Earth: Hoggar/Tibesti Cenozoic
   volumes are ~10⁵ km³-scale (memory, uncertain) — ≲ 10⁴ km³/Myr. Same at a ridge: L = 1 at age 0 gives 0.4
   km³/yr and ~30 km at slow spreading (Iceland 25–40 km total: OK, but fast-spreading should be less). Add an
   evaluation metric "max cumulative tail thickness per sample" and a test with a stationary plate; consider a
   smaller continental tail (e.g. z_L for tails = lithosphere thinned by the plume is *not* modelled, so 0.077 is
   generous rather than stingy) or let crustal flow (>40 km) be the safety valve and say so.
5. **Carbon per kg is inconsistent between head and tail.** #196's plume term (1 Tmol/yr for Σflux 0.036) over the
   plan's tail magma (0.5–1 km³/yr ≈ 1.5–3×10¹² kg/yr) implies ~0.3–0.7 mol/kg (1.5–3 wt% CO₂), 5–10× the LIP's
   0.07 mol/kg, and the implied per-kg varies 9× with L because #196 ignores the lid. Not a double count, but
   state it in the notes (plume CO₂ ≈ Marty & Tolstikhin-type estimates of ~1–3 Tmol/yr include deep low-degree
   melt degassing that never becomes crust). 0.07 mol/kg itself is fine (0.3 wt%; basalt 0.1–0.5 wt%; ridge in
   model 0.03 mol/kg — plume sources are enriched). Also the head-ramp (0–10 Myr) plume CO₂ has no crust: fine,
   document it.
6. **Calibration window**: LIPs between 100–150 Myr enter #196's φ calibration as single-step pulses (a 5×10⁷ km³
   plateau ≈ 10 Tmol/yr in its step, +0.2 Tmol/yr over the 50 Myr window, ~3%). Acceptable; mention, or smooth the
   LIP over its 1–3 Myr eruption (also more physical than one step).
7. **Footprint shape for big plateaus.** σ = 400 km gives 50 km at the centre for 5×10⁷ km³; Ontong Java is
   flat-topped (+20–25 km over ~2×10⁶ km²). Widen with volume: σ = max(400 km, √(V / (2π·25 km))) (≈ 560 km for
   5×10⁷), keeps OJP-like plateaus below the cap and crustal-flow threshold. Use chord distance |a−b|·R for the
   Gaussian (no transcendental, <0.1% error at 1000 km).
8. **Continental crust budget.** Thick plateau centres (≳ 19 km) accrete at trenches: realistic (Wrangellia,
   Caribbean), but at ~0.5 km³/yr LIP output it can be a sizable mafic addition to a continental balance tuned to
   −0.97 Gt/yr felsic+sediment. Evaluation should report accreted plateau mass/yr and felsic fraction of
   continents vs main.
9. Lid: using `Crust::age` (not strength age) is right — the head has reheated the cell for 10 Myr, so strength
   age would make every LIP see L ≈ 1. Z = 180 km is at the generous end (dry solidus at ΔT ≈ 200–250 K ~150–170
   km from memory); the square law is a reasonable White & McKenzie/Ellam-style simplification. Volumes:
   lognormal *mean* is 1.94× the median; long-term LIP output ≈ 9×10⁶ km³ / 15 Myr ≈ 0.6 km³/yr — plausible order
   for Earth's Mesozoic–Cenozoic LIP average (memory: ~0.5–2 km³/yr incl. plateaus), CO₂ ~0.1 Tmol/yr mean.

## Nice
10. Sea level: book the volcanism step's change in its own `sea_level_budget` term (it is solved right after
    `mantle.step`, so it would land in `tectonics`). Size: ~4 m per 10⁷ km³ plateau, tens of m of stock.
11. Continental z_L could shrink with `rifted` fabric (LIPs precede breakup; volcanic margins) — follow-up.
12. Volume normalisation must use the same uniform cell area as `ledger`/`carbon_drivers`; iterate cells in
    index order (no hash maps) for the sum. `Volcanism` must be `Clone` for the viewer history.
13. #217 (primed plumes) is not on main: the "old plume, tail only" path needs a constructed-plume test.

## Missing tests
- enabled=0 bit-identical to main (state hash); cap clamp conserves felsic mass exactly;
- log-normal draws: median/σ/clamp over many seeds; stationary-plate pile bound; tail at a ridge;
- footprint over a plate boundary and with hidden samples (each visible sample once); `--web-math` hash equality.
