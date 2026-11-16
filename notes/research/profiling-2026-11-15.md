# Runtime profile (2026-11-15, commit 101d084, seed 0, subdivisions 6, 500 Myr, 358 s)

Instrumented scratch copy (`agent-scratch/prof`, patch `agent-scratch/profile_patch.py`;
`perf` is blocked by `perf_event_paranoid = 4`). Wall-clock seconds per 50 Myr window:

| Phase | 50 | 100 | 150 | 200 | 250 | 300 | 350 | 400 | 450 | 500 |
|---|---|---|---|---|---|---|---|---|---|---|
| whole step | 25.8 | 37.7 | 58.7 | 76.4 | 35.7 | 17.1 | 19.4 | 22.0 | 29.4 | 35.6 |
| tectonics | 23.1 | 35.0 | 55.9 | 73.7 | 33.0 | 14.5 | 16.9 | 19.5 | 26.9 | 33.0 |
| – plate lookup per cell (resolve) | 17.9 | 28.1 | 46.2 | 61.1 | 26.0 | 10.0 | 11.8 | 14.0 | 20.4 | 25.6 |
| – additions (incl. ledger) | 1.4 | 2.0 | 3.0 | 4.1 | 1.9 | 0.9 | 1.1 | 1.3 | 1.7 | 2.1 |
| – ledger total-mass sums | 1.6 | 2.1 | 3.2 | 4.1 | 2.1 | 1.1 | 1.2 | 1.3 | 1.8 | 2.1 |
| – consumption, gap filling, aging | 2.0 | 2.8 | 4.0 | 6.1 | 2.7 | 1.3 | 1.5 | 1.9 | 2.4 | 2.9 |
| – mantle flow evaluation | 0.7 | 1.0 | 1.2 | 1.3 | 1.2 | 1.3 | 1.3 | 1.4 | 1.3 | 1.3 |
| temperature model | 1.5 | 1.3 | 1.3 | 1.2 | 1.2 | 1.2 | 1.2 | 1.2 | 1.2 | 1.2 |
| precipitation | 0.6 | 0.6 | 0.6 | 0.6 | 0.6 | 0.6 | 0.5 | 0.5 | 0.6 | 0.6 |
| erosion | 0.4 | 0.4 | 0.5 | 0.4 | 0.5 | 0.4 | 0.4 | 0.4 | 0.4 | 0.4 |
| events, flow, removal, boundaries, snapshots | ~1.5 each window |||||||||
| **substeps per step (mean)** | 6.5 | 8.9 | 11.1 | 20.2 | 9.4 | 4.4 | 5.4 | 6.5 | 8.3 | 9.6 |
| **plates (mean)** | 13.7 | 15.4 | 19.8 | 17.0 | 15.3 | 13.0 | 12.4 | 12.1 | 14.7 | 15.7 |

Findings:

- The cost per step is ≈ substeps × plates × cells × ~85 ns: every substep, every plate looks up
  its nearest local sample for every one of the 40,962 cells (greedy mesh walk). This is 75–83%
  of the run time.
- Run time does not grow steadily; it follows the number of substeps (set by the **fastest**
  plate moving ≤ 0.5 cell per substep — usually a tiny plate at 30–55 cm/yr) times the number
  of plates. The slowdown after the first iterations comes from plates multiplying (12 → 20)
  and small plates speeding up.
- Everything else together is ~15–20%: ledger sums (~6%), aging all samples every substep,
  mantle flow evaluation, temperature, precipitation, erosion.

## Options discussed with the user (2026-11-15)

User decisions: replace "bounding cap per plate" by **incremental matching** (each plate only
searches the cells where it had crust after the last matching plus a ring around them, valid
while it moved less than one cell; otherwise a full scan), with a debug mode comparing against
the full scan and rigorous tests that both modes give identical results. **No multi-core**
for now (efficiency first, not raw speed). Other options (flat neighbour arrays, cheaper
ledger, birth time instead of age, cached boundary list; accuracy-changing: substeps from
typical speeds, 1-cell movement, slab resistance, scatter matching, coarser tectonic grid,
less frequent temperature/mantle updates, f32) remain open for the user to decide.

## Result: incremental matching (implemented 2026-11-15)

`src/tectonics/matching.rs`. Seed 0, 500 Myr: 414 s → 168 s (2.5×), identical output; 99.8% of
matchings incremental, ~0.1 searched cells per plate and cell. A post-change perf profile
(200 Myr, debug symbols) is in `agent-scratch/speed/perf.data` if still present; see the session
log for its summary.

## Follow-ups (2026-11-16)

Hidden-sample tracking in the matching (−12% cycles) and arrays + `rustc-hash` instead of SipHash
maps (−11% cycles); details and the hasher comparison in `../sessions/2026-11-16.md`.

Further (2026-11-16, session 2): ledger checked only when reported, formation time instead of
age, boundary list reused within a step, ring scratch in the gap filling, trig-free mantle flow:
−34% cycles at 200 Myr (rounding changes only; see the session log). Still open: flat neighbour
arrays and the accuracy-changing options above.
