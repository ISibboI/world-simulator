#!/usr/bin/env python3
"""Summarise #202 runs: python3 analyse.py RUNDIR [RUNDIR...] (each with seed-*/log.txt, seconds).

Per snapshot time (means over seeds): global T, CO2 (bar) and its forcing, total greenhouse
forcing change, H2 and CH4 mixing ratios, gas forcing, sources and sinks (Tmol/yr), escape H
flux, water loss, O2 in the air; then per-seed end values and runtimes."""
import glob, os, re, sys, statistics as st

num = r"([-+]?[0-9.]+(?:e[-+]?[0-9]+)?)"
TIMES = (2, 10, 20, 50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 1000)


def parse(path):
    snaps, t, cur = [], None, {}
    for line in open(path, errors="replace"):
        m = re.search(r"INFO world_simulator: t =\s+([0-9.]+) Myr", line)
        if m:
            t = float(m.group(1))
        m = re.search(r"temperature global\s+" + num, line)
        if m:
            cur["T"] = float(m.group(1)); cur["t"] = t
        m = re.search(r"carbon: pCO₂ " + num + r" bar .*?forcing " + num, line)
        if m:
            cur["co2"] = float(m.group(1)); cur["fco2"] = float(m.group(2))
        m = re.search(r"escape: tropics .*?\| H " + num + r" cm⁻² s⁻¹ \((\S+);.*?water " + num
                      + r" kg/yr.*?in the air " + num + r" mol \(" + num + r" bar\)", line)
        if m:
            cur.update(H=float(m.group(1)), regime=m.group(2), wrate=float(m.group(3)),
                       po2=float(m.group(5)))
            g = re.search(r"reduced gases: H₂ " + num + r" \(" + num + r" mol\), CH₄ " + num + r" \("
                          + num + r" mol\), CH₄/CO₂ " + num + r" \| forcing " + num
                          + r" W/m² \(CH₄ " + num + r", H₂ CIA " + num + r"\) \| sources Tmol/yr: "
                          r"volcanic H₂ " + num + r", CO " + num + r", serpentinization H₂ " + num
                          + r" \(serpentinite " + num + r" kg/kg\), CH₄ " + num
                          + r" \| sinks Tmol/yr: H₂ escape " + num + r", CH₄ escape " + num
                          + r", CH₄ photolysis " + num + r", by the escape's O₂ H₂ " + num
                          + r" CH₄ " + num + r", by background O₂ H₂ " + num + r" CH₄ " + num
                          + r" \| H " + num, line)
            if g:
                v = [float(x) for x in g.groups()]
                keys = ["fh2", "nh2", "fch4", "nch4", "ratio", "fgas", "fch4f", "fcia", "volc",
                        "co", "serp", "serpshare", "ch4src", "h2esc", "ch4esc", "ch4ph",
                        "h2tit", "ch4tit", "h2ox", "ch4ox", "Hgas"]
                cur.update(dict(zip(keys, v)))
            snaps.append(cur); cur = {}
    return snaps


def fmt(x, f):
    return format(x, f) if x == x else "  —  "


for d in sys.argv[1:]:
    runs = {p: parse(p) for p in sorted(glob.glob(f"{d}/seed-*/log.txt"))}
    runs = {p: s for p, s in runs.items() if s}
    secs = []
    for p in runs:
        try:
            secs.append(float(open(os.path.join(os.path.dirname(p), "seconds")).read()))
        except Exception:
            pass
    print(f"== {d}: {len(runs)} seeds, runtime {st.mean(secs) if secs else float('nan'):.0f} s")
    gas = any("fh2" in s for ss in runs.values() for s in ss)
    hdr = " t(Myr)  T(°C)  CO2(bar) F_CO2   H(cm-2s-1) water(kg/yr) O2(bar)"
    if gas:
        hdr += " | f_H2     f_CH4    CH4/CO2  F_gas  F_CH4  | volc  CO    serp  (share) CH4src | H2esc CH4esc CH4ph  titr(H2) bgox(H2)"
    print(hdr)
    for t in TIMES:
        rows = [s for ss in runs.values() for s in ss if s.get("t") == t]
        if not rows:
            continue
        m = lambda k: st.mean(r.get(k, float("nan")) for r in rows)
        line = (f" {t:6.0f} {m('T'):6.2f}  {m('co2'):.4f}  {m('fco2'):6.2f}  {m('H'):.2e}   {m('wrate'):.2e}  {m('po2'):.1e}")
        if gas:
            line += (f" | {m('fh2'):.2e} {m('fch4'):.2e} {m('ratio'):.1e} {m('fgas'):+.3f} {m('fch4f'):+.3f}"
                     f" | {m('volc'):.3f} {m('co'):.3f} {m('serp'):.3f} ({m('serpshare'):.4f}) {m('ch4src'):.4f}"
                     f" | {m('h2esc'):.3f} {m('ch4esc'):.4f} {m('ch4ph'):.4f} {m('h2tit'):.4f} {m('h2ox'):.4f}")
        print(line)
    print(" per seed (end): T, CO2, f_H2, f_CH4, F_gas, seconds")
    for p, ss in runs.items():
        s = ss[-1]
        sd = os.path.dirname(p)
        sec = open(os.path.join(sd, "seconds")).read().strip() if os.path.exists(os.path.join(sd, "seconds")) else "?"
        print(f"   {sd.split('/')[-1]}: t {s.get('t')}, T {s.get('T', float('nan')):.2f}, CO2 {s.get('co2', float('nan')):.4f},"
              f" f_H2 {s.get('fh2', float('nan')):.2e}, f_CH4 {s.get('fch4', float('nan')):.2e}, F_gas {s.get('fgas', float('nan')):+.3f}, {sec} s")
