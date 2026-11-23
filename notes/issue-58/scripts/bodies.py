"""List closed below-sea-level bodies >= 20 cells in one cell dump: size, pass, mean/min elevation, oceanic share, lake? (precip/evap)
usage: bodies.py SEEDDIR TIME"""
import csv, heapq, sys
from collections import deque
import numpy as np
sd, t = sys.argv[1], int(sys.argv[2])
with open(f'{sd}/mesh.csv') as f:
    r = csv.reader(f); h = next(r); i = h.index('neighbors'); nb = [list(map(int, row[i].split(';'))) for row in r]
with open(f'{sd}/cells_{t:05d}.csv') as f:
    r = csv.reader(f); h = next(r); rows = list(r)
col = lambda n: np.array([float(x[h.index(n)]) for x in rows])
e, oc, cls, lat, lon, pr = col('elev'), col('ocean') > 0.5, col('cls'), col('lat'), col('lon'), col('precip')
n = len(e); pas = np.full(n, np.inf); heap = [(e[c], c) for c in np.nonzero(oc)[0]]
for v, c in heap: pas[c] = v
heapq.heapify(heap); done = np.zeros(n, bool)
while heap:
    lv, c = heapq.heappop(heap)
    if done[c]: continue
    done[c] = True
    for m in nb[c]:
        v = max(e[m], lv)
        if not done[m] and v < pas[m]: pas[m] = v; heapq.heappush(heap, (v, m))
inland = (e < 0) & ~oc; seen = np.zeros(n, bool)
for c in np.nonzero(inland)[0]:
    if seen[c]: continue
    comp = []; q = deque([c]); seen[c] = True
    while q:
        x = q.popleft(); comp.append(x)
        for m in nb[x]:
            if inland[m] and not seen[m]: seen[m] = True; q.append(m)
    if len(comp) < 20: continue
    comp = np.array(comp)
    print(f'{len(comp):5d} cells pass {pas[comp].min():6.0f} m  mean {e[comp].mean():6.0f} min {e[comp].min():6.0f}  oceanic {np.mean(cls[comp]==2):.2f} precip {pr[comp].mean():.2f}  at lat {lat[comp].mean():.0f} lon {lon[comp].mean():.0f}')
