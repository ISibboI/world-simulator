# #37 feasibility check before implementing, 2026-11-27: physical kernels make the depressions deeper, not shallower

**Short answer:** the approved fix (dynamic-topography kernels from a layered viscosity profile
plus thermal assimilation of slabs) rests on the check's cause 2, "the response per unit slab
mass is at the high end". A propagator calculation shows the opposite: the model's current
Gaussian stages respond **less** per unit slab mass than any literature viscosity profile does.
Done with literature values, the fix would deepen the depressions by roughly 10–50%, not reduce
them. It would only make them shallower with a slab "assimilation" time chosen short enough,
and the literature gives no basis for such a time. That would be tuning under another name.
So I stopped before implementing and need a decision (options at the end).

## Method

Script: [kernels_prototype.py](kernels_prototype.py) (Python, ~20 s).

- Surface kernels `K_l(d)` of an incompressible, radially layered viscous sphere with free-slip
  surface and core–mantle boundary. This is the propagator formulation of Hager & O'Connell 1981
  and Hager & Clayton 1989.
- `K_l(d)` is the fraction of the weight of a degree-`l` load at depth `d` that the surface
  supports by deflection. It is 1 for a load at the surface and 0 at the CMB.
- The current model's stage `(f, σ)` corresponds to `K_l = f · exp(−l(l+1) σ²/2R²)`.
- Checks:
  - I verified the propagator matrix symbolically (sympy) against the analytic constant-viscosity
    Stokes solutions for l = 2 and 3, all four of them.
  - At high `l` the kernel matches the half-space result `(1 + kd) e^{−kd}`
    (l = 40, d = 330 km: 0.376 vs 0.387).
  - The limits are right: K = 1.00 at 1 km depth and K → 0.06 at 2800 km.

## Kernels: physics vs the model

Mean over each stage's depth range, at spherical-harmonic degrees l = 2 / 5 / 10 / 20
(wavelengths ≈ 16,000 / 7,000 / 3,800 / 2,000 km):

| stage (depth) | current model (Gaussian) | lith 100× / UM 1 / LM 30× | + asthenosphere 0.1× (100–300 km) | lith 100× / UM 1 / LM 100× |
|---|---|---|---|---|
| 1 (100–660 km) | 0.49 / 0.44 / 0.31 / 0.08 | 0.90 / 0.76 / 0.59 / 0.50 | 0.87 / 0.66 / 0.47 / 0.37 | 0.76 / 0.61 / 0.52 / 0.49 |
| 2 (660–1200 km) | 0.38 / 0.30 / 0.13 / 0.01 | 0.76 / 0.51 / 0.15 / 0.02 | 0.72 / 0.36 / 0.08 / 0.01 | 0.57 / 0.25 / 0.05 / 0.01 |
| 3 (1200–2000 km) | 0.17 / 0.09 / 0.01 / 0.00 | 0.57 / 0.36 / 0.07 / 0.00 | 0.54 / 0.25 / 0.03 / 0.00 | 0.43 / 0.18 / 0.02 / 0.00 |

- The viscosity contrasts are relative to the upper mantle. Lower mantle 30× is from Hager 1984
  and Hager & Richards 1989; 30–100× is the range of later inversions, e.g. Mitrovica &
  Forte 2004. All from memory, not re-checked.
- At the long wavelengths that carry most of the field's power, the physical kernels are
  **1.5–3.5× larger** than the model's.
- A stiffer lower mantle (100×) or a weak asthenosphere lowers them, but not to the model's values.

## Effect on a test case

The unit test's case: a 5000 km trench, 5 cm/yr, old plate (7 × 10⁶ kg/m²), in steady state.
Same slab masses and positions as the model, kernels tapered at 300 km. The table gives the dry
deflection on the line of the sunken slabs and at distances from it:

| response | 0 km | 600 km | 1500 km |
|---|---|---|---|
| current model | −1616 m | −1251 m | −438 m |
| lith/UM/LM 30× | −3597 m | −2492 m | −1218 m |
| … with α(P) | −2449 m | −1497 m | −684 m |
| + weak asthenosphere, with α(P) | −1908 m | −1253 m | −663 m |
| LM 100×, with α(P) | −1732 m | −935 m | −501 m |

α(P) means the excess mass of the slab falls with the thermal expansivity:
- relative to the lithosphere's α ≈ 3.1 × 10⁻⁵ K⁻¹ (GDH1), it is ×0.65 at 660–1200 km and
  ×0.45 at 1200–2000 km;
- α is ~2 × 10⁻⁵ at the top of the lower mantle and ~1.4 × 10⁻⁵ at 1500 km (from memory; cf.
  Katsura et al. 2009 and the profile of Steinberger & Calderwood 2006).

Every physically based variant is 7–120% deeper on the slab line than the current model.

## Why "thermal assimilation" does not rescue it

- Conduction conserves a slab's heat deficit. It spreads the cold anomaly (√(κt) ≈ 55 km in
  100 Myr), but the anomaly's integrated excess mass stays.
- At the kernels' scales (≥ 600 km) that spreading is invisible.
- Radiogenic heating warms a slab by ~0.1 K/Myr, i.e. ~10–30 K in 100–200 Myr, against an
  anomaly of several hundred K.
- Tomography shows slabs for ~200–300 Myr, down to ~2500 km (van der Meer et al. 2010, 2018).
- The real physical decrease of the excess mass with time is the drop of α with depth while the
  slab sinks. It is already in the "with α(P)" rows.
- I found no literature value of a slab "assimilation time" as a loss of excess mass (searched
  2026-11-27; Bower et al. 2015 use a thermal diffusion time scale for slab shapes, not a mass
  sink).

The 660 km phase boundary partly compensates slabs while they cross it: a deflection of
~30–40 km, about 25% of the upper-mantle stage. The exothermic 410 km boundary adds back
comparable mass. Their net effect is small; my rough estimate, not run.

## What this means

- Instantaneous-flow theory with realistic slab masses predicts ±2 km of dynamic topography for
  Earth too. Observations show less: Hoggard et al. 2016 (Nat. Geosci. 9, 456) find typical
  peaks of ±1 km water-loaded at ~1000 km wavelengths and ±0.5 km at ~10⁴ km (checked against
  the abstract, 2026-11-27).
- This gap is a known open problem, the "dynamic topography conundrum" (Molnar et al. 2015;
  Hoggard et al. 2016; Davies et al. 2019). Proposed causes include:
  - weak mantle wedges or asthenospheric channels (Billen & Gurnis 2001);
  - strong slabs resting on the stiffer lower mantle;
  - compensation by chemical or thermal buoyancy elsewhere.

  None of them is settled.
- The model's current stage responses (0.5 / 0.4 / 0.2) already sit **below** the theory, i.e.
  they behave like an effective response that already includes part of that reduction.
- The model's excess depth therefore comes mainly from the slab mass flux, ~1.6× Earth's (#32).
  The check estimated that an Earth-like flux gives −1.0…−1.3 km dry at the deepest point per
  snapshot, i.e. ~−1.5…−1.9 km under water. That is at the upper edge of Earth's observations.

## Options

1. **Wait for #32 (recommended).**
   - Keep the current response and make #37 depend on #32.
   - The flux is the physical cause. Fixing it lowers the depressions by ~40% with no change
     here.
2. **Physical kernels anyway, for consistency, without a free decay.**
   - Layered viscosity (lith 100× / weak asthenosphere / LM 30×), α(P), maybe the 660 km
     boundary.
   - Expect the depressions to get ~10–50% deeper until #32 lands.
   - ~1–2 days with the 8-seed evaluation.
   - It makes the dynamic topography derivable rather than fitted, but #37's symptom gets worse.
3. **Physical kernels plus an explicit, documented calibration to the observed amplitude.**
   - One factor fitted to Hoggard et al. 2016 (±1 km water-loaded), justified as the unresolved
     reduction mechanisms of the conundrum.
   - Honest, but it is calibration (the project prefers physics over tuning). It is not better
     than `topography_response = 0.5` on the current kernels, apart from the kernel shapes.
4. **Model one proposed reduction mechanism.**
   - For example a weak mantle wedge or channel above slabs (Billen & Gurnis 2001), which reduces
     the back-arc depression.
   - Research-grade, uncertain, and it needs lateral viscosity variations that the 1D kernels
     cannot represent.
   - Not recommended at this grid and model level.

**Grid (#64):** not involved. The kernels and slab loads are ≥ 600 km wide; the cells are ~110 km.
