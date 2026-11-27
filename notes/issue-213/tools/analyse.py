"""Summarise world-simulator runs (#213): per-time means over seeds of the snapshot lines
(plates, ocean age, plumes, largest plate, temperature), the carbon per step (sources,
pCO2), the ramp-up end, and the effective plate count from plate_maps.csv.
Usage: analyse.py RUNDIR [RUNDIR...]  (each RUNDIR holds seed-*/log.txt)"""
import sys, re, glob, collections, statistics as st

def num(pattern, line, cast=float):
    m = re.search(pattern, line)
    return cast(m.group(1)) if m else None

def seed_data(seed_dir):
    snap = {}; carbon = {}; ramp = None; windows = []; phi = None
    t = None
    for line in open(seed_dir + "/log.txt", errors="replace"):
        if "INFO world_simulator: t = " in line:
            t = num(r"t = +([0-9.]+) Myr", line)
            snap[t] = dict(plates=num(r"\| (\d+) plates", line), age=num(r"mean ocean age +([0-9.]+)", line),
                           plumes=num(r"(\d+) plumes", line), largest=num(r"largest +([0-9.]+)%", line),
                           speed=num(r"mean speed +([0-9.]+)", line))
        elif "temperature global" in line and t is not None and t in snap:
            snap[t]["temp"] = num(r"temperature global +([-0-9.]+)", line)
        elif "world_simulator::planet: carbon:" in line:
            tt = num(r"time=([0-9.]+)", line)
            carbon[tt] = dict(p=num(r"pCO₂ ([0-9.]+) bar", line), src=num(r"sources ([0-9.]+) Tmol", line),
                              ridges=num(r"ridges ([0-9.]+)", line), arcs=num(r"arcs ([0-9.]+)", line),
                              plumes=num(r"plumes ([0-9.]+), metamorphic", line), meta=num(r"metamorphic ([0-9.]+)\)", line))
        elif "Ramp-up phase ended at" in line:
            ramp = (num(r"ended at ([0-9.]+) Myr", line), "settled" in line)
        elif "Ramp-up over the last" in line:
            windows.append((num(r"time=([0-9.]+)", line), num(r"effective plates ([0-9.]+)", line),
                            num(r"degassing ([0-9.]+) Tmol", line)))
        elif "sets the CO₂ from now on" in line:
            phi = num(r"phi=([0-9.]+)", line)
        elif "calibrated φ lies outside" in line:
            phi = ("clamped", num(r"calibrated=([0-9.e-]+)", line))
    eff = {}
    try:
        for row in open(seed_dir + "/plate_maps.csv"):
            parts = row.rstrip().split(",")
            ids = parts[1:]; c = collections.Counter(ids); n = len(ids)
            eff[float(round(float(parts[0])))] = 1 / sum((v / n) ** 2 for v in c.values())
    except FileNotFoundError:
        pass
    return dict(snap=snap, carbon=carbon, ramp=ramp, windows=windows, phi=phi, eff=eff)

def mean(v):
    v = [x for x in v if x is not None]
    return st.mean(v) if v else float("nan")

for run in sys.argv[1:]:
    seeds = {d.split("-")[-1]: seed_data(d) for d in sorted(glob.glob(run + "/seed-*")) if glob.os.path.exists(d + "/log.txt")}
    print(f"== {run} ({len(seeds)} seeds)")
    print("ramp-up end:", {s: d["ramp"] for s, d in seeds.items()}, " phi:", {s: d["phi"] for s, d in seeds.items()})
    times = sorted(set().union(*[d["snap"].keys() for d in seeds.values()]))
    print(" t     plates largest age  plumes speed temp | eff | deg(10Myr mean) ridges arcs plumes meta | pCO2")
    for t in times:
        if t > 300 and t % 100: continue
        g = lambda k: mean(d["snap"].get(t, {}).get(k) for d in seeds.values())
        win = lambda k: mean(mean(d["carbon"].get(x, {}).get(k) for x in range(int(t) - 9, int(t) + 1)) for d in seeds.values())
        e = mean(d["eff"].get(t) for d in seeds.values())
        print(f"{t:6.0f} {g('plates'):6.1f} {g('largest'):5.1f} {g('age'):5.1f} {g('plumes'):5.1f} {g('speed'):4.1f} {g('temp'):5.1f} | {e:4.1f} | "
              f"{win('src'):5.2f} {win('ridges'):5.2f} {win('arcs'):5.2f} {win('plumes'):5.2f} {win('meta'):5.2f} | {mean(d['carbon'].get(t, {}).get('p') for d in seeds.values()):.3f}")
