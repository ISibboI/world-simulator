"""Offline replay of a trend criterion (#213): every 10 Myr, the least-squares trend of the
per-step degassing over the last S Myr, relative to its mean and scaled to S, and the change of
the effective plate count (window means, or snapshots for the base) over S."""
import sys, glob, re, collections, itertools

def load(seed_dir):
    src = {}; eff = {}
    for line in open(seed_dir + "/log.txt", errors="replace"):
        if "world_simulator::planet: carbon:" in line:
            t = round(float(re.search(r"time=([0-9.]+)", line).group(1)))
            src[t] = float(re.search(r"sources ([0-9.]+) Tmol", line).group(1))
        elif "Ramp-up: window means" in line:
            t = round(float(re.search(r"time=([0-9.]+)", line).group(1)))
            eff[t] = float(re.search(r"effective plates ([0-9.]+)", line).group(1))
    if not eff and glob.os.path.exists(seed_dir + "/plate_maps.csv"):
        for row in open(seed_dir + "/plate_maps.csv"):
            parts = row.rstrip().split(","); ids = parts[1:]; n = len(ids)
            c = collections.Counter(ids)
            eff[round(float(parts[0]))] = 1 / sum((v / n) ** 2 for v in c.values())
    return src, eff

def trend(values):
    n = len(values); xs = range(n); mx = (n - 1) / 2; my = sum(values) / n
    slope = sum((x - mx) * (y - my) for x, y in zip(xs, values)) / sum((x - mx) ** 2 for x in xs)
    return slope * n / my

def end_time(src, eff, S, tol_d, tol_p, passes, min_dur, max_dur=150):
    ok = 0
    for t in range(10, max_dur + 1, 10):
        if t >= max(min_dur, S):
            vals = [src[x] for x in range(t - S + 1, t + 1) if x in src]
            pts = [eff[x] for x in range(t - S + 10, t + 1, 10) if x in eff]
            if len(vals) < S * 0.8 or len(pts) < 2: continue
            g = trend(vals)
            pc = trend(pts) if len(pts) > 2 else (pts[-1] - pts[0]) / pts[-1]
            ok = ok + 1 if (g <= tol_d and abs(pc) <= tol_p) else 0
            if ok >= passes: return t
    return max_dur

runs = sys.argv[1:]
data = {r: [load(d) for d in sorted(glob.glob(r + "/seed-*"))] for r in runs}
for S, tol_d, tol_p, passes in itertools.product([20, 30, 40], [0.05, 0.08, 0.1], [0.15, 0.25], [1, 2]):
    cells = []
    for r in runs:
        ends = [end_time(s, e, S, tol_d, tol_p, passes, 30) for s, e in data[r] if s]
        cells.append("/".join(str(x) for x in ends) + f" ({sum(ends)/len(ends):.0f})")
    print(f"S{S} d{tol_d} p{tol_p} k{passes}: " + "  ".join(cells))
