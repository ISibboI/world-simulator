# Review of the #154 results (carbon cycle), independent evaluation

Reviewer: independent agent, 2026-11-27. I read the plan, the plan review, results.md, per-seed.txt,
fluxes.txt, analyse.py, `src/carbon.rs`, `Planet::update_carbon`, `presets::co2_forcing`, and the
`toa_imbalance` diagnostic in `src/temperature.rs`. I re-parsed all 32 logs (`runs/{on,off,modern,modern-off}/seed-0..7/log.txt`)
with my own scripts in `~/tasks/issue-154/tmp/rev/` (p.py parses the logs; a–h.py run the
analyses). I did not run any builds or simulations. The standard is physical plausibility for an
Earth-like planet, and artefacts of the ~110 km grid are accepted.

**Verdict.** The implementation is sound, stable and conserves carbon. Most of the table numbers
check out exactly. The problems are in the interpretation, not the code:

- The headline "~¼ of the warming remains" depends on the metric chosen. By endpoints or by
  forcing, about **half** remains.
- At the calibration step the modern runs jump by up to +5.4 W/m². The results do not mention
  this.
- After the window the spin-up continues and drives CO₂ up for 100–200 Myr. The results describe
  this only for the modern runs.

Nothing here blocks the PR, but these points should be fixed or stated in it.

## 1. Number check

My numbers match results.md for:

- **Temperatures:** t0, t150, the 200–1000 mean, the end values and the trends with their per-seed
  ranges, for all four sets.
- **Ice:** 1.7/1.2/6.8/4.9% mean over 200–1000; 2.0/0.5/6.5/1.8% at the end; at most 7.5/16.2% at
  t = 0.
- **pCO₂:** barren 0.326 bar at the end (0.26–0.39), range 0.25–0.74. Modern 76 ppm (13–147),
  peak 892 ppm.
- **φ:** barren 0.955–1.565, modern 3.50–8.15.
- **Forcing means:** 60.4 / −2.8 W/m².
- **The whole flux table**, including off at 1000 Myr (D 18.2, W 21.6 = 6.1 + 15.5) and the modern
  rows.
- **Lag-1 autocorrelation of ΔF:** barren +0.27, modern −0.26 (results: −0.24).
- **Snowball and runaway:** neither occurs. The coldest barren end is 15.1 °C with 4.1% ice; the
  hottest is 18.4 °C (modern 25.0 °C).

Small corrections:

- **minor** — "with it off, the sinks exceed the sources by up to 3.4 Tmol/yr at 1000 Myr": 3.4
  is the 8-seed **mean**. Per seed W − D at 1000 Myr is −0.2…+8.2 (barren) and +4.5…+11.7
  (modern-off).
- **minor** — Conservation: the barren reservoir change matches ΣΔt(D − W) to ≤ 1.1 × 10⁻³ of N.
  In the modern runs the cumulative residual is up to 5% of N, but every per-step residual is
  ≤ 1 × 10¹⁶ mol. That is the rounding of the logged N and D − W, and 2 × 10⁻⁵ of the integrated
  flux. So conservation is fine, but "0.1%" only describes the barren runs.

## 2. Findings

**1. Major — the compensation of the brightening sun is overstated ("~¼ remains", "drift mostly gone").**
The ratio of the 200–1000 Myr trends (1.35 vs 6.04 K/Gyr) is the most favourable of several
reasonable metrics. Other metrics (8-seed means; ΔS_abs = ΔS₀/4 × 0.7):

| interval | ΔS_abs | warming on / off | warming remaining | −ΔF_CO₂ / ΔS_abs |
|---|---|---|---|---|
| barren 150→1000 (cycle active) | 10.5 W/m² | 1.94 / 4.44 K | **44%** | **45%** |
| barren 200→1000 | 9.9 | 1.18 / 4.20 | 28% | 57% |
| barren 400→1000 | 7.5 | 0.50 / 3.70 | 14% | 67% |
| modern 150→1000 | 19.7 | 4.11 / 8.91 | **46%** | **45%** |
| modern 200→1000 | 18.6 | 2.48 / 8.49 | 29% | 54% |

Why the 200→ numbers look better: after calibration the CO₂ first **rises** (finding 3). Between
150 and 200 Myr the "on" runs warm by 0.76 K against 0.24 K for "off", so a trend that starts at
200 Myr begins from an elevated point. The off runs' warming also speeds up after 500 Myr as their
ice disappears.

Counted over the whole active period, the carbon cycle offsets **about half** of the solar
brightening, in both the barren and the modern case. That is exactly the plan reviewer's "about
half remains" and the finite feedback gain one expects.
*Recommendation:* in results.md and the PR, give the 150→1000 endpoint numbers (+1.9 K vs +4.4 K
barren; +4.1 vs +8.9 K modern) and the forcing offset (−4.7 of +10.5 W/m²). Replace "~¼" with
"about half, more late in the run". Drop "mostly gone"; "halved" is accurate for both cases.

**2. Major — the modern runs have a forcing jump at the first active step (150 Myr), and the results do not mention it.**
ΔF at step 150, seeds 0–7: +0.30, **+2.73, +5.40**, −0.79, +1.63, −0.27, −0.87, −1.79 W/m². The
later step-to-step s.d. is 0.30–0.52 W/m². In seed 2 the predicted ΔT is +4.25 K in one step, and
F keeps rising to +6.7 W/m² by step 152 (892 ppm, the run's maximum).

The barren runs are smooth (|ΔF| ≤ 0.10 W/m²), so "the CO₂ changes smoothly after the window" is
true only for them. The v2 −2.7 W/m² dip has been replaced by jumps of either sign.

Cause, from the code: in the modern regime the reservoir holds ~0.3 Myr of degassing, so the CO₂
is in equilibrium every step. The calibration balances the 100–150 Myr **mean**, but step 150 uses
`smoothed = None`, which means:

- the **unsmoothed** single-step sources, which still grow after the window (D 150 → 151–160);
- the **unsmoothed** `ridge_crust` in the seafloor sink (see 4).

*Recommendation:*
- (a) Start the smoothed sources at the window's mean (with the slab return at the new φ), not
  at the raw step.
- (b) Calibrate on the last ~10–20 Myr of the window, or on a linear fit evaluated at its end,
  rather than the 50-Myr mean.
- (c) Smooth `ridge_crust` in the sink (finding 4).
- Then re-check ΔF at step 150 for the 8 modern seeds.
- If the change is not made, state the jump in results.md and the PR.

**3. Major (presentation) — the spin-up continues after the 150-Myr window and raises CO₂ for 100–200 Myr; describe it as the start-from-rest artefact it is.**

- **Degassing still rises:** the barren degassing is 13.0 Tmol/yr at 150, 14.4 at 200 and ~15–16
  after 400 Myr. In every barren seed but #7 the mean degassing at 150 is 15–30% above the window
  mean (logged at φ = 1).
- **Barren CO₂ overshoot:** the mean CO₂ goes 0.49 → 0.54 bar at 200 Myr. The per-seed maxima
  are 0.51–0.74 bar (mean 0.60) at 178–382 Myr. Only then does the brightening sun take it down.
- **Modern:** 280 → ~410 ppm mean at 200 Myr, with seed maxima of 320–890 ppm.

The results mention the modern peak ("~400 ppm while the tectonics spins up") but not the barren
one. The pCO₂ trajectory row shows it without comment.

For a real planet at 4.0 Ga one would expect a monotonic decline, since the sun brightens and the
mantle cools. The rise is an artefact of tectonics starting at rest, not physics.
*Recommendation:* one sentence in results.md and the PR that the CO₂ rise from 150 to ~300 Myr
is the remaining tectonic spin-up. A real fix is follow-up 6c.

**4. Minor — the modern step noise comes from the unsmoothed seafloor sink, not from the degassing.**
`sources` are smoothed over 3 Myr, but `sinks()` uses the step's raw `ridge_crust`, which jumps
when cells open. In the modern runs:

- corr(ΔF, ΔD) between steps = +0.06;
- corr(ΔF, Δseafloor) = −0.35;
- corr(Δseafloor, Δland) = −0.63. A sea-floor pulse lowers the CO₂ and with it the land sink.

So the 0.3–0.5 W/m² step noise (up to 4.2 W/m²), and part of the jump in 2, is grid noise in the
sink. results.md attributes it to "the noise of the tectonic rates". The treatment is also
asymmetric: the ridge source (0.03 mol/kg × M) is smoothed, while the larger sink (0.04–0.09 mol/kg
× M) is not. Physically, off-axis alteration takes ~10–20 Myr after the crust forms, so smoothing
is justified.
*Recommendation:* smooth `ridge_crust` for the sink with the same or a longer time constant. This
is a cheap change, and it would roughly halve the modern step noise.

**5. Minor — the warming in the first 150 Myr is partly the sun.**
results.md attributes the +2.8 K (barren) and +4.1 K (modern) only to the highlands wearing down
and the loss of the initial ice. The sun adds +1.7 W/m² (barren) and +3.2 W/m² (modern) of absorbed
sunlight in that time. With the measured λ of 0.58 and 0.72 K per W/m², that is ~1.0 K and ~2.3 K
of it.
*Recommendation:* split the attribution. Also state the cost of the window: 150 Myr (30% of the
default 500-Myr run) in which the sun is not compensated.

**6. Minor — the 150-Myr window: is it sound, fairly described, and what are the alternatives?**
Given the start from rest it is a defensible choice, and it clearly beats 20 Myr (the v1 runs
ended warmer than off). But:

- (i) it does not cover the whole spin-up (finding 3);
- (ii) it costs 150 Myr without the cycle;
- (iii) runs shorter than 150 Myr never activate the cycle.

The CLI help ("by default, tectonic degassing and weathering set the CO₂ in every step") and plan
§8 (still "20 Myr") do not say this.

Alternatives, roughly from cheapest to most work:
- (a) Calibrate on the window's last 10–20 Myr and start the smoothing from it (finding 2).
- (b) After the window, relax φ toward its balancing value over ~50 Myr, or blend the forcing in.
- (c) Real fix, as a separate issue: spin up the tectonics before t = 0 (the sun's clock not
  running), so the epoch starts with tectonics in steady state. That removes both the 150-Myr
  wait and the overshoot.
- (d) The plan reviewer's option: φ = 1 with a forcing offset.

*Recommendation:* update the CLI help, the module docs and plan §8 to describe the 150-Myr window.
Log one clear line at activation ("carbon cycle active from 150 Myr, φ = …").

**7. Minor — the predicted ΔT has a persistent negative bias.**
The mean predicted ΔT over the active steps is −0.11…−0.18 K (barren) and −0.17…−0.25 K (modern)
in every seed. Per step, ΔF + ΔS is ~0.01 W/m², so `toa_imbalance` must sit at about −0.2 W/m².
For a slowly warming climate it should be ≈ 0 or slightly positive.

The weathering is therefore evaluated ~0.15 K too cold, roughly 1–2% too weak. That error was not
present in the calibration, where the warming is 0, so it adds a little to the post-window CO₂
rise. The effect is small but points to a biased diagnostic. Possible causes are the ocean heat
convergence not summing to zero globally, the treatment under sea ice, or unequal cell weights.
*Recommendation:* check `toa_imbalance` → 0 in a fixed-sun equilibrium run. Otherwise subtract
its running mean.

**8. Minor — partitioning wording (gross vs net).**

- **Arcs:** "arcs are the largest source" (9.4 of 16.3 Tmol/yr) counts ~5.5 Tmol/yr of
  **recycled** seafloor carbon (0.5 × 1.2 × 10¹⁴ kg/yr × ~0.09 mol/kg). The primary arc flux (wedge
  plus sediment) is ~3.9, about equal to the ridges' 4.3.
- **Seafloor:** in net terms the seafloor sink (11.0 − 5.5 = 5.5) is about equal to land (5.2) at
  1000 Myr.

results.md mentions the net figure. The PR should say "largest gross source/sink" or give the net
values.

The magnitudes are plausible for a ~4 Ga planet: degassing 12–16 Tmol/yr (Archean models assume
higher degassing than today anyway), and seafloor uptake ~0.09 mol/kg ≈ 0.4 wt% CO₂ averaged over
the crust. In the modern runs land is the largest sink (7.1 vs 4.8), which is Earth-like.

**9. Minor — the response to tectonic events: the conclusion is mostly fair but should be more careful.**

- **Supercontinents:** with 5-Myr-averaged fluxes I get corr(Δln pCO₂, Δlargest land mass) =
  +0.19…+0.24 (barren; results: +0.29) and +0.11 (modern). With n ≈ 130 the barren value is
  nominally significant (t ≈ 2.2–2.7), but the intervals are not independent. "Weak tendency,
  assembly slightly raises CO₂, not robust across seeds" fits better than "no robust signal".
- **"The degassing moves the CO₂" is partly circular:** D contains the slab return, which is
  ∝ the seafloor uptake ∝ r^0.23 averaged over 100 Myr.
- **Ridge production is nearly neutral at epoch 0:** ridge source plus return (0.03 + 0.045
  mol/kg) is about equal to the uptake (0.09 mol/kg). That explains the weak corr(Δln r, Δridge)
  of +0.17…+0.24, and it is the "outcome to explain" that the plan review predicted (its item 3b).
- *Recommendation:* add that sentence. Optionally correlate against the primary degassing (without
  the return) and log the return separately.

**10. Minor — the modern seafloor sink often loses its temperature feedback.**
The logged "bottom water" is the raw polar surface mean: in modern seed 6 it is −5 °C. In 52% of
the active modern steps it is below −1.8 °C, so `seafloor_uptake` clamps it and that sink then has
only the r^0.23 dependence. This weakens the modern feedback. The barren runs are never clamped
(their bottom water is 0–2 °C).
*Recommendation:* log the clamped value, and add this to the caveats. It helps explain why the
modern CO₂ has to fall so far.

**11. Note — physical plausibility.**

- **Barren:** 0.49 → 0.33 bar from 4.0 to 3.0 Ga (indicative, given the extrapolated forcing law),
  at 12–17 °C with 0–4% polar ice. This is within the wide envelope of Archean carbon-cycle models
  (~0.01–1 bar). The decline is gentler than in those models because the mantle does not cool
  here and the degassing instead *grows* with the spin-up; the plan lists this as not modelled.
- **Modern:** CO₂ falls from 280 to 13–147 ppm within 1 Gyr while the temperature rises ~4 K/Gyr.
  That is close to the classic future-Earth result (Caldeira & Kasting, 1992: the CO₂ reaches the
  C3 limit of ~150 ppm in ~0.5 Gyr and the C4 limit of ~10 ppm in ~0.9 Gyr; from memory), which is
  a good sign.
- **Numerics:** no oscillating or growing mode (barren ΔF s.d. 0.04–0.07 W/m², max 0.59; modern
  s.d. 0.30–0.52, max 4.2), no snowball, no runaway, no warnings from the carbon code.
- **Runtime:** the on runs (700–805 s) were not slower than off (967–1049 s), all on GitHub
  runners.

## 3. Decisions for Sebastian (to put in the PR)

1. **Default on:** I recommend yes for the barren default. It is stable, it keeps the temperate
   climate, and it halves the drift. State the 150-Myr window in the help text.
2. **φ calibrated vs φ = 1:**
   - **Barren:** φ = 0.95–1.57, so the literature laws already agree with the epoch-0 forcing.
     Calibrating barely matters there, and that is a useful validation.
   - **Modern:** φ = 3.5–8.2. With φ = 1 the modern planet would need roughly ln 5.5 / (0.07–0.15
     per K) ≈ 10–20 K more warming, or far more CO₂, to balance: a large jump at the start.
   - I recommend keeping the calibration as the default and reporting φ. φ = 1 stays available
     through `--set carbon.phi=1`.
3. **CO₂ floor in the modern case:** a hard clamp at `MIN_CO2` would break the carbon balance.
   Better, as a follow-up: let the plant weathering enhancement fall from 1 to the barren 0.25
   between ~150 ppm (C3) and ~10 ppm (C4). That gives a natural floor and the right physics (a
   vegetation collapse). For this PR, document it. Only seed 6 gets near the limit (13 ppm) within
   1000 Myr, but longer runs will go below it.
4. **Follow-up issue:** a tectonic spin-up before t = 0 (finding 6c), which removes both the
   150-Myr window and the CO₂ overshoot.

## 4. Must-fix for the PR text (short list)

- Replace "~¼ of the warming remains / drift mostly gone" with endpoint numbers: about half
  remains, in both cases (1).
- Mention the modern calibration jump (up to +5.4 W/m²), or fix it with 2a–c (2).
- Describe the post-window CO₂ rise as the remaining spin-up, in the barren case as well (3).
- Update plan §8 and the CLI help for the 150-Myr window (6). Correct the attribution of the early
  warming (5) and the "up to 3.4 Tmol/yr" (§1).
