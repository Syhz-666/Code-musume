"""生成占位应用图标（1024×1024 PNG）——纯标准库，无第三方依赖。

用法：python tools/gen-placeholder-icon.py <输出路径>
产出由 `pnpm --filter @code-musume/pet tauri icon <png>` 转换为全套图标（M4 替换为正式美术）。
"""

import math
import struct
import sys
import zlib

SIZE = 1024
RADIUS = SIZE * 0.42
CX = CY = SIZE / 2.0
RING_W = 30
# 占位配色（与 apps/pet/src/pet.ts 的占位圆一致）
FILL = (0x8E, 0xCA, 0xE6)
RING = (0x21, 0x9E, 0xBC)


def pixel(x: int, y: int) -> tuple[int, int, int, int]:
    d = math.hypot(x - CX + 0.5, y - CY + 0.5)
    if d > RADIUS:
        return (0, 0, 0, 0)
    alpha = 255 if RADIUS - d >= 1 else int(255 * (RADIUS - d))
    r, g, b = RING if RADIUS - d <= RING_W else FILL
    return (r, g, b, alpha)


def main() -> None:
    out = sys.argv[1]
    rows = []
    for y in range(SIZE):
        row = bytearray([0])  # PNG filter: none
        for x in range(SIZE):
            row += bytes(pixel(x, y))
        rows.append(bytes(row))
    raw = b"".join(rows)

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    with open(out, "wb") as f:
        f.write(png)
    print(f"written {out}")


if __name__ == "__main__":
    main()
