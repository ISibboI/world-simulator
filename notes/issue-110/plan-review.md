# Review of notes/issue-110/plan.md (tectonic activity view)

Reviewed against the worktree at ae5f2c0, which already has a `motion` field in `BoundaryMap` and a new
`src/tectonics/activity.rs`. I checked the numbers by hand. Overall the plan is sound: the
B&K/Kostrov boundary formula, the dips and μ are applied the way B&K did, the data exists
where the plan expects it, and the view reads the state without changing it. The must-fix items
below are about consistency and wrong expected values, not about the approach.

## Must fix

1. **The expected ridge values are about 10³ too low.** With c z = 0.13 km, δ = 55°,
   μ = 27.7 GPa and v = 5 cm/yr, Ṁ/L = 3.8 × 10¹¹ N·m yr⁻¹ m⁻¹. Per cell (½ · s/A, s ≈ 110 km,
   A ≈ 1.05 × 10⁴ km²) that is **~2 × 10¹²** N·m yr⁻¹ km⁻², not ~10⁹. The "≈ Mw 5 at a fast ridge"
   is really **≈ Mw 6.1**. Fix the text, and any test or legend anchor based on it. The
   subduction (2.3 × 10¹⁴ per m, ~1.2 × 10¹⁵ per km², Mw ≈ 8.0, ~10²² globally), slow OTF
   (~6 × 10¹³) and collision (~2.4 × 10¹⁴ at 3 cm/yr) numbers check out.
2. **Clamp the rates by sign.** A cell's class does not guarantee the sign of v_n.
   `Subducting`/`Overriding` stays assigned through the slab hysteresis
   (`convergence > -min_convergence`), and Ridge/Rift can differ in sign from the cell's
   `motion`, which comes from the deciding boundary. So use max(v_n, 0) for the convergent rows
   and max(−v_n, 0) for the opening rows (ridge and rift magma included). Otherwise a weakly
   opening "trench" gives negative moment or magma and NaN in log₁₀.
3. **Make the boundary and distributed moment consistent.** At the boundary, CCB gives
   μ · 18 km · v / (sin20° cos20°) = **56 μ v** (km) per length. The same shortening taken up
   by #182's band gives 2 μ · 9 km · v = **18 μ v**, which is 3× less. For rifts it goes the
   other way: CRB 3 km / (sin55° cos55°) = 6.4 μ v at the boundary against 18 μ v distributed
   (2.8× more). The map would then change by ~3× whenever the sheet takes over part of a
   collision or rift (`record_partitioning` shows this partitioning is real). Fix: classify each
   strained cell by style using the dilatation the sheet already stores (`strain_rate()` returns
   the second invariant and the area rate):
   - shortening: CCB, or OCB for oceanic lithosphere
   - extension: CRB, or OSR for oceanic
   - otherwise: CTF/OTF

   Then use that class's c z with the same 1/(sinδ cosδ) dip factor. This is what Bird's SHIFT
   (Bird et al. 2010, Bird & Kreemer 2015) does, with the Savage–Simpson style factor. If you
   keep the single 9 km value, state the factor-3 jump as a known caveat.

## Should

4. **Set a floor on distributed strain.** At ε̇ = 10⁻¹⁷ s⁻¹ (0.3 mm/yr per 1000 km, i.e.
   numerical creep), 2 μ · 9 km · ε̇ is already 1.6 × 10¹¹ N·m yr⁻¹ km⁻², inside the colour range
   that starts at 10⁹. Any small strain the sheet carries would then tint whole deforming plates.
   Use a floor of about 10⁻¹⁷ s⁻¹ (or the sheet's own threshold) and say "stable interiors: not
   modelled" in the legend.
5. **Arcs at diffuse zones.** The plan says "the model grows no arc either", but in
   `boundaries.rs` (~l. 405–440) overlaps at non-localised segments *do* subduct and get an
   `AdditionKind::Arc` (counted in `polarities.statistics.diffuse_subducted`). The same may apply
   to OtherConvergent overlaps. Either include Diffuse (and OtherConvergent where crust is
   actually consumed) in the arc source, or justify leaving it out with the
   `diffuse_subducted` share from a run.
6. **The arc total against Earth.** At 25 km³ km⁻¹ Myr⁻¹ (500 m per subducted cell) and
   ~45 000 km of trenches, the model's arc total is **~1–1.5 km³/yr**, below the "Earth ~3–8"
   quoted (Crisp's total, intrusive included). It is consistent with Reymer & Schubert, so
   state the expected model total next to the Earth range. Also, the doc comment on
   `arc_crust_per_subduction` says ~800 m matches Earth while the value is 500 m. Mention it,
   but don't change the parameter here; the view should mirror the model.
7. **Ridge markers in "Both".** At 10⁻² mm/yr every ridge cell (~1.6 mm/yr) gets a triangle,
   so the 65 000 km of ridges become a chain of markers that drowns the arcs and hotspots.
   Volcano maps normally show subaerial or edifice volcanism. Draw markers for arc, hotspot
   and rift sources only, leave ridges to the colour, and say so in the legend.
8. **Check B&K's exact definitions.** c z was fitted with B&K's own geometry, so a different
   factor biases the result by 1/cos δ (3 % at 14°, 74 % at 55°). Confirm against the paper (not
   only the abstract):
   - whether v is the heave (the plan's 1/(sinδ cosδ)) or the slip (1/sinδ)
   - the OTF speed classes (thresholds 40 and 68 mm/yr) and their c z
   - that SUB is 18 km

   Also note the 7× step in OTF c z at 4 cm/yr: it is real per B&K but will show as a step
   along a transform that speeds up. Fine, but state it.
9. **Cells on several boundaries.** `BoundaryMap.motion` keeps only the deciding boundary.
   At triple junctions and one-cell slivers the other contact's moment is dropped. Better:
   accumulate in `activity()` over all `boundary_cells()` entries of the cell, each with its own
   class. At minimum, note it as an underestimate.
10. **Plume lid factor.** Evaluate Λ once, at the cell above the plume centre (as #106 does),
    then spread Q. Otherwise the "Gaussian conserves Q" test is ill-defined.
11. **Codec and struct sizes.** `CellInfo` is 152 bytes today (`assert_eq!(size_of, 152)` in
    `cell_info.rs`; the doc comments still say 148). Adding f32 + f32 + u32 makes it **164**, not 160.
    Update the assert and both doc comments (`cell_info.rs` l. 14, `fields.rs` l. 4, `codec.rs`
    l. 5: 57 → 60). Compute the `GlobalStatus` totals in f64 from the unquantised activity, not
    by summing decoded cells.

## Nice to have

- The second invariant here includes ε̇₃₃: √(½(ε̇₁₁² + ε̇₂₂² + ε̇₃₃²) + ε̇₁₂²). It equals
  Savage–Simpson's max(|ε̇₁|, |ε̇₂|, |ε̇₁+ε̇₂|) for pure shear and uniaxial strain, and is √3/2 of
  it for equal biaxial strain. That is within 15 %, which is fine; just name it correctly in the
  notes.
- Arc placement: reuse the logic of `borders_reversed_segment` (it is short: a neighbour of the
  partner plate plus `segment_at(...).overriding == partner`) instead of "overriding plate
  only", so that the view matches the model at polarity flips.
- "Mw per century per cell" depends on the cell size (subdivisions). Label it "per cell" or use
  "per 100 km of boundary" for boundary cells.
- Volcano triangles are planet-space geometry, so check their size at the zoom limits and on
  phones. The legend's pale-yellow low end must stay distinguishable from the pale-grey
  "no activity" land.
- Hash check: `boundary_map`, `material_relative_velocity`, `strain_rate` and `plumes()` are all
  `&self` and pure, and `cell_infos` already calls `boundary_map()` once (l. 509). Pass that
  same map to `activity()` rather than computing it twice.

## Feasibility, codec and UI: OK

- **Data:** the boundary classes and `motion` (cm/yr, sign convention positive = converging)
  are in `display.rs`. The strain rate is computed per plate in sheet.rs (l. 1400–1430: the
  stencil uses only neighbours on the same plate, so there is **no double counting** of the
  jump across the boundary with the distributed strain). The plumes (`flux()` in m²/s,
  2–6 × 10⁻³) and `arc_profile` (normalised by total weight in `apply_additions_inner`, from
  the overriding contact cell) match the plan. The plume constants match #106's plan
  (Q ≈ 0.09 km³/yr for a typical plume under old ocean).
- **Codec:** a 1-byte `Decades` code is reasonable. Over 12 decades in 253 steps, half a step is
  2.4 % in log, i.e. ≤ 5.6 % relative, so the "≤ 6 %" test is right. One `Integer8` for the
  two 4-bit sources works. +3 bytes per cell, mostly zeros after the delta and lz4, and +8
  header bytes, are negligible. Bump `VERSION` 14 → 15.
- **UI:** `OptionKind::Choice` (a row of buttons) plus `Switch` fits the mode registry. Three
  short choices fit at 390 px with touch-sized buttons. Unlit is correct for a legend-matched
  scale. The overlay already has `triangle`, `thin` and `category_boundaries` for markers and
  coastlines, so nothing new is needed on the parameter panel or as separate mobile UI.
  Screenshots at 390×844 cover the phone case. Okabe–Ito for the sources is good.
