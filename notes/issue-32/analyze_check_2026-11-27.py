"""Issue #32 re-check (2026-11-27): sea-floor production, ridges, spreading, slab flux, plate
speed and sea-floor age on current main.

Usage: python3 analyze_check_2026-11-27.py RUN_DIR [T0 T1]
RUN_DIR/seed-S/{log.txt, cells_*.csv, boundaries_*.csv, mesh.csv} (`--cell-dump`).
Window: snapshots T0..T1 Myr (default 500..1000). Means over snapshots, then over seeds.

Definitions as in investigation.md (2026-11-20):
- production: the log's `sea floor production (km²/yr): kinematic` (opening rate × length over
  all diverging boundary cells, both flanks halved) and `created` (cells filled with new sea
  floor), as rates between consecutive snapshots;
- lengths: one-sided (a ridge counts once, a trench on its subducting side), per cell
  R × mean edge angle / 1.1 (zig-zag);
- full spreading rate: −conv at ridge-flank cells (`r`) that open, length-weighted;
- slab flux: trench cells (`t`) converging, × (ρm − ρw)(d(age) − d(0)) (GDH1), as in #37;
- sea-floor age: oceanic crust (`cls` 2) in the cell dump, area-weighted (equal-area cells).
"""
import csv, glob, math, os, re, statistics as st, sys

R = 6.371e6
ZIGZAG = 1.1
RHO_M, RHO_W = 3300.0, 1030.0


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


NUM = r"([-+]?[0-9.]+(?:e[-+]?\d+)?)"
LOG_PATTERNS = {
    "speed": r"\| mean speed\s+" + NUM + r" cm/yr \|\s+(\d+) plates",
    "ocean_age_log": r"mean ocean age\s+" + NUM + " Myr",
    "speed_oc_cont": r"speed oceanic\s+" + NUM + r", continental\s+" + NUM,
    "rel": r"relative speed median " + NUM + r" cm/yr, " + NUM + "% slower than 2",
    "agebins": r"ocean area by age \(% per 20 Myr\): ([\d ]+) \|",
    "seafloor_age": r"sea floor: mean age " + NUM + " Myr",
    "plates1": r"plates > 1%: (\d+)",
    "lengths": r"trenches (\d+) km, ridges (\d+) km, collisions (\d+) km",
    "prod": r"sea floor production \(km²/yr\): kinematic " + NUM + ", created " + NUM
    + r" \| continental opening: kinematic " + NUM,
    "slabpull": r"slab pull " + NUM + " N, ridge push " + NUM,
}


def parse_log(path):
    """Per snapshot time: dict of parsed values."""
    out, t = {}, None
    for line in open(path, errors="replace"):
        m = re.search(r"INFO world_simulator: t =\s*([0-9.]+) Myr", line)
        if m:
            t = round(float(m.group(1)))
            out[t] = {}
        if t is None or "world_simulator::" in line:
            continue
        for key, pat in LOG_PATTERNS.items():
            m = re.search(pat, line)
            if m:
                out[t][key] = m.groups()
    return out


def analyse_seed(sd, t0, t1):
    lpc = R * mean_edge_angle(os.path.join(sd, "mesh.csv")) / ZIGZAG / 1e3  # km per cell
    log = parse_log(os.path.join(sd, "log.txt"))
    rows = []
    for t, v in sorted(log.items()):
        if t < t0 or t > t1:
            continue
        row = dict(t=t)
        if "prod" in v:
            row["prod_kin"], row["prod_created"], row["cont_open"] = map(float, v["prod"])
        if "speed" in v:
            row["speed"] = float(v["speed"][0])
            row["plates"] = int(v["speed"][1])
        if "plates1" in v:
            row["plates1"] = int(v["plates1"][0])
        if "speed_oc_cont" in v:
            row["speed_oc"], row["speed_cont"] = map(float, v["speed_oc_cont"])
        if "rel" in v:
            row["rel_median"], row["rel_slow"] = map(float, v["rel"])
        if "lengths" in v:
            row["log_trench"], row["log_ridge"], row["log_collision"] = (
                float(x) / 1e3 for x in v["lengths"])
        if "seafloor_age" in v:
            row["seafloor_age_log"] = float(v["seafloor_age"][0])
        if "ocean_age_log" in v:
            row["ocean_age_log"] = float(v["ocean_age_log"][0])
        if "agebins" in v:
            bins = [float(x) for x in v["agebins"][0].split()]
            for i, b in enumerate(bins):
                row[f"bin{i}"] = b
        if "slabpull" in v:
            row["slab_pull"] = float(v["slabpull"][0])
        # Boundary dump.
        bf = os.path.join(sd, f"boundaries_{t:05d}.csv")
        if os.path.exists(bf):
            n = {"r": 0, "t": 0, "c": 0, "d": 0, "o": 0}
            spread_sum = spread_len = 0.0
            open_sum = 0.0
            sub_flux = slab_mass = sub_age = 0.0
            sub_young = 0.0
            for r in csv.DictReader(open(bf)):
                k = r["kind"]
                n[k] += 1
                conv = float(r["conv"])
                if conv < 0:
                    open_sum += -conv
                if k == "r" and conv < 0:
                    spread_sum += -conv
                    spread_len += 1
                if k == "t" and conv > 0:
                    age = float(r["age"])
                    a = conv / 1e5 * lpc  # km²/yr
                    sub_flux += a
                    slab_mass += conv / 100 * lpc * 1e3 * (RHO_M - RHO_W) * (
                        oceanic_depth(0) - oceanic_depth(age)) / 1e12
                    sub_age += a * age
                    if age < 20:
                        sub_young += a
            total = sum(n.values())
            row["b_total"] = total * lpc / 2 / 1e3  # 10³ km, one-sided
            row["b_ridge"] = n["r"] * lpc / 2 / 1e3
            row["b_trench"] = n["t"] * lpc / 1e3
            row["b_coll"] = n["c"] * lpc / 2 / 1e3
            row["b_diffuse"] = n["d"] * lpc / 1e3
            row["b_other"] = row["b_total"] - row["b_ridge"] - row["b_trench"] - row["b_coll"] - row["b_diffuse"]
            row["ridge_trench_share"] = 100 * (row["b_ridge"] + row["b_trench"]) / row["b_total"]
            row["spread_rate"] = spread_sum / max(spread_len, 1)  # cm/yr, full
            row["prod_dump"] = open_sum / 1e5 * lpc / 2  # km²/yr, both flanks halved
            row["prod_ridge_dump"] = spread_sum / 1e5 * lpc / 2
            row["sub_flux"] = sub_flux
            row["slab_mass"] = slab_mass
            row["sub_age"] = sub_age / sub_flux if sub_flux else 0
            row["sub_young_pct"] = 100 * sub_young / sub_flux if sub_flux else 0
        cf = os.path.join(sd, f"cells_{t:05d}.csv")
        if os.path.exists(cf):
            ages = []
            for r in csv.DictReader(open(cf)):
                if r["cls"] == "2":
                    ages.append(float(r["age"]))
            ages.sort()
            if ages:
                row["oc_age_mean"] = sum(ages) / len(ages)
                young = [a for a in ages if a < 200]
                row["oc_age_mean_lt200"] = sum(young) / len(young)
                row["oc_age_median"] = ages[len(ages) // 2]
                row["oc_ge200_pct"] = 100 * (len(ages) - len(young)) / len(ages)
                row["oc_lt20_pct"] = 100 * sum(1 for a in ages if a < 20) / len(ages)
                row["oc_area_frac"] = len(ages)  # cells
                for lo in range(0, 200, 20):
                    row[f"ab{lo}"] = 100 * sum(1 for a in ages if lo <= a < lo + 20) / len(ages)
                # Production estimated from young sea floor: area < 10 Myr / 10 Myr.
                n_cells = sum(1 for _ in open(os.path.join(sd, "mesh.csv"))) - 1
                cell_area = 4 * math.pi * (R / 1e3) ** 2 / n_cells
                row["prod_young10"] = sum(1 for a in ages if a < 10) * cell_area / 10e6
                row["ocean_area_frac"] = 100 * len(ages) / n_cells
        rows.append(row)
    return rows


def main():
    run = sys.argv[1]
    t0, t1 = (float(sys.argv[2]), float(sys.argv[3])) if len(sys.argv) > 3 else (500, 1000)
    per_seed = {}
    for sd in sorted(glob.glob(os.path.join(run, "seed-*")), key=lambda s: int(s.split("-")[-1])):
        rows = analyse_seed(sd, t0, t1)
        if rows:
            per_seed[os.path.basename(sd)] = rows
    keys = []
    for rows in per_seed.values():
        for r in rows:
            for k in r:
                if k not in keys and k != "t":
                    keys.append(k)
    # Production between snapshots: the first snapshot of the window covers 450–500; skip it.
    seed_means = {}
    for s, rows in per_seed.items():
        means = {}
        for k in keys:
            vals = [r[k] for r in rows if k in r and not (k.startswith("prod") and k in ("prod_kin", "prod_created", "cont_open") and r["t"] == t0)]
            if vals:
                means[k] = sum(vals) / len(vals)
        seed_means[s] = means
    print(f"window {t0:.0f}..{t1:.0f} Myr, seeds {len(seed_means)}")
    print(f"{'quantity':22s} {'mean':>9s} {'seed min':>9s} {'seed max':>9s}  per seed")
    for k in keys:
        vals = [m[k] for m in seed_means.values() if k in m]
        if not vals:
            continue
        print(f"{k:22s} {st.mean(vals):9.2f} {min(vals):9.2f} {max(vals):9.2f}  "
              + " ".join(f"{v:.2f}" for v in vals))


if __name__ == "__main__":
    main()
