# Plan: subduction polarity per boundary segment (issue 1k/1l)

_2026-11-16. User decision: the thorough fix (polarity per boundary segment), before issue 2._

_Status: implemented (see `../sessions/2026-11-16.md`, session 3)._

## Problem

`Polarity` is stored per plate pair (`Simulation::polarities`). `choose_overriding`
(`boundaries.rs`) re-decides it at every overlap cell: where the lower plate shows continental
crust and the upper does not, the pair flips and `slab_length` is reset to 0. Pairs with
continental crust on both sides at different places flip back and forth within one pass (7,300–
9,300 flips per 500 Myr, ~2,700 live slabs destroyed). Slab pull, slab resistance and slab
length are per pair, so one continental contact anywhere resets the trench everywhere along the
pair's boundary; polarity reversals (issue 1 tuning) are undone within ~1 Myr. Real boundaries
between two plates often change polarity along strike (e.g. Pacific–Australia: Tonga–Kermadec
vs Puysegur–Hjort).

## Design

Split the pair state into pair-level and segment-level state:

```rust
struct PairState {               // per plate pair (a, b), a < b
    last_contact: f64,           // any contact of the pair (memory)
    collision_time: f64,         // suturing, as now (pair level)
    segments: Vec<Segment>,
}
struct Segment {
    anchor: DVec3,               // position in the local frame of the overriding plate
    overriding: u32,
    slab_length: f64,
    last_contact: f64,
    arrival_time: f64,           // sustained continental arrival at this segment (Myr)
}
```

- **Segments are local stretches of a pair's convergent boundary**, at most ~2R long
  (`segment_radius` R = 500 km, in km so resolution independent). The anchor is kept in the
  overriding plate's frame, so a segment moves with the overriding plate (trenches are roughly
  fixed to it); its world position is `rotation(overriding) · anchor`.
- **Assignment:** a cell of a pair belongs to the pair's segment whose world position is nearest,
  if within R; otherwise a new segment is created there (polarity by buoyancy at the cell, as
  first contact now; slab 0). Lookup is a linear scan over the pair's segments (few per pair).
- **Tracking:** after each `resolve_ownership` pass, each segment's anchor is re-centred to the
  centroid of the overlap cells assigned to it in that pass (if any) and `last_contact` set;
  segments of the same pair closer than R/2 merge (keep the polarity/slab of the one with the
  longer slab, sum the contact information). Segments without contact for `polarity_memory` and
  without slab are removed; pairs without segments and without contact are removed as now.
- **Per-cell winner vs segment polarity:** `choose_overriding` no longer mutates polarity. At an
  overlap cell, the winner is the overriding plate of the cell's segment, **except** where the
  lower plate's crust is continental and the upper's is not: then the continental crust stays on
  top at that cell (buoyant crust does not subduct; terrane accretion), without changing the
  segment. Such cells increment a per-segment counter in the pass.
- **Sustained flip:** after the pass, a segment whose overlap cells are mostly continental
  arrivals accumulates `arrival_time += dt`, otherwise it decays (`max(0, t − dt)`); when
  `arrival_time ≥ flip_time` (3 Myr) the segment flips: overriding = the other plate, slab = 0
  (the old slab breaks off after collision), `arrival_time` = 0; counted (`polarity_flips`).
  Three-plate cells: pairwise segment lookups; the plate overriding all others wins, else
  buoyancy, as now.
- **Classification and forces:** `boundary_cells` uses the segment at the cell (nearest within
  R, no creation) for `subducts_here`; `boundary_force` and `slab_resistance` use that segment's
  slab length. Store the segment index in `BoundaryCell` to avoid repeated lookups.
- **Slab lengths:** `update_slab_lengths` per segment: mean convergence of the trench cells
  assigned to it; decay and break-off (< `min_slab_length`) per segment as now.
- **Events:**
  - passive-margin initiation: after the split, create a segment at the margin cell with
    overriding = continental part and `initiation_slab_length`; the rest of the new boundary
    gets polarities by buoyancy on first contact (today the whole great circle gets the slab);
  - induced subduction / reversal: candidates per boundary cell use the cell's segment (skip if
    the plate already subducts there); the event sets only that segment (create one if none);
    skip candidates where the would-be subducting plate has continental crust at the cell;
  - suturing: unchanged, pair level.
- **Topology:**
  - split of P into P and N: each segment of a pair (P, X) goes to (P, X) or (N, X) by the side
    of the split plane its world position lies on (anchors in P's frame stay valid for N, which
    starts with P's rotation); `collision_time` reset as now;
  - merge of B into A: segments of (B, X) move to (A, X) with anchors converted into A's frame
    when B was overriding, then the merge rule applies; pair (A, B) disappears;
  - removal / renumbering: as now, on the pair keys and `overriding`.
- **Initial conditions:** initial subduction zones become one segment per R along the initial
  boundaries (created by the first pass with the predefined overriding plate and 600 km slab):
  generation stores the intended polarity per pair, and the first pass seeds segments from it.

## Small items from the evaluation (1m), done alongside

- Hazards include the 1.1 zig-zag factor (shared constant with statistics).
- Failed passive-margin splits are counted (`failed_initiations`).
- `plate_events` doc: boundaries may be stale for slabs that broke off this step (or recompute
  when `slabs_changed`).
- Kinematic production averaged over the steps between snapshots, like created production.

## Verification

- Unit tests: polarity does not flip within one pass with continental crust on both sides at
  different places; two distant segments of one pair can have opposite polarity and independent
  slab lengths; a brief continental arrival (< 3 Myr) does not flip, a sustained one does and
  resets that segment's slab only; continental crust arriving at a trench is not subducted;
  split/merge/renumber carry segments to the right pairs; incremental vs full matching stay
  identical (helper compares segments); determinism.
- Runs (seeds 0–2, 500 Myr, compared with the current logs): flips per Myr (expect ≪ 15),
  slab length distribution, mean speed (expected to rise towards 4–6 cm/yr), reversals that
  survive > 10 Myr, mean/median age, ancient fraction, plates, production, continental mass;
  runtime.

## Risks

- Segment churn at ragged boundaries (many tiny segments): bounded by R and the merge rule; log
  segment counts.
- Behaviour changes broadly (slab pull now accumulates) → re-check slab resistance/drag
  calibration; do not retune unless targets are clearly missed.

## Plan review (independent agent, 2026-11-16): sound with changes — all adopted

1. Directional flip with hysteresis: count arrivals (lower continental, upper not) and reverse
   arrivals (upper continental, lower not) per segment and pass; accumulate `arrival_time` only
   if arrivals > 0.5 of the segment's overlap cells and exceed reverse arrivals clearly.
2. Convert the anchor to the new overriding plate's frame whenever `overriding` changes (flip,
   induced reversal, merges).
3. Seed initial subduction zones in `Simulation::new` over the boundary cells (600 km slabs
   before the first velocity solve).
4. Passive-margin initiation seeds segments along all cells of the new boundary where old ocean
   faces the continental part (one per R); log the initiation success rate (slab ≥ 300 km).
5. New segments inherit the polarity (not the slab) of the nearest segment of the same pair
   within 2R unless the cell is an arrival; buoyancy only without neighbour.
6. Never drop pairs/segments with a slab; merge rule takes the max of `arrival_time` and
   `last_contact`; merge only segments of equal polarity (or where one has no slab).
7. Stable segment ids (pair + id); all segment mutations outside the pass bump
   `configuration_changes`; `update_slab_lengths` never removes segments.
8. `R = max(500 km, 3 × mean edge length)`; test bounded segment counts at s4.
9. The per-cell continental override enters the pairwise relation before the three-plate rule;
   `SubductedContinental` should drop to ~0.
Also: re-centre once per step from the overlap cells of all its passes, damped; segments within
R of a split plane are copied to both new pairs; cache world positions per pass; diagnostics
(segments, creations/merges/expiries, flips, override cells, initiation success).
