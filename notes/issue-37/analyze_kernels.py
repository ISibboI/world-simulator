"""Issue #37 option 2: compare the slab dynamic topography (and its side effects) of two runs.

Usage: python3 analyze_kernels.py NAME=RUN_DIR [NAME=RUN_DIR ...]
RUN_DIR/seed-S/{cells_*.csv, log.txt, seconds} from `runs submit ... --cell-dump`.
Window: snapshots 500..1000 Myr. Field `dyn` is the dry dynamic topography (m); under water the
visible deflection is x RHO_M / (RHO_M - RHO_W) = 1.45.
"""
import csv, glob, math, os, re, sys

import numpy as np
from scipy.special import sph_harm_y

WINDOW = (500, 1000)
THRESH = [-500, -1000, -1500, -2000]
WATER_LOADING = 3300.0 / (3300.0 - 1030.0)
BANDS = [(2, 3), (4, 8), (9, 16), (17, 30)]


def pct(a, p):
    return float(np.percentile(a, p))


def spectrum(lat, lon, f, lmax=30):
    """Degree RMS (m) of f on a near-uniform grid: sqrt(sum_m |c_lm|^2 / 4pi)."""
    theta = np.radians(90.0 - lat)
    phi = np.radians(lon)
    w = 4 * np.pi / len(f)
    out = np.zeros(lmax + 1)
    for l in range(1, lmax + 1):
        total = 0.0
        for m in range(0, l + 1):
            c = np.sum(f * np.conj(sph_harm_y(l, m, theta, phi))) * w
            total += abs(c) ** 2 * (1 if m == 0 else 2)
        out[l] = math.sqrt(total / (4 * np.pi))
    return out


def analyse(run):
    rows, spectra = [], []
    for sd in sorted(glob.glob(os.path.join(run, "seed-*"))):
        seed = os.path.basename(sd)
        seconds = None
        if os.path.exists(os.path.join(sd, "seconds")):
            seconds = float(open(os.path.join(sd, "seconds")).read().strip() or "nan")
        log = open(os.path.join(sd, "log.txt"), errors="replace").read()
        sea = {}
        time = None
        for line in log.splitlines():
            m = re.search(r"t =\s*([0-9.]+) Myr", line)
            if m:
                time = round(float(m.group(1)))
            m = re.search(r"sea level ([+-]?\d+) m \(dry offset", line)
            if m and time is not None:
                sea[time] = int(m.group(1))
        for f in sorted(glob.glob(os.path.join(sd, "cells_*.csv"))):
            t = int(re.search(r"cells_(\d+)", f).group(1))
            if not WINDOW[0] <= t <= WINDOW[1]:
                continue
            data = {k: [] for k in ("dyn", "elev", "cls", "lat", "lon")}
            for r in csv.DictReader(open(f)):
                for k in data:
                    data[k].append(float(r[k]))
            dyn, elev, cls = (np.array(data[k]) for k in ("dyn", "elev", "cls"))
            land = elev >= 0
            cont = cls == 0
            row = dict(seed=seed, time=t, seconds=seconds, sea=sea.get(t, float("nan")),
                       min=dyn.min(), max=dyn.max(), p1=pct(dyn, 1), p99=pct(dyn, 99),
                       rms=float(np.sqrt(np.mean(dyn ** 2))),
                       land_min=dyn[land].min() if land.any() else 0.0,
                       wet_min=dyn[~land].min() * WATER_LOADING if (~land).any() else 0.0,
                       cont_abs=float(np.mean(np.abs(dyn[cont]))) if cont.any() else 0.0,
                       land_frac=land.mean() * 100,
                       flooded_cont=(cont & ~land).sum() / max(1, cont.sum()) * 100,
                       land_p10=pct(elev[land], 10), land_p50=pct(elev[land], 50), land_p90=pct(elev[land], 90),
                       ocean_p10=pct(elev[~land], 10), ocean_p50=pct(elev[~land], 50), ocean_p90=pct(elev[~land], 90))
            for th in THRESH:
                sel = dyn < th
                row[f"area{th}"] = sel.mean() * 100
                row[f"dryland{th}"] = (sel & land).sum() / max(1, sel.sum()) * 100
            rows.append(row)
            if t == WINDOW[1]:
                spectra.append(spectrum(np.array(data["lat"]), np.array(data["lon"]), dyn))
    return rows, spectra


def main():
    results = {}
    for arg in sys.argv[1:]:
        name, run = arg.split("=", 1)
        results[name] = analyse(run)
    names = list(results)
    keys = [k for k in results[names[0]][0][0] if k not in ("seed", "time")]
    print(f"{'quantity (mean over snapshots 500-1000 Myr, all seeds)':58s}" + "".join(f"{n:>14s}" for n in names))
    for k in keys:
        line = f"{k:58s}"
        for n in names:
            vals = [r[k] for r in results[n][0] if r[k] is not None and not (isinstance(r[k], float) and math.isnan(r[k]))]
            line += f"{(sum(vals) / len(vals) if vals else float('nan')):14.1f}"
        print(line)
    print("\ndeepest point per seed over 500-1000 Myr (m, dry):")
    for n in names:
        seeds = sorted({r["seed"] for r in results[n][0]})
        mins = [min(r["min"] for r in results[n][0] if r["seed"] == s) for s in seeds]
        print(f"  {n:10s} " + " ".join(f"{m:6.0f}" for m in mins) + f"  | mean {np.mean(mins):6.0f}")
    print("\ndegree RMS of the field at 1000 Myr (m, mean over seeds): " + ", ".join(f"l {a}-{b}" for a, b in BANDS) + ", l=1")
    for n in names:
        sp = np.mean(results[n][1], axis=0) if results[n][1] else None
        if sp is None:
            continue
        bands = [math.sqrt(sum(sp[l] ** 2 for l in range(a, b + 1))) for a, b in BANDS]
        print(f"  {n:10s} " + " ".join(f"{b:7.0f}" for b in bands) + f" {sp[1]:7.0f}" + "   per degree 1-10: " + " ".join(f"{sp[l]:.0f}" for l in range(1, 11)))


if __name__ == "__main__":
    main()
