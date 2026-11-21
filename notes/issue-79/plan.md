# #79: `--web-math`, a portable math mode

Goal (Sebastian): a CLI flag with which a desktop run reproduces exactly what the web version
computes, so that web runs can be reproduced in the development setup and a seed means the same
planet everywhere. Default runs stay as they are (D6 on #60: deterministic per platform).

## Where the platforms differ

- The RNG (seeded `ChaCha8Rng`) and the basic operations (`+ − × ÷`, `sqrt`, `mul_add`,
  rounding, float↔int conversions, `to_degrees`/`to_radians`, which are a multiplication by a
  constant) give the same bits on every platform. Rust does not contract `a * b + c` into FMA.
- The transcendental functions do not. Natively `f64::sin` etc. call the system math library
  (glibc on Linux: `sin@GLIBC_2.2.5`, `exp@GLIBC_2.29`, …). On `wasm32-unknown-unknown` there is
  none: LLVM lowers the calls to the symbols `sin`, `exp`, … which `compiler-builtins` provides
  from its copy of the `libm` crate (Rust 1.100: `library/compiler-builtins/libm`, version
  0.2.16, a port of musl). Not JavaScript's `Math` (the generated JS imports no `Math.*`).
- glibc and `libm` differ in the last bit for 3 % (sin), 10 % (exp, ln), 15 % (atan2) of
  inputs in [−3, 3]; the simulation is chaotic, so the planets diverge (already the initial
  planet differs in ~1 % of the cells).
- `powi` is the same everywhere: LLVM expands constant exponents into multiplications and calls
  `compiler-builtins`' `__powidf2` otherwise (the same square-and-multiply sequence).
- glam's `DVec3::angle_between`, `DQuat::from_axis_angle`, `DQuat::from_scaled_axis` (the plate
  rotation every substep) etc. call `f64` functions internally.

## Design

- `src/math.rs`: free functions `math::sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`,
  `exp`, `ln`, `powf`, … (all transcendental methods of `f64`), `sin_cos`, and glam replacements
  `angle_between`, `quat_from_axis_angle`, `quat_from_scaled_axis` that do exactly glam's
  arithmetic. Each dispatches on a global `AtomicBool` (relaxed load, a predictable branch):
  `platform::*` (the `f64` methods) by default, `portable::*` (the `libm` crate, called as plain
  Rust code) with `--web-math`. On wasm32 `web_math()` is the constant `true`
  (`cfg!(target_arch = "wasm32") || …`), so the web build always calls `libm` directly.
- The web build therefore no longer goes through LLVM's math intrinsics and `compiler-builtins`
  but calls the `libm` crate itself, the same Rust code as natively. This makes the equality
  independent of how LLVM treats intrinsics (constant folding with the host's libm,
  `pow(2, x) → exp2(x)`) and of the `libm` copy in a future toolchain. The crates.io `libm`
  0.2.16 and the copy in Rust 1.100's `compiler-builtins` have the same code for every function
  used (diff: only `cfg` names and test attributes).
- `--web-math` is set in `build_planet`, before anything is computed; the desktop view starts
  one simulation at a time, so a restart with another setting is safe. On the web the URL
  accepts `?web-math` (so a desktop command line carries over) but it changes nothing, and the
  parameter panel does not show it there.
- All call sites in the crate were rewritten (mechanically, ~210) except the 3D view
  (`src/viewer/`), which only displays the planet. The snapshot images go through `math` too.
- `clippy.toml` `disallowed-methods` lists `f64::sin` … and the glam methods that use them;
  `src/math.rs` and `src/viewer/` allow them. CI's clippy jobs (warnings denied) enforce it.
- Each snapshot logs `state hash` (FNV-1a over time and, per cell, elevation, crust, owner,
  precipitation, discharge, annual temperature) on both platforms, to compare runs.

## Not covered / caveats

- Hash map iteration order differs between wasm32 and 64-bit platforms (`FxHasher` depends on
  the pointer width, hashbrown's probing group width differs). The simulation's results don't
  depend on it in the runs checked (bit-identical up to 300 Myr), but this is evidence, not a
  proof; if a future change makes the results depend on the iteration order, `--web-math` runs
  would diverge from the web. The state hash shows it immediately.
- Different `libm` versions (after `cargo update`) can change results, like any code change; a
  test pins a few `libm` results (`portable_results_are_unchanged`).
