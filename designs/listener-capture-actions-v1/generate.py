"""Capture actions live with listeners, while TUN still references an exit."""
from monitor import *

def overview():
    shell(); nav('概览',99,selected=True); nav('代理入口',147,'input'); nav('程序分流',195,'apps')
    section('资源',276); nav('出口',289,'exit'); nav('节点与订阅',333,'nodes'); nav('Tailscale 实例',374,child=True)
    section('运行查看',483); nav('实时连接',496,'connections'); nav('日志',540,'log'); nav('设置',752,'settings')
    title('概览')
    for x,label,value in [(224,'上传速率','0 B/s'),(465,'下载速率','444 B/s'),(706,'累计流量','317.3 MB'),(947,'代理入口','3 / 3 运行中')]: stat(x,108,229,label,value)
    txt(224,241,'监听器',17,weight=600); txt(297,240,'3 个运行中',11,MUTED); button(1056,219,120,'＋ 新建监听器',True)
    txt(224,270,'系统代理：浏览器 :7894  ·  TUN：关闭',11,MUTED)
    rect(224,289,952,309,'#fff',BORDER,7)
    for x,label in [(242,'名称 / 端口'),(420,'出口'),(604,'当前节点'),(799,'网卡'),(905,'实时流量'),(1008,'系统代理'),(1114,'启用')]: txt(x,316,label,11,MUTED)
    line(224,332,1176,332)
    rows=[('机场','7893 · 混合','机场出口','SG 新加坡01','182 ms','359 B/s'),('浏览器','7894 · 混合','机场出口','SG 新加坡01','182 ms','0 B/s'),('下载器','7895 · SOCKS5','下载专用','JP 东京02','96 ms','85 B/s')]
    for i,(name,port,exit,node,delay,rate) in enumerate(rows):
        y=332+i*88
        if i: line(224,y,1176,y)
        dot(245,y+28); txt(257,y+33,name,14,weight=600); txt(242,y+58,port,10,MUTED)
        field(420,y+15,165,exit); txt(420,y+67,'共用 2 个入口' if i<2 else '独立出口',10,MUTED)
        field(604,y+15,174,node); txt(604,y+67,delay,11,GREEN)
        txt(799,y+35,'系统默认',12)
        txt(905,y+29,'↑ 0 B/s',10,MUTED); txt(905,y+52,'↓ '+rate,11)
        if i==0: txt(1008,y+35,'设为系统代理',11,BLUE)
        elif i==1:
            rect(1006,y+14,82,24,'#eaf3fc',r=4); txt(1014,y+30,'系统代理',11,BLUE); chevron(1073,y+24,BLUE)
            txt(1008,y+60,'已启用',10,GREEN)
        else:
            txt(1008,y+34,'不适用',11,'#8b929b'); txt(1008,y+56,'需 HTTP / 混合',9,'#8b929b')
        toggle(1116,y+23); more(1160,y+32)
    txt(224,641,'程序分流',16,weight=600); txt(1176,641,'管理程序规则 →',12,BLUE,anchor='end')
    rect(224,659,952,62,'#fff',BORDER,7); dot(245,690); txt(259,695,'分流组件运行中',12); txt(473,695,'3 条启用规则',12,MUTED); txt(789,695,'按程序选择代理入口',11,MUTED)
    line(224,751,1176,751,'#e0e2e5'); txt(224,781,'12 条活跃连接',11,MUTED); txt(356,781,'查看连接 →',11,BLUE)
    dot(886,777,'#b17b18',2.5); txt(897,781,'日志有 1 条警告',11,MUTED); txt(1176,781,'查看日志 →',11,BLUE,anchor='end')

overview(); save('01-listener-inline-actions')

overview()
# The listener's context menu offers a shortcut to the shared exit, not a per-port TUN switch.
rect(831,377,345,342,'#fff','#d8dde5',7,'filter="url(#shadow)"')
txt(847,403,'机场 · 7893',11,MUTED)
txt(847,438,'复制代理地址',12)
txt(847,472,'设为系统代理',12)
rect(837,486,333,67,'#edf5ff',r=3)
txt(847,511,'使用此出口开启 TUN…',12,BLUE,500)
txt(847,535,'出口：机场出口（与浏览器共用）',10,MUTED)
line(847,565,1159,565,'#edf0f3')
txt(847,591,'查看实时连接',12)
txt(847,624,'查看相关日志',12)
line(847,641,1159,641,'#edf0f3')
txt(847,668,'编辑监听器…',12)
txt(847,701,'删除监听器',12,'#b52f28')
save('02-listener-context-menu')
