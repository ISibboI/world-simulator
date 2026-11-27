#!/usr/bin/env python3
"""Map of the extension fabric (Crust::rifted) and the lithosphere strength of one snapshot.

Usage: fabric_map.py RUN_DIR TIME OUT.png
RUN_DIR holds mesh.csv, cells_TTTTT.csv and boundaries_TTTTT.csv (--cell-dump); plume
positions are read from the log if given as RUN_DIR/log.txt (not needed).
"""
import csv
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np


def main():
    run, time, out = sys.argv[1], int(float(sys.argv[2])), sys.argv[3]
    cells = list(csv.DictReader(open(f"{run}/cells_{time:05d}.csv")))
    lon = np.array([float(c["lon"]) for c in cells])
    lat = np.array([float(c["lat"]) for c in cells])
    rifted = np.array([float(c["rifted"]) for c in cells])
    strength = np.array([float(c["strength"]) for c in cells])
    continental = np.array([c["cls"] == "0" for c in cells])
    plate = np.array([int(c["plate"]) for c in cells])
    neighbours = [list(map(int, r["neighbors"].split(";"))) for r in csv.DictReader(open(f"{run}/mesh.csv"))]
    boundary = np.array([any(plate[n] != plate[i] for n in neighbours[i]) for i in range(len(cells))])
    fig, axes = plt.subplots(2, 1, figsize=(10, 10), constrained_layout=True)
    for ax, values, title, cmap, vmax in [
        (axes[0], np.where(continental, rifted, np.nan), f"Extension fabric on continental crust, {time} Myr", "magma_r", 1.0),
        (axes[1], strength, f"Lithosphere strength, {time} Myr", "viridis", 1.0),
    ]:
        ax.set_facecolor("#dddddd")
        ax.scatter(lon[~continental], lat[~continental], s=1.2, c="#b9cfe0", marker="s", linewidths=0)
        sc = ax.scatter(lon, lat, s=1.2, c=values, cmap=cmap, vmin=0, vmax=vmax, marker="s", linewidths=0)
        ax.scatter(lon[boundary], lat[boundary], s=0.6, c="k", marker="s", linewidths=0)
        ax.set_xlim(-180, 180)
        ax.set_ylim(-90, 90)
        ax.set_title(title)
        fig.colorbar(sc, ax=ax, shrink=0.8)
    fig.savefig(out, dpi=110)


if __name__ == "__main__":
    main()
