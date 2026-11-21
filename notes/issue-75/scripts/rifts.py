"""Rift-follows-belt statistics per time window from the log line
'rift belts (continental cells, boundary/plates): ever marked a/b of c/d, orogenic weakening x/y,
weak (>= 0.25) u/v | plume rifts failed f, break vs plate motion |cos| m over k rifts'
plus the rift count from the main statistics line. Sums are cumulative; windows are differences
between snapshots.
usage: rifts.py T0 T1 label=DIR ...   (DIR holds seed-S/log.txt)"""
import glob, math, os, re, sys
import numpy as np
T0, T1 = float(sys.argv[1]), float(sys.argv[2])
LINE = re.compile(r'ever marked (\d+)/(\d+) of (\d+)/(\d+), orogenic weakening ([\d.]+)/([\d.]+), weak \(≥ [\d.]+\) (\d+)/(\d+) \| plume rifts failed (\d+), break vs plate motion \|cos\| ([\d.]+) over (\d+) rifts')
RIFTS = re.compile(r'rifts (\d+), initiations')
TIME = re.compile(r'(?:t ?=|time[ =]|at) ?([\d.]+) Myr')

def records(path):
    """Cumulative values at each snapshot, in order."""
    out = []
    rifts = None
    for line in open(path, errors='replace'):
        m = RIFTS.search(line)
        if m and 'trenches' in line:
            rifts = int(m.group(1))
        m = LINE.search(line)
        if m:
            g = [float(x) for x in m.groups()]
            out.append(dict(marked_b=g[0], marked_p=g[1], cont_b=g[2], cont_p=g[3], oro_b=g[4], oro_p=g[5],
                            weak_b=g[6], weak_p=g[7], failed=g[8], align=g[9] * g[10], aligned=g[10], rifts=rifts or 0))
    return out

def welch(a, b):
    a, b = np.array(a), np.array(b)
    va, vb = a.var(ddof=1) / len(a), b.var(ddof=1) / len(b)
    return (b.mean() - a.mean()) / math.sqrt(va + vb) if va + vb > 0 else float('nan')

sets = []
for arg in sys.argv[3:]:
    label, run = arg.split('=')
    per = {}
    for log in sorted(glob.glob(os.path.join(run, 'seed-*', 'log.txt'))):
        rec = records(log)
        # snapshots are every `interval` Myr from the first; index by time = (i+1)*interval... use count
        if not rec: continue
        n = len(rec)
        per[log] = rec
    sets.append((label, per))

def window(rec, interval):
    # one record per snapshot, the first at 0 Myr
    i0 = int(round(T0 / interval))
    i1 = int(round(T1 / interval))
    if i1 >= len(rec): return None
    a, b = rec[i0], rec[i1]
    return {k: b[k] - a[k] for k in b}

for label, per in sets:
    print(f'## {label}, window {T0:.0f}–{T1:.0f} Myr')
    vals = {k: [] for k in ['rifts', 'failed plume rifts', 'ever marked: boundary/plates', 'never marked: boundary/plates', 'orogenic weakening: boundary/plates', 'weak (>=0.25): boundary/plates', 'share of boundary cells weak (%)', '|cos| break vs motion']}
    for log, rec in per.items():
        interval = float(os.environ.get('INTERVAL', '100'))
        w = window(rec, interval)
        if w is None: continue
        r = lambda x, y: x / y if y > 0 else float('nan')
        vals['rifts'].append(w['rifts'])
        vals['failed plume rifts'].append(w['failed'])
        mb, mp = r(w['marked_b'], w['cont_b']), r(w['marked_p'], w['cont_p'])
        vals['ever marked: boundary/plates'].append(r(mb, mp))
        vals['never marked: boundary/plates'].append(r(1 - mb, 1 - mp))
        vals['orogenic weakening: boundary/plates'].append(r(r(w['oro_b'], w['cont_b']), r(w['oro_p'], w['cont_p'])))
        vals['weak (>=0.25): boundary/plates'].append(r(r(w['weak_b'], w['cont_b']), r(w['weak_p'], w['cont_p'])))
        vals['share of boundary cells weak (%)'].append(100 * r(w['weak_b'], w['cont_b']))
        vals['|cos| break vs motion'].append(r(w['align'], w['aligned']))
    pooled = {}
    for log, rec in per.items():
        w = window(rec, float(os.environ.get('INTERVAL', '100')))
        if w is None: continue
        for k, x in w.items(): pooled[k] = pooled.get(k, 0.0) + x
    if pooled:
        r = lambda x, y: x / y if y > 0 else float('nan')
        P = pooled
        mb, mp = r(P['marked_b'], P['cont_b']), r(P['marked_p'], P['cont_p'])
        print(f"| pooled over seeds: rifts {P['rifts']:.0f}, failed plume rifts {P['failed']:.0f}, boundary cells {P['cont_b']:.0f} | ever marked {100*mb:.0f}% vs {100*mp:.0f}% ({r(mb,mp):.2f}); never marked {r(1-mb,1-mp):.2f}; weakening {r(r(P['oro_b'],P['cont_b']),r(P['oro_p'],P['cont_p'])):.2f} ({100*r(P['oro_b'],P['cont_b']):.1f}% vs {100*r(P['oro_p'],P['cont_p']):.1f}%); weak>=0.25 {100*r(P['weak_b'],P['cont_b']):.0f}% vs {100*r(P['weak_p'],P['cont_p']):.0f}% ({r(r(P['weak_b'],P['cont_b']),r(P['weak_p'],P['cont_p'])):.2f}); |cos| {r(P['align'],P['aligned']):.3f} over {P['aligned']:.0f} |")
    for k, v in vals.items():
        v = [x for x in v if not math.isnan(x)]
        if v:
            print(f'| {k} | {np.mean(v):.2f} ({min(v):.2f}–{max(v):.2f}, n={len(v)}) |')
    print()
