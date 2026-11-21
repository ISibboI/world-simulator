"""Suture marks over time from cell dumps.
usage: marks.py label=DIR ...  (columns orogeny = time of the mark by the max rule, orogenic = fraction if present)"""
import csv, glob, os, re, sys
import numpy as np
for arg in sys.argv[1:]:
    label, run = arg.split('=')
    series = {}
    for sdir in sorted(glob.glob(os.path.join(run, 's*'))):
        for p in sorted(glob.glob(os.path.join(sdir, 'cells_*.csv'))):
            t = int(re.search(r'cells_(\d+)', p).group(1))
            with open(p) as f:
                r = csv.reader(f); h = next(r); rows = list(r)
            H = {k: i for i, k in enumerate(h)}
            cont = np.array([x[H['cls']] == '0' for x in rows])
            marked = np.array([x[H['orogeny']] != '' for x in rows])
            d = series.setdefault(t, {})
            d.setdefault('marked (% of continental cells)', []).append(100 * (cont & marked).sum() / cont.sum())
            if 'orogenic' in H:
                f_ = np.array([float(x[H['orogenic']]) for x in rows])
                total = f_[cont].sum()
                d.setdefault('mean fraction (% of continental cells)', []).append(100 * total / cont.sum())
                d.setdefault('fraction >= 0.5 (%)', []).append(100 * (cont & (f_ >= 0.5)).sum() / cont.sum())
                d.setdefault('fraction >= 0.9 (%)', []).append(100 * (cont & (f_ >= 0.9)).sum() / cont.sum())
                d.setdefault('share of the orogenic total in cells >= 0.9 (%)', []).append(100 * f_[cont & (f_ >= 0.9)].sum() / max(total, 1e-9))
                d.setdefault('share of the orogenic total in cells < 0.5 (%)', []).append(100 * f_[cont & (f_ < 0.5)].sum() / max(total, 1e-9))
                m = cont & (f_ > 0)
                d.setdefault('cells with any fraction (%)', []).append(100 * m.sum() / cont.sum())
    keys = list(next(iter(series.values())).keys()) if series else []
    for t in series:
        for k in series[t]:
            if k not in keys: keys.append(k)
    print(f'## {label} ({len(next(iter(series.values()))[keys[0]])} seeds): mean over seeds by time (Myr)')
    ts = sorted(series)
    print('| | ' + ' | '.join(str(t) for t in ts) + ' |'); print('|---|' + '---|' * len(ts))
    for k in keys:
        print(f'| {k} | ' + ' | '.join(f"{np.mean(series[t][k]):.1f}" if k in series[t] else '—' for t in ts) + ' |')
