# Code review: breakup of plates by internal stress (#123)

Independent review of `agent/issue-123` against `origin/main` (`git diff origin/main -- src`),
up to commit ed8a585 (sections analysed every `breakup.interval` = 2 Myr). Files:
`src/tectonics/breakup.rs` (new), `sheet.rs`, `topology.rs`, `events.rs`, `weak_zones.rs`,
`mod.rs`, `main.rs`. Line numbers refer to ed8a585.

Checks run: `cargo fmt --check` (clean), `cargo clippy --all-targets` with
`CARGO_BUILD_WARNINGS=deny` (**fails**, finding 1), `cargo test --release breakup` (5 passed).
The full test suite and the wasm32 check were not run in this review.

## Findings, by severity

### 1. CI blocker: clippy `neg_cmp_op_on_partial_ord` (breakup.rs:357)

`if !(d < best) { break; }` in `limit_overload` triggers
`clippy::neg_cmp_op_on_partial_ord`; with warnings denied, the clippy job (and `cargo clippy
--all-targets`) fails. The negation is intentional (it also stops on NaN).
**Fix:** `if d.partial_cmp(&best) != Some(std::cmp::Ordering::Less) { break; }` (same
semantics incl. NaN), or `if d.is_nan() || d >= best`.

### 2. Medium: the quick sweep and the warm start drop half of the mechanisms (breakup.rs:~273–281, ~322–330)

`quick_overload` normalises each candidate mechanism to the plane `ω · T = 1` by
`candidate / along` but only `if along > 0.0`. Dividing by a negative `along` also gives
`ω · T = 1` (the mechanism turned the other way), which is just as admissible. So the
bending candidate (`middle`, rotation about the section's middle) is discarded whenever
`middle · T < 0`, i.e. for about half of all sections, and the warm mechanism of the previous
level likewise. The same filter is on the warm start in `limit_overload`.
Why it matters: `quick_overload` decides which `REFINED = 4` levels per field get the full
analysis. Bending is exactly what the revision after the plan review added (necks, arms,
three-point bending); for the sections where `middle · T < 0` the quick estimate falls back to
the net-force mechanism, which underestimates bending-dominated levels by up to the
"~5–20×" quoted in the plan, so the most overloaded level can be missed (the result is still
a lower bound, so the error is towards too few breaks, but in a sign-dependent, i.e. somewhat
arbitrary, way). The full analysis of the selected levels is not affected (IRLS reaches the
other sign on its own).
**Fix:** use `along != 0.0` (or `along.abs() > tiny * |candidate| |T|`) in both places; the
mechanism `candidate / along` is then valid for either sign. A unit test: a pure bending
moment `T = -X * plastic` on `straight_section()` should give `quick_overload ≈ 1` (today it
gives the net-force estimate).

### 3. Medium (diagnostics only): stale rows in `plate_sections.csv` with `interval > dt` (main.rs, `append_plate_sections`; breakup.rs `analyse_sections`)

Since ed8a585 the sections are analysed only every `interval` (2 Myr), but `main.rs` still
appends `section_records()` after **every** step with the current time. `section_records` is
only replaced on analysis steps, so every analysis appears twice (default `dt` = 1), the second
time with a time stamp it was not computed at and the old `timer`. Also, the snapshot log line's
"plates overloaded now / largest overload" refers to the last analysis, not to "now".
**Fix:** either expose the time of the last analysis (`last_section_analysis`) and write the
records only when it equals the current step's analysis (or write that time instead of
`simulation.time()`), or clear `section_records` in `deform_plates` when the analysis is not
due and skip writing an empty set. `--plate-log` could also force `interval = 0` (it already
sets `diagnose`). Adjust the doc of `Cli::plate_log` ("after every step") accordingly.

### 4. Low–medium: the breakup timer survives other splits of the plate (topology.rs:~300–306, breakup.rs timer)

`split_plate_cells` clears the parent's `pending_breakup` but keeps its `breakup_timer`
(the new plate's is reset). If a plate that has been overloaded for a while is split by a plume
rift, a deforming-plate split or a detached piece, the remaining part keeps the timer and can
break at its very next analysis along a section that was never overloaded for
`breakup.time` (its shape and loads have changed). Merges keep the receiving plate's timer too
(`merge_plates_inner` clears only `pending_breakup`).
Why it matters: the "overloaded for `time`" rule is the delay that models localisation; it is
bypassed after any other topology change, which happens often to large plates. Bounded by the
decay rule, so not dramatic.
**Fix:** reset `breakup_timer` of the parent in `split_plate_cells` (and of `into` in
`merge_plates_inner`), next to the `pending_breakup = None` lines — or document that the timer
is deliberately per plate and survives topology changes.

### 5. Low: timer granularity with `interval` (breakup.rs:~385–392, ~470–485)

With `interval` = 2 and `dt` = 1 the timer advances in steps of 2 Myr, so the default
`time` = 5 Myr effectively becomes 6 Myr (3 analyses), and a section must be overloaded at
three sample points. The sampling also means an overload that lasts only between two analyses
is invisible. Fine physically (loads change over Myr), but `BreakupParams::time` docs should
say the delay is rounded up to a multiple of `interval`, or the threshold compared as
`timer + 0.5 * interval >= time`.

### 6. Low: loads omit the sheet's boundary couplings on the deformation and the lagged torque (breakup.rs:~395–420)

The load is `forces − drag · u` (drag at the material velocity, correct, see below), but the
sheet's solve also applies the boundary couplings (`entries`: shear coupling and nascent
resistances) on the *deformation* velocities and balances a `lagged_torque`; neither is in the
analysed loads. The net-torque removal absorbs their net effect and `residual` reports it, so
this only redistributes loads near boundaries of deforming plates. Document it next to the
residual (or add the coupling forces `−k (Δu · d) d` at the two boundary cells, which are
cheap to compute from `entries`).

### 7. Low: disconnected sides of a level section (breakup.rs `most_overloaded_section`, `break_overloaded_plates`)

A level set of a BFS distance can split a plate into a side `A` that is not connected (e.g. the
far ends of a plate measured from an interior source; also possible after the min-cut flood).
The analysis then treats the pieces of `A` as one rigid body (a valid but non-optimal
mechanism: a lower bound of the overload of each part), and `break_overloaded_plates` splits
off all of `A`; `separate_detached_pieces` then makes each piece a plate or captures it
(`min_breakup_area` 1% > `min_area` 0.5%), so a "break" may end up as several plates or as a
capture. Known from the plan ("handled by the detached-piece rule"); the minimum-size check
in `break_overloaded_plates` applies to the total, not to the pieces. Consider keeping only the
connected component of the small side that touches the section's most loaded edges, or
checking `min_cells` per connected piece.

### 8. Low: bookkeeping

* Stress breakups are not counted in `EventCounts::births()` (nor are sheet splits, the
  existing pattern), so plate births/deaths in the statistics no longer balance the plate
  count. Either add them or note it in the log line.
* A due breakup blocked by `max_plates` (breakup.rs:~850) is silently skipped; the other splits
  count `blocked_splits`. Count it too, for comparability.
* The breakup silently requires `sheet.enabled` (the analysis runs inside `deform_plates`,
  which returns early otherwise). Say so in `BreakupParams::enabled`'s doc.

### 9. Doc nit: sign in `SectionEdge::halves` comment (breakup.rs:~226–229)

The comment says the motion "opens by (ω × x) · n = ω · (n × x)". The two are negatives of
each other ((ω × x) · n = ω · (x × n)). The code is right: with `n` pointing from the loaded
side to the other, opening is `−(ω × x) · n = ω · (n × x)` = `ω · q`, and the tension test
(`opening > 0.9` for a pull away from the section) confirms it. **Fix:** "opens by
−(ω × x) · n = ω · (n × x) (n points away from the loaded side)".

### 10. Performance (minor)

* `section_overload` for the level side reruns the full limit analysis only to get
  opening/length/mean strength, which the sweep could have kept (the overload is already known).
  Cheap compared with the sweep, but trivially avoidable.
* `analyse_sections` calls `surface_crust` per cell again (already done in `cell_forces`), and
  `cell_forces` now runs for all plates on analysis steps. With `interval` = 2 this is ~3.5% of
  a step (author's figure); fine.
* `break_overloaded_plates` allocates and scans `O(cells)` per due plate; rare, fine.

## Checked and found fine

* **Limit analysis maths.** `D(ω) = Σ w (|ω·p| + a|ω·q| + b ω·q)` with `a = (1+k)/2`,
  `b = (1−k)/2` gives `v_n` in opening and `k |v_n|` in closing: correct. Units: torques
  `Σ r × f` (N, unit `r`), strength `F · R` per unit-sphere length, weights in unit-sphere
  lengths, so `ω · T / D` is dimensionless. The two points ±ℓ/4 per edge give a one-edge neck
  the exact plastic moment `F ℓ²/4`. Sliding lever `p = −n`, opening lever `q = n × x`: signs
  confirmed by derivation and by the beam-theory test (tension, compression ×3, shear, bending
  with k = 3 and k = 1).
* **IRLS.** Majoriser `|x| ≤ x²/(2|x₀|) + |x₀|/2` plus the exact linear term; the constrained
  minimum `ω = Q⁻¹(λT − g)`, `λ = (1 + T·Q⁻¹g)/(T·Q⁻¹T)` is correct; monotone acceptance,
  floor `1e-6 · scale`, tiny ridge keeps `Q` positive definite. The result is a kinematic
  (upper-bound) mechanism, i.e. a lower bound of the overload if not fully converged —
  conservative. `quick_overload` is a lower bound of `limit_overload` (any admissible ω).
* **Load assembly.** `forces` from `cell_forces` (all plates on analysis steps) contain the drag
  at the pre-solve rotations; subtracting `drag · ((ω_new − ω_old) × r + R·u)` with the stored
  `u` (plate frame, frame rotation already removed, rad/Myr) gives the drag at the material
  velocity `ω_new × r + u`, consistent with the sheet's solve. Frames (`plate.rotation` is not
  changed in `deform_plates`) and unit conversions (rad/Myr → m/s via `R / SECONDS_PER_MYR`)
  are right. Net-torque removal: `Σ r × ((M⁻¹ net) × r) c = net`, correct.
* **Level sweep.** Bucketing with `edge_count[d+2]` and prefix sums puts the edges between
  levels `d` and `d+1` at `[edge_count[d+1], edge_count[d+2])`, the fill matches the count
  (`SheetGeometry` neighbours are built from `mesh.neighbors` in the same order); the section at
  `level` uses levels `< level` for the side and its torque, edges oriented from the loaded side
  outward; BFS guarantees all crossing edges are between `level−1` and `level`; min part size
  checked on both sides.
* **Min-cut in the band.** `low < high` always; source tied to `distance == low`, sink to
  `distance == high`, each pair linked once (`m < cell`), capacities `face · s`; the residual
  reachable set is the source side, flood from `distance ≤ low` restricted to it; size check
  before use; the better of level and cut section is kept.
* **Timer/pending across renumbering.** `pending_breakup` and the timer live in `PlateSheet`,
  so they move with the plate through `swap_remove` in `remove_dead_plates`, sutures and
  captures; merges clear the receiver's pending side, splits the parent's (and the new plate
  starts clean); `pending_breakup` holds local vertices, which stay valid through the substeps;
  the cells are re-collected from the owners at break time and the sizes re-checked;
  `split_deforming_plates` runs first and clears a conflicting pending breakup. New plates
  appended during `break_overloaded_plates` are outside its loop range and have no pending side.
  `max_plates` respected.
* **Bit-identity.** With `breakup.enabled=0` (and no `--plate-log`) `analyse` is false:
  `cell_forces(&deforming)` as on main, no analysis, no pending side, topology changes only
  touch the new fields. With `diagnose` only, `cell_forces(all)` differs from main only for
  non-deforming plates, whose forces are not used by the solve; the test
  `the_analysis_alone_changes_nothing` covers it.
* **Determinism.** Only `Vec`s, fixed iteration order, sort with `total_cmp` and level
  tie-break, farthest-point ties by index, no hash maps, no transcendental calls (only
  `sqrt` via `normalize`/`length`; `math::cos` in a test). Wall-clock time only in statistics.
* **Web build.** `web_time::Instant`; the new `cfg(not(wasm32))` gates are only in `main.rs`'s
  file output.
* **Conventions.** `tracing::debug!` for the break event, `info!` for the log line, no
  `println!`, docs on all new items.

## Response (commit 1418629)

1. Fixed (`partial_cmp`, stops on NaN). Clippy native and wasm32, doc, fmt, tests pass.
2. Fixed: candidates are kept for `along != 0` in `quick_overload` and the warm start; new
   test `quick_estimates_see_bending_either_way_round`. The final 8-seed runs use this code.
3. Fixed: the records are cleared on steps without an analysis, so `plate_sections.csv` gets
   one row per plate and analysis; the log line says "at the last analysis".
4. Fixed: the parent of any split and the receiver of a merge restart their timer.
5. Documented on `breakup.time` (5 Myr take 6 Myr with the 2 Myr interval).
6. Accepted as is; documented in the results' caveats (`residual` logged).
7. Accepted: disconnected far sides are handled by the detached-piece rule (pieces < 1% are
   captured); noted in the plan.
8. Blocked breakups now count in `blocked_splits`; the dependence on the sheet is documented.
   Breakups are reported separately (log line "breakup by stress"), like the sheet's splits,
   not in `births()`.
9. Comment fixed.
10. Left: ~3.5% of the run time in total with the 2 Myr interval.
