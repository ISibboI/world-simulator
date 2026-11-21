# Issue #78: independent plan review (summary) and resolutions

An independent agent reviewed research.md and plan.md v1 against the code and the raw logs. It
re-ran both scripts. Its findings are listed by importance, each with how plan v2 resolves it.

1. **URL and defaults.**
   - Finding: the web default is a compile-time clap default (5). The panel drops values equal
     to it, and the worker query and the page URL share one string. So an inserted guess would
     leak into the URL on unrelated restarts, an accepted "5" would be dropped from the URL, and
     "Defaults" would reset to 5.
   - Resolution: the guess becomes the session's default (a panel default override). The worker
     query always carries `subdivisions`, and the page URL only when it is pinned (plan A.5).
2. **Panel state after applying a hint.**
   - Finding: if the hint restarts without updating the panel's field, the panel claims
     "Changed".
   - Resolution: the field is set to N and other pending edits stay pending (C.7).
3. **Prediction factors.**
   - Finding: one factor per pair cannot be cautious both ways. The tectonics grows more than ×4
     per level (substeps double), so 6 → 7 is likely ×3.5–4.5. At 3 the climate grid shrinks too.
   - Resolution: separate factors for going down and going up, and measured values win (C.1).
     D5 recommends at most 6 on the web.
4. **Drift mechanism.**
   - Finding: the drift is mostly the substep count, driven by the fastest plate and
     proportional to `dt`. Part of the late spread is machine load.
   - Resolution: research.md is corrected, and the memory of measured levels is keyed by all
     arguments except `subdivisions` (C.1).
5. **What is measured.**
   - Finding: `on_step` cannot time itself into its own message.
   - Resolution: the period between `on_step` calls, passed as `Option<Duration>` (B.1).
6. **Hidden tabs and frozen workers.**
   - Resolution: updates that arrive while the page is hidden are dropped, the window restarts
     on `visibilitychange`, and `Viewer::start` resets the window (B.3, B.4).
7. **Hint rule.**
   - Finding: the margins were asymmetric, there was no early verdict for very slow devices, and
     a decision was missing on hints late in a run.
   - Resolution: aim for ≤ 0.8 s both ways, a fast path after 3 steps over 2.5 s, and D7.
8. **Placement.**
   - Finding: an Area below an auto-sized panel can end up off-screen on phones.
   - Resolution: the hint sits inside the panel when it is open, and below the ⚙ button when it
     is hidden (C.6).
9. **Data caveats.**
   - Finding: the web timings include the surface, the post, the statistics and SwiftShader on
     the same cores, so ~2× is an upper bound on the wasm overhead. The p90 of 1.5× came only
     from native runs disturbed by load.
   - Resolution: noted in research.md, and the p90 claim was dropped.
10. **Browser API claims.**
    - Finding: some claims could not be re-verified (Chrome 152 `cpuPerformance`, the new
      `deviceMemory` values and status, Firefox 4/8).
    - Resolution: research.md flags them. The spec text of `cpuPerformance` was checked
      directly, and the plan does not depend on any of them. Blind spots added: "Request desktop
      site", and iPads with a trackpad.
