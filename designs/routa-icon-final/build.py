"""Routa 最终图标（C7「钉钉蓝 · 轨道」精修版）：主图标、小尺寸版、托盘状态图标，以及多尺寸 ICO。"""
import math
import struct
from pathlib import Path

ROOT = Path(__file__).parent
SVG = ROOT / "svg"
SVG.mkdir(exist_ok=True)

BLUE_TOP, BLUE_BOTTOM = "#2BA6FF", "#0071E6"
LIGHT = "#BFE3FF"          # 远处小节点：比原稿 #9FD4FF 更亮，避免小尺寸时和背景糊在一起


def pt(a):
    return math.cos(math.radians(a)), math.sin(math.radians(a))


def orbit(rx, ry, ring_w, radii, colors, ring_color="#fff", cy=132, ring_extra=""):
    """倾斜 -18° 的椭圆轨道 + 三个节点（左后、右上远、正前）。"""
    nodes = "".join(
        f'<circle cx="{128 + rx * pt(a)[0]:.1f}" cy="{cy + ry * pt(a)[1]:.1f}" r="{r}" fill="{c}"/>'
        for a, r, c in zip((205, 330, 85), radii, colors) if c)
    return (f'<g transform="rotate(-18 128 {cy})"><ellipse cx="128" cy="{cy}" rx="{rx}" ry="{ry}" fill="none" '
            f'stroke="{ring_color}" stroke-width="{ring_w}" {ring_extra}/>{nodes}</g>')


def doc(body, defs=""):
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256"><defs>{defs}</defs>{body}</svg>'


GRAD = (f'<linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{BLUE_TOP}"/>'
        f'<stop offset="1" stop-color="{BLUE_BOTTOM}"/></linearGradient>')

# 1) 主图标（48 px 及以上）
# 图形在底板中的占比：参考钉钉，主图标图形宽约占底板 63%，小尺寸版留得稍满（约 72%）保证可读。
# 轨道图形的视觉外框中心约在 (122, 135)，以它为基准缩放后放到画布中心。
MASTER_SCALE, SMALL_SCALE = 0.72, 0.74


def fit(glyph, s):
    return f'<g transform="translate(128 128) scale({s}) translate(-122 -135)">{glyph}</g>'


master = doc('<rect width="256" height="256" rx="58" fill="url(#bg)"/>'
             + fit(orbit(100, 44, 14, (25, 19, 33), ("#fff", LIGHT, "#fff")), MASTER_SCALE), GRAD)
# 2) 小尺寸版（16 / 24 / 32 px）：圆角更小、轨道加粗、节点放大、远处节点也用纯白
small = doc('<rect width="256" height="256" rx="44" fill="url(#bg)"/>'
            + fit(orbit(98, 50, 24, (33, 27, 42), ("#fff", "#fff", "#fff")), SMALL_SCALE), GRAD)
# 3) 托盘状态：实心底板 + 加粗轨道，底色表示状态，不依赖任务栏深浅。
# 16 px 下轨道也有约 1.5 px 粗；未连接额外加一道斜杠，只靠形状也能区分。
TRAY_SCALE = 0.80
TRAY_BLUE, TRAY_AMBER, TRAY_GREY = "#0A84FF", "#FBBF24", "#64748B"


def tray_tile(bg, ink="#fff", slash=False):
    glyph = fit(orbit(100, 62, 28, (28, 22, 34), (ink, ink, ink), ring_color=ink), TRAY_SCALE)
    cut = (f'<path d="M48 48 208 208" stroke="{bg}" stroke-width="58" stroke-linecap="round"/>'
           f'<path d="M48 48 208 208" stroke="{ink}" stroke-width="26" stroke-linecap="round"/>') if slash else ""
    return doc(f'<rect width="256" height="256" rx="56" fill="{bg}"/>{glyph}{cut}')


tray = {"tray-idle": tray_tile(TRAY_GREY), "tray-connected": tray_tile(TRAY_BLUE),
        "tray-warning": tray_tile(TRAY_AMBER, ink="#1A1A1A"), "tray-disconnected": tray_tile(TRAY_GREY, slash=True)}

files = {"master": master, "small": small, **tray}
for name, content in files.items():
    (SVG / f"{name}.svg").write_text(content, encoding="utf-8")
print("svg:", ", ".join(files))


def build_ico(out, entries):
    """entries: [(size, png_path)]，把 PNG 直接打包进 ICO（Vista+ 支持 PNG 压缩条目）。"""
    blobs = [(s, Path(p).read_bytes()) for s, p in entries]
    header = struct.pack("<HHH", 0, 1, len(blobs))
    offset = 6 + 16 * len(blobs)
    dirs, data = b"", b""
    for s, b in blobs:
        dirs += struct.pack("<BBBBHHII", 0 if s >= 256 else s, 0 if s >= 256 else s, 0, 0, 1, 32, len(b), offset + len(data))
        data += b
    Path(out).write_bytes(header + dirs + data)


if __name__ == "__main__":
    import sys
    if len(sys.argv) > 1 and sys.argv[1] == "ico":
        png = ROOT / "png"
        build_ico(ROOT / "icon.ico",
                  [(s, png / f"small-{s}.png") for s in (16, 24, 32)] + [(s, png / f"master-{s}.png") for s in (48, 64, 128, 256)])
        print("wrote icon.ico")
