# Issue #78: choosing the subdivisions of the web version by CPU speed (research)

Request: can the browser tell how fast the CPU is for our single-threaded simulation, from
parameters it can read, without benchmarking? Later additions from Sebastian: measuring the real
step times "sounds good"; find how many steps the simulation needs to reach steady step time at
subdivisions 5 and 6; and independently show a hint that proposes a better number of subdivisions
to keep 1–4 steps per second (0.25–1 s per step).

The design that follows from this is in [plan.md](plan.md).

## 1. What the browser exposes

**Short answer:** no browser exposes the CPU model, clock rate or single-thread speed. All of
them leave it out on purpose because it is fingerprinting surface (see 1.8). What we can read
reliably in every browser is the **form factor** (phone or tablet vs desktop or laptop). That is
the best static proxy we have, because almost every desktop or laptop beats almost every phone
on single-thread speed. The exception is Apple: recent iPhones and iPads come close to desktop
speed.

Sources were read on 2026-11-21. A research subagent read most of the pages, and I spot-checked
the most surprising claim (the CPU Performance spec) myself. The independent plan review could
not re-verify the Chrome 152 release, the new `deviceMemory` values or its "Discontinued Draft"
status, or Firefox's 4/8 value. Follow the links before relying on them; nothing in the plan
depends on them. Items marked *(memory)* are general
knowledge that was not checked against a source.

| Signal | Chrome/Edge desktop | Chrome Android | Firefox | Safari macOS / iOS | In a worker? | Says about single-thread speed |
|---|---|---|---|---|---|---|
| `navigator.hardwareConcurrency` | real count | real count (phones usually 8) | real count; 4 or 8 with fingerprinting protection | **clamped to 4 or 8** (iPhone: 4) | yes | almost nothing (core count, not speed) |
| `navigator.deviceMemory` | yes; since ~147 values 2–32 | yes; ≤ 8 | no | no | yes | weak (≥ 16 GB → desktop) |
| `navigator.userAgentData` (`mobile`, `platform`; high entropy: `model`, `architecture`, `formFactors`) | yes (90+) | yes | no | no | yes | `mobile`/`formFactors`: form factor only |
| UA string | reduced (fixed OS and model strings) | reduced, model always `K`, but has `Mobile`/`Android` | has `Mobile`/`Android` | frozen Mac string; **iPadOS claims to be a Mac** | yes | form factor only |
| `navigator.cpuPerformance` (new) | Chrome 152 | Chrome 152 | no (neutral position) | no (WebKit opposed) | **no** (Window only) | weak: buckets of logical core count |
| WebGL unmasked renderer | full GPU string | full string (Adreno/Mali…) | sanitized ("…, or similar") | constant "Apple GPU" | via OffscreenCanvas | GPU, not CPU; Chrome only |
| WebGPU `GPUAdapter.info` | coarse vendor/architecture | coarse | partial | coarse | yes | GPU, not CPU |
| `(pointer: coarse)`, `(hover: none)`, screen size, DPR | yes | yes | yes | yes | **no** (main thread) | form factor only |
| `getBattery`, `navigator.connection`, Compute Pressure | Chromium only | partly | no | no | — | no (Compute Pressure reports load or thermal state, not speed) |

### 1.1 `navigator.hardwareConcurrency`
- **What it returns:** the number of logical processors. Browsers may report fewer.
  [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Navigator/hardwareConcurrency)
- **Safari:** clamps the value to **4 (fewer than 8 cores) or 8**.
  [WebKit source](https://github.com/WebKit/WebKit/blob/main/Source/WebCore/page/NavigatorBase.cpp),
  [bug 233381](https://bugs.webkit.org/show_bug.cgi?id=233381)
- **Firefox with resistFingerprinting:** reports a fixed value.
  [bug 1360039](https://bugzilla.mozilla.org/show_bug.cgi?id=1360039)
- **Relation to our question:** none useful. A budget Android phone has 8 cores, a desktop i5
  may have 4, and the simulation uses one core.

### 1.2 `navigator.deviceMemory`
- **What it returns:** approximate RAM in GiB, rounded and clamped.
- **Support:** Chromium only, secure contexts only.
- **Spec status:** the spec was marked a *Discontinued Draft* in October 2026. Recent Chrome
  versions report 2–32 on desktop and ≤ 8 on Android (the sources disagree on the exact
  milestone).
  [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Navigator/deviceMemory),
  [W3C](https://www.w3.org/TR/device-memory/),
  [blink-dev](https://groups.google.com/a/chromium.org/g/blink-dev/c/vXtAmrVZeDk)
- **Relation to our question:** a value of ≤ 4 on a desktop browser suggests a low-end laptop or
  Chromebook. A value of ≥ 16 suggests a desktop.

### 1.3 User-Agent Client Hints and the UA string
- **`navigator.userAgentData`:** Chromium only.
  - `mobile` is a low-entropy value and is available right away.
  - `model`, `architecture` and `formFactors` need `getHighEntropyValues()`.
  - [MDN](https://developer.mozilla.org/en-US/docs/Web/API/User-Agent_Client_Hints_API),
    [Chrome docs](https://developer.chrome.com/docs/privacy-security/user-agent-client-hints)
- **UA string reduction:**
  - **Chrome** freezes the OS version and replaces the Android model with `K`, but keeps the
    `Mobile` token. [Chromium](https://www.chromium.org/updates/ua-reduction/)
  - **Safari** sends a frozen macOS string, and iPadOS sends the *Mac* string by default.
    [WebKit bug 241322](https://bugs.webkit.org/show_bug.cgi?id=241322)
  - The common workaround for iPads: a Mac UA with `navigator.maxTouchPoints > 1` is an iPad
    *(memory, community practice)*.
- **Using the model for speed:** the Android `model` could be looked up in a SoC table, but we
  would have to maintain that table, and it would only work in Chrome. Not worth it.

### 1.4 `navigator.cpuPerformance`: the one CPU "tier" API (new in 2026)
- **What it returns:** a tier from 0 (unknown) to 4.
- **How the tier is computed:** mainly from the **logical core count**:
  - 1 core → tier 1
  - 2–4 cores → tier 2
  - 5–10 cores → tier 3
  - more than 10 cores → tier 4
  - Browsers may move a known CPU model up or down by one tier.
- **Where it works:** `Exposed=Window`, secure contexts only, so the worker cannot read it.
- **Sources:**
  [spec draft, 15 Aug 2026](https://wicg.github.io/cpu-performance/) (checked),
  [explainer](https://github.com/WICG/cpu-performance)
- **Status:**
  - Shipped in [Chrome 152](https://developer.chrome.com/release-notes/152).
  - WebKit [opposes it](https://github.com/WebKit/standards-positions/issues/622): fingerprinting,
    and a static value conflicts with dynamic power management.
  - Mozilla's position is [neutral](https://github.com/mozilla/standards-positions/issues/1364).
- **Relation to our question:** an 8-core phone lands in tier 3 with most desktops, so this is
  weak too. At most it could act as a tie-breaker in Chrome.

### 1.5 GPU strings (WebGL `WEBGL_debug_renderer_info`, WebGPU `GPUAdapter.info`)
- **What they tell us:** in Chrome, an Adreno or Mali renderer string means a phone.
  - [MDN WEBGL_debug_renderer_info](https://developer.mozilla.org/en-US/docs/Web/API/WEBGL_debug_renderer_info),
    [MDN GPUAdapterInfo](https://developer.mozilla.org/en-US/docs/Web/API/GPUAdapterInfo)
- **Limits in other browsers:** Firefox sanitizes the string
  ([bug 1722113](https://bugzilla.mozilla.org/show_bug.cgi?id=1722113)), and Safari always says
  "Apple GPU".
- **How projects use it:** [pmndrs/detect-gpu](https://github.com/pmndrs/detect-gpu) builds GPU
  tiers from this string. It measures the GPU, not CPU single-thread speed, and adds nothing
  over the form factor.

### 1.6 Pointer, hover, screen
- **What they tell us:** `(pointer: coarse)` together with `(hover: none)` and a small screen is
  a strong phone signal in every browser.
  [MDN pointer](https://developer.mozilla.org/en-US/docs/Web/CSS/@media/pointer)
- **Where it already runs:** `src/web.rs` evaluates `(pointer: coarse)` on the page for the UI
  size (#61).
- **Limits:** touch laptops may report `fine`, because the primary pointer is the trackpad.

### 1.7 Battery, Compute Pressure, network
- **`getBattery` and `navigator.connection`:** Chromium only, and unrelated to CPU speed.
- **Compute Pressure (`PressureObserver`):** Chrome desktop only. It reports load or thermal
  states (nominal … critical), not device class.
  [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Compute_Pressure_API)
- **Relation to our question:** none for the initial guess. Compute Pressure would at most tell us
  that a measured slow step was caused by throttling.

### 1.8 Why the CPU is not exposed
- **Fingerprinting:** the CPU model or clock rate adds identifying bits for every site, without
  any permission prompt. The W3C
  [Mitigating Browser Fingerprinting](https://www.w3.org/TR/fingerprinting-guidance/) note asks
  spec authors for coarse categories and opt-in exposure. The same note warns that timing
  channels leak hardware details anyway.
- **Precedent:** Safari clamps the core count, Firefox sanitizes GPU strings, and Chrome reduced
  its UA string, all for the same reason.
- **Proposals:** `cpuPerformance` is the only "device tier" proposal that shipped, and even it
  deliberately avoids clock speed. Chrome's Privacy Budget idea is archived
  ([repo](https://github.com/mikewest/privacy-budget)).
- **Static signals are weak by design:** WebKit's objection includes that a static value
  conflicts with dynamic power management. The practical conclusion is to measure the real
  workload, which is the step-time measurement Sebastian liked.

### 1.9 How well static signals predict our step time
| Signal | Correlation with single-thread speed | Failure cases |
|---|---|---|
| form factor (phone/tablet vs desktop) | good on average | iPhone/iPad Pro with A17/M-series ≈ desktop speed (would get 5, could do 6); low-end laptops, Chromebooks, old desktops (would get 6, too slow) |
| `deviceMemory` ≤ 4 (Chrome desktop) | moderate: catches many low-end laptops and Chromebooks | Chrome only; a fast machine with little RAM is rare |
| `hardwareConcurrency`, `cpuPerformance` | weak | phones have many cores |
| GPU string | weak for CPU | Chrome only |
| phone rule blind spots | — | Android "Request desktop site" and iPads with a trackpad (fine pointer, Mac UA, but `maxTouchPoints` > 1 still catches most iPads) look like desktops |
| unknowable | — | thermal throttling, power-saving mode, background tabs, other load. Only a measurement sees these |

Conclusion: a static guess can only choose between two classes (phone → 5, desktop → 6). The
measured step time has to correct it.

## 2. Measured step times

### 2.1 Method
- **Code:** main at 5c07b8a (#61 merged). Runs were on tktie (x86) while other jobs were
  running, so single steps are noisy and the medians are what counts.
- **Native runs:** `runs submit` with `RUST_LOG=debug`, seeds 0 and 1, 500 Myr, `dt` 1 Myr.
  `Planet::step` logs `tectonics_time`, `climate_time` and `erosion_time` for every step.
  Analysed with [scripts/native_step_times.py](scripts/native_step_times.py).
- **Web runs:**
  - Build: the trunk release build of the #72 branch (same simulation code), served locally and
    opened in headless Chromium (SwiftShader WebGL, `cores run 2`).
  - Settings: `?snapshot-interval=1`, so the worker logs one statistics line per step. Chromium's
    stderr log stamps each line, and the gap between two lines is one step plus the surface
    publish. This is what the user experiences.
  - Statistics overhead: negligible. At subdivisions 4 the run took 261 ms per step with the
    statistics line and 265 ms without.
  - Analysed with [scripts/web_step_times.py](scripts/web_step_times.py).

### 2.2 Native, median ms per step (tectonics / climate / erosion)
| steps (Myr) | sub 4 seed 1 | sub 5 seed 0 | sub 5 seed 1 | sub 6 seed 0 | sub 6 seed 1 |
|---|---|---|---|---|---|
| 1–3 | 122 | 172 (24/126/22) | 168 | 364 (66/180/107) | 359 |
| 4–10 | 127 | 174 | 175 | 371 | 366 |
| 11–20 | 132 | 178 | 173 | 367 | 347 |
| 21–50 | 128 | 173 | 178 | 377 | 357 |
| 51–100 | 132 | 178 | 175 | 386 | 376 |
| 101–200 | 136 | 192 | 184 | 413 | 523 |
| 201–300 | 137 | 192 | 266 | 584 (187/220/101) | 602 |
| 301–400 | 139 | 187 | 291 | 638 | 491 |
| 401–500 | 139 | 188 | 318 | 485 | 458 |

Sub 4 seed 0 was disturbed by load on the machine (its climate times were 1.5× those of seed 1
at the same work), so it is left out.

### 2.3 Web (headless Chromium on tktie), median ms per step
| steps (Myr) | sub 4 | sub 5 | sub 6 |
|---|---|---|---|
| 1 | 320 | 580 | 976 |
| 2–3 | 290 | 378 | 738 |
| 4–10 | 265 | 379 | 731 |
| 11–20 | 260 | 372 | 754 |
| 21–50 | 260 | 387 | 776 |
| 51–100 | — | 385 | 802 |
| 101–150 | — | 391 | 815 |
| 151–200 | — | 442 | — |

Whole-run averages without the statistics line: 265 ms per step at 4 and 350 ms at 5 (60 Myr
each).

### 2.4 Findings
1. **There is no short warm-up with fast steps.**
   - In the browser, the **first step is the slowest** (1.3–1.5× the following ones: the wasm
     tier-up and the first publish with the triangles).
   - From step 2 on, the step time sits at its base level for about the first 100 Myr.
2. **What Sebastian saw as "a bit faster in the first steps" is a slow drift, not a warm-up.**
   - The main cause is the **number of tectonic substeps**. It is set by the fastest plate:
     `substeps = ceil(max plate speed × dt / (max_substep_movement × mean edge angle))`, in
     `src/tectonics/mod.rs`.
   - As plates speed up and reorganise, a step gets more substeps. That makes the drift jumpy,
     dependent on the seed, and proportional to `dt`.
   - At subdivisions 6, the tectonics time grows from ~65 ms to 180–250 ms after 100–300 Myr.
     The total step time grows **×1.3–1.7**:
     - natively 360 → 450–640 ms;
     - on the web 735 → 815 ms by 150 Myr.
   - At 5 the growth is smaller: +10% for seed 0. Seed 1 shows +80% after 200 Myr, but its
     climate time rose too, which points to load on the machine. Part of the late spread is
     noise from that load.
   - The drift does not settle at a fixed step.
   - So the measurement should skip **1–2 steps** and then keep tracking a moving median, not
     wait for a steady state.
3. **The step time does not grow ×4 per subdivision level.**
   - The climate part takes ~110 / 125 / 180 ms at 4 / 5 / 6, early in the run. That is far
     less than ×4 per level. The reason: the temperature model runs on its own fixed coarse icosphere
     (`coarse_subdivisions: 4`, 2562 cells, in `src/temperature.rs`), whatever the
     subdivisions of the planet, as long as the planet has at least 4 (the code takes the
     smaller of the two, so at 3 the climate grid shrinks too).
   - The tectonics part grows faster than ×4 per level: 5 → 6 is ×2.75 early and ×4–5 late.
     The reason is that the substep count doubles per level, because the edges are half as
     long, on top of 4× the cells. An estimate for 6 → 7 from the parts gives ×3.4 early and
     ×4.5 late. The Newton iterations of the water cycle are the same (~4.5) at 4, 5
     and 6.
   - Measured factors, early in the run:

     | | native | web |
     |---|---|---|
     | 4 → 5 | ×1.35 | ×1.4 |
     | 5 → 6 | ×2.1 | ×2.0 |

     Late in the run, 5 → 6 reaches ×2.5–3.
   - **Going from 5 down to 4 only buys ~1.35×.** A phone that is too slow at 5 is probably
     still too slow at 4. (The fixed climate cost is a possible optimisation target; that is not
     part of this issue.)
4. **The web is 2.0–2.2× slower than native here** (wasm in Chromium on the same x86 machine).
   #60 measured 2.4× with older code.
   - This is an upper bound on the wasm overhead.
   - The web numbers also include building and posting the surface and the statistics line.
     SwiftShader rendering ran on the same 2 reserved cores.
   - The native numbers are `Planet::step` alone.
5. **Implications for the 0.25–1 s band:**
   - On this machine, the web runs at **~0.38 s at 5** and **~0.75–0.8 s at 6**. Both are in the
     band, which matches "6 runs nicely on desktop".
   - A phone that is 2× slower per thread would need ~0.8 s at 5 and ~1.5 s at 6, which matches
     "5 is the limit on mobile".

## 3. Does the hint make sense natively?
- **Native step times:** about 0.36–0.65 s at the default 6 here, so a fast desktop sits inside
  the band and gets no hint. A slow laptop (or a debug build) at 6 may exceed 1 s, and 7 on a
  fast desktop surely will (~×2.5).
- **Cost:** the hint lives in the shared viewer and the measurement in the shared simulation
  loop, so enabling it on both targets costs nothing extra, and it is useful there too.
- **What stays web-only:** the initial guess. Native keeps the default 6.
