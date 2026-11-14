#!/usr/bin/env python3
"""Compare two PNG screenshots for the Zig/Rust parity check (plan.md §6).

Pure standard library on purpose: decoding/encoding PNG only needs `zlib`, so
the parity scripts run on a machine without Pillow installed.

    python3 scripts/parity/imgdiff.py a.png b.png [--diff out.png]
        [--tolerance 2] [--max-ratio 0.001]

Exit status 0 when the images match within the tolerance, 1 otherwise.
A pixel "differs" when any of its RGBA channels differs by more than
`--tolerance`; the images match when at most `--max-ratio` of the pixels
differ AND both images have the same dimensions. Images with an alpha channel
are compared including alpha.

With `--diff`, writes an RGBA image where differing pixels are red on a faded
copy of the first image (handy to see what moved).
"""

import argparse
import struct
import sys
import zlib


class PngError(Exception):
    pass


def read_png(path):
    """Decodes an 8-bit, non-interlaced RGB/RGBA/gray PNG into (width, height, channels, pixels)."""
    with open(path, "rb") as file:
        data = file.read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise PngError(f"{path}: not a PNG file")
    pos = 8
    width = height = None
    bit_depth = color_type = interlace = None
    palette = None
    idat = bytearray()
    while pos + 8 <= len(data):
        length, kind = struct.unpack(">I4s", data[pos : pos + 8])
        body = data[pos + 8 : pos + 8 + length]
        pos += 12 + length  # length + kind + body + crc
        if kind == b"IHDR":
            width, height, bit_depth, color_type, _, _, interlace = struct.unpack(">IIBBBBB", body)
        elif kind == b"PLTE":
            palette = body
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break
    if width is None:
        raise PngError(f"{path}: missing IHDR")
    if bit_depth != 8:
        raise PngError(f"{path}: only 8-bit PNGs are supported (got {bit_depth})")
    if interlace:
        raise PngError(f"{path}: interlaced PNGs are not supported")
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}.get(color_type)
    if channels is None:
        raise PngError(f"{path}: unsupported color type {color_type}")

    raw = zlib.decompress(bytes(idat))
    stride = width * channels
    pixels = bytearray(stride * height)
    previous = bytearray(stride)
    offset = 0
    for row in range(height):
        filter_type = raw[offset]
        offset += 1
        line = bytearray(raw[offset : offset + stride])
        offset += stride
        if filter_type == 0:
            pass
        elif filter_type == 1:  # Sub
            for i in range(channels, stride):
                line[i] = (line[i] + line[i - channels]) & 0xFF
        elif filter_type == 2:  # Up
            for i in range(stride):
                line[i] = (line[i] + previous[i]) & 0xFF
        elif filter_type == 3:  # Average
            for i in range(stride):
                left = line[i - channels] if i >= channels else 0
                line[i] = (line[i] + ((left + previous[i]) >> 1)) & 0xFF
        elif filter_type == 4:  # Paeth
            for i in range(stride):
                left = line[i - channels] if i >= channels else 0
                up = previous[i]
                up_left = previous[i - channels] if i >= channels else 0
                p = left + up - up_left
                pa, pb, pc = abs(p - left), abs(p - up), abs(p - up_left)
                predictor = left if (pa <= pb and pa <= pc) else (up if pb <= pc else up_left)
                line[i] = (line[i] + predictor) & 0xFF
        else:
            raise PngError(f"{path}: bad filter type {filter_type} on row {row}")
        pixels[row * stride : (row + 1) * stride] = line
        previous = line

    # Normalise to RGBA so comparisons do not care about the source format.
    rgba = bytearray(width * height * 4)
    for index in range(width * height):
        source = index * channels
        if color_type == 6:
            rgba[index * 4 : index * 4 + 4] = pixels[source : source + 4]
        elif color_type == 2:
            rgba[index * 4 : index * 4 + 3] = pixels[source : source + 3]
            rgba[index * 4 + 3] = 255
        elif color_type == 0:
            value = pixels[source]
            rgba[index * 4 : index * 4 + 4] = bytes((value, value, value, 255))
        elif color_type == 4:
            value = pixels[source]
            rgba[index * 4 : index * 4 + 4] = bytes((value, value, value, pixels[source + 1]))
        else:  # palette
            entry = pixels[source] * 3
            rgba[index * 4 : index * 4 + 3] = palette[entry : entry + 3]
            rgba[index * 4 + 3] = 255
    return width, height, rgba


def write_png(path, width, height, rgba):
    """Writes an 8-bit RGBA PNG (filter 0 on every row)."""
    stride = width * 4
    raw = bytearray()
    for row in range(height):
        raw.append(0)
        raw += rgba[row * stride : (row + 1) * stride]

    def chunk(kind, body):
        return (
            struct.pack(">I", len(body))
            + kind
            + body
            + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)
        )

    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    with open(path, "wb") as file:
        file.write(b"\x89PNG\r\n\x1a\n")
        file.write(chunk(b"IHDR", header))
        file.write(chunk(b"IDAT", zlib.compress(bytes(raw), 9)))
        file.write(chunk(b"IEND", b""))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("first")
    parser.add_argument("second")
    parser.add_argument("--diff", help="write a diff image here")
    parser.add_argument("--tolerance", type=int, default=2, help="per-channel difference allowed (default 2)")
    parser.add_argument(
        "--max-ratio", type=float, default=0.001, help="fraction of differing pixels allowed (default 0.001)"
    )
    args = parser.parse_args()

    width_a, height_a, pixels_a = read_png(args.first)
    width_b, height_b, pixels_b = read_png(args.second)
    if (width_a, height_a) != (width_b, height_b):
        print(
            f"MISMATCH size: {args.first} is {width_a}x{height_a}, "
            f"{args.second} is {width_b}x{height_b}"
        )
        return 1

    total = width_a * height_a
    differing = 0
    diff_pixels = bytearray(pixels_a)
    for index in range(total):
        base = index * 4
        if max(
            abs(pixels_a[base] - pixels_b[base]),
            abs(pixels_a[base + 1] - pixels_b[base + 1]),
            abs(pixels_a[base + 2] - pixels_b[base + 2]),
            abs(pixels_a[base + 3] - pixels_b[base + 3]),
        ) > args.tolerance:
            differing += 1
            if args.diff:
                diff_pixels[base : base + 4] = bytes((255, 0, 0, 255))

    ratio = differing / total if total else 0.0
    if args.diff and differing:
        write_png(args.diff, width_a, height_a, diff_pixels)

    verdict = "ok" if ratio <= args.max_ratio else "MISMATCH"
    print(
        f"{verdict} {args.first} vs {args.second}: {differing}/{total} pixels differ "
        f"({ratio * 100:.4f}%, allowed {args.max_ratio * 100:.2f}%)"
        + (f"; diff: {args.diff}" if args.diff and differing else "")
    )
    return 0 if verdict == "ok" else 1


if __name__ == "__main__":
    sys.exit(main())
