# #202 results: abiotic H₂ and CH₄

Plan and its review: [plan.md](plan.md). Full tables per snapshot: [runs.txt](runs.txt)
(`python3 analyse.py RUNDIR…`). Code at `b87d8d3` (runs) on `agent/issue-202`; main at `e55bf1d`.

## What the model does

- **Sources.** Volcanic H₂ and CO come from the mantle CO₂ of arcs and plumes (#212's
  `arc_plume_mantle_degassing`). At the FMQ buffer the mantle gases are H₂/H₂O = 0.01 and
  CO/CO₂ = 0.03, both ∝ 10^(−ΔFMQ/2), with 10 mol H₂O per mol of mantle C.
- **Serpentinization.** H₂ comes from new oceanic crust, weighted by its full spreading rate:
  0.08 kg of serpentinite per kg of crust at ≤ 2 cm/yr, falling to 0 at 8 cm/yr, and 0.25 mol
  H₂ per kg of serpentinite. The tectonics now records new crust per 1 cm/yr bin.
- **Fischer–Tropsch CH₄.** 1% of the serpentinization H₂ becomes CH₄, at 4 H₂ per CH₄.
- **Sinks.**
  - The escape's O₂ takes the gases along with #212's sulfur and seafloor sinks. The volcanic
    sink keeps only its sulfur part, 0.085 mol O₂ per mol C, so at FMQ the total stays ≈ 0.15.
  - A living planet's background O₂ oxidizes the gases within ~2–10 yr.
  - Without O₂, CH₄ is photolysed with a lifetime of 10⁴ yr (CH₄ + 2 H₂O → CO₂ + 4 H₂).
  - H₂ and CH₄ escape at their diffusion limits (`f_T = f_H₂O + f_H₂ + 2 f_CH₄`). The XUV
    energy limit is shared with the water's escape.
- **Forcing.** The CH₄ forcing is Myhre's, relative to the pre-industrial 700 ppbv, which is
  the climate model's zero; with no CH₄ it is −0.95 W/m². H₂–N₂ CIA is added as
  40 W/m² · p_H₂ · p_N₂. The epoch's start forcing is the total. During the ramp-up hold the
  CO₂ makes up any change in the gas forcing, so the start is continuous.
- **Budgets.** Carbon (CH₄ ↔ exogenic carbon), hydrogen (juvenile H₂ and water), the redox
  (H₂-equivalents) and #212's O₂ all close. Unit tests check this over 300 steps with changing
  climates, redox and O₂.
- **Switch.** `--no-reduced-gases` turns the gases off, and so does `--no-escape`. State hashes
  over 50 Myr match main for the default run, for `--biosphere modern --epoch 4` and for
  `--no-carbon-cycle`.

## Runs (1000 Myr)

| case | seeds | f_H₂ | f_CH₄ | gas forcing (W/m², vs 700 ppbv) | T 200–1000 Myr | pCO₂ 200–1000 |
|---|---|---|---|---|---|---|
| main, barren | 8 | — | — | — | 17.03 ± 0.31 °C | 0.475 bar |
| **#202, barren** | 8 | 1.4–1.9 × 10⁻⁴ | 0.21–0.32 ppmv | −0.30 → −0.43 | 16.79 ± 0.27 °C | 0.486 bar |
| #202, barren, ΔFMQ = −4 | 4 | 2.0–2.8 × 10⁻³ | 0.23–0.33 ppmv | −0.16 → −0.28 | 17.22 ± 0.45 °C | 0.498 bar |
| main, barren, `--no-carbon-cycle` | 2 | — | — | — | 17.68 °C (17.67, 17.68) | fixed |
| #202, barren, `--no-carbon-cycle` (`6c12102`) | 2 | ~1.3–2 × 10⁻⁴ | ~0.2–0.3 ppmv | −0.30, −0.53 at the end | 17.07 °C (17.26, 16.89) | fixed |
| main, modern epoch 4 | 4 | — | — | — | 22.17 ± 0.36 °C | 0.001 bar |
| #202, modern epoch 4 | 4 | ~10⁻⁸ | ~4 × 10⁻¹⁰ (+700 ppbv biotic) | 0.000 | 22.41 ± 0.41 °C | 0.000 bar |

(± is the standard error of the seed means.)

### Barren default: sources and sinks (Tmol/yr, means over 8 seeds)

| t (Myr) | volcanic H₂ | CO | serp. H₂ (serpentinite kg/kg) | CH₄ made | H₂ escape | CH₄ photolysis | CH₄ escape | H₂ to the escape's O₂ |
|---|---|---|---|---|---|---|---|---|
| 50 | 0.27 | 0.08 | 0.95 (0.035) | 0.0095 | 1.30 | 0.0073 | 0.0021 | 0.0045 |
| 500 | 0.31 | 0.09 | 0.92 (0.027) | 0.0092 | 1.31 | 0.0071 | 0.0021 | 0.0046 |
| 1000 | 0.26 | 0.08 | 0.60 (0.023) | 0.0060 | 0.93 | 0.0046 | 0.0014 | 0.0052 |

- **Serpentinization is the largest source**, at 0.6–1.0 Tmol H₂/yr. It falls by about a
  third over the run because the plates speed up: the mean plate speed rises from 3.6 to
  4.5 cm/yr, and the serpentinite share falls from 0.035 to 0.023 kg/kg (r = −0.46 over all
  snapshots). New crust production stays roughly flat. That is 2–3 times the
  ~0.4 Tmol/yr an Earth spread of rates would give (unit test). The model makes 1.5–2 times
  Earth's crust, and its share of slow ridges is about Earth's: serpentinite 0.023–0.035 kg/kg
  against Earth's ~0.023. The volcanic H₂ + CO is 0.35 Tmol/yr, at the low end of the
  photochemical models' 1–8 Tmol/yr because it is tied to #212's 0.15 mol O₂ per mol of arc and
  plume mantle CO₂.
- **H₂ at 140–200 ppmv** is set by the diffusion limit (lifetime ~27 kyr): Kasting's "~0.1%"
  regime, scaled to a smaller source. Effectively all H₂ escapes. The escape's O₂ from the water
  (~0.01 Tmol/yr) titrates < 0.5% of it, so the barren planet keeps **0 O₂**, as on main.
- **CH₄ at 0.2–0.3 ppmv**, with an effective lifetime of ~7 kyr (photolysis 10⁴ yr plus escape).
  This is the ppm level Guzmán-Marmolejo et al. (2013) expect for abiotic CH₄. CH₄/CO₂ ≈ 6 ×
  10⁻⁷, far below the haze threshold of ~0.1 (#203).
- **Hydrogen escape** is ~100 times main's: 7–10 × 10⁹ against 0.9–1.3 × 10⁸ H cm⁻² s⁻¹. Almost
  all of it is from H₂, not water.
- **Water.** The serpentinization water whose O stays in the rock (magnetite) and whose H
  escapes costs the ocean **1.1–1.2% per Gyr**: 6.6–10 × 10²⁰ mol, ~40 m of global ocean depth.
  The sea level relative to the land falls by **18 m** (the `escape` term of the sea-level
  budget, with water loading and hypsometry; main: −0.3 m). This is the irreversible oxidation of an anoxic planet by
  hydrogen escape (Catling et al., 2001), now driven by its geology. With ΔFMQ −4 the CO
  conversion (CO + H₂O → CO₂ + H₂) adds to it: −58 m, 3.6% of the ocean per Gyr. These are
  lower bounds. The volcanic H₂ counts as juvenile, coming from the mantle's water, and
  volcanic H₂O is not modelled. If the arc H₂ came from recycled ocean water, the barren loss
  would be ~30% larger and the reduced one about twice as large.

### Forcing and temperature

- The barren gas forcing is −0.30 to −0.43 W/m² against 700 ppbv, made of CH₄ (−0.31 to −0.44)
  and H₂–N₂ CIA (+0.01).
- During the ramp-up hold the CO₂ makes up the gas forcing, so the start is unchanged: the hash
  at t = 0 equals main's.
- After the hold the gas forcing drifts by **−0.13 W/m² over 1000 Myr**, following the
  serpentinization. The carbon cycle compensates with slightly more CO₂: total forcing
  60.94 vs 61.04 W/m², and pCO₂ 0.486 vs 0.475 bar.
- **Temperature: ≈ −0.2 K, marginal.** 16.79 vs 17.03 °C over 200–1000 Myr. Paired by seed
  the difference is −0.24 ± 0.14 K, with 6 of 8 seeds cooler. Pairing is only partial,
  because the tectonics of the two runs diverge by ~200 Myr. This fits the 0.1 W/m² lower
  total forcing. As the issue
  expected, abiotic CH₄ and H₂ are small against tens of W/m² of CO₂.
- `--no-carbon-cycle`:
  - A first version held the CO₂ forcing at the start's total minus the "no CH₄" forcing
    (−0.95 W/m²). Because the gases reach their steady state in the first step, that planet sat
    0.5–0.7 W/m² above main for the whole run (independent evaluation; one finished seed pair
    was +1.1 K).
  - Fixed in `6c12102`: as with the carbon cycle, the CO₂ forcing makes up the gas forcing
    until the end of the ramp-up and then stays. The total therefore changes only by the gas
    forcing's drift after the ramp-up, −0.1 to −0.2 W/m² by 1000 Myr.
  - Two seeds: 17.07 vs 17.68 °C over 200–1000 Myr (paired −0.4 and −0.8 K). That is more than
    the forcing explains (~−0.1 K at the model's ~0.6 K per W/m²). With two seeds whose
    tectonics diverge it is within the seed noise (±0.9 K between seeds in the first version).
  - The carbon line of the log still prints the carbon cycle's own forcing, which is unused
    when it is off, as on main.
- **Reduced mantle (ΔFMQ −4, near IW):**
  - Volcanic H₂ reaches 13–15 Tmol/yr (and CO 2 Tmol/yr), so H₂ is 0.2–0.35% and the escape is
    1.2 × 10¹¹ H cm⁻² s⁻¹. That is still diffusion-limited: the XUV limit falls from
    1.75 × 10¹² at 50 Myr to 6 × 10¹¹ cm⁻² s⁻¹ at 1000 Myr, ~5 times above the escape.
  - The H₂–N₂ CIA is only +0.14 W/m²; it needs several % H₂ to matter (Wordsworth &
    Pierrehumbert, 2013).
  - Temperature 17.22 °C. Against main's same seeds 0–3 (17.44 °C) that is −0.22 ± 0.07 K,
    with all 4 seeds cooler: the slightly more negative gas forcing in the late run (the
    carbon cycle compensates most of it).
  - A unit test with a late sun (energy-limited, sources above the XUV limit) shows H₂ building
    up to tens of % with the escape held at the limit.
- **Modern** (`--biosphere modern --epoch 4`): the background O₂ oxidizes the gases, so the
  abiotic H₂ is ~10 ppbv and CH₄ ~0.4 ppbv on top of the held biotic 700 ppbv. The forcing
  change is 0.000 W/m². T is 22.41 ± 0.41 vs 22.17 ± 0.36 °C (noise). The juvenile volcanic
  H₂, oxidized by the background O₂, adds 2.6 × 10²⁰ mol of water per Gyr (+5 m of sea level,
  ~13 m of ocean depth). The background O₂ is not debited, because the biosphere's O₂ cycle
  is #201's, and volcanic H₂O itself is not modelled.

### Runtime

There is no measurable cost. Local 50 Myr runs take 54 s with and without the gases; the gas
step is a few bisections per step. The GitHub-runner times (main 985 s, #202 1056 s, mean of
8) differ by runner, not by code.

## Caveats

- **The volcanic H₂ is low and the serpentinization high, compared with Earth.** Both follow
  the model's tectonics: fast crust production, and mantle CO₂ restricted to arcs and plumes.
  `mantle_water` (10) and `serpentinite` (0.08) are the main uncertainties, a factor of ~3
  each.
- **Grid.** Spreading rates come from the plates' relative motion at each new ~110 km cell.
  Transform offsets and ultraslow amagmatic segments shorter than a cell are not resolved
  (#64), so the slow-ridge share is coarse.
- **Lifetimes.** The anoxic CH₄ lifetime (10⁴ yr) really depends on the CH₄ and H₂ fluxes, and
  the O₂ dependence is linear only between the end states; it is non-monotonic in between
  (Goldblatt et al., 2006). CO is assumed to oxidize via OH to CO₂ + H₂. At large CO fluxes
  (the reduced mantle's 2 Tmol/yr) a real anoxic atmosphere may build up CO (Kharecha et al.,
  2005; Zahnle et al., 2006).
- **Magmatic CH₄ is left out.** It is negligible at FMQ, but a very reduced, cool outgassing
  could release some. At reduced redox the sulfur leaves as H₂S, which demands more O₂, but
  `volcanic_sulfur` stays fixed.
- **Forcing laws.** Myhre's √ law is used from 0 to 100 ppmv and held above, where shortwave
  absorption flattens the net forcing (Byrne & Goldblatt, 2014). The CIA coefficient is a
  rough fit to Wordsworth & Pierrehumbert (2013).
- **Prediction lag.** A change of the gas forcing reaches the carbon cycle's prediction one
  step late, through the TOA imbalance. This is negligible at < 0.01 W/m² per step.
- **#212's seafloor sink.** The basalt oxidation (0.008 mol O₂/kg) stays a fixed O₂ capacity.
  Serpentinization is a separate, peridotite-hosted H₂ source.

## Decisions for Sebastian

1. **Serpentinization strength.** 0.6–1.0 Tmol H₂/yr is inside the literature range of
   0.1–1.5 for Earth's ridges (Cannat et al., 2010; Worman et al., 2016). It is the main new
   water sink: 1.1% of the ocean per Gyr. Keep `reduced.serpentinite` = 0.08?
2. **The redox parameter.** `--set reduced.mantle_redox=-4` gives a "reduced planet": 0.3%
   H₂, still no climate effect. Is a CLI option or a preset wanted, or is `--set` enough?

## Independent evaluation and what changed

An independent agent checked these results against the logs and the code. It found that the
numbers reproduce, the physics is plausible for an Earth-like barren planet and the budgets
close. Its findings and the changes:

1. **`--no-carbon-cycle` (must-fix):** the CO₂ forcing was held at the start's total minus the
   "no CH₄" forcing, a permanent offset of +0.5–0.7 W/m². *Fixed* (`6c12102`, see above).
2. **Temperature wording:** unpaired means were reported as "no change". *Reworded* to the
   paired −0.24 ± 0.14 K (barren) and −0.22 ± 0.07 K (ΔFMQ −4 against the same seeds), marginal.
3. **Falling serpentinization:** *explained* by the plates speeding up (3.6 → 4.5 cm/yr).
4. **Sea-level numbers:** the ocean-depth equivalent and the sea level are now *stated
   separately*.
5. **Juvenile volcanic H₂:** its water is not tracked. *Stated*: the loss numbers are lower
   bounds, and the modern run's water gain comes from the undebited background O₂.
6. **Logged diffusion limit:** it used the last step's gases, so the flux could exceed it.
   *Fixed*: it now logs the step's.
7. **The gases' water bypasses the escape's 1% ocean floor.** *Kept*: it matters only near
   the floor, and `remove_water` still never takes more than the ocean holds.
8. **XUV limit numbers in the reduced case:** *corrected*.

## Overlap with open PRs

PR #221 (LIPs and plume tails, CO₂ through #196's plume source) raises
`arc_plume_mantle_degassing`. The volcanic H₂ and CO of this issue scale with it
automatically, by design without double counting. No code conflicts are expected beyond
`planet.rs` context.
