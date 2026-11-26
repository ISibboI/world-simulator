"""Lake and endorheic metrics of #46 from run logs (snapshots 500-1000 Myr).
usage: check.py DIR   (DIR/seed-S/log.txt); prints per-seed means and the mean (range) over seeds."""
import sys, os, re, glob
import numpy as np

T0, T1 = 500, 1000
F = r'([\d.]+)'
PAT = [
    ('endorheic (% of land)', r'endorheic ([\d.]+)% of land'),
    ('… into single-cell pits', r'\(([\d.]+)% into single-cell pits'),
    ('… into 1-cell depressions (#46 diag)', r'size 1 / 2-3 / 4-10 / >10 cells: ' + F),
    ('… into 2–3-cell depressions', r'>10 cells: [\d.]+ / ' + F),
    ('… into 4–10-cell depressions', r'>10 cells: [\d.]+ / [\d.]+ / ' + F),
    ('… into > 10-cell depressions', r'>10 cells: [\d.]+ / [\d.]+ / [\d.]+ / ' + F + '%'),
    ('… into sea-fed lakes', r'endorheic land into them ([\d.]+)%'),
    ('lakes (% of land)', r'lakes ([\d.]+)% of land, lake potential'),
    ('… of spilling depressions (% of land)', r'spilling lakes ([\d.]+)% of land'),
    ('lake evaporation (km³/yr)', r'lake evaporation ([\d.]+) km'),
    ('… of spilling depressions (km³/yr)', r'spilling lakes [\d.]+% of land, evaporation ([\d.]+) km'),
    ('… beyond own inflow of sea-fed lakes (km³/yr)', r'beyond their own inflow (\d+) km'),
    ('lake potential evaporation (m/yr)', r'lake potential evaporation ([\d.]+) m/yr'),
    ('lake water (m of sea level)', r'lake water \d+ km³ \(([\d.]+) m of sea level'),
    ('depressions holding water', r'lakes in (\d+) depressions'),
    ('… with several local minima', r'endorheic \d+, with inner sills (\d+)'),
    ('… holding several lakes (inner sills)', r'(\d+) of them with several'),
    ('endorheic depressions', r'depressions \d+, endorheic (\d+)'),
    ('playas (% of land)', r'playas \d+ \(([\d.]+)% of land'),
]

def seed(d):
    acc, t = {}, None
    for line in open(os.path.join(d, 'log.txt')):
        m = re.search(r't = +([\d.]+) Myr', line)
        if m:
            t = float(m.group(1))
        if t is None or t < T0 or t > T1:
            continue
        for name, p in PAT:
            m = re.search(p, line)
            if m:
                acc.setdefault(name, []).append(float(m.group(1)))
    return {k: (np.mean(v), max(v), len(v)) for k, v in acc.items()}

dirs = sorted(glob.glob(os.path.join(sys.argv[1], 'seed-*')), key=lambda s: int(s.split('-')[-1]))
res = [seed(d) for d in dirs]
print('| metric | mean (range over seeds) | largest snapshot | per seed |')
print('|---|---|---|---|')
for name, _ in PAT:
    v = [r[name][0] for r in res if name in r]
    if not v:
        continue
    mx = max(r[name][1] for r in res if name in r)
    print(f'| {name} | {np.mean(v):.3g} ({min(v):.3g}…{max(v):.3g}) | {mx:.3g} | ' + ' '.join(f'{x:.3g}' for x in v) + ' |')
print('snapshots per seed:', [r.get('endorheic (% of land)', (0, 0, 0))[2] for r in res])
