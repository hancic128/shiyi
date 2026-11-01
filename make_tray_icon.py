#!/usr/bin/env python3
"""生成拾遗的 macOS 菜单栏图标（template image）。

菜单栏图标必须是单色 template image：只有 alpha 通道有意义，颜色由系统按
浅色/深色菜单栏自动反色。这里画一枚朱砂方印「遗」：实心印面 + 镂空字形。

用法: python3 make_tray_icon.py
"""
import os
import sys

from PIL import Image, ImageDraw, ImageFont

SIZE = 44  # 菜单栏槽位 22pt @2x
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "src-tauri", "icons", "tray-icon.png")

BLACK = (0, 0, 0, 255)
CLEAR = (0, 0, 0, 0)

# 中文字体候选，按优先级尝试
FONT_CANDIDATES = [
    "/System/Library/Fonts/STSongti-SC-Black.otf",
    "/System/Library/Fonts/Supplemental/Songti.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/PingFang.ttc",
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


def draw_tray_icon() -> Image.Image:
    img = Image.new("RGBA", (SIZE, SIZE), CLEAR)
    draw = ImageDraw.Draw(img)

    # 实心印面
    margin = 3
    draw.rounded_rectangle(
        [margin, margin, SIZE - margin, SIZE - margin],
        radius=4,
        fill=BLACK,
    )

    # 「遗」字镂空：先画在单独的图层上，再把该区域从印面里抠掉
    font = load_font(25)
    text = "遗"
    bbox = draw.textbbox((0, 0), text, font=font)
    tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
    cx, cy = SIZE // 2, SIZE // 2

    mask = Image.new("L", (SIZE, SIZE), 0)
    mdraw = ImageDraw.Draw(mask)
    mdraw.text(
        (cx - tw // 2 - bbox[0], cy - th // 2 - bbox[1] - 1),
        text,
        font=font,
        fill=255,
    )

    alpha = img.getchannel("A")
    alpha = Image.composite(Image.new("L", (SIZE, SIZE), 0), alpha, mask)
    img.putalpha(alpha)

    return img


def main() -> int:
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    img = draw_tray_icon()
    img.save(OUT)
    print(f"✓ {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
