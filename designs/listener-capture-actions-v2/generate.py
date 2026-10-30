"""Four listener-local capture-control patterns, without changing the app."""
from monitor import *

DATA=[('机场','7893 · 混合','机场出口','SG 新加坡01','182 ms','359 B/s'),('浏览器','7894 · 混合','机场出口','SG 新加坡01','182 ms','0 B/s'),('下载器','7895 · SOCKS5','下载专用','JP 东京02','96 ms','85 B/s')]
def off(x,y): rect(x,y,32,17,'#fff','#888e97',8.5); dot(x+8.5,y+8.5,'#868d96',5.5)
def shellOverview():
    shell(); nav('概览',99,selected=True); nav('代理入口',147,'input'); nav('程序分流',195,'apps')
    section('资源',276); nav('出口',289,'exit'); nav('节点与订阅',333,'nodes'); nav('Tailscale 实例',374,child=True)
    section('运行查看',483); nav('实时连接',496,'connections'); nav('日志',540,'log'); nav('设置',752,'settings')
    title('概览')
    for x,label,value in [(224,'上传速率','0 B/s'),(465,'下载速率','444 B/s'),(706,'累计流量','317.3 MB'),(947,'代理入口','3 / 3 运行中')]: stat(x,108,229,label,value)
    txt(224,242,'监听器',17,weight=600); txt(297,241,'3 个运行中',11,MUTED)
def footer():
    line(224,751,1176,751,'#e0e2e5'); txt(224,781,'12 条活跃连接',11,MUTED); txt(356,781,'查看连接 →',11,BLUE)
    dot(886,777,'#b17b18',2.5); txt(897,781,'日志有 1 条警告',11,MUTED); txt(1176,781,'查看日志 →',11,BLUE,anchor='end')
def program(y=666):
    txt(224,y-19,'程序分流',16,weight=600); txt(1176,y-19,'管理程序规则 →',12,BLUE,anchor='end')
    rect(224,y,952,56,'#fff',BORDER,7); dot(245,y+28); txt(259,y+33,'分流组件运行中',12); txt(473,y+33,'3 条启用规则',12,MUTED); txt(789,y+33,'按程序选择代理入口',11,MUTED)
def fullTable(purpose=False,selected=None,context=False):
    rect(224,280,952,360 if context else 308,'#fff',BORDER,7)
    if purpose: cols=[(242,'名称 / 端口'),(420,'出口'),(604,'当前节点'),(799,'网卡'),(905,'实时流量'),(1008,'接入'),(1114,'启用')]
    else: cols=[(242,'名称 / 端口'),(430,'出口'),(629,'当前节点'),(835,'网卡'),(946,'实时流量'),(1100,'操作')]
    for x,label in cols: txt(x,307,label,11,MUTED)
    line(224,322,1176,322)
    for i,(name,port,exit,node,delay,rate) in enumerate(DATA):
        y=322+i*88
        if i==selected: rect(225,y,950,88,'#eef5fe')
        if i: line(224,y,1176,y)
        dot(245,y+28); txt(257,y+33,name,14,weight=600); txt(242,y+58,port,10,MUTED)
        if not purpose and i==1: badge(310,y+14,'系统代理')
        ex,nx,nw,ix,rx=(420,604,174,799,905) if purpose else (430,629,184,835,946)
        field(ex,y+15,165 if purpose else 175,exit); txt(ex,y+67,'共用 2 个入口' if i<2 else '独立出口',10,MUTED)
        field(nx,y+15,nw,node); txt(nx,y+67,delay,11,GREEN)
        txt(ix,y+35,'系统默认',12); txt(rx,y+29,'↑ 0 B/s',10,MUTED); txt(rx,y+52,'↓ '+rate,11)
        if purpose:
            rect(1006,y+15,88,28,'#fff','#d9dde3',4); txt(1017,y+34,'接入',11); chevron(1075,y+25)
            txt(1008,y+66,'系统代理' if i==1 else '手动 / 程序',10,BLUE if i==1 else MUTED)
            toggle(1116,y+23); more(1160,y+32)
        else: toggle(1100,y+23); more(1153,y+32)

# 01: a dedicated compact dropdown contains capture uses rather than a whole system-proxy column.
shellOverview(); button(1056,219,120,'＋ 新建监听器',True)
txt(224,267,'系统代理：浏览器 :7894  ·  TUN：关闭',11,MUTED)
fullTable(True,1); program(); footer()
rect(805,466,371,235,'#fff','#d8dde5',7,'filter="url(#shadow)"')
txt(821,493,'浏览器 · 接入用途',11,MUTED)
path('M821 519l3 3 6-7',BLUE,1.4); txt(843,522,'当前系统代理',12,BLUE); txt(1157,522,'关闭',11,BLUE,anchor='end')
line(821,541,1158,541,'#edf0f3')
txt(821,570,'使用同一出口开启 TUN…',12)
txt(821,595,'默认出口：机场出口（2 个入口共用）',10,MUTED)
line(821,613,1158,613,'#edf0f3')
txt(821,641,'复制代理地址',12); txt(821,679,'接入设置 →',11,BLUE)
save('01-row-capture-dropdown')

# 02: one listener-section command bar owns the global settings, rows only show roles.
shellOverview()
rect(626,220,237,32,'#fff','#dedfe3',4); txt(638,241,'系统代理 · 浏览器 :7894',11); chevron(843,233)
rect(875,220,160,32,'#fff','#dedfe3',4); txt(887,241,'TUN · 关闭',11,MUTED); chevron(1015,233)
button(1047,220,129,'＋ 新建监听器',True)
fullTable(); program(); footer()
save('02-listener-section-toolbar')

# 03: selecting one row reveals an explicit contextual command bar beneath the list.
shellOverview(); button(1056,219,120,'＋ 新建监听器',True)
txt(224,267,'系统代理：浏览器 :7894  ·  TUN：关闭',11,MUTED)
fullTable(selected=0,context=True)
line(224,586,1176,586)
rect(225,587,950,52,'#fafbfd',r=0)
txt(242,618,'已选：机场 :7893',12,weight=500)
txt(455,618,'设为系统代理',12,BLUE)
txt(627,618,'使用出口开启 TUN…',12,BLUE)
txt(1156,618,'出口：机场出口',10,MUTED,anchor='end')
rect(224,664,952,58,'#fff',BORDER,7)
dot(245,693); txt(259,698,'程序分流运行中',12); txt(497,698,'3 条启用规则',12,MUTED); txt(1156,698,'管理程序规则 →',12,BLUE,anchor='end')
footer(); save('03-selected-listener-commandbar')

# 04: the detail pane contains the controls, keeping every table row concise.
shellOverview(); button(728,219,120,'＋ 新建监听器',True)
rect(224,267,624,333,'#fff',BORDER,7)
for x,label in [(242,'名称 / 端口'),(415,'出口'),(604,'当前节点'),(798,'启用')]: txt(x,294,label,11,MUTED)
line(224,309,848,309)
for i,(name,port,exit,node,delay,rate) in enumerate(DATA):
    y=309+i*96
    if i==1: rect(225,y,622,96,'#eef5fe')
    if i: line(224,y,848,y)
    dot(245,y+28); txt(257,y+33,name,14,weight=600); txt(242,y+59,port,10,MUTED)
    if i==1: txt(242,y+80,'系统代理使用此入口',9,BLUE)
    field(415,y+15,170,exit); txt(415,y+67,'共用 2 个入口' if i<2 else '独立出口',10,MUTED)
    field(604,y+15,166,node); txt(604,y+67,delay,11,GREEN); toggle(798,y+24)
rect(864,267,312,455,'#fff',BORDER,7)
txt(884,300,'浏览器',18,weight=600); txt(1156,300,'编辑 →',11,BLUE,anchor='end')
address(884,331,'127.0.0.1:7894')
line(864,352,1176,352)
txt(884,385,'系统代理',14,weight=600); toggle(1123,370)
txt(884,410,'当前使用此 Listener',10,MUTED)
line(884,438,1156,438,'#edf0f3')
txt(884,468,'TUN · 使用此出口',14,weight=600); off(1123,453)
txt(884,495,'当前未开启',10,MUTED); badge(884,509,'机场出口')
txt(884,553,'与「机场」入口共用此出口',10,MUTED)
txt(884,582,'开启前检查与进程分流的接管范围',10,'#956b20')
txt(884,615,'TUN 设置 →',11,BLUE)
line(884,637,1156,637,'#edf0f3')
txt(884,670,'查看实时连接 →',11,BLUE); txt(884,703,'查看相关日志 →',11,BLUE)
rect(224,623,624,99,'#fff',BORDER,7)
txt(242,653,'程序分流运行中',14,weight=600); txt(242,679,'3 条规则已启用',11,MUTED); txt(829,698,'管理程序规则 →',12,BLUE,anchor='end')
footer(); save('04-listener-detail-pane')
