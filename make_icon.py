#!/usr/bin/env python3
"""生成拾遗 app 图标（各尺寸 + .icns / .ico）。

依赖: pip install Pillow
用法: python3 make_icon.py
"""
import os
import subprocess
import sys

from PIL import Image, ImageDraw, ImageFont

SIZE = 1024
HERE = os.path.dirname(os.path.abspath(__file__))
ICON_DIR = os.path.join(HERE, "src-tauri", "icons")

# 宣纸底色
PAPER = (245, 237, 214, 255)
# 边框 / 深褐
BORDER = (122, 77, 53, 255)
# 朱砂印泥
SEAL = (158, 41, 36, 255)
WHITE = (255, 255, 255, 255)

# 候选中文字体，按优先级尝试
FONT_CANDIDATES = [
    "/System/Library/Fonts/STSongti-SC-Black.otf",
    "/System/Library/Fonts/Supplemental/Songti.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/PingFang.ttc",
    "/Library/Fonts/Arial Unicode.ttf",
]


def load_font(size: int):
    for path in FONT_CANDIDATES:
        if not os.path.exists(path):
            continue
        try:
            if path.endswith(".ttc"):
                return ImageFont.truetype(path, size, index=0)
            return ImageFont.truetype(path, size)
        except Exception:
            continue
    print("!! 找不到可用的中文字体，回退到默认字体（字形可能不准）")
    return ImageFont.load_default()


def draw_icon() -> Image.Image:
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # 圆角宣纸底
    margin = 56
    draw.rounded_rectangle(
        [margin, margin, SIZE - margin, SIZE - margin],
        radius=190,
        fill=PAPER,
    )

    # 外边框
    draw.rounded_rectangle(
        [margin, margin, SIZE - margin, SIZE - margin],
        radius=190,
        outline=BORDER,
        width=8,
    )

    # 内细边（双线边框，古典感）
    inner = margin + 26
    draw.rounded_rectangle(
        [inner, inner, SIZE - inner, SIZE - inner],
        radius=165,
        outline=BORDER,
        width=3,
    )

    # 中央朱砂印章
    seal = 336
    cx, cy = SIZE // 2, SIZE // 2
    draw.rounded_rectangle(
        [cx - seal // 2, cy - seal // 2, cx + seal // 2, cy + seal // 2],
        radius=46,
        fill=SEAL,
    )

    # 印章内白字「遗」
    font = load_font(214)
    text = "遗"
    bbox = draw.textbbox((0, 0), text, font=font)
    tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
    draw.text(
        (cx - tw // 2 - bbox[0], cy - th // 2 - bbox[1] - 8),
        text,
        font=font,
        fill=WHITE,
    )

    return img


def main() -> int:
    os.makedirs(ICON_DIR, exist_ok=True)
    master = os.path.join(ICON_DIR, "icon-1024.png")
    img = draw_icon()
    img.save(master)
    print(f"✓ {master}")

    # Tauri 期望的图标命名
    targets = {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "icon.png": 512,
    }
    for name, size in targets.items():
        path = os.path.join(ICON_DIR, name)
        img.resize((size, size), Image.LANCZOS).save(path)
        print(f"✓ {path}")

    # macOS .icns
    iconset = "/tmp/shiyi.iconset"
    subprocess.run(["rm", "-rf", iconset], check=True)
    os.makedirs(iconset, exist_ok=True)
    for size in (16, 32, 128, 256, 512):
        img.resize((size, size), Image.LANCZOS).save(
            os.path.join(iconset, f"icon_{size}x{size}.png")
        )
        img.resize((size * 2, size * 2), Image.LANCZOS).save(
            os.path.join(iconset, f"icon_{size}x{size}@2x.png")
        )
    icns = os.path.join(ICON_DIR, "icon.icns")
    subprocess.run(["iconutil", "-c", "icns", iconset, "-o", icns], check=True)
    print(f"✓ {icns}")

    # Windows .ico
    ico = os.path.join(ICON_DIR, "icon.ico")
    img.save(ico, sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    print(f"✓ {ico}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
