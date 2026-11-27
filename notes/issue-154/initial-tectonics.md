# #154: a more realistic initial tectonic state instead of a spin-up before t = 0

Sebastian on PR #196: "the spin-up idea is nice, but can we instead change initial
generation of the planet's tectonics to be more realistic?"

**Short answer: yes.** The degassing ramp has four causes. None of them is "plates at
rest": the plate velocities are a quasi-static force balance and reach ~3 cm/yr in the first
step. The causes are stocks and populations that the generator starts at zero or far from
their steady state:

1. too few plates;
2. no plumes;
3. an empty mantle memory of slabs;
4. a boundary/age field that is not consistent with the motion.

A generator that starts each of these at its steady state removes most of the ramp. In
diagnostic runs, the first, cheap part of it (more plates) already brings the degassing to
its steady level after 40–60 Myr instead of ~150–200 Myr. Recommendation and effort are in
§3.

Project goal: an Earth-like planet, not Earth. Targets are the model's own steady state,
measured in the 8-seed, 1000-Myr runs of #154 at 300–1000 Myr:

- 27 plates;
- mean speed 4.0 cm/yr;
- mean ocean age 54 Myr;
- ~9 plumes;
- trenches 111 000 km, ridges 102 000 km.

## 1. How t = 0 is generated today (`Simulation::new`, `generation.rs`, `mantle.rs`)

| item | at t = 0 | steady state | how it gets there |
|---|---|---|---|
| plates | `--plates` = **12** (noise-shaped partition, continents cohesive) | **27** (20–35) | Subduction initiation at old margins and plume rifts. Both need time: old enough crust, and plumes, which start at zero. In the base runs: 12 → 15 at 50 Myr, ~20 at 100–150 Myr, 25 at ~200 Myr. |
| plate velocities | 0 until the first solve | 4.0 cm/yr | **Not a spin-up.** The torque balance is quasi-static (no inertia), so step 1 already gives ~3 cm/yr. "Mean speed 0.0" in the t = 0 log line only means it is not solved yet. |
| boundaries | 50% of plate pairs become trenches, at random (`initial_subduction_fraction`); the denser side subducts. The rest have no type until the first solve. | trenches ≈ ridges ≈ 100 000 km | Sorted out in the first step, but the random typing does not match the motion (see ages). |
| slabs | Full-length slabs (600 km) at the initial trenches | — | Already steady: slab pull at t = 0 is even above its later mean. |
| ocean ages | Earth's triangular age–area distribution (max 180, mean 60 Myr), by distance from the non-trench boundaries | mean 54 Myr | The distribution assumes steady spreading at the generated boundaries. These are not real ridges of the solved motion, and spreading is slow at first, so the mean age **rises** to 70–75 Myr at 50–100 Myr and only then falls. |
| mantle downwellings (return flow towards slab graveyards) | **empty** | filled by the trenches with a 100-Myr memory | e-folding 100 Myr: 78% after 150 Myr. It drives the plates through basal drag. |
| sunken slab masses (dynamic topography) | **empty** | residence 13 / 45 / 67 Myr per depth stage | ~125 Myr to fill. |
| plumes | **none** | ~9 | Born every 15 Myr, live 80–200 Myr, so ~150 Myr to reach the steady population. Plume rifts (new plates) wait for them. |
| sediment burial / metamorphism | Initial sediment cover, but no deeply buried sediment | metamorphic source ~1.2 Tmol/yr | Builds up over ~250 Myr. |
| continents | uniform crust (`birth` −1000 Myr), no sutures, no orogenic belts | — | Weak zones for rifting appear only with collisions. |

### Degassing by source (Tmol C/yr, 8-seed means of the #154 runs, cycle on)

| t (Myr) | total | ridges | arcs* | plumes | metamorphic |
|---|---|---|---|---|---|
| 2–5 | 5.2 | 1.7 | 3.3 | 0.0 | 0.1 |
| 10 | 6.0 | 2.1 | 3.7 | 0.0 | 0.2 |
| 50 | 8.5 | 2.9 | 5.1 | 0.3 | 0.3 |
| 100 | 10.8 | 3.6 | 6.2 | 0.6 | 0.5 |
| 150 | 13.3 | 4.0 | 7.8 | 0.9 | 0.6 |
| 200 | 14.0 | 4.2 | 8.1 | 1.0 | 0.7 |
| 300–1000 | 14–15 | 4.0 | 8.3 | 0.9 | 1.2–1.6 |

\* From 150 Myr on, arcs include the return of the seafloor's carbon. Before that it is not
counted; compare ridges for the tectonic rate.

The ridge source tracks the new sea floor. It is a pure tectonic rate, and it doubles
from 2.0 to 4.0 Tmol/yr over ~150 Myr.

## 2. Diagnostic runs: which cause dominates?

These are quick local checks:

- seeds 0 and 3, 120 Myr, default resolution, one core each;
- a diagnostic build that only makes the mantle's slab memory settable;
- outputs in `agent-scratch/spinup/` of the worktree.

The table gives the mean degassing (Tmol/yr) per 20-Myr window, and in brackets the
ridge part (the new sea floor).

| variant | 0–20 | 20–40 | 40–60 | 60–80 | 80–100 | 100–120 | plates at 120 | ocean age at 120 |
|---|---|---|---|---|---|---|---|---|
| base (12 plates) | 5.9 (2.0) | 7.9 (2.6) | 8.6 (2.8) | 9.5 (3.1) | 11.1 (3.5) | 11.6 (3.7) | 20 | 51–72 |
| slab memory 10 Myr instead of 100 | 6.3 (2.1) | 8.6 (3.0) | 9.0 (3.0) | 9.0 (2.9) | 10.0 (3.1) | 11.2 (3.4) | 16–19 | 59–66 |
| 18 plates | 6.7 (2.4) | 8.2 (2.9) | 9.7 (3.4) | 11.0 (3.7) | 13.3 (4.2) | 15.3 (4.4) | 28–32 | 41–64 |
| 24 plates | 8.1 (2.9) | 10.6 (3.8) | 12.7 (4.3) | 13.3 (4.4) | 14.7 (4.8) | 14.7 (4.7) | 33–36 | 40–43 |
| 24 plates + memory 10 Myr | 8.8 (3.2) | 11.8 (4.1) | 12.4 (4.2) | 13.9 (4.6) | 13.6 (4.5) | 13.4 (4.5) | 27–30 | 39–52 |
| steady state (base, 300–1000 Myr) | 14–15 (4.0) | | | | | | 27 | 54 |

**What this shows:**

- **The plate count is the main lever.** With 24 plates the degassing reaches the steady
  ~13–15 Tmol/yr after 40–60 Myr instead of ~150–200.
  - 24 plates then overshoots to 33–36 plates with a 40-Myr mean ocean age: the new plates
    form on top of an already full set.
  - So the start should have about the steady count, *with* steady sizes and ages, not just
    a higher `--plates`.
  - The first 20 Myr still give only ~60% of the steady degassing. The other causes below
    account for that.
- **The mantle memory matters less.** A 10× faster memory adds ~0.5 cm/yr in the first
  60 Myr (3.7 vs 3.1 cm/yr) and ~0.5–1 Tmol/yr. It does not remove the ramp on its own.
- **Plumes** (0 → 0.9 Tmol/yr) and **metamorphism** (0.1 → 1.2 Tmol/yr over ~250 Myr) are
  smaller, slower stocks, each 5–10% of the total.
- **The ocean-age field is not steady:**
  - its mean rises from 60 to 70–75 Myr in the base runs, because the generated
    "ridges" are not the spreading ridges of the solved motion;
  - with more plates it falls to ~40 Myr.

## 3. Options for a steady-state initial generation

Each option starts something at t = 0 at its steady state instead of empty. They are
independent and can be combined. Effort is for one subagent task, including runs.

| | option | effort | risk | what it fixes |
|---|---|---|---|---|
| A | **Plate count and size distribution near the model's steady state**: ~25 plates with a heavy-tailed size distribution (a few large plates, many small ones; Bird 2003), instead of 12 similar-sized ones. The default `--plates`, plus the partition in `generation.rs`. | low | Low–medium. Plate count and age must be checked against the steady state (24 overshoots); the supercontinent preset needs its own check. | **The largest part of the ramp**: ridge and arc degassing at their steady level in ~40–60 Myr instead of ~150. |
| B | **Solve the velocities at t = 0, then type the boundaries and ages by them**: <ul><li>keep the initial slabs and trenches;</li><li>solve the torque balance;</li><li>make diverging boundaries ridges and converging ones trenches (older/denser side down) or collisions;</li><li>repeat 2–3 times;</li><li>give the sea floor ages from distance to the real ridges ÷ the solved half-spreading rate, capped at ~180–200 Myr and rescaled to the steady age–area distribution;</li><li>thermal subsidence and ridge push follow from the ages.</li></ul> | medium–high | Medium. Iteration convergence; consistency at trenches; very old crust where spreading is slow; elevation and sea level change at t = 0. | Ridge push and slab pull consistent with the motion from the first step; no ocean-age overshoot (60 → 72 Myr); ridge length right from step 1. The force-balanced start Sebastian asked about. |
| C | **Initial plume population**: draw ~`lifetime/interval` ≈ 9 plumes at random positions away from the initial slabs, with ages uniform in their lifetimes. | low | Low. A few plume rifts in the first Myr; swells appear at t = 0. | The plume source (~1 Tmol/yr) and plume rifting (plate births) from the start. |
| D | **Pre-filled mantle memory**: fill the downwellings with each initial trench's subduction rate (from B's or the first solve's velocities). Fill each slab stage with mass flux × residence time, as if the trenches had been active for ≥ 150 Myr. | low | Low. Dynamic-topography lows appear above the initial slabs at t = 0, which is realistic. | Mantle-driven plate motion and dynamic topography from the start: ~+0.5 cm/yr early on. |
| E | **Crust and sediment consistent with history**: <ul><li>old sutures and belts as weak zones inside continents (so rifting works early);</li><li>buried metasediment in thick basins (metamorphic source);</li><li>trench and accretionary sediment.</li></ul> | medium | Medium. Touches mass budgets and the sediment stocks of #190. | The slow metamorphic ramp (~250 Myr) and early continental rifting. A smaller share of the degassing. |
| F | (the earlier idea) **Spin up the tectonics before t = 0** with the sun's clock stopped, e.g. 150–200 Myr of tectonics with erosion. | low–medium | <ul><li>Runtime: +30–40% for a 500-Myr run.</li><li>The generated planet the viewer shows is no longer the start: continents have moved; a supercontinent start is no longer one.</li><li>The start depends on the spin-up's length.</li></ul> | Everything at once, but by brute force. Sebastian prefers a realistic generator. |

**Recommendation:**

1. **First do A + C + D, plus the t = 0 solve from B**, as one cheap issue: "steady-state
   initial tectonics".
   - A is the measured main lever; C and D take out most of the rest.
   - Each is a small change in `Simulation::new` / `MantleFlow::new`, with the 8-seed
     comparison of the degassing, plate count and ocean age over the first 200 Myr against
     the steady state.
   - Expected: degassing within ~10% of its steady level after ~20–40 Myr.
2. **Then do B in full** (motion-consistent boundaries and ages) as the second step: it is
   the physically right start, with more work and risk.
3. **E only if the metamorphic ramp still shows.**
4. **With A + C + D the 150-Myr window of the carbon cycle can shrink to ~30–50 Myr.**
   - Calibrate φ over its last 10–20 Myr.
   - The post-window CO₂ rise of 0.49 → 0.55 bar goes away with the ramp.
   - That is a small follow-up change in #196's parameters once the new start is in; #196
     needs no change now.

**Where #208 (epoch-dependent planet) fits.** #208 changes *what* the steady state is:

- less continental crust;
- a hotter mantle, perhaps with faster plates and more plumes;
- a shorter day.

This proposal makes the start *consistent with* whatever the steady state is. If the
generator takes its targets from the same parameters as the dynamics (plate count from the
steady breakup/initiation balance, plume count = lifetime/interval, the age distribution from
the spreading rate), an epoch-dependent mantle from #208 changes the initial state
automatically. Do this first (it is needed at every epoch, also for `--epoch 4`), then #208
on top. The 4.0-Ga continental fraction of #208 also changes the plate partition. A should
therefore be written for any continental fraction, not tuned to 40%.

## Notes on the method

- These runs are 2 seeds × 120 Myr each: enough to rank the causes, not to calibrate. The
  steady-state values are from the 8-seed #154 runs.
- The diagnostic build only added `mantle.slab_memory` to `--set`; nothing of it is
  committed.
