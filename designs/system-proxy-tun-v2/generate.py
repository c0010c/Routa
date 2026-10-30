"""Quieter capture controls: compact status buttons and deliberate popovers."""
from monitor import *

def off(x,y):
    rect(x,y,32,17,'#fff','#888e97',8.5); dot(x+8.5,y+8.5,'#868d96',5.5)
def close(x,y): path(f'M{x-3} {y-3}l6 6m0-6-6 6',MUTED,1.2)
def navigation():
    shell(); nav('概览',99,selected=True); nav('代理入口',147,'input'); nav('程序分流',195,'apps')
    section('资源',276); nav('出口',289,'exit'); nav('节点与订阅',333,'nodes'); nav('Tailscale 实例',374,child=True)
    section('运行查看',483); nav('实时连接',496,'connections'); nav('日志',540,'log'); nav('设置',752,'settings')
def overview():
    navigation(); title('概览')
    # Status buttons are smaller than normal command buttons and have no primary fill.
    rect(871,98,163,28,'#f9f9fa','#e0e2e6',4); txt(885,117,'系统代理：关',11,MUTED); chevron(1014,109)
    rect(1046,98,130,28,'#f9f9fa','#e0e2e6',4); txt(1060,117,'TUN：关',11,MUTED); chevron(1156,109)
    for x,label,value in [(224,'上传速率','0 B/s'),(465,'下载速率','444 B/s'),(706,'累计流量','317.3 MB'),(947,'代理入口','3 / 3 运行中')]: stat(x,143,229,label,value)
    txt(224,276,'代理入口',17,weight=600); button(1056,252,120,'＋ 新建入口',True)
    rect(224,297,952,304,'#fff',BORDER,7)
    for x,label in [(242,'名称 / 端口'),(430,'出口'),(629,'当前节点'),(835,'网卡'),(946,'实时流量'),(1100,'操作')]: txt(x,324,label,11,MUTED)
    line(224,339,1176,339)
    for i,(name,port,proto,exit,node,delay,rate) in enumerate([
        ('机场','7893','混合','机场出口','SG 新加坡01','182 ms','359 B/s'),
        ('浏览器','7894','混合','机场出口','SG 新加坡01','182 ms','0 B/s'),
        ('下载器','7895','SOCKS5','下载专用','JP 东京02','96 ms','85 B/s'),
    ]):
        y=339+i*87
        if i: line(224,y,1176,y)
        dot(245,y+28); txt(257,y+33,name,14,weight=600); txt(242,y+58,port+' · '+proto,10,MUTED)
        field(430,y+15,175,exit); txt(430,y+67,'共用 2 个入口' if i<2 else '独立出口',10,MUTED)
        field(629,y+15,184,node); txt(629,y+67,delay,11,GREEN)
        txt(835,y+35,'系统默认',12); txt(946,y+29,'↑ 0 B/s',10,MUTED); txt(946,y+52,'↓ '+rate,11)
        toggle(1100,y+23); more(1153,y+32)
    txt(224,645,'程序分流',16,weight=600); txt(1176,645,'管理程序规则 →',12,BLUE,anchor='end')
    rect(224,663,952,59,'#fff',BORDER,7); dot(245,692); txt(259,697,'分流组件运行中',12); txt(473,697,'3 条启用规则',12,MUTED); txt(789,697,'按程序选择代理入口',11,MUTED)
    line(224,751,1176,751,'#e0e2e5'); txt(224,781,'12 条活跃连接',11,MUTED); txt(356,781,'查看连接 →',11,BLUE)
    dot(886,777,'#b17b18',2.5); txt(897,781,'日志有 1 条警告',11,MUTED); txt(1176,781,'查看日志 →',11,BLUE,anchor='end')

overview(); save('01-overview-compact-status')

overview()
rect(772,137,404,247,'#fff','#d8dde5',7,'filter="url(#shadow)"')
txt(790,168,'系统代理',15,weight=600); txt(1047,168,'已关闭',10,MUTED); off(1123,154)
line(772,185,1176,185,'#edf0f3')
txt(790,213,'目标代理端口',11,MUTED); field(790,226,368,'浏览器 · 7894（混合）')
txt(790,286,'可选择 HTTP 或混合端口',10,MUTED)
txt(790,312,'当前目标出口：机场出口',11,MUTED)
line(790,332,1158,332,'#edf0f3')
txt(790,362,'系统代理设置 →',11,BLUE); txt(1158,362,'仅影响遵循系统设置的应用',10,MUTED,anchor='end')
save('02-system-proxy-popover')

overview()
rect(744,137,432,304,'#fff','#d8dde5',7,'filter="url(#shadow)"')
txt(762,168,'TUN 接管',15,weight=600); txt(1047,168,'已关闭',10,MUTED); off(1123,154)
line(744,185,1176,185,'#edf0f3')
txt(762,213,'默认出口',11,MUTED); field(762,226,396,'机场出口')
txt(762,286,'当前节点：SG 新加坡01',11,MUTED)
rect(762,304,396,67,'#fff7e7',r=4)
txt(774,328,'进程分流当前已启用 3 条规则。',11,'#956b20')
txt(774,350,'开启前检查接管范围与回环。',11,'#956b20')
line(762,389,1158,389,'#edf0f3')
txt(762,418,'路由与 DNS 设置 →',11,BLUE); txt(1158,418,'TUN 独立使用此出口',10,MUTED,anchor='end')
save('03-tun-popover')
