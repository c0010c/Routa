"""Routa 横版 / 竖版组合标志：C7「钉钉蓝 · 轨道」图标 + 几何笔画字标「Routa」。

字标不依赖任何字体：全部由等粗圆头笔画构成（笔画 22，单位见下），任何环境渲染一致、无授权问题。
"""
from pathlib import Path

ROOT = Path(__file__).parent
SVG = ROOT / "svg"
MASTER = (ROOT.parent / "routa-icon-final" / "svg" / "master.svg").read_text(encoding="utf-8")

INK = "#0B1B33"        # 字标深色（浅底用）
BLUE_TOP, BLUE_BOT = "#2BA6FF", "#0071E6"

# ── 字标几何（单位坐标）：基线 y=200，x 高 90（y=110），大写高 130（y=70），笔画 22 ──
W = 22
WORD_PATHS = [
    "M11 200 V70 H52 A35 35 0 0 1 52 140 H11 M46 140 L86 200",      # R
    "M193 155 A34 34 0 1 1 125 155 A34 34 0 1 1 193 155",           # o
    "M229 110 V160 A34 34 0 0 0 297 160 V110 M297 110 V200",         # u
    "M346 80 V176 Q346 200 370 200 M324 110 H368",                   # t
    "M465 155 A34 34 0 1 1 397 155 A34 34 0 1 1 465 155 M465 110 V200",  # a
]
WORD_BOX = (0, 59, 476, 152)
WW = 476          # x, y, w, h（含笔画外扩）
CAP_CENTER = 135                       # 大写 R 的竖向中心（70~200）


def word(color, s=1.0, tx=0.0, ty=0.0):
    paths = "".join(f'<path d="{d}"/>' for d in WORD_PATHS)
    return (f'<g transform="translate({tx:.1f} {ty:.1f}) scale({s})" fill="none" stroke="{color}" '
            f'stroke-width="{W}" stroke-linecap="round" stroke-linejoin="round">{paths}</g>')


def icon(tx=0.0, ty=0.0, s=1.0, uid="a"):
    """嵌入 C7 主图标（避免多个渐变 id 冲突）。"""
    inner = MASTER.split(">", 1)[1].rsplit("</svg>", 1)[0].replace('id="bg"', f'id="bg{uid}"').replace("url(#bg)", f"url(#bg{uid})")
    return f'<g transform="translate({tx:.1f} {ty:.1f}) scale({s})">{inner}</g>'


def symbol(color_ring, nodes, tx=0.0, ty=0.0, s=1.0, defs=""):
    """去掉圆角底板的“裸”轨道符号（用于横版的另一种形式）。"""
    n = "".join(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{c}"/>' for (x, y, r), c in zip(
        [(37.4, 113.4, 24), (214.6, 110.0, 18), (136.7, 175.8, 32)], nodes))
    return (f'{defs}<g transform="translate({tx:.1f} {ty:.1f}) scale({s})"><g transform="rotate(-18 128 132)">'
            f'<ellipse cx="128" cy="132" rx="100" ry="44" fill="none" stroke="{color_ring}" stroke-width="14"/>{n}</g></g>')


def save(name, w, h, body, bg=None):
    rect = f'<rect width="{w}" height="{h}" fill="{bg}"/>' if bg else ""
    (SVG / f"{name}.svg").write_text(
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}">{rect}{body}</svg>', encoding="utf-8")


# 字标单独
save("wordmark", WW, 152, word(INK, ty=-59))
save("wordmark-white", WW, 152, word("#FFFFFF", ty=-59))

# 横版：图标 256 + 间距 56 + 字标（大写高 = 图标高的 0.42）
S = 0.83
TX = 256 + 56
TY = 128 - CAP_CENTER * S
HW = round(TX + WW * S + 24)
save("lockup-horizontal", HW, 256, icon() + word(INK, S, TX, TY))
save("lockup-horizontal-white", HW, 256, icon(uid="b") + word("#FFFFFF", S, TX, TY))

# 横版（裸符号）：蓝色轨道符号 + 字标，适合白底网页 / 文档
GDEF = f'<defs><linearGradient id="sg" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="{BLUE_TOP}"/><stop offset="1" stop-color="{BLUE_BOT}"/></linearGradient></defs>'
save("lockup-horizontal-symbol", HW, 256,
     symbol("url(#sg)", [BLUE_BOT, "#7CC4FF", BLUE_TOP], defs=GDEF) + word(INK, S, TX, TY))

# 竖版：图标在上，字标在下居中
SS = 0.7
VW = round(WW * SS + 40)
VH = round(256 + 44 + 152 * SS)
save("lockup-stacked", VW, VH, icon((VW - 256) / 2, 0, uid="c") + word(INK, SS, (VW - WW * SS) / 2, 256 + 44 - 59 * SS))

# README / 官网横幅 1280×320
BW, BH = 1280, 320
bs = 0.78
lw = HW * bs
banner = (f'<defs><linearGradient id="bb" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#F3F8FF"/><stop offset="1" stop-color="#DCEBFF"/></linearGradient></defs>'
          f'<rect width="{BW}" height="{BH}" fill="url(#bb)"/>'
          f'<g transform="rotate(-18 1080 200)"><ellipse cx="1080" cy="200" rx="300" ry="120" fill="none" stroke="#2BA6FF" stroke-opacity=".12" stroke-width="22"/></g>'
          f'<g transform="translate({(BW - lw) / 2:.1f} {(BH - 256 * bs) / 2:.1f}) scale({bs})">{icon(uid="d")}{word(INK, S, TX, TY)}</g>')
save("banner-1280x320", BW, BH, banner)
print("ok")

# ───────────── 横版裸符号（主用）：按符号实际外框排版 ─────────────
# 裸符号（旋转后的轨道 + 节点）在 256 网格里的实际外框约为 x 12~231、y 66~203。
SYM_BOX = (10, 56, 224, 158)
SYM_CY = SYM_BOX[1] + SYM_BOX[3] / 2


def bare_lockup(name, ring, nodes, wcolor, defs="", bg=None):
    s_w = 0.83                                  # 字标缩放：大写高 ≈ 108，与符号高 137 比例约 0.8
    gap = 46
    pad = 24
    sx = pad - SYM_BOX[0]
    wx = pad + SYM_BOX[2] + gap
    h = SYM_BOX[3] + pad * 2
    cy = h / 2
    sy = cy - SYM_CY
    wy = cy - CAP_CENTER * s_w
    w = round(wx + WW * s_w + pad)
    save(name, w, round(h), symbol(ring, nodes, sx, sy, defs=defs) + word(wcolor, s_w, wx, wy), bg=bg)


SG = f'<defs><linearGradient id="sg" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="{BLUE_TOP}"/><stop offset="1" stop-color="{BLUE_BOT}"/></linearGradient></defs>'
SGD = '<defs><linearGradient id="sgd" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#9FD4FF"/><stop offset="1" stop-color="#3DA2FF"/></linearGradient></defs>'
bare_lockup("bare-light", "url(#sg)", [BLUE_BOT, "#7CC4FF", BLUE_TOP], INK, defs=SG)
bare_lockup("bare-dark", "url(#sgd)", ["#FFFFFF", "#9FD4FF", "#5EB8FF"], "#FFFFFF", defs=SGD)
bare_lockup("bare-mono-black", INK, [INK] * 3, INK)
bare_lockup("bare-mono-white", "#FFFFFF", ["#FFFFFF"] * 3, "#FFFFFF")

# 裸符号单独（方形，便于头像 / 文档插图）
save("symbol-bare", 256, 256, symbol("url(#sg)", [BLUE_BOT, "#7CC4FF", BLUE_TOP], defs=SG))

# 横幅（裸符号版）
bl = (SVG / "bare-light.svg").read_text(encoding="utf-8")
bw = int(bl.split('viewBox="0 0 ')[1].split(" ")[0]); bh = int(bl.split('viewBox="0 0 ')[1].split(" ")[1].split('"')[0])
inner = bl.split(">", 1)[1].rsplit("</svg>", 1)[0]
k = 1.3
banner2 = (f'<defs><linearGradient id="bb2" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#F7FAFF"/><stop offset="1" stop-color="#E4EFFF"/></linearGradient></defs>'
           f'<rect width="{BW}" height="{BH}" fill="url(#bb2)"/>'
           f'<g transform="rotate(-18 1090 210)"><ellipse cx="1090" cy="210" rx="300" ry="120" fill="none" stroke="#2BA6FF" stroke-opacity=".10" stroke-width="22"/></g>'
           f'<g transform="translate({(BW - bw * k) / 2:.1f} {(BH - bh * k) / 2:.1f}) scale({k})">{inner}</g>')
save("banner-bare-1280x320", BW, BH, banner2)
print("bare ok")
