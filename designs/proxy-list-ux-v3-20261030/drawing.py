"""Standalone Routa design review; only writes files beside this generator."""
from pathlib import Path
from html import escape
import json
import re

ROOT = Path(__file__).resolve().parent
PROJECT = ROOT.parent.parent
INK = '#1b1b1b'
MUTED = '#616161'
DIM = '#858585'
BLUE = '#0067c0'
LINE = '#e5e5e5'
BG = '#f3f3f3'
GREEN = '#0f7b0f'
AMBER = '#9d5d00'
RED = '#c42b1c'
FONT = 'Segoe UI,Microsoft YaHei UI,Microsoft YaHei,sans-serif'


class SVG:
    def __init__(self, w, h, title):
        self.w, self.h = w, h
        self.p = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" role="img" aria-label="{escape(title)}">',
                  f'<title>{escape(title)}</title>',
                  '<defs><filter id="shadow" x="-20%" y="-20%" width="140%" height="160%"><feDropShadow dx="0" dy="5" stdDeviation="10" flood-color="#243244" flood-opacity=".09"/></filter></defs>',
                  f'<g font-family="{FONT}" font-variant-numeric="tabular-nums">']

    def rect(self, x, y, w, h, fill='white', stroke=None, r=6, extra=''):
        self.p.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill}"' + (f' stroke="{stroke}"' if stroke else '') + f' {extra}/>')

    def txt(self, x, y, value, size=13, color=INK, weight=400, anchor='start', extra=''):
        self.p.append(f'<text x="{x}" y="{y}" font-size="{size}" fill="{color}" font-weight="{weight}" text-anchor="{anchor}" {extra}>{escape(str(value))}</text>')

    def lines(self, x, y, values, size=12, color=MUTED, step=23):
        for i, value in enumerate(values):
            self.txt(x, y+i*step, value, size, color)

    def line(self, x1, y1, x2, y2, color=LINE, width=1, extra=''):
        self.p.append(f'<path d="M{x1} {y1}H{x2}" stroke="{color}" stroke-width="{width}" {extra}/>' if y1 == y2 else f'<path d="M{x1} {y1}L{x2} {y2}" stroke="{color}" stroke-width="{width}" {extra}/>')

    def path(self, d, color=MUTED, width=1.5, fill='none', extra=''):
        self.p.append(f'<path d="{d}" fill="{fill}" stroke="{color}" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round" {extra}/>')

    def dot(self, x, y, r=3, color=GREEN):
        self.p.append(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{color}"/>')

    def icon(self, name, x, y, color=MUTED, size=20):
        source = (PROJECT/'node_modules'/'@mui'/'icons-material'/(name+'.mjs')).read_text(encoding='utf-8')
        paths = re.findall(r'd:\s*"([^"]+)"', source)
        if not paths:
            raise ValueError(name)
        self.p.append(f'<g transform="translate({x} {y}) scale({size/24})" fill="{color}">')
        self.p.extend(f'<path d="{d}"/>' for d in paths)
        self.p.append('</g>')

    def button(self, x, y, w, label, primary=False, icon=None, h=34, action=None):
        if action:
            self.p.append(f'<g data-action="{action}" tabindex="0" role="button" aria-label="{label}" style="cursor:pointer">')
        self.rect(x,y,w,h,BLUE if primary else 'white',None if primary else '#d2d2d2',4)
        if icon:
            self.icon(icon,x+11,y+(h-16)/2,'white' if primary else MUTED,16)
            self.txt(x+34,y+h/2+4.5,label,13,'white' if primary else INK,500)
        else:
            self.txt(x+w/2,y+h/2+4.5,label,13,'white' if primary else INK,500,'middle')
        if action:
            self.p.append('</g>')

    def link(self, x, y, label, action=None, color=BLUE, size=13):
        if action:
            self.p.append(f'<g data-action="{action}" tabindex="0" role="button" aria-label="{label}" style="cursor:pointer">')
            self.rect(x-6,y-21,max(42,len(label)*size+12),30,'transparent',r=4)
        self.txt(x,y,label,size,color,500)
        if action:
            self.p.append('</g>')

    def badge(self, x, y, label, kind='green'):
        c, bg = {'green':(GREEN,'#edf7ed'),'gray':(MUTED,'#f0f0f0'),'red':(RED,'#fdf0ee'),'amber':(AMBER,'#fff5e5'),'blue':(BLUE,'#eaf3fc')}[kind]
        w = len(label)*11+24
        self.rect(x,y,w,24,bg,r=12)
        self.dot(x+10,y+12,2.5,c)
        self.txt(x+18,y+16,label,11,c,500)

    def toggle(self, x, y, on=True, action=None):
        if action:
            self.p.append(f'<g data-action="{action}" tabindex="0" role="button" aria-label="切换代理启用状态" style="cursor:pointer">')
        self.rect(x,y,34,18,BLUE if on else 'white',None if on else '#8a8a8a',9)
        self.dot(x+(25 if on else 9),y+9,6,'white' if on else '#8a8a8a')
        if action:
            self.p.append('</g>')

    def more(self, x, y, action='more'):
        self.p.append(f'<g data-action="{action}" tabindex="0" role="button" aria-label="更多操作" style="cursor:pointer">')
        self.rect(x-15,y-15,30,30,'transparent',r=4)
        for xx in [x-5,x,x+5]: self.dot(xx,y,1.35,MUTED)
        self.p.append('</g>')

    def arrow(self, x, y, w=36, color=BLUE):
        self.path(f'M{x} {y}h{w}m-6-5 6 5-6 5',color,1.6)

    def num(self, x, y, value):
        self.dot(x,y,12,BLUE)
        self.txt(x,y+4,str(value),11,'white',600,'middle')

    def group(self, x=0, y=0, scale=1):
        self.p.append(f'<g transform="translate({x} {y}) scale({scale})">')

    def end(self): self.p.append('</g>')
    def save(self, name):
        text = '\n'.join(self.p+['</g></svg>'])
        (ROOT/name).write_text(text,encoding='utf-8')
        return text

