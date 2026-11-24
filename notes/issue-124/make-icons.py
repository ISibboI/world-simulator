#!/usr/bin/env python3
"""Makes the favicons and app icons (issue #124) from the planet screenshot source.png.

Usage: python3 notes/issue-124/make-icons.py [source.png] [output dir]
Defaults: notes/issue-124/source.png and web/icons/ (run from the repository root).
Needs Pillow and NumPy.

The planet disc is found by fitting a circle to its outline in the screenshot (the dark space
background is uniform), cropped to a square around it and scaled down. Outside the disc the icons
are transparent (anti-aliased edge), except apple-touch-icon.png, which iOS wants opaque: the
planet on the space background of the web page. The small sizes (16 to 48 px) get a little more
contrast and saturation so that land and sea stay distinct.
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageEnhance

SPACE = (5, 5, 15)  # The background of the view and of web/index.html (#05050f).


def fit_disc(image):
    """Returns (cx, cy, r) of the planet disc: a least-squares circle through the outline."""
    rgb = np.asarray(image.convert("RGB"), dtype=np.int32)
    planet = np.abs(rgb - np.array(SPACE)).sum(axis=2) > 40
    points = []
    for y in range(planet.shape[0]):
        xs = np.flatnonzero(planet[y])
        if xs.size:
            points += [(xs[0], y), (xs[-1], y)]
    p = np.array(points, dtype=np.float64)
    # Only the middle rows: mountains stick out of the disc at the top of the screenshot, and the
    # dark limb at the bottom is hard to tell from space.
    top, height = p[:, 1].min(), np.ptp(p[:, 1])
    p = p[(p[:, 1] > top + 0.15 * height) & (p[:, 1] < top + 0.85 * height)]
    # x² + y² = 2 cx x + 2 cy y + c
    a = np.column_stack([2 * p[:, 0], 2 * p[:, 1], np.ones(len(p))])
    b = (p**2).sum(axis=1)
    (cx, cy, c), *_ = np.linalg.lstsq(a, b, rcond=None)
    return cx, cy, np.sqrt(c + cx**2 + cy**2)


def disc_alpha(size, radius):
    """An anti-aliased disc of the given radius (pixels) centred in a size × size image."""
    scale = 8
    n = size * scale
    yy, xx = np.mgrid[0:n, 0:n]
    centre = n / 2
    inside = ((xx + 0.5 - centre) ** 2 + (yy + 0.5 - centre) ** 2) < (radius * scale) ** 2
    alpha = inside.reshape(size, scale, size, scale).mean(axis=(1, 3))
    return Image.fromarray(np.round(alpha * 255).astype(np.uint8), "L")


def planet(source, disc, size, boost=False):
    """The planet scaled to size × size pixels, and the alpha mask of its disc."""
    cx, cy, half = disc
    # Slightly inside the outline: its outermost pixels are darkened by the limb.
    half *= 0.99
    square = source.convert("RGB").resize(
        (size, size), Image.LANCZOS, box=(cx - half, cy - half, cx + half, cy + half)
    )
    if boost:
        square = ImageEnhance.Contrast(square).enhance(1.15)
        square = ImageEnhance.Color(square).enhance(1.2)
    return square, disc_alpha(size, size / 2)


def transparent(source, disc, size):
    square, alpha = planet(source, disc, size, boost=size <= 48)
    square.putalpha(alpha)
    return square


def main():
    root = Path(__file__).resolve().parents[2]
    src = Path(sys.argv[1]) if len(sys.argv) > 1 else root / "notes/issue-124/source.png"
    out = Path(sys.argv[2]) if len(sys.argv) > 2 else root / "web/icons"
    out.mkdir(parents=True, exist_ok=True)
    source = Image.open(src)
    disc = fit_disc(source)
    print("planet disc: centre (%.1f, %.1f), radius %.1f px" % disc)

    icons = {size: transparent(source, disc, size) for size in (16, 32, 48, 192, 256, 512)}
    icons[48].save(
        out / "favicon.ico", sizes=[(16, 16), (32, 32), (48, 48)],
        append_images=[icons[16], icons[32]],
    )
    for size in (32, 192, 256, 512):
        icons[size].save(out / f"icon-{size}.png", optimize=True)

    # iOS: opaque, with a margin (it rounds the corners).
    square, alpha = planet(source, disc, 156)
    touch = Image.new("RGB", (180, 180), SPACE)
    touch.paste(square, (12, 12), alpha)
    touch.save(out / "apple-touch-icon.png", optimize=True)

    # A preview for the pull request: the icons at 1:1 and the small ones enlarged.
    preview = Image.new("RGBA", (16 + 32 + 48 + 180 + 96 * 3 + 9 * 10, 200), (255, 255, 255, 255))
    x = 10
    for image in (icons[16], icons[32], icons[48], touch.convert("RGBA")):
        preview.alpha_composite(image, (x, 10))
        x += image.width + 10
    for size in (16, 32, 48):
        big = icons[size].resize((96, 96), Image.NEAREST)
        preview.alpha_composite(big, (x, 10))
        x += 106
    dark = Image.new("RGBA", preview.size, (40, 40, 48, 255))
    x = 10
    for image in (icons[16], icons[32], icons[48]):
        dark.alpha_composite(image, (x, 10))
        x += image.width + 10
    sheet = Image.new("RGBA", (preview.width, 300), (255, 255, 255, 255))
    sheet.alpha_composite(preview, (0, 0))
    sheet.alpha_composite(dark.crop((0, 0, preview.width, 100)), (0, 200))
    sheet.convert("RGB").save(Path(__file__).resolve().parent / "preview.png", optimize=True)


if __name__ == "__main__":
    main()
