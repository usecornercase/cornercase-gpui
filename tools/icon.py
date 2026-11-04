import struct
import sys
import zlib

SIZES = [16, 20, 24, 32, 40, 48, 64, 256]
GROUND = (0x0B, 0x0C, 0x0E)
BRAND = (0x00, 0x5F, 0xFF)
INK = (0xEC, 0xEE, 0xF1)
SAMPLES = 8
ICNS_ENTRIES = [
    (b"icp4", 16),
    (b"icp5", 32),
    (b"ic11", 32),
    (b"icp6", 64),
    (b"ic12", 64),
    (b"ic07", 128),
    (b"ic13", 256),
    (b"ic08", 256),
    (b"ic14", 512),
    (b"ic09", 512),
    (b"ic10", 1024),
]


def tile_coverage(size, x, y):
    radius = size * 28 / 128
    if radius <= x and x + 1 <= size - radius or radius <= y and y + 1 <= size - radius:
        return 1.0
    inside = 0
    for i in range(SAMPLES):
        for j in range(SAMPLES):
            px = x + (i + 0.5) / SAMPLES
            py = y + (j + 0.5) / SAMPLES
            cx = min(max(px, radius), size - radius)
            cy = min(max(py, radius), size - radius)
            if (px - cx) ** 2 + (py - cy) ** 2 <= radius * radius:
                inside += 1
    return inside / (SAMPLES * SAMPLES)


def mark_rects(size):
    unit = max(1, round(size * 6 / 128))
    at = (size - 12 * unit) // 2
    cell = lambda x, y, w, h: (at + x * unit, at + y * unit, w * unit, h * unit)
    return [
        (cell(3, 0, 9, 3), BRAND),
        (cell(0, 3, 3, 9), BRAND),
        (cell(6, 6, 2, 5), INK),
    ]


def pixels(canvas, tile):
    margin = (canvas - tile) // 2
    rects = mark_rects(tile)
    rows = []
    for y in range(canvas):
        row = []
        for x in range(canvas):
            tx, ty = x - margin, y - margin
            if not (0 <= tx < tile and 0 <= ty < tile):
                row.append((0, 0, 0, 0))
                continue
            colour = GROUND
            for (rx, ry, rw, rh), fill in rects:
                if rx <= tx < rx + rw and ry <= ty < ry + rh:
                    colour = fill
            row.append(colour + (round(255 * tile_coverage(tile, tx, ty)),))
        rows.append(row)
    return rows


def png(size, rows):
    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    raw = b"".join(b"\x00" + bytes(c for p in row for c in p) for row in rows)
    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def dib(size, rows):
    stride = ((size + 31) // 32) * 4
    colours = b"".join(bytes((b, g, r, a)) for row in reversed(rows) for (r, g, b, a) in row)
    mask = bytearray()
    for row in reversed(rows):
        bits = bytearray(stride)
        for x, (_, _, _, a) in enumerate(row):
            if a == 0:
                bits[x // 8] |= 0x80 >> (x % 8)
        mask += bits
    header = struct.pack("<IiiHHIIiiII", 40, size, size * 2, 1, 32, 0, len(colours) + len(mask), 0, 0, 0, 0)
    return header + colours + bytes(mask)


def ico():
    images = []
    for size in SIZES:
        rows = pixels(size, size)
        images.append((size, png(size, rows) if size >= 256 else dib(size, rows)))
    offset = 6 + 16 * len(images)
    entries = b""
    for size, data in images:
        side = size % 256
        entries += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    return struct.pack("<HHH", 0, 1, len(images)) + entries + b"".join(data for _, data in images)


def icns():
    entries = b""
    for kind, size in ICNS_ENTRIES:
        tile = size - 2 * round(size * 100 / 1024)
        data = png(size, pixels(size, tile))
        entries += kind + struct.pack(">I", 8 + len(data)) + data
    return b"icns" + struct.pack(">I", 8 + len(entries)) + entries


if __name__ == "__main__":
    out_path = sys.argv[1]
    with open(out_path, "wb") as out:
        out.write(icns() if out_path.endswith(".icns") else ico())
