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


def shell(s):
    s.rect(0,0,1280,768,BG,'#d8d8d8',8)
    s.rect(1,1,1278,31,'#fafafa',r=7)
    s.line(0,32,1280,32,'#e8e8e8')
    s.rect(15,10,12,12,BLUE,r=3)
    s.txt(36,22,'Routa',11,MUTED)
    s.line(1159,16,1169,16,MUTED)
    s.rect(1202,11,9,9,'none',MUTED,0)
    s.path('M1247 11l9 9m0-9-9 9',MUTED,1)
    s.rect(1,33,182,734,'#f3f3f3',r=0)
    s.rect(20,56,26,26,BLUE,r=6)
    s.icon('RouteOutlined',25,61,'white',16)
    s.txt(57,75,'Routa',17,INK,600)
    nav=[('SpaceDashboardOutlined','概览'),('SwapHorizOutlined','代理'),('AltRouteOutlined','进程分流'),('LanguageOutlined','订阅'),('DnsOutlined','节点')]
    for i,(icon,label) in enumerate(nav):
        y=109+i*46
        active=i==1
        if active:
            s.rect(8,y,168,40,'#e6e6e6',r=5)
            s.rect(8,y+11,3,18,BLUE,r=1.5)
        s.icon(icon,24,y+10,INK if active else MUTED,20)
        s.txt(60,y+26,label,14,INK,600 if active else 400)
    s.icon('SettingsOutlined',24,683,MUTED,20)
    s.txt(60,699,'设置',14)
    s.txt(24,744,'v0.2.0',11,DIM)
    s.txt(216,89,'代理',26,INK,600)
    s.dot(1015,78,3,GREEN)
    s.txt(1027,82,'内核运行中',12,MUTED)
    s.button(1136,61,112,'新增代理',True,'AddOutlined',action='add')


def toolbar(s,multiple=False):
    s.rect(216,125,1032,40,'#fafafa',LINE,6)
    chips=[('全部  5' if multiple else '全部  1',76),('运行中  3' if multiple else '运行中  1',96),('异常  1' if multiple else '异常  0',76)]
    x=224
    for i,(label,w) in enumerate(chips):
        if i==0: s.rect(x,131,w,28,'#e7f1fb',r=4)
        s.txt(x+w/2,150,label,12,BLUE if i==0 else MUTED,500 if i==0 else 400,'middle')
        x+=w+5
    s.p.append('<g data-action="view" tabindex="0" role="button" aria-label="切换卡片或列表视图" style="cursor:pointer">')
    s.rect(850,130,80,30,'white','#d5d5d5',4)
    s.rect(891 if multiple else 852,132,37,26,'#e7f1fb',r=3)
    s.icon('ViewAgendaOutlined',860,136,MUTED if multiple else BLUE,18)
    s.icon('ViewListOutlined',900,136,BLUE if multiple else MUTED,18)
    s.end()
    s.rect(942,129,300,32,'white','#d5d5d5',4)
    s.icon('SearchOutlined',952,136,DIM,18)
    s.txt(978,150,'搜索名称、端口或节点',12,DIM)

