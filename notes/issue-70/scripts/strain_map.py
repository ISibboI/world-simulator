"""Equirectangular map of the strain rate from a cell dump (#70), nearest cell per pixel.
usage: strain_map.py cells_TTTTT.csv out.png [width]
Grey: rigid (< 1e-17/s); blue-yellow-red: log10 strain rate 1e-17 … 1e-14 /s; continental
crust slightly lighter; plate boundaries black."""
import csv, math, sys
import numpy as np
from PIL import Image

rows = list(csv.DictReader(open(sys.argv[1])))
width = int(sys.argv[3]) if len(sys.argv) > 3 else 512
height = width // 2
lat = np.radians([float(r['lat']) for r in rows])
lon = np.radians([float(r['lon']) for r in rows])
xyz = np.stack([np.cos(lat) * np.cos(lon), np.cos(lat) * np.sin(lon), np.sin(lat)], 1)
strain = np.array([float(r['strain']) for r in rows])
plate = np.array([int(r['plate']) for r in rows])
cont = np.array([r['cls'] == '0' for r in rows])
img = np.zeros((height, width, 3), np.uint8)
plat = np.radians(90 - (np.arange(height) + 0.5) * 180 / height)
plon = np.radians(-180 + (np.arange(width) + 0.5) * 360 / width)
idx = np.zeros((height, width), int)
for y in range(height):
    p = np.stack([np.cos(plat[y]) * np.cos(plon), np.cos(plat[y]) * np.sin(plon),
                  np.full(width, np.sin(plat[y]))], 1)
    idx[y] = np.argmax(p @ xyz.T, axis=1)
s = strain[idx]
t = np.clip((np.log10(np.maximum(s, 1e-30)) + 17) / 3, 0, 1)
stops = np.array([[40, 70, 200], [250, 220, 50], [200, 20, 20]], float)
col = np.where(t[..., None] < 0.5,
               stops[0] + (stops[1] - stops[0]) * (t[..., None] * 2),
               stops[1] + (stops[2] - stops[1]) * (t[..., None] * 2 - 1))
grey = np.where(cont[idx][..., None], 150, 95)
rigid = (s < 1e-17)[..., None]
img = np.where(rigid, grey, col).astype(np.uint8)
pl = plate[idx]
edge = np.zeros_like(pl, bool)
edge[:, 1:] |= pl[:, 1:] != pl[:, :-1]
edge[1:, :] |= pl[1:, :] != pl[:-1, :]
img[edge] = 0
Image.fromarray(img).save(sys.argv[2])
