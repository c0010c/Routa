"""Navigation review mockup. All runtime values are illustrative."""
from pathlib import Path
import re

ROOT = Path(__file__).parent
helper = (ROOT.parent / 'navigation-proxy-v1' / 'generate.py').read_text(encoding='utf-8')
exec(helper.split("P.append('<svg")[0])
ROOT = Path(__file__).parent

def nav_icon(name,x,y,c=MUTED):
    """User-approved set A, taken directly from the project's icon library."""
    names={'overview':'SpaceDashboardOutlined','proxy':'SwapHorizOutlined',
           'apps':'AltRouteOutlined','exit':'LanguageOutlined',
           'nodes':'DnsOutlined','settings':'SettingsOutlined'}
    source=(ROOT.parents[1]/'node_modules'/'@mui'/'icons-material'/(names[name]+'.mjs')).read_text(encoding='utf-8')
    ds=re.findall(r'd:\s*"([^"]+)"',source)
    assert ds,name
    P.append(f'<g transform="translate({x-2} {y-2}) scale(.9)" fill="{c}">')
    P.extend(f'<path d="{d}"/>' for d in ds)
    P.append('</g>')

P.append('<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="800" viewBox="0 0 1280 800">')
P.append('<title>Routa 菜单方案：代理与进程分流独立，设置固定底部</title>')
P.append('<g font-family="Segoe UI,Microsoft YaHei UI,Microsoft YaHei,sans-serif">')
rect(0,0,1280,800,BG,r=10)
rect(0,0,1280,34,'#1c1c1c')
rect(14,10,14,14,BLUE,r=3); text(37,22,'Routa',12,MUTED)
line(1150,17,1160,17,MUTED); rect(1201,12,10,10,'none',MUTED)
path('M1251 12l10 10m0-10-10 10',MUTED,1)

# Five primary destinations, with settings anchored at the bottom.
rect(0,34,204,766,NAV)
rect(22,60,28,28,BLUE,r=7)
path('M30 66v10q0 4 4 4h8M40 66v9m-3-3 3 3 3-3','#17222b',1.8)
text(62,81,'Routa',18,INK,600)
for i,(name,label) in enumerate([
    ('overview','概览'),('proxy','代理'),('apps','进程分流'),
    ('exit','出口'),('nodes','订阅与节点')
]):
    y=116+i*48
    selected=i==1
    if selected:
        rect(10,y,184,40,'#303030',r=6)
        rect(10,y+11,3,18,BLUE,r=1.5)
    nav_icon(name,25,y+11,INK if selected else MUTED)
    text(59,y+26,label,14,INK if selected else MUTED,600 if selected else 400)
line(22,701,182,701)
nav_icon('settings',25,724); text(59,739,'设置',14,MUTED)
text(24,778,'v0.2.0',11,DIM)

# Proxy page: a single default mixed listener, no process sub-navigation.
text(240,105,'代理',28,INK,600)
dot(1121,96); text(1133,101,'内核运行中',12,MUTED)
text(240,143,'监听器',13,MUTED)
rect(292,127,23,22,'#343434',r=4)
text(303.5,143,'1',11,INK,500,'middle')
button(1104,123,140,'＋ 新建监听器',True)

rect(240,181,1004,289,PANEL,LINE,8)
icon('input',264,210,INK)
text(295,226,'默认监听器',17,INK,600)
rect(404,207,44,24,'#393939',r=4)
text(426,224,'混合',11,MUTED,anchor='middle')
dot(472,219); text(484,224,'运行中',12,GREEN)
toggle(1142,210); more(1216,219)
line(264,250,1220,250)

text(264,282,'监听地址',12,MUTED)
text(264,321,'127.0.0.1:7881',22,INK,500)
icon('copy',461,305,MUTED)
text(264,350,'HTTP / SOCKS5',12,DIM)

text(758,282,'出口',12,MUTED)
field(758,296,286,'日常出口')
text(1070,318,'香港 01',13,INK)
text(1070,345,'42 ms',12,GREEN)
line(264,377,1220,377)

text(264,420,'系统代理',13,INK); info(332,415)
toggle(356,406,False)
text(402,420,'未开启',12,DIM)
line(688,402,688,434)
text(758,420,'TUN',13,INK); info(802,415)
toggle(826,406,False)
text(872,420,'未开启',12,DIM)

text(240,514,'进程分流',13,MUTED)
text(322,514,'›',18,DIM)
text(346,514,'单独管理应用的代理规则',12,DIM)
text(1244,514,'前往进程分流 →',12,BLUE,anchor='end')

text(240,778,'菜单方案 · 设置固定底部',11,DIM)
text(1244,778,'设计预览 · 示例数据',11,DIM,anchor='end')
P.append('</g></svg>')
(ROOT/'04-approved-menu-a.svg').write_text('\n'.join(P),encoding='utf-8')
print(ROOT/'04-approved-menu-a.svg')
