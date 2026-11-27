# #213: plate count of the supercontinent start

Sebastian on PR #217: "For supercontinent, use as many plates as earth supercontinent is
conjectured to have had." The default `--plates` now depends on `--initial`: 21 for dispersed
continents (unchanged), **11 for `--initial supercontinent`**. Nothing is tuned; the number is
read from published plate reconstructions.

## Source

Plate polygons of the global reconstructions were fetched from the GPlates Web Service
(`https://gws.gplates.org/topology/plate_polygons?time=T&model=M`) and counted by plate id,
with each plate's share of the sphere (`~/tasks/issue-213/tools/count_plates.py`):

| model | 250 Ma | 230 Ma | 200 Ma | 0 Ma |
|---|---|---|---|---|
| Matthews et al. (2016), `MATTHEWS2016_mantle_ref` (Domeier & Torsvik 2014 before 250 Ma) | 11 | 9 | 12 | 46 |
| Müller et al. (2019), `MULLER2019` | 11 | 12 | 15 | 65 ids |

- Matthews, K. J., et al. (2016). Global plate boundary evolution and kinematics since the late
  Paleozoic. *Global and Planetary Change* 146, 226–250. Their abstract: outside ~260–160 Ma the
  number of plates is 16–46; in ~260–160 Ma the model reaches its minimum, "in contrast to what
  we would expect during initial Pangea breakup", partly because small, now subducted oceanic
  plates cannot be reconstructed.
- Müller, R. D., et al. (2019). A global plate model including lithospheric deformation along
  major rifts and orogens since the Triassic. *Tectonics* 38, 1884–1907.

Pangaea before its breakup (250–230 Ma) is the right analogue for the preset (one connected
supercontinent in a superocean); by 200 Ma the breakup has begun. The four counts at 250 and
230 Ma are 11, 9, 11, 12; their median, 11, is the default.

How the plates looked:

- 250 Ma (Matthews): Pangaea on **one** plate, 37% of the sphere; Farallon 16%, Izanagi 15%,
  Phoenix 13% (Panthalassa), Cimmerian–Neotethys 9%, Mongol–Okhotsk 6%, then small ones
  (Alexander 3%, South China 2%, < 1%).
- 230 Ma: Pangaea on two to three plates (Gondwana 23%, Laurasia 14% or Eurasia 8% + a
  Proto-Caribbean mesh 7%), the same Panthalassa and Tethys plates.

The model's start keeps its heavy-tailed rank–area shape (#213); with 11 plates its largest
target is ~33% (with 25% scatter), and the plates' cohesion with continental crust puts the
supercontinent (40% of the surface) on one or two plates, as in the reconstructions.

Caveat: the reconstructions' low count around 250–200 Ma is partly a gap in knowledge (subducted
microplates leave no record), so the real count was likely somewhat higher; 11 is what the
published models give, not a fit.

## Check: supercontinent, 4 seeds × 500 Myr, 11 vs 21 plates

Runs on 2408831 (this branch merged with main incl. #212), `--initial supercontinent
--duration 500 --snapshot-interval 10 --plate-log 10`, with the new default (11) and with
`--plates 21`; outputs in `~/tasks/issue-213/runs/super11`, `super21`, analysed with
`~/tasks/issue-213/tools/super.py`. "Breakup" is when the largest connected mass of
continental crust holds less than 75% of it.

| | 11 plates (new default) | 21 plates (before) |
|---|---|---|
| ramp-up end (Myr), seeds 0–3 | 40, 80, 40, 90 | 40, 40, 100, 40 |
| plates at 100 / 200 / 500 Myr | 14–17 / 12–20 / 13–17 | 15–20 / 14–22 / 12–15 |
| largest plate at 0 / 500 Myr | 33–48% / 30–67% | 18–35% / 22–40% |
| breakup | 1 of 4 seeds (seed 1 at 100–130 Myr, re-welded by 200 Myr); 3 seeds keep one connected continent (≥ 95%) to 500 Myr | 3 of 4 seeds at 40–100 Myr into 2–3 continents (largest 46–66%), all re-welded by 300 Myr |

- The 11-plate start puts the supercontinent on one large plate (33–48%), as at 250 Ma
  (Pangaea 37%), plus parts on one or two more; the plate count rises to 14–17 by 100 Myr
  (plates are born by rifting and subduction initiation) and stays at 12–20.
- The ramp-up still ends within the safety net (40–90 Myr), somewhat later than with 21
  plates (median 60 vs 40 Myr), because the plate count grows during the first 100 Myr.
- **Behaviour change**: with fewer, larger plates the supercontinent mostly does not break up
  within 500 Myr. This returns to the slow breakup known from before #213 (the archived
  summary: 200–300 Myr vs Pangaea's ~100 Myr). Pangaea's breakup began ~200 Ma, some 50 Myr
  after the 250-Ma state; the model's breakup is driven by its rifting model (#195), which
  the plate count only sets the stage for. Not tuned here; the plate count is from the
  reconstructions as asked.
