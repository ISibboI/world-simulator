#!/usr/bin/env python3
"""Compare arms of 8-seed runs for #51/#54 from their log.txt files.

Usage: compare.py ARM=DIR [ARM=DIR ...] [--window A B]
Each DIR holds seed-N/log.txt. Prints per-arm means over seeds (seed range) and Welch t of
each arm against the first.
"""
import math
import re
import sys
from pathlib import Path

CAUSES = ["plume rule", "stress opening", "stress other", "sheet", "initiation"]


def parse(log):
    snaps = []
    cur = None
    for line in open(log, encoding="utf-8", errors="replace"):
        m = re.search(r"t =\s*([\d.]+) Myr \| continental\s+([\d.]+)% \| land\s+([\d.]+)%.*land mean\s+(-?\d+) m.*?(\d+) plates \((\d+) born, (\d+) gone\), largest\s+([\d.]+)%, (\d+) plumes", line)
        if m:
            cur = dict(t=float(m[1]), continental=float(m[2]), land=float(m[3]), land_mean=float(m[4]),
                       plates=int(m[5]), born=int(m[6]), gone=int(m[7]), largest_plate=float(m[8]), plumes=int(m[9]))
            snaps.append(cur)
            continue
        if cur is None:
            continue
        m = re.search(r"crust of continental origin ([\d.]+)% of the surface, (\d+)% flooded.*?(\d+)% > 50 km", line)
        if m:
            cur.update(cont_origin=float(m[1]), flooded=float(m[2]), thick50=float(m[3]))
        m = re.search(r"continental crust masses (\d+) \((\d+) specks, largest (\d+)%\)", line)
        if m:
            cur.update(largest_continent=float(m[3]))
            m2 = re.search(r"land masses (\d+) \((\d+) specks, largest (\d+)%\)", line)
            cur.update(land_masses=int(m2[1]) - int(m2[2]))
        m = re.search(r"deformation: strain rate ≥ 1e-16/s ([\d.]+)% of the surface \(continental ([\d.]+)%", line)
        if m:
            cur.update(deforming=float(m[1]), deforming_cont=float(m[2]))
        m = re.search(r"\| splits (\d+) \|", line)
        if "deformation:" in line:
            m = re.search(r"fronts vacated \d+, grown \d+ \| splits (\d+)", line)
            if m:
                cur.update(sheet_splits=int(m[1]))
        if "splits by cause" in line:
            for cause in CAUSES:
                m = re.search(re.escape(cause) + r" (\d+) \((\d+)/(\d+) at plumes, (\d+)/(\d+) at antipodes, mean distance ([\d.]+)/([\d.]+) rad; boundary ([\d/]+), plates ([\d/]+)\)", line)
                if m:
                    cur[cause] = dict(count=int(m[1]), near=(int(m[2]), int(m[3])), anti=(int(m[4]), int(m[5])),
                                      dist=float(m[6]) * int(m[1]), antidist=float(m[7]) * int(m[1]),
                                      boundary=[int(x) for x in m[8].split("/")], plates=[int(x) for x in m[9].split("/")])
        m = re.search(r"breakup by stress: (\d+) plates broken \((\d+) cells; (\d+) opening, (\d+) closing, the others sliding(?:; (\d+) through a plume\), plume sections by overload \(< 0.25, < 0.5, < 1, ≥ 1\) \[(\d+), (\d+), (\d+), (\d+)\])?.*?, ([\d.]+) s$", line.strip())
        if m:
            cur.update(breakups=int(m[1]), opening=int(m[3]), closing=int(m[4]),
                       plume_splits=int(m[5]) if m[5] else 0,
                       plume_overloads=[int(m[i]) for i in range(6, 10)] if m[6] else [0, 0, 0, 0],
                       breakup_seconds=float(m[10]))
        m = re.search(r"extension fabric: mean ([\d.]+) on continental crust, ≥ [\d.]+ on ([\d.]+)% near margins and ([\d.]+)% deep inside continents.*shortening on fabric ([\d.]+)% of ([\d.e+-]+)", line)
        if m:
            cur.update(fabric_mean=float(m[1]), fabric_margin=float(m[2]), fabric_interior=float(m[3]),
                       inversion_share=float(m[4]), shortening=float(m[5]))
        m = re.search(r"Finished in ([\d.]+)(m?s)", line)
        if m:
            pass
    return snaps


def at(snaps, t):
    best = min(snaps, key=lambda s: abs(s["t"] - t))
    return best if abs(best["t"] - t) < 1e-6 else None


def window_mean(snaps, key, a, b):
    vals = [s[key] for s in snaps if a <= s["t"] <= b and key in s]
    return sum(vals) / len(vals) if vals else float("nan")


def rate(snaps, f, a, b):
    """Change of a cumulative quantity f(snap) per 100 Myr over [a, b]."""
    sa, sb = at(snaps, a), at(snaps, b)
    if sa is None or sb is None:
        return float("nan")
    return (f(sb) - f(sa)) / (b - a) * 100


def cause(s, c, field="count", k=None):
    d = s.get(c)
    if not d:
        return 0
    v = d[field]
    return v[k] if k is not None else v


def metrics(snaps, a, b):
    out = {}
    end = max(s["t"] for s in snaps)
    b = min(b, end)
    out["plume rule splits /100Myr"] = rate(snaps, lambda s: cause(s, "plume rule"), a, b)
    for c in CAUSES[1:]:
        out[f"{c} /100Myr"] = rate(snaps, lambda s, c=c: cause(s, c), a, b)
    allc = lambda s: sum(cause(s, c) for c in CAUSES)
    near = lambda k: (lambda s: sum(cause(s, c, "near", k) for c in CAUSES))
    anti = lambda k: (lambda s: sum(cause(s, c, "anti", k) for c in CAUSES))
    nonrule = lambda f: (lambda s: f(s) - (cause(s, "plume rule", "near", 0) if f is near(0) else 0))
    out["all splits /100Myr"] = rate(snaps, allc, a, b)
    out["splits ≤500 km of plume /100Myr"] = rate(snaps, near(0), a, b)
    out["splits ≤500 km of antipode /100Myr"] = rate(snaps, anti(0), a, b)
    out["splits ≤1000 km of plume /100Myr"] = rate(snaps, near(1), a, b)
    out["splits ≤1000 km of antipode /100Myr"] = rate(snaps, anti(1), a, b)
    out["opening breakups ≤500 km of plume /100Myr"] = rate(snaps, lambda s: cause(s, "stress opening", "near", 0), a, b)
    out["breakups through plume sections /100Myr"] = rate(snaps, lambda s: s.get("plume_splits", 0), a, b)
    out["births /100Myr"] = rate(snaps, lambda s: s["born"], a, b)
    out["deaths /100Myr"] = rate(snaps, lambda s: s["gone"], a, b)
    for key in ["plates", "largest_plate", "continental", "cont_origin", "land", "land_mean", "flooded", "thick50",
                "largest_continent", "land_masses", "deforming", "deforming_cont", "plumes",
                "fabric_mean", "fabric_margin", "fabric_interior"]:
        out[key] = window_mean(snaps, key, a, b)
    # Reactivation shares over [a, b]: boundary vs plate shares of belts / fabric.
    def share(k, which):
        d = lambda s: sum(cause(s, c, which)[k] if len(cause(s, c, which) or []) > k else 0 for c in CAUSES)
        tot = lambda s: sum(cause(s, c, which)[0] if cause(s, c, which) else 0 for c in CAUSES)
        sa, sb = at(snaps, a), at(snaps, b)
        if sa is None or sb is None:
            return float("nan")
        n = tot(sb) - tot(sa)
        return 100 * (d(sb) - d(sa)) / n if n else float("nan")
    for k, name in [(1, "belts"), (2, "fabric"), (3, "interior fabric")]:
        out[f"{name} % on new boundaries"] = share(k, "boundary")
        out[f"{name} % in split plates"] = share(k, "plates")
    # Supercontinent: when the largest continental mass first falls below 50% and 35% of the
    # continental crust, and aggregation cycles (up-crossings of 50% after a fall below 35%).
    series = [(s["t"], s.get("largest_continent", float("nan"))) for s in snaps]
    for limit in (50, 35):
        out[f"first t with largest continent < {limit}%"] = next(
            (t for t, v in series if t > 0 and v < limit), float("nan"))
    cycles, low = 0, False
    for t, v in series:
        if v < 35:
            low = True
        elif v >= 50 and low:
            cycles += 1
            low = False
    out["assemblies (largest continent back ≥ 50% after < 35%)"] = cycles
    out["max largest continent % after 100 Myr"] = max((v for t, v in series if t >= 100), default=float("nan"))
    sa, sb = at(snaps, a), at(snaps, b)
    if sa and sb and "shortening" in sb:
        out["inversion: shortening on fabric %"] = sb.get("inversion_share", float("nan"))
    return out


def mean_sd(vals):
    vals = [v for v in vals if not math.isnan(v)]
    if not vals:
        return float("nan"), float("nan"), 0
    m = sum(vals) / len(vals)
    sd = math.sqrt(sum((v - m) ** 2 for v in vals) / (len(vals) - 1)) if len(vals) > 1 else 0.0
    return m, sd, len(vals)


def welch(a, b):
    ma, sa, na = mean_sd(a)
    mb, sb, nb = mean_sd(b)
    if na < 2 or nb < 2:
        return float("nan")
    se = math.sqrt(sa * sa / na + sb * sb / nb)
    return (mb - ma) / se if se > 0 else float("nan")


def main():
    args = sys.argv[1:]
    window = (500.0, 1000.0)
    if "--window" in args:
        i = args.index("--window")
        window = (float(args[i + 1]), float(args[i + 2]))
        del args[i:i + 3]
    arms = []
    for spec in args:
        name, d = spec.split("=", 1)
        runs = []
        for log in sorted(Path(d).glob("seed-*/log.txt")):
            snaps = parse(log)
            if snaps:
                runs.append(metrics(snaps, *window))
        arms.append((name, runs))
    keys = list(arms[0][1][0].keys())
    print(f"window {window[0]:.0f}–{window[1]:.0f} Myr; mean over seeds [min–max]; t vs {arms[0][0]}")
    print("| metric | " + " | ".join(f"{n} (n={len(r)})" for n, r in arms) + " |")
    print("|---|" + "---|" * len(arms))
    for k in keys:
        cells = []
        base = [r.get(k, float("nan")) for r in arms[0][1]]
        for i, (n, runs) in enumerate(arms):
            vals = [r.get(k, float("nan")) for r in runs]
            m, sd, cnt = mean_sd(vals)
            fin = [v for v in vals if not math.isnan(v)]
            rng = f" [{min(fin):.2f}–{max(fin):.2f}]" if fin else ""
            t = "" if i == 0 else f" t={welch(base, vals):+.1f}"
            cells.append(f"{m:.2f}{rng}{t}")
        print(f"| {k} | " + " | ".join(cells) + " |")


if __name__ == "__main__":
    main()
