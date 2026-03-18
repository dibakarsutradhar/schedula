#!/usr/bin/env python3
"""
Generate Schedula app icons from scratch.
Matches the landing-page logo-mark: 3×3 grid of rounded squares.
  cell 2 (top-mid)   — blue  #3b82f6  opacity 0.9
  cell 5 (center)    — gold  #f59e0b  opacity 0.9
  cell 8 (bottom-mid)— blue  #3b82f6  opacity 0.7
  rest               — light blue rgba(147,197,253,0.45)

macOS guidelines: ~10 % visual padding on each side so the icon
doesn't appear oversized in the Dock compared to other apps.
"""

import os
import math
import shutil
import subprocess
from PIL import Image, ImageDraw

CANVAS  = 1024          # total canvas
PADDING = int(CANVAS * 0.10)   # 10 % each side → icon grid lives in 80 % of canvas
GRID_SZ = CANVAS - 2 * PADDING  # 819 px
COLS = ROWS = 3
GAP_FRAC = 0.08         # gap between cells as fraction of cell size

# Colours  (on a dark #0b1426 background, matching the landing-page dark theme)
BG          = (11,  20,  38)          # --bg-deep: #0b1426
BLUE        = (59, 130, 246)          # #3b82f6
GOLD        = (245, 158,  11)         # #f59e0b
BLUE_LT     = (147, 197, 253)         # #93c5fd  (light blue cells)

CELL_ALPHA  = { 1: 0.45,  2: 0.90,  3: 0.45,
                4: 0.45,  5: 0.90,  6: 0.45,
                7: 0.45,  8: 0.70,  9: 0.45 }
CELL_COLOR  = { 1: BLUE_LT, 2: BLUE, 3: BLUE_LT,
                4: BLUE_LT, 5: GOLD, 6: BLUE_LT,
                7: BLUE_LT, 8: BLUE, 9: BLUE_LT }


def blend(fg, alpha, bg=BG):
    return tuple(int(fg[i] * alpha + bg[i] * (1 - alpha)) for i in range(3))


def draw_icon(size):
    img  = Image.new("RGBA", (size, size), BG + (255,))
    draw = ImageDraw.Draw(img)

    pad     = PADDING * size / CANVAS
    grid_sz = GRID_SZ  * size / CANVAS
    cell_sz = grid_sz  / COLS
    gap     = cell_sz  * GAP_FRAC
    inner   = cell_sz  - gap
    radius  = max(2, inner * 0.22)   # rounded corners

    for idx in range(1, 10):
        row = (idx - 1) // COLS
        col = (idx - 1) %  COLS
        x0  = pad + col * cell_sz + gap / 2
        y0  = pad + row * cell_sz + gap / 2
        x1  = x0 + inner
        y1  = y0 + inner
        color = blend(CELL_COLOR[idx], CELL_ALPHA[idx])
        draw.rounded_rectangle([x0, y0, x1, y1], radius=radius, fill=color + (255,))

    return img


def make_iconset(base_dir):
    iconset = os.path.join(base_dir, "AppIcon.iconset")
    os.makedirs(iconset, exist_ok=True)
    specs = [
        ("icon_16x16.png",       16),
        ("icon_16x16@2x.png",    32),
        ("icon_32x32.png",       32),
        ("icon_32x32@2x.png",    64),
        ("icon_128x128.png",    128),
        ("icon_128x128@2x.png", 256),
        ("icon_256x256.png",    256),
        ("icon_256x256@2x.png", 512),
        ("icon_512x512.png",    512),
        ("icon_512x512@2x.png",1024),
    ]
    for filename, sz in specs:
        img = draw_icon(sz)
        img.save(os.path.join(iconset, filename))
        print(f"  {filename}")
    return iconset


def main():
    here    = os.path.dirname(os.path.abspath(__file__))
    icons   = os.path.join(here, "icons")
    os.makedirs(icons, exist_ok=True)

    print("Generating icons …")
    iconset = make_iconset(here)

    # macOS .icns
    icns_path = os.path.join(icons, "icon.icns")
    subprocess.run(["iconutil", "-c", "icns", iconset, "-o", icns_path], check=True)
    print(f"  icon.icns")

    # PNG sizes needed by Tauri (overwrite existing placeholders)
    for sz, name in [(32, "32x32.png"), (128, "128x128.png"), (256, "128x128@2x.png")]:
        draw_icon(sz).convert("RGB").save(os.path.join(icons, name))
        print(f"  {name}")

    # 1024 source PNG (useful reference)
    draw_icon(1024).save(os.path.join(icons, "icon.png"))
    print(f"  icon.png")

    # Windows .ico — pack 16,32,48,256
    ico_imgs = [draw_icon(s).convert("RGBA") for s in [16, 32, 48, 256]]
    ico_imgs[0].save(
        os.path.join(icons, "icon.ico"),
        format="ICO",
        sizes=[(16,16),(32,32),(48,48),(256,256)],
        append_images=ico_imgs[1:],
    )
    print("  icon.ico")

    # Cleanup temp iconset
    shutil.rmtree(iconset)
    print("Done.")


if __name__ == "__main__":
    main()
