"""Lake persistence and flicker causes from the per-step depression log (`--lake-log-from`).
usage: lakes.py DIR [DIR...]   (DIR/seed-S/{lakes.csv,mesh.csv}); prints per seed and the mean.
Lake cells: lake depth > 0. Lakes: connected groups of lake cells. A lake continues into the
next step if it overlaps a lake there; lifetimes follow the largest overlap."""
import sys, os, glob
import numpy as np
from pits import mesh

filled = []
starts = []

def read(path):
    steps = []
    filled.clear()
    starts.clear()
    for line in open(path):
        parts = line.strip().split(',')
        starts.append(set())
        d = {}
        for p in parts[1:]:
            if p.startswith('s'):
                starts[-1].add(int(p[1:]))
                continue
            if p.startswith('f:'):
                filled.append(tuple(int(x) for x in p.split(':')[1:]))
                continue
            c, lake, spill = p.split(':')
            d[int(c)] = (float(lake), float(spill))
        steps.append((float(parts[0]), d))
    return steps

def components(cells, nb):
    cells = set(cells); comp = {}; out = []
    for c in sorted(cells):
        if c in comp: continue
        k = len(out); stack = [c]; comp[c] = k; members = []
        while stack:
            x = stack.pop(); members.append(x)
            for n in nb[x]:
                if n >= 0 and n in cells and n not in comp:
                    comp[n] = k; stack.append(n)
        out.append(members)
    return comp, out

def seed(d):
    nb = mesh(d); steps = read(os.path.join(d, 'lakes.csv'))
    lakes = [set(c for c, (l, s) in st.items() if l > 0) for _, st in steps]
    dep = [set(st) for _, st in steps]
    r = {}
    for lag in (1, 10):
        r[f'lake cells still lake after {lag} Myr (%)'] = 100 * np.mean([len(a & b) / max(len(a), 1) for a, b in zip(lakes, lakes[lag:])])
    r['depression cells still depression after 1 Myr (%)'] = 100 * np.mean([len(a & b) / max(len(a), 1) for a, b in zip(dep, dep[1:])])
    # causes of lost lake cells
    dried = vanished = lost = 0; sp_v = []; sp_all = []
    for (t, a), (_, b), la, lb in zip(steps, steps[1:], lakes, lakes[1:]):
        for c in la - lb:
            lost += 1
            if c in b: dried += 1
            else:
                vanished += 1; sp_v.append(a[c][1])
        sp_all += [a[c][1] for c in la]
    r['lost lake cells: depression vanished (%)'] = 100 * vanished / max(lost, 1)
    r['lost lake cells: depression stays, dry (%)'] = 100 * dried / max(lost, 1)
    r['spill depth of lake cells, median (m)'] = np.median(sp_all) if sp_all else 0
    r['spill depth where depression vanished, median (m)'] = np.median(sp_v) if sp_v else 0
    if any(starts):
        # end of step t -> start of t+1 (tectonics etc.), start -> end (erosion, deposition)
        tect = [len(a & b) / max(len(a), 1) for a, b in zip(dep, starts[1:])]
        ero = [len(a & b) / max(len(a), 1) for a, b in zip(starts, dep)]
        r['end-of-step depression cells still depression at the next start (%)'] = 100 * np.mean(tect)
        r['start depression cells still depression at the end (%)'] = 100 * np.mean(ero)
        r['depression cells at step start'] = np.mean([len(x) for x in starts])
        r['depression cells new at step start (% of start)'] = 100 * np.mean([len(b - a) / max(len(b), 1) for a, b in zip(dep, starts[1:])])
        r['depression cells new at step end (% of end)'] = 100 * np.mean([len(b - a) / max(len(b), 1) for a, b in zip(starts, dep)])
    r['lake cells per step'] = np.mean([len(l) for l in lakes])
    if filled:
        r['depression cells filled by sediment per step'] = np.mean([f[1] for f in filled])
        r['lake cells lost per step'] = lost / max(len(steps) - 1, 1)
    r['depression cells per step'] = np.mean([len(x) for x in dep])
    # lake objects and lifetimes
    comps = [components(l, nb) for l in lakes]
    r['lakes per step'] = np.mean([len(c[1]) for c in comps])
    r['single-cell lakes (% of lakes)'] = 100 * np.mean([np.mean([len(m) == 1 for m in c[1]]) if c[1] else 0 for c in comps])
    r['lake size, cells (area-weighted mean)'] = np.mean([sum(len(m) ** 2 for m in c[1]) / max(sum(len(m) for m in c[1]), 1) for c in comps])
    # lifetimes: id propagation by largest overlap
    ids = []; nid = 0; born = {}; ended = []
    for i, (comp, members) in enumerate(comps):
        cur = []
        for k, m in enumerate(members):
            best = None
            if i > 0:
                prev_comp, prev_ids = comps[i - 1][0], ids[i - 1]
                cnt = {}
                for c in m:
                    if c in prev_comp: cnt[prev_comp[c]] = cnt.get(prev_comp[c], 0) + 1
                if cnt:
                    best = prev_ids[max(cnt, key=lambda x: (cnt[x], -x))]
            if best is None or best in cur:
                best = nid; nid += 1; born[best] = i
            cur.append(best)
        if i > 0:
            for old in set(ids[i - 1]) - set(cur):
                ended.append((i - born[old], len(comps[i - 1][1][ids[i - 1].index(old)])))
        ids.append(cur)
    # censor: lakes alive at the end are excluded except as a lower bound
    life = np.array([l for l, _ in ended]) if ended else np.array([0])
    area = np.array([a for _, a in ended]) if ended else np.array([0])
    r['lake lifetime, mean (Myr)'] = life.mean()
    r['lake lifetime, area-weighted mean (Myr)'] = (life * area).sum() / max(area.sum(), 1)
    alive = [born[x] for x in ids[-1]]
    n = len(comps)
    r['lake cells in lakes older than 10 Myr (%)'] = 100 * np.mean([
        sum(len(m) for m, x in zip(comps[i][1], ids[i]) if i - born[x] >= 10) / max(len(lakes[i]), 1) for i in range(10, n)])
    return r

if __name__ == '__main__':
    res = []
    for d in sys.argv[1:]:
        for s in sorted(glob.glob(os.path.join(d, 'seed-*'))):
            if os.path.exists(os.path.join(s, 'lakes.csv')):
                r = seed(s); res.append(r)
                print(os.path.basename(s), ' | '.join(f'{k}={v:.2f}' for k, v in r.items()))
    for k in res[0]:
        print(f'{k}: {np.mean([r[k] for r in res]):.2f}')
