# Feasibility of the open issues within the current framework (2026-11-15)

Research agent (web + reasoning) asked whether issues 1–9 in `../ISSUES.md` can be solved in
the current framework (rigid resampled plates, local Airy isostasy, torque balance with a
parameterised mantle, ~110 km / 1 Myr).

**Verdict:** 2, 3, 5, 6, 9 yes; 4, 7, 8 mostly (each needs one extra parameterised ingredient);
1 only as a *statistically* stable state, by closing the loop with state-dependent plate
birth/death rules (hazard functions) — a torque balance with prescribed mantle flow has no
feedback that stabilises plate number or ocean age, and even self-consistent 3D convection
models only give statistically cyclic behaviour (~2× variation of seafloor production over a
Wilson cycle). No 3D convection, viscoplastic lithosphere or GCM is needed for "Earth-like within
20–30%".

| # | Approach | References |
|---|---|---|
| 1 | Keep torque balance; add (a) subduction-initiation hazard rising steeply with age (~0 below 80 Myr, high above 150 Myr) at passive margins and fracture zones plus a backstop ~180–200 Myr, (b) trench rollback, (c) ridge subduction → plate reorganisation, (d) fragmentation at long/curved trenches (microplates, back-arc spreading) and plume rifting, (e) suturing. **Lateral trench propagation along adjacent old passive margins** is probably the cheapest high-impact addition (Scotia, Lesser Antilles). Treat the triangular area–age distribution as a diagnostic. Slowing of continental plates is real (Forsyth & Uyeda): fix with more trench length, not faster continents. Expect ±30–50% fluctuations in mean age. Targets: time-mean ocean age 55–75 Myr, max < ~200 Myr, ~8–15 major plates, continental fraction 0.3–0.4, bounded variance. Rifting probability rising with plate area and time since supercontinent assembly (Gurnis 1988) gives a supercontinent cycle. | Forsyth & Uyeda 1975; Conrad & Lithgow-Bertelloni 2002; Parsons 1982; Labrosse & Jaupart 2007; Becker et al. 2009; Coltice et al. 2012, 2013; Rolf et al. 2014; Mallard et al. 2016; Cortial et al. 2019; Stern & Gerya 2018; Mueller & Phillips 1991 |
| 2 | Solve sea level by bisection for fixed water volume, with water-load isostasy (otherwise sea-level changes ~30% too large); optional ice-sheet storage. **Do after issue 1's age distribution is plausible** (basin volume depends on ages). | Müller et al. 2008 |
| 3 | Conserve mass (bedrock→sediment ×ρc/ρs), deep-sea fans instead of deletion, accrete ~50–70% of trench sediment, replace thickness cap by flow/eclogite (issue 4), conservative sample↔cell scatter, global mass ledger with asserts. Target arc addition ≈ subduction erosion + sediment subduction ≈ 3 km³/yr; continental volume ~constant over 2 Gyr. | Scholl & von Huene 2009; Clift et al. 2009; Stern & Scholl 2010 |
| 4 | Separate thermal age: continental/arc crust gets a continental geotherm (no GDH1 term); rifting resets it (McKenzie). Eclogitisation: crust below ~50–55 km depth → 3300–3500 kg/m³, limits elevation to ~5 km and enables delamination (controlled mass return). | Stein & Stein 1992; Jull & Kelemen 2001; Bousquet et al. 1997; McKenzie 1978 |
| 5 | Create age-0 crust only where the two nearest plates diverge (relative velocity across the gap); otherwise extend the nearest/overriding plate's sample (flagged, no new mass); threshold on divergence speed against aliasing. | Cortial et al. 2019 |
| 6 | Isostasy-aware implicit stream power (or Picard iterations); recalibrate K (eroded volume rises ~5×). Runoff = P − AET (Budyko/Fu/Zhang curve, PET from temperature). Endorheic lakes with water balance (spill only if inflow > (E−P)·A). | Braun & Willett 2013; Yuan et al. 2019; Barnes et al. 2014; Zhang et al. 2001; Matsubara et al. 2011 |
| 7 | McKenzie stretching (β 2–4) of continental crust at diverging continental boundaries + thermal subsidence from rift age → shelves on continental crust; marine sediment diffusion and per-step progradation limit; optional thin-plate flexure (Te 20–40 km). Under Airy, filling 5 km of ocean needs ~13 km sediment, so the 11 km piles are what Airy predicts for sediment on oceanic crust — the margin crust matters more than flexure. | McKenzie 1978; Yuan et al. 2019; Wickert 2016 (gFlex) |
| 8 | Conservative 2D advection–diffusion of vapour (ΣP = ΣE), seasonal winds, bulk evaporation from EBM temperature, P = q/τ + convergence + orographic (Smith–Barstad); ITCZ/Hadley edge follow the energy-flux equator; moist EBM to calibrate zonal P−E. Monsoons only crude. | Siler et al. 2018; Smith & Barstad 2004; Bischoff & Schneider 2014; Petoukhov et al. 2000 |
| 9 | Seasonal land/ocean EBM: land C 1/30–1/60 of mixed layer but ≥ ~1e7 J/m²/K; latitude-dependent diffusivity; sea ice with lower heat capacity, higher albedo; moist-static-energy diffusion; optional prescribed ocean heat flux. | North et al. 1981; North, Mengel & Short 1983; Siler et al. 2018 |

**Recommended order:** 3 + 5 (bookkeeping; they corrupt the measurement of issue 1) → 4 →
tune issue 1 against clean bookkeeping (trench propagation, age-hazard initiation, long runs over
several seeds) → 2 → 7 + 6 → 9 → 8.

**Fundamental limits:** no emergent plate boundaries (parameterised birth/death unavoidable),
no dynamic topography or flexural trenches/forebulges, no ocean currents or monsoon dynamics.
