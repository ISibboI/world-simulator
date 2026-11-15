# world-simulator

A Rust binary that simulates a planet, starting with plate tectonics on a sphere.

## Session notes

Notes on every chat session live in [notes/](notes/) and are checked into git.

- `notes/SUMMARY.md` is the rolling summary of the project: goals, current state, architecture,
  decisions, known issues and next steps. It is imported below, so it is loaded at the start of
  every session. Familiarise yourself with it before starting work.
- `notes/sessions/YYYY-MM-DD.md` holds the detailed log of the session(s) on that day: what the
  user asked for, what was done, decisions and their reasons, and open questions. If several
  sessions happen on one day, append a new `## Session N` section.

- `notes/ISSUES.md` is the numbered list of known physical-accuracy issues and their order of
  work. Keep it up to date when issues are fixed or found.

Before ending a session (or after any significant piece of work), update the session log and
`notes/SUMMARY.md`, and commit the notes together with the work they describe. Keep the summary
short; move detail into the session logs.

@notes/SUMMARY.md

## Development

- Toolchain pinned in `rust-toolchain.toml`; CI in `.github/workflows/ci.yml` runs fmt, check
  (all and default features), clippy (pinned/stable/beta/nightly), test, doc and a
  Windows/macOS check, with warnings denied.
- Run CI checks locally with `CARGO_BUILD_WARNINGS=deny` set.
- Run the simulation: `cargo run --release -- --help`; snapshots go to `output/` (ignored by
  git).
