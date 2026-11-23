"""Closed depressions below sea level by the height of their pass (branch cell dumps with 'ocean').
usage: passes.py DIR   (DIR/seed-S/cells_*.csv, mesh.csv), window 500-1000"""
import csv, glob, heapq, os, re, sys
from collections import deque
import numpy as np

def cols(path, names):
    with open(path) as f:
        r = csv.reader(f); h = next(r); idx = [h.index(n) for n in names]
        rows = list(r)
    return [np.array([float(row[i]) for row in rows]) for i in idx]

edges = [0, 10, 50, 100, 200, 500, 1e9]
tot = {}
nsnap = 0
for sd in sorted(glob.glob(os.path.join(sys.argv[1], 'seed-*'))):
    with open(os.path.join(sd, 'mesh.csv')) as f:
        r = csv.reader(f); h = next(r); i = h.index('neighbors')
        nb = [list(map(int, row[i].split(';'))) for row in r]
    for p in sorted(glob.glob(os.path.join(sd, 'cells_*.csv'))):
        t = float(re.search(r'cells_(\d+)', p).group(1))
        if not (500 < t <= 1000): continue
        nsnap += 1
        e, oc = cols(p, ['elev', 'ocean'])
        n = len(e); ocean = oc > 0.5
        pas = np.full(n, np.inf); heap = []
        for c in np.nonzero(ocean)[0]:
            pas[c] = e[c]; heap.append((e[c], c))
        heapq.heapify(heap); done = np.zeros(n, bool)
        while heap:
            lv, c = heapq.heappop(heap)
            if done[c]: continue
            done[c] = True
            for m in nb[c]:
                v = max(e[m], lv)
                if not done[m] and v < pas[m]:
                    pas[m] = v; heapq.heappush(heap, (v, m))
        inland = (e < 0) & ~ocean
        seen = np.zeros(n, bool)
        for c in np.nonzero(inland)[0]:
            if seen[c]: continue
            comp = []; q = deque([c]); seen[c] = True
            while q:
                x = q.popleft(); comp.append(x)
                for m in nb[x]:
                    if inland[m] and not seen[m]:
                        seen[m] = True; q.append(m)
            pv = min(pas[x] for x in comp)
            k = next(j for j in range(len(edges) - 1) if edges[j] <= pv < edges[j + 1])
            size = 'big(>=50)' if len(comp) >= 50 else ('mid(4-49)' if len(comp) > 3 else 'small(1-3)')
            key = (size, k)
            a = tot.setdefault(key, [0, 0])
            a[0] += 1; a[1] += len(comp)
print(f'snapshots {nsnap}; per snapshot: bodies / cells, by size and pass height above sea level (m)')
for size in ['small(1-3)', 'mid(4-49)', 'big(>=50)']:
    row = []
    for k in range(len(edges) - 1):
        a = tot.get((size, k), [0, 0])
        row.append(f'{edges[k]:g}-{edges[k+1]:g}: {a[0]/nsnap:.1f}/{a[1]/nsnap:.0f}')
    print(size, ' | '.join(row))
