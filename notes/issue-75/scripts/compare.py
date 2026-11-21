"""Compare run sets on log metrics: mean over seeds (of the mean over snapshots in the window), seed
range, and Welch t against the first set.
usage: compare.py [T0 T1] label=DIR [label=DIR ...]   (DIR holds seed-S/log.txt, the layout of `runs`)"""
import glob, math, os, re, sys

args = sys.argv[1:]
T0, T1 = 500.0, 1000.0
if args and '=' not in args[0]:
    T0, T1 = float(args[0]), float(args[1]); args = args[2:]

PAT = [
    ('plates', r'\| (\d+) plates \('),
    ('plates > 1%', r'plates > 1%: (\d+)'),
    ('mean speed (cm/yr)', r'mean speed +([\d.]+) cm/yr'),
    ('speed 99% (cm/yr)', r'99% +([\d.]+), RMS'),
    ('trenches (10^3 km)', r'trenches (\d+) km', 1e-3),
    ('ridges (10^3 km)', r'ridges (\d+) km', 1e-3),
    ('collisions (10^3 km)', r'collisions (\d+) km', 1e-3),
    ('incipient trenches (10^3 km)', r'incipient trenches (\d+) km', 1e-3),
    ('convergence at incipient trenches (cm/yr)', r'converging at ([\d.-]+) cm/yr'),
    ('convergence at other trenches (cm/yr)', r'others at ([\d.-]+) cm/yr'),
    ('slab pull (1e21 N)', r'slab pull ([\d.e+-]+) N', 1e-21),
    ('relative speed median (cm/yr)', r'relative speed median ([\d.]+) cm/yr'),
    ('boundary slower than 2 cm/yr (%)', r'(\d+)% slower than 2 cm/yr'),
    ('opening/closing (%)', r'opening/closing (\d+)%'),
    ('strike-slip (%)', r'strike-slip (\d+)%'),
    ('sea-floor production kinematic (km2/yr)', r'sea floor production \(km²/yr\): kinematic ([\d.]+)'),
    ('sea-floor production created (km2/yr)', r'kinematic [\d.]+, created ([\d.]+)'),
    ('mean sea-floor age (Myr)', r'sea floor: mean age ([\d.]+) Myr'),
    ('ocean age median (Myr)', r'ocean age median +(\d+)'),
    ('sea floor >= 200 Myr (%)', r'sea floor: mean age [\d.]+ Myr, ≥ 200 Myr ([\d.]+)%'),
    ('sea floor < 20 Myr (% of ocean)', r'ocean area by age \(% per 20 Myr\): (\d+)'),
    ('sea floor 20-40 Myr (%)', r'ocean area by age \(% per 20 Myr\): \d+ (\d+)'),
    ('ocean deeper than 6 km (%)', r'deeper than 6 km +([\d.]+)%'),
    ('continental (% of surface)', r'\| continental +([\d.]+)%'),
    ('land (% of surface)', r'\| land +([\d.]+)%'),
    ('mean land (m)', r'land mean +(-?\d+) m'),
    ('highest point (m)', r'elevation +-?\d+ m \.\. +(-?\d+) m'),
    ('flooded continental crust (%)', r'(\d+)% flooded'),
    ('crust > 50 km (% of continental)', r'(\d+)% > 50 km'),
    ('sediment part of sea level (m)', r'sediment part of the sea level \(m\): all ([+-][\d.]+)'),
    ('land lower than 10 m (% of land)', r'land lower than 10 m ([\d.]+)%'),
    ('shelf area (% of surface)', r'shelves \(0 to -200 m\) ([\d.]+)%'),
    ('shelf mean sediment (km)', r'mean sediment ([\d.]+) km'),
    ('dynamic topography min (m)', r'dynamic topography \(m, dry\): ([+-]\d+)'),
    ('dynamic topography max (m)', r'dynamic topography \(m, dry\): [+-]\d+ \.\. ([+-]\d+)'),
    ('sea-level budget mantle (m/100 Myr)', r'mantle [+-][\d.]+ \| ([+-][\d.]+)'),
    ('felsic+sediment net (Gt/yr)', r'together ([+-][\d.]+)'),
    ('trench sediment (Gt/yr)', r'trench sediment ([\d.]+) Gt/yr'),
    ('collision stacking (Gt/yr)', r'collision stacking \(Gt/yr\): ([\d.]+) stacked'),
    ('burial metamorphism on continental crust (Gt/yr)', r'burial metamorphism ([\d.]+) on continental'),
    ('burial metamorphism on other crust (Gt/yr)', r'on continental crust, ([\d.]+) on other crust'),
    ('sediment trapped in lakes (km3/yr)', r'sediment trapped in lakes ([\d.]+)'),
    ('depressions (lakes)', r'depressions (\d+)'),
    ('land precipitation (m/yr)', r'land precipitation ([\d.]+) m/yr'),
    ('runoff (km3/yr)', r'hydrology: runoff (\d+) km³/yr'),
    ('largest river (m3/s)', r'largest river +(\d+) m³/s'),
    ('sediment to ocean (km3/yr)', r'sediment to ocean +([\d.]+) km³/yr'),
    ('endorheic (% of land)', r'endorheic ([\d.]+)% of land'),
    ('endorheic into single-cell pits (% of land)', r'\(([\d.]+)% into single-cell pits\)'),
    ('lakes (% of land)', r'lakes ([\d.]+)% of land'),
    ('lake evaporation (km3/yr)', r'lake evaporation (\d+) km³/yr'),
    ('global temperature (C)', r'temperature global +([\d.-]+)'),
    ('equator (C)', r'equator +([\d.-]+) °C'),
    ('poles (C)', r'poles +([\d.-]+) °C'),
    ('permanently frozen (%)', r'permanently frozen +([\d.]+)%'),
    ('ocean heat transport 20N (PW)', r'ocean at 20°N ([+-][\d.]+)'),
    ('gyres (Sv)', r'gyres (\d+) Sv'),
]
CUM = [  # cumulative counters: value at T1 minus value at T0, per 100 Myr
    ('rifts /100 Myr', r'rifts (\d+), initiations'),
    ('initiations /100 Myr', r'initiations (\d+), breakups'),
    ('sutures /100 Myr', r'sutures (\d+), captures'),
    ('plates born /100 Myr', r'plates \((\d+) born'),
    ('segments created /100 Myr', r'\| created (\d+), merged'),
    ('terminal fans and deltas (km cells /100 Myr)', r'terminal fans and deltas (\d+) km·cells'),
    ('overfill (km cells /100 Myr)', r'sediment overfill (\d+) km·cells'),
]


def parse(path):
    snaps, t, runtime = {}, None, math.nan
    for line in open(path, errors='replace'):
        line = re.sub(r'\x1b\[[0-9;]*m', '', line)
        m = re.search(r't = +([\d.]+) Myr \|', line)
        if m:
            t = float(m.group(1)); snaps[t] = {}
        m = re.search(r'Finished in ([\d.]+)(m?s)', line)
        if m:
            runtime = float(m.group(1)) * (60 if m.group(2) == 'm' else 1)
        m = re.search(r'[Ff]inished.*?([\d.]+) ?s', line)
        if m and math.isnan(runtime):
            runtime = float(m.group(1))
        if t is None:
            continue
        for p in PAT + CUM:
            mm = re.search(p[1], line)
            if mm and p[0] not in snaps[t]:
                snaps[t][p[0]] = float(mm.group(1).replace('+', '')) * (p[2] if len(p) > 2 else 1)
    return snaps, runtime


def slope(snaps, key, a, b):
    pts = [(t, d[key]) for t, d in sorted(snaps.items()) if a <= t <= b and key in d]
    if len(pts) < 2:
        return math.nan
    n = len(pts); mt = sum(p[0] for p in pts) / n; mv = sum(p[1] for p in pts) / n
    return 100 * sum((p[0] - mt) * (p[1] - mv) for p in pts) / sum((p[0] - mt) ** 2 for p in pts)


def evaluate(d):
    res = {}
    for log in sorted(glob.glob(os.path.join(d, 'seed-*', 'log.txt'))):
        snaps, runtime = parse(log)
        if not snaps or max(snaps) < T1:
            continue
        seed = os.path.basename(os.path.dirname(log))
        r = {}
        win = [snaps[t] for t in sorted(snaps) if T0 < t <= T1]
        for p in PAT:
            v = [x[p[0]] for x in win if p[0] in x]
            if v:
                r[p[0]] = sum(v) / len(v)
        for p in CUM:
            if p[0] in snaps.get(T1, {}) and p[0] in snaps.get(T0, {}):
                r[p[0]] = (snaps[T1][p[0]] - snaps[T0][p[0]]) * 100 / (T1 - T0)
        r['sediment-part slope (m/100 Myr)'] = slope(snaps, 'sediment part of sea level (m)', T0, T1)
        r['sediment part at 1000 Myr (m)'] = snaps.get(T1, {}).get('sediment part of sea level (m)', math.nan)
        r['highest point, max over all snapshots (m)'] = max(x.get('highest point (m)', -1e9) for x in snaps.values())
        r['runtime (s)'] = runtime
        res[seed] = r
    return res


def stats(v):
    v = [x for x in v if not math.isnan(x)]
    n = len(v)
    if n == 0:
        return math.nan, math.nan, 0, math.nan, math.nan
    m = sum(v) / n
    var = sum((x - m) ** 2 for x in v) / (n - 1) if n > 1 else math.nan
    return m, var, n, min(v), max(v)


sets = [(a.split('=')[0], evaluate(a.split('=')[1])) for a in args]
keys = [p[0] for p in PAT] + [p[0] for p in CUM] + ['sediment-part slope (m/100 Myr)', 'sediment part at 1000 Myr (m)', 'highest point, max over all snapshots (m)', 'runtime (s)']
print(f'window {T0:g}-{T1:g} Myr; seeds: ' + ', '.join(f'{l}: {len(r)}' for l, r in sets))
print('| metric | ' + ' | '.join(l + (' | t' if i else '') for i, (l, _) in enumerate(sets)) + ' |')
print('|---|' + '---|' * (2 * len(sets) - 1))
for k in keys:
    cells = []
    base = None
    for i, (l, res) in enumerate(sets):
        m, var, n, lo, hi = stats([r.get(k, math.nan) for r in res.values()])
        if n == 0:
            cells.append('—'); cells += ['—'] if i else []
            continue
        f = (lambda x: f'{x:.0f}') if abs(m) >= 100 else (lambda x: f'{x:.2f}')
        cells.append(f'{f(m)} ({f(lo)}…{f(hi)})')
        if i == 0:
            base = (m, var, n)
        else:
            if base and n > 1 and base[2] > 1 and (var / n + base[1] / base[2]) > 0:
                cells.append(f'{(m - base[0]) / math.sqrt(var / n + base[1] / base[2]):+.1f}')
            else:
                cells.append('—')
    if any(c != '—' for c in cells):
        print(f'| {k} | ' + ' | '.join(cells) + ' |')
