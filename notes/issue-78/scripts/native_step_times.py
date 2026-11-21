#!/usr/bin/env python3
"""Per-step times of a native run logged with RUST_LOG=debug ("Step finished" lines).

Usage: native_step_times.py LOG...  Prints, per log, the median step time (tectonics + climate +
erosion, and each part) in windows of steps, to show the warm-up and the drift over a run.
"""
import re
import statistics
import sys

UNITS = {"ns": 1e-6, "µs": 1e-3, "us": 1e-3, "ms": 1.0, "s": 1e3}
LINE = re.compile(r"step\{time=([\d.]+)\}.*Step finished tectonics_time=([\d.]+)(ns|µs|us|ms|s) "
                  r"climate_time=([\d.]+)(ns|µs|us|ms|s) erosion_time=([\d.]+)(ns|µs|us|ms|s)")
WINDOWS = [(1, 3), (4, 10), (11, 20), (21, 50), (51, 100), (101, 200), (201, 300), (301, 400),
           (401, 500)]

for path in sys.argv[1:]:
    steps = []
    for line in open(path, errors="replace"):
        m = LINE.search(line)
        if m:
            parts = [float(m.group(i)) * UNITS[m.group(i + 1)] for i in (2, 4, 6)]
            steps.append((float(m.group(1)), parts))
    print(f"{path}: {len(steps)} steps")
    print("  steps      total  tecton climate erosion  (median ms)  p10..p90 total")
    for lo, hi in WINDOWS:
        window = [p for t, p in steps if lo <= t <= hi]
        if not window:
            continue
        totals = sorted(sum(p) for p in window)
        med = [statistics.median(p[i] for p in window) for i in range(3)]
        p10, p90 = totals[len(totals) // 10], totals[(len(totals) * 9) // 10]
        print(f"  {lo:3d}-{hi:<3d} {statistics.median(totals):8.0f} {med[0]:7.0f} {med[1]:7.0f}"
              f" {med[2]:7.0f}   {p10:5.0f}..{p90:5.0f}")
