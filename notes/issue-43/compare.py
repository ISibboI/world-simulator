#!/usr/bin/env python3
"""Seed-by-seed comparison of two sets of runs (logs and boundary dumps), Welch t.

Usage: compare.py --a DIR... --b DIR... [--from 500] [--to 1000] [--no-boundaries]

Each DIR is one seed's output directory with `log` and `boundaries_<Myr>.csv`/`mesh.csv`.
Per seed, every metric is the mean over the snapshots in the window (event counts: the rate
between the first and the last snapshot of the window). Prints a Markdown table: mean of set A,
mean of set B, Welch t (B − A).
"""
import argparse
import math
import os
import re
import sys
from collections import defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sim_classes  # noqa: E402

PATTERNS = [
    ("mean ocean age (Myr)", r"mean ocean age\s+([\d.]+)"),
    ("sea floor mean age (Myr)", r"sea floor: mean age ([\d.]+)"),
    ("sea floor >= 200 Myr (%)", r"sea floor: mean age [\d.]+ Myr, ≥ 200 Myr ([\d.]+)%"),
    ("mean plate speed (cm/yr)", r"mean speed\s+([\d.]+) cm/yr"),
    ("plates", r"\| (\d+) plates \("),
    ("largest plate (%)", r"largest ([\d.]+)%"),
    ("highest cell (m)", r"elevation\s+-?\d+ m \.\.\s+(\d+) m"),
    ("mean land (m)", r"land mean\s+(-?\d+) m"),
    ("continental crust flooded (%)", r"% of the surface, (\d+)% flooded"),
    ("log: relative speed median (cm/yr)", r"relative speed median ([\d.]+) cm/yr"),
    ("log: slower than 2 cm/yr (%)", r"(\d+)% slower than 2 cm/yr"),
    ("log: trenches (10^3 km)", r"trenches (\d+) km", 1e-3),
    ("log: ridges (10^3 km)", r"ridges (\d+) km", 1e-3),
    ("log: collisions (10^3 km)", r"collisions (\d+) km", 1e-3),
    ("slab pull (10^21 N)", r"slab pull ([\d.e+]+) N", 1e-21),
    ("ridge push (10^20 N)", r"ridge push ([\d.e+]+) N", 1e-20),
    ("production kinematic (km^2/yr)", r"kinematic ([\d.]+), created"),
    ("production created (km^2/yr)", r"created ([\d.]+) \|"),
    ("dynamic topography min (m)", r"dynamic topography \(m, dry\): (-?\d+)"),
    ("collision stacking (Gt/yr)", r"collision stacking \(Gt/yr\): ([\d.]+) stacked"),
    ("sediment part of sea level (m)", r"sediment part \+?(-?[\d.]+) m"),
    ("subducted oceanic (Gt/yr)", r"subducted oceanic -([\d.]+)"),
]
COUNTERS = [
    ("sutures per 100 Myr", r"sutures (\d+), captures"),
    ("terrane accretions per 100 Myr", r"terranes (\d+) \("),
    ("rifts per 100 Myr", r"rifts (\d+), initiations"),
    ("initiations per 100 Myr", r"initiations (\d+), breakups"),
    ("plates born per 100 Myr", r"plates \((\d+) born"),
    ("resumed subductions per 100 Myr", r"resumed subductions (\d+)"),
]


def parse_log(path, t0, t1):
    snaps = []
    cur = None
    finished = None
    for line in open(path, errors="replace"):
        m = re.search(r"t =\s+([\d.]+) Myr \|", line)
        if m:
            cur = {"t": float(m.group(1)), "text": ""}
            snaps.append(cur)
        if cur is not None:
            cur["text"] += line
        m = re.search(r"Finished in ([\d.]+)s", line)
        if m:
            finished = float(m.group(1))
    snaps = [s for s in snaps if t0 <= s["t"] <= t1]
    out = {}
    for spec in PATTERNS:
        name, pat = spec[0], spec[1]
        scale = spec[2] if len(spec) > 2 else 1.0
        vals = [float(m.group(1)) * scale for s in snaps for m in [re.search(pat, s["text"])] if m]
        if vals:
            out[name] = sum(vals) / len(vals)
    for name, pat in COUNTERS:
        vals = [(s["t"], float(m.group(1))) for s in snaps for m in [re.search(pat, s["text"])] if m]
        if len(vals) >= 2:
            out[name] = (vals[-1][1] - vals[0][1]) / (vals[-1][0] - vals[0][0]) * 100
    if finished:
        out["runtime (s)"] = finished
    return out


def boundary_metrics(run, t0, t1):
    import glob
    pos, nb, edge = sim_classes.load_mesh(os.path.join(run, "mesh.csv"))
    items, n = [], 0
    for path in sorted(glob.glob(os.path.join(run, "boundaries_*.csv"))):
        t = float(re.search(r"boundaries_(\d+)", path).group(1))
        if t0 <= t <= t1:
            items += sim_classes.classify_snapshot(sim_classes.load_boundaries(path), pos, nb, 0, edge)
            n += 1
    by = defaultdict(float)
    for cls, l, *_ in items:
        by[cls] += l
    total = sum(by.values())
    out = {}
    groups = {
        "subduction": lambda c: c.startswith("subduction"),
        "subduction, intra-oceanic": lambda c: c == "subduction, ocean-ocean",
        "ridge": lambda c: c == "divergent, ridge",
        "transform": lambda c: c.startswith("transform"),
        "continental rift": lambda c: c == "divergent, continental rift",
        "convergent without slab": lambda c: c.startswith("convergent"),
        "continental (rift, transform, convergent)": lambda c: c in (
            "divergent, continental rift", "transform, continental",
            "convergent, continent-continent", "convergent, ocean-continent, no slab"),
    }
    out["boundary length (10^3 km)"] = total / n / 1e3
    for g, f in groups.items():
        L = sum(v for c, v in by.items() if f(c))
        out[f"{g} (10^3 km)"] = L / n / 1e3
        out[f"{g} (%)"] = 100 * L / total
    sp = sorted((i[2], i[1]) for i in items)
    acc = 0
    for v, l in sp:
        acc += l
        if acc >= total / 2:
            out["median relative speed (cm/yr)"] = v
            break
    out["slower than 2 cm/yr (%)"] = 100 * sum(l for v, l in sp if v < 2) / total
    sub = [(i[4], i[1]) for i in items if i[0].startswith("subduction")]
    st = sum(l for _, l in sub)
    out["trench within 20 deg of strike (%)"] = 100 * sum(l for a, l in sub if a < 20) / st
    out["trench 70-90 deg from strike (%)"] = 100 * sum(l for a, l in sub if a >= 70) / st
    non = [(i[4], i[1]) for i in items if not i[0].startswith("subduction")]
    nt = sum(l for _, l in non)
    out["non-subducting within 10 deg of strike (%)"] = 100 * sum(l for a, l in non if a < 10) / nt
    return out


def welch(a, b):
    na, nb = len(a), len(b)
    ma, mb = sum(a) / na, sum(b) / nb
    va = sum((x - ma) ** 2 for x in a) / (na - 1) if na > 1 else 0
    vb = sum((x - mb) ** 2 for x in b) / (nb - 1) if nb > 1 else 0
    se = math.sqrt(va / na + vb / nb)
    return ma, mb, (mb - ma) / se if se > 0 else float("nan")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--a", nargs="+", required=True)
    ap.add_argument("--b", nargs="+", required=True)
    ap.add_argument("--from", dest="t0", type=float, default=500)
    ap.add_argument("--to", dest="t1", type=float, default=1000)
    ap.add_argument("--no-boundaries", action="store_true")
    ap.add_argument("--names", nargs=2, default=["A", "B"])
    x = ap.parse_args()
    sets = []
    for dirs in (x.a, x.b):
        per = []
        for d in dirs:
            m = parse_log(os.path.join(d, "log"), x.t0, x.t1)
            if not x.no_boundaries:
                m.update(boundary_metrics(d, x.t0, x.t1))
            per.append(m)
        sets.append(per)
    keys = []
    for m in sets[0] + sets[1]:
        for k in m:
            if k not in keys:
                keys.append(k)
    print(f"| metric | {x.names[0]} | {x.names[1]} | t |")
    print("|---|---|---|---|")
    for k in keys:
        a = [m[k] for m in sets[0] if k in m]
        b = [m[k] for m in sets[1] if k in m]
        if len(a) < 2 or len(b) < 2:
            continue
        ma, mb, t = welch(a, b)
        fmt = (lambda v: f"{v:.0f}") if max(abs(ma), abs(mb)) >= 100 else (lambda v: f"{v:.2f}")
        print(f"| {k} | {fmt(ma)} | {fmt(mb)} | {t:+.1f} |")


if __name__ == "__main__":
    main()
