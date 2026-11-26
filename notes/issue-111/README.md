# Issue #111: random seed when opening the 3D view

- **Desktop 3D view** (no `--headless`): without `--seed`, a random seed in 0..999 999 is chosen
  (`viewer::with_random_seed`, entropy from `std`'s `RandomState`) and added to the arguments
  the parameter panel starts from, so the panel shows it. `--seed N` is used as given.
- **Web**: without `seed` in the URL, a random seed (from `Math.random`) is chosen and put into
  the URL with `history.replaceState` (`?seed=N`, the rest of the query unchanged). A reload or
  a shared link therefore makes the same planet; opening the bare address makes a new one. A
  URL with `seed` is used as given.
- **Parameter panel**: the seed field has no default any more (hint "random"); an explicit `0`
  is kept like any other seed. "Defaults" empties it, and Restart with an empty field picks a
  new random seed, which the field then shows. The step-time hint's restart keeps the running
  seed.
- **Headless runs** keep clap's default seed 0, so experiments and tests stay deterministic.
- Random seeds are below one million so URLs stay short; any `u64` seed still works when given.
- The browser tests (`web/tests/browser_test.py`, `.github/workflows/browser-tests.yml`) now
  pass `seed=0` so CI does not depend on a random planet.
