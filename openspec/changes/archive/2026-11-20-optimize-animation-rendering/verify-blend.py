#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 The pixel-perfect Authors
"""Checks whether real decoders composite "masked" animation frames exactly.

A masked frame is blended OVER the previous canvas; its pixels are either
fully opaque (must replace the canvas) or (0, 0, 0, 0) (must keep it).
This script builds 2-frame APNG and animated WebP files by hand:

- frame 0: a full canvas of destination pixels;
- frame 1: rows 0-4 opaque source pixels, rows 5-9 transparent ones, with
  blend OVER (APNG) / alpha blending (WebP), dispose none.

Columns cover every channel value 0-255; rows cover destination alpha
0, 1, 128, 254 and 255. It then decodes frame 1 with every available decoder
and compares it with the expected canvas.

Usage: python3 -I verify-blend.py <scratch-dir>
"""

import io
import os
import struct
import subprocess
import sys
import zlib

from PIL import Image

W, H = 256, 10
DST_ALPHAS = [0, 1, 128, 254, 255]


def dst_pixel(x, y):
    a = DST_ALPHAS[y % 5]
    if a == 0:
        return (0, 0, 0, 0)  # canonical transparency, as the engine renders
    return (255 - x, x, (x * 7) % 256, a)


def src_pixel(x, y):
    if y < 5:
        return (x, 255 - x, (x * 13) % 256, 255)
    return (0, 0, 0, 0)


def expected_pixel(x, y):
    return src_pixel(x, y) if y < 5 else dst_pixel(x, y)


def raster(fn):
    return [fn(x, y) for y in range(H) for x in range(W)]


# --- APNG -------------------------------------------------------------------

def png_chunk(kind, data):
    body = kind + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))


def rgba_rows(pixels):
    out = bytearray()
    for y in range(H):
        out.append(0)  # filter none
        for p in pixels[y * W:(y + 1) * W]:
            out.extend(p)
    return zlib.compress(bytes(out), 9)


def fctl(seq, blend):
    # sequence, width, height, x, y, delay num, delay den, dispose, blend
    return png_chunk(b"fcTL", struct.pack(">IIIIIHHBB", seq, W, H, 0, 0, 1, 10, 0, blend))


def build_apng():
    out = b"\x89PNG\r\n\x1a\n"
    out += png_chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
    out += png_chunk(b"acTL", struct.pack(">II", 2, 0))
    out += fctl(0, 0)  # SOURCE
    out += png_chunk(b"IDAT", rgba_rows(raster(dst_pixel)))
    out += fctl(1, 1)  # OVER
    out += png_chunk(b"fdAT", struct.pack(">I", 2) + rgba_rows(raster(src_pixel)))
    out += png_chunk(b"IEND", b"")
    return out


# --- WebP -------------------------------------------------------------------

def riff_chunk(kind, data):
    pad = b"\x00" if len(data) % 2 else b""
    return kind + struct.pack("<I", len(data)) + data + pad


def vp8l(pixels):
    image = Image.new("RGBA", (W, H))
    image.putdata(pixels)
    buf = io.BytesIO()
    image.save(buf, "WEBP", lossless=True, exact=True, quality=100, method=6)
    data = buf.getvalue()
    i = 12
    while i < len(data):
        kind = data[i:i + 4]
        size = struct.unpack("<I", data[i + 4:i + 8])[0]
        if kind == b"VP8L":
            return data[i + 8:i + 8 + size]
        i += 8 + size + size % 2
    raise RuntimeError("no VP8L chunk in Pillow output")


def u24(v):
    return struct.pack("<I", v)[:3]


def anmf(bitstream, blend):
    flags = 0 if blend else 0b10  # bit 1 set: do not blend; bit 0: dispose none
    header = u24(0) + u24(0) + u24(W - 1) + u24(H - 1) + u24(100) + bytes([flags])
    return riff_chunk(b"ANMF", header + riff_chunk(b"VP8L", bitstream))


def build_webp():
    body = riff_chunk(b"VP8X", bytes([0b0001_0010, 0, 0, 0]) + u24(W - 1) + u24(H - 1))
    body += riff_chunk(b"ANIM", bytes(6))
    body += anmf(vp8l(raster(dst_pixel)), blend=False)
    body += anmf(vp8l(raster(src_pixel)), blend=True)
    return b"RIFF" + struct.pack("<I", len(body) + 4) + b"WEBP" + body


# --- Decoders ----------------------------------------------------------------

def pillow_frame1(path):
    image = Image.open(path)
    image.seek(1)
    rgba = image.convert("RGBA")
    flat = getattr(rgba, "get_flattened_data", None)
    return list(flat() if flat else rgba.getdata())


def ffmpeg_frame1(path):
    result = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", path, "-fps_mode", "passthrough",
         "-f", "rawvideo", "-pix_fmt", "rgba", "-"],
        capture_output=True,
    )
    raw = result.stdout
    frame = W * H * 4
    if result.returncode != 0 or len(raw) < 2 * frame:
        return None, (result.stderr.decode(errors="replace").strip().splitlines() or ["no output"])[-1]
    data = raw[frame:2 * frame]
    return [tuple(data[i:i + 4]) for i in range(0, frame, 4)], None


def compare(name, got, expected):
    if got is None:
        return
    bad = [(i % W, i // W, g, e) for i, (g, e) in enumerate(zip(got, expected)) if tuple(g) != e]
    opaque = [b for b in bad if b[1] < 5]
    clear = [b for b in bad if b[1] >= 5]
    status = "EXACT" if not bad else f"{len(bad)} wrong pixels ({len(opaque)} opaque-over, {len(clear)} transparent-over)"
    print(f"{name:28} {status}")
    for x, y, g, e in bad[:3]:
        print(f"    ({x},{y}) dst alpha {DST_ALPHAS[y % 5]}: got {tuple(g)}, expected {e}")


def main():
    scratch = sys.argv[1]
    os.makedirs(scratch, exist_ok=True)
    expected = raster(expected_pixel)
    files = {"apng": build_apng(), "webp": build_webp()}
    paths = {}
    for ext, data in files.items():
        paths[ext] = os.path.join(scratch, f"masked.{ext if ext == 'webp' else 'png'}")
        with open(paths[ext], "wb") as f:
            f.write(data)
    compare("APNG / Pillow", pillow_frame1(paths["apng"]), expected)
    got, err = ffmpeg_frame1(paths["apng"])
    if err:
        print(f"{'APNG / FFmpeg':28} not decoded: {err}")
    compare("APNG / FFmpeg", got, expected)
    compare("WebP / Pillow (libwebp)", pillow_frame1(paths["webp"]), expected)
    got, err = ffmpeg_frame1(paths["webp"])
    if err:
        print(f"{'WebP / FFmpeg':28} not decoded: {err}")
    compare("WebP / FFmpeg", got, expected)
    print(f"files: {paths['apng']} {paths['webp']}")


if __name__ == "__main__":
    main()
