"""Plate shapes, narrow-plate lifetimes and section overloads of simulation runs (#123).
usage: sim_shapes.py T0 T1 label=DIR [label=DIR ...]
DIR holds seed-S/{mesh.csv, plate_maps.csv, plate_sections.csv} (run with --plate-log 10
--cell-dump). Plates >= 0.2% of the surface. Narrow: aspect >= 5 or a neck <= 2 erosion steps
between two bulbs (shapes.is_narrow). Episodes: consecutive map rows in which a plate id is
narrow (from all of the run; counted if they overlap [T0, T1])."""
import csv, glob, os, statistics as st, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from shapes import load_mesh, plate_table, is_narrow


def seed_data(d, mesh_cache={}):
    mesh = os.path.join(d, 'mesh.csv')
    key = os.path.getsize(mesh)
    if key not in mesh_cache:
        mesh_cache[key] = load_mesh(mesh)
    xyz, nb, spacing = mesh_cache[key]
    snaps = []
    for line in open(os.path.join(d, 'plate_maps.csv')):
        v = line.rstrip().split(',')
        t = float(v[0]); ids = [int(x) for x in v[1:]]
        snaps.append((t, plate_table(ids, nb, xyz, spacing)))
    sections = {}
    path = os.path.join(d, 'plate_sections.csv')
    if os.path.exists(path):
        for r in csv.DictReader(open(path)):
            sections[(float(r['time']), int(r['plate']))] = r
    return snaps, sections


def summarise(label, base, t0, t1):
    rows, counts, narrow_counts, episodes, ov_n, ov_c, big = [], [], [], [], [], [], []
    per_seed = []
    for d in sorted(glob.glob(os.path.join(base, 'seed-*'))):
        if not os.path.exists(os.path.join(d, 'plate_maps.csv')):
            continue
        snaps, sections = seed_data(d)
        first = len(narrow_counts)
        dt = snaps[1][0] - snaps[0][0] if len(snaps) > 1 else 10
        open_ep = {}
        for t, table in snaps:
            narrow_now = {p for p, m in table.items() if is_narrow(m) and m['area'] >= 0.005}
            for p in list(open_ep):
                if p not in narrow_now:
                    start = open_ep.pop(p)
                    end_reason = 'gone' if p not in table else 'compact'
                    if t > t0 and start <= t1:
                        episodes.append((t - start, end_reason))
            for p in narrow_now:
                open_ep.setdefault(p, t)
            if t0 <= t <= t1:
                counts.append(len(table))
                big.append(sum(1 for m in table.values() if m['area'] >= 0.01))
                narrow_counts.append(len(narrow_now))
                for p, m in table.items():
                    rows.append(m)
                    s = sections.get((t, p))
                    if s is not None and m['area'] >= 0.005:
                        (ov_n if is_narrow(m) else ov_c).append(float(s['overload']))
        if len(narrow_counts) > first:
            per_seed.append(st.mean(narrow_counts[first:]))
        for p, start in open_ep.items():
            if start <= t1:
                episodes.append((snaps[-1][0] + dt - start, 'open'))
    if not rows:
        return
    sel = [m for m in rows if m['area'] >= 0.005]
    def q(v, f):
        v = sorted(v); return v[min(len(v) - 1, int(f * len(v)))] if v else float('nan')
    print(f"== {label} ({t0:.0f}-{t1:.0f} Myr, {len(counts)} maps)")
    print(f"plates >= 0.2%: {st.mean(counts):.1f}, >= 1%: {st.mean(big):.1f} per map; "
          f"narrow plates (>= 0.5%) per map {st.mean(narrow_counts):.2f} (seeds {min(per_seed):.2f}-{max(per_seed):.2f})")
    print(f"plates >= 0.5%: aspect median {st.median(m['aspect'] for m in sel):.1f} p90 {q([m['aspect'] for m in sel], .9):.1f}; "
          f"bend median {st.median(m['bend'] for m in sel):.2f} p90 {q([m['bend'] for m in sel], .9):.2f}; "
          f"with a neck <= 2: {sum(1 for m in sel if m['neck'] is not None and m['neck'] <= 2)/len(sel)*100:.1f}%; "
          f"share within 2 cells of the edge median {st.median(m['thin'] for m in sel):.2f}")
    sizes = sorted((m['area'] for m in rows), reverse=True)
    if episodes:
        d = [e[0] for e in episodes]
        reasons = {r: sum(1 for e in episodes if e[1] == r) for r in ('gone', 'compact', 'open')}
        print(f"narrow episodes: {len(episodes)}, duration median {st.median(d):.0f} Myr, mean {st.mean(d):.0f}, p90 {q(d, .9):.0f}; ended: {reasons}")
    if ov_n and ov_c:
        print(f"largest section overload (F as run): narrow plates median {st.median(ov_n):.2f} (overloaded {sum(o > 1 for o in ov_n)/len(ov_n)*100:.0f}%), "
              f"others median {st.median(ov_c):.2f} (overloaded {sum(o > 1 for o in ov_c)/len(ov_c)*100:.0f}%)")


t0, t1 = float(sys.argv[1]), float(sys.argv[2])
for spec in sys.argv[3:]:
    label, base = spec.split('=', 1)
    summarise(label, base, t0, t1)
