"""Generate editable vector mockups; does not import or modify application code."""
from pathlib import Path
from html import escape

ROOT = Path(__file__).parent
INK, MUTED, BLUE, GREEN = '#1b1b1b', '#616161', '#0067c0', '#0f7b0f'
parts = []

def add(s): parts.append(s)
def rect(x,y,w,h,fill='#ffffff',stroke='none',r=0,extra=''):
    add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill}" stroke="{stroke}" {extra}/>')
def text(x,y,s,size=14,fill=INK,weight=400,anchor='start'):
    add(f'<text x="{x}" y="{y}" font-size="{size}" fill="{fill}" font-weight="{weight}" text-anchor="{anchor}">{escape(str(s))}</text>')
def line(x,y,x2,y2,color='#e5e5e5',width=1):
    add(f'<path d="M{x} {y}L{x2} {y2}" fill="none" stroke="{color}" stroke-width="{width}"/>')
def dot(x,y,c=GREEN,r=3): add(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{c}"/>')
def path(d,c=INK,w=1.5): add(f'<path d="{d}" fill="none" stroke="{c}" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round"/>')
def chevron(x,y,c=MUTED): path(f'M{x} {y}l4 4 4-4',c,1.3)
def more(x,y):
    for dx in [-4,0,4]: dot(x+dx,y,MUTED,1.2)
def copy(x,y):
    rect(x+3,y+3,9,11,'none','#616161',1.5)
    path(f'M{x+8} {y}H{x+1}Q{x} {y} {x} {y+1}V{y+9}',MUTED,1.1)
def icon(name,x,y,c=INK):
    if name=='overview':
        for dx,dy,w,h in [(0,0,6,9),(10,0,6,5),(0,13,6,3),(10,9,6,7)]: rect(x+dx,y+dy,w,h,'none',c,0.5)
    elif name=='input':
        path(f'M{x+6} {y}h10v16H{x+6} M{x} {y+8}h10m-3-3 3 3-3 3',c)
    elif name=='apps':
        for dx in [0,6,12]:
            for dy in [0,6,12]: rect(x+dx,y+dy,3,3,c,r=0.5)
    elif name=='exit': path(f'M{x+8} {y+16}V{y+9}L{x+2} {y+3}m0 5V{y+3}h5 M{x+8} {y+9}l6-6m-5 0h5v5',c)
    elif name=='nodes':
        for a,b in [(8,1),(1,7),(15,7),(4,15),(12,15)]:
            line(x+8,y+8,x+a,y+b,c,1.2); add(f'<circle cx="{x+a}" cy="{y+b}" r="2" fill="#f3f3f3" stroke="{c}" stroke-width="1.4"/>')
        dot(x+8,y+8,c,2)
    elif name=='globe':
        add(f'<circle cx="{x+8}" cy="{y+8}" r="7.5" fill="none" stroke="{c}" stroke-width="1.4"/>')
        path(f'M{x+1} {y+8}h14M{x+8} {y}c-5 5-5 11 0 16m0-16c5 5 5 11 0 16',c,1.2)
    elif name=='settings':
        add(f'<circle cx="{x+8}" cy="{y+8}" r="5.5" fill="none" stroke="{c}" stroke-width="2.5" stroke-dasharray="3 2"/>')
        add(f'<circle cx="{x+8}" cy="{y+8}" r="2.3" fill="none" stroke="{c}" stroke-width="1.3"/>')
def button(x,y,w,label,primary=False):
    rect(x,y,w,34,BLUE if primary else '#fff','none' if primary else '#dedede',5)
    text(x+w/2,y+22,label,13,'#fff' if primary else INK,500,'middle')
def pill(x,y,label,blue=False):
    w=len(label)*11+16
    rect(x,y,w,22,'#eaf3fc' if blue else '#f3f3f3',r=4)
    text(x+8,y+15,label,11,BLUE if blue else MUTED)
def select(x,y,w,label,active=False):
    rect(x,y,w,34,'#fff',BLUE if active else '#d2d2d2',4)
    text(x+10,y+22,label,13)
    chevron(x+w-19,y+15)
def toggle(x,y,on=True):
    rect(x,y,34,18,BLUE if on else '#999999',r=9)
    dot(x+(25 if on else 9),y+9,'#fff',6)
def spark(x,y,w,h,c,seed):
    values=[.12,.16,.11,.17,.13,.15,.12,.20,.14,.13,.16,.18,.12,.14,.13,.16,.12,.19,.16,.12,.14,.16,.13,.12,.18,.14,.13,.17,.12,.16,.14,.13,.18,.13,.15,.13]
    if seed==1:
        for i,v in [(7,.62),(12,.85),(24,.38)]: values[i]=v
    else:
        for i,v in [(5,.22),(9,.3),(14,.25),(17,.38),(18,.76),(19,.51),(21,.62),(22,.43),(23,.8),(24,.58),(25,.67),(26,.36)]: values[i]=v
    points=[(x+i*w/(len(values)-1),y+h-v*h) for i,v in enumerate(values)]
    d='M'+'L'.join(f'{a:.1f} {b:.1f}' for a,b in points)
    add(f'<path d="{d}L{x+w} {y+h}L{x} {y+h}Z" fill="{c}" opacity=".07"/>')
    path(d,c,1.4)

def shell(shared=False):
    parts.clear()
    add('<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="832" viewBox="0 0 1200 832">')
    add('<defs><filter id="shadow" x="-20%" y="-20%" width="140%" height="150%"><feDropShadow dx="0" dy="6" stdDeviation="10" flood-color="#000000" flood-opacity=".14"/></filter></defs>')
    add('<g font-family="Microsoft YaHei UI,Microsoft YaHei,Segoe UI,sans-serif">')
    rect(0,0,1200,832,'#f3f3f3','#d7d7d7',8)
    rect(10,9,14,14,BLUE,r=2); text(30,20,'Routa',12)
    line(1080,16,1090,16,'#1b1b1b'); rect(1127,11,9,9,'none','#1b1b1b'); path('M1173 11l9 9m0-9-9 9',INK,1)
    rect(20,48,25,25,BLUE,r=5)
    path('M28 54v10q0 3 3 3h5M36 55v8m-2-2 2 2 2-2','#fff',1.5)
    text(55,66,'Routa',16,weight=600)
    text(24,103,'运行',11,MUTED)
    for name,label,y,selected in [('overview','概览',114,True),('input','监听器',162,False),('apps','进程分流',210,False)]:
        if selected:
            rect(8,y,184,42,'#e5e5e5',r=5); rect(9,y+10,3,22,BLUE,r=1.5)
        icon(name,28,y+13); text(60,y+27,label,14,weight=600 if selected else 400)
    text(24,285,'资源',11,MUTED)
    for name,label,y in [('exit','出口',296),('nodes','节点与订阅',344)]:
        icon(name,28,y+13); text(60,y+27,label)
    text(24,419,'扩展',11,MUTED); icon('globe',28,443); text(60,457,'Tailscale')
    icon('settings',28,746); text(60,760,'设置'); text(24,807,'v0.2.0',11,MUTED)
    text(224,80,'概览',25,weight=600)
    text(224,104,'查看代理入口与当前连接路径',12,MUTED)
    rect(1004,56,172,34,'#ffffff','#e1e1e1',5)
    dot(1020,73); text(1032,78,'内核运行中',12); more(1157,73)
    # Summary cards: the live chart is consolidated; units never wrap.
    rect(224,125,410,120,'#fff','#e1e1e1',7)
    text(242,150,'实时流量',12,MUTED)
    line(429,163,429,224,'#eeeeee')
    text(242,177,'↑ 上传',11,MUTED); text(447,177,'↓ 下载',11,MUTED)
    text(242,204,'0',25,weight=600); text(267,203,'B/s',13,MUTED)
    text(447,204,'444',25,weight=600); text(504,203,'B/s',13,MUTED)
    spark(242,211,172,22,BLUE,1); spark(447,211,168,22,GREEN,2)
    rect(646,125,270,120,'#fff','#e1e1e1',7)
    text(664,150,'累计流量',12,MUTED); more(895,146)
    text(664,179,'↑',14,BLUE); text(685,180,'153.1 MB',19,weight=600)
    text(664,212,'↓',14,GREEN); text(685,213,'164.2 MB',19,weight=600)
    rect(928,125,248,120,'#fff','#e1e1e1',7)
    text(946,150,'入口状态',12,MUTED)
    text(946,187,'3 / 3' if shared else '1 / 1',25,weight=600)
    text(1014,186,'运行中',13,GREEN)
    dot(950,216,GREEN,3); text(960,220,'所有入口均已生效',12,MUTED)
    text(224,289,'监听器',18,weight=600)
    text(294,288,'3 个' if shared else '1 个',12,MUTED)
    button(1072,265,104,'＋ 新建监听器',True)
    text(224,316,'修改后自动应用。切换共用出口的节点时，将影响所有关联入口。',12,MUTED)
    height=44+(3 if shared else 1)*132
    rect(224,333,952,height,'#fff','#dedede',7)
    rect(225,334,950,42,'#fafafa',r=6)
    rect(225,357,950,19,'#fafafa')
    for x,label in [(242,'代理入口'),(454,'当前路由'),(912,'实时流量'),(1038,'操作')]: text(x,360,label,12,MUTED)
    line(224,376,1176,376)
    row(376,'机场',7893,'SOCKS5','机场出口','SG 新加坡01','182',shared,True)
    if shared:
        row(508,'浏览器',7894,'HTTP + SOCKS5','机场出口','SG 新加坡01','182',True,True)
        row(640,'下载器',7895,'SOCKS5','下载专用','JP 东京02','96',False,True)
    text(224,533 if not shared else 797,'提示：节点与网卡不可用时，相关入口会停用；恢复后自动恢复。',11,MUTED)

def row(y,name,port,protocol,exit,node,delay,shared,on):
    if y>376: line(224,y,1176,y)
    dot(246,y+30,GREEN,3.5); text(258,y+35,name,15,weight=600)
    text(242,y+63,f'127.0.0.1:{port}',13)
    copy(417,y+50)
    text(242,y+89,protocol,11,MUTED)
    text(242,y+111,'仅本机 · 已生效',11,GREEN)
    for x,label in [(454,'出口'),(607,'节点'),(794,'网卡')]: text(x,y+25,label,11,MUTED)
    select(454,y+37,122,exit)
    text(585,y+59,'→',14,'#8c8c8c')
    select(607,y+37,158,node)
    text(774,y+59,'→',14,'#8c8c8c')
    text(794,y+59,'系统默认',13)
    pill(454,y+84,'手动选择')
    text(532,y+100,'出口延迟',11,MUTED); text(589,y+100,delay+' ms',12,GREEN,600)
    if shared: pill(666,y+84,'共用 · 2 个入口',True)
    text(912,y+53,'↑ 0 B/s',12,MUTED); text(912,y+78,'↓ 359 B/s',12)
    toggle(1038,y+45,on); text(1080,y+59,'启用',11,MUTED); more(1148,y+54)

def save(name):
    add('</g></svg>'); (ROOT/(name+'.svg')).write_text('\n'.join(parts),encoding='utf-8')

shell(False); save('overview-default')
shell(True)
# Node picker: shared scope is shown before a selection is made.
rect(588,441,356,302,'#fff','#dddddd',7,'filter="url(#shadow)"')
text(606,467,'切换节点',14,weight=600); text(926,466,'机场出口',11,MUTED,anchor='end')
rect(602,480,328,61,'#eef5fc',r=5)
dot(615,498,BLUE,6); text(615,502,'i',11,'#fff',600,'middle')
text(629,499,'此出口由 2 个入口共用',12,BLUE,500)
text(629,520,'切换后将同时影响：机场、浏览器',11,BLUE)
rect(602,551,328,31,'#fff','#d6d6d6',4)
add('<circle cx="616" cy="565" r="4" fill="none" stroke="#777" stroke-width="1.2"/>'); line(619,568,623,572,'#777',1.2)
text(631,571,'搜索节点',12,'#8a8a8a')
rect(596,591,340,37,'#eaf3fc',r=3); path('M607 609l3 3 6-7',BLUE,1.4)
text(626,615,'SG 新加坡01',13); text(924,615,'182 ms',12,GREEN,anchor='end')
text(626,652,'JP 东京02',13); text(924,652,'96 ms',12,GREEN,anchor='end')
text(626,689,'HK 香港03',13); text(924,689,'48 ms',12,GREEN,anchor='end')
line(588,705,944,705); text(606,728,'复制为独立出口…',12,BLUE)
save('overview-shared-picker')
