# Issue #32: sea-floor production ~1.7–2× Earth's — investigation

_2026-11-20, `main` at cd940a0 (after #39). Investigation only, no physics changed._

## Verdict

The excess is **real, not a measurement artefact**. With the same convention as Earth's
plate-boundary model (all diverging boundaries, both flanks counted once), the model makes
**6.0–6.2 km²/yr** of sea floor against **3.4–3.5 km²/yr** for Earth (PB2002, Bird 2003;
~3.1 km²/yr without continental rifts): **~1.8×**. An estimate that does not use the boundary
geometry at all (area of sea floor younger than 5 Myr, divided by 5 Myr) gives the same
5.9 km²/yr.

The total plate-boundary length is Earth's (~255–260 thousand km in both). The excess comes from
**how that length is used**: in the model ~87% of it is ridge or trench (Earth 46%), and plates
move about **2× faster relative to their neighbours** (median 7.0 vs 3.0 cm/yr), while their
absolute speeds are Earth-like (mean 3.8 cm/yr).

## Data

- `main` (pre-#39 build, physics of plate motion unchanged by #39), 8 seeds × 1000 Myr with cell
  dumps (runs of the relief-5-7 task); 500–1000 Myr window.
- `main` + a logging-only diagnostic (production and consumption split by boundary kind,
  rate and plate size), seeds 0–3 × 1000 Myr; 400–1000 Myr window.
- Sensitivity runs (diagnostic build, `--set`), seeds 0–1 × 600 Myr; 300–600 Myr window.
- An independent agent reviewed the draft against code and data and reproduced the PB2002
  values; its corrections (young-crust consumption, ranking of C1 vs C2/C3, C2 code description,
  normals) are incorporated.
- Earth: PB2002 (Bird 2003) `PB2002_steps.dat` (5819 boundary steps with class, length,
  relative velocity and its divergent component) and `PB2002_plates.dig` (plate outlines, for
  areas), evaluated directly.

## 1. Is the metric right?

`sea floor production (km²/yr): kinematic` (`Simulation::step`, `tectonics/mod.rs`) sums the
opening rate (boundary-normal relative velocity with the raw per-cell normal, i.e. the summed
directions to the other plate's neighbours) over all diverging boundary cells of **both** sides
and halves it (a triple-junction cell counts once per other plate); length per cell = R × mean edge angle / 1.1 (zig-zag
factor). That is full spreading rate × ridge length, the same convention as Bird's 0.108 m²/s.
Checks:

| check | value | conclusion |
|---|---|---|
| units (km × km/yr) and both flanks | halved per side ✓ | correct |
| ledger `new sea floor` / (7 km × 2900 kg/m³) | 136 Gt/yr ↔ 6.7 km²/yr = `created` ✓ | consistent |
| `created` (cells actually filled with new sea floor) vs kinematic | 6.78 vs 6.16 (+10%) | small numerical excess (gap filling, #2 finding 1b/1m, #6) — on top of the kinematic excess, not its cause |
| young sea floor in the cell dumps: area < 2 / 5 / 10 / 20 Myr ÷ age | 6.4 / 6.0 / 5.6 / 5.1 km²/yr | independent of boundary geometry; confirms ~6 |
| continental opening included | 0.15–0.18 km²/yr (3%) | Earth's PB2002 total also includes continental rifts (0.36) |
| total boundary length (cell dumps, both sides) | 262 thousand km | Earth 260 thousand km |

Only the label could be sharper: the logged "ridges" length counts only `Ridge` cells (young
oceanic crust), while production counts all diverging cells (8% of production is at `Other`
cells: new rifts in older ocean, oblique boundaries). Not a bug. A possible small bias: the
filter `convergence < 0` is per cell, so noisy per-cell normals on the zig-zag boundary can turn
oblique or strike-slip motion into spurious opening (the diverging `Other` cells open at only
~2.5 cm/yr). The age-based estimate and the ledger bound this to a few percent.

## 2. Model vs Earth

| quantity | model | Earth | ratio |
|---|---|---|---|
| sea-floor production (km²/yr) | 6.0–6.2 (seed means 5.2–7.2) | 3.4–3.5 (PB2002; Parsons 1982 3.45; Rowley 2002 ~3.4); 3.0–3.1 sea floor only | 1.8–2.0 |
| total boundary length (10³ km) | 254–262 | 260 | 1.0 |
| trench (subduction) length (10³ km) | 123 | 51 (SUB) | **2.4** |
| ridge length (10³ km) | 93 | 67 (OSR) | 1.4 |
| transform / other (10³ km) | 24 | 74 (OTF + CTF) | 0.3 |
| collision / diffuse convergent (10³ km) | 11 | 40 (CCB + OCB) | 0.3 |
| continental rift (10³ km) | ~0 as boundary (stretching) | 27 (CRB) | — |
| ridge + trench share of the boundary | 87% | 46% | |
| mean full spreading rate at ridges (cm/yr) | 5.9 | 4.5 | 1.3 |
| mean convergence at trenches (cm/yr) | 4.5 | 5.3 | 0.85 |
| relative speed across boundaries, median (cm/yr) | 7.0 | 3.0 | **2.3** |
| boundary length with relative speed < 2 cm/yr | 3–6% | 39% | |
| mean \|cos\| (relative motion vs boundary normal) | 0.71 | 0.64 | |
| length-weighted mean \|normal velocity\| (cm/yr) | ~4.7 | 2.7 | 1.75 |
| mean absolute speed (cm/yr; oceanic / continental cells) | 3.8 (5.3 / 1.9) | ~3.5–4 | ~1 |
| plates total / > 1% of the surface | 25 / 14 | 52 / 13 | |
| share of production with full rate < 2 / 2–5 / 5–8 / 8–12 / > 12 cm/yr | 4 / 20 / 30 / 33 / 13% | 14 / 25 / 31 / 15 / 15% | |
| share of production at boundaries whose smaller plate is < 1 / 1–3 / 3–10 / > 10% of the surface | 25 / 32 / 35 / 9% | 36 / 3 / 45 / 15% | |
| consumption at trenches / at collisions and diffuse boundaries | 95% / 5% | 78% / 19% | |
| mean sea-floor age (Myr) | 50–51 (all oceanic basement 56–57) | ~60–65 (Seton et al. 2020; Müller et al. 2008) | |
| mean sea-floor age without ≥ 200 Myr crust (Myr) | 40–41 | ~62 | |
| sea floor by age, % per 20 Myr (0–20, 20–40, …, 160–180, ≥ 180) | 36, 24, 14–15, 9, 5, 3, 2, 1.5, 1, 3.5–4 | ~21, 19, 16, 14, 11, 9, 6, 3, 1, 0 (triangular, Parsons 1982; Coltice et al. 2012) | |
| production consumed before 20 / 40 Myr (age–area density at 20 / 40 Myr: 4.1 / 2.6 km²/yr per Myr, vs 6.0–6.4 at 0) | 31–36% / 57–60% | ~11% / ~22% | ~3 |

Model numbers: 8 seeds × 500–1000 Myr (logs and cell dumps) and the 4 diagnostic seeds
(400–1000 Myr), which agree within seed scatter. Production = ½ × total boundary length ×
mean |normal relative velocity| (area is conserved): with Earth's boundary length, the whole
excess is in the mean normal velocity, i.e. in the partitioning of the boundary into fast ridges
and trenches.

## 3. Causes, ranked

The structural causes are C2 and C3 (how the boundary is partitioned); C1 is how they show up in
the kinematics; C4 is their effect on the age distribution.

### C1 = C3 seen kinematically. Neighbouring plates move too fast relative to each other

Ridge production alone is 5.55 km²/yr vs 3.01 on Earth's OSR: 1.84× = **1.39 (ridge length
93.7 vs 67.3 thousand km) × 1.32 (ridge rate 5.9 vs 4.5 cm/yr)**, so the excess splits about
evenly between too much ridge and too-fast ridges. Relative speed median 7.0 vs 3.0 cm/yr; only 3–6% of the boundary is slower than 2 cm/yr
(Earth 39%), while the absolute speeds are Earth-like. On Earth the slow boundaries are
between the large, slow, continent-bearing plates (Eurasia, Africa, North and South America,
Antarctica) and inside continents; the fast ones are around the oceanic plates. Here plates are
driven independently by their own trenches, so neighbours rarely move together. This was
already found on #2 (finding 1q: "the over-production is kinematic, not geometric … remaining
lever: the magnitude of the relative speeds", and "relative plate speed tuning (1q, later,
user)" in its open list). **Covered by #2** (open). Most of the 2.3× median relative speed comes
from the boundary partitioning (C2, C3: missing slow continental, transform and diffuse
boundaries, extra trenches), not from faster ridges, so C1 is largely the outcome of C2/C3. This
qualifies #2's "kinematic, not geometric": the total length is Earth's, but its partitioning
into boundary types is not.

### C2. Every converging oceanic contact becomes a trench with slab pull

`Polarities::contact` creates a segment with a polarity wherever two plates overlap, and
`boundary_cells` makes it a trench as soon as the plates converge faster than
`min_trench_convergence` = 0.1 cm/yr (an existing slab keeps it a trench until the plates
diverge faster than that). The new segment starts with `slab_length` 0, so its slab pull ramps
up with the slab length and is weak for young slabs; but there is no slow, diffuse, mostly
slab-less convergent oceanic boundary type (Earth's OCB, 17 thousand km, partly incipient
subduction such as Puysegur) and no resistance to starting subduction at an existing contact (on
Earth induced initiation needs ~100–150 km of forced convergence; Gurnis et al. 2004, Stern &
Gerya 2018). The explicit induced-initiation events in `events.rs` exist besides this implicit
path. Result: trench length 2.4× Earth's; consumption at trenches is 95% (Earth 78%), and
growing slabs pull their plates (feeds C1). Sensitivity: see §4. **No issue yet** (#2 discusses initiation at passive margins and induced
initiation at transform boundaries, but not this automatic trench formation).

### C3. Few slow / strike-slip and continental boundaries

Transform and other boundaries are 24 thousand km (Earth 74), collision 11 (Earth CCB + OCB 40),
continental-crust boundaries in total ~14 thousand km of continent–continent contact (Earth
CRB + CTF + CCB = 77). Sutures merge colliding plates and plate splits run mostly through ocean,
so continents carry almost no slow diffuse boundaries; the same boundary length is used by fast
ridges and trenches. **Partly covered**: #2 (finding 1q, ~90% ridge or trench vs Earth 45–50%,
transform friction). No issue on continental (diffuse) plate boundaries.

### C4. Too many trenches close to ridges: young sea floor is subducted

31–36% of new sea floor is subducted before 20 Myr and 57–60% before 40 Myr (Earth ~11% and
~22%); the age–area distribution is exponential rather than triangular. Same roots as C2
(trenches everywhere, short ridge–trench distances) and C1 (fast plates bring trenches to young
crust sooner). **Covered by #2** (finding 1n: "age distribution: young part exponential, not
triangular").

### Not causes (checked)

- Microplates: Earth makes 36% of its sea floor at boundaries of plates < 1% (Cocos, back-arc
  plates); the model 25%. Not an excess.
- Ridge push 3·10¹² N/m and slab pull 2·10¹³ N/m per metre are Earth values; the total slab pull
  (1.4·10²¹ N) is Earth-like because the model's trenches are 2.4× longer but subduct young,
  light slabs.
- The measurement (§1).

## 4. Sensitivity runs (seeds 0–1 × 600 Myr, means over 300–600 Myr)

Diagnostic build with three more parameters settable; one change at a time, 2 seeds, so only
large effects count (seed scatter of production ~±10%).

| run | production kinematic / created (km²/yr) | trench / ridge (10³ km) | relative speed median (cm/yr), < 2 cm/yr | mean speed (cm/yr) | plates (> 1%) | mean sea-floor age (Myr) |
|---|---|---|---|---|---|---|
| baseline (`main`) | 6.41 / 7.08 | 133 / 97 | 6.7, 3% | 4.1 | 26 (13) | 47 |
| `min_trench_convergence` 0.1 → 2 cm/yr | 5.92 / 6.47 | 135 / 94 | 6.7, 7% | 3.9 | 22 (15) | 54 |
| `mantle.return_flow_fraction` 0.5 → 0 | 6.95 / 7.36 | 160 / 112 | 6.1, 10% | 2.7 | 35 (18) | 52 |
| `slab_sinking_speed` 10 → 5 cm/yr | 4.85 / 5.52 | 141 / 107 | 4.8, 6% | 2.8 | 32 (15) | 57 |

- A higher convergence threshold for **new** trenches does not shorten the trenches: existing
  slabs keep a boundary a trench (hysteresis), and segments appear wherever crust overlaps. C2
  is about where trenches can exist at all, not about a rate threshold.
- Without return flow plates are slower in absolute terms (2.7 cm/yr) but production does not
  drop: more plates, longer boundaries. Absolute speed is not the lever.
- Halving the slab-driven terminal speed lowers production by ~25%, but it remains ~1.4× Earth,
  with even longer trenches and more plates: speed tuning alone does not fix the partitioning
  (and would slow the plates below Earth's).

## 5. Consequence for the mean age

Mean sea-floor age ≈ shape × ocean area / production. The model's shape factor is ~1.1
(exponential + ancient tail; Earth's triangular distribution gives ~0.65). Halving production
alone with today's shape would raise the mean age to ~90 Myr (too old); C2/C4 (fewer trenches
near ridges) also change the shape towards triangular, so production and shape must be fixed
together, and the ancient tail (#14) must be gone, for an Earth-like 60–65 Myr. #37 (slab
depressions too deep) waits on the subduction flux, i.e. on this issue.

## 6. Existing issues and proposed new ones

| cause | issue |
|---|---|
| C1 relative speeds ~2× | #2 (open; 1q "relative plate speed tuning") — #32 is effectively the measurable target of that open item |
| C4 young-crust subduction, exponential age distribution | #2 (finding 1n) |
| numerical extra sea floor (created +10% over kinematic) | #2 (1b/1m), #6 |
| ancient tail raises the mean age, not production | #14 |
| downstream: slab dynamic topography | #37 |
| C2 automatic trench at every converging contact (no OCB-type boundary, no initiation resistance) | **none — proposed new issue** |
| C3 no slow diffuse continental boundaries (CRB/CTF/CCB); sutures merge plates | **none — proposed new issue** (could be folded into the C2 issue) |

Proposed metric improvement (no bug): log the production split by boundary kind and the
relative-speed share below 2 cm/yr next to Earth's PB2002 values (39%), and the ridge length of
all diverging boundary cells, so that fixes for C1–C3 can be judged.

## References

- Bird, P. (2003), An updated digital model of plate boundaries, G³ 4(3), 1027 (PB2002; global
  area production 0.108 m²/s).
- Parsons, B. (1982), Causes and consequences of the relation between area and age of the ocean
  floor, JGR 87, 289–302.
- Rowley, D. B. (2002), Rate of plate creation and destruction: 180 Ma to present, GSA Bull. 114.
- Cogné, J.-P., Humler, E. (2004), Temporal variation of oceanic spreading and crustal
  production rates during the last 180 My, EPSL 227.
- Coltice, N., Rolf, T., Tackley, P. J., Labrosse, S. (2012), Dynamic causes of the relation
  between area and age of the ocean floor, Science 336, 335–338.
- Müller, R. D. et al. (2008), Age, spreading rates, and spreading asymmetry of the world's ocean
  crust, G³ 9; Seton, M. et al. (2020), A global data set of present-day oceanic crustal age and
  seafloor depth, G³ 21.
- Gurnis, M., Hall, C., Lavier, L. (2004), Evolving force balance during incipient subduction,
  G³ 5; Stern, R. J., Gerya, T. (2018), Subduction initiation in nature and models,
  Tectonophysics 746.
