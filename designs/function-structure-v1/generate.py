"""Five functional architectures expressed as desktop screens, not implementation."""
from ui import *

def nav(label,y,name='overview',selected=False,child=False):
    if selected:
        rect(9,y,182,40,'#e4e4e4',r=5); rect(10,y+9,3,22,BLUE,r=1.5)
    if not child: icon(name,27,y+12,'#32373d')
    txt(67 if child else 58,y+26,label,12 if child else 14,MUTED if child else INK,600 if selected else 400)
    if child: line(51,y+2,51,y+36,'#cfd2d6'); line(51,y+20,59,y+20,'#cfd2d6')
def section(label,y): txt(24,y,label,10,'#81858b')
def shell():
    start('#f3f3f3')
    rect(1,33,199,798,'#f3f3f3')
    rect(20,48,25,25,BLUE,r=5); path('M28 54v10q0 3 3 3h5M36 55v8m-2-2 2 2 2-2','#fff',1.5)
    txt(55,66,'Routa',16,weight=600); txt(22,807,'v0.2.0',10,MUTED)
def title(label,desc=None):
    txt(224,80,label,25,weight=600)
    if desc: txt(224,105,desc,12,MUTED)
    dot(1032,73); txt(1044,78,'内核运行中',11,MUTED); more(1161,74)
def tabs(items,x=224,y=128):
    for w,label,selected in items:
        txt(x+12,y+19,label,13,INK if selected else MUTED,600 if selected else 400)
        if selected: rect(x+12,y+31,w-24,3,BLUE,r=1.5)
        x+=w
    line(224,y+35,1176,y+35,'#dedfe2')
def tick(x,y,c=BLUE):
    rect(x,y,15,15,c,r=3); path(f'M{x+3} {y+8}l3 3 6-7','#fff',1.4)
def cellSelect(x,y,w,label): field(x,y,w,label)
def address(x,y,s): txt(x,y,s,12); copy(x+len(s)*6.8+14,y-12)
def stat(x,y,w,label,value):
    rect(x,y,w,88,'#fff',BORDER,7); txt(x+16,y+26,label,11,MUTED); txt(x+16,y+62,value,23,weight=600)

# 01 — Object ownership. Six modules, grouped as operation/resources/system.
shell()
section('运行',101)
nav('概览',112,selected=True); nav('代理入口',158,'input'); nav('程序分流',204,'apps')
section('资源',282); nav('出口',295,'exit'); nav('节点与订阅',341,'nodes'); nav('Tailscale 实例',382,child=True)
section('系统',692); nav('诊断',706,'network'); nav('设置',752,'settings')
title('概览','查看运行状态，并快速处理连接问题')
rect(224,123,952,53,'#fff',BORDER,7); dot(243,150)
txt(257,154,'所有入口均已生效',13,weight=500); txt(1157,154,'查看诊断 →',12,BLUE,anchor='end')
for x,label,value in [(224,'上传速率','0 B/s'),(465,'下载速率','444 B/s'),(706,'累计流量','317.3 MB'),(947,'代理入口','3 / 3 运行中')]: stat(x,192,229,label,value)
txt(224,322,'当前连接',17,weight=600); txt(1176,322,'管理代理入口 →',12,BLUE,anchor='end')
rect(224,342,952,272,'#fff',BORDER,7)
for x,label in [(242,'代理入口'),(431,'出口'),(637,'当前节点'),(847,'延迟'),(952,'实时流量'),(1100,'启用')]: txt(x,368,label,11,MUTED)
line(224,384,1176,384)
for i,(name,port,exit,node,delay,rate) in enumerate([('机场','7893 · SOCKS5','机场出口','SG 新加坡01','182 ms','359 B/s'),('浏览器','7894 · 混合','机场出口','SG 新加坡01','182 ms','0 B/s'),('下载器','7895 · SOCKS5','下载专用','JP 东京02','96 ms','85 B/s')]):
    y=384+i*76
    if i: line(224,y,1176,y)
    dot(245,y+27); txt(257,y+32,name,14,weight=600); txt(242,y+55,port,10,MUTED)
    txt(431,y+32,exit,13); chevron(591,y+24); txt(431,y+55,'共用 2 个入口' if i<2 else '独立出口',10,MUTED)
    txt(637,y+32,node,13); chevron(802,y+24)
    txt(847,y+32,delay,12,GREEN); txt(952,y+29,'↓ '+rate,11); txt(952,y+52,'↑ 0 B/s',10,MUTED); toggle(1100,y+23)
txt(224,658,'程序分流',15,weight=600); txt(1176,658,'管理规则 →',12,BLUE,anchor='end')
rect(224,676,952,65,'#fff',BORDER,7)
dot(244,708); txt(258,713,'组件运行中',12); txt(431,713,'3 条启用规则',12,MUTED); txt(680,713,'查看各程序的目标入口与实时连接',11,MUTED)
save('01-object-modules')

# 02 — A single connection workspace owns both ways of entering the network.
shell(); nav('连接',99,'input',True); nav('出口',151,'exit'); nav('节点资源',203,'nodes'); nav('诊断',703,'network'); nav('设置',752,'settings')
title('连接','管理端口接入与程序分流')
tabs([(145,'代理端口  3',False),(145,'程序规则  3',True)])
txt(224,204,'程序规则',17,weight=600); button(1056,181,120,'＋ 添加程序',True)
rect(224,226,952,55,'#fff',BORDER,6)
dot(244,253); txt(257,257,'分流组件运行中',12,weight=500); txt(466,257,'规则已启用',11,MUTED); txt(1157,257,'组件与故障策略 →',11,BLUE,anchor='end')
rect(224,299,952,277,'#fff',BORDER,7)
for x,label in [(242,'程序'),(459,'目标入口'),(662,'当前线路'),(881,'故障时'),(1002,'下载'),(1118,'操作')]: txt(x,326,label,11,MUTED)
line(224,341,1176,341)
for i,(name,exe,target,node,rate) in enumerate([('浏览器','chrome.exe','浏览器 · 7894','SG 新加坡01','0 B/s'),('代码编辑器','Code.exe','机场 · 7893','SG 新加坡01','359 B/s'),('下载工具','aria2c.exe','下载器 · 7895','JP 东京02','85 B/s')]):
    y=341+i*78
    if i: line(224,y,1176,y)
    txt(242,y+31,name,14,weight=600); txt(242,y+54,exe,10,MUTED)
    cellSelect(459,y+17,174,target); txt(662,y+31,node,12); txt(662,y+54,'机场出口' if i<2 else '下载专用',10,MUTED)
    txt(881,y+31,'阻断',12); txt(1002,y+31,rate,11); more(1136,y+30)
rect(224,600,952,155,'#fff',BORDER,7)
txt(242,631,'入口详情：浏览器',15,weight=600); txt(1157,631,'编辑入口 →',12,BLUE,anchor='end')
address(242,668,'127.0.0.1:7894'); txt(478,668,'HTTP + SOCKS5',12,MUTED); txt(691,668,'仅本机访问',12,MUTED)
line(242,690,1158,690); txt(242,720,'出口',11,MUTED); cellSelect(307,700,219,'机场出口')
txt(552,721,'与「机场」入口共用此出口',11,MUTED); txt(1157,721,'前往出口配置 →',12,BLUE,anchor='end')
save('02-unified-connections')

# 03 — Entry method first. Local applications and proxy ports are peer workflows.
shell(); nav('总览',98,'overview'); section('接入方式',170)
nav('本机程序',183,'apps',True); nav('代理端口',229,'input')
section('网络资源',312); nav('出口线路',325,'exit'); nav('节点与订阅',371,'nodes'); nav('Tailscale 实例',412,child=True)
nav('诊断与设置',752,'settings')
title('本机程序','为指定程序选择代理入口，并查看实际连接')
rect(224,126,952,54,'#fff',BORDER,7); dot(244,153); txt(258,158,'进程分流已启用',12,weight=500); toggle(418,144)
txt(1156,158,'组件管理 →',11,BLUE,anchor='end')
txt(224,220,'程序',17,weight=600); button(722,197,120,'＋ 添加程序',True)
for i,(name,exe,entry,node,rate) in enumerate([('浏览器','chrome.exe','浏览器 · 7894','SG 新加坡01','0 B/s'),('代码编辑器','Code.exe','机场 · 7893','SG 新加坡01','359 B/s'),('下载工具','aria2c.exe','下载器 · 7895','JP 东京02','85 B/s')]):
    y=240+i*158
    rect(224,y,618,140,'#fff',BORDER,7)
    rect(242,y+18,34,34,'#edf3fa',r=7); icon('apps',251,y+27,'#6b86a2')
    txt(289,y+35,name,14,weight=600); txt(289,y+56,exe,10,MUTED); txt(815,y+35,'↓ '+rate,11,anchor='end')
    line(242,y+72,824,y+72); txt(242,y+101,'目标入口',11,MUTED); cellSelect(324,y+81,194,entry)
    txt(546,y+102,'→',12,MUTED); txt(573,y+102,node,12); more(809,y+101)
rect(858,240,318,260,'#fff',BORDER,7)
txt(878,270,'运行与保护',14,weight=600)
txt(878,307,'启用规则',11,MUTED); txt(1155,307,'3 条',13,anchor='end')
txt(878,347,'组件故障时',11,MUTED); txt(1155,347,'按程序规则处理',12,anchor='end')
txt(878,388,'退出 Routa 后',11,MUTED); txt(1155,388,'阻断受管程序',12,anchor='end')
line(878,411,1155,411); txt(878,442,'规则设置',12,BLUE); txt(878,476,'查看组件诊断 →',11,BLUE)
rect(858,516,318,196,'#fff',BORDER,7)
txt(878,547,'其他设备接入',14,weight=600)
txt(878,580,'设备或软件也可自行设置代理，',11,MUTED); txt(878,601,'连接 Routa 开放的端口。',11,MUTED)
txt(878,640,'3 个代理端口运行中',12,GREEN); txt(878,686,'管理代理端口 →',12,BLUE)
save('03-access-methods')

# 04 — Network resources consolidated; lifecycle-specific subpages stay distinct.
shell(); nav('总览',98,'overview'); nav('连接管理',150,'input'); nav('网络资源',202,'nodes',True)
nav('诊断',704,'network'); nav('设置',752,'settings')
title('网络资源','集中管理出口、节点来源与专用网络实例')
tabs([(106,'出口  2',True),(106,'节点  24',False),(106,'订阅  1',False),(146,'Tailscale  1',False),(126,'网卡状态',False)])
txt(224,204,'出口',17,weight=600); button(1056,181,120,'＋ 新建出口',True)
rect(224,225,260,531,'#fff',BORDER,7)
rect(233,237,242,81,'#edf4fc',r=5); rect(234,255,3,44,BLUE,r=1.5)
txt(250,267,'机场出口',15,weight=600); txt(250,293,'手动选择 · 2 个入口使用',10,MUTED)
txt(250,353,'下载专用',15,weight=600); txt(250,379,'固定节点 · 1 个入口使用',10,MUTED)
txt(244,732,'共 2 个出口',11,MUTED)
rect(500,225,676,531,'#fff',BORDER,7)
txt(521,263,'机场出口',20,weight=600); badge(632,244,'正常',GREEN,'#edf5ee'); txt(1157,263,'编辑 →',12,BLUE,anchor='end')
line(500,284,1176,284)
txt(521,314,'策略',11,MUTED); txt(700,314,'手动选择',13)
txt(521,352,'当前节点',11,MUTED); field(700,329,330,'SG 新加坡01')
txt(521,392,'出口网卡',11,MUTED); txt(700,392,'系统默认',13)
line(521,414,1156,414)
txt(521,444,'候选节点',14,weight=600); txt(1156,444,'管理节点资源 →',11,BLUE,anchor='end')
for i,(name,delay) in enumerate([('SG 新加坡01','182 ms'),('JP 东京02','96 ms'),('HK 香港03','48 ms')]):
    y=479+i*42
    if i==0: dot(530,y-4,BLUE,4)
    txt(546,y,name,12); txt(842,y,'来自：机场订阅',10,MUTED); txt(1156,y,delay,12,GREEN,anchor='end')
line(521,583,1156,583); txt(521,615,'使用此出口的入口',14,weight=600)
badge(521,634,'机场 · 7893'); badge(653,634,'浏览器 · 7894')
txt(521,710,'节点切换影响这两个入口。',11,MUTED); txt(1156,710,'查看连接 →',12,BLUE,anchor='end')
save('04-resource-center')

# 05 — Operational shortcuts and persistent configuration have separate modes.
shell()
rect(9,93,182,35,'#e6e6e6',r=5); rect(100,96,88,29,'#fff',r=4)
txt(55,116,'日常使用',11,MUTED,anchor='middle'); txt(143,116,'配置管理',11,INK,600,'middle')
section('接入配置',165); nav('代理端口',177,'input'); nav('程序规则',223,'apps')
section('路由配置',306); nav('出口策略',318,'exit',True)
section('节点资源',400); nav('节点与订阅',413,'nodes'); nav('Tailscale 实例',459,'globe')
nav('系统与诊断',752,'settings')
txt(224,66,'配置管理  /  出口策略',11,MUTED); txt(224,103,'机场出口',25,weight=600)
button(1056,76,120,'保存更改',True)
rect(224,125,952,50,'#eaf3fc',r=6); txt(242,156,'此出口由「机场」「浏览器」两个入口共用，修改后将一起生效。',12,BLUE)
rect(224,197,566,530,'#fff',BORDER,7)
txt(244,230,'策略与节点',16,weight=600); line(224,250,790,250)
txt(244,284,'选择策略',12,MUTED); field(244,296,524,'手动选择')
txt(244,372,'候选节点',14,weight=600); txt(768,371,'从资源添加 →',11,BLUE,anchor='end')
for i,(name,src,delay) in enumerate([('SG 新加坡01','机场订阅','182 ms'),('JP 东京02','机场订阅','96 ms'),('HK 香港03','机场订阅','48 ms')]):
    y=394+i*72
    if i: line(244,y-7,768,y-7,'#edf0f3')
    tick(244,y+13); txt(273,y+29,name,13,weight=500); txt(273,y+51,src,10,MUTED); txt(768,y+30,delay,12,GREEN,anchor='end')
txt(244,661,'当前选中',11,MUTED); field(346,639,422,'SG 新加坡01')
rect(806,197,370,260,'#fff',BORDER,7)
txt(826,230,'网卡与运行行为',16,weight=600); line(806,250,1176,250)
txt(826,283,'出口网卡',11,MUTED); field(826,296,330,'系统默认')
txt(826,374,'节点或网卡不可用时',12,weight=500)
txt(826,402,'关联入口停用，恢复后自动恢复。',11,MUTED)
rect(806,473,370,254,'#fff',BORDER,7)
txt(826,507,'关联入口',16,weight=600)
for i,(name,port) in enumerate([('机场','7893 · SOCKS5'),('浏览器','7894 · 混合')]):
    y=543+i*62; dot(831,y+5); txt(843,y+10,name,13,weight=500); txt(843,y+32,port,10,MUTED); txt(1156,y+12,'查看',11,BLUE,anchor='end')
txt(224,773,'日常使用中可快速切节点、启停入口；完整配置集中在此模式。',11,MUTED)
save('05-use-config-modes')
