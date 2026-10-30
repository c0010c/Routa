"""Five independent overview directions. Only static design assets are emitted."""
from ui import *

def rail(dark=False,accent=BLUE,warm=False):
    bg='#242526' if dark else '#f4f3f0' if warm else '#f3f5f7'
    fg='#aaaeb2' if dark else '#737b86'
    rect(1,33,71,798,bg)
    rect(22,53,28,28,accent,r=7); path('M30 60v10q0 3 4 3h6M39 60v8m-3-3 3 3 3-3','#202020' if dark else '#fff',1.4)
    for i,(name,label) in enumerate([('overview','概览'),('input','监听器'),('apps','分流'),('exit','出口'),('nodes','节点'),('globe','组网')]):
        y=113+i*65
        if i==0: rect(10,y-4,52,52,'#363a3d' if dark else '#fff',r=7); rect(10,y+7,3,24,accent,r=1.5)
        icon(name,28,y+4,accent if i==0 else fg); txt(36,y+39,label,9,accent if i==0 else fg,500 if i==0 else 400,'middle')
    icon('settings',28,763,fg); txt(36,798,'设置',9,fg,anchor='middle')

def check(x,y,c=BLUE,filled=True):
    add(f'<circle cx="{x}" cy="{y}" r="10" fill="{c if filled else "#fff"}" stroke="{c if filled else "#d8dde4"}"/>')
    if filled: path(f'M{x-4} {y}l3 3 6-6','#fff',1.6)

def search(x,y,w,label):
    rect(x,y,w,32,'#fff','#dee2e7',4); add(f'<circle cx="{x+14}" cy="{y+14}" r="4" fill="none" stroke="#9299a2"/>'); line(x+17,y+17,x+21,y+21,'#9299a2'); txt(x+30,y+21,label,11,'#9299a2')

def summaryline(x,y):
    dot(x,y-4); txt(x+12,y,'内核运行中',11,MUTED)

# F — Flat, quiet, typographic. No dashboard card grid.
start('#ffffff'); rail()
txt(106,81,'概览',16,weight=600); summaryline(1012,79); more(1163,75)
txt(106,157,'一切正常',38,weight=600)
txt(108,190,'3 个代理入口已就绪，所有配置均已生效。',13,MUTED)
txt(821,127,'下载',11,MUTED); txt(821,164,'444',32,weight=600); txt(901,162,'B/s',12,MUTED)
txt(1015,127,'上传',11,MUTED); txt(1015,164,'0',32,weight=600); txt(1047,162,'B/s',12,MUTED)
txt(821,192,'累计流量 317.3 MB',11,MUTED)
line(106,227,1168,227,'#e9edf0')
txt(106,271,'代理入口',17,weight=600); txt(206,270,'3',12,MUTED)
search(840,247,192,'查找入口'); button(1048,247,120,'＋ 新建监听器',True)
for x,label in [(106,'名称与地址'),(445,'出口与节点'),(710,'实时流量'),(888,'出口延迟'),(1078,'启用')]: txt(x,317,label,11,'#8b929b')
for i,(name,port,exit,node,delay,rate,proto) in enumerate([('机场',7893,'机场出口','SG 新加坡01','182 ms','359 B/s','SOCKS5'),('浏览器',7894,'机场出口','SG 新加坡01','182 ms','0 B/s','HTTP + SOCKS5'),('下载器',7895,'下载专用','JP 东京02','96 ms','85 B/s','SOCKS5')]):
    y=335+i*110
    line(106,y,1168,y,'#edf0f3')
    rect(106,y+25,42,42,'#f2f6fb',r=9); icon('input',119,y+38,'#6b8dac')
    txt(165,y+43,name,16,weight=600); dot(220 if i==0 else 237,y+38,GREEN,2.5)
    txt(165,y+68,f'127.0.0.1:{port}',12,MUTED); copy(302,y+56,'#9aa1aa')
    txt(165,y+90,proto,10,'#9298a0')
    txt(445,y+44,node,16); chevron(646,y+35)
    txt(445,y+69,exit+(' · 共用 2 个入口' if i<2 else ' · 独立出口'),11,MUTED)
    txt(710,y+44,'↓ '+rate,13); txt(710,y+68,'↑ 0 B/s',11,'#9298a0')
    txt(888,y+44,delay,13,GREEN); txt(888,y+68,'系统默认网卡',11,MUTED)
    toggle(1078,y+36); more(1157,y+45)
line(106,665,1168,665,'#edf0f3')
txt(106,707,'节点与网卡不可用时，相关入口自动停用。',11,'#9298a0')
txt(106,793,'Routa',11,'#939aa3'); txt(1168,793,'v0.2.0',10,'#939aa3',anchor='end')
save('overview-f-minimal')

# G — Quick-settings tiles. A listener is a self-contained daily-use control.
start('url(#mica)'); rail()
txt(100,86,'快捷连接',29,weight=600); summaryline(1012,79); more(1163,75)
txt(100,115,'选择一个入口，直接切换线路或复制连接地址。',12,MUTED)
txt(100,165,'↓ 444 B/s',22,weight=600); txt(280,165,'↑ 0 B/s',17,MUTED); txt(436,164,'3 / 3 运行中',13,GREEN)
button(1048,143,120,'＋ 新建监听器',True)
tiles=[(100,'机场',7893,'SOCKS5','新加坡 01','SG','机场出口','182 ms','359 B/s','#eaf3ff',BLUE),(460,'浏览器',7894,'HTTP + SOCKS5','新加坡 01','SG','机场出口','182 ms','0 B/s','#f0edf9','#7662ad'),(820,'下载器',7895,'SOCKS5','东京 02','JP','下载专用','96 ms','85 B/s','#eaf4f0','#38866c')]
for x,name,port,proto,node,flag,exit,delay,rate,tint,c in tiles:
    rect(x,204,344,348,'#fff','#dce2ec',9,'filter="url(#shadow)"')
    rect(x+1,205,342,108,tint,r=8); rect(x+1,298,342,15,tint)
    rect(x+20,224,36,36,'#ffffff',r=8); icon('input',x+30,234,c)
    txt(x+69,245,name,17,weight=600); txt(x+69,268,f'{port} · {proto}',11,MUTED)
    toggle(x+289,234,c)
    txt(x+20,342,'当前节点',10,MUTED); txt(x+20,374,flag+' '+node,23,weight=600); chevron(x+310,359)
    txt(x+20,403,exit,11,MUTED); txt(x+324,403,delay,12,GREEN,500,'end')
    line(x+20,424,x+324,424,'#edf0f5')
    txt(x+20,450,'↓ '+rate,13); txt(x+324,450,'系统默认网卡',11,MUTED,anchor='end')
    rect(x+16,478,312,51,'#f7f8fa',r=5)
    txt(x+29,508,f'127.0.0.1:{port}',12); copy(x+294,491)
rect(100,575,704,200,'#fff','#dce2ec',8)
txt(121,603,'实时流量',13,weight=600); txt(783,603,'最近 60 秒',11,MUTED,anchor='end')
graph(121,622,662,118); txt(121,758,'上传 0 B/s',10,MUTED); txt(783,758,'下载 444 B/s',10,BLUE,anchor='end')
rect(820,575,344,200,'#fff','#dce2ec',8)
txt(840,604,'共用出口',13,weight=600); badge(840,621,'机场出口')
txt(840,671,'机场  ·  浏览器',15,weight=600)
txt(840,697,'切换节点会同时影响这两个入口。',11,MUTED)
txt(840,751,'管理出口',12,BLUE); txt(1144,751,'→',16,BLUE,anchor='end')
save('overview-g-quick-tiles')

# H — A routing matrix. Assignment is a row/column operation, not a chain of forms.
start('#f5f6f8'); sidebar(184)
txt(216,82,'连接分配',28,weight=600); summaryline(1012,79); more(1163,75)
txt(216,112,'入口选出口，出口选节点。',12,MUTED)
txt(216,162,'3 个入口运行中',13,GREEN); txt(395,162,'2 个出口在用',12,MUTED); txt(565,162,'↓ 444 B/s',13)
button(1048,139,120,'＋ 新建监听器',True)
rect(216,194,952,397,'#fff','#dce1e8',7)
rect(217,195,950,153,'#fafbfd',r=6); rect(217,327,950,21,'#fafbfd')
txt(239,236,'出口',12,MUTED); path('M271 225l5 5-5 5',MUTED,1.2)
txt(239,318,'代理入口',13,weight=600)
for x,title,node,sub,count,c in [(436,'机场出口','SG 新加坡01','手动选择 · 182 ms','2 个入口',BLUE),(680,'下载专用','JP 东京02','手动选择 · 96 ms','1 个入口','#6b5b9b'),(924,'直连','不经过代理节点','系统默认网卡','未使用','#76818e')]:
    line(x,194,x,552,'#e7ebf0')
    if x==436: rect(x+1,195,242,152,'#eef5ff',r=0); rect(x+1,195,242,3,BLUE)
    txt(x+18,224,title,15,weight=600); txt(x+226,223,count,10,MUTED,anchor='end')
    if x!=924: field(x+18,244,208,node)
    else: txt(x+18,266,node,12,MUTED)
    txt(x+18,304,sub,11,MUTED); more(x+223,315)
for i,(name,port,proto,active) in enumerate([('机场',7893,'SOCKS5',0),('浏览器',7894,'混合',0),('下载器',7895,'SOCKS5',1)]):
    y=348+i*68
    line(216,y,1168,y,'#e7ebf0'); dot(240,y+25); txt(252,y+30,name,14,weight=600); txt(252,y+51,f'{port} · {proto}',10,MUTED)
    for j,cx in enumerate([558,802,1046]):
        if j==active:
            rect(cx-121,y+1,242,66,'#f4f8fe' if j==0 else '#f6f4fa')
            check(cx,y+32,BLUE if j==0 else '#7662ad')
        else: check(cx,y+32,filled=False)
line(216,552,1168,552,'#e7ebf0')
txt(239,577,'点击交点，为该入口选择出口。',11,MUTED); txt(1147,577,'更改后自动应用',11,MUTED,anchor='end')
rect(216,613,952,160,'#fff','#dce1e8',7)
icon('exit',238,637,BLUE); txt(264,651,'机场出口',16,weight=600); badge(362,634,'2 个入口共用')
txt(238,684,'节点切换将同时影响「机场」「浏览器」。',12,MUTED)
txt(238,751,'当前出口网卡：系统默认',11,MUTED)
txt(738,645,'当前节点',11,MUTED); field(738,659,253,'SG 新加坡01')
button(1038,660,108,'编辑出口')
txt(738,720,'累计上传',10,MUTED); txt(738,744,'150.7 MB',16,weight=500)
txt(988,720,'累计下载',10,MUTED); txt(988,744,'150.3 MB',16,weight=500)
save('overview-h-routing-matrix')

# I — A channel-strip console; compact modules and per-listener live meters.
ACC='#e0b575'; DF='#e6e6e6'; DM='#9d9fa3'
start('#202122',True); rail(True,ACC)
txt(100,82,'连接控制台',26,DF,600); dot(1035,74,'#83b996'); txt(1048,79,'内核运行中',11,DM); more(1163,75,DM)
rect(100,110,1068,89,'#2a2c2e','#3b3d3f',5)
txt(120,138,'总下载',10,DM); txt(120,176,'444',28,DF,600); txt(194,174,'B/s',12,DM)
txt(279,138,'总上传',10,DM); txt(279,174,'0 B/s',22,DF,500)
line(425,129,425,181,'#454749')
txt(449,138,'累计流量',10,DM); txt(449,174,'317.3 MB',22,DF,500)
txt(702,138,'监听器',10,DM); txt(702,173,'3 / 3',22,'#83b996',600)
txt(983,145,'所有配置已生效',11,DM); txt(983,174,'内核操作',11,ACC); more(1145,169,DM)
txt(100,242,'监听器',15,DF,600); txt(185,242,'3 条连接',11,DM)
rect(1048,220,120,32,ACC,r=4); txt(1108,241,'＋ 新建监听器',12,'#242424',500,'middle')
for x,label in [(120,'入口'),(333,'出口'),(518,'节点'),(735,'网卡'),(928,'实时流量')]: txt(x,282,label,10,DM)
trackdata=[('01','机场','7893 · SOCKS5','机场出口','SG 新加坡01','182 ms','359 B/s',ACC),('02','浏览器','7894 · 混合','机场出口','SG 新加坡01','182 ms','0 B/s','#8babc1'),('03','下载器','7895 · SOCKS5','下载专用','JP 东京02','96 ms','85 B/s','#9caf94')]
for i,(num,name,port,exit,node,delay,rate,c) in enumerate(trackdata):
    y=299+i*139
    rect(100,y,1068,123,'#2c2e30','#45413a' if i==0 else '#393c3f',5)
    rect(100,y,4,123,c,r=2)
    txt(120,y+25,num,10,c,600); txt(147,y+30,name,17,DF,600); txt(120,y+57,port,11,DM)
    toggle(120,y+80,c); txt(162,y+93,'启用',10,DM)
    line(309,y+50,333,y+50,'#66625c',1.4); line(492,y+50,518,y+50,'#66625c',1.4); line(708,y+50,735,y+50,'#66625c',1.4)
    for x,w,label in [(333,159,exit),(518,190,node),(735,148,'系统默认')]:
        rect(x,y+28,w,43,'#232527','#4d5053',3)
        txt(x+12,y+55,label,13,DF)
        if x!=735: chevron(x+w-20,y+47,DM)
    txt(333,y+96,'共用 · 2 个入口' if i<2 else '独立出口',10,DM)
    txt(518,y+96,'手动选择',10,DM); dot(740,y+91,'#83b996'); txt(750,y+96,delay,11,'#83b996')
    txt(928,y+28,'↓ '+rate,13,DF); txt(1131,y+28,'↑ 0 B/s',10,DM,anchor='end')
    vals=[2,3,2,4,7,4,3,5,8,13,7,10,18,12,10,7,4,5,3,4,2,3,2,4,3,2,3,2]
    for j,v in enumerate(vals):
        ht=(v*(1 if i==0 else .32 if i==1 else .7))+3
        rect(928+j*7,y+80-ht,3,ht,c if j<23 else '#585b5e',r=1)
    more(1144,y+96,DM)
line(100,747,1168,747,'#383b3e')
txt(100,780,'机场   ·   127.0.0.1:7893',11,DM); copy(311,767,DM)
txt(1168,780,'仅本机访问',11,DM,anchor='end')
save('overview-i-channel-console')

# J — App-first overview. Executable names are illustrative, not local inventory.
TEAL='#087f78'; WARM='#f8f7f4'
start(WARM); sidebar(184)
rect(184,33,1015,798,WARM)
txt(216,82,'概览',28,weight=600); dot(1032,75,GREEN); txt(1044,80,'内核运行中',11,MUTED); more(1163,75)
rect(216,112,171,33,'#e7e6e1',r=5); rect(219,115,81,27,'#fff',r=4)
txt(259,134,'按程序',12,TEAL,600,'middle'); txt(342,134,'按端口',12,MUTED,anchor='middle')
txt(216,187,'程序连接',18,weight=600); badge(316,168,'分流已启用',TEAL,'#e6f2ee')
txt(216,212,'3 个程序使用代理  ·  下载 444 B/s  ·  上传 0 B/s',11,MUTED)
button(1048,169,120,'＋ 添加程序',True)
apps=[('浏览器','chrome.exe','浏览器 :7894','机场出口','SG 新加坡01','0 B/s','browser'),('Visual Studio Code','Code.exe','机场 :7893','机场出口','SG 新加坡01','359 B/s','code'),('下载工具','aria2c.exe','下载器 :7895','下载专用','JP 东京02','85 B/s','download')]
for i,(name,exe,entry,exit,node,rate,kind) in enumerate(apps):
    y=236+i*165
    rect(216,y,610,147,'#fff','#e1e0dc',7)
    rect(235,y+20,39,39,'#f0f3f4' if kind=='browser' else '#edf3fb' if kind=='code' else '#e8f3ee',r=9)
    if kind=='browser':
        add(f'<circle cx="254.5" cy="{y+39.5}" r="11" fill="none" stroke="#648ea7" stroke-width="2"/>'); path(f'M244 {y+39.5}h21M254.5 {y+29}c-6 5-6 16 0 21m0-21c6 5 6 16 0 21','#648ea7',1.3)
    elif kind=='code': path(f'M246 {y+35}l-5 5 5 5m16-12-11 14m11-14v17','#4382b4',2)
    else: path(f'M254 {y+28}v17m-6-6 6 6 6-6M244 {y+49}h20',TEAL,1.8)
    txt(288,y+38,name,15,weight=600); txt(288,y+58,exe,10,MUTED)
    txt(744,y+39,'↓ '+rate,12,anchor='end'); more(802,y+34)
    line(235,y+77,807,y+77,'#eeede9')
    txt(235,y+105,'代理路径',10,MUTED)
    txt(311,y+105,entry,12,TEAL,500); txt(441,y+105,'→',12,'#a3a5a3'); txt(468,y+105,node,12)
    txt(311,y+128,exit+' · 系统默认网卡',10,MUTED)
    txt(786,y+109,'更改',11,TEAL,anchor='end')
rect(846,236,322,477,'#f0f0eb','#e0e0d9',7)
txt(866,268,'代理入口',15,weight=600); txt(1148,268,'3 个',11,MUTED,anchor='end')
for i,(name,port,proto,rate) in enumerate([('机场',7893,'SOCKS5','359 B/s'),('浏览器',7894,'混合','0 B/s'),('下载器',7895,'SOCKS5','85 B/s')]):
    y=287+i*115
    if i: line(866,y-6,1148,y-6,'#dedfd7')
    dot(870,y+17,TEAL); txt(882,y+22,name,14,weight=600); txt(1148,y+22,'↓ '+rate,10,MUTED,anchor='end')
    txt(866,y+49,f'127.0.0.1:{port}',12); copy(1129,y+37)
    txt(866,y+73,proto+' · 仅本机',10,MUTED)
txt(866,689,'管理监听器',12,TEAL); txt(1148,689,'→',16,TEAL,anchor='end')
txt(216,779,'程序分流规则已生效',11,MUTED); txt(1168,779,'累计流量 317.3 MB',11,MUTED,anchor='end')
save('overview-j-app-workspace')
