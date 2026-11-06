"""Divergent vector concepts for review. No application code is imported."""
from pathlib import Path
from html import escape

ROOT=Path(__file__).parent
P=[]
INK='#202126'; MUTED='#6d7179'; BLUE='#0067c0'; GREEN='#147d3e'; BORDER='#e1e4e8'
def add(s): P.append(s)
def rect(x,y,w,h,fill='#fff',stroke='none',r=0,extra=''):
    add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill}" stroke="{stroke}" {extra}/>')
def txt(x,y,s,size=14,c=INK,weight=400,anchor='start'):
    add(f'<text x="{x}" y="{y}" font-size="{size}" fill="{c}" font-weight="{weight}" text-anchor="{anchor}">{escape(str(s))}</text>')
def path(d,c=MUTED,w=1.4,extra=''):
    add(f'<path d="{d}" fill="none" stroke="{c}" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round" {extra}/>')
def line(x,y,x2,y2,c=BORDER,w=1): path(f'M{x} {y}L{x2} {y2}',c,w)
def dot(x,y,c=GREEN,r=3): add(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{c}"/>')
def chevron(x,y,c=MUTED): path(f'M{x} {y}l3.5 3.5 3.5-3.5',c,1.2)
def more(x,y,c=MUTED):
    for d in [-4,0,4]: dot(x+d,y,c,1.1)
def copy(x,y,c=MUTED):
    rect(x+3,y+3,9,11,'none',c,1.5); path(f'M{x+8} {y}H{x}v10',c,1)
def toggle(x,y,c=BLUE): rect(x,y,32,17,c,r=8.5); dot(x+24,y+8.5,'#fff',5.5)
def button(x,y,w,label,primary=False,dark=False):
    fg='#202020' if primary and dark else '#fff' if primary else '#eee' if dark else INK
    rect(x,y,w,32,'#60cdff' if primary and dark else BLUE if primary else '#363636' if dark else '#fff','none' if primary else '#494949' if dark else '#d9dde3',4)
    txt(x+w/2,y+21,label,12,fg,500,'middle')
def field(x,y,w,label,dark=False):
    rect(x,y,w,34,'#303030' if dark else '#fff','#505050' if dark else '#d0d6de',4)
    txt(x+11,y+22,label,13,'#eee' if dark else INK); chevron(x+w-19,y+15,'#aaa' if dark else MUTED)
def badge(x,y,label,c=BLUE,bg='#eaf3fc'):
    w=12+len(label)*11
    rect(x,y,w,22,bg,r=4); txt(x+6,y+15,label,11,c)
def icon(name,x,y,c=INK):
    if name=='overview':
        for dx,dy,w,h in [(0,0,6,9),(10,0,6,5),(0,13,6,3),(10,9,6,7)]: rect(x+dx,y+dy,w,h,'none',c,.6)
    elif name=='input': path(f'M{x+6} {y}h10v16H{x+6}M{x} {y+8}h10m-3-3 3 3-3 3',c)
    elif name=='apps':
        for dx in [0,6,12]:
            for dy in [0,6,12]: rect(x+dx,y+dy,3,3,c,r=.5)
    elif name=='exit': path(f'M{x+8} {y+16}V{y+9}L{x+2} {y+3}m0 5V{y+3}h5M{x+8} {y+9}l6-6m-5 0h5v5',c)
    elif name=='nodes':
        for a,b in [(8,1),(1,7),(15,7),(4,15),(12,15)]:
            line(x+8,y+8,x+a,y+b,c); add(f'<circle cx="{x+a}" cy="{y+b}" r="2" fill="#f3f3f3" stroke="{c}" stroke-width="1.4"/>')
        dot(x+8,y+8,c,2)
    elif name=='globe':
        add(f'<circle cx="{x+8}" cy="{y+8}" r="7.5" fill="none" stroke="{c}" stroke-width="1.4"/>')
        path(f'M{x+1} {y+8}h14M{x+8} {y}c-5 5-5 11 0 16m0-16c5 5 5 11 0 16',c,1.2)
    elif name=='network':
        rect(x+3,y,10,7,'none',c,1); path(f'M{x+8} {y+7}v5m-6 0h12M{x+2} {y+12}v4m12-4v4',c)
    else:
        add(f'<circle cx="{x+8}" cy="{y+8}" r="5.5" fill="none" stroke="{c}" stroke-width="2.5" stroke-dasharray="3 2"/>'); add(f'<circle cx="{x+8}" cy="{y+8}" r="2.3" fill="none" stroke="{c}"/>')
def start(bg='#f3f3f3',dark=False):
    P.clear(); add('<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="832" viewBox="0 0 1200 832">')
    add('''<defs>
    <linearGradient id="mica" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#e9f0fc"/><stop offset=".55" stop-color="#f3f4f8"/><stop offset="1" stop-color="#f5f0f8"/></linearGradient>
    <linearGradient id="bluewash" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#eef5ff"/><stop offset="1" stop-color="#fafbfe"/></linearGradient>
    <linearGradient id="graphwash" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#60cdff" stop-opacity=".18"/><stop offset="1" stop-color="#60cdff" stop-opacity="0"/></linearGradient>
    <pattern id="grid" width="20" height="20" patternUnits="userSpaceOnUse"><circle cx="10" cy="10" r=".7" fill="#dce2ea"/></pattern>
    <filter id="shadow" x="-10%" y="-15%" width="120%" height="140%"><feDropShadow dx="0" dy="3" stdDeviation="5" flood-color="#172749" flood-opacity=".06"/></filter>
    </defs>''')
    add('<g font-family="Microsoft YaHei UI,Microsoft YaHei,Segoe UI,sans-serif">')
    rect(0,0,1200,832,bg,'#3e3e3e' if dark else '#d7dce4',8)
    rect(10,9,14,14,'#60cdff' if dark else BLUE,r=2); txt(30,20,'Routa',12,'#ddd' if dark else INK)
    c='#ccc' if dark else INK
    line(1080,16,1090,16,c); rect(1127,11,9,9,'none',c); path('M1173 11l9 9m0-9-9 9',c,1)
def sidebar(width=184,mica=False):
    rect(1,33,width-1,798,'url(#mica)' if mica else '#f2f3f5',r=0)
    rect(20,54,27,27,BLUE,r=6); path('M28 60v11q0 3 4 3h6M37 60v9m-3-3 3 3 3-3','#fff',1.5); txt(58,73,'Routa',17,weight=600)
    for i,(name,label) in enumerate([('overview','概览'),('input','监听器'),('apps','进程分流'),('exit','出口'),('nodes','节点'),('globe','Tailscale')]):
        y=111+i*49
        if not i: rect(9,y,width-18,39,'#fff','#dfe5ee' if mica else 'none',5); rect(10,y+10,3,19,BLUE,r=1.5)
        icon(name,26,y+12,BLUE if not i else '#474d56'); txt(58,y+26,label,13,BLUE if not i else '#41464f',600 if not i else 400)
    icon('settings',26,749,'#474d56'); txt(58,763,'设置',13,'#41464f'); txt(22,808,'v0.2.0',10,'#818795')
def save(name):
    add('</g></svg>'); (ROOT/(name+'.svg')).write_text('\n'.join(P),encoding='utf-8')
def graph(x,y,w,h,c=BLUE,dark=False):
    v=[.09,.12,.10,.13,.12,.14,.11,.2,.16,.24,.31,.29,.46,.37,.42,.71,.54,.62,.45,.88,.72,.49,.55,.36,.29,.34,.19,.22,.16,.18,.14,.19,.15,.21,.15,.17,.16,.14,.18,.16]
    for f in [.15,.5,.85]: line(x,y+f*h,x+w,y+f*h,'#363636' if dark else '#edf0f4')
    pts=[(x+i*w/39,y+h-a*h) for i,a in enumerate(v)]
    d='M'+'L'.join(f'{a:.1f} {b:.1f}' for a,b in pts)
    add(f'<path d="{d}L{x+w} {y+h}L{x} {y+h}Z" fill="url(#graphwash)"/>'); path(d,c,1.8)

