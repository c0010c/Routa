"""Review-only SVG concepts. Application code and runtime configuration are untouched."""
from pathlib import Path

ROOT = Path(__file__).parent
helper = (ROOT.parent / 'navigation-proxy-v1' / 'generate.py').read_text(encoding='utf-8')
exec(helper.split("P.append('<svg")[0])
ROOT = Path(__file__).parent

def shell(active):
    P.clear()
    P.append('<svg xmlns="http://www.w3.org/2000/svg" width="1360" height="820" viewBox="0 0 1360 820">')
    P.append(f'<title>Routa 代理 · {active} Tab 设计预览</title>')
    P.append('<g font-family="Segoe UI,Microsoft YaHei UI,Microsoft YaHei,sans-serif">')
    rect(0,0,1360,820,BG,r=10)
    rect(0,0,1360,34,'#1c1c1c')
    rect(14,10,14,14,BLUE,r=3); text(37,22,'Routa',12,MUTED)
    line(1232,17,1242,17,MUTED); rect(1281,12,10,10,'none',MUTED)
    path('M1328 12l10 10m0-10-10 10',MUTED,1)
    rect(0,34,204,786,NAV)
    rect(22,60,28,28,BLUE,r=7)
    path('M30 66v10q0 4 4 4h8M40 66v9m-3-3 3 3 3-3','#17222b',1.8)
    text(62,81,'Routa',18,INK,600)
    for i,(name,label) in enumerate([('overview','概览'),('proxy','代理'),('exit','出口'),('nodes','订阅与节点')]):
        y=116+i*48
        if i==1:
            rect(10,y,184,40,'#303030',r=6); rect(10,y+11,3,18,BLUE,r=1.5)
        icon(name,26,y+12,INK if i==1 else MUTED)
        text(59,y+26,label,14,INK if i==1 else MUTED,600 if i==1 else 400)
    line(22,714,182,714)
    icon('settings',26,739); text(59,753,'设置',14,MUTED)
    text(24,798,'v0.2.0',11,DIM)
    text(240,106,'代理',28,INK,600)
    dot(1200,96); text(1212,101,'内核运行中',12,MUTED)
    # Page-level navigation; each workspace gets the entire content width.
    for x,label,count in [(240,'监听器','1'),(370,'进程','2')]:
        selected=label==active
        text(x+8,165,label,15,INK if selected else MUTED,600 if selected else 400)
        cx=x+(72 if label=='监听器' else 57)
        rect(cx,148,24,23,'#383838' if selected else '#292929',r=5)
        text(cx+12,164,count,11,INK if selected else MUTED,500,'middle')
        if selected: rect(x+8,179,82 if label=='监听器' else 67,3,BLUE,r=1.5)
    line(240,184,1324,184)
    text(1324,798,'设计预览 · 示例数据',11,DIM,anchor='end')

def save(name):
    P.append('</g></svg>')
    (ROOT/(name+'.svg')).write_text('\n'.join(P),encoding='utf-8')

shell('监听器')
text(240,235,'监听器',18,INK,600); info(311,229)
button(1184,210,140,'＋ 新建监听器',True)
rect(240,272,1084,278,PANEL,LINE,8)
icon('input',264,299,INK)
text(296,315,'默认监听器',17,INK,600)
rect(401,296,44,24,'#373737',r=4); text(423,313,'混合',11,MUTED,anchor='middle')
dot(472,308); text(484,313,'运行中',12,GREEN)
toggle(1220,299); more(1296,308)
line(264,341,1300,341)
text(264,375,'监听地址',12,MUTED)
text(264,411,'127.0.0.1:7881',21,INK,500); icon('copy',462,395,MUTED)
text(264,442,'HTTP / SOCKS5',12,DIM)
text(790,375,'出口',12,MUTED)
field(790,389,308,'日常出口')
dot(1120,405,GREEN,2.5); text(1132,410,'香港 01',12,INK)
text(1132,436,'42 ms',11,MUTED)
line(264,473,1300,473)
icon('apps',264,497,MUTED); text(292,511,'关联进程',12,MUTED)
text(359,511,'2',12,INK,600)
text(1300,511,'查看进程 →',12,BLUE,anchor='end')
save('01-proxy-listeners')

shell('进程')
text(240,235,'进程',18,INK,600); info(291,229)
dot(329,229); text(341,234,'分流运行中',12,GREEN); toggle(431,220)
button(1184,210,140,'＋ 添加进程',True)
rect(240,272,1084,333,PANEL,LINE,8)
rect(264,293,1036,36,'#242424','#424242',5)
P.append('<circle cx="280" cy="309" r="5" fill="none" stroke="#929292" stroke-width="1.2"/>')
path('M284 313l4 4',DIM,1.2); text(300,316,'搜索进程名称或路径',12,DIM)
for x,label in [(264,'进程'),(632,'监听器'),(961,'状态'),(1196,'启用')]: text(x,364,label,12,MUTED)
line(264,381,1300,381)
for i,(name,desc) in enumerate([('chrome.exe','Google Chrome'),('Code.exe','Visual Studio Code')]):
    y=382+i*88
    rect(264,y+24,34,34,'#373737',r=6)
    if i==0:
        P.append(f'<circle cx="281" cy="{y+41}" r="9" fill="none" stroke="{MUTED}" stroke-width="1.3"/>')
        path(f'M272 {y+41}h18M281 {y+32}c-5 5-5 13 0 18m0-18c5 5 5 13 0 18',MUTED,1.2)
    else: path(f'M276 {y+36}l-5 5 5 5m12-14-10 18m10-18v18',MUTED,1.5)
    text(312,y+37,name,14,INK,500); text(312,y+59,desc,11,DIM)
    field(632,y+25,276,'7881 · 默认监听器')
    dot(967,y+42,GREEN,2.5); text(979,y+47,'已代理',12,GREEN)
    toggle(1196,y+33); more(1286,y+42)
    line(264,y+87,1300,y+87)
text(264,584,'2 条规则已启用',11,MUTED)
text(1300,584,'查看连接 →',12,BLUE,anchor='end')
rect(240,625,1084,60,PANEL,LINE,7)
icon('settings',264,647,MUTED); text(296,660,'分流设置',13,INK)
text(1298,661,'›',20,MUTED)
save('02-proxy-processes')
print('Saved listener and process Tab previews.')
