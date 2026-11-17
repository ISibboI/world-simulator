# Plan: accretionary vs erosive margins (trench sediment follow-up)

_2026-11-17. User: after the intra-oceanic arc felsic fix, pursue accretionary margins next._

## State

`boundaries.rs`: at every subducted sample, sediment up to `subduction_channel_thickness`
(1 km) plus 70% of the excess subducts, `frontal_accretion_fraction` 0.3 of the excess is
accreted to the overriding plate as metasediment; every overriding trench sample is eroded by
`subduction_erosion` (300 m per subducted sample, sediment first). Trench input 2.4–6.9 Gt/yr,
~20% accreted; ~80% of the input is thick fill on oceanic crust (continental rise, fans).

## Earth (Clift & Vannucchi 2004; von Huene & Scholl 1991; Scholl & von Huene 2007)

- ~57% of margins are erosive: trench fill < 1 km and/or convergence > ~6–7.5 cm/yr; all
  incoming sediment subducts, and the forearc is eroded (~1.4 km³/yr globally).
- ~43% are accretionary: fill > 1 km and convergence < ~7.6 cm/yr; the frontal prism takes the
  part above the décollement (often ~half of the column or more at thick-fill margins: Nankai,
  Makran, Cascadia, Barbados); the rest subducts or is underplated; little or no forearc
  erosion.

## Design

Per subducted sample (consumption in `boundaries.rs`), with the convergence rate at the contact
(relative plate velocity at the cell from the plates' angular velocities, projected on the
boundary normal or its magnitude):

1. **Erosive** if sediment ≤ `subduction_channel_thickness` or convergence >
   `erosive_convergence` (~7 cm/yr): all sediment subducts; the overriding sample is eroded as
   now.
2. **Accretionary** otherwise: a décollement at `decollement_fraction` (~0.5) of the column — the
   top part is accreted as metasediment, the rest subducts; the overriding sample is **not**
   eroded (or much less).
3. Replaces `frontal_accretion_fraction`. Diagnostics: share of trench samples / sediment at
   accretionary vs erosive margins, accreted share, forearc erosion rate.

## Expected effects (to check, not tuned to)

- Accreted share of trench sediment rises from ~20% towards ~30–40% (thick fill dominates the
  mass); subduction erosion falls (fewer eroded forearcs) → felsic loss down.
- **Caveat: sea level.** Accreted sediment stays on the surface (forearc crust), so more
  accretion removes less sediment from the surface than now; the sediment part of the sea level
  would likely rise, not fall. This step is about realism of margins and the felsic balance,
  not the sea-level trend. On Earth the marine sediment stock is removed mainly when margins are
  consumed in orogens (collision stacking), not by subduction; whether a flat sea-level part is
  a realistic target at all needs checking (what is Earth's long-term trend of sediment-driven
  sea level?).

## Verification

Test: thick fill + slow convergence accretes the top fraction and does not erode the forearc;
thin fill or fast convergence subducts all sediment and erodes. Runs seeds 0–3, 500 Myr:
accreted share, forearc erosion, felsic + sediment rate, sediment part of the sea level,
collision length, flooded, mean land.

## Plan review (independent agent, 2026-11-17): sound in direction, with changes — adopted

Felsic + sediment flows (`agent-scratch/review-accretion/`, seeds 0/1, 150–300 Myr, Gt/yr; residual
≈ 0): gains — arcs on non-oceanic upper plates +3.59 / +3.41, intra-oceanic arcs (×0.4)
+1.51 / +1.77, frontal accretion +0.78 / +1.12, erosion of mafic crust booked as sediment
(tracer artefact) +0.26 / +0.30; losses — sediment subducted −3.95 / −5.21, felsic in subducted
crust with felsic < 0.5 (recycled arcs, mostly < 12 km thick) −1.75 / −2.24, felsic ≥ 0.5 −0.17 /
−0.34, subduction erosion −1.85 / −1.70 (crust 0.85 / 0.74, sediment 1.0 / 0.96; ~70% at
continental forearcs), collision stacking cut at max thickness −0.01 / −0.41; net −1.59 / −3.30.
The intra-oceanic loop is nearly closed; the deficit is in the continental-margin system
(continental arcs + accretion < sediment subduction + forearc erosion), each gross flux Earth-
like. Missing: maturation of intra-oceanic arcs (20–35 km) and their transfer into continents.
Earth may be at 0 to −2 Gt/yr (Scholl & von Huene 2007). The continental-origin area loss is
mostly mass now in the sediment stock and non-continental-class crust.

Design: per-sample classification is too narrow (only 4.7–5.7% of subducted samples have
> 1 km fill and < 7 cm/yr; erosion at their forearcs 0.14–0.16 Gt/yr): per-sample gain only
+0.55 / +0.78 Gt/yr (upper bound). Required (adopted): (1) classify per polarity segment by a
time-averaged (~5–10 Myr) mass-weighted incoming fill and the segment's trench-normal
convergence (not the magnitude — strike-slip inflates it), hysteresis 6–7.6 cm/yr; (2)
décollement on the whole column (0.4–0.5), the channel always subducted (accreted share
~30–33%); (3) no or reduced forearc erosion at accretionary segments; report trench length by
type (Earth 57% erosive / 43% accretionary) and forearc erosion (km³/yr); (4) **sea-level caveat
was wrong**: the incoming sediment leaves the sediment stock whether accreted or subducted;
accreted metasediment raises the forearc (+5–15 m per 100 Myr in the tectonic part); (5) report
the mafic-erosion tracer artefact. Targets re-derived: continental-system net 0 to −1.5 Gt/yr as
the 4-seed mean over 150–500 Myr; sediment part of the sea level: 4-seed mean decelerating and
≲ 20 m per 100 Myr after 300 Myr (Earth's stock relaxes to a bounded level and swings with
Wilson cycles; no per-seed flatness). Next lever after this: maturing intra-oceanic arcs (their
thickness vs Earth's 20–35 km, losses to their own forearc erosion vs terrane accretion); seed
1's collision-cap loss separately.

## Result (seeds 0–3, 500 Myr, `agent-scratch/accretion/`, vs `agent-scratch/arcs/`)

Per-segment classification (fill averaged over 10 Myr by time since the last update, since
samples are consumed only now and then per segment; trench-normal convergence; hysteresis
6–7.6 cm/yr), décollement 0.45 of the column, no forearc erosion at accretionary segments.
Accreted share of trench sediment 21–32% (was ~20%; Earth long-term ≲ 20–30%); 51–75% of the
trench sediment arrives at accretionary segments, which are only 4–21% of the trench length
(Earth ~43%: most model trenches carry thin pelagic fill). Subduction erosion 2.1–3.3 Gt/yr
(crust mass). Felsic + sediment after 150 Myr −0.9…−2.7 Gt/yr, 4-seed mean −1.56 (before
−1.52) — the target (0 to −1.5) is just missed, the change is neutral for the balance.
Sediment part of the sea level, 4-seed mean: 245 → 302 → 327 m at 300/400/500 Myr, i.e. +57 then
+25 m per 100 Myr (before +40 / +31): decelerating, close to the ≲ 20 m per 100 Myr target.
Collision length 7–14k km, flooded 16–22%, mean land 0.52–0.75 km, ocean age 52–68 Myr.
