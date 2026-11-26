"""Plate shape metrics on the icosphere (#123), shared by the simulation and Earth analyses.

Per plate (cells of one id): area (fraction of the surface), inscribed radius (cells; erosion
steps until the plate vanishes), share of cells within 2 cells of its edge, intrinsic length L
(longest shortest path inside the plate, km, double sweep), mean width W = area / L, aspect L/W,
bend = L / great-circle distance between the ends (U/L shapes > ~1.4), and the narrowest neck:
the smallest erosion depth k at which the plate falls into two parts that each stay at least 3
erosion steps thicker than k (two bulbs joined by a neck ~2k cells wide).
"""
import math
from collections import deque

import numpy as np


def load_mesh(path):
    xyz, neighbours = [], []
    with open(path) as f:
        next(f)
        for line in f:
            c, x, y, z, nb = line.rstrip('\n').split(',')
            xyz.append((float(x), float(y), float(z)))
            neighbours.append([int(v) for v in nb.split(';')])
    xyz = np.array(xyz)
    # mean spacing in km
    a = xyz[[i for i in range(0, len(xyz), 97)]]
    b = xyz[[neighbours[i][0] for i in range(0, len(xyz), 97)]]
    spacing = float(np.mean(np.arccos(np.clip((a * b).sum(1), -1, 1)))) * 6371.0
    return xyz, neighbours, spacing


def _bfs(start, inside, neighbours):
    dist = {start: 0}
    q = deque([start])
    while q:
        c = q.popleft()
        for m in neighbours[c]:
            if m in inside and m not in dist:
                dist[m] = dist[c] + 1
                q.append(m)
    return dist


def _components(cells, neighbours):
    cells = set(cells)
    seen, out = set(), []
    for s in sorted(cells):
        if s in seen:
            continue
        seen.add(s)
        comp, q = [s], [s]
        while q:
            c = q.pop()
            for m in neighbours[c]:
                if m in cells and m not in seen:
                    seen.add(m)
                    comp.append(m)
                    q.append(m)
        out.append(comp)
    return out


def metrics(cells, plate_of, pid, neighbours, xyz, spacing, n_total):
    cells = max(_components(cells, neighbours), key=len)
    inside = set(cells)
    # distance to the edge (1 for edge cells)
    dt = {}
    q = deque()
    for c in cells:
        if any(m not in inside for m in neighbours[c]):
            dt[c] = 1
            q.append(c)
    while q:
        c = q.popleft()
        for m in neighbours[c]:
            if m in inside and m not in dt:
                dt[m] = dt[c] + 1
                q.append(m)
    for c in cells:  # plates covering everything (no edge)
        dt.setdefault(c, 1)
    r_in = max(dt.values())
    thin = sum(1 for c in cells if dt[c] <= 2) / len(cells)
    d0 = _bfs(cells[0], inside, neighbours)
    p1 = max(d0, key=lambda c: (d0[c], -c))
    d1 = _bfs(p1, inside, neighbours)
    p2 = max(d1, key=lambda c: (d1[c], -c))
    length = d1[p2] * spacing * 1.0
    area_km2 = len(cells) / n_total * 4 * math.pi * 6371.0 ** 2
    width = area_km2 / max(length, spacing)
    chord = math.acos(max(-1.0, min(1.0, float(np.dot(xyz[p1], xyz[p2]))))) * 6371.0
    bend = length / max(chord, spacing)
    neck = None
    for k in range(1, r_in):
        core = [c for c in cells if dt[c] > k]
        bulbs = [comp for comp in _components(core, neighbours) if max(dt[c] for c in comp) >= k + 3]
        if len(bulbs) >= 2:
            neck = k
            break
    return dict(cells=len(cells), area=len(cells) / n_total, r_in=r_in, thin=thin,
                length=length, width=width, aspect=length / width, bend=bend, neck=neck)


def plate_table(plate_of, neighbours, xyz, spacing, min_area=0.002):
    n = len(plate_of)
    members = {}
    for c, p in enumerate(plate_of):
        members.setdefault(p, []).append(c)
    out = {}
    for p, cells in sorted(members.items()):
        if len(cells) / n < min_area:
            continue
        out[p] = metrics(cells, plate_of, p, neighbours, xyz, spacing, n)
    return out


def is_narrow(m):
    """Long and narrow, or two bulbs joined by a neck at most ~4 cells (~450 km) wide."""
    return m['aspect'] >= 4.0 or (m['neck'] is not None and m['neck'] <= 2)
