# Review: lowering the breakup strength F (#51, PR #195)

_2026-11-27, independent evaluation of [`strength.md`](strength.md) and the scan tables. Goal: an
Earth-like planet, not Earth; effects of the ~110 km grid are accepted (#64)._

## 1. Does strength.md describe the code correctly?

Yes, in substance. Checked against `breakup.rs` (module doc, `SectionEdge::halves`, `dissipation`,
`limit_overload`, `analyse_sections`, `section_force`, `rift_tension`) and `weak_zones.rs`:

* F is read only in `section_force()` (F × planet radius) and enters `D(ω)` linearly; sections are
  chosen and refined by the relative strength `s` alone (minimum cut), so **S ∝ loads / F exactly**
  and the ranking of sections does not depend on F. The only other use is a unit test
  (`matching.rs`, 1e12). "Exactly one place" is right.
* `s` terms match: `√(age/100 Myr)`, thinning `h/35 km`, orogenic to 0.25, fabric to 0.5, floor 0.05.
* Small inaccuracies: (a) the ordinary breakup needs `S > 1` (strict), the rift `S_rift ≥ 1`;
  (b) "5 Myr" is effectively 6 Myr (2-Myr analysis steps; said in #123, worth repeating);
  (c) magma weakening is `s → max(m·s, min(0.05, s))` with `m = 1 − 0.9·supply`, i.e. 0.1 only at
  full supply at the plume centre (the text says "tapered", fine); (d) sliding uses the undyked `s`
  and closing `k·s` undyked, as stated. No errors that change the conclusions.

## 2. Literature: where do 5, 3, 2, 1 × 10¹³ N/m sit?

* Continental lithosphere, ordinary geotherms: ~10¹²–10¹³ N/m (Kusznir & Park 1987; Burov &
  Diament 1995; Burov 2011); cold, thick cratons: several 10¹³ and more.
* Old ocean: Byerlee + dry olivine envelopes (Kohlstedt, Evans & Mackwell 1995) give ~10¹⁴ N/m for
  ≥ 80 Myr; with low-temperature (Peierls) plasticity, which newer experiments and flexure
  favour (Mei et al. 2010; Hunter & Watts 2016), the strong core is weaker: a few × 10¹³ N/m.
* Buck (2004, 2006): amagmatic rifting of normal continental lithosphere needs ≳ 10¹³ N/m, more
  than the forces available (ridge push 2–4 × 10¹², net transmitted slab pull ~10¹³) ("tectonic
  force paradox"); dyking lowers it by ~10×. Corollary: on Earth, s = 1 lithosphere (old ocean,
  cratons) essentially does not break by plate loads alone; breakup follows weak zones and magma.
* In the model `s = 1` stands for old ocean AND cratons AND any continent > 100 Myr after its last
  stretching. So F is the strength of the *strongest* lithosphere: the defensible range is
  **~3 × 10¹³ to 10¹⁴**. 5e13: middle of that range (not really "upper end" once Peierls creep is
  considered). 3e13: lower end, still in range. 2e13: below old ocean, fits only warm cratons.
  1e13: ordinary continents, clearly too weak for old ocean.
* Loads: basal drag 10¹⁵ Pa·s/m (1.6 MPa at 5 cm/yr, ×4 under continents) is upper-end
  (asthenosphere 10¹⁹–10²⁰ Pa·s over 100–200 km gives 5 × 10¹³–10¹⁵). Drag × 4 under a 3000 km
  continent alone is ~2 × 10¹³ N/m. Only loads/F matters: upper-end loads with a mid/low F
  overstates breakup, which argues against lowering F rather than for it.
* Structural caveat (not an F issue): ordinary Phanerozoic continental lithosphere gets s = 1, the
  same as cratons. Lowering F globally to weaken continents also weakens old ocean.

## 3. The scan (500–1000 Myr; Welch t against F = 5e13, recomputed from the logs)

| | 3e13 | 2e13 | 1e13 |
|---|---|---|---|
| plates (5e13: 18.1) | 19.2, t +0.5 | 21.7, t +1.6 | 33.0, t +9.3 (cap 40 on all seeds) |
| largest plate (5e13: 33.8%) | 32.1, t −0.5 | 30.8, t −0.7 | 23.4, t −2.6 |
| largest plate, max in window (41.7%) | 41.8, t 0.0 | 41.3, t −0.1 | 31.6, t −1.9 |
| stress breakups sliding/closing (0.38) | 0.55, t +1.1 | 1.30, t +4.6 | 4.25, t +10.4 |
| plume rifts /100 Myr (0.03) | 0.08 | 0.10 | 0.30 |

* **5e13 vs 3e13: indistinguishable** on every metric (|t| < 1.2). **2e13**: plate count and
  largest plate still within seed scatter (t < 1.7); the one significant change is a 3.4× rise of
  breakups by sliding/closing, i.e. plates sheared or thrust apart through strong lithosphere,
  which the sheet should take up as distributed deformation and which is rare on Earth (diffuse
  boundaries such as the Indo-Australian are the exception). Plume rifts stay ~0.1 per 100 Myr,
  ~20× below main's rule rate: F does not revive plume-driven rifting.
* **1e13 is unphysical**: old ocean at 1e13 is below every estimate; all seeds hit the 40-plate
  cap; sliding/closing breakups (4.3) rival opening (5.2); births and deaths ~1.6× main. It only
  reaches a main-like largest plate by over-fragmenting everything.
* Supercontinent start: no F disperses it (lowest largest-continent fraction after 50 Myr is
  noisy and non-monotonic in F); all arms re-assemble to 96–100%. The largest continent holds
  ~89–96% of continental crust at 500–1000 Myr in every arm, main included.
* Land, continental area, thick crust, mean land elevation: no significant F effect (|t| < 1.4).

## 4. Recommendation

* **Keep F = 5 × 10¹³ N/m** (3 × 10¹³ is equally defensible and gives the same results; if a
  rounder "middle of old-ocean estimates with Peierls creep" is preferred, 3e13 is fine). Both lie
  in the literature range for the lithosphere that `s = 1` represents (old ocean, cratons) and are
  consistent with the upper-end drag. Do not go to 2e13 or below: it is below old-ocean strength,
  combined with upper-end loads, and its only significant effect is more shear/thrust breakups.
* **Lowering F is not the lever for the largest-plate deficit** (~31–34% vs main's 18%). From
  5e13 to 2e13 the largest plate moves 3 points within seed scatter; only 1e13 closes the gap, and
  only by breaking strong lithosphere everywhere. The deficit comes from removing the plume rule
  (~1.9 splits/100 Myr, mostly near plumes: splits ≤ 500 km of a plume 2.5 → 0.7), and option 3's
  magma-fed rifts are too rarely overloaded (median gain ×1.8, since sections fail by mixed modes).
* Levers that are physical, in order: (1) the continental `s` (Phanerozoic vs cratonic
  lithosphere; a non-cratonic continent at s ≈ 0.3–0.5 would put it at ~1–2 × 10¹³ N/m, in its
  literature range, without weakening old ocean); (2) thermal blanketing/plume heating of the
  lithosphere under a long-lived supercontinent (more weakening in time and area); (3) magma
  weakening of sliding as well as opening near plumes (dyke-fed rifts accommodate oblique opening).
  Each needs its own test against the same scan metrics.
