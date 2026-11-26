"""Map of the plates coloured by the overload of their most overloaded section (#123).
usage: overload_map.py DIR TIME OUT.png   (DIR with mesh.csv, plate_maps.csv, plate_sections.csv)"""
import csv, os, sys
import numpy as np
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from shapes import load_mesh, plate_table, is_narrow

d, time, out = sys.argv[1], float(sys.argv[2]), sys.argv[3]
xyz, nb, spacing = load_mesh(os.path.join(d, 'mesh.csv'))
ids = None
for line in open(os.path.join(d, 'plate_maps.csv')):
    v = line.split(',', 1)
    if abs(float(v[0]) - time) < 1e-6:
        ids = np.array([int(x) for x in line.rstrip().split(',')[1:]])
over = {int(r['plate']): float(r['overload']) for r in csv.DictReader(open(os.path.join(d, 'plate_sections.csv'))) if abs(float(r['time']) - time) < 1e-6}
table = plate_table(list(ids), nb, xyz, spacing)
lat = np.degrees(np.arcsin(xyz[:, 2])); lon = np.degrees(np.arctan2(xyz[:, 1], xyz[:, 0]))
val = np.array([over.get(p, np.nan) for p in ids])
edge = np.array([any(ids[m] != ids[c] for m in nb[c]) for c in range(len(ids))])
fig, ax = plt.subplots(figsize=(12, 6.2))
sc = ax.scatter(lon, lat, c=np.log10(np.clip(val, 0.03, 30)), cmap='coolwarm', vmin=-1, vmax=1, s=4 if len(ids) < 20000 else 1.2, marker='h')
ax.scatter(lon[edge], lat[edge], c='k', s=1.5 if len(ids) < 20000 else 0.5)
for p, m in table.items():
    cells = np.where(ids == p)[0]
    v = xyz[cells].mean(0); v /= np.linalg.norm(v)
    la, lo = np.degrees(np.arcsin(v[2])), np.degrees(np.arctan2(v[1], v[0]))
    s = over.get(p)
    txt = f"{p}\n{s:.2f}" if s is not None else str(p)
    if is_narrow(m):
        txt += "\nnarrow"
    ax.text(lo, la, txt, ha='center', va='center', fontsize=7, weight='bold')
cb = fig.colorbar(sc, ax=ax, shrink=0.7)
cb.set_label('overload S of the plate\'s most overloaded section (log10; > 0 breaks)')
ax.set_xlim(-180, 180); ax.set_ylim(-90, 90); ax.set_aspect('equal')
ax.set_title(f"{d.rstrip('/').split('/')[-1]}, {time:.0f} Myr: plate id / overload")
fig.tight_layout(); fig.savefig(out, dpi=110)
