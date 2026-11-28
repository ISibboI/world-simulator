# #113: independent evaluation of the 1000 Myr results

Data: `agent-scratch/runs/{main,branch,flow-only,erosion-only}` (16/16/8/8 seeds, default
parameters, `--duration 1000 --cell-dump --snapshot-interval 100`), tables
[compare.txt](compare.txt) and [orogens.txt](orogens.txt). Extra analyses (scripts in
`agent-scratch/eval/`, not committed): null distribution of |t| from all 8-vs-8 splits of main,
per-half comparisons, a 2×2 factorial on seeds 0–7, per-seed values, per-snapshot ensemble
means, and orogen heights conditioned on collision stacking.

Judged by physical plausibility for an Earth-like planet (not Earth's numbers); ~110 km grid
artefacts accepted.

## Noise level

- 95% of |t| over all 6435 8-vs-8 splits of main (same code): **2.0–2.3** per metric. With ~75
  correlated metrics, 4–5 values with |t| > 2 per column are expected from chaos alone.
- Trajectories decorrelate from main within 300–500 Myr (seed correlation of collisions
  0.95 at 100 Myr, 0.6 at 500, ~0.1 at 700–900; plates, mean land similar). The 500–1000 Myr
  window is effectively independent realisations; pairing by seed does not help there.
- Collision length per seed is **bimodal** (near 0 when no active collision, else 15–60 ×
  10³ km; e.g. seeds 1 and 2 drop to ~0 at 600 Myr in branch and flow-only, seed 3 in all four
  variants). Means over 8 seeds are dominated by how many seeds sit in the "no collision"
  regime (main 2/16, branch 4/16, flow-only 4/8, erosion-only 2/8; Fisher-type difference not
  significant). Orogen metrics follow collision stacking.

## 1. Real vs chaotic

| shift (branch vs main) | verdict |
|---|---|
| crustal flow 79% of per-cell request; erosion 90% on sample / 10% neighbours | real, by construction |
| orogens (collisions, stacking, p90/p99, crust > 50 km, mean land, highest point): t −1 to −2 | **noise / regime sampling** (see 3). Branch seeds 8–15 vs main 8–15: t −0.6 to +0.5; the whole signal sits in seeds 0–7, where main has a late (800–1000 Myr) orogeny spike (collisions 31–34, p90 41–43 km) not seen before 800 Myr |
| ocean deeper than 6 km +3.6 | **robust against main, origin unclear** (see 4) |
| sea floor ≥ 200 Myr −3.3, mean sea-floor age −2.0, slab pull +2.5 | borderline; coherent group (younger floor, more pull), 2×2 attributes it to the erosion change (t −3.6/−2.4/+2.3, flow 0), but per snapshot it shows only at 700–800 Myr and the main split itself gives −1.9 on ≥ 200 Myr. Harmless direction |
| sutures +2.1, sediment change +2.2, sediment part at 1000 Myr +2.4, dyn. topo min −2.0, land crust p99 −2.1 | at the noise threshold; consistent with multiple testing |
| runtime +2.0 / −4.0 | machine mix (branch 4 seeds local, others GitHub), not a code effect |
| everything else (land, continental area, balance, climate, hydrology, plates) | noise (|t| < 2) |

## 2. Plausibility of the branch (means over 500–1000 Myr, 16 seeds)

| metric | branch | main | assessment (Earth only orientation) |
|---|---|---|---|
| crust > 50 km | 1.0% of continental (0–4.2) | 1.6% (0–6.2) | plausible; Earth ~2–3% (Tibet, Andes), strongly regime dependent; 110 km cells smear orogen roots |
| mean land | 495 m (197–907) | 594 m | plausible, a bit low vs Earth 840 m; within main's seed range |
| land fraction | 28.4% | 26.9% | plausible (Earth 29%) |
| continental area | 33.6% | 33.1% | plausible |
| felsic + sediment change | −1.4 Gt/yr | −1.8 | unchanged within noise |
| continental crust change | −3.7 Gt/yr (−6.4…+0.5) | −2.75 | t −1.7, noise; slow net loss (~14% per 500 Myr) exists in main too, not new |
| highest point | 3.8 km (2.5–4.6); max over run 5.3 km | 4.1 / 5.3 km | plausible for 110 km cell means (a cell-mean of Tibet ≈ 5 km) |
| land p90 / p99 | 1.5 / 2.6 km | 1.8 / 3.0 km | plausible |

Nothing leaves a plausible range; all branch means lie inside main's seed range.

## 3. Flow-only: lower orogens and halved collisions

Most likely seed noise / regime sampling, not a mechanism of the change:

- **Direct effect has the opposite sign.** The 1/k factor reduces collapse at duplicated
  samples (−21% of requested flow); with less collapse, thick crust stays thicker. Duplicated
  samples are over-represented in orogens (17–19% of cells with crust > 40 km, land > 2 km or
  crust > 50 km vs 9% overall), so the direct effect should raise peaks, as the plan expected.
  Flow-only also has slightly *more* continental area (+1.9) and continental-origin area (+2.0),
  also not what less spreading would give.
- **Conditioned on collision stacking**, the orogen deficit mostly disappears: residual
  continental-origin p90 −0.4 ± 0.5 km (branch), −1.6 ± 0.5 km (flow-only), −0.7 ± 0.9 km
  (erosion-only); highest point and mean land residuals within 1–1.5 SE. The drop comes through
  collision activity, i.e. plate geometry, which the change affects only chaotically.
- **Collisions are regime-like**: flow-only seeds 1, 2, 3 (and 7 partly) end without active
  collisions; seed 3 does so in main and erosion-only too. The deficit appears only after
  600–800 Myr (per snapshot, flow-only vs main 0–7 differs by < 1.2 SE until 500 Myr).
- **Not reproduced where it should be**: the branch contains the same flow change but shows only
  −1 to −2, and branch seeds 8–15 vs main 8–15 show nothing (−0.6…+0.5). Flow alone lowering
  orogens while flow + erosion does not would need an interaction without a mechanism.
- Caveat: in a pooled exchangeability test (random 8 of all 48 runs), flow-only's mean p90
  (P ≈ 0.002) and collisions (P ≈ 0.015) are unusually low, so a modest real reduction cannot be
  fully excluded from 8 seeds. It is irrelevant for the shipped code, which is the branch.

## 4. Ocean deeper than 6 km

Robust against main but probably not a direct effect of either change:

- All three variants are higher (4.0–4.6% vs 2.85%); a random 16 of all 48 runs is as low as
  main with P ≈ 4e-5. Seeds 0–7 and 8–15 both show it (t +3.6 / +1.6).
- **Time structure**: no difference up to 500 Myr (pooled variants vs main t −0.8…0); it opens
  at 600 Myr (+2.0…+3.9) because main's deep ocean falls from 4.0% (500) to 2.4–2.7% while the
  variants stay 4–5%. Main's ensemble is at the same time in a late phase with more collisions,
  thicker orogens and more ≥ 200 Myr floor: one coherent baseline regime, not separate effects.
- **No mechanism**: neither change touches oceanic crust (flow receivers are continental-origin
  only; holes are continental; erosion acts on the overriding plate). The shift is spread over
  all sea-floor ages and dynamic-topography classes. The ~100 m deeper mean ocean (t −1.5)
  explains only ~0.4 of the +1.75 points (main's < −5.9 km fraction is 0.4 points above its
  < −6 km).
- Non-additive (flow +1.7, erosion +1.2, both +1.8): "any change gives the same shift" fits
  main being an unusual (phase-locked) realisation better than two independent physical
  effects.
- Either way the values are plausible for an Earth-like planet (Earth ~1–2%, here more old,
  thinly sedimented floor at coarse resolution); not a blocker.

## 5. Re-tuning

**Keep the coefficients.**

- `collapse_diffusivity` 1e4 km²/Myr is physical (Clark & Royden 2000 channel flow,
  3.7e3–3.7e4) and was not fitted against the double counting. The −21% effective rate equals
  ~7.9e3 km²/Myr in the old counting, well inside the physical range. No orogen metric left its
  plausible range, and the direct effect (less collapse at duplicates) would, if anything, raise
  orogens.
- `subduction_erosion` (300 m per subducted cell): total flux unchanged by design (budget
  −2.78 vs −2.76 Gt/yr, t −0.2); nothing to re-tune.
- If certainty is wanted on deep ocean or orogens, the informative test is more seeds of main
  and branch (e.g. 16–31), not more diagnostic variants: the shifts are set by a few seeds'
  regimes after 600 Myr.
