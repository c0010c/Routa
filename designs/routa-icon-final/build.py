"""Routa 最终图标（C7「钉钉蓝 · 轨道」精修版）：主图标、小尺寸版、托盘单色与状态图标，以及多尺寸 ICO。"""
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
master = doc('<rect width="256" height="256" rx="58" fill="url(#bg)"/>'
             + orbit(100, 44, 12, (24, 18, 32), ("#fff", LIGHT, "#fff")), GRAD)
# 2) 小尺寸版（16 / 24 / 32 px）：圆角更小、轨道加粗、节点放大、远处节点也用纯白
small = doc('<rect width="256" height="256" rx="44" fill="url(#bg)"/>'
            + orbit(98, 50, 22, (34, 28, 44), ("#fff", "#fff", "#fff")), GRAD)
# 3) 托盘单色（透明底）
tray_glyph = lambda c: orbit(104, 52, 24, (36, 30, 46), (c, c, c), ring_color=c)
tray_dark = doc(tray_glyph("#FFFFFF"))      # 深色任务栏
tray_light = doc(tray_glyph("#1A1A1A"))     # 浅色任务栏
# 4) 托盘状态（深色任务栏）
st_on = doc(orbit(104, 52, 24, (36, 30, 46), ("#fff", "#fff", "#4ADE80")))
st_warn = doc(orbit(104, 52, 24, (36, 30, 46), ("#fff", None, "#FBBF24"))
              + '<g transform="rotate(-18 128 132)"><circle cx="{:.1f}" cy="{:.1f}" r="24" fill="none" stroke="#fff" stroke-width="10"/></g>'.format(
                  128 + 104 * pt(330)[0], 132 + 52 * pt(330)[1]))
st_off = doc(orbit(104, 52, 18, (32, 26, 40), ("#8B95A5", "#8B95A5", "#8B95A5"), ring_color="#8B95A5",
                   ring_extra='stroke-dasharray="20 16"'))

files = {"master": master, "small": small, "tray-dark": tray_dark, "tray-light": tray_light,
         "tray-connected": st_on, "tray-warning": st_warn, "tray-disconnected": st_off}
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
