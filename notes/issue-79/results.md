# #79: results

Plan and design: [plan.md](plan.md).

## Bit-identity: web build vs native `--web-math`

Method: the real web build (`trunk build --release`, as `pages.yml` builds it: wasm-opt -O3,
the page plus the simulation Web Worker) in headless Chromium (`--headless=new`, SwiftShader for
WebGL), with the parameters as URL query; the console log is captured and the `state hash` lines
(logged with every snapshot, see `Planet::state_hash`) are compared with a native release run with
the same parameters as command-line options. Native runs on Linux x86-64 (glibc 2.36), Rust
1.100.0.

| Case | Parameters | Snapshots | Web = native `--web-math` | Native default = web |
| --- | --- | --- | --- | --- |
| A | `seed=0&subdivisions=5&duration=100&snapshot-interval=10` | 11 | yes (all 11) | no (all 11 differ) |
| B | `seed=1&subdivisions=5&duration=100&snapshot-interval=10&epoch=2.5&biosphere=abiotic` | 11 | yes | no |
| C | `seed=2&subdivisions=4&duration=300&snapshot-interval=25&initial=supercontinent` | 13 | yes | no |
| D | `seed=5&subdivisions=6&duration=30&snapshot-interval=10` | 4 | yes | no |
| E | `seed=0&subdivisions=5` (the web defaults: 500 Myr, snapshots every 50 Myr) | 11 | yes | no |
| – | `seed=3&subdivisions=5&duration=20&snapshot-interval=5` | 5 | yes | no |

Cases A–D and the last were run with the branch before the rebase onto #82/#83, case E with the
final code. Final hashes, e.g. E at 500 Myr: web `1992c6bce1a2a6f0` = native `--web-math`
`1992c6bce1a2a6f0`, native default `11307426d67fefcd`. Scripts: `compare.sh`/`webrun.sh` in the
task directory (headless Chromium + `python3 -m http.server`; nothing of it is in the repo).

Native default vs native `--web-math` differ already in the initial planet (~1 % of the cells'
values in the cell dump at t = 0, e.g. elevation of 93, precipitation of 384 of 10 242 cells at
seed 3), as expected: glibc and `libm` round 3–15 % of the results differently in the last bit.

## The web version itself is unchanged

The web build of `main` and of this branch give the same snapshot statistics (seed 3,
subdivisions 5, 20 Myr: all 131 statistics log lines identical, every printed digit), although
the branch calls the `libm` crate directly instead of `f64::sin` → LLVM → `compiler-builtins`.
`main` has no state hash, so this is a comparison of the printed statistics, not of bits.

## Default runs are unchanged

Native default runs of this branch are byte-identical to `main`: all snapshot files (PNG images,
cell, boundary and mesh CSVs with `--cell-dump`) of `--seed 3 --subdivisions 5 --duration 30
--snapshot-interval 10` (33 files), checked against `main` before and after the rebase onto
#82/#83 (also 20 Myr before it). Only the log gains the `state hash` line.

## Cost of `--web-math`

Paired runs on the shared server (other agents' jobs running, so ±15 % noise), `--seed 0
--duration 100 --snapshot-interval 100`, default 6 subdivisions, single-threaded, release:

| Round | `main` | branch, default | branch, `--web-math` |
| --- | --- | --- | --- |
| 1 | 47.2 s | 43.5 s | 46.2 s |
| 2 | 50.3 s | 47.5 s | 68.4 s |
| 3 | 60.3 s | 57.8 s | 46.8 s |

- Off path (flag not set): no measurable cost; the branch was 4–8 % *faster* than `main` in every
  pair, which is noise (same planet, byte-identical outputs). The dispatch is a relaxed atomic
  load and a predictable branch next to a 6–17 ns library call.
- On path: not resolvable at the whole-run level (medians 46.8 s vs 47.5 s); `--web-math` gives
  a different planet, whose runtime differs by itself. Per call (microbenchmark, ns per call,
  glibc 2.36 vs `libm`): sin 14.4/11.8, cos 14.5/12.7, exp 10.1/17.2, ln 5.9/8.8, asin 9.1/9.2,
  atan2 14.7/12.6, powf 17.1/60.7. Only `powf` (5 call sites, not per cell and substep) and `exp`
  are slower; the transcendental functions are a small share of a step.

## Review

An independent review ([review.md](review.md)) found no change of arithmetic in the
~210 rewritten call sites, the glam replacements identical to glam 0.34.3, `random_range` over
`usize` portable (rand samples through u32), and the viewer restart safe (the new simulation
thread joins the old one before `build_planet` sets the flag). Addressed:

- A web link without `subdivisions` uses 5, the desktop default is 6: the help text and
  `CLAUDE.md` now say to add `--subdivisions 5`. (Making `--web-math` change the default would
  make the flag change more than the math.)
- More glam methods in `clippy.toml` (`DVec3::angle_to`, `DVec4`, `DMat2`, `DMat4`, `DAffine*`
  rotations); f32 is not covered (the simulation uses only f64), stated in `clippy.toml` and
  `src/math.rs`.
- `state_hash` treats all NaNs alike (NaN payloads differ between x86 and wasm).

Not changed, documented:

- Two logged statistics sum floats in hash-map iteration order (`transient_stacked`,
  `mean_shortening`), which differs between wasm32 and 64-bit; they may differ in the last bit
  between web and desktop logs. They don't feed back into the simulation or the hash.
- No automated test against a recorded web hash: it would have to be re-recorded with a web build
  after every change of the simulation. The unit tests check each backend against its reference
  (`std`, `libm`), the glam replacements against glam, and pin a few `libm` results.
