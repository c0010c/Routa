"""Compact, sidebar-free alternatives for review. All values are examples."""
from pathlib import Path

ROOT=Path(__file__).parent
source=(ROOT.parent/'simple-mode-v1'/'generate.py').read_text(encoding='utf-8')
exec(source.split('# First launch:')[0])
ROOT=Path(__file__).parent

def begin(w,h,title):
    P.clear()
    P.append(f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">')
    P.append(f'<title>{escape(title)}</title><g font-family="Segoe UI,Microsoft YaHei UI,Microsoft YaHei,sans-serif">')
    rect(0,0,w,h,'#202020',10,'#444444')
    rect(0,0,w,34,'#1b1b1b')
    rect(13,10,14,14,BLUE,3);text(36,22,'Routa',12,MUTED)
    line(w-125,17,w-115,17,MUTED)
    rect(w-81,12,9,9,'none',0,MUTED)
    path(f'M{w-34} 12l10 10m0-10-10 10',MUTED,1)

def brand(x=28,y=59):
    rect(x,y,27,27,BLUE,6)
    icon('RouteOutlined',x+4,y+4,'#17222b',19)
    text(x+40,y+21,'Routa',19,weight=600)

def footer(w,y):
    line(28,y,w-28,y)
    icon('DnsOutlined',30,y+22,MUTED,20)
    text(61,y+38,'订阅与节点',13,MUTED)
    icon('SettingsOutlined',w-100,y+22,MUTED,20)
    text(w-71,y+38,'设置',13,MUTED)

def segmented(x,y,w,selected):
    rect(x,y,w,36,'#1d1d1d',5,'#444444')
    half=w/2
    rect(x+3+selected*half,y+3,half-6,30,'#3b3b3b',3)
    for i,label in enumerate(['全部应用','指定应用']):
        text(x+half*(i+.5),y+23,label,12,INK if i==selected else DIM,500 if i==selected else 400,'middle')

# 1. Everyday compact window. No sidebar, giant power illustration, or nested dashboard.
begin(520,640,'新方案 1：紧凑连接窗口')
brand();chip(429,61,'简洁版')
circle(36,132,4,GREEN)
text(50,139,'代理已开启',16,weight=600)
text(28,170,'系统代理',12,DIM)
# A modest signal indicator is informational; the primary control is the button below.
for i,height in enumerate([8,13,18,23]): rect(447+i*9,148-height,5,height,GREEN,1)

text(28,216,'当前节点',12,MUTED)
rect(28,232,464,100,'#2c2c2c',7,'#414141')
icon('LanguageOutlined',46,266,MUTED,26)
text(87,270,'香港 01',20,weight=600)
text(87,300,'日常订阅',12,DIM)
text(405,286,'42 ms',12,GREEN)
path('M468 274l5 5-5 5',MUTED,1.5)
button(28,352,464,'关闭代理',False,46)

text(28,448,'↓  下载',12,DIM)
text(28,478,'2.4 MB/s',20,weight=500)
line(260,430,260,483)
text(286,448,'↑  上传',12,DIM)
text(286,478,'128 KB/s',20,weight=500)
text(28,527,'连接选项',12,MUTED);chevron(91,519)
footer(520,557)
save('01-compact-daily')

# 3. Single-page everyday and per-app controls, sharing a single node.
begin(960,680,'新方案 3：连接与应用同页')
brand();chip(869,61,'简洁版')
rect(28,112,904,142,'#282828',8,'#3e3e3e')
circle(53,142,3.5,GREEN)
text(65,147,'代理已开启',14,weight=600)
text(52,179,'当前节点',11,DIM)
text(52,213,'香港 01',23,weight=600)
text(175,211,'42 ms',12,GREEN)
text(246,211,'更换节点',12,BLUE);chevron(303,203)
button(718,169,190,'关闭代理',False,44)

text(28,299,'代理范围',14,weight=600)
segmented(116,276,272,1)
button(800,277,132,'＋ 添加应用',False,34)

rows=[('LanguageOutlined','Google Chrome','chrome.exe',True),
      ('SendOutlined','Telegram','Telegram.exe',True),
      ('ChatBubbleOutlineOutlined','微信','WeChat.exe',False),
      ('SportsEsportsOutlined','Steam','steam.exe',False)]
for i,(name,label,exe,on) in enumerate(rows):
    x=28+(i%2)*464;y=336+(i//2)*114
    rect(x,y,440,96,'#2b3033' if on else '#262626',7,'#42606c' if on else '#3d3d3d')
    rect(x+18,y+23,42,42,'#363636',7)
    icon(name,x+29,y+34,MUTED,20)
    text(x+76,y+38,label,15,weight=500)
    text(x+76,y+63,exe,11,DIM)
    toggle(x+384,y+28,on)
text(28,586,'2 个应用走代理，其余应用直连',12,MUTED)
footer(960,612)
save('02-single-page-apps')

# First launch and settings use the same compact window as the daily design.
begin(520,640,'首次启动：紧凑模式选择')
brand()
text(28,143,'先选一种使用方式',27,weight=600)
text(28,178,'以后可在设置中更改',13,MUTED)
for i,(label,desc) in enumerate([
    ('简洁版','选节点、开代理，日常使用更轻松'),
    ('完整版','多监听器、多出口与精细分流')
]):
    y=215+i*122
    rect(28,y,464,102,'#29353b' if i==0 else '#282828',7,BLUE if i==0 else LINE)
    circle(54,y+33,8,BLUE if i==0 else 'none',BLUE if i==0 else '#7f7f7f')
    if i==0: path(f'M50 {y+33}l3 3 5-6','#17222b',1.6)
    text(77,y+39,label,19,weight=600)
    if i==0: chip(388,y+18,'推荐')
    text(77,y+72,desc,12,MUTED)
button(28,485,464,'开始使用',True,46)
text(260,578,'切换界面会保留已有配置',12,DIM,anchor='middle')
save('03-first-launch')

begin(520,640,'简洁版：设置与模式切换')
icon('ArrowBackOutlined',28,66,MUTED,21)
text(63,84,'设置',23,weight=600)
text(28,137,'界面模式',13,MUTED)
rect(28,153,464,60,'#2b2b2b',6,LINE)
text(46,189,'简洁版',15,weight=500)
text(419,189,'切换',12,BLUE);chevron(464,181)
text(28,241,'保留已有连接和配置',12,DIM)
text(28,292,'常规',13,MUTED)
rect(28,308,464,116,'#2b2b2b',6,LINE)
text(46,344,'开机启动',14);toggle(438,327,False)
line(46,366,474,366)
text(46,400,'启动后自动连接',14);toggle(438,383,False)
rect(28,454,464,56,'#2b2b2b',6,LINE)
text(46,489,'故障诊断',14);path('M468 477l5 5-5 5',MUTED,1.3)
text(28,590,'Routa',12,MUTED);text(492,590,'v0.2.0',11,DIM,anchor='end')
save('04-compact-settings')
print('Saved four alternative layout mockups')
