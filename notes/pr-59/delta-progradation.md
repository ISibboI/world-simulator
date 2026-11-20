# How river deltas grow into the sea, and what `build_terminal_fan` does

_2026-11-20, for the question on PR #59: "The current mechanism would require that the sediment
extends the river outward, right? Is there research on this?" Literature search by web; where
only an abstract or a citing paper was read, that is said. No code was changed for this note._

## Short answer

Yes. A delta grows seaward because the river deposits its load at its mouth and then **extends
its own channel across the new deposit**. This is well researched, from Gilbert (1885) to
current work on mouth bars and avulsions. The model reproduces the result of that process
averaged over a 1 Myr step (a fan-shaped body of new land, with the rivers routed across it in
the next step), but not the process itself.

## The mechanism in nature

1. **Deposition at the mouth.** Where the river enters standing water, the flow spreads as a jet
   and slows; sand settles a short distance in front of the mouth and builds a **mouth bar**.
   Edmonds & Slingerland (2007) modelled this with Delft3D and found that the distance from the
   mouth to the bar scales with the jet's momentum flux and inversely with grain size
   ([paper](https://ftp.ems.psu.edu/pub/geosc/sling/PUBLICATIONS_SLINGERLAND/2001-2010/Edmondsetal2007.pdf);
   read as abstract and search summary).
2. **The channel extends and splits.** The bar grows until the flow goes around it: the channel
   **bifurcates**, levees grow along the two branches, and each branch builds its own bar
   further out. Repeating this makes the distributary network; the same paper explains the
   observed channel lengths of eleven deltas this way.
3. **Clinoform geometry.** The deposit has a nearly flat top (topset: the delta plain with its
   channels), a sloping front (foreset: 20–35° in coarse Gilbert-type deltas, a fraction of a
   degree to a few degrees in large muddy deltas) and thin distal bottomsets
   ([Budai et al.](https://eprints.whiterose.ac.uk/id/eprint/179249/1/Budai_et_al_gtd.pdf);
   [Patruno & Helland-Hansen 2018](https://hamhillgeology.github.io/publications/patruno2018clinoforms.pdf)).
   The shoreline advances as sediment is added to the front, so the advance per unit of
   sediment is inversely proportional to the water depth to be filled.
4. **Lengthening lowers the slope, the bed aggrades, the river avulses.** A longer channel to
   the same sea level is flatter; the bed builds up upstream until the river breaks out
   (**avulsion**) and takes a shorter, steeper path to the sea, where it starts a new lobe.
   - Avulsions cluster at a node about one **backwater length** (channel depth / bed slope,
     tens to hundreds of km for large rivers) upstream of the shoreline, and the node moves
     seaward as the shoreline does (Ganti et al. 2014, 2016; Chadwick et al. 2019, 2020;
     [Ganti et al. 2016](https://lamb.caltech.edu/documents/19669/Ganti_et_al_JGR_2016.pdf),
     [Chadwick et al. 2020, PNAS](https://pmc.ncbi.nlm.nih.gov/articles/PMC7395564)). These are
     mostly flume and model results; the Yellow River is the main field test (Moodie et al.
     2019).
   - Mississippi: the delta switched to a new complex every 1000–2000 years in the Holocene,
     giving five or six complexes
     ([Roberts 1997, "The delta cycle"](https://repository.lsu.edu/geo_pubs/1657); numbers from
     search summaries). Single distributaries can advance by more than 100 m per year.
   - Over many avulsions the lobes fill a fan-shaped area around the node: the delta.
5. **Accommodation limits it.** Subsidence, compaction and sea-level rise make room that must be
   filled before the shoreline can advance; if supply cannot keep up, the delta stacks upward or
   retreats (search summary of the clinoform literature; I did not find and read Muto & Steel's
   autoretreat papers).
6. **Rates.** Patruno & Helland-Hansen (2018,
   [abstract](https://ui.adsabs.harvard.edu/abs/2018ESRv..185..202P/abstract)): delta
   shorelines prograde 1–100 km per kyr while active; **shelf-edge clinoforms, hundreds of
   metres high, advance 1–100 km per Myr**, built over 0.1–20 Myr by repeated passes of deltas
   across the shelf.

## Deltas over oceanic crust (checking what I said on the PR)

- **Niger delta: confirmed, with a caveat.** Offshore, the delta is underlain by Lower
  Cretaceous oceanic crust, passing landward into hyperextended continental crust (Spina et al.
  2025, Tectonics, [pdf](https://pubblicazioni.unicam.it/retrieve/096ea8fe-5a4a-4f1e-a79b-a49946689ca4/Spina%20et%20al%202025%20Tectonics.pdf));
  oceanic crust 5–7 km thick under the deep-water western delta (Briggs et al.,
  [summary](https://www.earth.ox.ac.uk/node/2172291)). Where exactly the boundary lies is
  disputed: a 2019 conference abstract puts the onshore delta on thin continental or
  transitional crust ([abstract](https://www.searchanddiscovery.com/abstracts/html/2019/ace2019/abstracts/135.html)).
- **Bengal fan: confirmed.** It lies on thin oceanic crust; sediment is 16.5 km thick in the fan
  and up to ~22 km off Bangladesh
  ([gravity study](https://krishi.icar.gov.in/ohs-2.3.1/index.php/record/view/126512),
  [sediment thickness](https://hal.archives-ouvertes.fr/hal-03108158)). It is a deep-sea fan,
  not a shelf; the shelf part is the Bengal delta itself.
- **Mississippi fan: not verified.** My search returned nothing on the crust below it, so I
  withdraw it as an example until checked.

So thick sediment bodies on oceanic crust at river mouths are real. A shelf (water shallower
than 200 m) on oceanic crust is confirmed here only for the outer Niger delta.

## What the model does

`build_terminal_fan` (this PR) takes the sediment of one river mouth that the shelf and the
deep-sea fans refused in this step and deposits it as a cone around the mouth:
- surface slope fixed at 2 × 10⁻⁴, apex at the mouth, height such that the cone holds the
  amount, with the isostatic response of every column;
- it spreads cell by cell through the cells it covers, land or sea; a sea cell filled above sea
  level becomes land.

In the next step the drainage is computed anew from the sea (`drainage`, a priority flood from
all cells below sea level). The new land slopes away from the apex, so the river **does run
across it to the new coast**: the channel is extended, one step late. The cone's sediment is
then eroded like any sediment and carried on to the new mouth.

| | nature | model |
|---|---|---|
| Sediment is laid down at the mouth and the river crosses it afterwards | yes | yes, from the next step on |
| Advance costs more sediment in deeper water | yes (foreset height) | yes (the cell is filled from its floor to the cone surface, with isostasy and compaction) |
| Shape averaged over many avulsions | fan around the avulsion node | cone around the mouth |
| Mouth bars, bifurcations, lobes, avulsion cycles (10–100 km, 10²–10³ yr) | yes | no: below one cell (~110 km) and one step (1 Myr) |
| Surface slope | set by discharge and grain size: ~10⁻⁵–10⁻⁴ on large delta plains, steeper on small fans | one value, 2 × 10⁻⁴ |
| Steep front (foreset), slope failure, turbidity currents off the front | yes | no: the cone surface continues at the same slope; deep-sea fans are filled before the cone, not fed by it |
| Bed aggradation upstream over a backwater length | yes | no: incision has no transport threshold, so a river does not hold a grade |
| Room made by subsidence and sea level | flexure, compaction, eustasy | local isostasy, compaction, the model's sea level |

## Assessment

- The model is the mass balance of progradation without its dynamics. At this resolution that
  is the right level: everything left out is smaller than a cell or faster than a step, except
  the two points below.
- **The rate was checked afterwards** (next section): 1–7 km per Myr on average over deep
  water, against 1–100 km per Myr for natural shelf edges.
- **Known difference: no front.** A real delta that reaches the shelf edge sheds much of its
  load down the slope as turbidity currents. In the model the fans are filled first and the
  cone gets what is left, so the order is reversed, and once the fans are at their graded
  surface nothing limits the cone but volume.

## Measurement of the advance (added after the PR discussion)

Logging-only diagnostic of every cone (physics unchanged, code not committed), seeds
0–3 × 500 Myr. "Deep" is a cell more than 1000 m below sea level before the cone; it counts as
converted when the cone leaves it shallower than 200 m or as land.

| | seed 0 | seed 1 | seed 2 | seed 3 |
|---|---|---|---|---|
| Cones per Myr | 32 | 21 | 57 | 31 |
| … that put sediment into deep water | 35% | 37% | 41% | 53% |
| … of those, converting a deep cell in that step | 1% | 2% | 2% | 1% |
| Cone sediment that goes into deep cells | 23% | 23% | 26% | 29% |
| Deep cells converted per Myr (whole planet) | 0.09 | 0.16 | 0.60 | 0.30 |
| Advance per deep-touching cone, front one cell wide (km/Myr) | 0.9 | 2.2 | 2.8 | 2.1 |
| Advance per deep-touching cone, by the distance of the converted cell from the mouth (km/Myr; upper bound) | 2.4 | 5.9 | 6.8 | 4.2 |
| Mouths above 100 km/Myr by that upper bound (of mouths with ≥ 5 such steps) | 0.3% | 1.3% | 1.6% | 1.2% |

- **The mean advance over deep water is 1–7 km per Myr**, at the low end of the 1–100 km per Myr
  of natural shelf edges. A quarter of the cone sediment goes into deep cells, but a deep cell
  takes many steps to fill.
- The second measure is an upper bound: when the mouth lies in a shelf sea, the converted cell
  is several cells from the mouth, but the shelf edge itself moved by one cell.
- **There is a fast tail.** Half to two thirds of the converted deep cells come from rare cones
  (1% of those touching deep water) with loads of 11–17 km·cells in one step, and 0.3–1.6% of
  the mouths exceed 100 km per Myr by the upper-bound measure.
- With "deep" taken as 200 m instead of 1000 m, the numbers are 3–4 times larger (mean
  10–20 km per Myr by the upper bound), because shallow epicontinental seas fill quickly. Still
  inside the range.

**Conclusion: no change.** The advance is not too fast, so the delta front of the proposal
below is not needed now. The fast tail would be the reason to revisit it.

## Proposal (not implemented)

1. Measure the advance of the coast over oceanic crust per mouth (km per Myr) in the existing
   runs and compare with 1–100 km per Myr. If it is inside, leave the PR as it is.
2. If it is too fast: give the cone a front. Where the cone surface is above a deep cell, only
   part of the sediment builds the front and the rest goes down the slope into the fan,
   without the fan's surface limit. That is the physical sink that "stop the cones at deep
   water" would need.
3. Later, with #58 (inland depressions as lakes): fewer cones are needed at all, since 39–47%
   of the refused sediment is at 1–3-cell inland seas.
4. Not proposed: avulsions, lobes or mouth bars. They cannot be resolved at ~110 km and 1 Myr.
