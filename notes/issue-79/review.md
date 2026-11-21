# Review 1 of agent/issue-79 (`--web-math`)

Reviewed: `git diff origin/main` (commits 50a1b6d, d1d5d4d, plus the uncommitted `src/viewer/mod.rs`
change) and `notes/issue-79/plan.md`. I edited nothing in the worktree.

Verification done:
- I read every rewritten call site (all ~210, file by file). I checked unary minus (`-x.sin()`,
  `-75f64.to_radians().sin()`, `-(a/b).ln() / c`, `-latitude.tan() * …`), parenthesised receivers
  (`(-x*x).exp()`, `(a*b).exp()`), and operand order for `atan2`/`powf` (receiver becomes the
  first argument). **Every one is equivalent.** I found no mis-parsed receiver.
- I compared against the glam 0.34.3 source (`f64/dvec3.rs`, `dquat.rs`, `math.rs`, std branch, since
  glam's `libm` feature is not enabled: `cargo tree -e features -i glam`).
  - `angle_between` is `acos_approx(dot / sqrt(ls*ls))`, where `acos_approx` is `acos(clamp(-1,1))`.
  - `from_axis_angle` is `sin_cos(angle*0.5)` followed by `axis*s`, `from_xyzw`.
  - `from_scaled_axis` matches bit for bit.
  - std's `f64::sin_cos` is `(sin, cos)`, so `math::sin_cos` matches on the default path.
- I built a small crate (outside the worktree) with this `clippy.toml` and glam 0.34.3. It confirms
  the lint resolves `f64::sin`, `glam::DVec3::angle_between`, `glam::DQuat::from_axis_angle` and
  `from_scaled_axis`, and catches function references (`.map(f64::exp)`, `let f: fn(f64)->f64 = f64::sin`).
  It does **not** catch `DVec3::angle_to` or `f32::sin` (see M1).
- rand 0.10.3: `random_range(0..=i)` with `usize` samples via u32 when the range fits
  (`UniformUsize`). It is portable. f64 ranges and `random_bool` use only exact operations.
- libm 0.2.16 arch code: wasm32 overrides only ceil/floor/fabs/sqrt/trunc/rint, x86 (sse2) only
  sqrt/fma, aarch64 fma/rint/sqrt. All are exactly rounded, so the crate gives the same bits on
  x86_64, aarch64 and wasm32.
- Viewer restart (`simulation_thread_launcher`): the new thread joins the previous one before
  `build_planet`, so `set_web_math` never changes during a run. Thread spawn and join give the
  needed happens-before, so the Relaxed ordering is fine. **No race.**
- I cancelled the full worktree clippy run (machine saturated). The implementer's own builds exist.
  The `use super::*` in `weak_zones.rs` picks up `math` from `tectonics/mod.rs`.

Requirement 1 (flag off: same results as main) holds by inspection.

## Findings, by severity

### H1: A web URL does not reproduce on desktop with `--web-math` alone (subdivisions default differs)
`src/main.rs:173-176`: `DEFAULT_SUBDIVISIONS` is 6 natively and 5 on wasm. The web URL / panel
args contain only the options that were given (`args_to_query`). So `?seed=3` on the web plus
`--web-math --seed 3` on desktop gives different planets. The help text (`src/main.rs:163-166`)
and CLAUDE.md promise "bit for bit the same planet as on the web". That is the issue's main use
case ("reproduce runs from web").

Fix (any of these):
- (a) With `--web-math`, default `--subdivisions` to 5 when not given. Make the field
  `Option<u32>` and resolve it in `build_planet`/`check`.
- (b) Make the web always write `subdivisions` (and any other wasm-specific defaults) into the URL.
- (c) At minimum, say in the help text and the notes that web runs need `--subdivisions 5`.

(a) is the most robust and matches "reflects exactly what the web version does". Whichever you
pick, grep for other `cfg(target_arch = "wasm32")` defaults in `Cli`.

### M1: Gaps in the `clippy.toml` coverage
Confirmed with the lint-check crate. These glam methods call `math::atan2/sin_cos/sin/cos/exp/…`
internally but are not listed:
- `glam::DVec3::angle_to` (atan2)
- all of `glam::DVec4::{sin, cos, sin_cos, exp, exp2, ln, log2, powf}`
- `glam::DMat2::{from_angle, from_scale_angle}`
- `glam::DMat3::from_scale_angle_translation`
- `glam::DMat4::{from_axis_angle, from_rotation_x, from_rotation_y, from_rotation_z, from_euler}`
  and the `to_euler` family
- `glam::DAffine2::{from_angle, from_scale_angle_translation}`
- `glam::DAffine3::{from_axis_angle, from_rotation_x/y/z}`

The f32 side is not covered at all: `f32::sin` etc. and `glam::Vec3/Quat/…` equivalents. None
of these is used outside `src/viewer/` today, so there is no current bug. But the stated guarantee
("Clippy rejects the methods elsewhere") is weaker than claimed.

Fix: add the missing f64 glam entries, and either add the f32 methods (`f32::sin`, …,
`f32::powf`) or say in the `clippy.toml` header that f32 is not covered.

### M2: No test exercises the portable path end to end
`src/math.rs` tests check `portable::*` against `libm::*` and the default dispatch against std.
Nothing runs the dispatch with the flag on, `quat_from_scaled_axis` (including the zero vector)
is untested, and nothing ties a `--web-math` run to an actual web run. The plan says results
were compared manually.

Fix:
- Add `quat_from_scaled_axis` cases (zero, small and large vectors) to
  `vector_functions_match_glam`.
- Add one golden-value test: a tiny planet (e.g. subdivisions 3, a few steps) with
  `--web-math`, whose `state_hash` was recorded from the wasm build. Run it in a separate process,
  e.g. an integration test invoking the binary with `--headless --web-math` and parsing the log,
  so the global flag cannot leak into other tests. This also catches `libm` or toolchain drift,
  which `portable_results_are_unchanged` only samples at 8 points.

### L1: Hash-map iteration order still reaches two logged statistics
I checked every iteration over the tectonics `HashMap`/`HashSet` (FxBuildHasher): `polarity.pairs`
in `dynamics.rs:230,701`, `polarity.rs:390,424,713`, `events.rs:1812,1843`; `mod.rs:713` (sorted);
`events.rs:396,2127` (tie-broken `max_by`/`min_by`); `statistics.rs:587-588` (integer). State updates
are per pair or per segment, or have deterministic tie-breaks, so **I agree with the plan that the
simulation state is order-independent today.**

Two exceptions are float sums in hash order, so wasm32 and 64-bit can differ in the last bit:
- `statistics.transient_stacked += segment.stacked` (`src/tectonics/dynamics.rs:750`), logged at
  `src/main.rs:877`.
- `mean_shortening` = sum over `polarities.segments()` (`src/tectonics/statistics.rs:533-536,731`).

These affect log statistics only, not state or output files. With the usual `{:.N}` formatting
they will practically never show.

Fix (optional): collect and sort before summing. Or make `Polarities::pairs` a `BTreeMap<(u32,u32), _>`,
which removes the whole class of risk for the most-iterated map (the plan's caveat). Note that
a portable hasher alone would not fix it, since hashbrown's group width also differs on wasm.

### L2: `state_hash` hashes NaN payloads
`src/planet.rs` (`state_hash`) hashes `to_bits()`. NaN bit patterns are not portable: x86 SSE
produces the negative default NaN, and wasm's NaN payloads are non-deterministic by spec. If any
hashed field can legitimately be NaN, identical runs would hash differently.

Fix: `let bits = if v.is_nan() { f64::NAN.to_bits() } else { v.to_bits() };`

### L3: Small doc and test nits
- `src/tectonics/polarity.rs:926` (test): `DQuat::from_rotation_z(0.3)` became
  `quat_from_axis_angle(DVec3::Z, 0.3)`. This is equal for a positive angle. For a negative angle
  the x/y components would be `-0.0` instead of `+0.0`. Test-only, harmless; just noting that it
  is not a bit-exact replacement in general.
- Help text "About 5% slower": still provisional. Update it once measured, in both
  `src/main.rs:165` and the `CLAUDE.md` line.
- `src/math.rs` module doc: "Everything else in this crate goes through here". True for `f64` only;
  add "(f64)" or cover f32 (see M1).
- The web version's own results change with this PR: it now calls the `libm` crate instead of
  LLVM intrinsics / compiler-builtins, which LLVM may also constant-fold with the build host's
  libm. Web seeds shared before this PR may no longer reproduce. Worth one line in the PR
  description.
- The `#![allow(clippy::disallowed_methods)]` inner attribute in `src/viewer/mod.rs` is still
  uncommitted. Commit it, or CI clippy fails on the viewer.

## Allow scope
`src/math.rs` (module-level allow, plus a redundant one on `platform`, fine) and `src/viewer/` are
sensible. The viewer only builds display data (`surface_vertices`, `cell_infos`, camera, picking).
Nothing computed there feeds back into the `Cli` or the simulation: panel values are passed as
argument strings. The snapshot PNGs and the cell dump go through `math`. Good.

## Flag semantics (web)
`?web-math` is accepted (it is not in `WEB_UNAVAILABLE`), parses, and is a no-op because
`web_math()` is constant `true` on wasm. It is hidden in the web panel via `WEB_IMPLIED` and
visible in the desktop panel, so a desktop restart can toggle it safely. The test
`implied_options_are_accepted` covers the parsing. OK.
