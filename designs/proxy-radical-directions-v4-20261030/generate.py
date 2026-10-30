"""Three intentionally different Routa design directions. Design artifacts only."""
from drawing import *

ROWS=[
 dict(name='机场',port='7893',proto='SOCKS5',node='新加坡01',mode='手动选择',nic='系统默认网卡',delay='182 ms',up='24 KB/s',down='2.8 MB/s',state='运行中',kind='green',on=True),
 dict(name='开发工具',port='7894',proto='Mixed',node='新加坡01',mode='手动选择',nic='以太网',delay='182 ms',up='8 KB/s',down='420 KB/s',state='运行中',kind='green',on=True),
 dict(name='下载专用',port='7895',proto='HTTP',node='美国西部02',mode='自动测速',nic='系统默认网卡',delay='356 ms',up='126 KB/s',down='8.2 MB/s',state='运行中',kind='green',on=True),
 dict(name='浏览器',port='7896',proto='Mixed',node='东京01',mode='手动选择',nic='系统默认网卡',delay='58 ms',up='12 KB/s',down='1.4 MB/s',state='运行中',kind='green',on=True),
 dict(name='内网直连',port='7897',proto='SOCKS5',node='直连',mode='直连',nic='Wi-Fi',delay='—',up='—',down='—',state='已关闭',kind='gray',on=False),
 dict(name='备用线路',port='7898',proto='Mixed',node='香港02',mode='手动选择',nic='系统默认网卡',delay='—',up='—',down='—',state='启动失败',kind='red',on=True),
]


def frame(s,white=False,title='代理'):
    s.rect(0,0,1280,768,BG,'#d8d8d8',8)
    s.rect(1,1,1278,31,'#fafafa',r=7);s.line(0,32,1280,32,'#e8e8e8')
    s.rect(15,10,12,12,BLUE,r=3);s.txt(36,22,'Routa',11,MUTED)
    s.line(1159,16,1169,16,MUTED);s.rect(1202,11,9,9,'none',MUTED,0);s.path('M1247 11l9 9m0-9-9 9',MUTED,1)
    if white:s.rect(184,33,1095,734,'white',r=0)
    s.rect(20,56,26,26,BLUE,r=6);s.icon('RouteOutlined',25,61,'white',16);s.txt(57,75,'Routa',17,INK,600)
    for i,(icon,label) in enumerate([('SpaceDashboardOutlined','概览'),('SwapHorizOutlined','代理'),('AltRouteOutlined','进程分流'),('LanguageOutlined','订阅'),('DnsOutlined','节点')]):
        y=109+i*46
        if i==1:s.rect(8,y,168,40,'#e5e9ed',r=5);s.rect(8,y+11,3,18,BLUE,r=1.5)
        s.icon(icon,24,y+10,INK if i==1 else MUTED,20);s.txt(60,y+26,label,14,INK,600 if i==1 else 400)
    s.icon('SettingsOutlined',24,683,MUTED,20);s.txt(60,699,'设置',14);s.txt(24,744,'v0.2.0',11,DIM)
    s.txt(216,88,title,26,INK,600);s.dot(1015,78,3,GREEN);s.txt(1027,82,'内核运行中',12,MUTED)
    s.button(1136,61,112,'新增代理',True,'AddOutlined')


def chip(s,x,y,w,label,active=False):
    s.rect(x,y,w,30,'#e8f2fc' if active else 'white',None if active else '#e0e0e0',5)
    s.txt(x+w/2,y+20,label,12,BLUE if active else MUTED,500 if active else 400,'middle')


def stage_single(s):
    frame(s,True)
    s.txt(216,147,'机场',14,BLUE,600);s.line(216,160,272,160,BLUE,2)
    s.icon('AddOutlined',303,132,MUTED,18)
    s.line(216,172,1248,172,'#eceef0')
    s.txt(216,220,'机场',18,INK,600);s.badge(274,201,'运行中')
    s.txt(1146,220,'启用',12,MUTED);s.toggle(1202,207)
    s.txt(252,260,'本地入口',12,MUTED)
    chip(s,252,278,78,'SOCKS5')
    s.txt(247,382,':7893',70,INK,300)
    s.txt(252,420,'127.0.0.1',13,MUTED);s.icon('ContentCopyOutlined',345,406,DIM,17)
    s.txt(944,284,'当前节点',12,MUTED)
    s.txt(944,334,'新加坡01',35,INK,600)
    s.txt(944,367,'系统默认网卡',13,MUTED)
    s.button(944,395,160,'切换节点',True,'SwapHorizOutlined',h=36)
    s.path('M478 350 C560 350 578 414 650 414H754C830 414 842 318 910 318',BLUE,2.2)
    s.dot(478,350,5,BLUE);s.path('M902 313l8 5-8 5',BLUE,1.8)
    s.rect(648,399,108,30,'white','#b8d7f1',15);s.txt(702,419,'手动选择',12,BLUE,500,'middle')
    s.line(216,512,1248,512,'#e6e9ec')
    for x,label,value in [(252,'延迟','182 ms'),(454,'上传','↑ 24 KB/s'),(672,'下载','↓ 2.8 MB/s'),(894,'本次累计','1.24 GB')]:
        s.txt(x,559,value,22,INK,500);s.txt(x,590,label,12,MUTED)
    s.icon('RefreshOutlined',337,543,DIM,18)
    s.link(1120,558,'编辑配置',size=12);s.more(1224,552)
    s.txt(216,736,'1 个代理 · 1 个运行中',11,MUTED)


def graph_input(s,y,name,port,proto):
    s.rect(240,y,232,104,'#fafbfc','#e0e4e8',8)
    s.txt(260,y+28,name,14,INK,600);s.dot(380,y+23,2.5,GREEN);s.txt(390,y+27,'运行中',10,GREEN);s.more(448,y+23)
    s.txt(260,y+77,':'+port,32,INK,400);s.txt(447,y+76,proto,11,MUTED,anchor='end')


def stage_multiple(s):
    frame(s,True)
    chip(s,216,128,108,'连接总览',True);chip(s,334,128,82,'逐个查看')
    s.rect(976,128,272,32,'white','#d4d4d4',4);s.icon('SearchOutlined',986,136,DIM,17);s.txt(1014,149,'搜索入口或节点',12,DIM)
    s.txt(240,196,'本地入口',12,MUTED);s.txt(960,196,'当前去向',12,MUTED)
    # Shared target is deliberately explicit: these two ports reference one exit.
    for y,name,port,proto in [(220,'机场','7893','SOCKS5'),(370,'开发工具','7894','Mixed'),(520,'下载专用','7895','HTTP')]:graph_input(s,y,name,port,proto)
    s.path('M472 272C640 272 674 353 960 353',BLUE,2)
    s.path('M472 422C640 422 674 353 960 353',BLUE,2)
    s.path('M472 572C660 572 700 584 960 584',BLUE,2)
    for yy in [353,584]:s.path(f'M950 {yy-5}l10 5-10 5',BLUE,1.6)
    for yy,label in [(338,'手动选择'),(569,'自动测速')]:
        s.rect(670,yy,108,30,'white','#b8d7f1',15);s.txt(724,yy+20,label,12,BLUE,500,'middle')
    s.rect(960,283,264,140,'#f6faff','#bdd7ef',8)
    s.txt(984,319,'新加坡01',24,INK,600);s.txt(984,346,'系统默认网卡',11,MUTED)
    s.txt(984,371,'182 ms · 共用 2 个代理',11,MUTED);s.icon('InfoOutlined',1181,357,DIM,15)
    s.button(984,385,216,'切换节点',False,'SwapHorizOutlined',h=28)
    s.rect(960,520,264,132,'#f6faff','#bdd7ef',8)
    s.txt(984,555,'美国西部02',22,INK,600);s.txt(984,580,'系统默认网卡 · 356 ms',11,MUTED)
    s.button(984,604,216,'测速',h=28,icon='RefreshOutlined')
    s.txt(216,736,'3 个代理 · 2 个去向 · 3 个运行中',11,MUTED)


def console(s,many=True):
    frame(s)
    s.rect(216,124,700,40,'white','#c8cdd3',5);s.icon('SearchOutlined',230,134,MUTED,20)
    s.txt(262,150,'搜索代理、端口或节点',14,DIM)
    s.rect(844,132,58,24,'#f2f3f4',r=4);s.txt(873,149,'Ctrl K',11,MUTED,anchor='middle')
    s.button(930,128,148,'全部 6' if many else '全部 1',icon='FilterListOutlined')
    s.button(1090,128,158,'全部测速',icon='RefreshOutlined')
    rows=ROWS if many else ROWS[:1]
    h=40+68*len(rows)
    s.rect(216,188,1032,h,'white','#dedfe2',6)
    s.rect(217,189,1030,39,'#fafafa',r=5);s.rect(217,216,1030,12,'#fafafa',r=0)
    for x,label in [(244,'端口 / 协议'),(382,'代理'),(526,'当前去向'),(756,'策略'),(902,'延迟 / 速率'),(1120,'状态')]:s.txt(x,213,label,11,MUTED)
    for i,r in enumerate(rows):
        y=228+i*68
        if i==0:s.rect(217,y,1030,68,'#e9f3ff',r=0);s.rect(217,y+8,3,52,BLUE,r=1.5)
        if i:s.line(217,y,1247,y)
        c=INK if r['on'] else DIM
        s.txt(244,y+29,r['port'],19,c,500,extra='font-family="Consolas,Segoe UI,sans-serif"')
        s.txt(244,y+50,r['proto'],10,MUTED)
        s.txt(382,y+30,r['name'],14,c,600 if i==0 else 400)
        s.txt(526,y+30,'未生效' if r['kind']=='red' else r['node'],14,DIM if r['kind']=='red' else c,500)
        s.txt(526,y+51,'端口被占用' if r['kind']=='red' else r['nic'],10,RED if r['kind']=='red' else MUTED)
        s.txt(756,y+30,r['mode'],12,MUTED)
        s.txt(902,y+29,r['delay'],13,AMBER if i==2 else MUTED)
        s.txt(902,y+50,f"↑ {r['up']}  ↓ {r['down']}",10,MUTED)
        s.badge(1120,y+20,r['state'],r['kind']);s.more(1222,y+33)
    s.rect(216,656,1032,64,'white','#d4dbe3',6)
    s.txt(236,681,'机场  :7893 → 新加坡01',13,INK,600);s.txt(236,704,'手动选择 · 系统默认网卡',11,MUTED)
    s.txt(652,695,'启用',11,MUTED);s.toggle(696,681)
    s.button(764,672,168,'切换节点  Enter',True,h=32)
    s.button(944,672,132,'编辑配置  E',h=32)
    s.button(1088,672,112,'测速  T',h=32);s.more(1224,688)
    s.txt(216,748,'6 个代理 · 4 个运行中 · 1 个关闭 · 1 个异常' if many else '1 个代理 · 1 个运行中',11,MUTED)


def tile(s,x,y,r,w=328,h=244,selected=False):
    failed=r['kind']=='red';c=INK if r['on'] and not failed else DIM
    s.rect(x,y,w,h,'white',BLUE if selected else '#dedfe2',8,extra='stroke-width="1.3"' if selected else '')
    s.txt(x+20,y+27,r['name'],14,INK,600);s.badge(x+106,y+10,r['state'],r['kind'])
    s.toggle(x+w-82,y+14,r['on']);s.more(x+w-25,y+23)
    s.txt(x+20,y+88,r['port'],48,c,400,extra='font-family="Segoe UI,Microsoft YaHei UI,sans-serif"')
    s.txt(x+w-20,y+82,r['proto'],11,MUTED,anchor='end')
    s.path(f'M{x+28} {y+104}v16m-4-4 4 4 4-4',DIM if failed or not r['on'] else BLUE,1.6)
    s.txt(x+49,y+126,'未生效' if failed else r['node'],18,c,600)
    s.txt(x+20,y+150,'配置：'+r['node']+' · 端口被占用' if failed else r['mode']+' · '+r['nic'],11,RED if failed else MUTED)
    s.txt(x+20,y+178,f"{r['delay']}  ·  ↑ {r['up']}  ·  ↓ {r['down']}",11,MUTED)
    s.line(x+1,y+h-47,x+w-1,y+h-47)
    s.link(x+20,y+h-18,'编辑配置',color=MUTED,size=12)
    s.link(x+w-90,y+h-18,'切换节点' if r['mode']=='手动选择' else '复制地址' if r['mode']=='直连' else '测速',size=12)


def matrix(s,many=True):
    frame(s)
    for x,label,w,active in [(216,'全部 6' if many else '全部 1',76,True),(304,'运行中 4' if many else '运行中 1',92,False),(408,'异常 1' if many else '异常 0',76,False)]:chip(s,x,128,w,label,active)
    s.rect(966,128,282,32,'white','#d4d4d4',4);s.icon('SearchOutlined',976,136,DIM,17);s.txt(1004,149,'搜索名称、端口或节点',12,DIM)
    if many:
        for i,r in enumerate(ROWS):tile(s,216+(i%3)*352,188+(i//3)*264,r,selected=i==0)
    else:
        # Deliberately centered and capped; the port panel does not fill the canvas.
        x,y,w,h=512,196,440,338;r=ROWS[0]
        s.rect(x,y,w,h,'white','#cbd5df',10)
        s.txt(x+28,y+36,'机场',18,INK,600);s.badge(x+88,y+18,'运行中');s.toggle(x+w-96,y+23);s.more(x+w-32,y+32)
        s.txt(x+28,y+133,'7893',78,INK,300);s.txt(x+w-28,y+117,'SOCKS5',12,MUTED,anchor='end')
        s.txt(x+30,y+166,'127.0.0.1',12,MUTED);s.icon('ContentCopyOutlined',x+120,y+153,DIM,16)
        s.path(f'M{x+38} {y+185}v20m-5-5 5 5 5-5',BLUE,1.7)
        s.txt(x+66,y+213,'新加坡01',24,INK,600)
        s.txt(x+28,y+247,'手动选择 · 系统默认网卡',12,MUTED)
        s.txt(x+28,y+275,'182 ms  ·  ↑ 24 KB/s  ·  ↓ 2.8 MB/s',12,MUTED)
        s.line(x+1,y+h-45,x+w-1,y+h-45)
        s.link(x+28,y+h-16,'编辑配置',color=MUTED,size=12);s.link(x+w-108,y+h-16,'切换节点',size=12)
    s.txt(216,736,'6 个代理 · 4 个运行中 · 1 个关闭 · 1 个异常' if many else '1 个代理 · 1 个运行中',11,MUTED)


META={
 'stage':dict(code='A',title='连接舞台',subtitle='一次专注一条连接，多条连接用拓扑呈现',notes=[('页面中心：流量关系',['单代理用大字号入口与当前节点。','多代理以连线呈现共享去向，','能看出哪些端口实际共用出口。']),('核心交互：目的地操作',['切换按钮直接放在目的地旁。','共用去向明确标记影响范围，','提交时就近提醒受影响代理。']),('动效：沿路线确认结果',['只在切换期间出现移动光点。','当前去向保持可见，','生效后才替换目的地名称。'])],bottom=[('更像连接控制界面','适合少量代理、重视去向关系。'),('跨度在于图形组织','多代理可合并共享出口，减少重复信息。'),('需要控制复杂度','连接多时收敛为分组，避免交叉线过密。')]),
 'console':dict(code='B',title='代理终端',subtitle='以搜索、焦点和就近操作服务高频使用',notes=[('页面中心：可操作的行',['端口、名称、去向、策略和状态，','使用固定列与紧凑行对齐。','常用操作收敛到固定底部动作栏。']),('核心交互：选中后行动',['上下键定位，Enter 打开节点候选。','候选菜单靠近去向单元格，','支持搜索与键盘确认。']),('动效：只更新目标字段',['焦点快速移动，不移动整行。','候选菜单轻微淡入，','确认后只更新节点与相关指标。'])],bottom=[('更像桌面效率工具','适合大量代理与频繁切换。'),('跨度在于输入方式','搜索、键盘与固定动作栏成为操作骨架。'),('需要照顾可发现性','按钮保留可见，快捷键作为补充。')]),
 'matrix':dict(code='C',title='入口矩阵',subtitle='用端口作为视觉锚点，像操作一组独立入口',notes=[('页面中心：端口身份',['端口是最大的视觉元素。','入口向下指向当前节点，','名称、策略和性能围绕端口组织。']),('核心交互：聚焦一块面板',['单代理居中且限制宽度。','多代理形成可扫描的矩阵，','点击切换时聚焦展开当前面板。']),('动效：展开，再回到原位',['背景降低对比，其余入口不重排。','确认后面板收回，','节点与状态在原位置更新。'])],bottom=[('更像独立设备面板','适合按端口分配应用的使用习惯。'),('跨度在于信息权重','端口优先，连接关系转为纵向。'),('需要注意名称识别','代理名称始终可见，不能只靠端口记忆。')])
}


def board(key,ui):
    meta=META[key];s=SVG(1680,1120,'Routa 大跨度方向 '+meta['code']+' '+meta['title'])
    s.rect(0,0,1680,1120,'#f6f7f9',r=0)
    s.txt(32,39,'方向 '+meta['code']+' / 组织方式探索',12,BLUE,600)
    s.txt(32,79,meta['title'],31,INK,600)
    s.txt(1648,43,'Routa · 第四轮发散',12,MUTED,anchor='end');s.txt(1648,75,meta['subtitle'],12,MUTED,anchor='end')
    s.rect(32,112,1280,768,'white',r=8,extra='filter="url(#shadow)"')
    s.group(32,112);s.p.append(ui[ui.index('<g font-family='):ui.rindex('</svg>')]);s.end()
    for i,(title,lines) in enumerate(meta['notes']):
        yy=145+i*230;s.num(1352,yy+12,i+1);s.txt(1376,yy+17,title,16,INK,600);s.lines(1340,yy+58,lines,13,MUTED,25)
    for x,(title,body) in zip([32,578,1124],meta['bottom']):
        s.rect(x,912,524,162,'white','#e1e4e8',8);s.txt(x+20,948,title,15,INK,600);s.txt(x+20,985,body,12,MUTED)
    s.txt(32,1103,'设计评审产物 · 所有内容均为虚构数据 · 单代理与多代理变体可在本地查看器中切换',11,DIM)
    s.txt(1648,1103,'方向 '+meta['code'],11,DIM,anchor='end')
    s.save({'stage':'01-connection-stage.svg','console':'02-proxy-console.svg','matrix':'03-port-matrix.svg'}[key])


def mini_spinner(s,x,y,f):
    s.p.append(f'<g transform="translate({x} {y}) rotate({f*45})">');s.path('M0 -6a6 6 0 1 1-5.2 3',BLUE,1.8);s.end()


def motion(f):
    s=SVG(1464,464,'三个方向的动效语言对比：连接确认、焦点操作、面板展开')
    s.rect(0,0,1464,464,'#f5f6f8',r=0)
    s.txt(24,31,'三种组织方式，三种反馈语言',22,INK,600)
    s.txt(1440,30,'前半成功 / 后半失败 · 虚构演示',12,MUTED,anchor='end')
    phase='idle' if f<8 or 40<=f<48 else 'pending' if 8<=f<20 or 48<=f<60 else 'success' if 20<=f<40 else 'failure'
    node='东京02' if phase=='success' else '新加坡01'
    for x,title,sub in [(24,'A 连接舞台','沿路径反馈，保持当前去向'),(504,'B 代理终端','焦点定位，就近操作'),(984,'C 入口矩阵','聚焦展开，原位返回')]:
        s.txt(x,64,title,18,INK,600);s.txt(x,86,sub,11,MUTED)
        s.rect(x,100,456,310,'white','#dfe3e8',8)
    # A: route-focused feedback.
    s.txt(44,132,'机场',15,INK,600);s.badge(90,115,'运行中')
    s.txt(44,170,'入口 · SOCKS5',10,MUTED);s.txt(44,217,':7893',33,INK,300)
    s.txt(326,170,'当前节点',10,MUTED);s.txt(326,216,node,22,INK,600)
    s.arrow(192,202,112);s.txt(248,232,'手动选择',10,MUTED,anchor='middle')
    if phase=='pending':s.dot(192+(f%8)/7*112,202,3,BLUE)
    s.button(326,253,132,'重试' if phase=='failure' else '切换中…' if phase=='pending' else '切换节点',phase!='pending',h=30)
    if phase in ['pending','success','failure']:
        color,bg,msg={ 'pending':(BLUE,'#edf5fd','正在切换到东京02…'),'success':(GREEN,'#edf7ed','已切换到东京02'),'failure':(RED,'#fdf0ee','未生效，当前仍为新加坡01')}[phase]
        s.rect(44,309,416,40,bg,r=4);s.txt(58,334,msg,12,color)
    s.line(44,365,460,365);s.txt(44,388,'58 ms' if phase=='success' else '182 ms',11,MUTED);s.txt(166,388,'↑ 24 KB/s',11,MUTED);s.txt(304,388,'↓ 2.8 MB/s',11,MUTED)
    # B: focus plus a cell-anchored menu.
    s.rect(524,116,416,30,'#fafafa','#e1e4e8',4);s.icon('SearchOutlined',534,124,DIM,15);s.txt(560,136,'搜索代理或端口',11,DIM)
    for i,(port,name) in enumerate([('7893','机场'),('7894','开发工具'),('7895','下载专用')]):
        yy=161+i*54;selected=i==(0 if f<4 or 40<=f<44 else 1)
        if selected:s.rect(524,yy,416,52,'#e9f3ff',r=3);s.rect(524,yy+9,3,34,BLUE,r=1)
        s.txt(540,yy+31,port,17,INK,500,extra='font-family="Consolas,Segoe UI,sans-serif"');s.txt(604,yy+30,name,12,INK)
        s.txt(774,yy+30,node if i==1 else '新加坡01',14,BLUE if i==1 and phase=='success' else INK,500)
    s.line(524,340,940,340);s.txt(540,378,'↑↓ 定位',11,MUTED);s.txt(665,378,'Enter 切换',11,BLUE);s.txt(813,378,'E 编辑配置',11,MUTED)
    if phase in ['pending','failure']:
        s.rect(690,245,250,137,'white','#cbd3dd',6,extra='filter="url(#shadow)"')
        s.txt(706,269,'切换节点',12,INK,600);s.rect(702,282,226,34,'#edf5fd',r=4)
        s.txt(716,304,'东京02',12,INK);s.txt(912,304,'58 ms',10,MUTED,anchor='end')
        if phase=='pending':mini_spinner(s,714,339,f);s.txt(731,344,'切换中…',11,BLUE)
        else:s.txt(706,340,'请求失败，保留当前节点。',11,RED)
        s.txt(912,366,'重试' if phase=='failure' else '等待确认',10,BLUE if phase=='failure' else MUTED,anchor='end')
    # C: fixed underlying tiles plus an expanded foreground panel.
    overlay=phase in ['pending','failure'] or (phase=='success' and f<24)
    s.p.append('<g'+(' opacity=".32"' if overlay else '')+'>')
    for i,port in enumerate(['7893','7894','7895']):
        xx=1004+i*140;s.rect(xx,164,128,192,'white',BLUE if i==0 else '#dedfe2',6)
        s.txt(xx+14,190,['机场','开发工具','下载专用'][i],12,INK,600)
        s.txt(xx+14,236,port,32,INK,400);s.path(f'M{xx+18} 251v15m-4-4 4 4 4-4',BLUE,1.4)
        s.txt(xx+14,293,node if i==0 else '新加坡01',13,INK,500);s.txt(xx+14,330,'切换节点',11,BLUE)
    s.end()
    if overlay:
        start=8 if f<40 else 48
        t=min(1,max(0,(f-start)/3))
        if phase=='success':t=max(0,1-(f-20)/3)
        scale=.4+.6*t;cx=1068+144*t;cy=260
        s.p.append(f'<g transform="translate({cx} {cy}) scale({scale})" opacity="{.45+.55*t}">')
        s.rect(-194,-122,388,244,'white','#b7cce0',8,extra='filter="url(#shadow)"')
        s.txt(-174,-89,'7893',26,INK,400);s.txt(-84,-91,'切换节点',13,MUTED)
        s.rect(-174,-66,348,40,'#edf5fd',BLUE,4);s.txt(-158,-41,'东京02',13,INK,500);s.txt(157,-41,'58 ms',11,MUTED,anchor='end')
        if phase=='failure':s.rect(-174,-12,348,51,'#fdf0ee',r=4);s.txt(-158,10,'切换未生效，请重试。',12,RED);s.txt(-158,29,'当前仍为新加坡01，保留选择。',10,RED)
        else:mini_spinner(s,-160,11,f);s.txt(-143,16,'正在切换到东京02…',12,BLUE)
        s.line(-194,65,194,65);s.button(46,80,128,'重试' if phase=='failure' else '切换中…',phase=='failure',h=30)
        s.end()
    caption={'idle':'选择要操作的代理','pending':'提交中：三种布局均保持当前生效节点','success':'成功：确认生效后才更新去向','failure':'失败：保留当前节点与候选选择，原位重试'}[phase]
    s.txt(24,446,caption,12,RED if phase=='failure' else MUTED)
    return s


def main():
    views={}
    for key in ['stage','console','matrix']:
        for many in [False,True]:
            name=key+('-multiple' if many else '-single')
            s=SVG(1280,768,'Routa '+META[key]['title']+(' 多代理' if many else ' 单代理'))
            if key=='stage':stage_multiple(s) if many else stage_single(s)
            elif key=='console':console(s,many)
            else:matrix(s,many)
            views[name]=s.save(name+'.svg')
        board(key,views[key+('-single' if key=='stage' else '-multiple')])
    frames=ROOT/'motion-frames';frames.mkdir(exist_ok=True)
    for f in range(80):motion(f).save(f'motion-frames/{f:03}.svg')
    motion(28).save('motion-static.svg')
    template=(ROOT/'viewer.template.html').read_text(encoding='utf-8')
    (ROOT/'index.html').write_text(template.replace('/* VIEW_DATA */','const views='+json.dumps(views,ensure_ascii=False)+';'),encoding='utf-8')
    print('Created three concept boards, six complete screens and 80 animation frames.')


if __name__=='__main__':main()
