# Review 1: branch agent/issue-87 (commits b700ef9, 74b6d95, e58b815) vs origin/main

Verdict: **correct and ready**. No blocking findings. Two low-severity documentation inaccuracies
and a few nits are listed below.

## Verification done
- Read the full diff, all of `src/hash.rs`, `clippy.toml` and rustc-hash 2.1.3's `src/lib.rs`.
- Re-derived 8 of the 16 reference values in an independent Python model (u8 7, u32 12345,
  isize -1 / i64 -1, (3,17), Kind::B(5), str "world-simulator" incl. the 0xff terminator,
  the 11-byte slice incl. its length prefix, u128). **All match.**
- `cargo clippy --all-targets --all-features -- -D warnings` on the branch: clean (1m19s, 2 cores,
  target agent-scratch/review-target).
- Clippy on a `git archive` copy with probe lines added: **all 8 probes flagged**:
  `std::collections::HashMap`, `std::collections::hash_map::HashMap`, `HashSet::new()`,
  `std::hash::RandomState`, `std::collections::hash_map::RandomState`,
  `std::collections::hash_map::DefaultHasher`, `std::hash::DefaultHasher`, and
  `std::collections::HashMap<_, _, Fx64BuildHasher>`. The `crate::hash::HashMap` alias is not
  flagged (expected: clippy matches the resolved DefId; the alias resolves to a TyAlias).
  The copy has been deleted.
- Followed every HashMap/HashSet use in `src/` (list below).

## 1. hash.rs correctness

**Portability of every `Hasher` method (verified by reading):**
- `write_u8/u16/u32/u64/usize`: widened to u64. `write_u128`: two u64 halves. OK.
- `write_i8/i16/i32/i64/i128`: the defaults call `write_uN(i as uN)`, which is the same width on
  every target. OK.
- `write_isize`: overridden to sign-extend through i64. OK. This matters for `#[derive(Hash)]` on
  enums: the discriminant is hashed as its repr type, which is `isize` by default.
- `write_length_prefix` (stable default, used for slice, Vec and str lengths) calls
  `write_usize`, which is widened. OK.
- `write_str` (stable default) does `write(bytes)` and then `write_u8(0xff)`. `write` reads
  little-endian 8-byte words (padded) and adds the length. Portable. OK.
- Tuples and structs hash field by field, so (u32,u32) and `Ownership{plate,local}` are portable.

**Finding L1 (low, doc only, verified by reading std's `impl_hash!`).** Integer *slices and arrays*
do not go through `write_uN`. std's `Hash::hash_slice` for every integer type, `usize` and
`isize` included, passes the slice's raw native bytes to `write`. So a key containing
`[usize]`, `Vec<usize>` or `[usize; N]` hashes 4-byte words on wasm32 and 8-byte words on
x86-64, and gets **different hashes**. The module doc claims "the same key gives the same hash
everywhere" (src/hash.rs:11-12). That claim is false for such keys, and the hasher cannot fix it.
No such key exists today (all keys are u32, (u32,u32) or `Ownership`). Suggest one sentence in the
doc: "except slices/arrays of `usize`/`isize`, which std hashes as raw bytes; don't use them as
keys". Slices of fixed-width integers (`[u32]`) are portable on little-endian targets (both
targets are LE), but they do *not* hash like rustc-hash, because `write` differs from rustc-hash's
`hash_bytes`. The claim "integer keys" is right as long as "integer keys" means scalars, tuples
and structs, not arrays.

**Claim "identical to rustc-hash 2.1.3 on 64-bit for integer keys": correct (verified).** On
64-bit, rustc-hash uses the same K `0xf1357aea2e62a9c5`, `add_to_hash` (`wrapping_add` then
`wrapping_mul`), rotate 26, `write_u64` adds once, `write_u128` adds the low half then the high
half, and `write_isize` defaults to `write_usize(i as usize)`, which is sign-extended on 64-bit.
Everything is bit-identical except `write(bytes)` (rustc-hash: `hash_bytes` + `write_u64`), so
str/byte/integer-slice keys differ. That is consistent with the doc ("Byte strings ... get a simple
portable hash"). The `same_as_rustc_hash_on_64_bit` test covers u32, u64, tuples, usize, isize,
u128 and the enum.

**Reference values:** plausible and independently re-derived (see above). `Kind::A` hashes to 0
(discriminant 0, `0*K`). That is correct, a known Fx property, and harmless.

**Hash quality:** the same as the tectonics had before (rustc-hash's Fx). For u32 the state is
x*K, and after the rotation the bucket index comes from bits 38 and up of x*K, which are
well-mixed for an odd multiplier. Pairs give `(a*K + b)*K`. With 32-bit components, full
collisions need `(a-a')*K ≡ b'-b mod 2^64` with |b'-b| < 2^32, which is practically impossible
for small indices. climate/ocean/sphere move from SipHash to Fx with (u32,u32) keys: fine and
faster. No u64-keyed map exists in `src/` today. Note: Fx is weak against adversarial or
high-bit-only keys (e.g. u64 keys that differ only above bit ~58), which is irrelevant here.

**Nit N1:** the test doc points to `notes/issue-87/` (src/hash.rs:137, and `notes/issue-87/results.md` is
referenced in CLAUDE.md), but `notes/issue-87/` is still **uncommitted** in the worktree, along
with the CLAUDE.md edit. results.md also still has placeholders (`E_LINES`, `E_DIFF` in case E).
Commit them before the PR.

## 2. Remaining iteration-order dependence (verified by reading each use)

All non-test uses:
| Location | Use | Order-safe? |
|---|---|---|
| climate.rs:1086 `corners` | built by entry/push in face order; read by lookup | yes |
| ocean.rs:50 `third` | same | yes |
| sphere.rs:70 `midpoints` | entry/or_insert_with in face order (ids by face order) | yes |
| main.rs:1137 `shown` | counts, lookup | yes |
| polarity.rs `pairs` | `end_pass` (integer counts, per-segment updates), `end_step` (per-pair only, integer stats, `retain`), `merge` (keys sorted), `split` (now sorted), `remove_plate` (drain/collect; layout only), `segments()` | yes |
| dynamics.rs:200 `sums` (by segment id) | accumulated in boundary order, read by lookup | yes |
| dynamics.rs:230 `pairs.values_mut()` in `update_segment_motion` | per-segment, integer stat, bool OR | yes |
| dynamics.rs:697 `in_contact` | contains | yes |
| dynamics.rs:714 slab loop | per-segment updates; `transient_stacked` now sorted; `localisations` is an integer | yes (fixed) |
| statistics.rs:456 `seen` | dedup in owners order | yes |
| statistics.rs:530 shortenings | sorted sum (fixed); max is a fold with `f64::max` (order-free without NaN) | yes |
| statistics.rs:541 `slabs` | only `.len()` | yes |
| statistics.rs:585 `cells_per_sample` | integer sum, max, keys -> sorted `origin_thickness` | yes |
| boundaries.rs:207/934 `collisions` | only `.len()` (mod.rs:1039) | yes |
| boundaries.rs:741 `seen` | insert/clear per event, results in `reached` order | yes |
| weak_zones.rs:353/373 | lookup / entry | yes |
| mod.rs:711 `boundary_buoyancy` | float sums per key in cell order; collected and **sorted** before rng draws | yes |
| mod.rs:738 `trench_pairs` | contains | yes |
| events.rs:390 `shared` | max_by with a total tie-break on the key | yes |
| events.rs:1782-1900 contacts/converging/suturing; `pairs` loop | per-pair state; `.min()` on keys; `ready` sorted | yes |
| events.rs:2087 `subducting` | contains | yes |
| events.rs:2111 `neighbours` | min_by mean, then count, then key (total) | yes |

`Polarities::split`: with the keys sorted, the id-collision check (`pairs.values().any(...)`,
which is order-free) and the progressive inserts now give deterministic ids. `merge` was
already sorted. I found **no remaining order dependence** in non-test code. The test-only
iterations (topology.rs:442/669/675, dynamics.rs:1078, `snapshot` sorted) are harmless.

Related caveat (not a bug): `Planet::state_hash` (planet.rs:237) does not cover polarity/segment
state (ids, slab lengths), so the empirical "identical state hashes" check would not see a
segment-only divergence. The diagnostic also compared logs and the boundary CSV, which does
cover ids.

## 3. clippy.toml disallowed-types
- Paths are correct and resolve through re-exports. Verified empirically: the `hash_map::`
  spellings and the `std::hash::` re-exports are all caught, because matching is by DefId.
- `rustc_hash::FxHasher` / `FxBuildHasher` resolve even in non-test builds where the
  dev-dependency is absent, because rustc-hash 2.1.3 is a transitive dependency (`type-map`, via
  egui/eframe, always enabled; no cargo features). If that transitive dependency ever went away,
  recent clippy would warn that the config path does not resolve. Low risk. Could add
  `allow-invalid = true` to those two entries if wanted (optional).
- `hashbrown`, `indexmap` and `ahash` are not direct dependencies, so they can't be named
  without editing Cargo.toml. Not listed, which is acceptable. Optional: add
  `hashbrown::HashMap`, `rustc_hash::FxHashMap` and `rustc_hash::FxHashSet` (these are type
  aliases to std's HashMap with FxBuildHasher, so the std entry already catches their expansion
  only if clippy sees the alias target. It does not, see the alias observation above). So
  **`rustc_hash::FxHashMap<K,V>` would currently slip through** unless the user also names
  `FxBuildHasher`. Very low risk (rustc-hash is now dev-only, so non-test code can't name it
  directly anyway, except via the transitive crate, which isn't in the extern prelude). Not
  verified empirically. Mention only.
- False positives: none seen; the branch is clippy-clean with `-D warnings`.

## 4. Native results vs main
- Integer-key hashes are bit-identical to rustc-hash on 64-bit, so tectonics map layouts and
  orders are unchanged.
- The climate/ocean/sphere/main maps changed hasher (SipHash with a random seed -> Fx). Their
  results never depended on order (lookups only), so there is no output change.
- `transient_stacked` and `mean_shortening` changed summation order, so the last bits can change.
  These are logged statistics only, not fed back into physics (transient_stacked is cumulative
  in `SegmentStatistics`; mean_shortening is computed only in `statistics()`).
- `split` key order changes the ids of copied segments. Ids are labels, read only via
  `segment(a,b,id)` / `sums[id]` lookups, so physics is unchanged and only the boundary CSV
  `segment` column changes. Consistent with the implementer's measurements.
- I found nothing else that would change native results.

## Summary of findings
1. L1 (low, doc): src/hash.rs:11-15. Slices/arrays of usize/isize are hashed by std as raw native
   bytes and are not portable. Narrow the claim. Verified by reading std.
2. N1 (nit): notes/issue-87/ and the CLAUDE.md edit are uncommitted; results.md has placeholders
   (E_LINES/E_DIFF). Verified.
3. Info: `rustc_hash::FxHashMap/FxHashSet` aliases are not in disallowed-types and would not be
   caught via the std entry. Unverified, very low risk.
4. Info: the rustc_hash disallowed paths resolve only through a transitive dependency.
   Optionally use `allow-invalid`.
