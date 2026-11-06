"""Routa route design study, v2. All writes stay inside this design directory."""
from drawing import *
from math import cos, sin, pi


def single(s):
    shell(s);toolbar(s)
    s.rect(216,188,872,216,'white',LINE,8)
    s.txt(240,220,'机场',18,INK,600);s.badge(294,201,'运行中')
    s.toggle(995,207,True,'toggle');s.more(1054,216)
    s.txt(240,253,'入口',11,MUTED)
    s.txt(504,253,'当前生效节点',11,MUTED)
    s.txt(240,286,'SOCKS5 :7893',22,INK,500)
    s.icon('ContentCopyOutlined',415,270,DIM,16)
    s.arrow(452,278,26)
    s.txt(504,286,'新加坡01',24,INK,600,extra='data-slot="current-node"')
    s.button(932,260,132,'切换节点',False,'SwapHorizOutlined',action='switch')
    s.txt(240,320,'手动选择 · 系统默认网卡',12,MUTED)
    s.rect(217,340,870,63,'#fafafa',r=7);s.rect(217,340,870,6,'#fafafa',r=0)
    s.line(217,340,1087,340)
    s.txt(240,379,'延迟',11,MUTED);s.txt(274,379,'182 ms',13,MUTED,extra='data-slot="delay-current"')
    s.icon('RefreshOutlined',327,366,DIM,16)
    s.line(358,362,358,383)
    s.txt(378,379,'↑ 24 KB/s',13,MUTED)
    s.txt(512,379,'↓ 2.8 MB/s',13,MUTED)
    s.txt(668,379,'累计 1.24 GB',11,DIM)
    s.link(991,379,'编辑配置','edit',size=12)
    s.txt(216,729,'1 个代理 · 1 个运行中',11,MUTED)


ROWS=[
 dict(name='机场',state='运行中',kind='green',proto='SOCKS5',port=7893,node='新加坡01',meta='手动选择 · 系统默认网卡',delay='182 ms',color=MUTED,rates='↑ 24 KB/s   ↓ 2.8 MB/s',on=True,action='切换节点'),
 dict(name='开发工具',state='运行中',kind='green',proto='Mixed',port=7894,node='按规则分流',meta='默认去向：东京01 · 以太网',delay='58 ms',color=MUTED,rates='↑ 8 KB/s     ↓ 420 KB/s',on=True,action='查看规则'),
 dict(name='下载专用',state='运行中',kind='green',proto='HTTP',port=7895,node='美国西部02',meta='自动测速 · 系统默认网卡',delay='356 ms',color=AMBER,rates='↑ 126 KB/s  ↓ 8.2 MB/s',on=True,action='测速'),
 dict(name='内网直连',state='已关闭',kind='gray',proto='SOCKS5',port=7896,node='直连',meta='直连 · Wi-Fi',delay='—',color=DIM,rates='↑ —            ↓ —',on=False,action='复制地址'),
 dict(name='备用线路',state='启动失败',kind='red',proto='Mixed',port=7897,node='未生效',meta='配置：香港02 · 端口被占用',delay='—',color=DIM,rates='↑ —            ↓ —',on=True,action='切换节点'),
]


def multiple(s):
    shell(s);toolbar(s,True)
    s.rect(216,188,1032,516,'white',LINE,8)
    s.rect(217,189,1030,35,'#fafafa',r=7);s.rect(217,216,1030,8,'#fafafa',r=0)
    for x,label in [(236,'代理 / 状态'),(412,'入口 → 当前去向'),(824,'性能'),(1048,'操作')]:s.txt(x,211,label,11,MUTED)
    for i,r in enumerate(ROWS):
        y=224+i*96;c=INK if r['on'] else DIM
        s.p.append(f'<g data-row="{i}" data-proxy="{r["name"]}">')
        s.line(217,y,1247,y)
        if i==4:s.rect(217,y+13,2,69,RED,r=1)
        s.txt(236,y+32,r['name'],16,c,600);s.badge(236,y+47,r['state'],r['kind'])
        s.txt(412,y+33,f"{r['proto']} :{r['port']}",15,c,500)
        s.arrow(552,y+27,24,BLUE if i<3 else DIM)
        s.txt(594,y+33,r['node'],17,DIM if i==4 else c,600,extra=f'data-slot="node-{i}"')
        s.txt(412,y+63,r['meta'],12,RED if i==4 else MUTED)
        s.txt(824,y+32,r['delay'],13,r['color'],500,extra=f'data-slot="delay-{i}"')
        s.txt(824,y+62,r['rates'],11,MUTED)
        action={'切换节点':'switch','查看规则':'edit','测速':'test','复制地址':'copy'}[r['action']]
        s.link(1048,y+32,r['action'],action)
        s.link(1048,y+63,'编辑配置','edit',color=MUTED,size=12)
        s.toggle(1160,y+18,r['on'],'toggle');s.more(1220,y+55)
        s.end()
    s.txt(216,736,'5 个代理 · 3 个运行中 · 1 个异常',11,MUTED)
    s.link(1168,736,'全部测速','test',size=12)


def board_header(s,no,title,subtitle):
    s.rect(0,0,1680,1120,'#f6f7f9',r=0)
    s.txt(32,38,'ROUTA  /  ROUTE STUDY  /  V2',11,BLUE,600,extra='letter-spacing="1.4"')
    s.txt(32,78,title,29,INK,600)
    s.txt(1648,42,'资深设计视角 · 信息结构 × 状态 × 动效',12,MUTED,anchor='end')
    s.txt(1648,74,subtitle,12,MUTED,anchor='end')
    s.txt(32,1104,'独立设计评审稿 · 虚构数据 · 标注位于应用画面之外',11,DIM)
    s.txt(1648,1104,no+' / 02',11,DIM,anchor='end')


def place_ui(s,ui):
    s.rect(32,112,1280,768,'white',r=8,extra='filter="url(#shadow)"')
    s.group(32,112);s.p.append(ui[ui.index('<g font-family='):ui.rindex('</svg>')]);s.end()


def note(s,y,no,title,lines):
    s.num(1352,y+12,no);s.txt(1376,y+17,title,16,INK,600)
    s.lines(1340,y+55,lines,13,MUTED,25)


def single_board(ui):
    s=SVG(1680,1120,'Routa 专业设计方案：单代理路线卡片')
    board_header(s,'01','把“入口 → 当前去向”作为视觉中心','单代理：紧凑路线卡片 / 872 × 216 px')
    place_ui(s,ui)
    note(s,141,1,'让去向一眼可读',['名称和状态负责识别。','协议、端口、箭头、节点连续排布；','“当前生效节点”消除状态歧义。','节点名称保持深色，蓝色用于行动。'])
    note(s,377,2,'操作按频率分层',['切换节点：紧邻去向，直接可达。','复制：靠近入口地址。','测速：靠近延迟值。','编辑：底栏右侧；删除：更多菜单。'])
    note(s,613,3,'性能安静、位置稳定',['速率和延迟使用等宽数字。','正常数值以灰色呈现，减少抢眼；','异常才提升颜色与原因。','保留留白，卡片不随窗口无限扩展。'])
    cards=[(32,'先读路线，再读指标',['入口 → 当前节点是最大字号的信息。','策略与网卡只有一行，减少字段标签。','相比前一版，卡片高度由 270 收敛到 216 px。']),
           (578,'同一套组件，两个密度',['单代理突出路线；多代理对齐相同的信息。','支持卡片 / 列表选择，并记住用户偏好。','新增代理或筛选时不突然改变当前视图。']),
           (1124,'用状态解释操作结果',['运行状态与启用意图分开：开关开着也可能失败。','只有确认生效后，才替换“当前节点”。','减少动态效果时，保留文字与颜色反馈。'])]
    for x,title,lines in cards:
        s.rect(x,912,524,162,'white','#e0e4e9',8);s.txt(x+20,944,title,15,INK,600);s.lines(x+20,978,lines,12,MUTED,27)
    s.save('01-single-refined.svg')


def multi_board(ui):
    s=SVG(1680,1120,'Routa 专业设计方案：多代理路线列表和准确状态')
    board_header(s,'02','列表的价值，是快速比较真实状态','多代理：96 px 等高行 / 操作无需悬停')
    place_ui(s,ui)
    note(s,141,1,'对齐用户真正要比较的项',['每行固定四个区域：','代理、入口与去向、性能、操作。','沿着箭头读路线，沿着列比较状态。','常用操作始终可见。'])
    note(s,377,2,'准确表达分流关系',['统一转发：展示当前生效节点。','规则分流：展示“按规则分流”，','并补充默认去向。','避免暗示所有流量都去同一个节点。'])
    note(s,613,3,'配置与生效状态分开',['启动失败时主路线显示“未生效”。','配置的目标节点留在原因行。','运行、关闭、失败都有文字；','错误就近出现，避免整行泛红。'])
    items=[(32,'切换中','#edf5fd',BLUE,'当前节点保持不变',['按钮原位禁用，加载指示以 800 ms 循环。','仅路径上的小光点流动，不让整行跳动。']),
           (578,'成功','#edf7ed',GREEN,'确认生效后，更新节点',['160 ms 交叉淡入，短暂高亮后恢复。','2 秒成功提示后消失，键盘焦点回到入口。']),
           (1124,'失败','#fdf0ee',RED,'保留选择，固定反馈区重试',['区分“未保存”与“已保存未生效”。','弹窗不关闭、按钮不移动，当前节点继续可见。'])]
    for x,title,bg,c,headline,lines in items:
        s.rect(x,912,524,162,'white','#e0e4e9',8)
        s.txt(x+20,941,title,14,c,600)
        s.rect(x+20,955,484,37,bg,r=4);s.txt(x+34,979,headline,13,c,500)
        s.lines(x+20,1022,lines,12,MUTED,25)
    s.save('02-list-refined.svg')


def spinner(s,x,y,angle):
    s.p.append(f'<g transform="translate({x} {y}) rotate({angle})">')
    s.path('M0 -6a6 6 0 1 1-5.2 3',BLUE,1.8);s.end()


def motion(frame):
    s=SVG(1280,448,'Routa 切换节点动效：成功与失败对比')
    s.rect(0,0,1280,448,'#f5f6f8',r=0)
    s.txt(24,35,'切换节点：让反馈忠于真实生效状态',22,INK,600)
    s.txt(1256,34,'循环演示 · 虚构数据',12,MUTED,anchor='end')
    phase='idle' if frame<10 else 'pending' if frame<30 else 'result'
    for x,failed in [(24,False),(656,True)]:
        s.txt(x,73,'失败：保留当前节点与候选选择' if failed else '成功：确认生效后更新节点',14,INK,600)
        s.rect(x,92,600,286,'white','#dedede',8)
        s.txt(x+20,126,'机场',16,INK,600);s.badge(x+70,108,'运行中')
        s.toggle(x+512,111);s.more(x+568,120)
        s.txt(x+20,166,'入口',11,MUTED);s.txt(x+250,166,'当前生效节点',11,MUTED)
        s.txt(x+20,202,'SOCKS5 :7893',20,INK,500)
        s.arrow(x+190,195,30)
        if phase=='pending':s.dot(x+190+(frame%8)/7*30,195,2.5,BLUE)
        if phase=='result' and not failed:
            alpha=max(0,1-(frame-30)/12)
            s.rect(x+245,176,152,38,'#e5f2ff',r=4,extra=f'opacity="{alpha}"')
            fade=min(1,(frame-30)/1.6)
            if fade<1:s.txt(x+250,203,'新加坡01',23,INK,600,extra=f'opacity="{1-fade}"')
            s.txt(x+250,203,'东京02',23,INK,600,extra=f'opacity="{fade}"')
        else:s.txt(x+250,203,'新加坡01',23,INK,600)
        label='切换中…' if phase=='pending' else '重试' if phase=='result' and failed else '切换节点'
        s.rect(x+460,177,116,34,'#f6f6f6' if phase=='pending' else 'white','#d4d4d4',4)
        if phase=='pending':
            spinner(s,x+477,194,frame*45)
            s.txt(x+492,199,label,12,MUTED)
        else:s.txt(x+518,199,label,12,BLUE if failed and phase=='result' else INK,500,'middle')
        s.txt(x+20,237,'手动选择 · 系统默认网卡',12,MUTED)
        msg=None
        if phase=='pending':msg='正在切换到东京02…';c=BLUE;bg='#edf5fd'
        elif phase=='result' and failed:msg='切换未生效，当前仍为新加坡01。请重试。';c=RED;bg='#fdf0ee'
        elif phase=='result' and frame<50:msg='已切换到东京02';c=GREEN;bg='#edf7ed'
        if msg:
            s.rect(x+20,257,560,40,bg,r=4)
            if phase=='pending':s.dot(x+37,277,3,BLUE)
            else:s.icon('ErrorOutlineOutlined' if failed else 'CheckCircleOutlineOutlined',x+28,267,c,20)
            s.txt(x+58,282,msg,12,c)
        s.rect(x+1,321,598,56,'#fafafa',r=7);s.rect(x+1,321,598,6,'#fafafa',r=0);s.line(x+1,321,x+599,321)
        delay='58 ms' if phase=='result' and not failed else '182 ms'
        s.txt(x+20,355,'延迟 '+delay,12,MUTED);s.txt(x+157,355,'↑ 24 KB/s',12,MUTED);s.txt(x+292,355,'↓ 2.8 MB/s',12,MUTED);s.txt(x+508,355,'编辑配置',12,BLUE)
    captions={'idle':'开始：用户已在节点弹窗选择“东京02”','pending':'切换中：按钮与反馈区保持原位，当前节点继续可见','result':'结果：确认生效才更新；失败时保留当前节点并支持重试'}
    s.txt(24,416,captions[phase],13,MUTED)
    s.txt(1256,416,'无位移、无抖动、无整行闪烁',12,DIM,anchor='end')
    return s


def main():
    s=SVG(1280,768,'Routa 单代理路线卡片 V2');single(s);one=s.save('single-ui.svg')
    s=SVG(1280,768,'Routa 多代理路线列表 V2');multiple(s);many=s.save('multiple-ui.svg')
    single_board(one);multi_board(many)
    frames=ROOT/'motion-frames';frames.mkdir(exist_ok=True)
    for frame in range(72):
        path=f'motion-frames/{frame:03}.svg';motion(frame).save(path)
    motion(40).save('motion-static.svg')
    data={'single':one,'multiple':many}
    template=(ROOT/'prototype.template.html').read_text(encoding='utf-8')
    (ROOT/'index.html').write_text(template.replace('/* SCREEN_DATA */','const screens = '+json.dumps(data,ensure_ascii=False)+';'),encoding='utf-8')
    print('Created two review boards, clean UI screens, and 72 motion frames.')


if __name__=='__main__':main()
