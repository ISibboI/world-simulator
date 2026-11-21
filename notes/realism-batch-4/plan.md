# Batch 4 plan: suture marks mixed by mass (#40); where plume rifts go (#52); #53, #51

_2026-11-21. Triage: [`../realism-triage/overview.md`](../realism-triage/overview.md) (batch 4).
`main` at 4bcd243 (after #66)._

## #40: suture mark mixing

### What is wrong

Lithosphere deformed in an orogeny (collision belts, sutures, accreted terranes) is marked on
its crust sample (`Crust::orogeny`, the time of the orogeny) and is half as strong as unmarked
lithosphere when plates break (`weak_zones::lithosphere_strength`). Samples are merged and
split in three places: when plates merge (`topology::add_crystalline`), when a rift stretches
crust into a new cell (`boundaries::stretch_rift`) and when an enclosed gap is healed
(`boundaries::heal_gap`). There the mixture is marked if **any** contributor is marked (the
"max rule"). So a marked sliver marks a whole cell, and each mixing can move the mark one cell
further. Triage (8 seeds): the marked share of the continental cells grows ~4 points per
100 Myr and reaches 44% at 1000 Myr.

One cell marked instead of a fraction of it is a grid-scale inaccuracy, which the project
accepts (#64). The spreading from cell to cell over many mixings is not: it grows with time
and does not conserve the amount of weakened lithosphere.

### Change

A second quantity on the crust sample, `Crust::orogenic`: the mass fraction of the sample that
stands on orogenic lithosphere.
- Set to 1 where an orogeny marks the sample (collision stacking above 35 km, sutures,
  terrane accretion: the places that set `orogeny` today).
- Mixed by mass in the three places above. A new cell stretched out of several donors gets the
  mass-weighted mean of their fractions; the donors keep theirs.
- Cleared, like the mark, when the mixture is no longer continental lithosphere (#31).
- Strength: `1 − 0.5 × orogenic` instead of `0.5 if marked`.
- `orogeny` (the time of the last orogeny that touched any part of the sample) stays, with the
  max rule, as a diagnostic and for the cell dump. The share of rift boundaries on belts
  counts cells with `orogenic ≥ 0.5`.
- Cell dump: new column `orogenic`.

This is option 2 of the issue. Option 1 (majority rule, no new field) keeps a binary mark and
so still gains or loses weakened lithosphere at every mixing; option 2 conserves it where
samples are mixed with each other (crust added on top of a sample, e.g. arc magma or sediment
turned to rock, leaves the share of the lithosphere below unchanged, so the product of share
and crust mass is not a conserved quantity in general). Cost: one
`f64` per crust sample.

Not changed: additions on top of a sample (arc crust, metamorphosed sediment, stacked collision
crust) do not dilute the fraction. The weakness is in the lithosphere (mantle fabric, faults),
not in the crust laid on it.

### Measurement of the over-spreading

In the branch runs both quantities exist side by side: the marked share (max rule) and the
mass-weighted fraction. Their ratio over time is the over-spreading of the old rule (in a
statistically equal run, not the identical one).

## #52: about 30% of plume rifts end more than 500 km from their plume

Investigation first. The branch logs, at debug level, for every plume rift: the distance from
the plume to the new boundary (already there), to the nearest other plate before the split, to
the nearest continental crust, whether the plume stands under crust that is not cut, and the
plate's area. Run on 8 seeds; classify the far rifts by these. A fix follows only if a cause
turns out to be a defect and not geometry the model intends (for example a plume at the very
edge of its plate, where a rift next to the old boundary is right).

## #53 and #51: not in this batch

- #53 (the plume-head weakening is a field in the mantle frame, without memory). Carrying it
  with the plate needs a thermal state on the crust sample that is separate from `birth`:
  `birth` also sets the ocean depth, the sea-floor age statistics, slab pull and the initiation
  hazards, and the plume swell is already in the dynamic topography (#34), so a reset of the
  thermal age would count the swell twice. PR #50's ablation found the head weakening changes
  the rift geometry little. Recommendation: keep as a low-priority enhancement.
- #51 (cut cost weighted by the plume's stress) depends on what #52 finds.

## Overlap with work in flight

PR #68 (#43) and the #42 branch change `boundary_cells`, `update_slab_lengths`, `events.rs`
(sutures of stalled convergence) and `polarity.rs`. This batch touches `crust.rs`,
`weak_zones.rs`, `topology::add_crystalline`, `boundaries::stretch_rift` / `heal_gap` /
the collision mark, and in `events.rs` only `mark_suture` (one line), the rift diagnostics and
one statistic. Every new `Crust { … }` literal needs the new field, so whichever lands second
must add `orogenic` to literals the other introduced.

## Tests

- Mixing a marked and an unmarked sample gives the mass-weighted fraction; the strength lies
  between the two.
- Stretching a rift out of a marked and an unmarked donor: the new cell has the weighted
  fraction, the donors keep theirs.
- A mixture that is no longer continental has fraction 0 (#31's rule).
- Existing weak-zone and plume-rift tests pass (rifts still follow sutures).

## Validation (8 seeds × 1000 Myr, `main` vs branch, 600–1000 Myr, Welch t)

- Marked share of continental cells (old rule) against mean fraction and share with
  fraction ≥ 0.5, over time.
- Rift boundary cells on belts against all continental cells of the rifted plates (rifts must
  still prefer belts: `main` ~52% against ~26%).
- Guard rails: plates, trench / ridge / collision length, rifts, initiations and sutures per
  100 Myr, mean sea-floor age, sea floor ≥ 200 Myr, production, continental area, flooded
  continental crust, mean land, runtime and memory.
