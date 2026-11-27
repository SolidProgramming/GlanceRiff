"""Draws windows/glanceriff.ico (dashboard colors, a history graph on a dark tile).

No dependencies: python windows/make_icon.py
"""

import struct
import zlib
from pathlib import Path

BG = (0x17, 0x1C, 0x22)  # --panel
ACCENT = (0x4F, 0xC3, 0xF7)  # --accent
FILL_ALPHA = 0.32
LINE_WIDTH = 0.09
MARGIN, RADIUS = 0.04, 0.2
# Graph in unit coordinates (y points down), running edge to edge of the tile.
POINTS = [(0.0, 0.70), (0.26, 0.50), (0.44, 0.62), (0.64, 0.28), (1.0, 0.46)]
SIZES = [16, 24, 32, 48, 64, 256]
SUPERSAMPLE = 4


def in_tile(x, y):
    lo, hi = MARGIN + RADIUS, 1 - MARGIN - RADIUS
    cx, cy = min(max(x, lo), hi), min(max(y, lo), hi)
    return (x - cx) ** 2 + (y - cy) ** 2 <= RADIUS**2 and MARGIN <= x <= 1 - MARGIN and MARGIN <= y <= 1 - MARGIN


def line_y(x):
    for (x0, y0), (x1, y1) in zip(POINTS, POINTS[1:]):
        if x0 <= x <= x1:
            return y0 + (y1 - y0) * (x - x0) / (x1 - x0)
    return POINTS[-1][1]


def dist_to_line(x, y):
    best = float("inf")
    for (x0, y0), (x1, y1) in zip(POINTS, POINTS[1:]):
        dx, dy = x1 - x0, y1 - y0
        t = max(0.0, min(1.0, ((x - x0) * dx + (y - y0) * dy) / (dx * dx + dy * dy)))
        best = min(best, ((x - x0 - t * dx) ** 2 + (y - y0 - t * dy) ** 2) ** 0.5)
    return best


def sample(x, y):
    """Premultiplied RGBA at one point."""
    if not in_tile(x, y):
        return (0.0, 0.0, 0.0, 0.0)
    if dist_to_line(x, y) <= LINE_WIDTH / 2:
        return (*ACCENT, 1.0)
    if y > line_y(x):
        return tuple(b + (a - b) * FILL_ALPHA for a, b in zip(ACCENT, BG)) + (1.0,)
    return (*BG, 1.0)


def render(size):
    """Rows of (r, g, b, a) bytes, top to bottom."""
    n = SUPERSAMPLE
    rows = []
    for py in range(size):
        row = []
        for px in range(size):
            acc = [0.0] * 4
            for sy in range(n):
                for sx in range(n):
                    s = sample((px + (sx + 0.5) / n) / size, (py + (sy + 0.5) / n) / size)
                    acc = [a + s[3] * c for a, c in zip(acc[:3], s[:3])] + [acc[3] + s[3]]
            a = acc[3] / (n * n)
            rgb = [round(c / acc[3]) if acc[3] else 0 for c in acc[:3]]
            row.append((*rgb, round(a * 255)))
        rows.append(row)
    return rows


def png(rows):
    size = len(rows)
    raw = b"".join(b"\0" + bytes(v for px in row for v in px) for row in rows)

    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data))

    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def bmp(rows):
    """Classic DIB icon entry: BGRA bottom-up plus an empty AND mask (alpha does the masking)."""
    size = len(rows)
    header = struct.pack("<IiiHHIIiiII", 40, size, size * 2, 1, 32, 0, 0, 0, 0, 0, 0)
    pixels = b"".join(bytes((b, g, r, a)) for row in reversed(rows) for r, g, b, a in row)
    mask = b"\0" * (((size + 31) // 32) * 4 * size)
    return header + pixels + mask


def ico(images):
    out = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    for size, data in images:
        out += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    return out + b"".join(data for _, data in images)


if __name__ == "__main__":
    # PNG only for 256 px, as Windows does; small sizes stay BMP for older readers.
    images = [(s, png(render(s)) if s >= 256 else bmp(render(s))) for s in SIZES]
    data = ico(images)
    assert struct.unpack("<HHH", data[:6]) == (0, 1, len(SIZES))
    out = Path(__file__).with_name("glanceriff.ico")
    out.write_bytes(data)
    print(f"wrote {out} ({len(data)} bytes)")
    # The Unraid template points at this PNG.
    png_out = Path(__file__).parent.parent / "unraid" / "glanceriff.png"
    png_out.write_bytes(images[-1][1])
    print(f"wrote {png_out}")
