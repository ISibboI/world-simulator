# #182's breakup strength F: how it is used, and a scan of lower values (#51, PR #195)

_2026-11-27. Asked on PR #195 ([comment](https://github.com/ISibboI/world-simulator/pull/195#issuecomment-7221213536)):
"Evaluate decreasing the strength. How is that value used?" Goal: an Earth-like planet, not
Earth; effects of the ~110 km grid are accepted (#64)._

## How F is used

`breakup.strength` (F, default 5 × 10¹³ N/m) is used in exactly one place:
the stress breakup of plates (`src/tectonics/breakup.rs`, #123/#182). Nothing else in the
model (the deforming-plate sheet, the weak-zone splits, the sheet's viscosity) reads it.

**What it is physically.** The depth-integrated strength of old, cold, unthinned lithosphere:
the force per metre of a section's length that the lithosphere can carry before it yields,
i.e. the yield stress integrated over the mechanically strong thickness (a strength envelope
integrated over depth). Unit N/m.

**Per cell it is scaled** by the relative strength `s` of the lithosphere
(`weak_zones::lithosphere_strength`, 0.05–1): thermal thickness `√(age/100 Myr)` (sea-floor age,
time since the last stretching or since a plume heated it), thinning of stretched continental
crust, orogenic weakening of young belts and sutures (to 0.25), and since this PR the
extension fabric of old rifts (#54, to 0.5). Old ocean and old cratons have `s = 1`.

**What it is compared against.** For each plate, candidate sections divide it into two parts A
and B (level sets of distances from three far-apart cells, plus sections through each plume
since this PR, refined along the weakest lithosphere). The section must carry the net torque
`T_A` of all external loads on A: basal drag at the material velocity, slab pull and the
resistances at A's boundaries, ridge push, the push of thick crust and (this PR) the plume
swells. The loads are integrated over A; the strength is integrated along the section. A
rigid-plastic limit analysis (upper-bound theorem) gives the section's **overload**

`S = max_ω (ω · T_A) / D(ω)`,
`D(ω) = Σ_edges F · s · ℓ · (|v_t| + max(v_n, 0) + k · max(−v_n, 0))`

over relative rotations ω of A (v = ω × r on each edge of length ℓ). **Opening** (v_n > 0,
rifting) and **sliding** (v_t, a transform) cost `F s` per metre of displacement, **closing**
(thrusting) costs `k F s` with k = 3 (Byerlee friction is ~3× stronger in compression).
**Bending** is not a separate term: a rotation about a point on the section opens one end and
closes the other, so a section of width w carries a force `F w` but a moment of only ~`F w²/4`.
That is why narrow necks and arms break first. If `S > 1` for 5 Myr (`breakup.time`, the
yielding zone localising; counted in 2-Myr analysis steps, so effectively 6 Myr), the plate
splits there.

So F is a pure scale: **S ∝ (loads) / F**. Halving F doubles every section's overload. Lowering
F is the same as making every load twice as strong, but only for breakup.

**Magma weakening** (`magma_weakening` = 0.1, Buck 2004/2006) lowers only the **opening** term,
`s → max(m s, min(0.05, s))` with `m = 1 − 0.9 · supply`, i.e. 0.1 only at full supply at the
plume's centre, tapering to 1 at the edge of its melt footprint (head radius in the head stage,
then the tail) and with the plume's flux. Sliding and closing keep their unweakened strength.

**Option 3** (propagating plume rifts) analyses, for each plume, sections through the plume as
**magma-fed rifts**: the opening strength along the whole section is lowered by the plume's
magma factor (a dyke-fed rift tip carries the magma along). Its overload `S_rift` uses the same F.
While `S_rift ≥ 1` the rift propagates at 50 km/Myr; it splits the plate once it has crossed it.
Because most overloaded plate-wide sections fail by mixed opening, sliding and bending, the
magma raises the overload by a median factor of only 1.8 (results-propagation.md). So option 3
is gated by F as well: with lower F, more rifts.

## Literature range of integrated lithospheric strength

Order-of-magnitude values from strength envelopes (brittle Byerlee friction over ductile creep,
integrated over depth). They depend strongly on the geotherm, composition, water content and
strain rate:

* **Continental lithosphere**: ~10¹²–10¹³ N/m for normal to warm geotherms, higher than ~10¹³
  only for cold, thick, dry lithosphere (cratons) (Kusznir & Park 1987; Burov & Diament 1995;
  Burov 2011). Forces to rift normal-thickness continental lithosphere without magma are
  ~10¹³ N/m; with dyking ~10¹² N/m or less (Buck 2004, 2006).
* **Old oceanic lithosphere**: several × 10¹³ up to ~10¹⁴ N/m (Kohlstedt, Evans & Mackwell 1995;
  Burov 2011), because olivine is strong and the plate thick and cold.
* **Available driving forces** for comparison: ridge push ~2–4 × 10¹² N/m, net slab pull up to a
  few × 10¹³ N/m (Turcotte & Schubert; Forsyth & Uyeda 1975).

F in the model is the strength at `s = 1`, which the model gives to old ocean, to cratons and to
any continent more than 100 Myr after its last stretching: **F is the strength of the strongest
lithosphere**, whose defensible range is **~3 × 10¹³ to 10¹⁴ N/m** (the 10¹⁴ end from Byerlee +
dry-olivine envelopes, Kohlstedt et al. 1995; low-temperature (Peierls) plasticity, favoured by
newer experiments and flexure, Mei et al. 2010, Hunter & Watts 2016, gives a few × 10¹³ for old
ocean). On that scale **5 × 10¹³ is mid-range** (#123 called it the upper end, against the
older 10¹³–10¹⁴ estimates), **3 × 10¹³ the low end but in range**, **2 × 10¹³ below old ocean**
(fits warm cratons) and **1 × 10¹³ the value of ordinary continents**, too weak for old ocean.
Buck's "tectonic force paradox" (2004, 2006): available forces (ridge push 2–4 × 10¹², net slab
pull ~10¹³ N/m) are too small to rift such lithosphere without magma or weak zones; on Earth,
`s = 1` lithosphere rarely breaks by plate loads alone.

## The scan

**Runs** (all on GitHub runners, `--duration 1000 --snapshot-interval 10`):

* **main**: current main (93323be, with #211) + the PR's diagnostics commit (cherry-picked,
  28dc101; no state change), 8 seeds.
* **PR at F = 5, 3, 2 × 10¹³ N/m**: the PR merged with main (a95dc7f, option 3 as it is),
  `--set breakup.strength=F`, 8 seeds each; **F = 1 × 10¹³**: 4 seeds.
* **Supercontinent start**: the same five arms, `--initial supercontinent --duration 500`,
  4 seeds each.
* Means over snapshots in the window, then over seeds [range]; Welch t against main.
  Script: [`scripts/compare.py`](scripts/compare.py). Full tables:
  [500–1000 Myr](scan-strength-500-1000.md), [0–500 Myr](scan-strength-0-500.md),
  [supercontinent](scan-strength-super.md).

### 500–1000 Myr

| | main | F = 5e13 (PR) | 3e13 | 2e13 | 1e13 (4 seeds) |
|---|---|---|---|---|---|
| plates | 27.7 [22.5–32.6] | 18.1 [9.8–23.1] | 19.2 [11.5–23.6] | 21.7 [11.3–25.9] | 33.0 [31.4–34.5] |
| most plates in any snapshot (per seed) | 34–40 | 20–34 | 22–39 | 23–37 | 40 on all seeds (the cap) |
| largest plate (%) | 17.7 | 33.8 | 32.1 | 30.8 | 23.4 |
| largest plate, max in window (%) | 26.6 | 41.7 | 41.8 | 41.3 | 31.6 |
| births /100 Myr | 10.4 | 6.5 | 7.4 | 8.8 | 15.9 |
| plume rifts /100 Myr | 1.90 (rule) | 0.03 | 0.08 | 0.10 | 0.30 |
| breakups through plume sections /100 Myr | 0 | 0.05 | 0.03 | 0.08 | 0.25 |
| `S_rift ≥ 1` (% of rift analyses) | | 3.8 | 8.8 | 18.4 | 36.9 |
| stress breakups: opening / sliding+closing /100 Myr | 2.2 / 0.5 | 1.7 / 0.4 | 1.9 / 0.6 | 2.6 / 1.3 | 5.2 / 4.3 |
| subduction initiations /100 Myr | 4.9 | 3.9 | 4.5 | 4.1 | 5.7 |
| splits ≤ 500 km of a plume /100 Myr | 2.5 | 0.7 | 0.7 | 0.9 | 2.0 |
| continental crust (% of surface) | 33.0 | 33.5 | 34.9 | 33.8 | 33.8 |
| land (%) | 27.4 | 29.3 | 29.0 | 28.0 | 27.4 |
| mean land elevation (m) | 594 | 564 | 508 | 507 | 590 |
| crust > 50 km (% of continental) | 1.4 | 1.3 | 1.0 | 1.0 | 1.9 |
| #54 fabric ≥ 0.25: near margins / deep inside (%) | | 3.7 / 9.7 | 3.6 / 7.7 | 4.4 / 10.3 | 6.3 / 12.9 |
| #54 new boundaries on fabric vs split plates (%) | | 27 vs 10 | 22 vs 10 | 31 vs 13 | 36 vs 17 |
| runtime per seed (s, GitHub runners) | 1171 | 1128 | 1000 | 1081 | 1130 |

0–500 Myr, plates: main 26.0, F = 5/3/2/1 × 10¹³: 21.5 / 23.3 / 24.1 / 31.6; largest plate
18.0 vs 23.5 / 22.0 / 22.4 / 19.8%; plume rifts 0.05 / 0.17 / 0.12 / 0.65 per 100 Myr.

**Supercontinent start** (4 seeds × 500 Myr; the largest continental mass after 50 Myr, lowest
per seed): main 49, 86, 39, 61%; F = 5e13: 94, 63, 96, 88%; 3e13: 97, 62, 97, 99%; 2e13: 43,
56, 78, 91%; 1e13: 99, 88, 55, 91%. All arms, main included, re-assemble to 96–100% by 500 Myr.
Plume rifts from the supercontinent: 0–0.1 per 100 Myr at every F. Lower F does not disperse
the supercontinent: the sections through a supercontinent's interior are long, and its own
loads (drag under continents) do not pull it apart.

Runtime: no measurable effect of F (GitHub runners differ by ±15%).

## Independent evaluation

[`review-strength.md`](review-strength.md): the explanation above matches the code (its four
small corrections are adopted above); with Welch t against F = 5e13 recomputed from the logs,
**5e13 and 3e13 cannot be told apart on any metric** (|t| < 1.2); at **2e13** plates (t +1.6) and
the largest plate (t −0.7) are still within the seed scatter, and the one significant change is
3.4× more breakups by sliding or closing (t +4.6): strong lithosphere sheared or thrust apart,
rare on Earth. **1e13 is unphysical**: old ocean below every estimate, all seeds at the 40-plate
cap, sliding/closing breakups (4.3) nearly as many as opening ones (5.2).

## Recommendation (not changed in the PR; Sebastian decides)

**Keep F = 5 × 10¹³ N/m** (3 × 10¹³ is equally defensible and gives the same results). Both lie in
the literature range for the lithosphere that `s = 1` stands for, and the model's loads are at
the upper end (basal drag 10¹⁵ Pa·s/m, ×4 under continents), which argues against lowering F:
only loads / F matters. Not 2 × 10¹³ or below.

**Lowering F is not the lever for the largest plate** (31–34% vs main's 18%): from 5e13 to 2e13 it
moves ~3 points, within scatter; only 1e13 closes the gap, by breaking strong lithosphere
everywhere. The gap is what the plume rule used to do (splits within 500 km of a plume:
main 2.5, PR 0.7 per 100 Myr). Physical levers, each to be tested on its own (a new issue if
wanted):

1. the relative strength of ordinary (non-cratonic) continental lithosphere: today any continent
   100 Myr after its last stretching has `s = 1`, as strong as a craton and old ocean; at
   `s ≈ 0.3–0.5` it would sit at ~1–2 × 10¹³ N/m, its literature range, without weakening old
   ocean;
2. heating (thermal blanketing) of the lithosphere under a long-lived supercontinent;
3. plume magma also weakening sliding (oblique opening of dyke-fed rifts), not only opening.

## The plume rule (removed in 42f8aab)

Yes, it was a non-physical patch: every plume older than 20 Myr split the plate above it once
(if continental crust was within reach or the plate covered ≥ 15% of the surface), whatever the
plate's loads and strength. Only *where* the break ran was physical (the weakest path through the
plume's heated lithosphere); *whether* and *when* were rules that encoded the observed link
between plumes (large igneous provinces) and breakups, without the forces. It stays out.
