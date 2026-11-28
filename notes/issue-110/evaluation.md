# Independent evaluation of the results of #110 (tectonic activity estimate and render mode)

Reviewed: notes/issue-110/plan.md, results.md, the six PNGs, src/tectonics/activity.rs,
src/viewer/modes/tectonic_activity.rs, and the diffs (against ae5f2c0) of cell_info.rs, global.rs,
history/codec.rs and tectonics/display.rs. I did not run builds or simulations.

## Verdict

Sound and ready for a PR once the "should" items are done. I found no must-fix bugs. The
formulas, units, signs and per-cell scaling are right. The totals agree with the model's own
production and consumption of sea floor. The map pattern matches what is expected: the
strongest seismicity is at the trenches, there are thin lines of weak seismicity along the
ridges, ridges carry the most magma, arcs sit behind the trenches, and hotspots show as round
spots. Two problems remain. First, the results explain the model's 2.8× Earth moment rate the
wrong way. Second, they say too little about how strongly distributed deformation colours the
continents at web resolution.

## Checks of the numbers

- **Per-cell scaling is right.** `boundary_cells` lists one row of cells on each side of a
  boundary. Each cell takes ½ · (Ṁ/L) · s / A, so a straight boundary gets Ṁ/L · s per spacing s.
  The zig-zag excess is at most 2/√3 ≈ 1.15, as stated. The results confirm this
  independently: the ridge magma estimate (47.5 km³/yr) matches the ledger's sea-floor
  production (53 km³/yr).
- **Units are right.**
  - Moment rate: N·m yr⁻¹ m⁻¹ · m / km² gives N·m yr⁻¹ km⁻². For distributed deformation, ε̇ is
    converted to yr⁻¹ and the result multiplied by 10⁶ to go from per m² to per km².
  - Magma: m³ yr⁻¹ / m² · 10³ gives mm/yr. The plume constant 1e11 m³ yr⁻¹ per m² s⁻¹ equals
    10⁸ km³ Myr⁻¹ per m² s⁻¹.
  - Global totals: Σ · A_km² for the moment, and mm/yr · km² · 10⁻⁶ for km³/yr.
  - Hand checks agree: 6 cm/yr subduction gives 2.26e14 per m. A 5 cm/yr ridge gives
    1.55 mm/yr and 1.7e12 N·m yr⁻¹ km⁻². Mw 8.0 for 1.4e15 is right, and so is the test value
    of Mw 7.5 for 2e14.
- **Signs are right.** Convergence is positive towards the other plate (`relative · normal`,
  material velocities). Moment uses |v|. Magma uses max(−v_n, 0). Convergent classes that open
  slightly get nothing.
- **No double counting.** The boundary motion uses the *material* velocities, so any
  convergence a deforming band takes up is missing from the contact. The sheet's in-plate
  strain and the boundary term therefore split the convergence between them rather than
  counting it twice, as long as the band's coupling equals the boundary's, which it does by
  construction.
- **Arc spreading matches the model.** The volume is v · `arc_crust_per_subduction` · s per
  Overriding cell, which is the same as the model's 500 m × one cell area per subducted sample
  (h · v · L). The BFS is a per-seed ring walk, like `apply_additions_inner`. Like the model, it
  puts part of the ring-1 weight on the neighbouring trench-line cells along strike (ring 1
  counts hops, not distance from the trench). The view is therefore faithful to the model, and
  it is the model that puts about a third of its arc crust on the trench line. This is outside
  this PR, but worth noting.
- **Plume footprint is normalised correctly.** The weights are normalised over the cells
  inside the cutoff, so Q is conserved. Even at 4 subdivisions the nearest cell (≲ 260 km) is
  inside the 400 km cutoff. The lid is taken from the nearest cell, using `is_continental_origin`
  (lithosphere) rather than `_column`, which is the right choice for a lid.
- **Codec is right.** `Decades` maps 0 to 0, 1–254 on a log scale, and 255 to NaN. Its
  half-step error is 0.024 and 0.016 decades. Values above the top are clamped. Channel indices
  38–40 match, and the low byte of the source byte is kept.

## Is the comparison with Earth fair?

Mostly yes. Model magma is 52 km³/yr against Earth's 26–34 (Crisp). Arcs are 3.3 against 3–8,
hotspots 1.3 against 1–2.5, and the subduction share of moment is 90 %, as on Earth.

The **explanation of the 2.8× moment rate is wrong**. The results put it down to "mostly at
faster trenches". In this model the subduction moment is μ·cz/(sin δ cos δ) × (area subducted
per year) = 3.76e15 N/m × v·L. So 2.6e22 means v·L ≈ 6.9 km²/yr of sea floor consumed. That
equals the ridge production (47.5 / 7 km ≈ 6.8 km²/yr) and is about twice Earth's ~3.3 km²/yr.
The panel shows a mean plate speed of 3.4 cm/yr, which is Earth-like, so the cause is more
boundary length or opening, not faster trenches.

The same formula with Earth's ~3.3 km²/yr gives about 1.2e22, already at the top of the
observed (0.7–1)e22. The rest of the ratio is therefore B&K's long-term "tectonic" moment
against a ~century-long catalogue. That makes the factor about 2 from the model's tectonics and
about 1.2–1.4 from the method.

## Does the map match expectations?

- **Earthquakes (desktop, 5 subdivisions):** yes. The trenches form a dark-red ring. Collision
  and deforming belts are red. Ridges are thin orange lines and transforms are orange. Plate
  interiors are grey.
- **Volcanism:** yes. Ridges are darkest. Arcs are paler bands behind the trenches, consistent
  with about 0.15 against 1.5 mm/yr. Hotspots appear as spots.
- **Both:** the arc markers sit on the concave (overriding) side of the trenches, the hotspot
  markers are in the ocean, and the rift markers are in continental interiors.
- **Phone (4 subdivisions):** most of the continental area is orange to red
  (10¹²–10¹³, Tibet-like rates) from #182's sheet strain of 10⁻¹⁶–10⁻¹⁵ s⁻¹. This is far from
  Earth's quiet cratons. The caveat mentions it, but the results also call the pattern "Earth's"
  without qualifying it.

## Must-fix

None.

## Should

1. **Fix the 2.8× explanation in results.md and the PR.** It comes from about 2× Earth's sea
   floor consumed (about 6.9 km²/yr, matching ridge production), not from faster trenches.
   Better still, cross-check the subduction moment against the ledger's subducted
   oceanic area, as was done for the ridges. Also mention that the method alone gives about
   1.2e22 for Earth's rates.
2. **State the continental-interior seismicity plainly in the PR summary**, not only as a
   caveat: at web resolution most of the continental area shows Tibet-level rates.
   - This is a property of #182's sheet (strain rates 10–1000× those of stable interiors), and
     the view reproduces it faithfully.
   - Qualify "the pattern is Earth's" for 4–5 subdivisions.
   - Consider suggesting a follow-up issue on the sheet's background strain. Raising
     `MIN_STRAIN_RATE` would only hide the problem; I would not tune it.
3. **The `desktop-earthquakes.png` screenshot is out of date.** It shows the old single row
   "deformation, 4.0·10¹¹ …, ≈ Mw 5.7 per century". The code now has a separate "Century quake"
   row with "per 100 × 100 km". Retake the screenshot or say that it predates the change.
4. **Touch sizes are not shown.** The phone screenshots look like a fine-pointer Chromium: the
   switcher buttons are about 22 px tall. Either take one shot with touch emulation (coarse
   pointer) to confirm the ~44–48 px targets of the shared switcher, or say that the mode adds
   no new kind of widget.

## Minor

- **Add a caveat that the century quake depends on resolution.** The density is spread over
  2 cells across a boundary, so at 4 subdivisions a boundary's Mw per 100 × 100 km comes out
  about 0.4 lower than at 6 subdivisions.
- **Reword "Century quake".** It may read as a forecast of an actual event. Something like
  "Century's moment as one quake" would be clearer, or explain it in the row's hover if rows
  have one.
- **Hotspots on ridges get no marker in "Both".** A cell where the ridge dominates (an
  Iceland-like hotspot) is hidden. Consider using the largest non-ridge source for the markers.
- **Oblique trenches use transform couplings for their strike-slip part.** On oceanic plates
  below 4 cm/yr this means 13 km. It is small, but B&K treat oblique SUB slip within SUB.
  Mention it or leave it.
- **The scale is continuous; the plan said classes per decade.** Either is fine, but align the
  plan with what was built, and likewise the magma palette (pink/magenta, not orange-red as
  planned).
- **Arc markers are near-black on the dark-purple trench colour.** The white border keeps them
  readable on desktop. On the phone they are small but visible.
- **The legend's description text is small on the phone** (shared legend style, so not specific
  to this mode).
- **Inconsistent "continental" test.** The lid uses `is_continental_origin` while the
  couplings use `_column`. This is defensible and needs no change, but a comment on why would
  help.

## Caveats: complete?

Most of the caveats are there:
- grid smearing and zig-zag
- triple junctions
- the OTF step
- distributed deformation
- no intraslab earthquakes, no back-arc, no diffuse arcs
- plumes and #106
- intrusions

Add three:
1. The model's sea-floor turnover is about 2× Earth's, and the global moment and magma follow
   from it (should 1).
2. Part of the arc crust lands on the trench line itself, as the model grows it.
3. The century quake depends on resolution.
