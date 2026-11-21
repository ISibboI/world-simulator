# #87: deterministic hash maps, identical on wasm32 and desktop

Follow-up of #79 (PR #85). Code: [`src/hash.rs`](../../src/hash.rs).

## Choice: a local Fx hasher with a `u64` state

No crate checked guarantees identical hashes on 32- and 64-bit targets (read from their
source, not their README):

| Crate | Pointer-width dependence (source) |
| --- | --- |
| `rustc-hash` 2.1.3 (used until now) | state is a `usize`; on 32-bit a different multiplier (`0x93d765dd`) and rotation (15 vs 26); `write_u64` adds two 32-bit halves; `write(bytes)` uses a different `multiply_mix` without 128-bit multiplication |
| `foldhash` 0.1/0.2 | `folded_multiply` has a different (carry-less approximation) path on 32-bit targets, wasm32 included unless `wide-arithmetic`; `write_usize` widens differently; the default seed is per-process random |
| `ahash` 0.8 | AES path on x86 vs fallback elsewhere, `folded_multiply` differs on 32-bit; random seeds by default |

So the crate now has its own hasher, as proposed on the issue: `Fx64Hasher`, the Fx hash of
`rustc-hash` 2.x (one wrapping add and multiply by `0xf1357aea2e62a9c5` per integer, rotate left
by 26 at the end) with the state fixed at `u64`:

- `write_u8/u16/u32/u64` and `write_usize` add the value widened to `u64`; `write_u128` adds two
  halves; `write_isize` sign-extends to `i64` (the default would go through `usize`, giving 32
  one bits for −1 on wasm32 and 64 natively; `#[derive(Hash)]` hashes enum discriminants as
  `isize`). The other signed writes default to the unsigned ones of the same width, which is
  portable.
- `write(bytes)` adds the little-endian 8-byte words (the last one zero-padded) and the length.
  No map in the crate has string or byte keys; this is only for completeness.
- Not fixable in a hasher: std hashes slices and arrays of integers as raw native bytes, so a
  key containing `[usize]`/`Vec<usize>` would still hash differently on 32- and 64-bit. No such
  key exists; the module documentation says to use fixed-width integers or tuples.
- For integer keys the hashes are **exactly** those of `rustc-hash` on 64-bit targets (test
  `same_as_rustc_hash_on_64_bit`), so native maps keep their layout and iteration order, and
  the speed is the same (on wasm32, `i64.mul` is a native instruction).

All maps and sets go through `crate::hash::{HashMap, HashSet}` (the tectonics aliases now
re-export them); the std-seeded maps in `climate.rs`, `ocean.rs`, `sphere.rs` and `main.rs` were
replaced. `rustc-hash` is only a dev-dependency (for the equality test). `clippy.toml`
`disallowed-types` rejects `std::collections::{HashMap, HashSet}` (with any hasher; `src/hash.rs`
is the one place allowed to name them), `RandomState`, `DefaultHasher` and `rustc_hash`'s
hashers (also the `FxHashMap`/`FxHashSet` aliases); checked by temporarily reintroducing a std `HashMap` in `sphere.rs` (3 warnings).

## Identical hashes are not an identical iteration order

std's `HashMap` (hashbrown) probes in groups of 16 slots with SSE2 (x86-64) but of 8 slots in
its generic implementation (wasm32, and also ARM with NEON), so colliding entries can land in
different slots, also tombstones after removals behave differently. Measured with a probe
program (maps of `(u32, u32)` keys, inserts and every 7th key removed; native x86-64 vs
wasm32-wasip1 under wasmtime):

| Entries | `rustc-hash` order native = wasm32 | `Fx64` order native = wasm32 |
| --- | --- | --- |
| 10, 100, 1000 | no | yes |
| 10 000, 40 000 | no | no |

(The probe's hashes: `rustc-hash` `(3u32, 17u32)` = `0x52eb296b8104116c` natively,
`0x0000000013040044` on wasm32; `Fx64` `0x52eb296b8104116c` on both.)

So the new hasher makes small maps iterate alike, but where iteration order affects a result,
a log line or an output file, the code must use a defined order. That is now the case:

- `transient_stacked` (stacked mass of transient diffuse zones; `dynamics.rs`) and
  `mean_shortening` (`statistics.rs`) sum their values sorted (`f64::total_cmp`), so the sums
  are the same for any map order.
- `Polarities::split` gave the copied segments of a split plate new ids in hash-map order of the
  pairs; it now sorts the pair keys first. Segment ids are labels only (no physics depends on
  them), but they are written to the boundary CSV dump (`--cell-dump`, column `segment`).

The other map iterations were already order-independent (lookups, counts, integer sums,
`min_by` with a key tie-break, keys sorted before use, e.g. initial subduction).

### Check: a permuted hasher changes nothing

A diagnostic build (branch `tmp/issue-87-seeded2`, not pushed) starts every hasher from a
non-zero state, which permutes the order of every map. Compared with the branch, 4 seeds,
`--subdivisions 5 --duration 200 --snapshot-interval 20 --cell-dump` (94 files per seed):

| | before the `split` fix (100 Myr) | after |
| --- | --- | --- |
| state hashes | identical | identical |
| all INFO log lines | identical | identical |
| output files (PNG, cell/boundary/mesh CSVs) | boundary CSVs differ (segment ids) in all 4 seeds (1–9 files each) | all identical |

(The state hash, `Planet::state_hash`, does not cover the subduction segments; the log lines and
the boundary CSVs do, which is how the segment-id dependence showed up.)

## Native default output vs `main`

- Default run (`--seed 0`, 6 subdivisions, 500 Myr; the timing runs below): all 71 output files
  byte-identical to `main`, all log lines identical (apart from the run time).
- 4 seeds × 200 Myr with `--cell-dump` (above): all PNGs, cell and mesh CSVs, state hashes and
  log lines identical to `main`. Only the `segment` column of the boundary CSVs differs after
  the first plate split (39 of 44 boundary files; checked that every other column is
  identical): the ids of copied segments are now assigned in key order instead of `rustc-hash`
  order. Before the `split` fix (100 Myr), everything was byte-identical to `main`.
- The two statistics are summed in a different order than before, so their last bits can differ
  from `main`; the printed values (rounded) were identical in all runs.

## Web = native `--web-math`, now also all statistics lines

As in #79: the real web build (`trunk build --release`) in headless Chromium vs a native
release run with `--web-math` and the same parameters; compared are all INFO lines the
simulation logs (state hashes and every statistics line, ~300 lines per run; only the run-time
line excluded). Scripts in the task directory (`~/tasks/issue-87/tools/`, not in the repo).

| Case | Parameters | Snapshots | INFO lines | Differing |
| --- | --- | --- | --- | --- |
| A | `seed=0&subdivisions=5&duration=100&snapshot-interval=10` | 11 | 303 | 0 |
| B | `seed=1&subdivisions=5&duration=100&snapshot-interval=10&epoch=2.5&biosphere=abiotic` | 11 | 303 | 0 |
| E | `seed=0&subdivisions=5` (web defaults, 500 Myr) | 11 | E_LINES | E_DIFF |

## Test of 32/64-bit identity

- `hash::tests::reference_values` pins the hashes of `u8`, `u16`, `u32`, `u64`, `u128`, `usize`,
  `isize`, signed values, tuples, a derived enum, a `str` and a byte slice to fixed values. The
  values were derived independently of the Rust code, by a Python model of the hasher and of
  std's `Hash` impls.
- `pointer_width_independent`: `usize`/`isize` hash like `u64`/`i64`.
- **Run on wasm32**: `src/hash.rs` is self-contained, so its tests compile on their own for
  `wasm32-wasip1` (`rustc --edition 2024 --test --target wasm32-wasip1 -O src/hash.rs`) and run
  under wasmtime (Python `wasmtime` package): both tests pass on wasm32, with the same reference
  values as on x86-64. The whole crate's test suite can't run on wasm32 (eframe/wasm-bindgen);
  CI checks the wasm32 build with `cargo clippy --target wasm32-unknown-unknown`. Not added to
  CI (would need the wasip1 target and a wasm runtime there); the reference test on x86-64 pins
  the same values.

## Timing

Paired default runs (`--seed 0`, 6 subdivisions, 500 Myr, release, `runs --placement local`,
alternating, other agents' jobs running):

| Round | `main` | branch |
| --- | --- | --- |
| 1 | 241.8 s | 243.3 s |
| 2 | 244.9 s | 243.1 s |
| 3 | 242.3 s | 243.5 s |

Medians 242.3 s vs 243.3 s (+0.4 %, noise; the hashes of the integer keys are bit-identical to
before, so the maps do the same work).
