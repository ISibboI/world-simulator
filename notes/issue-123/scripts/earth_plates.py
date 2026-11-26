"""Earth's present plates (PB2002, Bird 2003) rasterised on the model's icosphere, with the same
shape metrics as the simulation (shapes.py). Usage: earth_plates.py MESH.csv PB2002_boundaries.json PB2002_plates.json
Boundaries are walls (cells within half a cell spacing of a boundary line); the plates are the
connected regions between them, wall cells joined to their neighbours' plate; plates smaller
than 0.02% (~8 cells at 6 subdivisions) are lost at this resolution."""
import json, math, sys
from collections import Counter
import numpy as np
from matplotlib.path import Path
sys.path.insert(0, __file__.rsplit('/', 1)[0])
from shapes import load_mesh, plate_table, is_narrow, _components

xyz, nb, spacing = load_mesh(sys.argv[1])
n = len(xyz)
lat = np.degrees(np.arcsin(xyz[:, 2])); lon = np.degrees(np.arctan2(xyz[:, 1], xyz[:, 0]))

def to_xyz(lo, la):
    lo, la = math.radians(lo), math.radians(la)
    return np.array([math.cos(la) * math.cos(lo), math.cos(la) * math.sin(lo), math.sin(la)])

wall = np.zeros(n, bool)
# nearest-cell lookup by brute force on a coarse bucket grid
from scipy.spatial import cKDTree
tree = cKDTree(xyz)
for feat in json.load(open(sys.argv[2]))['features']:
    g = feat['geometry']
    lines = g['coordinates'] if g['type'] == 'MultiLineString' else [g['coordinates']]
    for line in lines:
        for (a, b) in zip(line[:-1], line[1:]):
            pa, pb = to_xyz(*a[:2]), to_xyz(*b[:2])
            ang = math.acos(max(-1, min(1, float(pa @ pb))))
            steps = max(1, int(ang * 6371 / 10))
            for t in np.linspace(0, 1, steps + 1):
                p = pa * (1 - t) + pb * t
                p /= np.linalg.norm(p)
                wall[tree.query(p)[1]] = True
plate = -np.ones(n, int)
comps = _components([c for c in range(n) if not wall[c]], nb)
for i, comp in enumerate(comps):
    for c in comp:
        plate[c] = i
for _ in range(10):
    changed = False
    for c in range(n):
        if plate[c] < 0:
            votes = Counter(plate[m] for m in nb[c] if plate[m] >= 0)
            if votes:
                plate[c] = votes.most_common(1)[0][0]; changed = True
    if not changed:
        break
# names
polys = []
for feat in json.load(open(sys.argv[3]))['features']:
    g = feat['geometry']
    rings = [g['coordinates'][0]] if g['type'] == 'Polygon' else [p[0] for p in g['coordinates']]
    polys.append((feat['properties']['PlateName'], [Path(np.array(r)[:, :2]) for r in rings]))
names = {}
for p in set(plate):
    cells = [c for c in range(n) if plate[c] == p]
    votes = Counter()
    for c in cells[:: max(1, len(cells) // 40)]:
        for name, paths in polys:
            if any(pt.contains_point((lon[c], lat[c])) for pt in paths):
                votes[name] += 1
    names[p] = votes.most_common(1)[0][0] if votes else '?'
table = plate_table(list(plate), nb, xyz, spacing, min_area=0.0002)
rows = sorted(table.items(), key=lambda kv: -kv[1]['area'])
print(f"spacing {spacing:.0f} km; {len(rows)} plates >= 0.02%")
print("name area% r_in thin length_km width_km aspect bend neck narrow")
for p, m in rows:
    print(f"{names[p][:22]:22s} {100*m['area']:6.2f} {m['r_in']:3d} {m['thin']:.2f} {m['length']:6.0f} {m['width']:6.0f} {m['aspect']:5.1f} {m['bend']:4.2f} {m['neck']} {int(is_narrow(m))}")
