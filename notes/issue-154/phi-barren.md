# #154: what is a realistic φ for a barren planet?

Question by Sebastian on PR #196: "What would be a realistic φ for a barren planet?"

**Short answer:** about **1, with a plausible range of roughly 0.3–3**. The calibrated
**0.95–1.6** of the barren default lies inside it, close to 1. The reason is that the
missing plants are not part of φ: the barren laws already contain them. φ is the residual
factor between those laws and the climate prescribed at the start. Recommendation: keep the
calibrated φ, and state the plausibility band in the code docs and the log. An optional
follow-up: warn when φ falls outside about 0.3–3 in barren runs.

Project goal: an Earth-like planet, not Earth; Earth values are orientation only.

## 1. What φ is in this model

φ multiplies the **kinetic** constants of both weathering laws (plan §3–4). It does not
scale the supply limits:

- land: `w = w_sup (1 − exp(−φ k_land b L q^0.65 e^{Ea/R(1/T₀−1/T)} g(r) / w_sup))`;
- seafloor: `c = c_max (1 − exp(−φ c₀ r^0.23 e^{E_sf/R(…)} / c_max))` per kg of new crust.

It is set once, at the end of the 150-Myr window, so that the sinks balance the sources at
the start's CO₂. The start's CO₂ follows from the epoch's prescribed greenhouse forcing
(epoch 0: +61.8 W/m², ~0.5 bar).

The factors Sebastian lists, and where each one sits:

| factor | in the barren laws already? | uncertainty that φ absorbs |
|---|---|---|
| **Plant enhancement** | Yes. `plant_factor` b = 0.25 (GEOCARB's pre-vascular f_E). The CO₂ law is the abiotic `r^0.3`, because soil CO₂ equals air CO₂ without roots, not plants' saturating `(2r/(1+r))^0.4`. | Abiotic/plant ratio 0.1–0.5, see §2: a factor ~0.4–2 around b = 0.25. |
| **Regolith / soil residence time** | Partly. The supply limit `w_sup` (fresh rock from the erosion) and the saturating form give West's (2012) kinetic-to-supply transition. | Without soils, water–rock contact time is short. Part of the plant factor is really this (Drever 1994; Berner 1992). Counted in the row above. |
| **Reactive surface area** | No, it is folded into `k_land`, which is calibrated on Earth's rivers. | Factor ~2–3. |
| **Runoff** | Yes, q^0.65 from the model's water cycle. | The exponent (0.5–1) matters little for φ. |
| **Lithology** | Yes. Basalt is 5× felsic; Ca+Mg contents come from the crust type. | Basalt/granite 5–10× (Dessert et al. 2003): factor ~1.5 on the land sink. |
| **Supply-limited formulation** | Yes: φ does not change `w_sup`. | Only acts where land is kinetic. On average it is: only ~25% of the supplied Ca+Mg dissolves (weathering intensity 0.22–0.28), so φ acts almost linearly on land. |
| **Temperature dependence** | Yes. Arrhenius, 50 kJ/mol on land and 92 kJ/mol on the seafloor. | Ea 40–75 / 40–110 kJ/mol. This changes the feedback strength more than the level at the reference T. |
| **Seafloor uptake** (the largest sink at epoch 0) | Yes. c₀ from Alt & Teagle (1999). | Factor ~3. Krissansen-Totton & Catling (2017) and Krissansen-Totton et al. (2018) find a smaller modern sink and a weaker CO₂ dependence. Plants play no role here. |
| **Degassing coefficients** | Yes (ridges, arcs, plumes, metamorphic). | Each is uncertain by a factor ~2. φ balances the sinks against them, so it absorbs these too. |
| **The prescribed start forcing** | Not a weathering factor. | The epoch-0 forcing is chosen for a temperate start (the faint young sun). φ measures whether the laws agree with it. |

So φ = 1 means "the literature laws, with the barren reductions already applied, balance
the model's degassing at the prescribed start CO₂". It is **not** "plants are present". If
φ had to absorb the plants, the barren value would be 0.1–0.5. That is a different question,
answered in §2.

## 2. Literature: how much slower is weathering without life?

- **Vascular plants vs bare ground:**
  - Iceland basalt, Ca and Mg: 3–8× (Moulton & Berner 1998, Mineral. Mag. 62A). The
    synthesis gives ~4× (Moulton, West & Berner 2000, Am. J. Sci.).
  - Hubbard Brook sandboxes: up to ~10× (Bormann et al. 1998).
  - A biotic enhancement of 3–10× is commonly cited to Drever (1994). Drever himself rated
    the direct chemical effect of plants as modest and the indirect effects (water retention,
    soil formation, CO₂) as larger.
  - GEOCARB uses f_E = 0.25 before ~380 Ma (Berner 1994; Berner & Kothavala 2001): a factor 4.
- **Non-vascular cover** (lichens, mosses, microbial mats): already raises weathering several
  times above sterile rock (Lenton et al. 2012; Porada et al. 2016). A truly abiotic surface
  (no microbes either) lies at the low end.
- **Schwartzman & Volk (1989)** argued for a total biotic enhancement of 10–100× or more. That
  is the upper bound of the debate, not the consensus.
- **So the abiotic/plant ratio is ~0.1–0.5.** The model's b = 0.25 is central.
- **The early Earth without land plants:** carbon-cycle models (Sleep & Zahnle 2001;
  Krissansen-Totton et al. 2018) need no extra factor beyond these laws to reach a temperate
  Archean with 0.01–1 bar CO₂. Krissansen-Totton et al. find seafloor weathering a
  **comparable** sink to continental weathering, not the dominant one. The model's seafloor
  sink (2–3× land) is at the high end of that.
- **Kinetic vs supply limit:**
  - West (2012) and Maher & Chamberlain (2014): where erosion is fast, weathering is
    kinetically limited and climate-sensitive. Where erosion is slow, it is supply-limited and
    insensitive.
  - Without soils and roots, the fluid residence time is short. That pushes barren land
    towards the kinetic, less efficient side. The model's intensity of ~0.25 (modern runs:
    ~0.5) agrees with that.

**A plausible φ for the barren planet.** Combine the residual uncertainties of §1 as
independent factors:

- seafloor ×3^±1;
- surface area/kinetics ×2^±1;
- barren factor ×2^±1;
- degassing ×2^±1.

The weights follow the share of each sink (seafloor ~2/3, land ~1/3). This gives a 1σ range
for φ of roughly **×2.5 around 1, i.e. 0.4–2.5**. **0.3–3** is a generous "plausible" band.

## 3. Comparison with the calibrated values

| case | calibrated φ | reading |
|---|---|---|
| barren, epoch 0 (default) | 0.95–1.6 | **Inside the plausible band, near 1**: the laws and the prescribed faint-sun forcing agree within a factor 1.6. |
| modern Earth (`--biosphere modern --epoch 4`) | 3.5–8.2 | **Above the band**: at 280 ppm the laws remove too little CO₂ for the model's degassing (12–16 Tmol/yr, ~2× Earth's), so φ ≈ 2 of it is the fast sea-floor production. The rest is the weak low-CO₂ land weathering at the model's climate. This is a modern-preset issue (tectonic rates, plant floor), not a barren one. |

**What φ = 1 would do instead.** The sink's sensitivity is d ln W / d ln r ≈ 1.2 at epoch 0,
and dF/d ln r ≈ 11 W/m². Switching from the calibrated φ to φ = 1 therefore moves the CO₂ by
`φ^{1/1.2}`:

- range over the seeds: −4% to +48%;
- forcing: −0.5 to +4.3 W/m²;
- temperature: about −0.3 to +2.6 K warmer, within a few feedback times (~15 Myr) after the
  window.

The whole 0.3–3 band would mean ±11 W/m² (±6 K). This is the true uncertainty of the
planet's absolute climate from the weathering laws. Calibration removes it by trusting the
prescribed start climate.

## 4. Recommendation

- **Keep the calibrated φ (default) for the barren planet.** It is within the literature band
  and close to 1. It keeps the start continuous, and it removes a seed-dependent jump of up to
  ~4 W/m² that would be noise from the parameter uncertainty, not physics.
- **No fixed literature φ ≠ 1.** No literature value is more specific than "~1 within a
  factor ~3" once the barren reductions are in the laws. A fixed φ = 0.25 would count the
  plants twice.
- **φ = 1** stays available (`--set carbon.phi=1`) for "the laws as they are".
- Optional small follow-up:
  - a warning when the calibrated φ lies outside **0.3–3** (barren) or **0.3–10** (modern,
    because of the 2× degassing), as a soundness check;
  - narrowing `phi_range` from 10³ to that band.

## Decision (Sebastian, 2026-11-27)

"Phi should be calibrated, but clamped to the range you proposed." Implemented:

- The calibrated φ is clamped to **0.3–3** (`carbon.phi_min = 0.3`, `carbon.phi_max = 3`; they
  replace `phi_range`). 0.3 rather than 1/3, because the band is the rounded plausible range of
  §2, not a symmetric factor. Where the clamp binds, a warning names the balancing φ and the
  clamped one.
- At a clamped φ the sinks do not balance the sources at the start's CO₂. The CO₂ is still held
  for the 150-Myr window, so the forcing is continuous at its end; afterwards the implicit step
  moves the CO₂ (within a few feedback times) to where the sinks balance. The carbon balance is
  conserved exactly as before; the clamp only changes a constant of the laws.
- A fixed `--set carbon.phi=X` is not clamped.
- The same band holds for the modern preset, derived separately in [phi-modern.md](phi-modern.md)
  (Sebastian: the clamp should be realistic for each preset).
- The barren default (0.95–1.6) never reaches the clamp: bit-identical. The modern runs
  (3.5–8.2) are clamped to 3; see [results.md](results.md#clamped-φ-modern-runs).

"No minimum CO₂ for plants": no plant CO₂ floor follow-up.

## Sources

Values marked "secondary" were checked through abstracts or citing papers only.

- Berner, R. A. (1994), GEOCARB II, Am. J. Sci. 294; Berner & Kothavala (2001), GEOCARB III,
  Am. J. Sci. 301.
- Moulton, K. L. & Berner, R. A. (1998), Geology 26, 895–898, and Mineral. Mag. 62A, 1030
  (3–8× for Ca, Mg); Moulton, West & Berner (2000), Am. J. Sci. 300 (~4×) (secondary).
- Drever, J. I. (1994), Geochim. Cosmochim. Acta 58, 2325–2332 (secondary).
- Bormann, B. T. et al. (1998), Biogeochemistry 43 (Hubbard Brook sandboxes).
- Schwartzman, D. W. & Volk, T. (1989), Nature 340, 457–460.
- Lenton, T. M. et al. (2012), Nature Geosci. 5, 86–89; Porada, P. et al. (2016), Nature
  Geosci. 9, 830–833.
- West, A. J. (2012), Geology 40, 811–814; Maher, K. & Chamberlain, C. P. (2014), Science 343,
  1502–1504.
- Dessert, C. et al. (2003), Chem. Geol. 202, 257–273.
- Alt, J. C. & Teagle, D. A. H. (1999), GCA 63, 1527–1535; Coogan & Dosso (2015), EPSL 415.
- Sleep, N. H. & Zahnle, K. (2001), JGR 106, 1373–1399; Krissansen-Totton, J. & Catling, D. C.
  (2017), Nature Commun. 8, 15423; Krissansen-Totton, J., Arney, G. & Catling, D. C. (2018),
  PNAS 115, 4105–4110.
