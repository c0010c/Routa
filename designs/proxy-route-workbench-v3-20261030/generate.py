"""Independent route-workbench design study. Only writes within this folder."""
from drawing import *

DATA=[
 dict(name='机场',proto='SOCKS5',port=7893,node='新加坡01',strategy='手动选择',nic='系统默认网卡',state='运行中',kind='green',delay='182 ms',up='24 KB/s',down='2.8 MB/s',total='1.24 GB',on=True),
 dict(name='开发工具',proto='Mixed',port=7894,node='东京01',strategy='手动选择',nic='以太网',state='运行中',kind='green',delay='58 ms',up='8 KB/s',down='420 KB/s',total='430 MB',on=True),
 dict(name='下载专用',proto='HTTP',port=7895,node='美国西部02',strategy='自动测速',nic='系统默认网卡',state='运行中',kind='green',delay='356 ms',up='126 KB/s',down='8.2 MB/s',total='3.6 GB',on=True),
 dict(name='备用线路',proto='Mixed',port=7897,node='香港02',strategy='手动选择',nic='Wi-Fi',state='启动失败',kind='red',delay='—',up='—',down='—',total='0 B',on=True),
]


def action(s,kind,row=None):
    attrs=f' data-row="{row}"' if row is not None else ''
    s.p.append(f'<g data-action="{kind}"{attrs} tabindex="0" role="button" style="cursor:pointer">')


def shell(s,many=False):
    s.rect(0,0,1280,768,BG,'#d8d8d8',8)
    s.rect(1,1,1278,31,'#fafafa',r=7);s.line(0,32,1280,32,'#e8e8e8')
    s.rect(15,10,12,12,BLUE,r=3);s.txt(36,22,'Routa',11,MUTED)
    s.line(1159,16,1169,16,MUTED);s.rect(1202,11,9,9,'none',MUTED,0);s.path('M1247 11l9 9m0-9-9 9',MUTED,1)
    s.rect(1,33,182,734,BG,r=0)
    s.rect(20,56,26,26,BLUE,r=6);s.icon('RouteOutlined',25,61,'white',16);s.txt(57,75,'Routa',17,INK,600)
    for i,(icon,label) in enumerate([('SpaceDashboardOutlined','概览'),('SwapHorizOutlined','代理'),('AltRouteOutlined','进程分流'),('LanguageOutlined','订阅'),('DnsOutlined','节点')]):
        y=109+i*46
        if i==1:s.rect(8,y,168,40,'#e5e9ed',r=5);s.rect(8,y+11,3,18,BLUE,r=1.5)
        s.icon(icon,24,y+10,INK if i==1 else MUTED,20);s.txt(60,y+26,label,14,INK,600 if i==1 else 400)
    s.icon('SettingsOutlined',24,683,MUTED,20);s.txt(60,699,'设置',14);s.txt(24,744,'v0.2.0',11,DIM)
    s.txt(216,88,'代理',26,INK,600)
    s.rect(280,65,24,24,'#e6e8eb',r=5);s.txt(292,82,'4' if many else '1',12,MUTED,500,'middle')
    s.dot(1015,78,3,GREEN);s.txt(1027,82,'内核运行中',12,MUTED)
    s.button(1136,61,112,'新增代理',True,'AddOutlined',action='add')
    # A quiet tool row, not another white card surrounding the workspace.
    for x,label,w,active in [(216,'全部 4' if many else '全部 1',72,True),(300,'运行中 3' if many else '运行中 1',92,False),(402,'异常 1' if many else '异常 0',72,False)]:
        if active:s.rect(x,128,w,32,'#e6f0fb',r=5)
        s.txt(x+w/2,149,label,12,BLUE if active else MUTED,500 if active else 400,'middle')
    s.rect(652,128,244,32,'white','#d4d4d4',4);s.icon('SearchOutlined',662,135,DIM,18);s.txt(688,149,'搜索名称、端口或节点',12,DIM)
    s.txt(920,149,'当前选择',12,MUTED)


def route(s,i,r,y,focus=False,selected=False):
    h=184 if focus else 116
    action(s,'select',i)
    s.rect(216,y,680,h,'white',BLUE if selected else '#dedfe2',8,extra='stroke-width="1.25"' if selected else '')
    if selected:s.rect(216,y+18,3,h-36,BLUE,r=1.5)
    s.txt(240,y+30,r['name'],17 if focus else 15,INK,600)
    badge_x=294 if len(r['name'])==2 else 313
    s.badge(badge_x,y+11,r['state'],r['kind'])
    act='switch' if r['strategy']=='手动选择' else 'test'
    label='切换节点' if act=='switch' else '测速'
    action(s,act,i);s.rect(785,y+11,92,30,'transparent',r=4);s.icon('SwapHorizOutlined' if act=='switch' else 'RefreshOutlined',790,y+18,BLUE,16);s.txt(813,y+30,label,12,BLUE,500);s.end()
    if focus:
        s.txt(240,y+68,'入口',11,MUTED);s.txt(574,y+68,'当前去向',11,MUTED)
        s.txt(240,y+105,f"{r['proto']} :{r['port']}",22,INK,500)
        s.arrow(437,y+97,110,BLUE)
        s.txt(492,y+122,r['strategy'],11,MUTED,anchor='middle')
        s.txt(574,y+105,r['node'],23,INK,600,extra=f'data-route-node="{i}"')
        s.txt(240,y+152,'127.0.0.1',12,MUTED);s.txt(574,y+152,r['nic'],12,MUTED)
    else:
        failed=r['kind']=='red';c=DIM if failed else INK
        s.txt(240,y+67,f"{r['proto']} :{r['port']}",16,c,500)
        s.arrow(437,y+61,110,DIM if failed else BLUE)
        s.txt(492,y+87,r['strategy'],10,MUTED,anchor='middle')
        s.txt(574,y+67,'未生效' if failed else r['node'],18,c,600,extra=f'data-route-node="{i}"')
        metrics='端口被占用' if failed else f"{r['delay']}  ·  ↑ {r['up']}  ·  ↓ {r['down']}"
        s.txt(240,y+97,metrics,11,RED if failed else MUTED,extra=f'data-route-metrics="{i}"')
        s.txt(574,y+97,'配置：'+r['node'] if failed else r['nic'],11,MUTED)
    s.end()


def spark(s,x,y,w=280,h=54,level=0):
    values=[.16,.2,.16,.25,.22,.27,.62,.54,.37,.45,.39,.71,.64,.73,.62,.87,.69,.72,.5,.67,.62,.8,.73,.68,.92,.82,.71,.77]
    if level==1:values=[v*.4 for v in values]
    if level==2:values=list(reversed(values))
    points=[(x+i*w/(len(values)-1),y+h-v*h) for i,v in enumerate(values)]
    d='M'+' L'.join(f'{a:.1f} {b:.1f}' for a,b in points)
    fill=d+f'L{x+w} {y+h}H{x}Z'
    s.path(fill,'none',0,'#e8f2fc');s.path(d,BLUE,1.6)
    lower='M'+' L'.join(f'{a:.1f} {y+h-4-(i%6)*1.1:.1f}' for i,(a,b) in enumerate(points))
    s.path(lower,'#9aa6b2',1.2)


def inspector(s,i,r,picker=False,phase='idle',frame=0,chosen='东京02'):
    x,y,w,h=920,188,328,516
    failed=r['kind']=='red'
    s.p.append('<g data-inspector="true">')
    s.rect(x,y,w,h,'white','#dedfe2',8)
    if not picker:
        s.txt(x+24,y+32,r['name'],18,INK,600)
        s.toggle(x+231,y+16,r['on'],'toggle');s.more(x+302,y+25)
        s.dot(x+27,y+54,3,RED if failed else GREEN);s.txt(x+37,y+58,r['state'],11,RED if failed else GREEN)
        s.line(x,y+76,x+w,y+76)
        s.txt(x+24,y+106,'实时速率',12,MUTED)
        s.txt(x+24,y+144,r['up'],20,INK,500);s.txt(x+168,y+144,r['down'],20,INK,500)
        s.txt(x+24,y+166,'↑ 上传',11,MUTED);s.txt(x+168,y+166,'↓ 下载',11,MUTED)
        if not failed:spark(s,x+24,y+187,280,49,i)
        else:s.line(x+24,y+236,x+304,y+236,'#d5d9dd')
        s.txt(x+24,y+255,'最近 30 秒',10,DIM)
        s.line(x+24,y+274,x+304,y+274)
        fields=[('延迟',r['delay']),('本次累计',r['total']),('本地地址',f"127.0.0.1:{r['port']}")]
        for n,(label,value) in enumerate(fields):
            yy=y+304+n*39
            s.txt(x+24,yy,label,12,MUTED)
            s.txt(x+304 if n<2 else x+280,yy,value,13,AMBER if value=='356 ms' else INK,500,'end',extra='data-detail-delay="true"' if n==0 else '')
            if n==2:s.icon('ContentCopyOutlined',x+290,yy-13,DIM,15)
        if failed:
            s.rect(x+24,y+405,280,37,'#fdf0ee',r=4);s.icon('ErrorOutlineOutlined',x+34,y+415,RED,16);s.txt(x+58,y+429,'端口被占用，请编辑配置。',11,RED)
        s.line(x,y+456,x+w,y+456)
        s.button(x+24,y+471,280,'编辑配置',action='edit',h=32)
    else:
        action(s,'back');s.rect(x+16,y+14,28,30,'transparent');s.icon('ArrowBackOutlined',x+21,y+20,MUTED,18);s.end()
        s.txt(x+52,y+34,'切换节点',18,INK,600);s.txt(x+24,y+61,r['name']+(' · 当前未生效' if failed else ' · 当前 '+r['node']),11,MUTED,extra='data-current="true"')
        s.line(x,y+78,x+w,y+78)
        s.rect(x+24,y+94,280,32,'white','#d3d3d3',4);s.icon('SearchOutlined',x+34,y+102,DIM,16);s.txt(x+58,y+115,'搜索节点',12,DIM)
        choices=[(r['node'],r['delay']),('东京02','58 ms'),('香港03','76 ms')]
        for n,(name,delay) in enumerate(choices):
            yy=y+142+n*69;active=name==chosen
            action(s,'candidate-'+name)
            s.rect(x+24,yy,280,58,'#eef5fc' if active else 'white',BLUE if active else '#e2e2e2',5)
            s.dot(x+42,yy+21,6,'white');s.p.append(f'<circle cx="{x+42}" cy="{yy+21}" r="6" fill="white" stroke="{BLUE if active else "#aaaaaa"}"/>')
            if active:s.dot(x+42,yy+21,3,BLUE)
            s.txt(x+58,yy+26,name,13,INK,500);s.txt(x+58,yy+45,'演示机场'+((' · 配置目标' if failed else ' · 当前生效') if name==r['node'] else ''),10,MUTED,extra=f'data-candidate-meta="{name}"')
            s.txt(x+264,yy+26,delay,11,MUTED,anchor='end');s.end()
        if phase=='pending':
            s.rect(x+24,y+365,280,66,'#edf5fd',r=4)
            s.p.append(f'<g transform="translate({x+41} {y+384}) rotate({frame*45})">');s.path('M0 -6a6 6 0 1 1-5.2 3',BLUE,1.8);s.end()
            s.txt(x+58,y+389,'正在切换到'+chosen+'…',11,BLUE,extra='data-feedback-main="true"')
            s.txt(x+38,y+414,'当前节点保持可见',10,MUTED)
        elif phase=='failure':
            s.rect(x+24,y+365,280,66,'#fdf0ee',r=4);s.icon('ErrorOutlineOutlined',x+34,y+375,RED,16)
            s.txt(x+58,y+389,'端口仍被占用，尚未生效。' if failed else '切换未生效，请重试。',11,RED,extra='data-feedback-main="true"');s.txt(x+38,y+414,'请编辑配置后重试。' if failed else '保留选择，当前仍为'+r['node'],10,RED,extra='data-feedback-detail="true"')
        s.line(x,y+456,x+w,y+456)
        s.txt(x+24,y+492,'3 个节点',11,MUTED)
        action(s,'confirm')
        if phase=='pending':s.rect(x+166,y+471,138,32,'#e6e8eb',r=4);s.txt(x+235,y+492,'切换中…',12,MUTED,anchor='middle')
        else:s.button(x+166,y+471,138,'重试' if phase=='failure' else '确认切换',True,h=32)
        s.end()
    s.end()


def screen(s,many=False,picker=False,selected=0,phase='idle',frame=0,result=False):
    shell(s,many)
    rows=[dict(r) for r in DATA]
    if result and rows[selected]['kind']!='red':rows[selected].update(node='东京02',delay='58 ms')
    if many:
        for i,r in enumerate(rows):route(s,i,r,188+i*128,False,selected==i)
    else:route(s,0,rows[0],188,True,True)
    inspector(s,selected,rows[selected],picker,phase,frame)
    s.txt(216,736,'4 个代理 · 3 个运行中 · 1 个异常' if many else '1 个代理 · 1 个运行中',11,MUTED)
    if result and frame<50:
        s.rect(598,707,298,38,'#edf7ed','#cddfcd',5);s.icon('CheckCircleOutlineOutlined',612,717,GREEN,18);s.txt(639,731,'已切换到东京02',12,GREEN)


def board(ui,many=False):
    s=SVG(1680,1120,'Routa V3 路线工作台：'+('多代理' if many else '单代理'))
    s.rect(0,0,1680,1120,'#f6f7f9',r=0)
    s.txt(32,39,'方向 C / 路线工作台',12,BLUE,600)
    s.txt(32,79,'多条路线，快速定位和比较' if many else '路线在左，所选代理的状态在右',29,INK,600)
    s.txt(1648,44,'Routa · 第三版发散方案',12,MUTED,anchor='end')
    s.txt(1648,75,'浅色 Windows 风格 / 蓝色强调 / 左侧导航',12,MUTED,anchor='end')
    s.rect(32,112,1280,768,'white',r=8,extra='filter="url(#shadow)"')
    s.group(32,112);s.p.append(ui[ui.index('<g font-family='):ui.rindex('</svg>')]);s.end()
    notes=(
      [('路线承担总览',['协议与端口 → 策略 → 实际节点，','按流量方向组织信息。','单代理用舒展的路线，','宽度服务于关系，保持留白。']),
       ('右侧承担深入查看',['延迟、上传、下载、累计量集中。','曲线展示所选代理最近 30 秒趋势。','启停、编辑配置、更多操作，','都围绕当前选择。']),
       ('切换节点，原位进入',['点击路线上的“切换节点”，','右侧进入候选节点列表。','左侧路线保持可见，','确认生效后才更新节点名称。'])]
      if not many else
      [('点击一条路线，即可聚焦',['以描边标记当前选择，','右侧显示对应代理详情。','快捷操作直接可见，','无需先进入单独的详情页。']),
       ('路线行保留比较信息',['延迟和双向速率缩到次级一行。','策略位于连线下方，','网卡位于去向下方。','不同代理保持同样的阅读顺序。']),
       ('异常就近，状态明确',['失败路线显示“未生效”，','原因在入口下方，配置目标另列。','删除仍收进更多菜单。','默认选中首条，避免右侧空置。'])]
    )
    for n,(title,lines) in enumerate(notes):
        yy=145+n*229;s.num(1352,yy+12,n+1);s.txt(1376,yy+17,title,16,INK,600);s.lines(1340,yy+56,lines,13,MUTED,25)
    topics=[('选择路线','100–120 ms 描边与背景变化。','右侧内容淡入，面板宽高保持不变。'),('切换节点','右侧在固定框架内进入候选列表。','成功后返回详情，路线节点同步更新。'),('窄窗口','收起右侧栏；选择后以固定抽屉打开。','标题与底部按钮固定，内容内部滚动。')]
    for x,(title,a,b) in zip([32,578,1124],topics):
        s.rect(x,912,524,162,'white','#e1e4e8',8);s.txt(x+20,946,title,15,INK,600);s.lines(x+20,984,[a,b],12,MUTED,28)
    s.txt(32,1104,'独立设计产物 · 所有数据为虚构 · 页面内仅展示必要信息，说明位于评审画面外',11,DIM)
    s.txt(1648,1104,'02 / 02' if many else '01 / 02',11,DIM,anchor='end')
    s.save('02-multiple-workbench.svg' if many else '01-single-workbench.svg')


def main():
    views={}
    for name,many,picker in [('single',False,False),('multiple',True,False),('picker',True,True)]:
        s=SVG(1280,768,'Routa 路线工作台');screen(s,many,picker);views[name]=s.save(name+'-ui.svg')
    board(views['single']);board(views['multiple'],True)
    # Reusable complete-screen scenarios for a self-contained prototype.
    for i in range(1,4):
        s=SVG(1280,768,'Routa 当前选择');screen(s,True,False,i);views['detail-'+str(i)]=s.save('detail-'+str(i)+'.svg')
    for i in [0,1,3]:
        for phase in ['picker','pending','failure']:
            s=SVG(1280,768,'Routa 节点切换反馈');screen(s,True,True,i,'idle' if phase=='picker' else phase);views[f'{phase}-{i}']=s.save(f'{phase}-{i}-ui.svg')
    for phase in ['picker','pending','failure']:
        s=SVG(1280,768,'Routa 单代理切换节点');screen(s,False,True,0,'idle' if phase=='picker' else phase);views['single-'+phase]=s.save('single-'+phase+'-ui.svg')
    s=SVG(1280,768,'Routa 节点切换成功');screen(s,True,False,0,result=True);views['success']=s.save('success-ui.svg')
    s=SVG(1280,768,'Routa 节点切换失败');screen(s,True,True,0,'failure');s.save('failure-ui.svg')
    (ROOT/'screens.json').write_text(json.dumps(views,ensure_ascii=False),encoding='utf-8')
    frames=ROOT/'motion-frames';frames.mkdir(exist_ok=True)
    for f in range(64):
        s=SVG(1048,568,'Routa 原位切换节点动效')
        s.rect(0,0,1048,568,'#f3f3f3',r=0)
        s.group(-204,-176)
        picker=8<=f<30;phase='pending' if 14<=f<30 else 'idle'
        r=dict(DATA[0])
        if f>=30:r.update(node='东京02',delay='58 ms')
        route(s,0,r,188,False,True)
        inspector(s,0,r,picker,phase,f)
        if 14<=f<30:s.dot(437+(f%8)/7*110,249,2.5,BLUE)
        if 30<=f<50:
            s.rect(216,324,680,46,'#edf7ed','#d0e1d0',5);s.icon('CheckCircleOutlineOutlined',236,337,GREEN,20);s.txt(270,353,'已切换到东京02',13,GREEN)
        s.end()
        label='选择节点：左侧路线保持可见' if 8<=f<14 else '正在切换：只更新局部反馈' if 14<=f<30 else '确认生效：返回详情并更新路线' if f>=30 else '查看路线：右侧展示当前代理详情'
        s.txt(12,555,label,12,MUTED)
        s.save(f'motion-frames/{f:03}.svg')
    (ROOT/'index.html').write_text((ROOT/'prototype.template.html').read_text(encoding='utf-8').replace('/* SCREEN_DATA */','const screens='+json.dumps(views,ensure_ascii=False)+';'),encoding='utf-8')
    print('Created route-workbench views, review boards and 64 motion frames.')


if __name__=='__main__':main()
