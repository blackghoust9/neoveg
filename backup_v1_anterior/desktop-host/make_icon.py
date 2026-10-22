"""Generates a crisp native Windows .ico icon for OpenVegasPro21.exe"""

import struct
import zlib
from pathlib import Path


def make_png_rgba(width: int, height: int) -> bytes:
    raw_rows = bytearray()
    for y in range(height):
        raw_rows.append(0)  # Filter type 0
        for x in range(width):
            # Rounded square check
            margin = 4
            if x < margin or x >= width - margin or y < margin or y >= height - margin:
                raw_rows.extend((0, 0, 0, 0))
                continue
            # Gradient background (#1e3a8a to #0284c7)
            t = (x + y) / float(width + height)
            r = int(30 * (1 - t) + 6 * t)
            g = int(64 * (1 - t) + 182 * t)
            b = int(175 * (1 - t) + 212 * t)

            # Draw iconic "V" shape in white/cyan
            nx = (x - width / 2.0) / (width / 2.0)
            ny = (y - height / 2.0) / (height / 2.0)
            in_v = False
            if -0.55 <= ny <= 0.55:
                target_x = (0.55 - ny) * 0.42
                if abs(abs(nx) - target_x) < 0.13:
                    in_v = True

            if in_v:
                raw_rows.extend((248, 250, 252, 255))
            else:
                raw_rows.extend((r, g, b, 255))

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    idat = zlib.compress(bytes(raw_rows), 9)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", idat) + chunk(b"IEND", b"")


def create_ico(path: Path) -> None:
    png_data = make_png_rgba(64, 64)
    # ICO header: reserved=0, type=1 (ICO), count=1
    header = struct.pack("<HHH", 0, 1, 1)
    # Directory entry: width=64, height=64, colors=0, reserved=0, planes=1, bpp=32, size, offset=22
    entry = struct.pack("<BBBBHHII", 64, 64, 0, 0, 1, 32, len(png_data), 22)
    path.write_bytes(header + entry + png_data)


if __name__ == "__main__":
    out = Path(__file__).resolve().parent / "OpenVegasPro21.ico"
    create_ico(out)
    print(f"Created icon: {out} ({out.stat().st_size} bytes)")
