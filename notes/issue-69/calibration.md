# Recalibration of `collision_resistance` (#69, requested on PR #81)

_2026-11-21. Only `collision_resistance` changes; everything else as in PR #81 (suture threshold
0.2 confirmed)._

## Target and search (written before the runs)

- Why: the belt resistance (PR #81) took over part of what `collision_resistance` = 50 was tuned
  for on `main` (no potential-energy term), and mean land fell to 0.72 km (Earth ~0.84).
- Target, in order: (1) continental crust > 50 km within 3–6% (8-seed mean ≤ 6%);
  (2) collision stacking ≤ 23 Gt/yr; (3) slow (< 2 cm/yr) and continental boundary shares not
  below PR #81's (18.6%, 17.6%) by more than the seed scatter; (4) mean land as close to
  0.84 km as (1)–(3) allow.
- Search: lower values only (less resistance → more convergence → more stacking → higher land):
  20, 30, 40 on 8 seeds (0–7), against PR #81 at 50 (`pr`, 16 seeds). The chosen value is then
  confirmed on 16 seeds against `main`. If no value meets (1)–(2) with mean land ≥ ~0.8 km, the
  trade-off is reported and the value closest to 0.84 km within (1)–(2) is proposed.

## First sweep (on main with #82, 1a4c25b; branch rebased, band strength via the new
`lithosphere_strength(crust, time)`)

The first sweep on the old base was cancelled when #82 merged; everything below is on the new
base. `main82` = new main, 16 seeds; branch at 20/30/40/50, 8 seeds each, 500–1000 Myr.

| collision_resistance | mean land (m) | crust > 50 km (%) | stacking (Gt/yr) | slow (%) | continental (%) | flooded (%) |
|---|---|---|---|---|---|---|
| main82 (50, no band/GPE, threshold 0.5) | 876 | 5.6 | 20.5 | 11.3 | 13.0 | 24.9 |
| 20 | 713 | 3.4 | 19.5 | 16.1 | 15.8 | 29.2 |
| 30 | 631 | 2.7 | 16.4 | 15.8 | 15.5 | 32.8 |
| 40 | 702 | 3.4 | 20.1 | 16.0 | 15.9 | 26.4 |
| 50 (PR #81) | 685 | 2.4 | 19.1 | 16.7 | 16.5 | 31.3 |

Mean land does not respond to `collision_resistance` between 20 and 50 (all within seed scatter,
~0.63–0.71 km): with the belt resistance, thick belts stop converging by their potential energy
whatever the viscous term, so a weaker viscous term does not stack more crust. The search was
extended to 0 and 10 to see whether the effect appears at all.
