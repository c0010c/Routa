"""Review-only navigation and proxy workspace concept; all values are examples."""
from pathlib import Path
from html import escape

ROOT = Path(__file__).parent
P = []
BG, NAV, PANEL, ROW = '#202020', '#202020', '#272727', '#2d2d2d'
INK, MUTED, DIM = '#f3f3f3', '#bcbcbc', '#929292'
LINE, BLUE, GREEN = '#3b3b3b', '#4cc2ff', '#82cd8c'

def rect(x,y,w,h,fill,stroke='none',r=0):
    P.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill}" stroke="{stroke}"/>')
def text(x,y,s,size=14,color=INK,weight=400,anchor='start'):
    P.append(f'<text x="{x}" y="{y}" font-size="{size}" fill="{color}" font-weight="{weight}" text-anchor="{anchor}">{escape(str(s))}</text>')
def path(d,color=MUTED,w=1.5):
    P.append(f'<path d="{d}" fill="none" stroke="{color}" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round"/>')
def line(x,y,a,b,color=LINE): path(f'M{x} {y}L{a} {b}',color,1)
def dot(x,y,color=GREEN,r=3):
    P.append(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{color}"/>')
def chevron(x,y): path(f'M{x} {y}l4 4 4-4',MUTED,1.2)
def more(x,y):
    for dx in [-4,0,4]: dot(x+dx,y,MUTED,1)
def toggle(x,y,on=True):
    rect(x,y,34,18,BLUE if on else '#454545',r=9)
    dot(x+(25 if on else 9),y+9,'#202020' if on else '#bcbcbc',6)
def info(x,y):
    P.append(f'<circle cx="{x}" cy="{y}" r="7" fill="none" stroke="{DIM}"/>')
    text(x,y+4,'i',10,MUTED,500,'middle')
def button(x,y,w,label,primary=False):
    rect(x,y,w,34,BLUE if primary else '#333333','none' if primary else '#454545',5)
    text(x+w/2,y+22,label,12,'#14212a' if primary else INK,500,'middle')
def field(x,y,w,label):
    rect(x,y,w,34,'#313131','#484848',5)
    text(x+12,y+22,label,12); chevron(x+w-22,y+14)
def icon(name,x,y,c=MUTED):
    if name=='overview':
        for dx,dy,w,h in [(0,0,6,6),(10,0,6,6),(0,10,6,6),(10,10,6,6)]: rect(x+dx,y+dy,w,h,'none',c,1)
    elif name=='proxy':
        path(f'M{x} {y+5}h16m-4-4 4 4-4 4M{x+16} {y+13}H{x}m4-4-4 4 4 4',c)
    elif name=='exit':
        path(f'M{x+8} {y+17}v-7l-6-7m0 5V{y+3}h5M{x+8} {y+10}l6-7m-5 0h5v5',c)
    elif name=='nodes':
        for dx,dy in [(1,3),(15,3),(8,16)]:
            line(x+8,y+8,x+dx,y+dy,c); dot(x+dx,y+dy,c,2)
        dot(x+8,y+8,c,2.5)
    elif name=='input':
        path(f'M{x+6} {y}h10v16H{x+6}M{x} {y+8}h10m-3-3 3 3-3 3',c)
    elif name=='apps':
        for dx in [0,9]:
            for dy in [0,9]: rect(x+dx,y+dy,6,6,'none',c,1)
    elif name=='copy':
        rect(x+4,y+3,8,10,'none',c,1); path(f'M{x+8} {y}H{x}v10',c,1.2)
    elif name=='settings':
        P.append(f'<circle cx="{x+8}" cy="{y+8}" r="6.5" fill="none" stroke="{c}" stroke-width="3" stroke-dasharray="3 3"/>')
        P.append(f'<circle cx="{x+8}" cy="{y+8}" r="2.7" fill="none" stroke="{c}" stroke-width="1.4"/>')

P.append('<svg xmlns="http://www.w3.org/2000/svg" width="1440" height="900" viewBox="0 0 1440 900">')
P.append('<title>Routa 代理工作区设计稿：监听器与进程左右分栏，设置固定底部</title>')
P.append('<g font-family="Segoe UI,Microsoft YaHei UI,Microsoft YaHei,sans-serif">')
rect(0,0,1440,900,BG,r=10)
rect(0,0,1440,34,'#1c1c1c')
rect(14,10,14,14,BLUE,r=3); text(37,22,'Routa',12,MUTED)
line(1311,17,1321,17,MUTED); rect(1361,12,10,10,'none',MUTED)
path('M1410 12l10 10m0-10-10 10',MUTED,1)

# Sidebar: four primary destinations, one anchored utility destination.
rect(0,34,204,866,NAV)
rect(22,60,28,28,BLUE,r=7)
path('M30 66v10q0 4 4 4h8M40 66v9m-3-3 3 3 3-3','#17222b',1.8)
text(62,81,'Routa',18,INK,600)
for i,(name,label) in enumerate([('overview','概览'),('proxy','代理'),('exit','出口'),('nodes','订阅与节点')]):
    y=116+i*48
    if i==1:
        rect(10,y,184,40,'#303030',r=6); rect(10,y+11,3,18,BLUE,r=1.5)
    icon(name,26,y+12,INK if i==1 else MUTED)
    text(59,y+26,label,14,INK if i==1 else MUTED,600 if i==1 else 400)
line(22,794,182,794)
icon('settings',26,819); text(59,833,'设置',14,MUTED)
text(24,878,'v0.2.0',11,DIM)

# Page title and compact runtime state.
text(236,103,'代理',28,INK,600)
dot(1280,95); text(1292,100,'内核运行中',12,MUTED)
text(236,137,'监听器',12,MUTED); text(286,137,'3',12,INK,600)
text(314,137,'·',12,DIM); text(332,137,'进程规则',12,MUTED); text(395,137,'3',12,INK,600)
text(1408,137,'设计预览 · 示例数据',11,DIM,anchor='end')

# Left: listener definitions and their exit choices.
lx,lw,rx,rw=236,516,776,632
rect(lx,164,lw,648,PANEL,LINE,8)
icon('input',256,188,INK); text(282,202,'监听器',17,INK,600)
info(351,196); button(618,180,114,'＋ 新建监听器')
line(lx,220,lx+lw,220)
for i,(port,name,exitname,users,node,latency) in enumerate([
    ('7881','监听器 01','日常出口','2 个进程','香港 01','42 ms'),
    ('7882','监听器 02','开发出口','1 个进程','新加坡 02','86 ms'),
    ('7883','监听器 03','备用出口','暂无进程','日本 01','68 ms'),
]):
    y=236+i*174
    rect(252,y,484,158,ROW,'#3e3e3e',6)
    text(270,y+29,port,22,INK,600)
    rect(336,y+11,43,23,'#3a3a3a',r=4); text(357.5,y+27,'混合',11,MUTED,anchor='middle')
    text(391,y+28,name,12,MUTED)
    toggle(655,y+15); more(712,y+24)
    text(270,y+55,'127.0.0.1',11,DIM); icon('copy',345,y+44,DIM)
    dot(384,y+51,GREEN,2.5); text(395,y+55,'运行中',11,GREEN)
    text(270,y+86,'出口',11,MUTED); text(718,y+86,users,11,MUTED,anchor='end')
    field(270,y+99,220,exitname)
    text(508,y+113,node,12,INK); text(508,y+132,latency,11,MUTED)
line(252,776,736,776)
text(256,798,'默认提供 3 个混合监听器',11,DIM)

# Right: process routing, distinct selection of a listener for each program.
rect(rx,164,rw,648,PANEL,LINE,8)
icon('apps',796,188,INK); text(822,202,'进程',17,INK,600)
dot(889,196,GREEN,2.5); text(900,201,'分流运行中',11,GREEN)
toggle(1020,187); info(1072,196)
button(1240,180,148,'＋ 添加进程',True)
line(rx,220,rx+rw,220)
rect(796,239,592,35,'#242424','#424242',5)
P.append('<circle cx="811" cy="254" r="4.5" fill="none" stroke="#929292" stroke-width="1.2"/>')
path('M814 257l4 4',DIM,1.2); text(831,261,'搜索进程名称或路径',12,DIM)
for x,label in [(796,'进程'),(1042,'监听器'),(1244,'状态')]: text(x,306,label,11,MUTED)
line(796,321,1388,321)
for i,(name,sub,target) in enumerate([
    ('chrome.exe','Google Chrome','7881 · 监听器 01'),
    ('Code.exe','Visual Studio Code','7882 · 监听器 02'),
    ('Telegram.exe','Telegram Desktop','7881 · 监听器 01'),
]):
    y=322+i*90
    rect(796,y+23,34,34,'#373737',r=6)
    if i==0:
        P.append(f'<circle cx="813" cy="{y+40}" r="9" fill="none" stroke="{MUTED}" stroke-width="1.3"/>')
        path(f'M804 {y+40}h18M813 {y+31}c-5 5-5 13 0 18m0-18c5 5 5 13 0 18',MUTED,1.2)
    elif i==1: path(f'M808 {y+35}l-5 5 5 5m12-14-10 18m10-18v18',MUTED,1.5)
    else: path(f'M803 {y+39}l20-7-6 18-5-7-9-4m9 4 7-7',MUTED,1.4)
    text(842,y+36,name,14,INK,500); text(842,y+58,sub,11,DIM)
    field(1042,y+24,178,target)
    dot(1250,y+41,GREEN,2.5); text(1260,y+46,'已代理',11,GREEN)
    toggle(1322,y+32); more(1376,y+41)
    line(796,y+89,1388,y+89)
text(796,624,'3 条规则已启用',11,MUTED)
text(1388,624,'查看连接 →',11,BLUE,anchor='end')

# Secondary controls are kept nearby and collapsed by default.
line(796,699,1388,699)
icon('settings',800,721,MUTED); text(828,735,'分流设置',12,INK)
text(1359,735,'›',20,MUTED)
text(828,758,'退出行为、故障策略与组件管理',11,DIM)
line(796,776,1388,776)
dot(802,794,GREEN,2.5); text(814,798,'所有规则已生效',11,MUTED)
text(236,858,'代理',11,DIM); text(270,858,'/  监听器与进程',11,DIM)
P.append('</g></svg>')
(ROOT/'proxy-workspace.svg').write_text('\n'.join(P),encoding='utf-8')
print(ROOT/'proxy-workspace.svg')
