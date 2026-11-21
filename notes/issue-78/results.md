# Issue #78: results (implementation)

Plan and decisions: [plan.md](plan.md). Research: [research.md](research.md).

## What changed
- **`src/device.rs`** (new, web and tests): the guess and the rules about the stored value.
  - **The guess:** phone or tablet → 5, otherwise 6. A device counts as a phone or tablet if
    any of these hold: `userAgentData.mobile`; `Android`, `iPhone`, `iPad` or `Mobile` in the
    UA; a Mac UA with `maxTouchPoints > 1` (iPad); or `(pointer: coarse)` together with
    `(hover: none)`.
  - Chromium devices with `deviceMemory` ≤ 4 also get 5.
  - **Precedence:** the URL wins over the stored value, which wins over the guess.
  - **Validation:** a stored value must parse and be ≤ 8.
  - **What is stored:** only the subdivisions, under the key `world-simulator.subdivisions`.
- **`src/web.rs`**: reads the signals on the page and guesses.
  - Without `subdivisions` in the URL, it inserts the stored value, if there is one, as an
    argument. The guess is the panel's default.
  - **The worker URL always names the subdivisions; the page URL only chosen ones.**
  - **Storage:** every restart from the panel or from the hint stores the subdivisions, or
    removes the key when they equal the guess. So "Defaults" followed by Restart resets the
    memory. Every storage access is fallible and is skipped if storage is unavailable.
  - **Hidden page:** a `visibilitychange` listener marks the page as hidden. Updates that
    arrive while the page is hidden, or that span a hidden time, carry no step time.
  - The worker sends `step_seconds` with every update after the first.
- **`src/main.rs` `simulate`**: `on_step` also gets the time since its previous call (`None` for
  the initial state). This is the period at which the view gets new states. Headless runs
  ignore it.
- **`src/viewer/pace.rs`** (new): the step-time tracker and the hint.
  - Median of 10 steps after skipping 2. An early verdict after 3 steps that each took more
    than 2.5 s.
  - **When a hint appears:** above 1.2 s or below 0.2 s.
  - **What it proposes:** subdivisions predicted at ≤ 0.8 s, never fewer than 5, at most 6 on
    the web and 7 natively.
  - **Prediction:** separate cautious factors for going down and going up. A step time measured
    earlier in the session with the same other options wins over the prediction.
  - **Hysteresis:**
    - the hint goes after 10 steps with the median in 0.3–0.9 s;
    - "Dismiss" moves the threshold out by ×1.5;
    - it never goes back to subdivisions measured as too quick.
  - **"Restart with N"** appears only in the first 10% of the duration. Later the hint only
    informs.
  - Nothing is shown before the first update, after a failure or after the end of the run.
- **`src/viewer/parameters.rs`**:
  - **Default override:** the default of the subdivisions can differ from clap's (the guess),
    for the field, for "Defaults" and for leaving the option out of the arguments. The
    simulation gets the override, see `launch_from`.
  - **Bug fixed:** `set_values` now takes only values given on the command line. It used to copy
    clap's defaults into the fields, which would have shown 5 instead of the guess.
  - `launch_with_subdivisions` restarts the running simulation with other subdivisions and keeps
    pending edits pending.
  - The status line shows "· 0.41 s per step".
  - **Where the hint is drawn:** under the status line when the panel is open, and right under
    the "⚙ Parameters" button when it is hidden.
- **Docs:** the `--web-math` help and `CLAUDE.md` no longer say that the web always uses 5.

## Tests
- **Unit tests** (all native):
  - the guess (UA strings of Android, iPhone, iPad as Mac, Mac, Windows, Linux; client hint,
    touch-only, touch laptop, low memory);
  - precedence and stored-value validation;
  - prediction, and proposals: aim ≤ 0.8 s, never below 5, nothing at ≤ 5 when slow, 3 → 5 when
    quick, the maximum;
  - measured values overriding predictions;
  - the median window with skipped steps and outliers, and the early verdict;
  - no hint in the band, slow and quick hints, no flicker around the threshold;
  - dismissal, restart memory (no way back), different options, invalid times, the finished
    run;
  - the default override in the panel, the hint restart keeping pending edits, and the option
    helpers.
- **CI checks run locally, with warnings denied:** fmt, clippy native and wasm32 (all targets
  and features), doc, tests.
- **Headless output byte-identical to main:**
  - Run: seed 3, 15 Myr, snapshots every 5 Myr, width 256, `--cell-dump`, main vs this branch.
    Checked before and after the rebase onto main with #81.
  - All 33 PNG and CSV files are identical, and the logs are identical apart from timestamps.
- **Web (trunk release build, headless Chromium with SwiftShader, Playwright):**

  | case | result |
  |---|---|
  | desktop, no parameters | worker `?subdivisions=6`, page URL unchanged, nothing stored ([screenshot](web-desktop-guess-6.png)) |
  | iPhone emulation (UA, touch, 390×844) | worker `?subdivisions=5`, panel hidden ([screenshot](web-phone-guess-5.png)) |
  | stored `4`, no parameters | worker `?subdivisions=4` |
  | stored `4`, URL `?subdivisions=5&seed=2` | the URL wins (5) |
  | stored `abc` | ignored, guess 6 |
  | `?subdivisions=7` | 2.6 s per step; hint "…too slow. With 5 subdivisions about 0.43 s. [Restart with 5]" in the panel ([screenshot](web-slow-hint.png)) and below ⚙ with the panel hidden ([screenshot](web-slow-hint-panel-hidden.png)) |
  | `?subdivisions=3&duration=2000` | 0.12 s per step; hint "…too quick. With 5 subdivisions about 0.37 s" ([screenshot](web-quick-hint.png)) |
  | clicking "Restart with 5" in that case | restart with `--subdivisions=5 --duration=2000`, URL `?subdivisions=5&duration=2000`, stored 5; then 0.36 s per step, no hint, the panel shows 5 with no pending change ([screenshot](web-after-hint-restart.png)) |
  | `?subdivisions=3` at 224 of 500 Myr | informational only: "Choose them in the parameters for the next run." ([screenshot](web-quick-hint-late.png)) |

- **Desktop** (vscreen, release build, `--subdivisions 7`): 2.5 s per step, hint proposing 5 in
  the panel ([screenshot](desktop-slow-hint.png)). The default of 6 takes 0.36–0.65 s per step
  natively, so normally there is no hint.

## Step times on the new main (after #81)
- Re-measured in headless Chromium after #81 (deformable margins) was merged. Other builds were
  running on the machine at the same time.

  | subdivisions | now | before #81 |
  |---|---|---|
  | 5 | 0.41–0.44 s | 0.38 s |
  | 6 | 0.74–0.91 s | 0.74–0.80 s |

- That is about +10–15%, partly from the load. The ratio 6/5 is still ~2.
- Both stay inside the target band, so the thresholds and prediction factors are unchanged.

## Caveats
- **The guess cannot see CPU speed.** Fast iPhones get 5 and slow laptops get 6; the hint
  corrects both.
- **At 5 subdivisions, a slow device gets no hint**, because nothing below 5 is proposed.
- **The predictions are cautious.**
  - From 7 at 2.5 s the hint proposes 5, not 6: 6 is predicted at 0.84 s, while the measured
    value is ~0.75 s.
  - Going up from a quick 5, the hint needs below 0.2 s × 3 ≤ 0.8 s.
- **Subdivisions in a link:** a link's `subdivisions` that equal the device's guess drop out of
  the page URL after the next restart, because the URL names only values that differ from the
  guess (D3). Any restart from the panel or the hint stores the running subdivisions, also when
  they came from a link.
- **Interaction with PR #84 (#80, start/restart banner):**
  - Both PRs touch `viewer/mod.rs`, `parameters.rs` and `web.rs`, so whichever lands second
    rebases.
  - The banner is a non-interactive full-width strip for waiting states. The hint needs buttons
    and sits next to the parameters, so it does not reuse the banner.
  - They do not overlap in time: the hint needs measured steps, and the banner disappears with
    the first update.
  - #84 closes the panel on narrow screens after a panel restart. The hint restart happens
    outside `show()` and would need the same rule.
