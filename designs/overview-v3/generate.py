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

# C: Exit-oriented groups in a light Mica shell. The groups reveal shared scope.
start('url(#mica)'); sidebar(184,True)
txt(216,82,'连接总览',27,weight=600); dot(1021,75); txt(1033,80,'内核运行中',12); more(1164,75)
txt(216,109,'3 个监听器，连接到 2 个出口',12,MUTED)
rect(216,132,952,87,'#ffffff', '#e0e5ed',8)
for x,label,value in [(238,'上传','0 B/s'),(452,'下载','444 B/s'),(690,'累计流量','317.3 MB'),(950,'监听器','3 / 3 运行中')]:
    txt(x,157,label,11,MUTED); txt(x,192,value,21,GREEN if label=='监听器' else INK,600)
for x in [430,668,928]: line(x,152,x,198,'#e9edf3')
txt(216,262,'连接',18,weight=600)
rect(292,240,162,32,'#e2e7ef',r=5); rect(373,243,78,26,'#fff',r=4)
txt(332,261,'按入口',12,MUTED,anchor='middle'); txt(412,261,'按出口',12,INK,500,'middle')
button(1058,240,110,'＋ 新建监听器',True)
# Shared exit card.
rect(216,289,573,369,'#fff','#dbe2ec',9,'filter="url(#shadow)"')
rect(217,290,571,171,'url(#bluewash)',r=8); rect(217,446,571,15,'url(#bluewash)')
icon('exit',238,311,BLUE); txt(264,326,'机场出口',16,weight=600); badge(350,308,'2 个入口共用'); more(766,321)
rect(238,353,42,42,'#e0ecfa',r=8); txt(259,381,'SG',15,BLUE,600,'middle')
txt(294,375,'新加坡 01',24,weight=600); txt(294,401,'手动选择',11,MUTED)
button(673,362,94,'切换节点')
dot(242,433); txt(252,437,'182 ms',12,GREEN,500); icon('network',335,422,MUTED); txt(360,436,'系统默认网卡',12,MUTED)
txt(238,490,'接入此出口',11,MUTED); txt(767,490,'实时流量',11,MUTED,anchor='end')
for y,name,port,rate in [(507,'机场','7893 · SOCKS5','359 B/s'),(577,'浏览器','7894 · 混合','0 B/s')]:
    if y>507: line(238,y-8,767,y-8,'#edf0f4')
    dot(242,y+17); txt(254,y+22,name,14,weight=600); txt(254,y+43,port,11,MUTED)
    copy(417,y+28); txt(663,y+23,'↓ '+rate,12,anchor='end'); toggle(692,y+9); more(755,y+17)
# Independent exit, same visual grammar but different width.
rect(805,289,363,369,'#fff','#dbe2ec',9,'filter="url(#shadow)"')
rect(806,290,361,171,'#f8f9fc',r=8); rect(806,446,361,15,'#f8f9fc')
icon('exit',827,311,'#6156a2'); txt(853,326,'下载专用',16,weight=600); more(1146,321)
rect(827,353,42,42,'#eeeaf7',r=8); txt(848,381,'JP',15,'#6156a2',600,'middle')
txt(883,375,'东京 02',24,weight=600); txt(883,401,'手动选择',11,MUTED)
button(1054,365,92,'切换节点')
dot(831,433); txt(841,437,'96 ms',12,GREEN,500); txt(928,436,'系统默认网卡',12,MUTED)
txt(827,490,'接入此出口',11,MUTED)
dot(831,524); txt(843,529,'下载器',14,weight=600); txt(843,551,'7895 · SOCKS5',11,MUTED); copy(949,538)
txt(1061,529,'↓ 85 B/s',12,anchor='end'); toggle(1091,514); more(1146,523)
txt(827,625,'独立出口，仅影响此入口',11,MUTED)
rect(216,678,952,67,'#ffffff','#dfe5ed',7)
icon('input',237,703,BLUE); txt(266,704,'连接地址',12,weight=600); txt(266,726,'选择入口即可复制地址，供程序或设备连接。',11,MUTED)
txt(1117,718,'管理监听器',12,BLUE,anchor='end'); path('M1134 709l5 5-5 5',BLUE)
txt(216,791,'全部配置已生效',11,MUTED); txt(1168,791,'节点或网卡不可用时，相关入口自动停用',11,MUTED,anchor='end')
save('overview-c-exit-groups')

# D: Dark Fluent desktop workbench, horizontal navigation, wider live monitor.
start('#202020',True)
rect(1,33,1198,56,'#272727')
rect(23,48,27,27,'#60cdff',r=6); path('M31 55v10q0 3 4 3h6M40 55v8m-3-3 3 3 3-3','#202020',1.5); txt(61,67,'Routa',17,'#f0f0f0',600)
for x,label in [(180,'概览'),(260,'监听器'),(358,'进程分流'),(478,'出口'),(556,'节点'),(634,'Tailscale')]:
    txt(x,67,label,13,'#f4f4f4' if x==180 else '#b8b8b8',600 if x==180 else 400)
rect(184,82,20,3,'#60cdff',r=1.5); icon('settings',1138,54,'#c6c6c6')
txt(32,134,'概览',27,'#f5f5f5',600); dot(1042,125,'#6ccb5f'); txt(1054,130,'内核运行中',12,'#c2c2c2')
button(1058,153,110,'＋ 新建监听器',True,True)
txt(32,177,'流量监控',14,'#eeeeee',600)
rect(32,201,744,267,'#292929','#383838',7)
txt(53,232,'下载速率',11,'#aaa'); txt(53,267,'444',32,'#f5f5f5',600); txt(130,265,'B/s',14,'#aaa')
txt(235,232,'上传速率',11,'#aaa'); txt(235,266,'0 B/s',21,'#d3d3d3',500)
txt(754,232,'最近 60 秒',11,'#999',anchor='end')
graph(54,287,698,143,'#60cdff',True); txt(54,449,'60 秒前',10,'#999'); txt(754,449,'现在',10,'#999',anchor='end')
rect(792,201,376,267,'#292929','#383838',7)
txt(814,232,'运行状态',12,'#aaa')
txt(814,284,'3',44,'#f4f4f4',600); txt(854,282,'个监听器运行中',15,'#e3e3e3')
dot(818,312,'#6ccb5f'); txt(830,317,'所有配置均已生效',12,'#b9b9b9')
line(814,339,1146,339,'#414141')
txt(814,367,'累计上传',11,'#aaa'); txt(1146,367,'153.1 MB',15,'#f2f2f2',500,'end')
txt(814,400,'累计下载',11,'#aaa'); txt(1146,400,'164.2 MB',15,'#f2f2f2',500,'end')
txt(814,443,'内核操作',11,'#a6a6a6'); more(1138,438,'#aaa')
txt(32,510,'监听器',16,'#eee',600); txt(113,509,'3 个',12,'#999')
txt(1168,509,'全部 3     异常 0',12,'#b0b0b0',anchor='end')
rect(32,530,1136,240,'#292929','#383838',7)
txt(52,556,'入口',11,'#999'); txt(302,556,'出口 / 节点',11,'#999'); txt(735,556,'网卡',11,'#999'); txt(895,556,'延迟与流量',11,'#999'); txt(1086,556,'操作',11,'#999')
line(32,572,1168,572,'#414141')
for i,(name,port,exit,node,delay,rate) in enumerate([('机场','7893 · SOCKS5','机场出口','SG 新加坡01','182 ms','359 B/s'),('浏览器','7894 · 混合','机场出口','SG 新加坡01','182 ms','0 B/s'),('下载器','7895 · SOCKS5','下载专用','JP 东京02','96 ms','85 B/s')]):
    y=572+i*66
    if i==0: rect(33,y,1134,66,'#2d353c'); rect(33,y+16,3,34,'#60cdff',r=1.5)
    if i: line(32,y,1168,y,'#383838')
    dot(56,y+24,'#6ccb5f'); txt(68,y+29,name,14,'#eee',600); txt(68,y+49,port,11,'#aaa')
    txt(302,y+29,exit,13,'#eee'); txt(405,y+29,'→',13,'#777'); txt(435,y+29,node,13,'#eee'); chevron(596,y+22,'#aaa')
    txt(302,y+49,'共用 · 2 个入口' if i<2 else '独立出口',10,'#9aa7b1')
    txt(735,y+29,'系统默认',12,'#c1c1c1')
    txt(895,y+27,delay,12,'#6ccb5f'); txt(982,y+27,'↓ '+rate,11,'#c4c4c4'); txt(895,y+48,'↑ 0 B/s',10,'#999')
    toggle(1086,y+24,'#60cdff'); more(1146,y+32,'#aaa')
txt(32,803,'机场   ·   127.0.0.1:7893',11,'#a1a1a1'); copy(242,790,'#a1a1a1'); txt(1168,803,'仅本机访问',11,'#a1a1a1',anchor='end')
save('overview-d-dark-workbench')

# E: The relationship itself is the overview. Shared exits have one visible node.
start('#f4f5f7'); sidebar(184)
txt(216,81,'路由概览',27,weight=600); txt(216,109,'查看每个入口的流量去向',12,MUTED)
dot(1020,75); txt(1032,80,'内核运行中',12); more(1164,75)
txt(216,151,'3 个入口运行中',12,GREEN); txt(377,151,'↑ 0 B/s',12,MUTED); txt(493,151,'↓ 444 B/s',12,INK)
button(1058,127,110,'＋ 新建监听器',True)
rect(216,178,952,460,'#fcfdff','#dfe4eb',8)
rect(217,179,950,458,'url(#grid)',r=8)
for x,label,sub in [(240,'代理入口','连接这些端口'),(542,'出口与节点','决定流量去向'),(970,'网卡','实际使用的网络')]:
    txt(x,211,label,13,INK,600); txt(x,232,sub,10,MUTED)
# Wire paths are drawn first so they sit beneath all nodes.
path('M430 300H469Q486 300 486 319V343Q486 361 505 361H542','#71a8e0',2.2)
path('M430 414H469Q486 414 486 395V379Q486 361 505 361H542','#71a8e0',2.2)
path('M430 548H542','#9daebd',1.8)
path('M818 361H871Q892 361 892 380V399Q892 419 912 419H970','#71a8e0',2.2)
path('M818 548H871Q892 548 892 529V439Q892 419 912 419H970','#9daebd',1.8)
dot(486,361,'#71a8e0',3.5); dot(892,419,'#71a8e0',3.5)
for x,y in [(520,361),(948,419),(520,548)]: path(f'M{x-5} {y-4}l5 4-5 4','#8ca6bd',1.3)
for y,name,port,rate in [(264,'机场','7893 · SOCKS5','359 B/s'),(378,'浏览器','7894 · 混合','0 B/s'),(512,'下载器','7895 · SOCKS5','85 B/s')]:
    rect(240,y,190,73,'#fff','#d7dfe9',6)
    dot(255,y+23); txt(266,y+28,name,14,weight=600); txt(255,y+52,port,11,MUTED)
    txt(415,y+27,'↓ '+rate,10,MUTED,anchor='end')
# Shared exit is selected.
rect(542,289,276,145,'#fff','#0067c0',7,'filter="url(#shadow)"')
rect(543,290,274,37,'#edf5ff',r=6); rect(543,313,274,14,'#edf5ff')
icon('exit',556,301,BLUE); txt(581,315,'机场出口',14,weight=600); badge(705,296,'共用 2 个')
txt(558,357,'SG 新加坡01',19,weight=600); chevron(790,347)
txt(558,383,'手动选择',11,MUTED); dot(561,411); txt(571,416,'182 ms',12,GREEN)
more(796,410)
rect(542,494,276,110,'#fff','#d7dfe9',7)
icon('exit',558,509,'#6156a2'); txt(583,524,'下载专用',14,weight=600)
txt(558,558,'JP 东京02',18,weight=600); txt(558,585,'手动选择',11,MUTED); txt(800,585,'96 ms',12,GREEN,500,'end')
rect(970,371,174,97,'#fff','#d7dfe9',7)
rect(984,385,28,28,'#edf2f6',r=5); icon('network',990,391,'#67778a')
txt(1024,405,'系统默认',14,weight=600); dot(989,445); txt(999,450,'网卡可用',11,MUTED)
txt(232,622,'选择出口可切换节点；入口共用同一个出口时，将同时生效。',10,MUTED)
# Contextual bottom panel, not a permanent editing form.
rect(216,655,952,117,'#fff','#dfe4eb',7)
txt(236,684,'机场出口',14,weight=600); txt(236,709,'关联入口：机场、浏览器',11,MUTED)
txt(505,682,'当前节点',11,MUTED); field(505,692,223,'SG 新加坡01')
txt(757,682,'网卡',11,MUTED); field(757,692,187,'系统默认')
button(1056,693,91,'编辑出口')
line(236,739,1148,739,'#edf0f4'); txt(236,758,'修改当前节点将影响 2 个入口',10,BLUE)
txt(1148,758,'已生效',10,GREEN,anchor='end')
txt(216,805,'全部配置已生效',11,MUTED); txt(1168,805,'累计流量 317.3 MB',11,MUTED,anchor='end')
save('overview-e-route-map')
