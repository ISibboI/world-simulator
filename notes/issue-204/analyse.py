#!/usr/bin/env python3
"""Summarise #204 runs: per configuration directory (runs output with seed-*/log.txt), means
over 200-1000 Myr and at 0-50 Myr of the snapshot lines, phi, runtime.
Usage: analyse.py DIR [DIR...]"""
import re, sys, glob, os, statistics as st

num = r'([-+]?[0-9.]+|NaN)'
T = re.compile(r't = +' + num + r' Myr')
TEMP = re.compile(r'temperature global +' + num + r' °C, equator +' + num + r' °C, poles +' + num +
                  r' °C .*permanently frozen +' + num + r'%(?: \| planetary albedo ' + num + ')?')
CARB = re.compile(r'    carbon: pCO₂ ' + num + r' bar \(' + num + r'× PAL\), forcing ' + num)
AER = re.compile(r'aerosol: sources volcanic ' + num + r' \(degassing ' + num + r"× Earth's\), dust " + num +
                 r' × Earth \| marine CCN ' + num + r', droplets ' + num + r' cm⁻³, cloud albedo ' + num +
                 r', ocean albedo ' + num + r' \| continental CCN ' + num + r', droplets ' + num +
                 r' cm⁻³, cloud albedo ' + num + r', land albedo ' + num)
PHI = re.compile(r'sets the CO₂ from now on phi=' + num)
KEYS = ['T', 'Teq', 'Tpole', 'ice%', 'albedo', 'pCO2', 'CO2x', 'forcing', 'Rv', 'degas', 'Rd',
        'mCCN', 'mNd', 'mAc', 'dAo', 'cCCN', 'cNd', 'cAc', 'dAl']

def parse(log):
    rows, t, row, phi = [], None, {}, float('nan')
    for line in open(log, errors='replace'):
        if m := PHI.search(line): phi = float(m.group(1))
        if (m := T.search(line)) and 'INFO world_simulator:' in line and '| continental' in line:
            if t is not None: rows.append((t, row))
            t, row = float(m.group(1)), {}
        elif m := TEMP.search(line):
            g = [float(x) if x else float('nan') for x in m.groups()]
            row.update(zip(['T', 'Teq', 'Tpole', 'ice%', 'albedo'], g))
        elif (m := CARB.search(line)) and 'INFO world_simulator:' in line:
            row.update(zip(['pCO2', 'CO2x', 'forcing'], map(float, m.groups())))
        elif m := AER.search(line):
            row.update(zip(KEYS[8:], map(float, m.groups())))
    if t is not None: rows.append((t, row))
    return rows, phi

def mean(xs):
    xs = [x for x in xs if x == x]
    return st.mean(xs) if xs else float('nan')

for d in sys.argv[1:]:
    seeds = sorted(glob.glob(os.path.join(d, 'seed-*')))
    print(f'== {d} ({len(seeds)} seeds)')
    late, early, phis, secs = {k: [] for k in KEYS}, {k: [] for k in KEYS}, [], []
    for s in seeds:
        rows, phi = parse(os.path.join(s, 'log.txt'))
        phis.append(phi)
        try: secs.append(float(open(os.path.join(s, 'seconds')).read()))
        except Exception: pass
        per = {k: mean([r.get(k, float('nan')) for t, r in rows if 200 <= t <= 1000]) for k in KEYS}
        e = {k: mean([r.get(k, float('nan')) for t, r in rows if t <= 50]) for k in KEYS}
        for k in KEYS: late[k].append(per[k]); early[k].append(e[k])
        print(f'  {os.path.basename(s)}: T {per["T"]:.2f} ice {per["ice%"]:.2f}% albedo {per["albedo"]:.4f} '
              f'CO2 {per["pCO2"]:.4f} bar phi {phi:.3f} Rv {per["Rv"]:.2f} Rd {per["Rd"]:.2f}')
    fmt = lambda v: ' '.join(f'{k} {mean(v[k]):.4g}' for k in KEYS)
    print('  mean 200-1000:', fmt(late))
    print('  mean 0-50    :', fmt(early))
    print(f'  phi {[round(p, 3) for p in phis]} | runtime mean {mean(secs):.0f} s')
