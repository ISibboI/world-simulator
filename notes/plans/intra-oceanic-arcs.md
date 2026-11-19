# Plan: let intra-oceanic arcs mature (continental balance, step after accretionary margins)

_2026-11-19. From the current plan in `SUMMARY.md`: measure intra-oceanic arcs, then plan._

## Measured (scratch worktree `agent-scratch/arc-diag/`, seeds 0/1, 500 Myr, 150–500 Myr)

Felsic mass by the class of the crust sample (oceanic: felsic ≤ 0.02; arc-like: felsic > 0.02
and not of continental origin; continental origin), Gt/yr:

| | oceanic | arc-like | continental origin |
|---|---|---|---|
| arc felsic added | 0.6–0.7 | 2.4–2.5 | 2.5–3.3 |
| subducted | 0.3 | 1.9–2.3 | 0 |
| stacked in collisions | 0 | 0.15–0.2 | 6–15 (churn) |
| eroded at the forearc | 0 | 0.3 | 0.4–0.6 |

Arc-like felsic stock grows only 0.2–0.35 Gt/yr; ~0.1–0.3 Gt/yr matures into continental-origin
crust. **Arc-like crust stays thin**: ~90% of arc-like samples < 12 km, 0.4–0.6% > 20 km, ≤ 5 > 30
km (Earth's intra-oceanic arcs: 20–35 km; Izu–Bonin–Mariana, Aleutians). Of the arc-like felsic
subducted (seed 0, 0–300 Myr, 567 Gt): 48% in crust < 9 km, 34% 9–12 km, 17% > 12 km; 26% at felsic
≤ 0.1, 41% at 0.1–0.3, 33% > 0.3 — mostly thin arc aprons on normal oceanic crust.

Cause: `arc_profile` [0.25, 1, 1, 0.25] spreads 500 m·cell per subducted sample over a half disc
of ~3 rings (~330 km) on the upper plate, including the trench cell (ring 0, which is eroded and
often overridden). Total production is Earth-like (~25 km³ per km of arc per Myr at 5 cm/yr;
Reymer & Schubert 1984: 20–40), but spread over ~330 km it takes ~270 Myr to build 20 km of
crust, longer than trench segments persist; the thin crust then subducts when its plate goes
down elsewhere. On Earth arc magmatism is concentrated 100–200 km behind the trench
(volcanic front ~100–150 km behind it, amagmatic forearc), arcs reach 20–30 km in ~50–100 Myr,
and once thicker than ~15–17 km they resist subduction (Cloos 1993) and are accreted.

## Design

1. **Concentrate arc magmatism:** `arc_profile` [0, 1, 0.5] — nothing on the trench cell
   (amagmatic forearc; it is also the cell eroded by the slab), most at 1 ring (~110 km), half
   at 2 rings. Same total; frontal accretion keeps its own profile at the trench (split the
   profile: `accretion_profile` [1, 0.5] for accreted wedges).
2. No change to `oceanic_arc_felsic`, resistance thresholds or terrane rules: thicker arcs
   resist subduction by the existing buoyancy criterion and are then accreted by collisions and
   terrane accretion.

## Expected effects and checks

Arc-like samples > 15–20 km become common; arc-like felsic subducted falls; stacked / terrane
accretion of arcs and maturation into continental origin rise; felsic + sediment 4-seed mean
towards 0 to −1.5 Gt/yr after 150 Myr. Watch: collision length (more buoyant arcs at trenches →
more collisions), terrane accretions, polarity flips, arc elevations (arcs emerge as island
chains), flooded, mean land, sediment part of the sea level.

Tests: arc crust is added behind the trench, none on the trench cell; accreted sediment still
lands at the trench.

## Plan review (independent agent, 2026-11-19): not sound as written — changes adopted

Measured (`agent-scratch/review-arcs/`, per-sample tags of the plate whose subduction built the
arc, seeds 0/1, 150–300 Myr, Gt/yr): arc-like felsic subducted 2.57 / 2.09, of which **under the
plate whose subduction built it, within 10 Myr of its last arc addition 1.12 / 0.86 (~43%)** —
consumed at its own active boundary: 0.40 (seed 0) at the same segment, where the per-cell rule
"buoyant bottom crust stays on top" (`Polarities::contact`) subducts the thin upper-plate arc
below arriving buoyant crust (Earth: the arc rides over it and is accreted — Taiwan, Banda,
Kohistan); 0.35 + 0.19 at a different segment of the pair with the opposite polarity (arc profile
spilling along strike onto reversed segments, or segments re-created by buoyancy); 0.18 genuine
flips; 72% at mature segments. Under another plate within 10 Myr 0.65 / 0.41 (triple junctions);
after > 10 Myr ("its plate goes down elsewhere", the plan's mechanism) 0.62 / 0.67 (24–32%). The
~14.6 km threshold works, no flicker. Profile [0, 1, 0.5] alone (seeds 0/1, 300 Myr): subducted
arc-like 2.57 → 2.04 / 2.09 → 1.95, arcs 12–20 km 365 → 477 / 512 → 432, 20–30 km 10 → 12 / 24 →
13; net felsic + sediment inconclusive; collision length tends up — **arcs do not get thick**.
Earth: intra-oceanic arc growth 30–95 km³/km/Myr (Dimalanta et al. 2002; Aleutians ~55), volcanic
front ~100–200 km, mature arcs 20–35 km, resist > 15–17 km.
Required (adopted): (1) at inverted contacts (segment says the arc plate overrides, buoyant
arriving crust stays on top) accrete the upper plate's crust onto the arriving crust instead of
subducting it; (2) restrict the arc profile to cells behind their own segment's trench (skip cells
bordering the partner along a segment where the partner overrides); (3) keep [0, 1, 0.5] as
realism; test nothing on the trench cell; (4) re-measure, then consider arc growth (less
dependent on convergence, or modestly higher within 30–95); (5) watch collision length, flips,
arc elevations, the arc-like class.

Implemented 1–3: `overridden_upper_plate` in the consumption (booked as collision stacking,
logged as "upper plates accreted to buoyant arrivals"), `borders_reversed_segment` for arc
additions (`CrustAddition::partner`), `arc_profile` [0, 1, 0.5], `accretion_profile` [1, 0.5].
Tests: arc crust grows behind the trench, none on the trench cell; a wedged margin arriving below
an oceanic upper plate gets the upper plate's crust stacked onto it (fails without the fix).

## Results (seeds 0–3, 500 Myr)

First version (`agent-scratch/arcs2/`, overridden upper-plate crust of any kind accreted):
felsic + sediment after 150 Myr 4-seed mean −0.58 Gt/yr, but 4.3–6.7 Gt/yr of mostly normal
oceanic forearc crust was stacked onto arriving continents: stacking 28–34 Gt/yr, collision length
14–22k km, > 50 km 4–8%, mean land 0.67–0.99 km, sediment part of the sea level +370…+435 m.
Restricted to non-oceanic (arc) crust (`agent-scratch/arcs3/`; ophiolite obduction is minor):
upper-plate arc crust accreted 0.5–0.7 Gt/yr; **felsic + sediment 4-seed mean −0.97 Gt/yr**
(seeds −0.29 / −1.41 / −0.86 / −1.31; before −1.56; target 0 to −1.5 met), −3.5…−6.3% per
500 Myr; collision length 12–17k km (before 7–14k), stacking 15–22 Gt/yr, > 50 km 1.7–2.6%,
mean land 0.52–0.66 km, flooded 17–22%, ocean age 57–72 Myr. Sediment part of the sea level,
4-seed mean 252 → 318 → 376 m at 300/400/500 Myr (+66, +58 per 100 Myr; before +57, +25): not
decelerating (seeds 1 and 3 accelerate) — the sea-level target is missed again.
