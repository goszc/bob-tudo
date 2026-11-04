#!/usr/bin/env python3
"""Rasterize the bob tudo mark into png and ico sizes."""

from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
ICON_DIR = ROOT / "src-tauri" / "icons"
ART = Path("/opt/cursor/artifacts")
STORE = Path("/cursor/stores/self/artifacts")

FACE = (47, 123, 255, 255)
EYE = (16, 32, 51, 255)
LEDGE = (36, 81, 181, 255)
INK = (17, 17, 17, 255)


def draw(size: int, mono: bool = False) -> Image.Image:
    im = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    s = size / 32.0

    def box(x, y, w, h):
        return [x * s, y * s, (x + w) * s, (y + h) * s]

    face = INK if mono else FACE
    ledge = INK if mono else LEDGE
    d.ellipse(box(18.2 - 10, 13.6 - 10, 20, 20), fill=face)
    d.rounded_rectangle(box(1, 18, 20.5, 12.5), radius=4.2 * s, fill=ledge)
    if mono:
        hole = Image.new("L", (size, size), 0)
        hd = ImageDraw.Draw(hole)
        for cx in (14.3, 21.7):
            hd.ellipse(box(cx - 2.05, 12.8 - 2.55, 4.1, 5.1), fill=255)
        pix = im.load()
        hp = hole.load()
        for y in range(size):
            for x in range(size):
                if hp[x, y]:
                    pix[x, y] = (0, 0, 0, 0)
    else:
        for cx in (14.3, 21.7):
            d.ellipse(box(cx - 2.05, 12.8 - 2.55, 4.1, 5.1), fill=EYE)
    return im


def main() -> None:
    ICON_DIR.mkdir(parents=True, exist_ok=True)
    ART.mkdir(parents=True, exist_ok=True)
    draw(32).save(ICON_DIR / "32x32.png")
    draw(128).save(ICON_DIR / "128x128.png")
    draw(256).save(ICON_DIR / "128x128@2x.png")
    draw(256).save(ICON_DIR / "icon.png")
    sizes = [16, 24, 32, 48, 64, 128, 256]
    images = [draw(s) for s in sizes]
    images[0].save(
        ICON_DIR / "icon.ico",
        format="ICO",
        sizes=[(im.width, im.height) for im in images],
        append_images=images[1:],
    )
    draw(32, mono=True).save(ICON_DIR / "tray-mono.png")
    preview = draw(512)
    preview.save(ART / "logo.png")
    preview.save(STORE / "logo.png")
    for src, name in ((ROOT / "assets" / "logo.svg", "logo.svg"), (ROOT / "assets" / "logo-mono.svg", "logo-mono.svg")):
        data = src.read_bytes()
        (ART / name).write_bytes(data)
        (STORE / name).write_bytes(data)
    print("icons", ICON_DIR)


if __name__ == "__main__":
    main()
