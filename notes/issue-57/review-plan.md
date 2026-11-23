# Review of notes/issue-57/plan.md (#57: duplicated samples get the sum of their cells' surface changes)

Reviewer: independent subagent, 2026-11-21. Code read at worktree HEAD 67ca0a2 (plus the uncommitted
`diag57` test in src/tectonics/mod.rs). Nothing was built or run: a `issue-57: diag` job was
already holding 4 cores, so I did not run the diagnostic a second time.

Overall: the diagnosis is right and the direction is sound. The sum is mass-conserving, but it
over-erodes (or over-fills) the shown column and leaves its hidden twin frozen. Mass that has no
world area of its own has to go to samples with no world area, so sending it to hidden partners
is essentially forced. One real conservation bug is in the plan (F1), plus several smaller points.

## Findings

**F1 — High: an equal share of a *thickness* is not an equal share of *mass*.**
`bedrock_delta[cell]` is metres of the shown sample's bedrock. Erosion turns it into sediment
with `bedrock_density[cell]`, which is `surface_crust(cell).density` (erosion.rs ~548,
mod.rs 1176). `apply_surface_changes` applies thickness and books `crust.mass()`. A partner with a
different density (arc vs. continental vs. oceanic, metasediment-rich columns, which are common at
margins and sutures) then gains or loses mass that erosion never produced. The ledger still closes,
because it records the actual change. But the planned test "Σ change = Σ cells (bedrock·ρ +
sediment·ρ_sed)" fails, and the felsic budget and the sediment-from-erosion balance drift.
*Recommendation:* sum the group's **mass** change: bedrock delta × the shown sample's density
(identical for all k cells, since they show the same sample), plus sediment as now. Divide that
mass by m. Turn each member's bedrock share back into metres with that member's own density.
Sediment shares need no conversion. The existing "removal beyond the available sediment from
bedrock at the same mass" step then works per member, as it does now.

**F2 — Medium: duplicates have two origins, and the plan names only one.**
Besides nearest-vertex aliasing in `match_plates`, `resolve_ownership` deliberately makes cells
show an adjacent sample:
- single-cell holes over a `consumed` vertex (`shared_cells`, boundaries.rs ~528);
- slivers at boundaries over consumed vertices (~643);
- `heal_gap` edge gaps that return `parent` (~1280).

In these cases the local vertex under the extra cell has `None` crust, so the "natural" partner
does not exist. The ring search then claims some other hidden sample nearby, often the true
aliasing partner of a different duplicated group. That causes shortfall elsewhere, depending on
sample-index order. This is not unsafe: `consumed` implies `crust == None`, so consumed or empty
vertices can never be claimed, and `None` cannot be hit. But the claim "typically a ring-1/2
neighbour lying under one of the k cells" is unproven for boundary cells.
*Recommendation:* in the diagnostic and in the run statistics, split duplicated samples into
gap-shared (the local vertex at the extra cell has no crust) and aliased, and report the
shortfall for each. Accept a shortfall at gap-shared samples. Shortfall at aliased samples
measures how good the rule is.

**F3 — Medium: claiming greedily in sample-index order is deterministic but order-biased.**
A ring-2 claim for group A can take the ring-1 partner of group B, which comes later.
*Recommendation:* either do two passes (ring 1 for all groups, then ring 2 for the groups still
short), or use the simpler geometric rule in F4.

**F4 — Medium (alternative worth preferring or comparing): attach hidden samples to the cell they
lie under.**
`update_crust_cells` already stores, for every hidden sample, its nearest world cell
(`plate.tracked.samples`, computed with the current rotation at the end of `resolve_ownership`).
Rule B: group G(s) = {s} ∪ {hidden samples h of s's plate whose nearest cell c has
`owners[c] == s`}, with share = Σ mass / |G|. This uses no ring search, no claim order and no
extra `nearest_from` calls. "Owned by the same plate" is then implied (`owners[c] == s`), and
"lies under one of the k cells" becomes exact instead of assumed. If |G| ≠ k, mass is still
conserved. It is cheap enough to prototype both rules in the diagnostic and compare their
shortfall before choosing. If you keep the ring rule, still require that the partner's nearest
cell is one of the group's cells or a neighbour of one, not just any cell of the plate.

**F5 — Low: the same-plate test is correct, but it mostly decides nothing.**
A hidden sample (one no cell matched) is never a candidate in `resolve_ownership`, so it is
never consumed. But a sample that sits under another plate's cell is normally *matched* by that
cell (nearest vertex), loses there and is consumed. So hidden samples under another plate are
rare. They occur only when aliasing coincides with a boundary. The test is still cheap and
correct; keep it. F4 makes it implicit.

**F6 — Low: partners of a different kind.**
At coastlines and margins, a land group's erosion can be applied to a hidden partner that is a
submerged oceanic or thinned column. This is mass-conserving, but it thins oceanic bedrock with
land incision. *Recommendation:* skip partners whose `is_continental_origin_column()` differs
from s's (they count as shortfall), or at least count them in the statistics.

**F7 — Low: a benefit to state, and a remaining limit.**
Today the hidden twin misses all erosion and deposition. When the duplicate pattern shifts, the
pair reappears as a 2×-changed pit or spike next to an unchanged column. The rule also fixes
these single-cell **peaks**, not just pits, so measure single-cell peaks too. Unclaimed hidden
samples still freeze. Report the share of hidden land samples that go unclaimed for a long time
(e.g. the age since they were last changed), so the residual is known.

**F8 — Bookkeeping: otherwise fine.**
- Ledger term unchanged: correct (with F1, the per-member mass sum equals the cell mass sum).
- Material budget is recomputed around the call: fine.
- Isostasy is per column: fine.
- Cohort: deposits go to cohort 0 and erosion keeps the fraction: unchanged and fine.
- `cover_turnover` in planet.rs counts per-cell deltas: unaffected.
- The sea-level solve and `sediment_sea_level_of` work on cells. With m = k the summed change on
  the shown cells equals Σ cell deltas exactly, so `sharing` should fall to the nonlinear residual
  and the shortfall.
- Partners' changes appear later as resampling effects, not as `sharing`. Say so in the results.
- `thin_from_top` clamping at 0 thickness is a pre-existing non-conservation; splitting makes it
  rarer. It is also possible on thin partners, so count clamped mass.

**F9 — Medium: audit of other write paths.**
- `apply_additions`: the claim is correct. `first_seen` is computed only `if band`, and the
  weights fall back to `reached...weight` (one per cell) when `!use_band`. So arcs, accretion,
  shortening and collisions without a band (or whose band weights are all 0) give a duplicated
  sample one profile weight per cell. Mass is still normalised. The fix is right: compute
  `first_seen` for every addition and zero the duplicate weights in the profile path too (the
  spill loop may still visit duplicates; that is harmless).
  *Recommendation:* fix it here, but in its **own commit**. Make the seeds-0–3 run "surface rule
  only" **mandatory, not conditional**: the effect on arc thickness and tectonic metrics must be
  attributable, and #57's acceptance should not depend on it.
- Crustal flow: listed-only is reasonable. The per-sample outflow and inflow caps make it
  self-limiting, and fixing it means recalibrating `collapse_diffusivity`.
- **Missed path, Low: subduction erosion.** `eroded.push(winner)` happens once per consumed loser
  sample (boundaries.rs ~473). A winner sample shown at k trench cells collects about k contacts
  and is thinned by `subduction_erosion` about k times, while holding the mass of one cell. It is
  the same bug class, small and only at trenches. List it in the PR (de-duplicating
  `eroded` per step would be the fix, but that also halves erosion rates there, so it needs a
  calibration look).
- Checked and fine: plume reheating (`max`, idempotent), initial sediment (in `new()`, before
  any resampling), metamorphism and healing (per sample), the dynamics/topology/statistics uses
  of `owners` (read-only or plate-level).

**F10 — Low: measurement and process.**
- Add a **go/no-go before the 8-seed sweep**: 2 seeds × ~200 Myr, reporting the shortfall per
  F2 class, the clamped mass and the per-step runtime. Example target: aliased shortfall < 10% of
  the extra cells.
- Add single-cell peaks (F7) and the ledger/felsic residual with F1 in place.
- State the pass criterion for the pit share (e.g. the share on duplicated samples within a few
  points of the land share of duplicated cells, Welch-significant drop).
- Remove the uncommitted `mod diag57` from src/tectonics/mod.rs before the PR, or move it
  behind `#[ignore]` with tidy formatting if kept (it is not rustfmt-clean).

**F11 — Info: longer-term alternative.**
A one-to-one resampling (a cell whose nearest vertex is already taken shows the nearest free
hidden vertex of the same plate) would remove aliasing duplicates at the source. That would fix
erosion, additions, flow and subduction erosion together. It touches matching and consumption,
so it is a follow-up and not for this issue; worth one line in the PR.

## Tests (additions to the plan)
- A group whose partner has a different density: mass change = Σ cells (bedrock·ρ_shown +
  sediment·ρ_sed) (F1).
- A gap-shared duplicate (local vertex under the extra cell is `None`/consumed): nothing is
  written to that vertex, and it remains `None`.
- Determinism with two duplicated samples competing for one hidden sample.

## Second look (revised rule, plan sections "Diagnostic before coding" / "Rule (revised …)")

**S1 — Info: the "regional density mismatch" explanation holds up.**
`SphereMesh::icosphere` is plain recursive midpoint subdivision with normalisation, and there is
no relaxation step. I recomputed it in Python (level 6, 40 962 vertices, area per vertex = ⅓ of
the adjacent triangles):
- relative cell area runs from 0.77 to 1.21 (p5 0.93, median 0.95, p95 1.19);
- the mean |area − 1| is 7%;
- the variation is smooth and regional: large cells sit at face centres, small cells near the 12
  icosahedron vertices.

A plate's fixed local mesh rotated over the world mesh therefore gets samples per cell ratios of
roughly ±20% in patches the size of a face. It starts at 0 (the identity at t = 0) and grows as
the plates rotate. All of that matches the diagnostic: ~5% duplicates after 60 Myr, ~93% of them
true nearest-vertex duplicates, and partners mostly far away. Expect the duplicate fraction to
level off at a few to ~10% of cells, not keep growing. Report it over 1000 Myr to confirm.

**S2 — Medium: the "thin share" claim is wrong, and the plate-wide equal spread is non-local in a way that matters.**
Hidden samples (~1840) are about as many as the extra cells (~2240). So each hidden sample's share
is about the change of one typical duplicated cell **per step**, not a thin fraction. A hidden
region persists for as long as the plate's orientation keeps it over larger world cells, which can
be tens of Myr. Over that time its samples take on the plate-mean (per class) change, whatever
their own setting.

Example: a hidden continental-shelf or lowland sample receives the mean land bedrock incision,
with no deposits of its own. At, say, 20–50 m/Myr over 30 Myr it resurfaces up to ~1 km too low.
That is a new resurfacing pit, the very artefact #57 is removing. In the other direction, a hidden
basin can be fed the plate's net deposition.

This is mass-correct, but it puts mass where it is not plausible.
*Recommendation:* keep the pools (mass, per plate and class) but weight the recipients locally.
Each hidden sample h lies under a world cell c_h. Its nearest cell is already in
`plate.tracked.samples` for hidden samples, computed at the end of `resolve_ownership` with the
current rotation, so no new search is needed. Use a fallback search only where `tracked` is None
(new, split or merged plates).
- Split each pool by sign: the bedrock-loss residual is spread over hidden samples in proportion
  to the erosion mass at c_h; the deposition residual in proportion to the deposition at c_h; the
  sediment-loss residual likewise.
- Positive weights normalise well. When a weight sum is 0, fall back to equal shares.

Hidden samples then carry roughly what their own location experienced, scaled to close the books.
If this is judged too complex, at least restrict recipients to the same plate, same class **and**
same side of sea level (c_h land vs. sea) as the change. Also report the cumulative change carried
by hidden samples at resurfacing (e.g. the |Δelevation| of a sample from hidden to shown against
its neighbours), so a new pit source would be visible.

**S3 — Low: netting inside a pool.**
Summing the residuals per plate and class nets the erosion of one region against the deposition
of another before spreading. That conserves mass, but if bedrock and sediment are pooled
separately and by sign (S2) nothing is lost by netting. If they are netted, at least keep bedrock
and sediment pools separate, so that eroded felsic bedrock becomes sediment rather than cancelling
deposits of another origin.

**S4 — Low: the other-class fallback moves felsic mass wrongly.**
Thinning oceanic (mafic) bedrock to absorb a felsic erosion residual changes the felsic +
sediment budget even though total mass closes. Prefer "no same-class recipient → the residual
stays on the group (the old sum)". It should be rare: continental plates nearly always have hidden
continental samples. Use the other class only if the measurement shows the first fallback is
frequent. Log both counts.

**S5 — Low: pitfalls to cover in tests and counters.**
- Thin recipients (new sea floor, rift-stretched crust) clamp at 0. With S2 weighting this
  mostly goes away; otherwise count the clamped mass and re-spread it in a second pass.
- Determinism: accumulate the pools in a fixed plate/sample order (no HashMap iteration).
- Plates without any hidden sample are a common case for small plates; the per-group sum then
  stays on them. Report how often.
- Changes on hidden samples resurface as resampling effects in the sea level, not as `sharing`.
  Say so in the results.

**S6 — Info: the rest is sound.**
Mass is conserved exactly when bedrock mass is converted with the shown sample's density and
reconverted with each recipient's density (F1 done). The mean on the shown sample fixes the
visible k× artefact. Rejecting area-weighted books and one-to-one resampling as follow-ups is
reasonable for this issue; the area range above (0.77–1.21) is worth quoting in the follow-up
issue.
