# world-simulator

A Rust binary that simulates a planet, starting with plate tectonics on a sphere.

## Project goal

The goal is to simulate an **Earth-like planet, not Earth itself**. Realism means physical
plausibility for an Earth-like planet: real physical processes, conservation, and values within
plausible ranges. Earth's present-day numbers (including those in the archived notes) are
orientation and plausibility checks, not exact targets to match.

Physical inaccuracies caused by the coarse grid (by default an icosphere with 6 subdivisions,
~41k cells about 110 km apart; 5 subdivisions and ~220 km on the web) are accepted, even when
they are atypical for an Earth-like planet or any planet at all. Don't spend effort fixing
sub-grid effects, e.g. transform-fault offsets shorter than a cell (why self-orienting ridges
were declared out of scope on #43). Do explain such artefacts where relevant: in notes, PR
descriptions and answers on issues.

## Issue and progress tracking

Issues are tracked on **GitHub** (https://github.com/ISibboI/world-simulator/issues); new
problems, ideas and tasks become new GitHub issues. Start work on an issue or PR by reading it on
GitHub (`gh issue view N --comments`, `gh pr view N --comments`) and follow its links into
`notes/` from there.

GitHub issues, PRs and comments carry only what matters to the user: brief answers to his
requests, a short summary of what changed, key results, caveats, decisions he must make, and
links. Plans, plan reviews, method write-ups, long result tables and other large prose go into
`notes/issue-<N>/` (or `notes/pr-<N>/` for work requested only on a PR), e.g. `plan.md`,
`results.md`, committed on the task's branch and linked from the issue or PR as
`https://github.com/ISibboI/world-simulator/blob/<branch-or-main>/notes/issue-<N>/<file>`.

The older files in [notes/](notes/) (old issues, `notes/plans/`, `notes/research/`,
`notes/sessions/` and the project summary) are an archive (as of 2026-11-19): read them for
context, but do not extend them. `notes/ISSUES.md` maps the old notes issue numbers to GitHub
numbers; cite issues as "issue 13 (#14)" where the old number is still in use, or just "#N". No
session logs and no new entries in `notes/SUMMARY.md` or `notes/ISSUES.md`.

The archived summary is imported below as background: its overview and architecture describe
the code as of 2026-11-19, and its "Current plan" is still the order of work until its items
are moved to GitHub issues. Its state and known-issue sections may be stale; the code and the
GitHub issues are authoritative, and its preference to keep session notes in `notes/` is
superseded by this section. Its still-binding user preferences are under "Working conventions".

@notes/SUMMARY.md

## Working conventions

- Before working on an issue, have an independent agent review the plan; get independent
  evaluations of the results. Plan, review and results go to `notes/issue-<N>/`, with a short
  summary and link on GitHub.
- Calibrate only as far as needed to confirm soundness; prefer fixing physics over tuning.
- Efficiency before parallelism: the simulation is single-threaded by choice.
- Logging uses `tracing`; use its macros, never `println!`, for console output.
- CI follows the user's existing "Tests and Lints" workflow template; pin GitHub Actions to the
  latest **major version tag** (e.g. `actions/checkout@v7`), not SHAs.

## Development

- Use `agent-scratch/` in the repository (ignored by git) for all temporary files of agents:
  scratch copies of the source, diagnostic builds, simulation outputs, logs. Do not use `/tmp`
  (a small RAM disk on this machine). Diagnostic builds should share one target directory,
  e.g. `CARGO_TARGET_DIR=agent-scratch/target`.

- Toolchain pinned in `rust-toolchain.toml`; CI in `.github/workflows/ci.yml` runs fmt, check
  (all and default features), clippy (pinned/stable/beta/nightly), test, doc and a
  Windows/macOS check and a wasm32 check/clippy of the web version, with warnings denied;
  `.github/workflows/pages.yml` builds the web version and deploys it to GitHub Pages from
  `main`.
- Web version (`src/web.rs`, `web/`, `Trunk.toml`): the same viewer and simulation in the
  browser, the simulation in a Web Worker, parameters as URL query parameters. Build or serve
  it with `trunk build --release` / `trunk serve`. Keep `cfg(target_arch = "wasm32")` gates
  confined to entry points and file output; see `notes/issue-60/results.md`.
- Run CI checks locally with `CARGO_BUILD_WARNINGS=deny` set.
- Run the simulation: `cargo run --release -- --help`; snapshots go to `output/` (ignored by
  git).
- Determinism: a seed gives the same run every time on one platform. `--web-math` makes a
  desktop run bit-identical to the web version (and to `--web-math` on any other platform; give
  `--subdivisions 5` for a web link without it), at about the same speed; each snapshot logs a
  `state hash` to compare runs. Transcendental math (`sin`, `exp`, `ln`, `atan2`, `powf`,
  glam's `angle_between`, …) must go through `src/math.rs`; Clippy (`clippy.toml`,
  `disallowed_methods`) rejects the direct calls outside it and the 3D view. See
  `notes/issue-79/`.
- **Experiments (simulation runs, seed sweeps, diagnostic builds) run on the server `tktie`**
  (`ssh tktie`, 8 cores, NixOS), not on this machine, so that this machine stays free for
  gaming. Leave 2 cores free on the server (at most 6 runs at once). Keep the server's files in
  `~/agent-scratch/world-simulator/` there. Builds, tests and short checks stay local. Build the
  release binary locally, copy it with `scp`, and start it through the wrapper
  `~/agent-scratch/world-simulator/run.sh BINARY ARGS…` (NixOS cannot run generic Linux binaries
  directly; the wrapper calls the Nix glibc loader). The login shell there is fish: wrap
  commands in `bash -c '…'`.
