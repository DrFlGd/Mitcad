#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""The HEIF test fixture of image references (mitcad#850).

Writes the HEVC data the tests wrap in HEIF files of their own: a 64 x 32
picture of four flat quarters (top left red, top right green, bottom left
blue, bottom right white), coded once with 8 and once with 10 bits per
channel, full range, BT.601. The pixels are made here; ffmpeg's libx265
codes them (one intra picture, quantiser 12, the Main and Main 10
profiles, no SEI with the encoder's version). x265 (GPL) is only the tool
that made the data: nothing of it is part of MitCad, its tests or its
packages, which decode with libde265. Regenerating needs an ffmpeg with
libx265; the tests keep the bytes this printed.

    tools/heif-test-fixture.py OUT

writes into OUT, for each bit depth, the raw stream (quadrants-8.hevc), the
HEIF files the tests make from it (quadrants-8-rot0.heic, -rot1.heic: the
image item with an irot property of 0 or 90 degrees anticlockwise, so that
ffmpeg or a viewer can check them) and prints the C++ arrays of
app/tests/heif_image.cpp (the hvcC property's content and the item's
data) and the printf lines of tools/ui-image-references-test.sh's file.
"""

import argparse
import pathlib
import struct
import subprocess
import sys

WIDTH, HEIGHT = 64, 32
QUARTERS = [[(255, 0, 0), (0, 255, 0)], [(0, 0, 255), (255, 255, 255)]]


def pixels():
    rows = bytearray()
    for y in range(HEIGHT):
        for x in range(WIDTH):
            rows += bytes(QUARTERS[int(y >= HEIGHT // 2)][int(x >= WIDTH // 2)])
    return bytes(rows)


def encode(bits, ffmpeg):
    pix = "yuv420p" if bits == 8 else "yuv420p10le"
    command = [
        ffmpeg, "-hide_banner", "-loglevel", "error",
        "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{WIDTH}x{HEIGHT}", "-i", "-",
        "-frames:v", "1",
        "-vf", f"scale=out_color_matrix=bt601:out_range=full,format={pix}",
        "-color_range", "pc", "-colorspace", "smpte170m",
        "-color_primaries", "bt709", "-color_trc", "iec61966-2-1",
        "-c:v", "libx265", "-profile:v", "main" if bits == 8 else "main10",
        "-x265-params", "info=0:qp=12:log-level=error",
        "-f", "hevc", "-",
    ]
    return subprocess.run(command, input=pixels(), capture_output=True, check=True).stdout


def nal_units(stream):
    starts = []
    at = 0
    while True:
        found = stream.find(b"\x00\x00\x01", at)
        if found < 0:
            break
        starts.append(found + 3)
        at = found + 3
    units = []
    for i, start in enumerate(starts):
        end = starts[i + 1] - 3 if i + 1 < len(starts) else len(stream)
        units.append(stream[start:end].rstrip(b"\x00"))
    return units


def unescape(data):
    out = bytearray()
    zeros = 0
    for byte in data:
        if zeros >= 2 and byte == 3:
            zeros = 0
            continue
        out.append(byte)
        zeros = zeros + 1 if byte == 0 else 0
    return bytes(out)


class Bits:
    def __init__(self, data, at):
        self.data, self.at = data, at

    def bit(self):
        value = (self.data[self.at // 8] >> (7 - self.at % 8)) & 1
        self.at += 1
        return value

    def ue(self):
        zeros = 0
        while self.bit() == 0:
            zeros += 1
        value = 1
        for _ in range(zeros):
            value = value * 2 + self.bit()
        return value - 1


def sps_info(sps):
    rbsp = unescape(sps[2:])
    if (rbsp[0] >> 1) & 7:
        sys.exit("the SPS has sub-layers")
    bits = Bits(rbsp, 13 * 8)
    bits.ue()  # sps_seq_parameter_set_id
    chroma = bits.ue()
    if chroma == 3:
        bits.bit()
    width, height = bits.ue(), bits.ue()
    if bits.bit():
        for _ in range(4):
            bits.ue()
    luma, chroma_bits = bits.ue() + 8, bits.ue() + 8
    return {"ptl": rbsp[1:13], "chroma": chroma, "size": (width, height), "luma": luma, "chroma_bits": chroma_bits}


def hvcc(units):
    kinds = {(unit[0] >> 1) & 0x3F: unit for unit in units}
    vps, sps, pps = kinds[32], kinds[33], kinds[34]
    info = sps_info(sps)
    ptl = info["ptl"]
    body = bytes([1]) + ptl
    body += struct.pack(">H", 0xF000)
    body += bytes([0xFC, 0xFC | info["chroma"], 0xF8 | (info["luma"] - 8), 0xF8 | (info["chroma_bits"] - 8)])
    body += struct.pack(">H", 0) + bytes([0x0F])
    body += bytes([3])
    for kind, unit in ((32, vps), (33, sps), (34, pps)):
        body += bytes([0x80 | kind]) + struct.pack(">HH", 1, len(unit)) + unit
    return body, info


def box(kind, body):
    return struct.pack(">I", 8 + len(body)) + kind + body


def full(kind, version, flags, body):
    return box(kind, bytes([version]) + flags.to_bytes(3, "big") + body)


def heif(hvcc_body, data, width, height, rotation):
    """The file app/tests/heif_image.cpp's heifFile makes."""
    ftyp = box(b"ftyp", b"heic" + struct.pack(">I", 0) + b"mif1heic")
    properties = [
        box(b"hvcC", hvcc_body),
        full(b"ispe", 0, 0, struct.pack(">II", width, height)),
        box(b"colr", b"nclx" + struct.pack(">HHHB", 1, 13, 6, 0x80)),
    ]
    associations = [0x81, 0x02, 0x03]
    if rotation:
        properties.append(box(b"irot", bytes([rotation])))
        associations.append(0x84)

    def meta(offset):
        hdlr = full(b"hdlr", 0, 0, bytes(4) + b"pict" + bytes(12) + b"\x00")
        pitm = full(b"pitm", 0, 0, struct.pack(">H", 1))
        iloc = full(b"iloc", 0, 0, bytes([0x44, 0x00]) + struct.pack(">HHHHII", 1, 1, 0, 1, offset, len(data)))
        infe = full(b"infe", 2, 0, struct.pack(">HH", 1, 0) + b"hvc1" + b"\x00")
        iinf = full(b"iinf", 0, 0, struct.pack(">H", 1) + infe)
        ipma = full(b"ipma", 0, 0, struct.pack(">IHB", 1, 1, len(associations)) + bytes(associations))
        iprp = box(b"iprp", box(b"ipco", b"".join(properties)) + ipma)
        return full(b"meta", 0, 0, hdlr + pitm + iloc + iinf + iprp)

    offset = len(ftyp) + len(meta(0)) + 8
    return ftyp + meta(offset) + box(b"mdat", data)


def cpp_array(name, data):
    lines = [f"const unsigned char {name}[] = {{"]
    for at in range(0, len(data), 16):
        lines.append("    " + ", ".join(f"0x{b:02x}" for b in data[at:at + 16]) + ",")
    lines.append("};")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("out", type=pathlib.Path)
    parser.add_argument("--ffmpeg", default="ffmpeg")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    for bits in (8, 10):
        stream = encode(bits, args.ffmpeg)
        (args.out / f"quadrants-{bits}.hevc").write_bytes(stream)
        units = nal_units(stream)
        body, info = hvcc(units)
        if info["size"] != (WIDTH, HEIGHT) or info["luma"] != bits:
            sys.exit(f"unexpected SPS: {info}")
        slices = [unit for unit in units if (unit[0] >> 1) & 0x3F < 32]
        data = b"".join(struct.pack(">I", len(unit)) + unit for unit in slices)
        for rotation in (0, 1):
            (args.out / f"quadrants-{bits}-rot{rotation}.heic").write_bytes(heif(body, data, WIDTH, HEIGHT, rotation))
        print(cpp_array(f"kHvcc{bits}", body))
        print(cpp_array(f"kData{bits}", data))
    file = (args.out / "quadrants-8-rot1.heic").read_bytes()
    print(f"# {len(file)} bytes")
    for at in range(0, len(file), 24):
        print("  printf '" + "".join(f"\\x{b:02x}" for b in file[at:at + 24]) + "'")


if __name__ == "__main__":
    main()
