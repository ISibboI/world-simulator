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
