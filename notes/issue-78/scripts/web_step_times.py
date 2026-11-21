#!/usr/bin/env python3
"""Per-step wall times of the web version from a headless-Chromium stderr log.

Run the page with `?snapshot-interval=1` so that the worker logs one `t = ...` statistics line
per step, and Chromium with `--enable-logging=stderr --v=0`, which stamps every console line
with the wall time. Prints `step seconds` (time between consecutive `t =` lines).
"""
import re
import sys
from datetime import datetime

pattern = re.compile(r"\[\d+:\d+:(\d{4})/(\d{6}\.\d+):INFO:CONSOLE[^\]]*\] \" INFO world_simulator: t = +([\d.]+) Myr")
previous = None
for line in open(sys.argv[1], errors="replace"):
    match = pattern.search(line)
    if not match:
        continue
    stamp = datetime.strptime("2026" + match.group(1) + match.group(2), "%Y%m%d%H%M%S.%f").timestamp()
    t = float(match.group(3))
    if previous is not None:
        print(f"{t:.0f} {stamp - previous:.3f}")
    previous = stamp
