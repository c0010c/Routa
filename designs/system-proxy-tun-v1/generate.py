"""Five capture-mode concepts. Coexistence is deliberately not asserted as verified."""
from monitor import *

def off(x,y):
    rect(x,y,32,17,'#fff','#888e97',8.5); dot(x+8.5,y+8.5,'#868d96',5.5)
def monitoring():
    section('运行查看',530); nav('实时连接',543,'connections'); nav('日志',587,'log'); nav('设置',752,'settings')
def footer():
    line(224,743,1176,743,'#e0e2e5'); txt(224,775,'12 条活跃连接',11,MUTED); txt(356,775,'查看连接 →',11,BLUE)
    dot(886,771,'#b17b18',2.5); txt(897,775,'日志有 1 条警告',11,MUTED); txt(1176,775,'查看日志 →',11,BLUE,anchor='end')
def navigation(mode):
    shell()
    if mode=='home':
        nav('概览',99,selected=True); nav('代理入口',147,'input'); nav('程序分流',195,'apps'); nav('系统与 TUN',243,'network')
        section('资源',329); nav('出口',342,'exit'); nav('节点与订阅',386,'nodes'); nav('Tailscale 实例',427,child=True)
    elif mode=='access':
        nav('概览',99); nav('流量接入',147,'input',True)
        section('资源',246); nav('出口',259,'exit'); nav('节点与订阅',303,'nodes'); nav('Tailscale 实例',344,child=True)
    elif mode=='takeover':
        nav('概览',99); nav('代理入口',147,'input'); nav('网络接管',195,'network',True)
        section('资源',281); nav('出口',294,'exit'); nav('节点与订阅',338,'nodes'); nav('Tailscale 实例',379,child=True)
    elif mode=='ports':
        nav('概览',99); nav('代理入口',147,'input',True); nav('程序分流',195,'apps'); nav('TUN 接管',243,'network')
        section('资源',329); nav('出口',342,'exit'); nav('节点与订阅',386,'nodes'); nav('Tailscale 实例',427,child=True)
    else:
        nav('接管工作台',99,'overview',True); nav('端口与规则',147,'input')
        section('资源',246); nav('出口',259,'exit'); nav('节点与订阅',303,'nodes'); nav('Tailscale 实例',344,child=True)
    monitoring()
def checkhint(x,y,w):
    rect(x,y,w,37,'#fff7e7',r=5); txt(x+12,y+24,'开启 TUN 前，检查与进程分流的接管范围。',11,'#956b20')

PORTS=[('机场','7893','混合','机场出口','SG 新加坡01'),('浏览器','7894','混合','机场出口','SG 新加坡01'),('下载器','7895','SOCKS5','下载专用','JP 东京02')]

# 01 — Familiar overview with three capture controls at the top.
navigation('home'); title('概览','选择流量如何进入 Routa，再配置各入口的出口')
for x,label,kind in [(224,'系统代理','system'),(546,'TUN 接管','tun'),(868,'进程分流','process')]:
    rect(x,125,308,171,'#fff',BORDER,7)
    txt(x+16,154,label,14,weight=600)
    if kind=='process': toggle(x+259,139)
    else: off(x+259,139)
    if kind=='system':
        txt(x+16,182,'目标代理端口',10,MUTED); field(x+16,192,276,'浏览器 · 7894（混合）')
        txt(x+16,255,'已关闭 · 面向遵循系统设置的应用',10,MUTED); txt(x+292,281,'配置 →',11,BLUE,anchor='end')
    elif kind=='tun':
        txt(x+16,182,'默认出口',10,MUTED); field(x+16,192,276,'机场出口')
        txt(x+16,255,'已关闭 · 开启前检查接管范围',10,'#956b20'); txt(x+292,281,'配置并开启 →',11,BLUE,anchor='end')
    else:
        txt(x+16,191,'3 条规则已启用',21,weight=600); txt(x+16,221,'指定程序 → 代理端口 → 出口',10,MUTED)
        txt(x+16,255,'Code.exe、chrome.exe、aria2c.exe',10,MUTED); txt(x+292,281,'管理规则 →',11,BLUE,anchor='end')
txt(224,338,'代理入口',17,weight=600); txt(1176,338,'＋ 新建入口',12,BLUE,anchor='end')
rect(224,358,952,263,'#fff',BORDER,7)
for x,label in [(242,'入口 / 端口'),(441,'出口'),(655,'当前节点'),(881,'使用方式'),(1080,'启用')]: txt(x,384,label,11,MUTED)
line(224,400,1176,400)
for i,(name,port,proto,exit,node) in enumerate(PORTS):
    y=400+i*73
    if i: line(224,y,1176,y)
    dot(245,y+26); txt(257,y+31,name,14,weight=600); txt(242,y+53,port+' · '+proto,10,MUTED)
    field(441,y+17,190,exit); txt(655,y+31,node,12); txt(655,y+53,'182 ms' if i<2 else '96 ms',10,GREEN)
    txt(881,y+31,'系统代理目标' if i==1 else '程序 / 手动接入',11,BLUE if i==1 else MUTED); txt(881,y+53,'系统代理当前关闭' if i==1 else '端口运行中',9,MUTED)
    toggle(1080,y+22); more(1153,y+30)
rect(224,641,952,77,'#fff',BORDER,7)
txt(242,669,'下载 444 B/s',15,weight=500); txt(463,669,'上传 0 B/s',13,MUTED); txt(685,669,'3 / 3 入口运行中',13,GREEN)
txt(242,699,'TUN 使用默认出口；系统代理与进程分流选择代理端口。',10,MUTED)
footer(); save('01-overview-capture-controls')

# 02 — Capture methods are sibling tabs, with a clear port eligibility table.
navigation('access'); title('流量接入','统一管理端口、系统代理、TUN 与进程分流')
tabs([(146,'代理端口  3',True),(146,'系统代理',False),(129,'TUN',False),(157,'进程分流  3',False)])
txt(224,202,'代理端口',17,weight=600); button(1056,179,120,'＋ 新建端口',True)
rect(224,223,952,268,'#fff',BORDER,7)
for x,label in [(242,'名称 / 端口'),(442,'协议'),(586,'出口'),(807,'系统代理'),(1090,'操作')]: txt(x,250,label,11,MUTED)
line(224,266,1176,266)
for i,(name,port,proto,exit,node) in enumerate(PORTS):
    y=266+i*74
    if i==1: rect(225,y,950,74,'#f0f6fe')
    if i: line(224,y,1176,y)
    txt(242,y+30,name,14,weight=600); txt(242,y+54,port,10,MUTED); txt(442,y+30,proto,12)
    field(586,y+17,194,exit)
    if i==1: badge(807,y+17,'已选为目标'); txt(807,y+57,'系统代理尚未开启',10,MUTED)
    elif i==0: txt(807,y+31,'设为系统代理',12,BLUE)
    else: txt(807,y+31,'需 HTTP / 混合端口',11,'#8c929a')
    toggle(1090,y+22); more(1153,y+30)
rect(224,511,952,209,'#fff',BORDER,7)
txt(242,542,'浏览器 · 系统代理目标',15,weight=600); txt(1157,542,'系统代理设置 →',12,BLUE,anchor='end')
address(242,580,'127.0.0.1:7894'); txt(513,580,'HTTP + SOCKS5',12,MUTED); txt(847,580,'当前状态：关闭',12,MUTED)
line(242,605,1157,605)
txt(242,637,'TUN 独立配置',13,weight=600); txt(242,662,'默认出口：机场出口',11,MUTED)
txt(242,696,'开启前检查与进程分流的接管范围。',11,'#956b20'); txt(1157,678,'配置 TUN →',12,BLUE,anchor='end')
footer(); save('02-capture-method-center')

# 03 — Dedicated Windows interception section, ports remain regular inbound objects.
navigation('takeover'); title('网络接管','管理 Windows 系统代理、TUN 和程序分流')
tabs([(157,'系统代理',False),(136,'TUN',True),(157,'进程分流',False)])
rect(224,185,952,60,'#fff',BORDER,7)
txt(242,221,'TUN 虚拟入口',15,weight=600); txt(485,221,'当前未开启',12,MUTED); off(1122,205)
rect(224,265,597,455,'#fff',BORDER,7)
txt(244,299,'路由与接管',16,weight=600); txt(244,337,'默认出口',11,MUTED); field(244,350,557,'机场出口')
txt(244,414,'该出口也被「机场」「浏览器」端口使用。',11,MUTED)
txt(244,438,'更换出口节点时，所有关联入口一同受影响。',11,MUTED)
line(244,461,801,461)
txt(244,494,'路由设置',13,weight=500); txt(801,494,'配置 →',11,BLUE,anchor='end')
line(244,514,801,514)
txt(244,547,'DNS 设置',13,weight=500); txt(801,547,'配置 →',11,BLUE,anchor='end')
checkhint(244,578,557)
button(244,648,557,'检查并开启 TUN',True)
rect(837,265,339,455,'#fff',BORDER,7)
txt(857,299,'当前接管情况',16,weight=600)
for y,label,value,c in [(346,'系统代理','已关闭',MUTED),(406,'进程分流','3 条规则已启用',GREEN),(466,'代理端口','3 个运行中',GREEN)]:
    txt(857,y,label,11,MUTED); txt(1156,y+25,value,13,c,500,'end')
line(857,519,1156,519)
txt(857,552,'开启前检查',13,weight=600)
txt(857,583,'重复接管、回环与路由范围',11,MUTED)
txt(857,610,'与进程分流的共存需验证',11,'#956b20')
txt(857,687,'查看程序规则 →',12,BLUE)
footer(); save('03-dedicated-network-takeover')

# 04 — Port-centric: assigning the single system-proxy target is a row operation.
navigation('ports'); title('代理入口','设置每个端口的出口，也可选作 Windows 系统代理')
rect(224,123,952,57,'#fff',BORDER,7)
txt(242,157,'系统代理',13,weight=600); txt(359,157,'已关闭',11,MUTED); txt(488,157,'目标：浏览器 · 7894',12); button(1066,135,90,'开启')
txt(224,220,'代理端口',17,weight=600); button(1056,197,120,'＋ 新建入口',True)
rect(224,241,952,268,'#fff',BORDER,7)
for x,label in [(242,'名称 / 协议'),(444,'端口'),(578,'出口 / 当前节点'),(861,'系统代理'),(1095,'操作')]: txt(x,268,label,11,MUTED)
line(224,284,1176,284)
for i,(name,port,proto,exit,node) in enumerate(PORTS):
    y=284+i*74
    if i==1: rect(225,y,950,74,'#f0f6fe')
    if i: line(224,y,1176,y)
    dot(245,y+27); txt(257,y+32,name,14,weight=600); txt(242,y+55,proto,10,MUTED)
    txt(444,y+32,port,13); copy(505,y+19)
    txt(578,y+29,exit,12); txt(578,y+52,node,11,MUTED)
    if i==1: badge(861,y+17,'当前目标'); txt(861,y+56,'点击上方开启',10,MUTED)
    elif i==0: txt(861,y+32,'设为系统代理',12,BLUE)
    else: txt(861,y+32,'不适用 · SOCKS5',11,'#8b929a')
    toggle(1095,y+24); more(1153,y+32)
rect(224,529,952,191,'#fff',BORDER,7)
txt(242,561,'浏览器 · 使用此入口',15,weight=600)
badge(242,579,'系统代理目标（未开启）'); badge(473,579,'chrome.exe 进程规则')
txt(242,632,'同一程序使用系统代理时，需检查实际采用的入口。',11,'#956b20')
line(242,651,1157,651)
txt(242,687,'TUN 未开启 · 默认出口：机场出口',12,MUTED); txt(1157,687,'配置独立 TUN 接管 →',12,BLUE,anchor='end')
footer(); save('04-port-centric-system-proxy')

# 05 — A control-path view makes TUN's direct relation to an exit explicit.
navigation('graph'); title('接管工作台','把流量来源、代理端口与出口放在同一条路径中')
txt(224,146,'系统代理：关',12,MUTED); txt(447,146,'TUN：关',12,MUTED); txt(624,146,'进程分流：3 条规则启用',12,GREEN)
rect(224,174,952,446,'#fcfdff',BORDER,7)
for x,label in [(244,'流量来源'),(521,'代理端口'),(889,'出口线路')]: txt(x,204,label,12,weight=600)
path('M441 290H521','#a9b3c0',1.6,extra='stroke-dasharray="4 4"')
path('M716 290H809Q845 290 845 326V379Q845 409 879 409H889','#a9b3c0',1.6,extra='stroke-dasharray="4 4"')
path('M441 406H521',BLUE,2)
path('M716 406H889',BLUE,2)
path('M441 548H815Q845 548 845 515V444Q845 409 879 409H889','#a9b3c0',1.6,extra='stroke-dasharray="4 4"')
for y,label,sub,on in [(252,'系统代理','遵循 Windows 代理的应用',False),(368,'进程分流','示例：Code.exe 规则',True),(510,'TUN','虚拟入口 · 独立接管',False)]:
    rect(244,y,197,77,'#fff','#d5dde7',6)
    txt(259,y+28,label,14,weight=600); txt(259,y+53,sub,10,MUTED)
    if on: toggle(392,y+14)
    else: off(392,y+14)
for y,label,port in [(252,'浏览器','7894 · HTTP + SOCKS5'),(368,'机场','7893 · HTTP + SOCKS5')]:
    rect(521,y,195,77,'#fff','#d5dde7',6); txt(537,y+28,label,14,weight=600); txt(537,y+53,port,10,MUTED)
rect(889,344,267,136,'#fff','#86b0da',6)
txt(906,375,'机场出口',16,weight=600); txt(906,406,'SG 新加坡01',18,weight=600); txt(906,436,'系统默认网卡 · 182 ms',11,MUTED); txt(1138,464,'配置 →',11,BLUE,anchor='end')
txt(548,539,'TUN 直接使用默认出口',11,MUTED); txt(548,562,'不绑定某个代理端口',10,MUTED)
checkhint(224,640,952)
txt(224,712,'接管组合的具体处理规则待兼容性核对后确定。',11,MUTED); txt(1176,712,'代理端口与出口配置 →',11,BLUE,anchor='end')
footer(); save('05-capture-path-workbench')
