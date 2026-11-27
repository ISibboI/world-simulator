# #213: start near the tectonic steady state, explicit ramp-up phase (with UI) — plan

Stacked on #196 (`agent/issue-154`). Analysis: [initial-tectonics.md](../issue-154/initial-tectonics.md).
Goal: an Earth-like planet, not Earth. Targets are the model's *own* steady state; Earth
numbers (Bird 2003) only orient the shape of the plate-size distribution.

## Steady state of the base (#196 head, 8 seeds, 300–1000 Myr, from the #154 runs)

27 plates (17 of them > 1% of the surface), largest ~20%, mean ocean age 54 Myr, ~9 plumes,
degassing 14–15 Tmol C/yr. A new 8 × 1000-Myr base run on afc4046 with `--plate-log 10`
(`~/tasks/issue-213/runs/base`) gives the rank–area distribution of the steady plates and
the per-step degassing, i.e. the reference for everything below.

## 1. Initial state (option A + C + D of the analysis)

**A. Plate count and sizes.**
- Default `--plates` 12 → about the steady count (≈ 25; final value from the experiments
  below, chosen so that the plate count neither ramps nor overshoots).
- `plate_partition`: the target areas become the model's steady rank–area shape instead of
  Zipf `rank^-0.9` (which for 25 plates gives only ~4 plates < 1% vs ~10 in the steady state
  and Earth's "few large, many small", Bird 2003). Parametrised by rank so it works for any
  `--plates` and any continental fraction (#208); random scatter kept. The algorithm
  (warped, weighted Voronoi flood, continent cohesion, weight iteration) stays.
- Ocean ages: keep the triangular age–area distribution, but check the mean against the
  steady 54 Myr (60 now); only change the maximum age if the runs show a clear transient.

**C. Steady plume population.** At the first mantle step (`MantleFlow::prime`), draw the
plumes that a steady birth process (one per `plume_interval`, lifetimes in
`plume_lifetime`) leaves alive: walk back over the longest lifetime in 1-Myr steps, a birth
with probability dt/interval each, kept if its age < its lifetime (≈ 9 plumes, ages spread).
Positions by the same rule as new plumes (farthest of the candidates from downwellings, after
D). Plumes older than `rift_delay` (20 Myr) count as having rifted already (they made the
existing plates), so there is no burst of plume rifts at t = 1; younger ones rift as usual.

**D. Pre-filled mantle memory.** At the same first step, with the first solve's trench rates:
downwellings = their steady value (the step's subduction rate), sunken-slab stage k =
mass flux × residence_k (the steady content of a chain of stages), i.e. as if the trenches had
been active for ≥ 150 Myr. One parameter `mantle.prime` (1 = on, 0 = the old empty start)
for comparison runs.

C and D are kept only if they help measurably (the brief: cheap and clearly helpful).
B (motion-consistent boundaries/ages) and E (crust history) are out of scope.

**Experiments** (via `runs`, 4 seeds × 200 Myr, snapshots every 10 Myr): plates 20/25/28
with the new sizes; with and without C+D. Measures: plate count, plates > 1%, largest plate,
mean ocean age, plumes, degassing by source per 10 Myr, vs the steady state. Pick the
variant whose plate count and degassing are flat (within noise) earliest; the supercontinent
preset checked separately (4 seeds).

## 2. Explicit ramp-up phase

New module `src/ramp_up.rs`: `RampUpParams` (`--set ramp_up.…`) and `RampUp`, owned by
`Planet`.
- Each step it records the plate count and the tectonic degassing (ridges + arcs w/o the
  seafloor-carbon return + plumes + metamorphic: independent of φ).
- From `min_duration` on, after every step it compares the time-weighted means over the last
  `window` W and the window before. The phase ends when both relative changes are below
  their tolerances (`degassing_tolerance`, `plate_tolerance`). Safety net: `max_duration`
  (150 Myr, the old fixed hold) — ends with a warning. Tolerances and W are set from the base
  and new runs, offline: the steady-state noise must pass reliably, a clear ramp (the base
  12-plate start, ~10%/20 Myr) must not. Deterministic: only integer counts and the
  simulation's own rates, no clocks or hash orders.
- Logging: start (with the criterion), one line per window during the phase (means and
  changes), end (time, means, which criterion, or the safety net).
- Carbon cycle: `calibration_time` is removed. The cycle holds the CO₂ while the planet says
  the ramp-up runs, keeping a rolling buffer of the last `calibration_span` Myr of steps
  (default 50 → ~20 Myr, the phase is shorter); `CarbonCycle::end_hold()` calibrates φ on
  that buffer when the phase ends, exactly as the fixed time did. With `carbon.phi` given,
  nothing is held (as now); with `--no-carbon-cycle` the phase is still tracked and logged.
- Expected end with the new start: ~30–50 Myr; the base start should end much later or at
  the safety net (a check that the criterion measures something).

## 3. UI

- `SurfaceUpdate` (desktop) and the worker message (web) carry a small `SimulationMeasures`
  / global status: the ramp-up state (running + what is held, or ended at t). It is a
  property of the run, shown next to "t = X of Y Myr", which is also the run's (not the
  shown frame's) time — so it does **not** go into the history frames; no codec change.
- Parameter panel: a small "global status" section under the time line (a function #209 can
  grow into a statistics section): "Ramp-up (tectonics settling) — CO₂ held" with a hover
  text explaining the criterion, then "Ramp-up ended at X Myr". Plain labels: no new touch
  targets; font follows the egui/browser scale; on narrow screens the panel stays collapsed
  by default as now.
- Screenshots via vscreen: desktop 1280×800, web at 390×844 (panel opened), in
  `notes/issue-213/`.

## Evaluation

- 8 seeds × 1000 Myr default barren vs the base: ramp-up end, degassing over the first
  200 Myr (per 10 Myr, by source), plate count, ocean age, temperature and CO₂ over time,
  the final state.
- `--biosphere modern --epoch 4` (4 seeds), `--initial supercontinent` (4 seeds), 1000 Myr.
- Independent evaluation by a separate agent; results in `notes/issue-213/results.md`.
- Caveat for the PR: #195 lowers the steady plate count to 19–20; the default `--plates`
  may then need to follow.

## Tests

- `plate_partition`: connected plates, heavy tail (≥ ~1/3 of 25 plates < 1%, largest
  15–30%).
- `MantleFlow::prime`: ~steady plume count over seeds, old plumes marked rifted, slab stage
  masses = flux × residence, downwelling = rate.
- `RampUp`: ends on a flat signal after min_duration, not on a ramp, at max_duration on a
  ramp; deterministic.
- Carbon: hold → `end_hold` calibrates on the rolling buffer; conservation test adapted.

## Plan review (independent agent) and what changed

The review (13 findings) found the initial state sound; the two must-fix points were about
the criterion and the calibration window. Adopted:

1. **Criterion vs noise** (must): the base ramp (~10%/20 Myr) is about as large as the noise
   of two 20-Myr means of the raw plate count. Now:
   - checked once per non-overlapping window W (not every step), and the phase ends only
     after `passes` consecutive passing windows;
   - degassing one-sided: its growth must be ≤ tolerance (the transient only rises; a fall
     is not a ramp);
   - plate count as the effective number of plates `1/Σaᵢ²` (area shares), which tiny
     plates hardly move, two-sided (an overshoot also counts as unsettled);
   - tolerances from the 300–1000 Myr steady segment of the 8 × 1000-Myr base runs.
2. **Calibration window** (must): φ calibrates on exactly the criterion's last window W, the
   one shown to be flat; `carbon.calibration_span` is removed (the cycle keeps a rolling
   buffer of W). Order: the step that ends the phase is recorded first, then `end_hold`.
   The smoothing still starts from the window's mean rates.
3. A run shorter than the phase never sets φ (as before with < 150 Myr); the UI then still
   shows the ramp-up. Tested.
4. Priming happens after the first step's mantle step: the swells and slab loads appear at
   t = 1 (a sea-level jump in the mantle budget); measured and reported.
5. "Plumes ≥ 20 Myr have rifted" is documented as an approximation (in the steady state a
   few old plumes under small oceanic plates have not).
6. Slab priming test asserts the exact discrete fixed point `deposit/leave`.
8. Plate targets get a floor of a few cells, tested at 4 subdivisions; the UI's default
   `--plates` follows the CLI.
9. #195 dependency stated prominently in the PR.
10. Tests: `RampUp` on synthetic noise and on a slow ramp; hold → `end_hold` uses only W.
11. UI: the hover text is unreachable on touch, so the short criterion goes into the label
    itself. Shading the ramp-up span on the time bar: not done (kept minimal).
12. Variant choice on 4 seeds; tolerances only from the 8 × 1000-Myr base.

## Update: #195 merged into main (coordinator, during the work)

#195 (plume rifts by stress, rift propagation; the plume "rule" and `Plume::has_rifted` are
gone) lowers the steady plate count from ~28 to ~18 and raises the largest plate from ~20%
to ~34%. This branch stays stacked on #196 (no merge of main here), but what ships is #196 +
#195 + this. So:

- The plate-size shape and the default `--plates` are fitted to the steady state **with
  #195**, measured on `origin/main` (4 seeds × 1000 Myr, `--plate-log 10`,
  `~/tasks/issue-213/runs/main195`). The shape is parametrised by rank/count, so it is one
  fit, not per-count tuning; a script to refit it is in the notes.
- The evaluation runs use a local scratch merge of this branch with `origin/main` (not pushed
  as a branch; `runs` pushes its temporary `runs/<sha>`), against the same scratch merge of
  #196 with main (it merges cleanly). The branch alone (pre-#195) is checked for the
  ramp-up criterion and the CI.
- The priming's "old plumes have rifted" mark falls away with #195 (no `has_rifted`); the
  merge resolution drops it.
