"""Secondary diagnostics: normal overview and optional contextual drawers."""
from layout import *

def search(x,y,w,label):
    rect(x,y,w,32,'#fff','#d9dde3',4)
    add(f'<circle cx="{x+14}" cy="{y+14}" r="4" fill="none" stroke="#9299a2"/>'); line(x+17,y+17,x+21,y+21,'#9299a2'); txt(x+30,y+21,label,11,'#9299a2')
def close(x,y): path(f'M{x-4} {y-4}l8 8m0-8-8 8',MUTED,1.2)
def overlay(x,w):
    rect(200,33,999,798,'#101820',extra='opacity=".045"')
    rect(x-8,33,8,798,'#c3c8cf',extra='opacity=".10"')
    rect(x,33,w,798,'#fff','#dedfe2')
def tag(x,y,s,warn=False):
    rect(x,y,45,19,'#fff1d8' if warn else '#f0f2f5',r=3); txt(x+22.5,y+13,s,9,'#996b13' if warn else '#6b7583',500,'middle')

def overview():
    shell()
    nav('概览',99,selected=True); nav('代理入口',147,'input'); nav('程序分流',195,'apps')
    section('资源',285); nav('出口',298,'exit'); nav('节点与订阅',346,'nodes'); nav('Tailscale 实例',387,child=True)
    # Diagnostics is a secondary tool launcher, not a business-module row.
    icon('network',27,712,'#7c838c'); txt(58,726,'诊断工具',12,'#747c86'); chevron(171,718,'#7c838c')
    nav('设置',752,'settings')
    title('概览')
    for x,label,value in [(224,'上传速率','0 B/s'),(465,'下载速率','444 B/s'),(706,'累计流量','317.3 MB'),(947,'代理入口','3 / 3 运行中')]: stat(x,108,229,label,value)
    txt(224,243,'代理入口',17,weight=600); button(1056,219,120,'＋ 新建监听器',True)
    rect(224,265,952,309,'#fff',BORDER,7)
    for x,label in [(242,'名称 / 端口'),(430,'出口'),(629,'当前节点'),(835,'网卡'),(946,'实时流量'),(1100,'操作')]: txt(x,292,label,11,MUTED)
    line(224,308,1176,308)
    for i,(name,port,exit,node,delay,rate) in enumerate([('机场','7893 · SOCKS5','机场出口','SG 新加坡01','182 ms','359 B/s'),('浏览器','7894 · 混合','机场出口','SG 新加坡01','182 ms','0 B/s'),('下载器','7895 · SOCKS5','下载专用','JP 东京02','96 ms','85 B/s')]):
        y=308+i*88
        if i: line(224,y,1176,y)
        dot(245,y+29); txt(257,y+34,name,14,weight=600); txt(242,y+60,port,10,MUTED)
        field(430,y+15,175,exit); txt(430,y+68,'共用 2 个入口' if i<2 else '独立出口',10,MUTED)
        field(629,y+15,184,node); txt(629,y+68,delay,11,GREEN)
        txt(835,y+35,'系统默认',12)
        txt(946,y+29,'↑ 0 B/s',10,MUTED); txt(946,y+52,'↓ '+rate,11)
        toggle(1100,y+23); more(1153,y+32)
    txt(224,619,'程序分流',16,weight=600); txt(1176,619,'管理程序规则 →',12,BLUE,anchor='end')
    rect(224,637,952,69,'#fff',BORDER,7); dot(245,671); txt(259,676,'分流组件运行中',12); txt(473,676,'3 条启用规则',12,MUTED); txt(789,676,'按程序选择代理入口',11,MUTED)
    txt(224,770,'配置已生效',11,MUTED)

# Default: no log stream, connection list, or monitoring dashboard in the main workspace.
overview(); save('01-default-overview')

# Optional connection inspection scoped to one listener.
overview(); overlay(726,473)
txt(748,62,'辅助查看',10,MUTED); close(1173,57)
txt(748,99,'机场 · 实时连接',19,weight=600)
txt(748,126,'仅显示通过此入口建立的连接',11,MUTED)
line(726,145,1199,145)
search(748,164,334,'搜索目标地址或程序'); button(1094,164,83,'暂停')
txt(748,224,'4 条活跃连接',11,MUTED)
for x,label in [(748,'目标地址'),(1075,'下载累计')]: txt(x,258,label,10,MUTED)
line(726,273,1199,273)
rows=[('api.example.net:443','Code.exe · TCP','628 KB'),('media.example.net:443','192.168.1.24 · UDP','8.1 MB'),('assets.example.com:443','Code.exe · TCP','194 KB'),('updates.example.org:443','Code.exe · TCP','71 KB')]
for i,(host,src,down) in enumerate(rows):
    y=273+i*67
    if i==0: rect(727,y,471,67,'#edf5ff')
    if i: line(726,y,1199,y)
    txt(748,y+27,host,12,weight=500); txt(748,y+49,src,10,MUTED); txt(1155,y+29,down,11,anchor='end')
line(726,541,1199,541)
txt(748,576,'选中连接',12,weight=600); txt(748,605,'api.example.net:443',14,weight=600)
for y,label,value in [(644,'入口','机场 · 7893'),(678,'出口','机场出口'),(712,'当前节点','SG 新加坡01')]: txt(748,y,label,10,MUTED); txt(887,y,value,12)
txt(748,750,'持续 46 秒 · 上传 18 KB · 下载 628 KB',10,MUTED)
rect(748,775,429,32,'#fff','#e4bdbb',4); txt(962,796,'断开此连接',12,'#b52f28',500,'middle')
save('02-context-connections')

# Optional related logs. Basic filtering and copying stay local to the drawer.
overview(); overlay(606,593)
txt(628,62,'辅助查看',10,MUTED); close(1173,57)
txt(628,99,'机场 · 相关日志',19,weight=600)
txt(628,126,'按入口关联；其他记录可在全部日志查看。',11,MUTED)
line(606,145,1199,145)
field(628,162,198,'全部来源'); field(838,162,198,'全部级别'); button(1048,162,129,'暂停滚动')
search(628,207,549,'搜索日志内容')
rows=[
    ('14:32:08','INFO','内核','TCP api.example.net:443 using 机场出口'),
    ('14:32:07','INFO','分流组件','Code.exe routed to 机场 :7893'),
    ('14:32:06','WARN','内核','Connection timeout: api.example.net:443'),
    ('14:32:04','INFO','应用','已应用「机场」入口的配置'),
    ('14:32:01','INFO','内核','Listener 机场 started on 127.0.0.1:7893'),
    ('14:31:58','INFO','分流组件','Process routing rules applied (3 rules)'),
]
for i,(tm,lv,source,msg) in enumerate(rows):
    y=260+i*61
    if lv=='WARN': rect(607,y,591,61,'#fff8e9')
    if i: line(628,y,1177,y,'#edf0f3')
    txt(628,y+21,tm,10,MUTED); tag(703,y+7,lv,lv=='WARN'); txt(763,y+21,source,10,MUTED)
    txt(628,y+45,msg,11)
line(606,626,1199,626)
txt(628,660,'选中日志',12,weight=600); txt(1177,660,'复制原文',11,BLUE,anchor='end')
txt(628,692,'14:32:06 · 内核 · WARN',10,MUTED)
txt(628,721,'Connection timeout: api.example.net:443',12)
line(606,754,1199,754)
txt(628,791,'日志接收中',10,GREEN); txt(1177,791,'查看全部日志 →',11,BLUE,anchor='end')
save('03-context-logs')
