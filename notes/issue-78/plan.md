# Issue #78: subdivisions that fit the device (plan)

Findings and sources: [research.md](research.md). Independent review: [plan-review.md](plan-review.md).
No code has been written yet.

The plan has three parts. C works whatever A guesses.

- **A** — a static initial guess on the web.
- **B** — measuring the real step times.
- **C** — a hint that proposes a better number of subdivisions and applies it with one click.

## Summary of the research
- **The browser cannot read the CPU's single-thread speed.** No CPU model, no clock rate
  (fingerprinting), and the core count is useless for a single-threaded simulation. The only
  reliable signal across browsers is the **form factor** (phone or tablet vs desktop). In
  Chromium, `deviceMemory` helps a little as well.
- **There is no fast warm-up.**
  - In the browser the first step is the *slowest* (~1.5×).
  - After that the step time stays at its base level for ~100 Myr.
  - Then it drifts up: ×1.3–1.7 at 6 over 100–300 Myr, less at 5. The cause is mostly more
    tectonic substeps as the plates speed up. The drift is jumpy and depends on the seed.
- **Web step times (headless Chromium on tktie):**

  | subdivisions | step time |
  |---|---|
  | 4 | ~0.26 s |
  | 5 | ~0.38 s (~0.44 s by 200 Myr) |
  | 6 | ~0.74 s (~0.82 s by 150 Myr) |

  That is ~2× native.
- **Cost per level:** the cost grows ×1.4 from 4 to 5 and ×2–2.2 from 5 to 6 early in a run (up
  to ×3 late). The climate runs on a fixed coarse grid, so **going from 5 down to 4 only buys
  ~1.4×**. Going up costs more than going down saves, because the substep count doubles per
  level. The estimate for 6 → 7 is ×3.5–4.5.

## A. Initial guess (web only)
1. **Where it is computed.** The page computes it in `run_page` (`src/web.rs`), next to
   `ui_options`, which already queries `(pointer: coarse)`. `matchMedia` and `screen` do not
   exist in the worker.
2. **Phone class.** A device counts as phone class if any of these hold:
   - `navigator.userAgentData?.mobile` (Chromium);
   - the UA contains `Android`, `iPhone`, `iPad` or `Mobile`;
   - a Mac UA with `navigator.maxTouchPoints > 1` (an iPad that claims to be a Mac);
   - `(pointer: coarse)` and `(hover: none)`.
3. **Low-end desktop (D2).** `navigator.deviceMemory` is defined and ≤ 4 (Chromium desktop).
   This catches many cheap laptops and Chromebooks.
4. **The rule.** Phone class or low-end desktop → **5**, otherwise **6**.
   - No high-entropy hints, GPU strings or `cpuPerformance`; they add little (research §1.9).
   - The decision and the signals behind it are logged at `info`.
5. **Defaults and URL** (today the web default is the compile-time clap default of 5).
   - **The guess is the *default* of this page session**, not a value typed by the user.
   - `ParameterPanel::new` gets a default override for `subdivisions`. The field then shows the
     guess, and "Defaults" resets to the guess, not to 5.
   - **The worker query always carries an explicit `subdivisions=N`.** Today `run_page` uses one
     query string for the worker and for `replace_query`; these become two strings.
   - **The page URL carries `subdivisions` only when it is pinned.** Pinned means: it came from
     the URL, the user edited it, or the user accepted a hint. Otherwise shared links stay
     adaptive to the device (D3).
   - The panel drops values equal to the clap default when it builds its arguments. That
     comparison must be against the effective default (the guess), and pinned values must be
     kept even if they are equal to the guess.
6. **Expected failure cases (C corrects them):**
   - fast iPhones and iPads get 5 although 6 may fit;
   - slow laptops, old desktops, Firefox or Safari on cheap hardware, Android "Request desktop
     site" and iPads with a trackpad get 6, which is too slow;
   - throttled or power-saving devices: only C sees these.

## B. Measuring the step time
1. **What is measured.** The **period between consecutive `on_step` calls** in `simulate`
   (`src/main.rs`): `web_time::Instant` at each call, passed to `on_step` as
   `Option<Duration>`, with `None` for the initial state.
   - The period covers `planet.step`, building and posting the surface, and `snapshot`. That is
     the rate the user sees.
   - `snapshot` writes PNGs natively every 50 steps. The median absorbs these spikes.
   - Rendering on the page is not included.
   - An `on_step` call cannot time itself into the message it sends, which is why the period is
     measured instead.
2. **How it reaches the viewer.**
   - Web: as a new `step_seconds` field in the `{ time, vertices }` message.
   - Native: the same value as a field in `SurfaceUpdate`.
3. **Smoothing in the viewer.**
   - Ignore the first 2 steps (wasm tier-up, first upload with the triangles).
   - Then keep the **median of the last 10 steps**.
   - Normally no verdict before 10 steps. Fast path: if 3 consecutive steps exceed 2.5 s, give
     the "too slow" verdict at once, so a slow phone does not wait 30+ s.
   - `Viewer::start` (every restart) clears the window. Old workers are terminated and old
     native threads lose their channel, so stale measurements cannot arrive.
4. **Hidden tabs.** Mobile browsers freeze a page and its worker in the background, so one step
   can appear to take minutes. On the web, an update that arrives while `document.hidden` is
   tagged and dropped from the window, and the window restarts on `visibilitychange`. This is a
   `web.rs` concern, so the `cfg` stays confined to the entry point.
5. **Shown in the panel.** The status line shows the value, e.g. "t = 123 of 500 Myr · 0.41 s
   per step". This is useful on its own and natively too.

## C. Hint: too slow / too fast
**Target band:** 0.25–1 s per step (1–4 steps/s).

1. **Prediction.** Separate factors per level change: low ones for going down (the saving is
   assumed small) and high ones for going up (the cost is assumed large). Both are cautious.

   | change | going down: divide by | going up: multiply by |
   |---|---|---|
   | 3 ↔ 4 | 1.5 | 2 |
   | 4 ↔ 5 | 1.3 | 1.5 |
   | 5 ↔ 6 | 2 | 3 |
   | 6 ↔ 7 | 3 | 4.5 |

   A level measured earlier in the session with the same other parameters (key: the full
   arguments minus `subdivisions`) uses its measured median instead.
2. **Too slow.**
   - **When:** the median is above **1.2 s**. This is 20% hysteresis above the band.
   - **What it proposes:** the largest n' < n predicted at ≤ **0.8 s**, at least 3. The 0.8 s
     leaves room for the drift.
   - **If even 3 is predicted too slow:** offer 3 and say that the device is slow.
3. **Too fast.**
   - **When:** the median is below **0.2 s**, n+1 is predicted at ≤ **0.8 s**, and n+1 ≤ the
     maximum (D5).
   - **What it proposes:** n+1.
4. **Hysteresis.**
   - A shown hint stays until it is dismissed or applied, or until the median has stayed within
     0.3–0.9 s for 10 steps.
   - **Dismissed:** not shown again in this run unless the time leaves the band by a further
     factor of 1.5 (> 1.8 s or < 0.13 s).
   - **Never back to a level measured outside the band:** the hint does not propose a level that
     was measured outside the band with the same other parameters. With the asymmetric factors,
     going 6 → 5 → 6 → … cannot happen: the hint proposes 6 only if 5 runs below 0.2 s × 3, and
     5 only if 6 runs above 1.2 s.
   - No hint while the simulation is starting, after it has failed, or after it has finished.
5. **Late in a run (D7).** A restart throws away the simulated history. Proposed: in the first
   **50 steps**, the hint offers "Restart with N". Later the hint says that a restart loses the run so far, e.g. "Steps take 1.6 s; with 5
   subdivisions about 0.7 s (a restart starts over)", and shows the button less prominently.
6. **Where it appears.**
   - **Panel open:** the hint sits in the panel's status area, under "t = … Myr".
   - **Panel hidden:** a small egui `Area` right below the "⚙ Parameters" button (top-right).
   - Putting it in the panel avoids an off-screen Area on phones, where the panel fills the
     height.
   - Text, e.g.: "Steps take 1.6 s (target 0.25–1 s). With 5 subdivisions about 0.7 s.
     [Restart with 5] [×]".
7. **Applying the hint.**
   - It takes the arguments of the *running* simulation, sets `subdivisions` to N (pinned), and
     restarts through the existing restart path (`Viewer::ui` → `set_running` + `start`).
   - The panel's `subdivisions` field is set to N. Other pending edits in the panel stay pending
     and are *not* applied, so the panel then correctly shows only those as changed.
   - The URL then contains `subdivisions=N`.
8. **Interaction with other work.**
   - **#80 (start/restart banner):** the hint appears only after measured steps, so the two do
     not overlap in time. Keep the hint below the banner row if both are ever visible.
   - **#72 (cell-info widget):** keep it out of the top-right corner, or stack the two.

## D. Desktop
- **B and C go into the shared viewer and loop**, so they run natively too. The native default
  of 6 takes 0.36–0.65 s on tktie, so normally there is no hint. A slow laptop at 6, or 7
  anywhere, gets one.
- **A stays web-only.** Native keeps 6.

## Tests
- **Unit tests** (native):
  - the guess rule, from a struct of signals;
  - the window: 2 skipped steps, median, fast path, reset;
  - proposal and hysteresis on synthetic sequences: in-band, slow, fast, drifting across 1 s,
    flapping around a threshold, dismissed, late in the run;
  - the prediction table, and measured values overriding it;
  - argument and URL handling: guess vs pinned, "Defaults", and applying a hint with other edits
    pending.
- **egui test**, as for the panel in #61: the hint is visible with the panel open and with it
  hidden, and applying it returns a `Launch` with only `subdivisions` changed.
- **Headless Chromium:** `?subdivisions=7` (or DevTools CPU throttling) shows the slow hint, and
  3 shows the fast hint. A reload after accepting keeps the accepted value.

## Decisions for Sebastian
- **D1 — initial guess:** use the guess (A), or keep a fixed default of 5 and rely on the hint?
  *Recommended: use it.*
- **D2 — low-end desktop:** treat `deviceMemory` ≤ 4 (Chromium only) as 5? *Recommended: yes.*
- **D3 — URL:** write `subdivisions` into the URL only when it is pinned (from the URL, edited by
  the user, or a hint accepted), so that shared links stay adaptive to the device?
  *Recommended: yes.*
- **D4 — thresholds:** hint above 1.2 s or below 0.2 s, and aim for ≤ 0.8 s? Or aim for the
  middle of the band (~0.5 s)?
- **D5 — upper limit for "too fast":** at most 7 (estimated ×3.5–4.5 the time of 6, and ×4
  memory and upload), or at most 6? *Recommended: 7 natively, 6 on the web.*
- **D6 — remember an accepted level** in `localStorage` for the next visit? *Recommended: no;
  the URL carries it.*
- **D7 — late in a run:** offer the restart button only in the first ~50 steps and inform after
  that, or always offer it?
- **Side note:** 5 → 4 speeds a step up only ~1.4×, because the temperature model has a fixed
  cost on its coarse grid. Making that part cheaper would help phones more than fewer
  subdivisions. That would be a separate issue, if wanted.
