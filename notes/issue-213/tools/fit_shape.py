"""Fits the plate-size shape of `tectonics::generation` (#213), share(rank r from 0) ∝
exp(−r^stretch / scale), to the mean rank–area table that rank_areas.py prints (percent, largest
first) for the steady part of runs with `--plate-log`. Usage: fit_shape.py 30.98 19.13 11.89 …"""
import math, sys
d = [float(x) for x in sys.argv[1:]]
best = None
for p in [x * 0.01 for x in range(40, 160)]:
    for scale in [x * 0.05 for x in range(4, 400)]:
        f = [math.exp(-(r ** p) / scale) for r in range(len(d))]
        s = sum(f)
        err = sum((math.log(100 * x / s) - math.log(y)) ** 2 for x, y in zip(f, d))
        if best is None or err < best[0]:
            best = (err, p, scale)
print(f"stretch {best[1]:.2f}, scale {best[2]:.2f} (squared log error {best[0]:.3f})")
