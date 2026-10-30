"""Runtime connections and log placement concepts. Static illustrative data only."""
from layout import *

def nav(label,y,name='overview',selected=False,child=False):
    if selected: rect(9,y,182,40,'#e4e4e4',r=5); rect(10,y+9,3,22,BLUE,r=1.5)
    if not child:
        if name=='log':
            rect(28,y+11,14,17,'none','#32373d',1.5)
            for d in [16,20,24]: line(31,y+d,39,y+d,'#32373d')
        elif name=='connections':
            path(f'M28 {y+16}h13m-4-4 4 4-4 4M41 {y+25}H28m4-4-4 4 4 4','#32373d')
        else: icon(name,27,y+12,'#32373d')
    txt(67 if child else 58,y+26,label,12 if child else 14,MUTED if child else INK,600 if selected else 400)
    if child: line(51,y+2,51,y+36,'#cfd2d6'); line(51,y+20,59,y+20,'#cfd2d6')

def search(x,y,w,label='搜索域名、IP 或程序'):
    rect(x,y,w,32,'#fff','#d9dde3',4)
    add(f'<circle cx="{x+14}" cy="{y+14}" r="4" fill="none" stroke="#9299a2"/>'); line(x+17,y+17,x+21,y+21,'#9299a2'); txt(x+30,y+21,label,11,'#9299a2')
def redbutton(x,y,w,label):
    rect(x,y,w,32,'#fff','#e4bdbb',4); txt(x+w/2,y+21,label,12,'#b52f28',500,'middle')
def checkbox(x,y,on=False):
    if on: tick(x,y)
    else: rect(x,y,14,14,'#fff','#c9cdd3',2)
def xmark(x,y,c='#8c9198'): path(f'M{x-3} {y-3}l6 6m0-6-6 6',c,1.2)

CONNS=[
    ('docs.example.com:443','chrome.exe','浏览器 · 7894','SG 新加坡01','TCP','2.4 MB','2 分 18 秒'),
    ('api.example.net:443','Code.exe','机场 · 7893','SG 新加坡01','TCP','628 KB','46 秒'),
    ('media.example.net:443','局域网设备','机场 · 7893','SG 新加坡01','UDP','8.1 MB','1 分 32 秒'),
    ('files.example.org:443','aria2c.exe','下载器 · 7895','JP 东京02','TCP','12.3 MB','4 分 05 秒'),
    ('images.example.com:443','chrome.exe','浏览器 · 7894','SG 新加坡01','TCP','946 KB','18 秒'),
]
LOGS=[
    ('14:32:08','INFO','内核','TCP docs.example.com:443 using 机场出口'),
    ('14:32:07','INFO','分流组件','Code.exe routed to 机场 :7893'),
    ('14:32:06','WARN','内核','Connection timeout: update.example.org:443'),
    ('14:32:04','INFO','应用','已应用出口「机场出口」的配置'),
    ('14:32:01','INFO','内核','Listener 浏览器 started on 127.0.0.1:7894'),
    ('14:31:58','INFO','内核','Tailscale instance 家庭网络 is running'),
    ('14:31:55','INFO','分流组件','Process routing rules applied (3 rules)'),
    ('14:31:52','INFO','应用','订阅「机场订阅」更新成功，共 24 个节点'),
]
def level(x,y,s):
    bg='#fff3db' if s=='WARN' else '#f0f2f5'; c='#996b13' if s=='WARN' else '#6b7583'
    rect(x,y,45,19,bg,r=3); txt(x+22.5,y+13,s,9,c,500,'middle')
def logrow(x,y,w,row,selected=False,compact=False):
    tm,lv,src,msg=row
    if selected: rect(x,y,w,39 if compact else 43,'#fff8e9')
    txt(x+13,y+24,tm,10,MUTED); level(x+86,y+11,lv)
    txt(x+145,y+24,src,10,MUTED)
    txt(x+221,y+24,msg,10 if compact else 11,INK)

# 01 — Two independent first-class pages. Configuration remains separate.
shell(); section('运行监控',100)
nav('概览',111); nav('实时连接',155,'connections',True); nav('日志',199,'log')
section('配置与资源',278); nav('代理入口',290,'input'); nav('程序分流',334,'apps'); nav('出口',378,'exit'); nav('节点与订阅',422,'nodes'); nav('Tailscale 实例',463,child=True)
nav('诊断',704,'network'); nav('设置',752,'settings')
title('实时连接','查看正在访问的目标、来源和实际出口')
txt(224,145,'12',29,weight=600); txt(270,144,'条活跃连接',12,MUTED)
txt(440,144,'↑ 12.3 KB/s',14,MUTED); txt(650,144,'↓ 428 KB/s',14)
button(1078,121,98,'暂停刷新')
search(224,176,333); field(569,175,173,'全部入口'); field(754,175,161,'全部协议'); redbutton(1016,176,160,'断开选中（1）')
rect(224,229,952,393,'#fff',BORDER,7)
for x,label in [(275,'目标地址'),(536,'来源'),(678,'入口 / 节点'),(945,'下载累计'),(1051,'时长')]: txt(x,256,label,11,MUTED)
checkbox(242,243); line(224,272,1176,272)
for i,(host,proc,entry,node,proto,down,age) in enumerate(CONNS):
    y=272+i*62
    if i==0: rect(225,y,950,62,'#eef5fe')
    if i: line(224,y,1176,y)
    checkbox(242,y+21,i==0); txt(275,y+25,host,12,weight=500); txt(275,y+46,proto,10,MUTED)
    txt(536,y+25,proc,11); txt(678,y+25,entry,11); txt(678,y+46,node,10,MUTED)
    txt(945,y+25,down,11); txt(1051,y+25,age,10); xmark(1155,y+28)
line(224,582,1176,582); txt(242,607,'共 12 条连接 · 当前显示 5 条',10,MUTED); txt(1157,607,'更新中',10,GREEN,anchor='end')
rect(224,643,952,120,'#fff',BORDER,7)
txt(242,674,'docs.example.com:443',15,weight=600); txt(1157,674,'查看相关日志 →',12,BLUE,anchor='end')
txt(242,707,'chrome.exe',12); txt(423,707,'浏览器 :7894',12); txt(601,707,'→  机场出口',12,MUTED); txt(789,707,'→  SG 新加坡01',12,MUTED)
txt(242,739,'来源 127.0.0.1:53928',10,MUTED); txt(602,739,'上传累计 24 KB',10,MUTED); txt(882,739,'下载累计 2.4 MB',10,MUTED)
save('01-independent-monitor-pages')

# 02 — All runtime observation belongs to one monitoring center with sub-tabs.
shell(); nav('运行监控',99,'overview',True); nav('接入配置',151,'input'); nav('出口',203,'exit'); nav('节点与订阅',255,'nodes'); nav('Tailscale 实例',296,child=True); nav('设置',752,'settings')
title('运行监控','状态、实时连接与日志集中查看')
tabs([(110,'概览',False),(153,'实时连接  12',False),(126,'日志  1 警告',True)])
field(224,184,161,'全部来源'); field(397,184,145,'全部级别'); search(554,185,363,'搜索日志内容')
button(929,185,76,'暂停'); button(1017,185,76,'导出'); button(1105,185,71,'更多')
txt(224,250,'内核  ·  分流组件  ·  应用',11,MUTED); txt(1176,250,'自动滚动已开启',11,BLUE,anchor='end')
rect(224,269,952,408,'#fff',BORDER,7)
for x,label in [(237,'时间'),(310,'级别'),(369,'来源'),(445,'内容')]: txt(x,295,label,10,MUTED)
line(224,310,1176,310)
for i,row in enumerate(LOGS):
    logrow(224,310+i*43,952,row,i==2)
    if i<7: line(224,353+i*43,1176,353+i*43,'#f0f1f3')
txt(242,670,'接收中 · 1 条警告',10,MUTED)
rect(224,694,952,88,'#fff',BORDER,7)
txt(242,725,'选中日志',12,weight=600); txt(354,725,'内核 · WARN · 14:32:06',11,MUTED)
txt(242,756,'Connection timeout: update.example.org:443',12); txt(1157,756,'复制原文',11,BLUE,anchor='end')
save('02-monitor-center')

# 03 — Activity center: connections and logs can be watched at the same time.
shell(); nav('活动中心',99,'overview',True); nav('接入管理',151,'input'); nav('出口线路',203,'exit'); nav('节点资源',255,'nodes'); nav('设置',752,'settings')
title('活动中心','同时观察连接与日志，按目标或入口定位问题')
search(224,128,380); field(616,127,183,'全部入口'); button(1088,128,88,'暂停更新')
txt(224,196,'实时连接',16,weight=600); txt(329,195,'12 条',11,MUTED)
rect(224,213,648,258,'#fff',BORDER,7)
for x,label in [(240,'目标地址'),(484,'入口'),(662,'下载累计')]: txt(x,239,label,10,MUTED)
line(224,254,872,254)
for i,(host,proc,entry,node,proto,down,age) in enumerate(CONNS[:4]):
    y=254+i*53
    if i==0: rect(225,y,646,53,'#edf5ff')
    if i: line(224,y,872,y)
    txt(240,y+22,host,11); txt(240,y+40,proc+' · '+proto,9,MUTED); txt(484,y+22,entry,10); txt(662,y+22,down,10); xmark(849,y+24)
txt(224,508,'日志',16,weight=600); badge(281,490,'按当前目标筛选'); txt(872,507,'全部日志 →',11,BLUE,anchor='end')
rect(224,526,648,247,'#fff',BORDER,7)
for i,(tm,lv,src,msg) in enumerate([
    ('14:32:08','INFO','内核','docs.example.com:443 → 机场出口'),
    ('14:32:08','INFO','内核','TCP connection established'),
    ('14:32:04','INFO','应用','机场出口配置已应用'),
    ('14:31:58','INFO','内核','Listener 浏览器 is ready'),
]):
    y=541+i*48
    txt(238,y+14,tm,9,MUTED); level(302,y,lv); txt(361,y+14,msg,10)
    if i<3: line(238,y+32,856,y+32,'#f0f1f3')
txt(239,756,'按目标 / 入口关联，无法关联的记录仍可在全部日志查看。',9,MUTED)
rect(888,213,288,560,'#fff',BORDER,7)
txt(907,244,'连接详情',15,weight=600); txt(907,281,'docs.example.com',14,weight=600); txt(907,305,'TCP · 443',11,MUTED)
for y,label,value in [(349,'来源程序','chrome.exe'),(409,'代理入口','浏览器 · 7894'),(469,'当前出口','机场出口'),(529,'当前节点','SG 新加坡01'),(589,'持续时间','2 分 18 秒'),(649,'下载累计','2.4 MB')]:
    txt(907,y,label,10,MUTED); txt(907,y+24,value,12)
redbutton(907,716,250,'断开此连接')
save('03-activity-split-view')

# 04 — Object-first inspection, plus global connection/log entry points.
shell(); nav('概览',99); nav('本机程序',151,'apps'); nav('代理入口',203,'input',True); nav('机场',244,child=True)
section('资源',326); nav('出口',338,'exit'); nav('节点与订阅',382,'nodes'); nav('Tailscale 实例',423,child=True)
section('全局监控',507); nav('所有连接',519,'connections'); nav('全部日志',563,'log'); nav('设置',752,'settings')
txt(224,66,'代理入口  /  机场',11,MUTED); txt(224,103,'机场',25,weight=600); badge(293,83,'运行中',GREEN,'#edf5ee')
address(224,139,'127.0.0.1:7893'); txt(471,139,'SOCKS5',12,MUTED); txt(1176,139,'机场出口 → SG 新加坡01',12,BLUE,anchor='end')
tabs([(104,'概览',False),(104,'配置',False),(112,'连接  4',True),(104,'日志',False)],y=160)
txt(224,232,'此入口的实时连接',16,weight=600); button(1066,209,110,'查看所有连接')
rect(224,252,952,296,'#fff',BORDER,7)
for x,label in [(242,'目标地址'),(545,'来源'),(748,'协议'),(857,'下载累计'),(1010,'时长')]: txt(x,280,label,11,MUTED)
line(224,296,1176,296)
for i,(host,proc,proto,down,age) in enumerate([
    ('api.example.net:443','Code.exe','TCP','628 KB','46 秒'),
    ('media.example.net:443','192.168.1.24','UDP','8.1 MB','1 分 32 秒'),
    ('assets.example.com:443','Code.exe','TCP','194 KB','21 秒'),
    ('updates.example.org:443','Code.exe','TCP','71 KB','12 秒'),
]):
    y=296+i*62
    if i: line(224,y,1176,y)
    txt(242,y+34,host,12); txt(545,y+34,proc,11); txt(748,y+34,proto,11); txt(857,y+34,down,11); txt(1010,y+34,age,10); xmark(1156,y+30)
txt(224,590,'最近日志',16,weight=600); txt(1176,590,'仅此入口 · 查看更多 →',11,BLUE,anchor='end')
rect(224,608,952,159,'#fff',BORDER,7)
for i,row in enumerate([
    ('14:32:07','INFO','分流组件','Code.exe routed to 机场 :7893'),
    ('14:32:04','INFO','应用','已应用「机场」入口的配置'),
    ('14:31:58','INFO','内核','Listener 机场 started on 127.0.0.1:7893'),
]):
    logrow(224,619+i*43,952,row)
    if i<2: line(224,662+i*43,1176,662+i*43,'#f0f1f3')
save('04-object-context-monitoring')

# 05 — A global dock follows configuration pages, like a desktop inspector.
shell(); nav('概览',99); nav('接入配置',151,'input'); nav('网络资源',203,'nodes',True); nav('出口',244,child=True); nav('节点与订阅',284,child=True); nav('Tailscale 实例',324,child=True)
nav('设置',752,'settings')
txt(224,66,'网络资源  /  出口',11,MUTED); txt(224,103,'机场出口',25,weight=600); txt(1176,103,'编辑出口 →',12,BLUE,anchor='end')
rect(224,126,952,242,'#fff',BORDER,7)
txt(243,158,'路由配置',15,weight=600); badge(335,140,'正常',GREEN,'#edf5ee')
txt(243,194,'当前节点',11,MUTED); field(243,207,326,'SG 新加坡01')
txt(600,194,'选择策略',11,MUTED); field(600,207,247,'手动选择')
txt(878,194,'出口网卡',11,MUTED); field(878,207,278,'系统默认')
line(243,261,1156,261); txt(243,292,'关联入口',11,MUTED); badge(334,275,'机场 · 7893'); badge(466,275,'浏览器 · 7894')
txt(243,336,'修改此出口会影响以上两个入口。',11,MUTED); txt(1156,336,'已生效',11,GREEN,anchor='end')
txt(224,412,'实时连接 12 条  ·  下载 428 KB/s  ·  上传 12.3 KB/s',11,MUTED)
# Dock spans the content area and is independent of the selected module.
rect(201,440,998,360,'#fff','#d3d7dc')
rect(201,440,998,40,'#f9fafb')
txt(224,465,'监视器',12,weight=600); txt(322,465,'实时连接  12',12,MUTED); txt(476,465,'日志',12,weight=600); badge(519,448,'1 警告','#986710','#fff1d8')
rect(475,477,30,3,BLUE,r=1.5)
path('M1114 461l4-4 4 4',MUTED,1.2); more(1150,459); xmark(1180,459)
field(224,491,143,'全部来源'); field(379,491,130,'全部级别'); search(521,492,369,'搜索日志内容'); button(902,492,70,'暂停'); button(984,492,70,'导出'); button(1066,492,110,'清空视图')
for i,row in enumerate(LOGS[:5]):
    logrow(211,538+i*43,978,row,i==2)
    if i<4: line(211,581+i*43,1190,581+i*43,'#f0f1f3')
txt(224,784,'日志接收中',10,GREEN); txt(1176,784,'打开日志目录 →',11,BLUE,anchor='end')
save('05-global-monitor-dock')
