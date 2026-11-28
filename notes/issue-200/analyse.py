#!/usr/bin/env python3
"""Summarises #200 runs: `analyse.py NEW_DIR [BASE_DIR]` (run directories of `runs`, with
seed-*/log.txt). Means over 200–1000 Myr per seed, then the ensemble mean ± SD over seeds."""
import re
import statistics
import sys
from pathlib import Path

NUM = r"([-+]?[0-9.]+(?:e[-+]?[0-9]+)?|NaN)"


def f(x):
    return float("nan") if x == "NaN" else float(x)


CARBON = re.compile(
    r"carbon: pCO₂ " + NUM + r" bar .*? sources " + NUM + r" Tmol/yr \(ridges " + NUM
    + r", arcs " + NUM + r", plumes " + NUM + r", metamorphic " + NUM + r"\) \| sinks " + NUM
    + r" \(land " + NUM + r", seafloor " + NUM + r"\) \| φ " + NUM + r".*time=" + NUM
)
STEP = re.compile(
    r"carbonate: burial land " + NUM + r" Tmol/yr, sea floor " + NUM + r" \| precipitated "
    + NUM + r" Gt/yr \(recycled " + NUM + r"\), saturation level " + NUM + r", max Ω " + NUM
    + r".*time=" + NUM
)
SNAP = re.compile(r"t = +" + NUM + r" Myr .*land mean +" + NUM + r" m")
TEMP = re.compile(r"temperature global +" + NUM)
CARB = re.compile(
    r"carbonate \(#200\): precipitated " + NUM + r" Gt/yr \(new " + NUM + r", recycled " + NUM
    + r"\) \| depth .*?m: ([0-9 /]+)% \| \|lat\|.*?°: ([0-9 /]+)% \| on continental crust " + NUM
    + r"% \| Ω mean " + NUM + r", max " + NUM + r".*?dissolved load " + NUM
    + r" Gt/yr, unmatched " + NUM + r" \| carbonate in sediment: all " + NUM + r"%, continental "
    + NUM + r"%, transitional " + NUM + r"%, oceanic " + NUM + r"% \| subducted " + NUM
    + r" Gt/yr of " + NUM + r" Gt/yr sediment"
)
RES = re.compile(
    r"carbon reservoirs \(10¹⁸ mol\): exogenic " + NUM + r", sediment carbonate " + NUM
    + r", altered sea floor " + NUM + r", in crust \(marble, wedges\) " + NUM
    + r", source buffers " + NUM + r" \| since the start: from the mantle " + NUM
    + r", into the mantle " + NUM + r" \(lost with crust " + NUM + r"\).*closure residual " + NUM
    + r" mol \(" + NUM
)
SUBDUCTED = re.compile(r"subducted sediment " + NUM)
FINISHED = re.compile(r"Finished in " + NUM + "s")


def mean(values):
    values = [v for v in values if v == v]
    return sum(values) / len(values) if values else float("nan")


def seed(path):
    out = {}
    carbon, steps, snaps, carbs, res, temps = [], [], [], [], [], []
    time = 0.0
    for line in path.read_text().splitlines():
        if m := CARBON.search(line):
            carbon.append([f(x) for x in m.groups()])
        elif m := STEP.search(line):
            steps.append([f(x) for x in m.groups()])
        elif m := SNAP.search(line):
            time = f(m.group(1))
            snaps.append((time, f(m.group(2))))
        elif m := TEMP.search(line):
            temps.append((time, f(m.group(1))))
        elif m := CARB.search(line):
            g = m.groups()
            carbs.append((time, g))
        elif m := RES.search(line):
            res.append((time, [f(x) for x in m.groups()]))
        elif m := FINISHED.search(line):
            out["seconds"] = f(m.group(1))
    late = [c for c in carbon if 200 <= c[-1] <= 1000]
    names = ["pco2", "sources", "ridges", "arcs", "plumes", "metamorphic", "sinks", "land",
             "seafloor", "phi"]
    for i, name in enumerate(names):
        out[name] = mean([c[i] for c in late])
    if carbon:
        out["pco2_1000"] = carbon[-1][0]
    late = [s for s in steps if 200 <= s[-1] <= 1000]
    for i, name in enumerate(["burial_land", "burial_seafloor", "precipitated", "recycled",
                              "level", "max_omega"]):
        out[name] = mean([s[i] for s in late])
    out["land_mean"] = mean([v for t, v in snaps if t >= 200])
    out["temperature"] = mean([v for t, v in temps if t >= 200])
    late = [g for t, g in carbs if t >= 200]
    if late:
        depth = [[float(x) for x in g[3].split("/")] for g in late]
        lat = [[float(x) for x in g[4].split("/")] for g in late]
        for i in range(5):
            out[f"depth{i}"] = mean([d[i] for d in depth])
        for i in range(4):
            out[f"lat{i}"] = mean([d[i] for d in lat])
        for i, name in [(5, "on_continental"), (6, "omega_mean"), (8, "dissolved"),
                        (9, "unmatched"), (10, "frac_all"), (11, "frac_continental"),
                        (12, "frac_transitional"), (13, "frac_oceanic"),
                        (14, "subducted_carbonate"), (15, "subducted_sediment")]:
            out[name] = mean([f(g[i]) for g in late])
        out["frac_all_1000"] = f(carbs[-1][1][10])
        out["frac_continental_1000"] = f(carbs[-1][1][11])
        out["frac_all_100"] = f(carbs[0][1][10])
        out["frac_continental_100"] = f(carbs[0][1][11])
    if res:
        names = ["exogenic", "sediment_c", "altered", "crust_c", "buffers", "from_mantle",
                 "to_mantle", "lost", "residual", "residual_rel"]
        for i, name in enumerate(names):
            out[name + "_1000"] = res[-1][1][i]
        out["sediment_c_0"] = None
        out["max_residual_rel"] = max(abs(r[1][9]) for r in res)
    return out


def ensemble(directory):
    seeds = sorted(Path(directory).glob("seed-*/log.txt"))
    return [seed(path) for path in seeds]


def show(name, runs, key, scale=1.0, fmt="{:.3f}"):
    values = [r[key] * scale for r in runs if r.get(key) is not None and r[key] == r[key]]
    if not values:
        return "-"
    sd = statistics.pstdev(values) if len(values) > 1 else 0.0
    return (fmt + " ± " + fmt).format(mean(values), sd)


def main():
    new = ensemble(sys.argv[1])
    base = ensemble(sys.argv[2]) if len(sys.argv) > 2 else []
    keys = ["pco2", "pco2_1000", "temperature", "phi", "sources", "ridges", "arcs", "plumes",
            "metamorphic", "sinks", "land", "seafloor", "land_mean", "seconds"]
    print(f"seeds: new {len(new)}, base {len(base)}")
    print("| quantity (200–1000 Myr) | base | new |")
    print("|---|---|---|")
    for key in keys:
        print(f"| {key} | {show(key, base, key)} | {show(key, new, key)} |")
    print()
    print("| carbonate (new, 200–1000 Myr) | value |")
    print("|---|---|")
    for key in ["burial_land", "burial_seafloor", "precipitated", "recycled", "level",
                "max_omega", "omega_mean", "depth0", "depth1", "depth2", "depth3", "depth4",
                "lat0", "lat1", "lat2", "lat3", "on_continental", "dissolved", "unmatched",
                "frac_all_100", "frac_all", "frac_all_1000", "frac_continental_100",
                "frac_continental", "frac_continental_1000", "frac_transitional", "frac_oceanic",
                "subducted_carbonate", "subducted_sediment", "exogenic_1000", "sediment_c_1000",
                "altered_1000", "crust_c_1000", "buffers_1000", "from_mantle_1000",
                "to_mantle_1000", "lost_1000", "max_residual_rel"]:
        print(f"| {key} | {show(key, new, key, fmt='{:.4g}')} |")
    print()
    print("per seed (new): seed, φ, pCO₂, T, arcs, metamorphic, precipitated, frac_all_1000")
    for i, r in enumerate(new):
        print(i, *(f"{r.get(k, float('nan')):.3f}" for k in
                   ["phi", "pco2", "temperature", "arcs", "metamorphic", "precipitated",
                    "frac_all_1000"]))
    if base:
        print("per seed (base): seed, φ, pCO₂, T, arcs, metamorphic")
        for i, r in enumerate(base):
            print(i, *(f"{r.get(k, float('nan')):.3f}" for k in
                       ["phi", "pco2", "temperature", "arcs", "metamorphic"]))


if __name__ == "__main__":
    main()
