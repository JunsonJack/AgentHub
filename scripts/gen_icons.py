# -*- coding: utf-8 -*-
"""生成 AgentHub 占位图标（纯 stdlib，无 PIL 依赖）。

产物：32x32.png / 128x128.png / 128x128@2x.png / icon.png(512) / icon.ico
设计：圆角方块底 + 同心圆环（"hub"意象），正式图标后续由设计替换。
"""
import os
import struct
import zlib

OUT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "src-tauri", "icons")
BG = (47, 111, 237, 255)        # #2F6FED
RING_LIGHT = (121, 192, 255, 255)  # #79C0FF
WHITE = (255, 255, 255, 255)


def rounded_inside(x, y, s, r):
    dx = min(x, s - 1 - x)
    dy = min(y, s - 1 - y)
    if dx >= r or dy >= r:
        return True
    return (r - dx) ** 2 + (r - dy) ** 2 <= r * r


def render(size):
    rows = []
    radius = max(2, int(size * 0.20))
    c = (size - 1) / 2.0
    r_outer = size * 0.34
    r_mid = size * 0.24
    r_core = size * 0.12
    for y in range(size):
        row = bytearray()
        for x in range(size):
            if not rounded_inside(x, y, size, radius):
                row += b"\x00\x00\x00\x00"
                continue
            d = ((x - c) ** 2 + (y - c) ** 2) ** 0.5
            if d <= r_core:
                px = WHITE
            elif d <= r_mid:
                px = RING_LIGHT
            elif d <= r_outer:
                px = WHITE
            else:
                px = BG
            row += bytes(px)
        rows.append(bytes(row))
    return rows


def png(size, rows):
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    raw = b"".join(b"\x00" + r for r in rows)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr)
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def ico(images):
    n = len(images)
    header = struct.pack("<HHH", 0, 1, n)
    entries = b""
    data = b""
    offset = 6 + 16 * n
    for size, blob in images:
        w = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", w, w, 0, 0, 1, 32, len(blob), offset)
        data += blob
        offset += len(blob)
    return header + entries + data


def main():
    os.makedirs(OUT, exist_ok=True)
    blobs = {}
    for size in (32, 128, 256, 512):
        blobs[size] = png(size, render(size))
    with open(os.path.join(OUT, "32x32.png"), "wb") as f:
        f.write(blobs[32])
    with open(os.path.join(OUT, "128x128.png"), "wb") as f:
        f.write(blobs[128])
    with open(os.path.join(OUT, "128x128@2x.png"), "wb") as f:
        f.write(blobs[256])
    with open(os.path.join(OUT, "icon.png"), "wb") as f:
        f.write(blobs[512])
    with open(os.path.join(OUT, "icon.ico"), "wb") as f:
        f.write(ico([(32, blobs[32]), (256, blobs[256])]))
    print("icons written to", OUT)


if __name__ == "__main__":
    main()
