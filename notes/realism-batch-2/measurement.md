# Batch 2: what the plate boundaries look like, measured (#14, #32, #42, #43, #6)

_2026-11-20. `main` at 11989cb (after #59). Triage: [`../realism-triage/overview.md`](../realism-triage/overview.md)._

The triage found that #42 ("every converging oceanic contact becomes a trench") is not the main
cause of the high sea-floor production (#32). This note measures what is.

## Data

- `main`: 8 seeds × 1000 Myr with a new boundary dump (`boundaries_<Myr>.csv`, written with
  `--cell-dump`: every boundary cell with its kind, relative velocity across and along the
  boundary, slab length, crust age and plate rotation). Means over the snapshots of
  500–1000 Myr, seed range in brackets.
- Sensitivity runs, seeds 0–2 × 1000 Myr, one change each (`--set`), against the same seeds
  of `main`.
- Earth: PB2002 (Bird 2003), evaluated the same way (`earth_pb2002.py`; a plate counts its
  subduction steps where it is the subducting plate).
- A plate is called **slab-attached** when its own mature trenches (slab ≥ 150 km) make up at
  least 10% of its boundary length.

## 1. Model against Earth

| | model | Earth (PB2002) |
|---|---|---|
| Plates ≥ 1% of the surface | 13.3 (10.8…16.8) | 13 |
| … of them slab-attached | **9.5** (7.2…11.2) | **5** (Pacific, Australia, Nazca, Sunda, Philippine Sea) |
| Surface on plates with an own-trench share < 2% / 2–10% / ≥ 10% | 16 / 35 / 49% | 18 / 45 / 37% |
| Boundary between two plates that are both not slab-attached | **9%** (5…13) | **48%** |
| … median relative speed there (cm/yr); share slower than 2 cm/yr | 3.5 (1.9…5.3); 34% | 1.5; 59% |
| Boundary with one plate slab-attached | 53% | 39% |
| … median relative speed; share slower than 2 cm/yr | 6.6; 3% | 5.5; 22% |
| Boundary with both plates slab-attached | 38% | 12% |
| … median relative speed; share slower than 2 cm/yr | 8.3; 2% | 6.0; 14% |
| Boundary slower than 2 cm/yr | 5.5% | 39% |
| Median relative speed (cm/yr) | 6.8 | 3.0 |
| Trench length (10³ km) | 113–125 | 51 |
| Trench where the overriding plate is itself slab-attached | 36% | 27% |
| Trench converging slower than 2 cm/yr | 20% | 21% |
| Mean speed of cells on slab-attached / other plates (cm/yr) | 5.9 / 2.1 | ~6–8 / ~1–3 |

Reading:
- Speeds per class are Earth-like. So is the split of the surface between the classes
  (49% on slab-attached plates against 37%).
- The main difference is the **mix**: plates without slabs **rarely border each other** (9% of
  the boundary against 48%). On Earth half of the boundary length lies between such plates
  (the Atlantic, Indian-Ocean and Arctic ridges, Africa–Eurasia, the Americas), and that is
  where the slow boundaries are.
- Within each class the model is faster than Earth by a factor of 1.2–2.3, and has fewer slow
  stretches. Boundaries between slab-free plates are the slowest in the model too (3.5 cm/yr).
- The threshold of 10% matters for the plate count but not for the conclusion (results
  evaluation, [`review-results.md`](review-results.md)): slab-attached plates in the model
  against Earth are 11.0 / 8 at 5%, 9.5 / 5 at 10% and 7.9 / 3 at 20%; the boundary share
  between slab-free plates is 3.8 / 26%, 9 / 48% and 19 / 68%. So "5–7 times less boundary
  between slab-free plates" is the robust statement; "twice as many slab-attached plates"
  holds at 10% only. Earth is a single snapshot.
- An earlier version of this table gave 6.2 cm/yr for the boundaries between slab-free plates.
  That was a bug in the analysis script (one key for three classes); the evaluation found it.

## 2. What is subducted

| | model | Earth |
|---|---|---|
| Trench length subducting crust younger than 20 Myr / 10 Myr | **33% / 17%** | roughly 10–15% younger than 20 Myr (Cascadia, Mexico, southern Chile, Nankai; my estimate, not computed) |
| Subduction flux carried by crust younger than 20 Myr | 27% | ~11% (from the age–area distribution, #32) |
| Median age of the subducting sea floor (Myr) | 32 | ~50–60 |
| Trench length with a slab shorter than 150 km | 7% | — |

A sixth of the trench length subducts lithosphere younger than 10 Myr. Such lithosphere is
lighter than the asthenosphere (Cloos 1993); in the model nothing resists its subduction, it
only stops pulling.

## 3. Where the ancient sea floor is (#14)

Oceanic crust ≥ 200 Myr is 4.9% (3.6…7.9) of the oceanic crust; median age 320 Myr, 90th
percentile 640 Myr (the oldest was 880 Myr in the batch 1 baseline).

| Of the ancient oceanic cells | |
|---|---|
| within 300 km of continental crust of their own plate | 84% |
| farther than 550 km from any trench | 79% |
| on plates that are not slab-attached | 76% |
| filled to shallower than 2000 m; mean sediment | 60%; 8.9 km |
| at a plate boundary | 3% |

So the ancient sea floor is not forgotten ocean in plate interiors. It is the **oceanic basement
of passive margins**: strips one to three cells wide along continents, buried under the
continental rise and shelf, on plates that subduct nowhere near. It stays until its margin is
caught in a collision, which for half of it takes more than 300 Myr.

Earth has such crust (central Atlantic margins ~180 Myr, Gulf of Mexico ~160 Myr, eastern
Mediterranean ~340 Myr), though less of it and younger.

## 4. Sensitivity runs (seeds 0–2, 500–1000 Myr; t against `main`)

| run | trenches (10³ km) | production (km²/yr) | plates | sea floor < 20 Myr | mean age (Myr) | sea floor ≥ 200 Myr |
|---|---|---|---|---|---|---|
| `main` | 131 | 6.15 | 26.8 | 35.7% | 52 | 3.2% |
| no subduction initiation at passive margins and fracture zones (`events.initiation_waiting_time`, `events.invasion_waiting_time` = 10⁹) | 99 (t −6.5) | 4.55 (−5.7) | 15.1 | 26.1% | **102** | **13.8%** |
| only invasion from existing trenches (`events.initiation_waiting_time` = 10⁹; 1 seed) | 120 | 6.0 | 23.6 | 35% | 54 | 4.4% |
| no initiation at existing boundaries (`events.boundary_initiation_waiting_time` = 10⁹) | 133 | 6.40 | 27.3 | 36.9% | 47 | 2.3% |
| no plume rifts in large oceanic plates (`events.min_oceanic_rift_area` = 1) | 118 (−1.9) | 5.77 (−1.2) | 23.1 | 33.9% | 54 | 3.5% |
| buoyant young lithosphere + initiation resistance, first version (see [`plan.md`](plan.md)) | 130 | 5.44 (−2.6) | 30.6 | 33.7% | 53 | 2.9% |
| supercontinent start (`--initial supercontinent`), 100–300 / 300–600 / 600–1000 Myr | 103 / 119 / 117 | 5.5 / 6.5 / 6.2 | 21 / 26 / 26 | | 50 / 46 / 51 | |

- **Subduction initiation at passive margins together with its spreading from existing
  trenches (invasion) is the largest single lever** (production −26%, trenches −24%), but it
  is also what keeps the ocean young: without both the mean sea-floor age doubles and the
  ancient share quadruples. Turning off the isolated initiation alone changes little (one
  seed: 6.0 against 6.2 km²/yr), so the invasion carries most of it.
- Even without any such initiation the trenches are 99 thousand km, twice Earth's.
- **The arrangement of the continents is not the cause.** From a supercontinent start the
  structure is the same in every window, also during the dispersal: boundary between plates
  without slabs 8–10%, median relative speed 7.0–7.1 cm/yr.
- Initiation at existing boundaries and plume rifts in oceanic plates change nothing
  significant.

## 5. #6: created against kinematic production

Created sea floor exceeds the kinematic opening by 12% (6.69 against 5.97 km²/yr). The area of
sea floor younger than 2 Myr gives 6.4 km²/yr (#32's investigation), so about half of the
excess is real area and half is sea floor that is created at a flickering boundary cell and
overridden again within a step or two. Mass is conserved (the ledger closes); it is churn of
3–6% of the production at boundaries. Not changed in this batch.

## Conclusions

1. No single rule or parameter explains the factor of ~2 in trench length and production. The
   model's plates are organised differently from today's Earth: more plates carry slabs, and
   slab-free plates rarely border each other.
2. Two physical gaps were found and are fixed in this batch (see [`plan.md`](plan.md)): young
   lithosphere is not buoyant, and nothing resists the start of subduction.
3. #14 is a different thing than its title says: old oceanic basement under passive margins.
4. What sets the number of slab-attached plates is still open. The candidates not yet tested:
   how continental plates are coupled to the mantle flow (they are dragged by it with four
   times the oceanic coupling), and how a passive-margin initiation divides a plate (each one
   makes a new plate).

## Earth numbers (PB2002, `earth_pb2002.py`)

```
plates 52 total boundary km 260483 trench km 51309
plates >= 1%: 13 ; slab-attached (>= 10% of boundary): ['AU', 'NZ', 'PA', 'PS', 'SU']
   PA: area 20.5%  own-trench share 28%
   AF: area 11.5%  own-trench share 5%
   AN: area 11.4%  own-trench share 5%
   NA: area 10.9%  own-trench share 3%
   EU: area 9.5%  own-trench share 1%
   AU: area 9.0%  own-trench share 19%
   SA: area 8.2%  own-trench share 6%
   SO: area 3.8%  own-trench share 0%
   NZ: area 3.2%  own-trench share 32%
   IN: area 2.4%  own-trench share 8%
   SU: area 1.7%  own-trench share 13%
   PS: area 1.1%  own-trench share 35%
   AM: area 1.0%  own-trench share 0%
surface on plates with own trench share < 2%: 18.2%
surface on plates with own trench share 2-10%: 44.5%
surface on plates with own trench share >= 10%: 37.2%
boundary, neither plate slab-attached: 48% of boundary, median relative speed 1.5 cm/yr, slower than 2 cm/yr 59%
boundary, one plate slab-attached: 39% of boundary, median relative speed 5.5 cm/yr, slower than 2 cm/yr 22%
boundary, both plate slab-attached: 12% of boundary, median relative speed 6.0 cm/yr, slower than 2 cm/yr 14%
all: median 3.0 cm/yr, slower than 2 cm/yr 39%
trench: overriding plate itself slab-attached 27%; convergence slower than 2 cm/yr 21%
```
