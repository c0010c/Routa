"""Moderate emphasis: visible secondary navigation, full connection/log pages."""
from monitor import *

def navigation(active):
    nav('概览',99,selected=active=='概览'); nav('代理入口',147,'input'); nav('程序分流',195,'apps')
    section('资源',276); nav('出口',289,'exit'); nav('节点与订阅',333,'nodes'); nav('Tailscale 实例',374,child=True)
    section('运行查看',483)
    nav('实时连接',496,'connections',active=='实时连接'); nav('日志',540,'log',active=='日志')
    nav('设置',752,'settings')

def overview():
    shell(); navigation('概览'); title('概览')
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
        txt(835,y+35,'系统默认',12); txt(946,y+29,'↑ 0 B/s',10,MUTED); txt(946,y+52,'↓ '+rate,11)
        toggle(1100,y+23); more(1153,y+32)
    txt(224,619,'程序分流',16,weight=600); txt(1176,619,'管理程序规则 →',12,BLUE,anchor='end')
    rect(224,637,952,69,'#fff',BORDER,7); dot(245,671); txt(259,676,'分流组件运行中',12); txt(473,676,'3 条启用规则',12,MUTED); txt(789,676,'按程序选择代理入口',11,MUTED)
    # A quiet global summary makes the secondary pages discoverable.
    line(224,742,1176,742,'#e0e2e5')
    txt(224,772,'12 条活跃连接',11,MUTED); txt(356,772,'查看连接 →',11,BLUE)
    dot(886,768,'#b17b18',2.5); txt(897,772,'日志有 1 条警告',11,MUTED); txt(1176,772,'查看日志 →',11,BLUE,anchor='end')

overview(); save('01-balanced-overview')

# Full page for deliberate inspection. This is not a permanent home-page panel.
shell(); navigation('实时连接'); title('实时连接','查看目标地址、来源入口和当前路径')
txt(224,145,'12 条活跃连接',14,weight=500); txt(440,144,'↑ 0 B/s',12,MUTED); txt(632,144,'↓ 444 B/s',12)
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
txt(242,707,'chrome.exe',12); txt(423,707,'浏览器 :7894',12); txt(601,707,'→ 机场出口',12,MUTED); txt(789,707,'→ SG 新加坡01',12,MUTED)
txt(242,739,'来源 127.0.0.1:53928',10,MUTED); txt(602,739,'上传累计 24 KB',10,MUTED); txt(882,739,'下载累计 2.4 MB',10,MUTED)
save('02-connections-page')

shell(); navigation('日志'); title('运行日志','按来源和级别查看记录，定位异常原因')
field(224,130,161,'全部来源'); field(397,130,145,'全部级别'); search(554,131,363,'搜索日志内容')
button(929,131,76,'暂停'); button(1017,131,76,'导出'); button(1105,131,71,'更多')
txt(224,196,'内核  ·  分流组件  ·  应用',11,MUTED); txt(1176,196,'自动滚动已开启',11,BLUE,anchor='end')
rect(224,215,952,408,'#fff',BORDER,7)
for x,label in [(237,'时间'),(310,'级别'),(369,'来源'),(445,'内容')]: txt(x,241,label,10,MUTED)
line(224,256,1176,256)
for i,row in enumerate(LOGS):
    logrow(224,256+i*43,952,row,i==2)
    if i<7: line(224,299+i*43,1176,299+i*43,'#f0f1f3')
txt(242,616,'接收中 · 1 条警告',10,MUTED)
rect(224,644,952,119,'#fff',BORDER,7)
txt(242,676,'选中日志',12,weight=600); txt(354,676,'内核 · WARN · 14:32:06',11,MUTED)
txt(242,712,'Connection timeout: update.example.org:443',12)
txt(242,744,'打开日志目录',11,BLUE); txt(1157,744,'复制原文',11,BLUE,anchor='end')
save('03-logs-page')
