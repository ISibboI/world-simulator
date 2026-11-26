"""Issue #6 check: share of young oceanic crust (< 3 Myr) at convergent boundaries.

Usage: python3 -I young_convergent.py RUN_DIR  (RUN_DIR/seed-S/{cells,boundaries}_TTTTT.csv)
"""
import csv
import glob
import os
import sys

run_dir = sys.argv[1]
YOUNG = 3.0
rows = []
for seed_dir in sorted(glob.glob(os.path.join(run_dir, "seed-*"))):
    seed = os.path.basename(seed_dir)
    for cells_path in sorted(glob.glob(os.path.join(seed_dir, "**", "cells_*.csv"), recursive=True)):
        t = int(os.path.basename(cells_path)[6:11])
        if t == 0:
            continue
        bpath = cells_path.replace("cells_", "boundaries_")
        young = set()
        oceanic = 0
        with open(cells_path) as f:
            for r in csv.DictReader(f):
                if r["cls"] == "2":
                    oceanic += 1
                    if float(r["age"]) < YOUNG:
                        young.add(int(r["cell"]))
        conv0, conv05, boundary, ridge = set(), set(), set(), set()
        kinds = {}
        all_boundary, all_conv0 = set(), set()
        with open(bpath) as f:
            for r in csv.DictReader(f):
                c = int(r["cell"])
                conv = float(r["conv"])
                all_boundary.add(c)
                if conv > 0.0:
                    all_conv0.add(c)
                if c not in young:
                    continue
                boundary.add(c)
                if conv > 0.0:
                    conv0.add(c)
                    kinds.setdefault(r["kind"], set()).add(c)
                if conv > 0.5:
                    conv05.add(c)
                if r["kind"] == "r":
                    ridge.add(c)
        rows.append((seed, t, len(young), oceanic, len(boundary), len(conv0), len(conv05),
                     len(ridge), {k: len(v) for k, v in kinds.items()},
                     len(all_boundary), len(all_conv0)))

print("seed t young young/ocean% at_boundary% conv>0% conv>0.5% ridge% conv_kinds | boundary_conv>0%")
tot = [0] * 6
for seed, t, n, oc, b, c0, c05, rg, kinds, ab, ac in rows:
    p = lambda x: 100.0 * x / max(n, 1)
    print(f"{seed} {t:4d} {n:5d} {100*n/oc:5.2f} {p(b):5.1f} {p(c0):5.1f} {p(c05):5.1f} {p(rg):5.1f} {kinds} | {100*ac/ab:5.1f}")
    for i, x in enumerate((n, b, c0, c05, rg)):
        tot[i] += x
print(f"pooled: young {tot[0]}, at boundary {100*tot[1]/tot[0]:.1f}%, convergent (>0) {100*tot[2]/tot[0]:.2f}%, "
      f"convergent (>0.5 cm/yr) {100*tot[3]/tot[0]:.2f}%, ridge {100*tot[4]/tot[0]:.1f}%")
by_seed = {}
for seed, t, n, oc, b, c0, c05, rg, kinds, ab, ac in rows:
    s = by_seed.setdefault(seed, [0, 0, 0])
    s[0] += n; s[1] += c0; s[2] += c05
for seed, (n, c0, c05) in by_seed.items():
    print(f"{seed}: convergent >0 {100*c0/n:.2f}%, >0.5 {100*c05/n:.2f}%")
