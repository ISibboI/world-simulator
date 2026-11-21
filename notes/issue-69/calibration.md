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
