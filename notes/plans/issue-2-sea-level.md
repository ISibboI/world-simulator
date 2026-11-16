# Plan: issue 2 — sea level from a fixed ocean water volume

_2026-11-16. User: sea level next (relative-speed tuning of issue 1q saved for later)._

## Problem

Sea level is fixed at elevation 0, so the ocean's water volume follows the topography instead of
being conserved. Measured (seeds 0–1, 500 Myr, sea level 0): 1.80–1.84e9 km³ at t = 0 (mean
ocean depth 5.0–5.1 km; the initial ocean is old and deep), falling to 1.30–1.45e9 km³ as the
ocean gets younger [Earth 1.335e9 km³]. Holding the initial volume would raise sea level by
~1 km later and flood the continents; holding Earth's volume puts sea level ~1.8 km below the
current datum at t = 0, with land still 30% (steep initial continental edges), later land
26–40%.

## Physics

With Airy isostasy and water loading, a column with dry elevation `d` (elevation without water,
`Crust::dry_elevation`) under sea level `S` (relative to the fixed datum, as a dry-elevation
offset) has its surface at `z = (ρm d − ρw S) / (ρm − ρw)` and the surface relative to sea level
`z − S = (d − S) ρm / (ρm − ρw) = isostasy::to_surface(d − S)`. So a sea level is an offset of
the dry elevation; water loading is included exactly (a sea-level change of ΔS shifts sea-floor
depths by ΔS·ρm/(ρm−ρw) relative to sea level, i.e. the ~30% amplification the research noted
is handled by working in dry elevations). The water volume
`V(S) = A · ρm/(ρm−ρw) · Σ_cells max(0, S − d_i)` is continuous, piecewise linear and strictly
increasing in S (for S above the deepest cell), so it can be solved **exactly**: sort the
d_i, prefix sums, find the interval.

## Design

- `TectonicParams::ocean_volume` (m³, default Earth 1.335e18) and `Simulation::sea_level`
  (dry-elevation offset, m). `Simulation::surface_elevation(cell)` returns
  `to_surface(dry − sea_level)`; everything downstream (erosion base level, shelves, climate,
  temperature lapse rate, rendering, viewer, statistics) keeps treating 0 as sea level. Replace
  the direct `Crust::surface_elevation(time)` call in `statistics.rs` by the simulation method.
- `Simulation::update_sea_level()`: solve S for the volume over the visible surface crust of
  all cells (equal cell areas); called at the end of `Simulation::new`, at the end of
  `Simulation::step` (after tectonics) and at the end of `apply_surface_changes` (erosion and
  sediment change topography). Within a step, erosion sees the sea level of the tectonic state
  it erodes (consistent with its isostasy, which works in dry elevations relative to sea level).
- Erosion's isostasy helpers (`add_layer`, `layer_for_rise`) take elevations relative to sea
  level and convert with `to_dry`/`to_surface` — consistent as long as S is fixed during the
  erosion step.
- Diagnostics: sea level (as the change of the sea surface relative to the initial one, m),
  ocean volume, mean ocean depth, land fraction.
- Not in scope: ice-sheet water storage (glacio-eustasy; the temperature model has permanent
  ice but no ice volume), thermal expansion of sea water, water exchange with the mantle.

## Verification

- Unit tests: the solver returns the exact volume (random dry elevations), monotonic in the
  volume; lowering all crust by Δ (deepening basins) lowers sea level by Δ; a basin deepening
  under a fixed volume gives the Airy-loaded response; the land/ocean classification and
  `surface_elevation` are consistent with S; volume conserved over a run (statistics).
- Runs (seeds 0–2, 500 Myr): volume constant to round-off; sea level history (expect a drop of
  ~1.8 km dry-equivalent at t = 0 vs the datum, then a rise as the ocean gets younger),
  land fraction, mean land elevation, continental shelf flooding (continental crust below sea
  level, Earth ~25–30%), climate/erosion effects (sediment to ocean, precipitation); runtime.

## Plan review (independent agent, 2026-11-16): sound with changes — adopted

1. Corrected numbers: with Earth's volume and the old initial planet, sea level starts 0.90–0.99
   km (dry offset) below the datum, not ~1.8 km; mean land 0.8 → 1.7 km, no flooded shelves.
2. Earth's volume conflicts with the initial planet for structural reasons: continental fraction
   0.30 (Earth ~0.41 incl. shelves), no flooded margins, no sediment; old initial ocean ages add
   to it. Later in runs the volume matches Earth only because the ocean is too young (issue 1).
   **User decision: Earth's volume, with an Earth-like initial planet** — 40% continental
   crust with thinned, partly flooded margins and a triangular initial age–area distribution
   (mean ~60 Myr); review estimate: residual initial drop ~0.3 km. Sea level over time is a
   diagnostic of issue 1 until its tuning is done.
3. `Crust::surface_elevation` (sea-level free) is no longer public under that name.
4. Statistics: land, mean land elevation, min/max, deep counts relative to sea level; crust
   classification (`is_continental`, `is_oceanic`) never uses sea level.
5. Invariant: nothing in tectonics (ownership, polarity, buoyancy comparisons, events, dynamics)
   uses sea level.
Also: solver edge cases (volume 0 → min d; above all cells → extend linearly; ties), tests
(volume recomputed from surface elevations equals the target; uniform shift; edge cases), and
diagnostics (sea level vs datum, change per step split into tectonic/erosion parts, flooded
continental fraction, freeboard, volume from surface elevations); `ocean_volume` in `--set`.
