# Volcanism, phases 0–2: results (issue #106)

Plan: [plan.md](plan.md) (with the review response). Plan review: [plan-review.md](plan-review.md).
Independent evaluation of the second version: [evaluation-1.md](evaluation-1.md). Raw tables:
[analysis-final.txt](analysis-final.txt) (final), [analysis-v2.txt](analysis-v2.txt),
[analysis-v1.txt](analysis-v1.txt); scripts in [tools/](tools/).

Runs: default barren preset, 8 seeds × 1000 Myr, `--cell-dump --snapshot-interval 100`, branch
against `main` at e55bf1d (with #217's ramp-up), on GitHub runners via `runs`.
`--set volcanism.enabled=0` gives `main`'s state hashes exactly (checked over 30 Myr), so
volcanism changes nothing else, also not the random draws.

## What was built

- **LIPs** (phase 1): one per plume when its head arrives (age 10 Myr). Log-normal volume
  `V = 2 × 10⁷ km³ · (peak flux / mean) · L · e^{σε}`, σ = ln(10)/3, clamped to 10⁵–10⁸ km³.
  `L = ((180 km − z_L) / 180 km)²` is the lid factor (melt column below the lithosphere):
  0.08 under continents (130 km lid), 0.2–0.7 under oceanic lithosphere (`2.32 √(κ t)`, ≤ 95 km).
  Spread as a Gaussian (σ ≥ 400 km, wider for large provinces so they stay ≤ ~25 km thick at
  the centre) as mafic crust of 2950 kg/m³ over the visible samples.
- **Tails** (phase 2): each step `3 × 10⁷ km³/Myr per m²/s · flux · L`, Gaussian σ = 150 km;
  the plate moves over the plume and records a track; on a ridge it thickens the new crust.
- **Limits**: plume crust only up to the crust thickness cap and up to a **40 km mafic column**
  (deeper mafic rock turns to eclogite and founders); the rest is not added.
- **Ledger** terms `LIPs` and `plume tails` (actual mass); **log** line per LIP and a
  `magmatism (km³/yr)` line per snapshot (ridges, arcs, LIPs, tails, LIP list).
- **Carbon**: LIP crust degasses 0.07 mol C/kg (0.3 wt% CO₂) into #196's plume source in its
  step; tails add no carbon of their own (#196's plume term already books them through the same
  `Plume::flux`); ridge crust is unchanged. No double counting.

## Key numbers (final version, 8 seeds)

| | branch | main | Earth (orientation) |
| --- | --- | --- | --- |
| LIPs | 534, one per 15.0 Myr | – | – |
| LIPs ≥ 10⁶ km³ | one per 16.8 Myr | – | ~1 per 20–30 Myr |
| LIPs ≥ 10⁷ km³ (plateaus) | one per 62 Myr | – | ~1 per 50–75 Myr |
| Oceanic LIPs | 398, median 6.7 × 10⁶ km³, p90 2.3 × 10⁷, max 10⁸ | – | plateaus 10⁷–10⁸ (Ontong Java 4–6 × 10⁷) |
| Continental LIPs | 136, median 1.5 × 10⁶ km³, p90 3.9 × 10⁶, max 9.6 × 10⁶ | – | 10⁵–4 × 10⁶ (Columbia River – Siberia) |
| LIP volume rate | 0.56 km³/yr | – | ~0.5 km³/yr (last 300 Myr, plateaus included) |
| Plume-tail rate | 0.27–0.47 km³/yr | – | 0.3–0.4 km³/yr oceanic intraplate (Crisp 1984) |
| Ridges / arcs | 35–43 / 2.5–3.0 km³/yr | same (not changed) | ~21 / ~3–8 |
| Plateau cells (mafic, > 12 km) | 1.4–3.6% of the surface, mean −1.0 to −3.5 km | 0.1–0.3% | – |
| Ocean shallower than 3 km | 27–38% | 27–40% | – |
| Mafic columns > 40 km | 0–0.17% of cells (collision-stacked plateaus) | 0 | rare |
| Felsic fraction of continental crust | 0.90–0.94 | 0.93–0.95 | – |
| pCO₂ / T at 1000 Myr (mean) | 0.347 bar / 17.5 °C | 0.355 bar / 17.9 °C | – |
| Runtime (mean, GitHub) | 863 s | 939 s | runner noise; the code alone costs < 1% (local test) |

The ledger closes (residual ~10⁻⁷ Gt over 1000 Myr); felsic mass is not touched by plume crust.

## Findings

**LIP frequency.** The trigger is the existing plume clock (`plume_interval` 15 Myr), so the
model has one LIP per 15 Myr, about one ≥ 10⁶ km³ per 17 Myr. That is somewhat more often than
Earth's ~1 per 20–30 Myr for large ones, but within the plausible range for an Earth-like
planet; it follows `mantle.plume_interval` directly. Plumes rise far from slab graveyards, and
they cluster: the evaluation found 23 LIP pairs within 10 Myr and 1000 km (≈ 2 expected at
random). That is a property of the plume-birth rule, not of the volcanism.

**Volumes.** Settings separate as on Earth: plateaus 10⁶–10⁸ km³ in the ocean, flood basalts
~10⁶ km³ on continents. The overall median (~4–5 × 10⁶ km³) is half a decade above Earth's
~10⁶: the model has almost no small (10⁵–10⁶ km³) LIPs, because every plume makes exactly one
and small plumes (low flux) are not a separate class. 95% of the LIP volume lands in the ocean
(the model's ocean is 70% of the surface, and the lid factor favours it).

**Relief.** Oceanic plateaus are the clear effect: 1–4% of the surface becomes mafic plateau
crust standing at −1 to −3.5 km (mean), against the −3.6 to −4 km median ocean floor; some
reach sea level. They are buoyant (≳ 19 km at 2950 kg/m³), resist subduction and accrete at
trenches (collision stacking rose from 16 to 25 Gt/yr in the second version). Continental
flood basalts add ~1–4 km of crust over ~10⁶ km², i.e. ~0.1–0.5 km of elevation. Hotspot tracks
are faint at ~110 km cells: streaks and spots of shallower sea floor (sub-grid shields, #64).
Land hypsometry and the continental fraction stay within the seed spread; the land differences
are felsic mountain belts in single seeds.

**Pile-up (fixed).** The first version (tail flux 10⁸, 1.2 km³/yr) and the second (3 × 10⁷)
built mafic columns of 47–75 km in every seed, standing up to 3.7 km high: plumes rising close
together stacked several LIPs and tails, and buoyant plateaus were stacked in collisions. The
40 km mafic column limit stops the plume part; the remaining > 40 km columns (0–0.17% of cells,
up to 70 km) are plateaus stacked in **collisions**, which treat mafic crust like continental
crust. On Earth, such crust eclogitises and founders: a follow-up for the collision model, not
for volcanism.

**CO₂ and climate.** A LIP of 10⁷–3 × 10⁷ km³ adds ~3.4 Tmol/yr to the plume source in its step
(background total ~12–14 Tmol/yr) and raises the forcing by ~0.4 W/m² at its peak (×1.04 CO₂;
~0.3 K with the model's sensitivity); the largest plateaus (> 3 × 10⁷ km³) add ~10 Tmol/yr and
~0.8 W/m² (max 1.9). The weathering of the extra mafic land and plateaus takes it back within a
few Myr: at 1000 Myr pCO₂ and temperature are within seed noise of main (0.347 vs 0.355 bar,
17.5 vs 17.9 °C). Continental LIPs (~10⁶ km³, ~0.2 Tmol/yr in their step) are invisible in the
climate, as the design expected for Myr steps; Earth's LIP climate spikes are sub-Myr pulses and
thermogenic carbon (phase 3).

**Carbon per kg, head vs tail.** #196's plume term gives ~1 Tmol/yr for ~9 plumes regardless of
the lid; spread over the tails' ~0.35 km³/yr that is ~1 mol/kg, far above basalt's 0.05–0.1. It
is not double counting (the tail crust adds no carbon), but #196's plume coefficient stands for
the plumes' whole deep degassing (including melts that freeze at depth), not for the tail crust
the model now shows. A follow-up could tie the plume term to the tail magma.

## Caveats

- Plateau crust is "transitional" (thicker than 12 km): no subduction initiation there, no
  ridge push, sediment zones of a shelf. Accepted as physical for plateaus.
- Mafic crust added to continents is mixed into the column's `felsic` fraction; a 1–2 km fresh
  basalt surface weathers in #196 only by that small change of the column (phase 3 needs a
  surface-lithology tracer).
- Plume heads that existed at the start (#217 primes the mantle) older than 10 Myr have had
  their LIP; they only feed their tails.
- Individual volcanoes are sub-grid (#64): a Hawaii-size shield is half a cell, so tracks show
  as cell-mean rises of a few hundred metres, not peaks.

## Follow-ups

1. Phase 3: surface lithology (fresh basalt) for weathering; thermogenic CO₂ of LIPs into
   sediment; optional LIP aerosol forcing (with #204).
2. Eclogitisation/foundering of thick mafic crust in collisions (stacked plateaus > 40 km).
3. Small LIPs (10⁵–10⁶ km³): weaker plumes or several pulses per head, if #110 needs them.
4. Tie #196's plume carbon to the tail magma volume.
5. Phase 4 with #110: activity view from the LIP record and the per-source rates.
