"""Alternative functional groupings; monitoring retains approved secondary prominence."""
from monitor import *

def monitoring():
    section('运行查看',483); nav('实时连接',496,'connections'); nav('日志',540,'log'); nav('设置',752,'settings')
def footer():
    line(224,743,1176,743,'#e0e2e5')
    txt(224,775,'12 条活跃连接',11,MUTED); txt(356,775,'查看连接 →',11,BLUE)
    dot(886,771,'#b17b18',2.5); txt(897,775,'日志有 1 条警告',11,MUTED); txt(1176,775,'查看日志 →',11,BLUE,anchor='end')
def dataNav(mode):
    shell()
    if mode=='access':
        nav('概览',99); nav('接入管理',147,'input',True)
        section('资源',246); nav('出口',259,'exit'); nav('节点与订阅',303,'nodes'); nav('Tailscale 实例',344,child=True)
    elif mode=='objects':
        nav('概览',99,selected=True); section('接入方式',179)
        nav('本机程序',192,'apps'); nav('代理端口',236,'input')
        section('资源',318); nav('出口',331,'exit'); nav('节点与订阅',375,'nodes'); nav('Tailscale 实例',416,child=True)
    elif mode=='workbench':
        nav('连接工作台',99,'overview',True); nav('程序规则',147,'apps')
        section('资源',246); nav('节点与订阅',259,'nodes'); nav('Tailscale 实例',300,child=True)
    else:
        nav('概览',99); nav('连接配置',147,'input',True)
        section('节点资源',246); nav('节点与订阅',259,'nodes'); nav('Tailscale 实例',300,child=True)
    monitoring()

# 01 — Ports and process rules share one workspace; exits stay independently managed.
dataNav('access'); title('接入管理','管理代理端口，以及使用这些端口的程序')
tabs([(157,'代理端口  3',True),(157,'程序规则  3',False)])
txt(224,201,'代理端口',16,weight=600); button(1056,179,120,'＋ 新建端口',True)
rect(224,222,952,260,'#fff',BORDER,7)
for x,label in [(242,'入口'),(447,'出口'),(663,'当前节点'),(889,'关联规则'),(1036,'运行状态')]: txt(x,248,label,11,MUTED)
line(224,264,1176,264)
for i,(name,port,exit,node,proc) in enumerate([('机场','7893 · SOCKS5','机场出口','SG 新加坡01','Code.exe'),('浏览器','7894 · 混合','机场出口','SG 新加坡01','chrome.exe'),('下载器','7895 · SOCKS5','下载专用','JP 东京02','aria2c.exe')]):
    y=264+i*72
    if i==0: rect(225,y,950,72,'#f0f6fe')
    if i: line(224,y,1176,y)
    txt(242,y+29,name,14,weight=600); txt(242,y+52,port,10,MUTED)
    field(447,y+17,190,exit); txt(663,y+29,node,12); txt(663,y+51,'182 ms' if i<2 else '96 ms',11,GREEN)
    txt(889,y+30,proc,11); toggle(1036,y+22); more(1153,y+31)
rect(224,501,952,220,'#fff',BORDER,7)
txt(242,532,'机场 · 入口详情',15,weight=600); txt(1156,532,'编辑端口设置 →',12,BLUE,anchor='end')
address(242,571,'127.0.0.1:7893'); txt(530,571,'SOCKS5 · 仅本机',12,MUTED)
line(242,591,1157,591)
txt(242,623,'使用此入口的程序',12,weight=500); txt(1157,623,'管理程序规则 →',11,BLUE,anchor='end')
badge(242,638,'Code.exe'); txt(397,654,'故障时阻断 · 规则已启用',11,MUTED)
txt(242,698,'当前出口：机场出口 · 与「浏览器」入口共用',11,MUTED); txt(1157,698,'管理出口 →',11,BLUE,anchor='end')
footer(); save('01-unified-access')

# 02 — Separate entry-method workflows; overview gives both an equal summary.
dataNav('objects'); title('概览','按程序或端口查看当前网络去向')
rect(224,124,952,75,'#fff',BORDER,7)
for x,label,value in [(242,'上传','0 B/s'),(465,'下载','444 B/s'),(706,'累计流量','317.3 MB'),(947,'入口状态','3 / 3 运行中')]:
    txt(x,148,label,10,MUTED); txt(x,177,value,19,weight=600)
for x in [444,685,926]: line(x,140,x,184,'#e9ecf0')
txt(224,240,'本机程序',17,weight=600); txt(677,240,'管理 →',11,BLUE,anchor='end')
txt(699,240,'代理端口',17,weight=600); txt(1176,240,'管理 →',11,BLUE,anchor='end')
rect(224,260,453,304,'#fff',BORDER,7); rect(699,260,477,304,'#fff',BORDER,7)
for i,(name,exe,target,rate) in enumerate([('代码编辑器','Code.exe','机场 :7893','359 B/s'),('浏览器','chrome.exe','浏览器 :7894','0 B/s'),('下载工具','aria2c.exe','下载器 :7895','85 B/s')]):
    y=279+i*93
    if i: line(242,y-11,659,y-11)
    rect(242,y+4,31,31,'#edf3fa',r=6); icon('apps',250,y+12,'#6b86a2')
    txt(287,y+18,name,13,weight=600); txt(287,y+39,exe,10,MUTED); txt(659,y+18,'↓ '+rate,11,anchor='end')
    txt(287,y+65,'→ '+target,11,BLUE)
for i,(name,port,node,delay) in enumerate([('机场','7893 · SOCKS5','SG 新加坡01','182 ms'),('浏览器','7894 · 混合','SG 新加坡01','182 ms'),('下载器','7895 · SOCKS5','JP 东京02','96 ms')]):
    y=279+i*93
    if i: line(717,y-11,1158,y-11)
    dot(721,y+12); txt(734,y+18,name,13,weight=600); txt(734,y+40,port,10,MUTED); toggle(1119,y+4)
    txt(734,y+65,node,12); txt(1158,y+65,delay,11,GREEN,anchor='end')
txt(224,607,'出口线路',16,weight=600); txt(1176,607,'管理出口 →',11,BLUE,anchor='end')
for x,name,node,count in [(224,'机场出口','SG 新加坡01','2 个入口共用'),(706,'下载专用','JP 东京02','1 个入口使用')]:
    rect(x,626,470,94,'#fff',BORDER,7)
    icon('exit',x+18,648,BLUE); txt(x+46,662,name,14,weight=600); txt(x+451,662,count,10,MUTED,anchor='end')
    txt(x+18,694,node+' · 系统默认网卡',11,MUTED)
footer(); save('02-entry-method-overview')

# 03 — Inbound/outbound pairing is the workspace; node resources and app rules stay separate.
dataNav('workbench'); title('连接工作台','给入口接上出口，按出口统一切换线路')
tabs([(145,'按入口  3',False),(145,'按出口  2',True)])
txt(224,202,'3 个入口运行中',12,GREEN); txt(426,202,'↓ 444 B/s',13); txt(608,202,'↑ 0 B/s',12,MUTED)
button(1056,179,120,'＋ 新建',True)
for x,w,name,node,code,delay in [(224,567,'机场出口','新加坡 01','SG','182 ms'),(807,369,'下载专用','东京 02','JP','96 ms')]:
    rect(x,225,w,405,'#fff',BORDER,7)
    rect(x+1,226,w-2,154,'#f7faff',r=6); rect(x+1,363,w-2,17,'#f7faff')
    icon('exit',x+18,248,BLUE); txt(x+43,264,name,16,weight=600); more(x+w-21,258)
    txt(x+18,308,code+' '+node,23,weight=600); txt(x+18,340,'手动选择 · '+delay,11,MUTED)
    txt(x+w-19,340,'切换节点 ▾',12,BLUE,anchor='end')
    txt(x+18,406,'接入此出口的入口',11,MUTED)
    if x==224:
        badge(x+190,389,'共用 2 个')
        for y,n,p,r in [(431,'机场','7893 · SOCKS5','359 B/s'),(518,'浏览器','7894 · 混合','0 B/s')]:
            if y>431: line(x+18,y-10,x+w-18,y-10)
            dot(x+23,y+17); txt(x+36,y+23,n,14,weight=600); txt(x+36,y+46,p,10,MUTED)
            txt(x+w-109,y+23,'↓ '+r,11,anchor='end'); toggle(x+w-89,y+8); more(x+w-24,y+17)
    else:
        dot(x+23,448); txt(x+36,454,'下载器',14,weight=600); txt(x+36,477,'7895 · SOCKS5',10,MUTED); toggle(x+w-88,439); more(x+w-23,449)
        txt(x+18,594,'独立出口 · 仅影响下载器入口',11,MUTED)
rect(224,650,952,69,'#fff',BORDER,7)
txt(242,680,'程序规则',14,weight=600); txt(393,680,'3 条规则已启用',12,MUTED); txt(242,703,'程序通过选定入口使用以上线路',10,MUTED); txt(1157,690,'管理程序规则 →',12,BLUE,anchor='end')
footer(); save('03-routing-workbench')

# 04 — One configuration center contains all three connected domain objects.
dataNav('configuration'); title('连接配置','集中管理代理入口、程序规则和出口之间的关联')
tabs([(157,'代理入口  3',True),(157,'程序规则  3',False),(131,'出口  2',False)])
txt(224,202,'代理入口',16,weight=600); button(1056,179,120,'＋ 新建入口',True)
rect(224,222,952,260,'#fff',BORDER,7)
for x,label in [(242,'入口 / 端口'),(436,'协议'),(586,'绑定出口'),(809,'当前节点'),(1083,'启用')]: txt(x,248,label,11,MUTED)
line(224,264,1176,264)
for i,(name,port,proto,exit,node) in enumerate([('机场','7893','SOCKS5','机场出口','SG 新加坡01'),('浏览器','7894','HTTP + SOCKS5','机场出口','SG 新加坡01'),('下载器','7895','SOCKS5','下载专用','JP 东京02')]):
    y=264+i*72
    if not i: rect(225,y,950,72,'#f0f6fe')
    if i: line(224,y,1176,y)
    dot(245,y+25); txt(258,y+31,name,14,weight=600); txt(242,y+53,port,10,MUTED)
    txt(436,y+31,proto,11); field(586,y+16,198,exit); txt(809,y+31,node,12); toggle(1083,y+21); more(1153,y+30)
rect(224,502,453,218,'#fff',BORDER,7); rect(693,502,483,218,'#fff',BORDER,7)
txt(242,535,'机场 · 使用它的程序',14,weight=600); txt(659,535,'程序规则 →',11,BLUE,anchor='end')
rect(242,555,35,35,'#edf3fa',r=7); icon('apps',251,564,'#6b86a2'); txt(291,575,'Code.exe',13,weight=500)
txt(242,620,'目标入口',11,MUTED); txt(399,620,'机场 · 7893',12)
txt(242,653,'故障处理',11,MUTED); txt(399,653,'阻断',12)
txt(242,693,'规则已启用',11,GREEN)
txt(711,535,'机场 · 当前出口',14,weight=600); txt(1158,535,'出口配置 →',11,BLUE,anchor='end')
txt(711,575,'机场出口',16,weight=600); badge(826,558,'共用 2 个入口')
txt(711,618,'节点',11,MUTED); txt(829,618,'SG 新加坡01',12)
txt(711,651,'网卡',11,MUTED); txt(829,651,'系统默认',12)
txt(711,693,'同时被「浏览器」入口使用',11,MUTED)
footer(); save('04-configuration-center')
