#!/usr/bin/env python3
"""Plate-boundary types of the model, from the `boundaries_<Myr>.csv` dumps (`--cell-dump`).

Usage: sim_classes.py [--from 500] [--to 1000] [--pca-km 300] RUN_DIR...

Each RUN_DIR holds boundaries_<Myr>.csv and mesh.csv. Every dump line is one boundary cell seen
from its plate towards one neighbouring plate; both sides of a boundary are in the dump, so every
line counts half a cell length, R * (mean edge angle) / 1.1 (the zig-zag factor the model's own
statistics use).

Classification of a line (thresholds as in PB2002, Bird 2003, so that Earth is evaluated the same
way, see pb2002_classes.py):
  * subduction: the plate subducts here (kind t), or it is the overriding plate (kind o, not a
    collision or ridge cell) of a segment that has a slab and subducting cells, and the plates do
    not diverge faster than 0.1 cm/yr (the model's own trench rule);
    split by the crust of the overriding side (ocean-ocean / ocean-continent);
  * otherwise by the angle between the relative velocity and the boundary strike:
    < 20 deg transform; else divergent (opening) or convergent (closing);
  * crust: "continent" if the cell or the other plate's adjacent cells are of continental origin
    (Bird: any step that is not entirely oceanic is continental).
The boundary direction is the model's smoothed normal (as in the dump) or, with --pca-km > 0, the
principal axis of the boundary cells of the same plate pair within that distance (an independent
estimate at a chosen scale). Speeds are full relative speeds (cm/yr).
"""
import argparse
import glob
import math
import os
import re
from collections import defaultdict

import numpy as np

R = 6371.0


def load_mesh(path):
    pos, nb = [], []
    with open(path) as f:
        next(f)
        for line in f:
            c, x, y, z, n = line.rstrip("\n").split(",")
            pos.append((float(x), float(y), float(z)))
            nb.append([int(v) for v in n.split(";")])
    pos = np.array(pos)
    angles = [math.acos(max(-1, min(1, float(np.dot(pos[i], pos[j]))))) for i in range(0, len(pos), 97) for j in nb[i]]
    return pos, nb, float(np.mean(angles))


def load_boundaries(path):
    import csv
    rows = []
    with open(path) as f:
        for r in csv.DictReader(f):
            rows.append(r)
    return rows


def classify_snapshot(rows, pos, nb, pca_km, edge_angle):
    cont = {}
    owner = {}
    for r in rows:
        cont[int(r["cell"])] = r["continental"] == "1"
        owner[int(r["cell"])] = int(r["plate"])
    omega = {}
    for r in rows:
        omega[int(r["plate"])] = np.array([float(r["wx"]), float(r["wy"]), float(r["wz"])])
    # Segments with at least one subducting cell (seen from the subducting plate).
    trench_segments = {(int(r["other"]), int(r["plate"]), r["segment"]) for r in rows if r["kind"] == "t"}
    by_pair = defaultdict(list)
    for i, r in enumerate(rows):
        by_pair[(int(r["plate"]), int(r["other"]))].append(i)
    cell_len = R * edge_angle / 1.1
    out = []
    max_angle = pca_km / R if pca_km > 0 else 0
    for (p, o), idx in by_pair.items():
        cells = np.array([int(rows[i]["cell"]) for i in idx])
        P = pos[cells]
        for k, i in enumerate(idx):
            r = rows[i]
            cell = int(r["cell"])
            conv, strike = float(r["conv"]), float(r["strike"])
            if pca_km > 0 and o in omega:
                x = pos[cell]
                near = P[np.arccos(np.clip(P @ x, -1, 1)) < max_angle]
                if len(near) >= 3:
                    d = near - near.mean(0)
                    d = d - np.outer(d @ x, x)
                    _, _, vt = np.linalg.svd(d, full_matrices=False)
                    t = vt[0] - x * (vt[0] @ x)
                    t /= np.linalg.norm(t)
                    rel = np.cross(omega[p] - omega[o], x) * R * 1e5 / 1e6  # cm/yr
                    along = abs(rel @ t)
                    across = math.sqrt(max(0.0, rel @ rel - along * along))
                    # keep the model's sign of the normal component
                    conv = math.copysign(across, conv) if conv != 0 else across
                    strike = along
            other_cont = any(cont.get(n, False) for n in nb[cell] if owner.get(n) == o)
            continental = cont[cell] or other_cont
            v = math.hypot(conv, strike)
            angle = math.degrees(math.atan2(abs(conv), abs(strike))) if v > 0 else 90.0
            overriding = r["overriding"]
            slab = float(r["slab_km"])
            if r["kind"] == "t":
                over_cont = other_cont
                cls = "subduction, under continent" if over_cont else "subduction, ocean-ocean"
            elif (overriding == r["plate"] and slab > 0 and conv > -0.1
                  and r["kind"] == "o" and (p, o, r["segment"]) in trench_segments):
                cls = "subduction, under continent" if cont[cell] else "subduction, ocean-ocean"
            elif angle < 20:
                cls = "transform, " + ("continental" if continental else "oceanic")
            elif conv < 0:
                cls = "divergent, " + ("continental rift" if continental else "ridge")
            else:
                if cont[cell] and other_cont:
                    cls = "convergent, continent-continent"
                elif continental:
                    cls = "convergent, ocean-continent, no slab"
                else:
                    cls = "convergent, ocean-ocean, no slab"
            out.append((cls, cell_len / 2, v, conv, angle, r["kind"]))
    return out


def summarise(items, title, n_snap):
    total = sum(i[1] for i in items)
    by = defaultdict(float)
    sp = defaultdict(list)
    for cls, l, v, *_ in items:
        by[cls] += l
        sp[cls].append((v, l))
    print(f"\n## {title}: total {total/n_snap/1e3:.0f} thousand km (mean per snapshot)")
    print("| class | length (10^3 km) | share | median speed (cm/yr) | < 2 cm/yr |")
    print("|---|---|---|---|---|")
    for c in sorted(by, key=lambda c: -by[c]):
        vs = sorted(sp[c])
        acc = 0
        med = 0
        for v, l in vs:
            acc += l
            if acc >= by[c] / 2:
                med = v
                break
        slow = sum(l for v, l in vs if v < 2) / by[c]
        print(f"| {c} | {by[c]/n_snap/1e3:.1f} | {100*by[c]/total:.0f}% | {med:.1f} | {100*slow:.0f}% |")
    allv = sorted((i[2], i[1]) for i in items)
    acc = 0
    for v, l in allv:
        acc += l
        if acc >= total / 2:
            break
    slow = sum(l for vv, l in allv if vv < 2) / total
    mean_n = sum(abs(i[3]) * i[1] for i in items) / total
    print(f"all: median speed {v:.1f} cm/yr, < 2 cm/yr {100*slow:.0f}%, mean |normal| {mean_n:.2f} cm/yr")
    hist = [0.0] * 9
    for cls, l, v, conv, angle, kind in items:
        if cls.startswith("subduction"):
            continue
        hist[min(8, int(angle // 10))] += l
    t = sum(hist)
    print("non-subduction length by angle from strike (10 deg bins, 0 = pure strike-slip): "
          + " ".join(f"{100*h/t:.0f}" for h in hist))
    hs = [0.0] * 9
    for cls, l, v, conv, angle, kind in items:
        if cls.startswith("subduction"):
            hs[min(8, int(angle // 10))] += l
    t = sum(hs)
    if t:
        print("subduction length by angle from strike (10 deg bins): " + " ".join(f"{100*h/t:.0f}" for h in hs))
    # speed bands
    bands = [0, 1, 2, 5, 8, 12, 1e9]
    hb = [0.0] * (len(bands) - 1)
    for i in items:
        for b in range(len(bands) - 1):
            if bands[b] <= i[2] < bands[b + 1]:
                hb[b] += i[1]
    print("length by relative speed 0-1/1-2/2-5/5-8/8-12/>12 cm/yr (%): "
          + " ".join(f"{100*h/total:.0f}" for h in hb))
    return by, total


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--from", dest="t0", type=float, default=500)
    ap.add_argument("--to", dest="t1", type=float, default=1000)
    ap.add_argument("--pca-km", type=float, default=0)
    ap.add_argument("--per-run", action="store_true")
    ap.add_argument("runs", nargs="+")
    a = ap.parse_args()
    allitems, n_all = [], 0
    for run in a.runs:
        pos, nb, edge = load_mesh(os.path.join(run, "mesh.csv"))
        items, n = [], 0
        for path in sorted(glob.glob(os.path.join(run, "boundaries_*.csv"))):
            t = float(re.search(r"boundaries_(\d+)", path).group(1))
            if not (a.t0 <= t <= a.t1):
                continue
            items += classify_snapshot(load_boundaries(path), pos, nb, a.pca_km, edge)
            n += 1
        if a.per_run and n:
            summarise(items, f"{run} ({n} snapshots)", n)
        allitems += items
        n_all += n
    summarise(allitems, f"all runs ({n_all} snapshots, {a.t0:.0f}-{a.t1:.0f} Myr, "
              f"{'smoothed model normal' if a.pca_km == 0 else f'PCA normal over {a.pca_km:.0f} km'})", n_all)


if __name__ == "__main__":
    main()
