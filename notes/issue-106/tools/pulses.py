"""CO₂ response to large LIPs (#106): forcing change after each LIP ≥ 10⁷ km³ vs the step before.

Usage: python3 pulses.py RUN_DIR"""
import re, sys, statistics as st
from pathlib import Path
rows = []
for seed_dir in sorted(Path(sys.argv[1]).glob("seed-*")):
    steps, lips = [], []
    for line in open(seed_dir / "log.txt", encoding="utf-8"):
        if "planet: carbon:" in line:
            steps.append((float(re.search(r"pCO₂ ([\d.e-]+)", line)[1]), float(re.search(r"forcing \+?([-\d.]+)", line)[1]), float(re.search(r"plumes ([\d.]+)", line)[1])))
        elif "LIP at" in line:
            m = re.search(r"LIP at (\d+) Myr.*?: ([\d.e+]+) km³", line)
            lips.append((int(m[1]), float(m[2])))
    for t, v in lips:
        if v >= 1e7 and 3 <= t < len(steps) - 10:
            before = steps[t - 2]
            after = steps[t - 1: t + 10]
            peak = max(after, key=lambda s: s[1])
            if steps[t - 3][0] == steps[t - 2][0]:
                continue  # CO₂ still held (ramp-up)
            rows.append((v, peak[1] - before[1], peak[0] / before[0], after[0][2] - before[2]))
for lo, hi in [(1e7, 3e7), (3e7, 1.01e8)]:
    sel = [r for r in rows if lo <= r[0] < hi]
    if sel:
        print(f"LIPs {lo:.0e}–{hi:.0e} km³: n={len(sel)}, plume source +{st.mean(r[3] for r in sel):.2f} Tmol/yr in the step, "
              f"peak forcing +{st.mean(r[1] for r in sel):.2f} W/m² (max +{max(r[1] for r in sel):.2f}), pCO₂ ×{st.mean(r[2] for r in sel):.3f}")
