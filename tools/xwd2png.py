#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Converts a 24/32-bit TrueColor XWD screen dump (xwd -root) to PNG.

Only the standard library is used, so the UI tests need no image tools.
Usage: xwd2png.py input.xwd output.png [x y width height]
With a rectangle, only that part of the screen is written (clipped to it).
"""

import struct
import sys
import zlib


def read_xwd(path):
    data = open(path, "rb").read()
    fields = struct.unpack(">25I", data[:100])
    header_size, version, pixmap_format = fields[0], fields[1], fields[2]
    width, height = fields[4], fields[5]
    byte_order, bits_per_pixel, bytes_per_line = fields[7], fields[11], fields[12]
    red_mask, green_mask, blue_mask = fields[14], fields[15], fields[16]
    ncolors = fields[19]
    if version != 7 or pixmap_format != 2 or bits_per_pixel not in (24, 32):
        raise SystemExit("unsupported XWD: need a 24/32-bit ZPixmap dump")

    pixels = data[header_size + ncolors * 12:]
    bytes_per_pixel = bits_per_pixel // 8
    order = "little" if byte_order == 0 else "big"

    def channel(value, mask):
        shift = (mask & -mask).bit_length() - 1
        return (value & mask) >> shift

    rows = []
    for y in range(height):
        line = pixels[y * bytes_per_line:y * bytes_per_line + width * bytes_per_pixel]
        row = bytearray()
        for x in range(0, len(line), bytes_per_pixel):
            value = int.from_bytes(line[x:x + bytes_per_pixel], order)
            row += bytes((channel(value, red_mask), channel(value, green_mask),
                          channel(value, blue_mask)))
        rows.append(bytes(row))
    return width, height, rows


def write_png(path, width, height, rows):
    def chunk(kind, payload):
        body = kind + payload
        return struct.pack(">I", len(payload)) + body + struct.pack(">I", zlib.crc32(body))

    raw = b"".join(b"\x00" + row for row in rows)
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 6))
    png += chunk(b"IEND", b"")
    open(path, "wb").write(png)


def crop(width, height, rows, x, y, w, h):
    x0, y0 = max(0, x), max(0, y)
    x1, y1 = min(width, x + w), min(height, y + h)
    if x1 <= x0 or y1 <= y0:
        raise SystemExit("the rectangle is off the screen")
    return x1 - x0, y1 - y0, [row[x0 * 3:x1 * 3] for row in rows[y0:y1]]


if __name__ == "__main__":
    if len(sys.argv) not in (3, 7):
        raise SystemExit(__doc__)
    image = read_xwd(sys.argv[1])
    if len(sys.argv) == 7:
        image = crop(*image, *(int(v) for v in sys.argv[3:7]))
    write_png(sys.argv[2], *image)
