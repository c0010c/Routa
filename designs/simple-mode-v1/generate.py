"""Review mockups for two alternative simple interfaces. No app source changes."""
from pathlib import Path
from html import escape
import re

ROOT=Path(__file__).parent
ICONS=ROOT.parents[1]/'node_modules'/'@mui'/'icons-material'
P=[]
BG,NAV,CARD,ROW='#202020','#202020','#282828','#303030'
INK,MUTED,DIM,LINE,BLUE,GREEN='#f3f3f3','#c0c0c0','#929292','#3c3c3c','#4cc2ff','#82cd8c'

def rect(x,y,w,h,c,r=0,stroke='none',sw=1):
    P.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{c}" stroke="{stroke}" stroke-width="{sw}"/>')
def text(x,y,s,size=14,c=INK,weight=400,anchor='start'):
    P.append(f'<text x="{x}" y="{y}" font-size="{size}" fill="{c}" font-weight="{weight}" text-anchor="{anchor}">{escape(s)}</text>')
def path(d,c=MUTED,sw=1.6):
    P.append(f'<path d="{d}" fill="none" stroke="{c}" stroke-width="{sw}" stroke-linecap="round" stroke-linejoin="round"/>')
def line(x,y,a,b,c=LINE): path(f'M{x} {y}L{a} {b}',c,1)
def circle(x,y,r,fill='none',stroke='none',sw=1):
    P.append(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>')
def icon(name,x,y,c=MUTED,size=22):
    source=(ICONS/(name+'.mjs')).read_text(encoding='utf-8')
    ds=re.findall(r'd:\s*"([^"]+)"',source)
    assert ds,name
    P.append(f'<g transform="translate({x} {y}) scale({size/24})" fill="{c}">')
    P.extend(f'<path d="{d}"/>' for d in ds); P.append('</g>')
def button(x,y,w,label,primary=False,h=36):
    rect(x,y,w,h,BLUE if primary else '#333333',5,'none' if primary else '#484848')
    text(x+w/2,y+h/2+5,label,13,'#14212a' if primary else INK,500,'middle')
def toggle(x,y,on=True):
    rect(x,y,36,20,BLUE if on else '#484848',10)
    circle(x+(26 if on else 10),y+10,6,'#202020' if on else '#c3c3c3')
def chevron(x,y): path(f'M{x} {y}l4 4 4-4',MUTED,1.3)
def chip(x,y,label):
    w=14+len(label)*11
    rect(x,y,w,23,'#333333',4);text(x+7,y+16,label,11,MUTED)
def start(title):
    P.clear()
    P.append('<svg xmlns="http://www.w3.org/2000/svg" width="1120" height="760" viewBox="0 0 1120 760">')
    P.append(f'<title>{escape(title)}</title><g font-family="Segoe UI,Microsoft YaHei UI,Microsoft YaHei,sans-serif">')
    rect(0,0,1120,760,BG,10)
    rect(0,0,1120,34,'#1c1c1c');rect(14,10,14,14,BLUE,3);text(37,22,'Routa',12,MUTED)
    line(990,17,1000,17,MUTED);rect(1038,12,10,10,'none',0,MUTED)
    path('M1083 12l10 10m0-10-10 10',MUTED,1)
def save(name):
    P.append('</g></svg>');(ROOT/(name+'.svg')).write_text('\n'.join(P),encoding='utf-8')
def sidebar(active='连接',apps=False):
    rect(20,60,27,27,BLUE,6);icon('RouteOutlined',24,64,'#17222b',19);text(58,80,'Routa',17,weight=600)
    items=[('SwapHorizOutlined','连接')]
    if apps: items.append(('AltRouteOutlined','应用规则'))
    items.append(('DnsOutlined','订阅与节点'))
    for i,(name,label) in enumerate(items):
        y=119+i*48
        if label==active:
            rect(10,y,164,40,'#303030',6);rect(10,y+11,3,18,BLUE,1.5)
        icon(name,25,y+9,INK if active==label else MUTED,21)
        text(59,y+26,label,14,INK if active==label else MUTED,600 if active==label else 400)
    line(22,661,162,661)
    if active=='设置': rect(10,680,164,40,'#303030',6);rect(10,691,3,18,BLUE,1.5)
    icon('SettingsOutlined',25,690,INK if active=='设置' else MUTED,21)
    text(59,706,'设置',14,INK if active=='设置' else MUTED)
    text(24,739,'v0.2.0',11,DIM)
def heading(title,apps=False,active='连接'):
    sidebar(active,apps)
    text(220,103,title,27,weight=600);chip(1020,81,'简洁版')
def power(cx,cy,on=True):
    circle(cx,cy,44,'#233842' if on else '#303030',BLUE if on else '#5b5b5b',1.5)
    path(f'M{cx-16} {cy-17}a24 24 0 1 0 32 0M{cx} {cy-28}v26',BLUE if on else MUTED,2.5)

# First launch: a single product-level choice between simple and full UI.
start('首次启动：选择界面模式')
rect(48,66,28,28,BLUE,6);icon('RouteOutlined',52,70,'#17222b',20);text(88,87,'Routa',18,weight=600)
text(560,158,'选择适合你的界面',30,weight=600,anchor='middle')
text(560,192,'以后可在「设置 → 界面模式」中随时切换',14,MUTED,anchor='middle')
for i,(name,desc) in enumerate([('简洁版','选好节点，快速开始使用'),('完整版','独立管理代理、应用规则与出口')]):
    x=164+i*410
    rect(x,232,382,318,'#293037' if i==0 else CARD,10,BLUE if i==0 else LINE,1.5 if i==0 else 1)
    circle(x+345,260,9,BLUE if i==0 else 'none',BLUE if i==0 else '#787878')
    if i==0: path(f'M{x+341} 260l3 3 5-6','#17222b',1.7)
    text(x+24,278,name,22,weight=600)
    if i==0: chip(x+110,258,'推荐')
    text(x+24,309,desc,13,MUTED)
    rect(x+24,334,334,128,'#202020',5,'#444444')
    if i==0:
        circle(x+191,371,19,'#233842',BLUE)
        path(f'M{x+184} 365a10 10 0 1 0 14 0M{x+191} 359v12',BLUE,1.4)
        rect(x+84,404,214,16,'#353535',3)
        rect(x+147,431,88,12,BLUE,3)
    else:
        rect(x+34,345,65,106,'#2c2c2c',2)
        for j in range(5): rect(x+42,355+j*17,47,6,'#535353',2)
        for j in range(3):
            rect(x+112,345+j*35,232,27,'#303030',3)
            rect(x+122,354+j*35,93,7,'#555555',2)
    text(x+24,493,'日常操作更集中' if i==0 else '使用全部功能',13,INK)
    text(x+24,520,'减少术语和配置步骤' if i==0 else '适合多端口、多出口和精细分流',12,DIM)
button(422,584,276,'使用简洁版',True,44)
text(560,660,'此选项仅决定界面的展示方式',12,DIM,anchor='middle')
save('01-first-launch')

# Alternative 1: everyday proxy, only a node picker and a connection control.
start('方案 1：日常代理简洁版')
heading('连接')
rect(282,144,740,468,CARD,9,LINE)
power(652,230)
text(652,316,'已连接',29,weight=600,anchor='middle')
circle(591,344,3,GREEN);text(604,349,'系统代理已开启',13,MUTED)
rect(314,379,676,80,ROW,6,'#454545')
icon('LanguageOutlined',334,405,MUTED,25)
text(375,407,'当前节点',11,DIM)
text(375,435,'香港 01',17,weight=500)
text(873,426,'42 ms',12,GREEN)
chevron(958,416)
button(530,486,244,'断开连接',False,42)
text(532,575,'↓  2.4 MB/s',13,MUTED)
text(688,575,'↑  128 KB/s',13,MUTED)
text(314,651,'更多连接选项',13,MUTED);chevron(412,643)
text(1088,738,'方案 1 · 日常代理 · 示例状态',11,DIM,anchor='end')
save('02-simple-daily')

# Alternative 3: one shared node for selected apps; no listener/exit concepts.
start('方案 3：简洁连接与应用规则')
heading('应用规则',True,'应用规则')
text(220,136,'只让选中的应用走代理',13,MUTED)
rect(220,163,868,102,CARD,8,LINE)
text(244,196,'应用代理',16,weight=600)
circle(247,226,3,GREEN);text(258,231,'已开启',12,GREEN)
toggle(401,201)
line(480,185,480,244)
text(508,195,'使用节点',11,DIM)
rect(508,207,556,38,ROW,5,'#494949')
text(521,232,'香港 01',14);text(962,232,'42 ms',12,GREEN);chevron(1038,222)
text(220,308,'选择应用',17,weight=600)
button(957,281,131,'＋ 添加应用',True)
rect(220,330,868,268,CARD,8,LINE)
apps=[('LanguageOutlined','Google Chrome','chrome.exe',True),('SendOutlined','Telegram','Telegram.exe',True),('ChatBubbleOutlineOutlined','微信','WeChat.exe',False)]
for i,(appicon,name,exe,on) in enumerate(apps):
    y=331+i*88
    rect(244,y+23,40,40,'#363636',7)
    icon(appicon,254,y+33,MUTED,20)
    text(301,y+37,name,15,weight=500);text(301,y+59,exe,11,DIM)
    text(925,y+48,'走代理' if on else '直连',12,GREEN if on else DIM)
    toggle(1018,y+32,on)
    if i<2: line(244,y+87,1064,y+87)
text(244,632,'其余应用保持直连',12,MUTED)
text(1088,632,'2 个应用走代理',12,DIM,anchor='end')
text(1088,738,'方案 3 · 连接＋应用规则 · 示例状态',11,DIM,anchor='end')
save('03-simple-with-apps')

# Settings: a UI preference, separate from proxy behavior or core lifecycle.
start('设置：随时切换界面模式')
heading('设置',False,'设置')
text(220,151,'界面',16,weight=600)
rect(220,170,868,105,CARD,8,LINE)
text(244,205,'界面模式',15,weight=500)
text(244,242,'切换后保留现有连接和配置',12,MUTED)
rect(766,192,296,46,'#202020',6,'#484848')
rect(770,196,144,38,'#3a3a3a',4)
text(842,221,'简洁版',14,weight=600,anchor='middle')
rect(822,231,40,2,BLUE,1)
text(988,221,'完整版',14,MUTED,anchor='middle')
text(220,325,'常规',16,weight=600)
rect(220,344,868,160,CARD,8,LINE)
text(244,383,'开机启动',14);toggle(1020,366,False)
line(244,399,1064,399)
text(244,438,'启动后自动连接',14);toggle(1020,421,False)
text(244,479,'仅在已有可用节点时连接',12,DIM)
rect(220,536,868,58,CARD,7,LINE)
text(244,572,'故障诊断',14);text(1053,573,'›',22,MUTED)
text(1088,738,'两种简洁方案共用此设置入口',11,DIM,anchor='end')
save('04-settings-mode')

# Fresh-install empty state for the everyday alternative.
start('简洁版：首次添加订阅')
heading('连接')
rect(282,144,740,468,CARD,9,LINE)
power(652,230,False)
text(652,316,'添加节点后即可连接',25,weight=600,anchor='middle')
text(652,353,'导入服务商提供的订阅链接',13,MUTED,anchor='middle')
button(530,396,244,'导入订阅',True,42)
text(652,476,'手动添加节点',13,BLUE,anchor='middle')
text(652,564,'还没有节点？ ⓘ',12,DIM,anchor='middle')
text(1088,738,'方案 1 · 首次使用状态',11,DIM,anchor='end')
save('05-first-import')
print('Saved five review-only mockups')
