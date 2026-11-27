"""Issue #37 check: slab dynamic-topography depressions and subduction flux on current main.

Usage: python3 analyze.py RUN_DIR   (RUN_DIR/seed-S/{cells_*.csv,boundaries_*.csv,mesh.csv,log.txt})
Window: snapshots 500..1000 Myr.
"""
import csv, glob, math, os, re, sys

R = 6.371e6
ZIGZAG = 1.1
RHO_M, RHO_W = 3300.0, 1030.0
WATER_LOADING = RHO_M / (RHO_M - RHO_W)


def oceanic_depth(age):
    age = max(age, 0.0)
    if age < 20.0:
        return -(2600.0 + 365.0 * math.sqrt(age))
    return -(5651.0 - 2473.0 * math.exp(-0.0278 * age))


def mean_edge_angle(mesh_path):
    pos, total, n = {}, 0.0, 0
    rows = list(csv.DictReader(open(mesh_path)))
    for r in rows:
        pos[int(r["cell"])] = (float(r["x"]), float(r["y"]), float(r["z"]))
    for r in rows:
        a = pos[int(r["cell"])]
        for nb in r["neighbors"].split(";"):
            b = pos[int(nb)]
            d = max(-1.0, min(1.0, sum(x * y for x, y in zip(a, b))))
            total += math.acos(d)
            n += 1
    return total / n


THRESH = [-250, -500, -750, -1000, -1250, -1500]


def pct(sorted_vals, p):
    return sorted_vals[min(len(sorted_vals) - 1, int(p / 100 * len(sorted_vals)))]


def main(run):
    seeds = sorted(glob.glob(os.path.join(run, "seed-*")))
    allrows = []
    for sd in seeds:
        name = os.path.basename(sd)
        mesh = os.path.join(sd, "mesh.csv")
        if not os.path.exists(mesh):
            print(name, "no mesh.csv"); continue
        lpc = R * mean_edge_angle(mesh) / ZIGZAG
        # Log: dynamic topography range and sea-floor production per snapshot.
        log = open(os.path.join(sd, "log.txt"), errors="replace").read().splitlines()
        logmins, prods = [], []
        t = None
        for line in log:
            m = re.search(r"t\s*=\s*([0-9.]+)\s*Myr", line) or re.search(r"snapshot.*?([0-9]+(?:\.[0-9]+)?) Myr", line)
            m2 = re.search(r"dynamic topography \(m, dry\): ([+-]\d+) \.\. ([+-]\d+) \| mean on continental crust ([+-]\d+) \(\|h\| (\d+)\), elsewhere ([+-]\d+)", line)
            if m2:
                logmins.append(tuple(int(x) for x in m2.groups()))
            m3 = re.search(r"sea floor production \(km²/yr\): kinematic ([0-9.]+), created ([0-9.]+)", line)
            if m3:
                prods.append((float(m3.group(1)), float(m3.group(2))))
        for f in sorted(glob.glob(os.path.join(sd, "cells_*.csv"))):
            time = int(re.search(r"cells_(\d+)", f).group(1))
            if time < 500 or time > 1000:
                continue
            dyn, elev, cls = [], [], []
            for r in csv.DictReader(open(f)):
                dyn.append(float(r["dyn"])); elev.append(float(r["elev"])); cls.append(int(r["cls"]))
            n = len(dyn)
            s = sorted(dyn)
            row = dict(seed=name, time=time, n=n, min=s[0], max=s[-1], p01=pct(s, 0.1), p1=pct(s, 1), p5=pct(s, 5))
            # Depressions: area share below thresholds; dry-land share of those; continental.
            for th in THRESH:
                sel = [i for i in range(n) if dyn[i] < th]
                row[f"a{th}"] = len(sel) / n * 100
                row[f"dry{th}"] = sum(1 for i in sel if elev[i] >= 0) / max(1, len(sel)) * 100
                row[f"cont{th}"] = sum(1 for i in sel if cls[i] == 0) / max(1, len(sel)) * 100
            land = [dyn[i] for i in range(n) if elev[i] >= 0]
            flooded_cont = [dyn[i] for i in range(n) if elev[i] < 0 and cls[i] == 0]
            ocean = [dyn[i] for i in range(n) if elev[i] < 0 and cls[i] != 0]
            row["land_min"] = min(land) if land else 0
            row["land_p1"] = pct(sorted(land), 1) if land else 0
            row["fcont_min"] = min(flooded_cont) if flooded_cont else 0
            row["ocean_min"] = min(ocean) if ocean else 0
            row["wet_min_surface"] = min(min(flooded_cont or [0]), min(ocean or [0])) * WATER_LOADING
            row["land_frac"] = len(land) / n * 100
            # Boundaries: trench length, subduction area flux, slab mass flux.
            bf = f.replace("cells_", "boundaries_")
            tl, flux, mass, agesum = 0, 0.0, 0.0, 0.0
            if os.path.exists(bf):
                for r in csv.DictReader(open(bf)):
                    if r["kind"] != "t":
                        continue
                    tl += 1
                    conv = float(r["conv"])  # cm/yr
                    age = float(r["age"])
                    if conv > 0:
                        v = conv / 100.0  # m/yr
                        flux += v * lpc  # m²/yr
                        mass += v * lpc * (RHO_M - RHO_W) * (oceanic_depth(0) - oceanic_depth(age))
                        agesum += v * lpc * age
            row["trench_km"] = tl * lpc / 1e3
            row["sub_flux"] = flux / 1e6  # km²/yr
            row["slab_mass"] = mass / 1e12  # Gt/yr... kg/yr / 1e12 = Gt/yr
            row["flux_age"] = agesum / flux if flux else 0
            allrows.append(row)
        sel_logs = logmins[len(logmins) // 2:]
        sel_prod = prods[len(prods) // 2:]
        if sel_logs:
            print(f"{name}: log dyn min over 2nd half {min(x[0] for x in sel_logs)}, max {max(x[1] for x in sel_logs)}, "
                  f"mean |h| on continents {sum(x[3] for x in sel_logs)/len(sel_logs):.0f}, "
                  f"mean on continents {sum(x[2] for x in sel_logs)/len(sel_logs):+.0f}")
        if sel_prod:
            print(f"{name}: production kinematic {sum(p[0] for p in sel_prod)/len(sel_prod):.2f}, created {sum(p[1] for p in sel_prod)/len(sel_prod):.2f} km2/yr")
    if not allrows:
        return
    keys = [k for k in allrows[0] if k not in ("seed", "time", "n")]
    print("\nper seed (means over 500..1000 Myr snapshots; min = min over snapshots):")
    for sd in sorted({r["seed"] for r in allrows}):
        rs = [r for r in allrows if r["seed"] == sd]
        print(sd, f"snapshots {len(rs)}", " ".join(
            f"{k}={(min(r[k] for r in rs) if k.endswith('min') or k == 'min' else sum(r[k] for r in rs)/len(rs)):.1f}" for k in keys))
    print("\nall seeds:")
    for k in keys:
        vals = [r[k] for r in allrows]
        print(f"  {k:16s} mean {sum(vals)/len(vals):9.1f}  min {min(vals):9.1f}  max {max(vals):9.1f}")


if __name__ == "__main__":
    main(sys.argv[1])
