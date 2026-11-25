"""Plate shape statistics from cell dumps (#123): per plate area, compactness and width.
usage: plate_shapes.py T0 T1 label=DIR [label=DIR ...]   (DIR holds seed-S/cells_TTTTT.csv, mesh.csv)
Per plate (> 0.5% of the surface): area fraction; compactness = 4 pi A / P^2 on the sphere
(1 for a disc; P = boundary edges x mean edge length); inscribed radius = number of
morphological erosion steps (cells) until the plate vanishes; elongation = sqrt(A)/(2 r_in)
in cells. Thin plates: more than 60% of their cells within 2 cells (~220 km) of their edge, area > 1%."""
import csv, glob, math, os, statistics as st, sys

def load_mesh(path):
    neighbours = []
    for row in csv.DictReader(open(path)):
        neighbours.append([int(x) for x in row['neighbors'].split(';')])
    return neighbours

def shapes(cells_path, neighbours):
    plate = [int(r['plate']) for r in csv.DictReader(open(cells_path))]
    n = len(plate)
    members = {}
    for c, p in enumerate(plate):
        members.setdefault(p, []).append(c)
    out = []
    for p, cells in members.items():
        area = len(cells) / n
        if area < 0.005:
            continue
        perimeter = sum(1 for c in cells for m in neighbours[c] if plate[m] != p)
        # perimeter edges ~ boundary length in cell spacings
        a_cells = len(cells)
        compact = 4 * math.pi * a_cells * (math.sqrt(3) / 2) / (perimeter / 1.1) ** 2
        alive = set(cells)
        radius = 0
        thin = 0
        while alive:
            edge = {c for c in alive if any(m not in alive for m in neighbours[c])}
            alive -= edge
            radius += 1
            if radius <= 2:
                thin += len(edge)
        elong = math.sqrt(a_cells) / (2 * radius)
        out.append((area, compact, radius, elong, thin / a_cells))
    return out

args = sys.argv[1:]
t0, t1 = float(args[0]), float(args[1])
for spec in args[2:]:
    label, base = spec.split('=', 1)
    rows = []
    narrow = []
    for seed_dir in sorted(glob.glob(os.path.join(base, 'seed-*'))):
        mesh = os.path.join(seed_dir, 'mesh.csv')
        if not os.path.exists(mesh):
            continue
        neighbours = load_mesh(mesh)
        for path in sorted(glob.glob(os.path.join(seed_dir, 'cells_*.csv'))):
            t = float(os.path.basename(path)[6:11])
            if t < t0 or t > t1:
                continue
            s = shapes(path, neighbours)
            rows.extend(s)
            narrow.append(sum(1 for a, c, r, e, t in s if t > 0.6 and a > 0.01))
    if not rows:
        continue
    comp = [r[1] for r in rows]
    rad = [r[2] for r in rows if r[0] > 0.01]
    elong = [r[3] for r in rows if r[0] > 0.01]
    thin = [r[4] for r in rows if r[0] > 0.01]
    print(f"{label}: plates > 0.5%: {len(rows)/max(len(narrow),1):.1f} per snapshot | compactness median {st.median(comp):.2f} "
          f"(p10 {sorted(comp)[len(comp)//10]:.2f}) | plates > 1%: inscribed radius median {st.median(rad):.1f} cells, "
          f"elongation median {st.median(elong):.2f} (p90 {sorted(elong)[9*len(elong)//10]:.2f}) | share within 2 cells of the edge: median "
          f"{st.median(thin):.2f} | thin plates (> 60% within 2 cells of the edge, > 1%) {st.mean(narrow):.2f} per snapshot")
