"""Compares volcanism runs with main (#106): LIPs, magmatism, climate, relief.

Usage: python3 analyse.py VOLC_DIR MAIN_DIR"""
import csv, re, statistics as st, sys
from pathlib import Path

def num(pattern, line, cast=float):
    m = re.search(pattern, line)
    return cast(m.group(1)) if m else None

def parse_log(path):
    snaps, lips, current = [], [], None
    for line in open(path, encoding="utf-8"):
        if "LIP at" in line:
            m = re.search(r"LIP at (\d+) Myr \(([-\d]+)°, ([-\d]+)°\): ([\d.e+]+) km³ under (\w+) .*lid factor ([\d.]+)\), (\d+) samples, up to ([\d.]+) km", line)
            lips.append(dict(t=float(m[1]), v=float(m[4]), setting=m[5], lid=float(m[6]), n=int(m[7]), peak=float(m[8])))
        elif re.search(r"t = +[\d.]+ Myr \| continental", line):
            current = dict(t=num(r"t = +([\d.]+) Myr", line), cont=num(r"continental +([\d.]+)%", line), land=num(r"land +([\d.]+)%", line))
            snaps.append(current)
        elif current is not None and "temperature global" in line:
            current["temp"] = num(r"global +([-\d.]+) °C", line)
        elif current is not None and "carbon: pCO₂" in line and "planet" not in line:
            current["pco2"] = num(r"pCO₂ ([\d.e-]+) bar", line)
            current["plumes"] = num(r"plumes ([\d.]+)", line)
            current["sources"] = num(r"sources ([\d.]+) Tmol", line)
        elif current is not None and "magmatism (km³/yr)" in line:
            for key in ["ridges", "arcs", "LIPs", "plume tails"]:
                current["m_" + key] = num(key + r" ([\d.]+)", line)
        elif current is not None and "sea level" in line and "budget" not in line and "sea_level" not in current:
            current["sea_level_line"] = line.strip()[-160:]
    return snaps, lips

def cells(path):
    rows = list(csv.DictReader(open(path)))
    out = dict(n=len(rows))
    elev = [float(r["elev"]) for r in rows]
    ocean = [float(r["elev"]) for r in rows if r["ocean"] == "1"]
    out["ocean_frac"] = len(ocean) / len(rows)
    ocean.sort()
    out["ocean_depth_p10_50_90"] = [ocean[int(q * (len(ocean) - 1))] for q in (0.1, 0.5, 0.9)] if ocean else None
    out["ocean_shallower_3km"] = sum(1 for e in ocean if e > -3000) / max(len(ocean), 1)
    mafic_thick = [r for r in rows if float(r["felsic"]) < 0.3 and float(r["thick"]) > 12000]
    out["plateau_frac"] = len(mafic_thick) / len(rows)
    out["plateau_mean_elev"] = st.mean(float(r["elev"]) for r in mafic_thick) if mafic_thick else None
    out["plateau_land_frac"] = sum(1 for r in mafic_thick if r["ocean"] != "1") / len(rows)
    out["max_mafic_thick_km"] = max((float(r["thick"]) for r in rows if float(r["felsic"]) < 0.3), default=0) / 1000
    cont = [r for r in rows if float(r["felsic"]) > 0.5 and float(r["thick"]) > 20000]
    out["cont_origin_frac"] = len(cont) / len(rows)
    out["cont_mean_felsic"] = st.mean(float(r["felsic"]) for r in cont) if cont else None
    land = sorted(e for r, e in zip(rows, elev) if r["ocean"] != "1")
    out["land_p50_90_99"] = [land[int(q * (len(land) - 1))] for q in (0.5, 0.9, 0.99)] if land else None
    return out

def summarise(directory, label):
    print(f"== {label}: {directory}")
    per_seed = []
    for seed_dir in sorted(Path(directory).glob("seed-*")):
        snaps, lips = parse_log(seed_dir / "log.txt")
        seconds = float(open(seed_dir / "seconds").read())
        c = cells(seed_dir / "cells_01000.csv") if (seed_dir / "cells_01000.csv").exists() else {}
        per_seed.append((seed_dir.name, snaps, lips, seconds, c))
    for name, snaps, lips, seconds, c in per_seed:
        last = snaps[-1]
        big = [l for l in lips if l["v"] >= 1e6]
        cont = [l for l in lips if l["setting"] == "continental"]
        print(f"{name}: {seconds:.0f} s | LIPs {len(lips)} (≥1e6: {len(big)}, continental {len(cont)}) | "
              f"t=1000: cont {last['cont']}% land {last['land']}% T {last.get('temp')} °C pCO2 {last.get('pco2')} bar plumes {last.get('plumes')} Tmol/yr | "
              f"ocean p10/50/90 {c.get('ocean_depth_p10_50_90')} shallow<3km {c.get('ocean_shallower_3km', 0):.3f} "
              f"plateau {c.get('plateau_frac', 0):.4f} (land {c.get('plateau_land_frac', 0):.4f}, mean elev {c.get('plateau_mean_elev')}) "
              f"max mafic {c.get('max_mafic_thick_km', 0):.1f} km cont-origin {c.get('cont_origin_frac', 0):.3f} felsic {c.get('cont_mean_felsic')} land p50/90/99 {c.get('land_p50_90_99')}")
    all_lips = [l for _, _, lips, _, _ in per_seed for l in lips]
    if all_lips:
        vols = sorted(l["v"] for l in all_lips)
        seeds = len(per_seed)
        for setting in ("oceanic", "continental"):
            v = sorted(l["v"] for l in all_lips if l["setting"] == setting)
            if v:
                print(f"  {setting}: {len(v)} LIPs, median {st.median(v):.2e} km³, p10 {v[int(0.1*(len(v)-1))]:.2e}, p90 {v[int(0.9*(len(v)-1))]:.2e}, max {v[-1]:.2e}, mean lid {st.mean(l['lid'] for l in all_lips if l['setting']==setting):.2f}, peak crust median {st.median(l['peak'] for l in all_lips if l['setting']==setting):.1f} km")
        print(f"  all: {len(vols)} LIPs over {seeds} × 1000 Myr = one per {seeds*1000/len(vols):.1f} Myr; ≥1e6 km³: one per {seeds*1000/sum(1 for x in vols if x>=1e6):.1f} Myr; ≥1e7: one per {seeds*1000/max(1,sum(1 for x in vols if x>=1e7)):.1f} Myr; total {sum(vols):.2e} km³ ({sum(vols)/seeds/1e9:.2f} km³/yr per run)")
        bins = [1e5, 3e5, 1e6, 3e6, 1e7, 3e7, 1e8 + 1]
        print("  volume histogram (km³ bins 1e5,3e5,1e6,3e6,1e7,3e7,1e8):", [sum(1 for x in vols if lo <= x < hi) for lo, hi in zip(bins, bins[1:])])
    # Time series means.
    keys = ["cont", "land", "temp", "pco2", "plumes", "sources", "m_ridges", "m_arcs", "m_LIPs", "m_plume tails"]
    times = sorted({s["t"] for _, snaps, *_ in per_seed for s in snaps})
    print("  t | " + " | ".join(keys))
    for t in times:
        vals = []
        for k in keys:
            xs = [s[k] for _, snaps, *_ in per_seed for s in snaps if s["t"] == t and s.get(k) is not None]
            vals.append(f"{st.mean(xs):.3g}" if xs else "-")
        print(f"  {t:.0f} | " + " | ".join(vals))
    print(f"  runtime mean {st.mean(s for *_, s, _ in per_seed):.0f} s")

for directory, label in zip(sys.argv[1:], ["volcanism", "main"]):
    summarise(directory, label)
