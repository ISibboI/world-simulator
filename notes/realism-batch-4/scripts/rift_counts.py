"""Plume rifts per set: successes, failures and distance of the break from the plume.
usage: rift_counts.py label=DIR ... (DIR/s*/log, with the debug lines of events.rs)"""
import glob, os, re, sys
KM = 6371.0
for arg in sys.argv[1:]:
    label, d = arg.split('=')
    ok = small = missed = 0
    dist = []
    for log in sorted(glob.glob(os.path.join(d, 's*', 'log'))):
        last = None
        for line in open(log, errors='replace'):
            line = re.sub(r'\x1b\[[0-9;]*m', '', line)
            if 'Rift break stages' in line:
                last = float(re.search(r'final_distance=([^ ]+)', line).group(1))
            elif 'Plume rifted a plate' in line:
                ok += 1
                dist.append(last * KM)
            elif 'too small' in line and 'Rift' in line:
                small += 1
    n = len(dist)
    near = sum(x <= 120 for x in dist)
    far = sum(x > 500 for x in dist)
    print(f"{label}: rifts {ok}, splits rejected as too small (lines) {small}, "
          f"break within one cell (<= 120 km) {100*near/n:.0f}%, > 500 km {far} ({100*far/n:.0f}%)")
