# Plan: orogenic weakness heals (#75); plume heating moves with the plate and fades (#53)

_2026-11-21. `main` at 536cecb (after PR #74). Goal: an Earth-like planet, not Earth; effects of
the ~110 km grid are accepted (#64) and named where they matter._

## What is wrong

- **#75.** A crust sample carries the share of its lithosphere deformed in an orogeny
  (`Crust::orogenic`, 0–1, from PR #74). The lithosphere's strength is
  `1 − (1 − SUTURE_WEAKENING) · orogenic` with `SUTURE_WEAKENING = 0.5` "for good". Collisions,
  sutures and accretion keep marking crust, nothing removes the mark: after 1000 Myr ~47% of
  the continental area is orogenic (mean share), still rising ~3 points per 100 Myr (batch-4
  results). In 2–3 Gyr runs the belt/craton contrast that makes rifts follow belts would
  vanish.
- **#53.** Above a plume head the lithosphere is weakened (`plume_head_strength`, a cap on the
  thermal factor: thermal age 25 Myr at the centre, `1 − (d/R)²` taper over R = 1000 km).
  This is a field around the plume's current mantle position, evaluated only when a plate is
  cut. It does not move with the plate, and it vanishes when the plume dies or the plate has
  moved on. Plume rifts happen at plume age 20 Myr (`rift_delay`); at ~5 cm/yr the plate has
  moved ~1000 km over the head by then.

## Common mechanism: a thermotectonic memory of the lithosphere, carried by the plate

Both are the same physics: a thermotectonic event (orogeny, plume) leaves the lithosphere hot
and thin; afterwards it cools by conduction and re-thickens. The memory lives on the crust
sample (plate frame), so it moves with the plate, and it decays with the conductive cooling
time of the lithosphere that has to be rebuilt, `τ ≈ L² / (π² κ)` (first mode of the
conductive relaxation of a layer of thickness L; κ ≈ 1e-6 m²/s):

| lithosphere rebuilt | L | τ = L²/(π²κ) | used for |
|---|---|---|---|
| plate model (oceanic, and the existing continental thermal age) | ~125 km | ~50 Myr; full thickness after ~100 Myr | plume reheating, via the existing thermal-age law (`FULL_STRENGTH_AGE` = 100 Myr) |
| thick continental lithosphere below an orogen | ~250 km (cratonic thickness) | **~200 Myr** | healing of orogenic weakness |

The two time scales are not tuned; they follow from the two thicknesses. (Plate model: Parsons
& Sclater 1977, Stein & Stein 1992 — the code already saturates the thermal strength at
100 Myr. Continental: Hyndman, Currie & Mazzotti 2005, GSA Today 15(2): mobile belts / former
back-arcs are hot and thin (50–60 km lithosphere, Moho 800–900 °C) and cool conductively once
subduction stops; "most of the rapid decrease in heat flow appears to be in the several
hundred m.y. following the last thermotectonic event"; regions with a thermotectonic age
above ~300 Ma have 150–300 km thick lithosphere like cratons, and "Paleozoic stable areas are
only slightly warmer than Archean cratons and are estimated to be almost as strong"; e.g. the
Appalachians, last deformed ~300 Ma, are "now cool and stable". Their conductive model
relaxes a 50 km lithosphere towards 250 km.)

### 1. Orogenic weakness heals (#75)

`Crust::orogenic` becomes the **remaining orogenic weakening** (0–1): set to 1 by an orogeny as
now, mixed by mass as now, and multiplied by `exp(−dt / OROGENIC_HEALING_TIME)` every step,
`OROGENIC_HEALING_TIME = 200 Myr`. Exponential decay is linear, so it commutes with the
mass-weighted mixing: a mixture of decayed samples equals the decayed mixture, and the
"orogenic mass" bookkeeping of PR #74 stays exact up to the decay. Strength unchanged in form:
`1 − (1 − SUTURE_WEAKENING) · orogenic`.

Consequences, by construction:
- A belt 200 Myr after its last orogeny has strength 1 − 0.5·e⁻¹ = 0.82; after 300 Myr 0.89;
  after 500 Myr 0.96 (Hyndman et al.: "almost as strong" after ~300 Myr). The Atlantic opening
  along ~200 Myr old Caledonian–Appalachian sutures (`SUTURE_WEAKENING`'s own example) is still
  favoured (0.82 vs 1 for unmarked crust, and much weaker than old crust while the belt is
  young).
- Steady state: with the marking rate of batch 4 (r ≈ 3–6% of the continental area per
  100 Myr) the mean remaining weakening tends to ~r·τ ≈ 6–12% of the continental area instead
  of growing without limit. The **ever-marked** area (`Crust::orogeny` set) still grows: on
  Earth too most continental crust outside the Archean cores has been reworked; that is not a
  weakness any more.
- `Crust::orogeny` (time of the last orogeny, max rule) stays as diagnostic.

Not modelled (named in the notes): the permanent part of orogenic inheritance (olivine fabric,
lithospheric scars that survive for > 1 Gyr: Tommasi & Vauchez 2001; Heron et al. 2016), and
the chemically depleted, buoyant keels of cratons, which no healing recreates (Proterozoic
belts have thinner lithosphere than Archean cratons even after > 1 Gyr: Artemieva & Mooney
2001; partly secular, the mantle that formed Archean keels was hotter). If the measurements
show that healing alone keeps the belt/craton contrast, no craton term is added (see decision
rule below).

### 2. Plume heating is a thermal-age reset in the plate frame (#53)

New field `Crust::reheated` (Myr; `f64::NEG_INFINITY` = never): the strength uses the thermal
age `time − max(birth, reheated)` — the lithosphere is as strong as lithosphere that formed at
`reheated`. This is #53's "thermal-age reset on the crust", but only for the strength:
`birth` (sea-floor age, thermal subsidence, buoyancy at trenches) is not touched, so
bathymetry, sea level and age statistics do not change through it. (The thermal-rejuvenation
swell model, Detrick & Crough 1978, would also raise the sea floor; heat-flow data argue for
less reheating, Von Herzen et al. 1989; the swell is already in the dynamic topography of the
mantle model.)

- Every step, for every live plume and every world cell within its heating radius, the
  surface sample's `reheated` is raised to `time − a(x)`, where `a(x)` is the thermal age
  whose strength equals today's cap: `a = FULL_STRENGTH_AGE · cap²`, with
  `cap = 1 − (1 − √(25/100)) · (1 − x²) · head_growth` (the same profile as today). A sample
  never gets colder by this (`max`).
- Afterwards the sample simply ages: from 25 Myr equivalent (cap 0.5) at the centre it
  regains full strength after 75 Myr, along the existing `√(age/100)` law. So the weak zone
  moves with the plate and fades over tens of Myr after the plate has passed or the plume has
  died, as #53 asks.
- **Head and tail.** The head stage heats the full head radius (1000 km). After the head stage
  the plume tail heats only a narrower swell: radius `PLUME_TAIL_RADIUS` = 500 km (hotspot
  swells such as Hawaii are ~1000–1200 km wide: Crough 1983; Wessel 1993). The head stage
  lasts `PLUME_HEAD_STAGE` = 20 Myr (head growth 10 Myr in the mantle model, plus spreading;
  flood basalts of plume heads erupt within a few Myr: Courtillot & Renne 2003, and the ponded
  head is sheared off by the plate within ~10–20 Myr). The plume rift (at 20 Myr) still finds
  the head-heated lithosphere, now where the plate has carried it, cooled by the time since.
  This keeps a slowly moving or long-lived plume from writing a 2000 km wide weak track for
  up to 200 Myr.
- `weak_zone_cut` drops the mantle-frame `plume_head_strength`; the strength comes only from
  the crust (`lithosphere_strength(crust, time)`). Cells above the plume right now carry the
  same cap as before (they were heated this step), so the first cut of a plume rift sees what
  it saw before, plus the trail.
- Mixing: `reheated` takes the latest (youngest) of the contributors where samples are mixed
  (`stretch_rift`, `heal_gap`), like `birth` in `heal_gap`; plate merges keep the receiver's,
  like `birth`. The max rule over-spreads a little, but the record fades in ≤ 75 Myr, so the
  over-spreading cannot accumulate (unlike the orogenic max rule of #40).
- Plume heating applies to oceanic and continental lithosphere alike, as the cap does today.

### Code

- `crust.rs`: `Crust::reheated`; `Crust::strength_age(time)`; doc of `orogenic` (remaining
  weakening); `Crust::heal(dt)` (orogenic decay).
- `weak_zones.rs`: `OROGENIC_HEALING_TIME`, `PLUME_TAIL_RADIUS`, `PLUME_HEAD_STAGE`;
  `lithosphere_strength(crust, time)` uses `strength_age`; `plume_head_strength` replaced by
  `Simulation::reheat_above_plumes(dt)` (per step, after the plate events, before the plume
  rifts of the next step) and a pure function `plume_heating_age(plumes, r) -> Option<f64>`
  for the profile (unit-testable). Per step cost: cells × plumes angle checks (~41k × ~10),
  negligible next to a step.
- `mod.rs::step`: call the healing (all samples of all plates, hidden ones included) and the
  reheating once per step.
- All `Crust { … }` literals get `reheated: f64::NEG_INFINITY`.
- Statistics: the rift statistic (`rift_suture_cells`) keeps "ever marked" for comparison with
  main and gets a weighted twin: sum of `orogenic` on rift-boundary cells and in the rifted
  plates (main: the same quantity, the share), and raw sums in the log so that windows can be
  computed from snapshot differences.
- Cell dump: columns `reheated` and `strength` (the lithosphere strength the cut uses), so the
  weak area can be measured directly.
- Web/wasm: nothing new there.

### Tests

- Healing: a marked sample's `orogenic` after `heal(dt)` repeatedly is `exp(−t/τ)`; mixing
  then healing equals healing then mixing.
- Plume: a plume's heating raises `reheated` only within the radius, with the profile of
  today's cap at the centre/half radius/edge; a sample moved away keeps its `reheated` and
  regains full strength after 75 Myr; after the head stage only the tail radius is heated; a
  sample is never cooled by heating.
- `lithosphere_strength` uses the younger of `birth` and `reheated`; the existing tests of
  rifts through the plume (`plume_rifts_run_through_the_plume`, `…near_a_plate_edge…`,
  `…arms_follow_sutures…`) pass after the plume's heating has been applied (they set plumes
  directly, so the tests call the reheating once before the split).
- Existing orogenic share tests keep passing (no decay within an event).

## Measurement

Runs (via `runs`): `main` (536cecb) and the branch, 8 seeds (0–7) × 1000 Myr, snapshots every
100 Myr, `--material-budget --cell-dump --width 512` (as batch 4); and seeds 0–3 × 3000 Myr,
snapshots every 250 Myr, both codes. Runtime: one local paired seed pair if the scheduler puts
the 1000 Myr runs on different machines.

Metrics (mean over seeds, seed range, Welch t vs main; 500–1000 Myr window for log metrics,
per snapshot for areas):
1. **Weak continental area over time** (cell dumps): mean remaining orogenic weakening; area
   with `orogenic ≥ 0.5` and ≥ 0.25; ever-marked area; area with strength < 0.75 from all
   causes; on the long runs up to 3000 Myr. Expected: bounded at ~5–15% on the branch, still
   growing on main.
2. **Rift-follows-belt contrast**: weighted twin of the rift statistic (mean `orogenic` on new
   rift boundaries vs in the rifted plates) and the old "ever marked" version, per window
   (0–1000, and 1000–2000, 2000–3000 Myr on the long runs). Expected: the ratio stays > 1.5 on
   the branch over 3 Gyr; on main the ever-marked ratio drifts towards 1 as marks saturate.
3. **Plume rifts**: number of plume rifts, `head_rifts` (boundary within half a head radius),
   rifts farther than 500 km (#52 counters). Expected: about unchanged (the cut at 20 Myr still
   sees the head heating, slightly cooled and displaced along the plate motion).
4. **Plume weak area** (cells with `reheated` > −∞ and strength cap < 1): trailing behind
   plumes, gone 75 Myr after the plume.
5. Guard rails with `compare.py` (batch-4 script, 69 metrics): plate count and speeds, trench/
   ridge length, sea-floor production and age, continental area, relief and hypsometry, land,
   initiations, sutures, rifts. Expected: small; the healing makes old belts stronger, so
   possibly fewer continental rifts/breakups and slightly older sea floor late in the runs.
6. Runtime (paired local).

**Decision rule for a craton term:** if on the 3000 Myr runs the weighted rift contrast stays
clearly above 1 (≥ 1.5) and the weak area is bounded, healing suffices and no craton term is
added. If the contrast still fades (e.g. because healed belts are as strong as the original
continent, so nothing distinguishes old cores), report the numbers and propose a keel term as
a decision for Sebastian rather than adding a new tuned strength.

## Risks

- Fewer plume rifts or a different rift geometry if the trail of heated lithosphere draws
  breaks away from the plume; #52's anchor still forces the break at the plume.
- Margin initiations use the same cut; old belts being stronger changes where margins break
  (intended).
- Hidden samples (overridden crust) also heal; fine.

## Interaction with #69 (deformable plate margins, branch `agent/issue-69`)

#69's band deformation will likely depend on lithosphere strength. The interface after this
change: **`weak_zones::lithosphere_strength(crust: &Crust, time: f64) -> f64`** (relative
strength 0.05–1 of the lithosphere under one crust sample) is the single place where the
thermal age, the plume reheating (`Crust::reheated`, via `Crust::strength_age`), the stretching
and the remaining orogenic weakening (`Crust::orogenic`) are combined. The mantle-frame
argument `plume_head` disappears (the plume's effect is already on the crust). #69 should call
this function rather than reading `orogenic`/`birth` itself; whichever PR lands second
rebases (the only conflict expected is the signature and the new `Crust` field in literals).
