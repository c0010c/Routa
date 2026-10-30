"""Two alternative overview mockups. SVG design assets only."""
from pathlib import Path
from html import escape

ROOT=Path(__file__).parent
P=[]
INK='#1b1b1b'; MUTED='#696969'; BLUE='#0067c0'; GREEN='#0f7b0f'; BORDER='#e3e3e3'
def add(s): P.append(s)
def rect(x,y,w,h,fill='#fff',stroke='none',r=0):
    add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill}" stroke="{stroke}"/>')
def txt(x,y,s,size=14,c=INK,weight=400,anchor='start'):
    add(f'<text x="{x}" y="{y}" font-size="{size}" fill="{c}" font-weight="{weight}" text-anchor="{anchor}">{escape(str(s))}</text>')
def path(d,c=MUTED,w=1.4): add(f'<path d="{d}" fill="none" stroke="{c}" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round"/>')
def line(x,y,x2,y2,c=BORDER): path(f'M{x} {y}L{x2} {y2}',c,1)
def dot(x,y,c=GREEN,r=3): add(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{c}"/>')
def chevron(x,y): path(f'M{x} {y}l3.5 3.5 3.5-3.5',MUTED,1.2)
def more(x,y):
    for d in [-4,0,4]: dot(x+d,y,MUTED,1)
def toggle(x,y): rect(x,y,32,17,BLUE,r=8.5); dot(x+24,y+8.5,'#fff',5.5)
def copy(x,y):
    rect(x+3,y+3,9,11,'none',MUTED,1.5); path(f'M{x+8} {y}H{x}v10',MUTED,1)
def button(x,y,w,label,primary=False):
    rect(x,y,w,32,BLUE if primary else '#fff','none' if primary else '#d8d8d8',4)
    txt(x+w/2,y+21,label,12,'#fff' if primary else INK,500,'middle')
def field(x,y,w,label):
    rect(x,y,w,34,'#fff','#cfcfcf',4); txt(x+11,y+22,label,13); chevron(x+w-19,y+15)
def smallpill(x,y,label,c=BLUE):
    rect(x,y,len(label)*11+12,20,'#edf4fb' if c==BLUE else '#f3f3f3',r=3); txt(x+6,y+14,label,10,c)
def icon(name,x,y,c=INK):
    if name=='overview':
        for dx,dy,w,h in [(0,0,6,9),(10,0,6,5),(0,13,6,3),(10,9,6,7)]: rect(x+dx,y+dy,w,h,'none',c,0.5)
    elif name=='input': path(f'M{x+6} {y}h10v16H{x+6}M{x} {y+8}h10m-3-3 3 3-3 3',c)
    elif name=='apps':
        for dx in [0,6,12]:
            for dy in [0,6,12]: rect(x+dx,y+dy,3,3,c,r=.5)
    elif name=='exit': path(f'M{x+8} {y+16}V{y+9}L{x+2} {y+3}m0 5V{y+3}h5M{x+8} {y+9}l6-6m-5 0h5v5',c)
    elif name=='nodes':
        for a,b in [(8,1),(1,7),(15,7),(4,15),(12,15)]:
            line(x+8,y+8,x+a,y+b,c); add(f'<circle cx="{x+a}" cy="{y+b}" r="2" fill="#f3f3f3" stroke="{c}" stroke-width="1.4"/>')
        dot(x+8,y+8,c,2)
    elif name=='globe':
        add(f'<circle cx="{x+8}" cy="{y+8}" r="7.5" fill="none" stroke="{c}" stroke-width="1.4"/>'); path(f'M{x+1} {y+8}h14M{x+8} {y}c-5 5-5 11 0 16m0-16c5 5 5 11 0 16',c,1.2)
    else:
        add(f'<circle cx="{x+8}" cy="{y+8}" r="5.5" fill="none" stroke="{c}" stroke-width="2.5" stroke-dasharray="3 2"/>'); add(f'<circle cx="{x+8}" cy="{y+8}" r="2.3" fill="none" stroke="{c}"/>')

def shell():
    P.clear(); add('<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="832" viewBox="0 0 1200 832"><g font-family="Microsoft YaHei UI,Microsoft YaHei,Segoe UI,sans-serif">')
    rect(0,0,1200,832,'#f3f3f3','#d7d7d7',8)
    rect(10,9,14,14,BLUE,r=2); txt(30,20,'Routa',12)
    line(1080,16,1090,16,INK); rect(1127,11,9,9,'none',INK); path('M1173 11l9 9m0-9-9 9',INK,1)
    rect(20,48,25,25,BLUE,r=5); path('M28 54v10q0 3 3 3h5M36 55v8m-2-2 2 2 2-2','#fff',1.5); txt(55,66,'Routa',16,weight=600)
    for i,(name,label) in enumerate([('overview','概览'),('input','监听器'),('apps','进程分流'),('exit','出口'),('nodes','节点'),('globe','Tailscale')]):
        y=91+i*52
        if i==0: rect(8,y,184,44,'#e3e3e3',r=5); rect(9,y+10,3,24,BLUE,r=1.5)
        icon(name,28,y+14); txt(60,y+28,label,14,weight=600 if i==0 else 400)
    icon('settings',28,747); txt(60,761,'设置'); txt(21,807,'v0.2.0',11,MUTED)
    txt(224,78,'概览',25,weight=600)
    dot(1005,72); txt(1018,77,'内核运行中',12); more(1158,72)

def save(name):
    add('</g></svg>'); (ROOT/(name+'.svg')).write_text('\n'.join(P),encoding='utf-8')

def graph(x,y,w,h):
    values=[.10,.13,.09,.12,.10,.12,.18,.42,.16,.13,.19,.17,.16,.35,.79,.42,.39,.62,.47,.73,.50,.34,.25,.15,.13,.16,.20,.12,.11,.14,.12,.13,.17,.11,.12,.10,.14,.16,.12,.15]
    for level in [.1,.5,.9]: line(x,y+level*h,x+w,y+level*h,'#efefef')
    pts=[(x+i*w/(len(values)-1),y+h-v*h) for i,v in enumerate(values)]
    d='M'+'L'.join(f'{a:.1f} {b:.1f}' for a,b in pts)
    add(f'<path d="{d}L{x+w} {y+h}L{x} {y+h}Z" fill="{BLUE}" opacity=".055"/>'); path(d,BLUE,1.5)
    path('M'+'L'.join(f'{x+i*w/39:.1f} {y+h-(.03+(i%7)/200)*h:.1f}' for i in range(40)),'#949494',1.2)

# A: compact table with text-first cells and a contextual command bar.
shell()
rect(224,105,952,92,'#fff',BORDER,6)
for x in [448,674,946]: line(x,123,x,178)
for x,label,value,unit in [(244,'上传速率','0','B/s'),(468,'下载速率','444','B/s'),(694,'累计流量','317.3','MB'),(966,'监听器','3 / 3','运行中')]:
    txt(x,132,label,12,MUTED); txt(x,171,value,24,weight=600)
    unitx=x+({'0':25,'444':58,'317.3':87,'3 / 3':77}[value]); txt(unitx,170,unit,12,GREEN if label=='监听器' else MUTED)
txt(224,239,'监听器',17,weight=600)
rect(300,218,63,29,'#e6e6e6',r=4); txt(331,237,'全部 3',12,anchor='middle')
txt(381,237,'异常 0',12,MUTED)
button(1066,216,110,'＋ 新建监听器',True)
rect(224,262,952,294,'#fff',BORDER,6)
rect(225,263,950,37,'#fafafa',r=5); rect(225,280,950,20,'#fafafa')
cols=[(242,'名称 / 端口'),(420,'出口'),(581,'当前节点'),(762,'网卡'),(861,'延迟'),(940,'实时流量'),(1082,'操作')]
for x,label in cols: txt(x,286,label,12,MUTED)
line(224,300,1176,300)
rows=[('机场','7893 · SOCKS5','机场出口','SG 新加坡01','182 ms','359 B/s'),('浏览器','7894 · 混合','机场出口','SG 新加坡01','182 ms','0 B/s'),('下载器','7895 · SOCKS5','下载专用','JP 东京02','96 ms','85 B/s')]
for i,(name,port,exit,node,delay,rate) in enumerate(rows):
    y=300+i*74
    if i==0: rect(225,y,950,74,'#f1f7fd'); rect(225,y+15,3,44,BLUE,r=1)
    if i: line(224,y,1176,y)
    dot(246,y+28); txt(258,y+33,name,14,weight=600); txt(242,y+56,port,11,MUTED)
    txt(420,y+33,exit,13); chevron(548,y+27)
    txt(420,y+55,'2 个入口共用' if i<2 else '独立出口',11,MUTED)
    txt(581,y+33,node,13); chevron(726,y+27); txt(581,y+55,'手动选择',11,MUTED)
    txt(762,y+33,'系统默认',12); txt(861,y+33,delay,12,GREEN)
    txt(940,y+28,'↑ 0 B/s',11,MUTED); txt(940,y+50,'↓ '+rate,11)
    toggle(1082,y+27); more(1149,y+36)
line(224,522,1176,522)
txt(242,544,'机场',12,weight=600); txt(282,544,'127.0.0.1:7893',12,MUTED); copy(397,531)
txt(451,544,'仅本机访问',11,MUTED); txt(1158,544,'编辑监听器',12,BLUE,anchor='end')
# History is secondary and visually quiet.
rect(224,574,952,199,'#fff',BORDER,6)
txt(242,601,'流量趋势',13,weight=600)
line(957,596,972,596,BLUE); txt(979,600,'下载',11,MUTED)
line(1023,596,1038,596,'#949494'); txt(1045,600,'上传',11,MUTED); txt(1158,600,'最近 60 秒',11,MUTED,anchor='end')
graph(243,622,914,111)
txt(243,754,'60 秒前',10,MUTED); txt(1157,754,'现在',10,MUTED,anchor='end')
save('overview-a-compact')

# B: master/detail; the overview works as a small desktop control center.
shell()
txt(224,117,'3 个入口运行中',13,GREEN)
txt(380,117,'↑ 0 B/s',13,MUTED); txt(490,117,'↓ 444 B/s',13)
txt(1176,117,'累计 317.3 MB',12,MUTED,anchor='end')
line(224,139,1176,139)
txt(224,176,'监听器',16,weight=600); button(378,154,82,'＋ 新建',True)
rect(224,193,236,579,'#fff',BORDER,6)
rect(236,205,212,32,'#fafafa','#dedede',4)
add('<circle cx="250" cy="219" r="4" fill="none" stroke="#777"/>'); line(253,222,257,226,'#777'); txt(266,227,'搜索名称或端口',12,'#888')
for i,(name,port,node,rate) in enumerate([('机场','7893 · SOCKS5','SG 新加坡01','359 B/s'),('浏览器','7894 · 混合','SG 新加坡01','0 B/s'),('下载器','7895 · SOCKS5','JP 东京02','85 B/s')]):
    y=251+i*103
    if i==0: rect(232,y,220,94,'#eaf3fc',r=4); rect(233,y+16,3,61,BLUE,r=1.5)
    dot(249,y+25); txt(261,y+30,name,14,weight=600)
    txt(248,y+53,port,11,MUTED); txt(248,y+77,node,11,MUTED); txt(438,y+30,'↓ '+rate,10,MUTED,anchor='end')
    if i<2: line(244,y+99,440,y+99,'#ededed')
txt(244,751,'全部 3 个 · 无异常',11,MUTED)
rect(476,154,700,618,'#fff',BORDER,6)
dot(500,186); txt(513,192,'机场',20,weight=600)
smallpill(572,173,'运行中',GREEN)
txt(500,218,'SOCKS5 · 127.0.0.1:7893',12,MUTED)
button(978,173,86,'复制地址'); toggle(1082,181); more(1149,189)
line(476,239,1176,239)
txt(500,272,'连接配置',14,weight=600)
txt(500,308,'出口',12,MUTED); field(500,319,310,'机场出口')
txt(834,308,'当前节点',12,MUTED); field(834,319,318,'SG 新加坡01')
rect(500,365,652,37,'#f0f5fb',r=4)
dot(516,384,BLUE,5); txt(516,388,'i',10,'#fff',600,'middle')
txt(530,388,'此出口由「机场」「浏览器」共用，切换节点会同时生效。',11,BLUE)
for x,label,value in [(500,'出口网卡','系统默认'),(732,'节点策略','手动选择'),(964,'出口延迟','182 ms')]:
    txt(x,433,label,11,MUTED); txt(x,458,value,14,GREEN if label=='出口延迟' else INK,500)
line(500,480,1152,480)
txt(500,512,'实时流量',14,weight=600)
txt(1152,512,'最近 60 秒',11,MUTED,anchor='end')
txt(500,548,'↑ 0 B/s',16,MUTED,500); txt(636,548,'↓ 359 B/s',16,BLUE,500)
graph(500,564,652,101)
txt(500,688,'累计上传 150.7 MB',11,MUTED); txt(727,688,'累计下载 150.3 MB',11,MUTED)
line(476,709,1176,709)
txt(500,746,'仅本机访问 · 配置已生效',11,MUTED); button(1056,727,96,'编辑监听器')
save('overview-b-detail')
