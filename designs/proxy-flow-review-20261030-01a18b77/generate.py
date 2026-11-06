"""Standalone Routa design review; only writes files beside this generator."""
from pathlib import Path
from html import escape
import json
import re

ROOT = Path(__file__).resolve().parent
PROJECT = ROOT.parent.parent
INK = '#1b1b1b'
MUTED = '#616161'
DIM = '#858585'
BLUE = '#0067c0'
LINE = '#e5e5e5'
BG = '#f3f3f3'
GREEN = '#0f7b0f'
AMBER = '#9d5d00'
RED = '#c42b1c'
FONT = 'Segoe UI,Microsoft YaHei UI,Microsoft YaHei,sans-serif'


class SVG:
    def __init__(self, w, h, title):
        self.w, self.h = w, h
        self.p = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" role="img" aria-label="{escape(title)}">',
                  f'<title>{escape(title)}</title>',
                  '<defs><filter id="shadow" x="-20%" y="-20%" width="140%" height="160%"><feDropShadow dx="0" dy="5" stdDeviation="10" flood-color="#243244" flood-opacity=".09"/></filter></defs>',
                  f'<g font-family="{FONT}" font-variant-numeric="tabular-nums">']

    def rect(self, x, y, w, h, fill='white', stroke=None, r=6, extra=''):
        self.p.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill}"' + (f' stroke="{stroke}"' if stroke else '') + f' {extra}/>')

    def txt(self, x, y, value, size=13, color=INK, weight=400, anchor='start', extra=''):
        self.p.append(f'<text x="{x}" y="{y}" font-size="{size}" fill="{color}" font-weight="{weight}" text-anchor="{anchor}" {extra}>{escape(str(value))}</text>')

    def lines(self, x, y, values, size=12, color=MUTED, step=23):
        for i, value in enumerate(values):
            self.txt(x, y+i*step, value, size, color)

    def line(self, x1, y1, x2, y2, color=LINE, width=1, extra=''):
        self.p.append(f'<path d="M{x1} {y1}H{x2}" stroke="{color}" stroke-width="{width}" {extra}/>' if y1 == y2 else f'<path d="M{x1} {y1}L{x2} {y2}" stroke="{color}" stroke-width="{width}" {extra}/>')

    def path(self, d, color=MUTED, width=1.5, fill='none', extra=''):
        self.p.append(f'<path d="{d}" fill="{fill}" stroke="{color}" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round" {extra}/>')

    def dot(self, x, y, r=3, color=GREEN):
        self.p.append(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{color}"/>')

    def icon(self, name, x, y, color=MUTED, size=20):
        source = (PROJECT/'node_modules'/'@mui'/'icons-material'/(name+'.mjs')).read_text(encoding='utf-8')
        paths = re.findall(r'd:\s*"([^"]+)"', source)
        if not paths:
            raise ValueError(name)
        self.p.append(f'<g transform="translate({x} {y}) scale({size/24})" fill="{color}">')
        self.p.extend(f'<path d="{d}"/>' for d in paths)
        self.p.append('</g>')

    def button(self, x, y, w, label, primary=False, icon=None, h=34, action=None):
        if action:
            self.p.append(f'<g data-action="{action}" tabindex="0" role="button" aria-label="{label}" style="cursor:pointer">')
        self.rect(x,y,w,h,BLUE if primary else 'white',None if primary else '#d2d2d2',4)
        if icon:
            self.icon(icon,x+11,y+(h-16)/2,'white' if primary else MUTED,16)
            self.txt(x+34,y+h/2+4.5,label,13,'white' if primary else INK,500)
        else:
            self.txt(x+w/2,y+h/2+4.5,label,13,'white' if primary else INK,500,'middle')
        if action:
            self.p.append('</g>')

    def link(self, x, y, label, action=None, color=BLUE, size=13):
        if action:
            self.p.append(f'<g data-action="{action}" tabindex="0" role="button" aria-label="{label}" style="cursor:pointer">')
            self.rect(x-6,y-21,max(42,len(label)*size+12),30,'transparent',r=4)
        self.txt(x,y,label,size,color,500)
        if action:
            self.p.append('</g>')

    def badge(self, x, y, label, kind='green'):
        c, bg = {'green':(GREEN,'#edf7ed'),'gray':(MUTED,'#f0f0f0'),'red':(RED,'#fdf0ee'),'amber':(AMBER,'#fff5e5'),'blue':(BLUE,'#eaf3fc')}[kind]
        w = len(label)*11+24
        self.rect(x,y,w,24,bg,r=12)
        self.dot(x+10,y+12,2.5,c)
        self.txt(x+18,y+16,label,11,c,500)

    def toggle(self, x, y, on=True, action=None):
        if action:
            self.p.append(f'<g data-action="{action}" tabindex="0" role="button" aria-label="切换代理启用状态" style="cursor:pointer">')
        self.rect(x,y,34,18,BLUE if on else 'white',None if on else '#8a8a8a',9)
        self.dot(x+(25 if on else 9),y+9,6,'white' if on else '#8a8a8a')
        if action:
            self.p.append('</g>')

    def more(self, x, y, action='more'):
        self.p.append(f'<g data-action="{action}" tabindex="0" role="button" aria-label="更多操作" style="cursor:pointer">')
        self.rect(x-15,y-15,30,30,'transparent',r=4)
        for xx in [x-5,x,x+5]: self.dot(xx,y,1.35,MUTED)
        self.p.append('</g>')

    def arrow(self, x, y, w=36, color=BLUE):
        self.path(f'M{x} {y}h{w}m-6-5 6 5-6 5',color,1.6)

    def num(self, x, y, value):
        self.dot(x,y,12,BLUE)
        self.txt(x,y+4,str(value),11,'white',600,'middle')

    def group(self, x=0, y=0, scale=1):
        self.p.append(f'<g transform="translate({x} {y}) scale({scale})">')

    def end(self): self.p.append('</g>')
    def save(self, name):
        text = '\n'.join(self.p+['</g></svg>'])
        (ROOT/name).write_text(text,encoding='utf-8')
        return text


def shell(s):
    s.rect(0,0,1280,768,BG,'#d8d8d8',8)
    s.rect(1,1,1278,31,'#fafafa',r=7)
    s.line(0,32,1280,32,'#e8e8e8')
    s.rect(15,10,12,12,BLUE,r=3)
    s.txt(36,22,'Routa',11,MUTED)
    s.line(1159,16,1169,16,MUTED)
    s.rect(1202,11,9,9,'none',MUTED,0)
    s.path('M1247 11l9 9m0-9-9 9',MUTED,1)
    s.rect(1,33,182,734,'#f3f3f3',r=0)
    s.rect(20,56,26,26,BLUE,r=6)
    s.icon('RouteOutlined',25,61,'white',16)
    s.txt(57,75,'Routa',17,INK,600)
    nav=[('SpaceDashboardOutlined','概览'),('SwapHorizOutlined','代理'),('AltRouteOutlined','进程分流'),('LanguageOutlined','订阅'),('DnsOutlined','节点')]
    for i,(icon,label) in enumerate(nav):
        y=109+i*46
        active=i==1
        if active:
            s.rect(8,y,168,40,'#e6e6e6',r=5)
            s.rect(8,y+11,3,18,BLUE,r=1.5)
        s.icon(icon,24,y+10,INK if active else MUTED,20)
        s.txt(60,y+26,label,14,INK,600 if active else 400)
    s.icon('SettingsOutlined',24,683,MUTED,20)
    s.txt(60,699,'设置',14)
    s.txt(24,744,'v0.2.0',11,DIM)
    s.txt(216,89,'代理',26,INK,600)
    s.dot(1015,78,3,GREEN)
    s.txt(1027,82,'内核运行中',12,MUTED)
    s.button(1136,61,112,'新增代理',True,'AddOutlined',action='add')


def toolbar(s,multiple=False):
    s.rect(216,125,1032,40,'#fafafa',LINE,6)
    chips=[('全部  5' if multiple else '全部  1',76),('运行中  3' if multiple else '运行中  1',96),('异常  1' if multiple else '异常  0',76)]
    x=224
    for i,(label,w) in enumerate(chips):
        if i==0: s.rect(x,131,w,28,'#e7f1fb',r=4)
        s.txt(x+w/2,150,label,12,BLUE if i==0 else MUTED,500 if i==0 else 400,'middle')
        x+=w+5
    s.p.append('<g data-action="view" tabindex="0" role="button" aria-label="切换卡片或列表视图" style="cursor:pointer">')
    s.rect(850,130,80,30,'white','#d5d5d5',4)
    s.rect(891 if multiple else 852,132,37,26,'#e7f1fb',r=3)
    s.icon('ViewAgendaOutlined',860,136,MUTED if multiple else BLUE,18)
    s.icon('ViewListOutlined',900,136,BLUE if multiple else MUTED,18)
    s.end()
    s.rect(942,129,300,32,'white','#d5d5d5',4)
    s.icon('SearchOutlined',952,136,DIM,18)
    s.txt(978,150,'搜索名称、端口或节点',12,DIM)


def single(s):
    shell(s); toolbar(s)
    x,y,w,h=216,188,872,270
    s.rect(x,y,w,h,'white',LINE,8)
    s.txt(240,224,'机场',18,INK,600)
    s.badge(294,205,'运行中')
    s.toggle(995,210,True,'toggle')
    s.more(1054,219)
    s.txt(240,271,'入口',11,MUTED)
    s.txt(528,271,'当前节点',11,MUTED)
    s.txt(240,305,'SOCKS5',23,INK,500)
    s.txt(345,305,':7893',23,INK,500)
    s.arrow(459,297,38)
    s.txt(528,305,'新加坡01',25,BLUE,600,extra='data-slot="current-node"')
    s.button(932,280,132,'切换节点',False,'SwapHorizOutlined',action='switch')
    s.txt(240,345,'手动选择 · 系统默认网卡',13,MUTED)
    s.rect(217,376,870,81,'#fafafa',r=7)
    s.rect(217,376,870,8,'#fafafa',r=0)
    s.line(217,376,1087,376)
    for xx,label,value,c in [(240,'延迟','182 ms',GREEN),(357,'上传','↑ 24 KB/s',INK),(501,'下载','↓ 2.8 MB/s',INK),(659,'累计流量','1.24 GB',MUTED)]:
        s.txt(xx,403,label,11,MUTED)
        s.txt(xx,433,value,14,c,500)
    s.line(799,397,799,436,LINE)
    s.link(823,432,'编辑配置','edit')
    s.link(922,432,'测速','test')
    s.p.append('<g data-action="copy" tabindex="0" role="button" aria-label="复制本地地址" style="cursor:pointer">')
    s.rect(1009,406,43,38,'transparent')
    s.icon('ContentCopyOutlined',1021,416,MUTED,18)
    s.end()
    s.txt(216,729,'1 个代理 · 1 个运行中',11,MUTED)


ROWS=[
    dict(name='机场',state='运行中',kind='green',proto='SOCKS5',port=7893,node='新加坡01',meta='手动选择 · 系统默认网卡',delay='182 ms',delayc=GREEN,rates='↑ 24 KB/s   ↓ 2.8 MB/s',on=True,action='切换节点'),
    dict(name='开发工具',state='运行中',kind='green',proto='Mixed',port=7894,node='东京01',meta='手动选择 · 以太网',delay='58 ms',delayc=GREEN,rates='↑ 8 KB/s     ↓ 420 KB/s',on=True,action='切换节点'),
    dict(name='下载专用',state='运行中',kind='green',proto='HTTP',port=7895,node='美国西部02',meta='自动测速 · 系统默认网卡',delay='356 ms',delayc=AMBER,rates='↑ 126 KB/s  ↓ 8.2 MB/s',on=True,action='测速'),
    dict(name='内网直连',state='已关闭',kind='gray',proto='SOCKS5',port=7896,node='直连',meta='直连 · Wi-Fi',delay='—',delayc=DIM,rates='↑ —            ↓ —',on=False,action='复制地址'),
    dict(name='备用线路',state='启动失败',kind='red',proto='Mixed',port=7897,node='香港02',meta='端口被占用 · 未生效',delay='—',delayc=DIM,rates='↑ —            ↓ —',on=True,action='切换节点'),
]


def multiple(s):
    shell(s);toolbar(s,True)
    s.rect(216,188,1032,516,'white',LINE,8)
    s.rect(217,189,1030,35,'#fafafa',r=7)
    s.rect(217,216,1030,8,'#fafafa',r=0)
    for x,label in [(236,'代理 / 状态'),(412,'入口 → 去向'),(824,'延迟 / 实时速率'),(1048,'操作')]:
        s.txt(x,211,label,11,MUTED)
    for i,r in enumerate(ROWS):
        y=224+i*96
        s.p.append(f'<g data-row="{i}" data-proxy="{r["name"]}">')
        if i==4: s.rect(217,y,1030,95,'#fffbfa',r=7)
        s.line(217,y,1247,y)
        color=INK if r['on'] else DIM
        s.txt(236,y+32,r['name'],16,color,600)
        s.badge(236,y+47,r['state'],r['kind'])
        s.txt(412,y+33,f"{r['proto']} :{r['port']}",15,color,500)
        s.arrow(552,y+27,24,BLUE if i<3 else DIM)
        s.txt(594,y+33,r['node'],17,BLUE if i<2 else color,600,extra=f'data-slot="node-{i}"')
        if i==4:
            s.rect(684,y+16,34,20,'#f0f0f0',r=3)
            s.txt(701,y+30,'配置',10,MUTED,anchor='middle')
        s.txt(412,y+63,r['meta'],12,RED if i==4 else MUTED)
        s.txt(824,y+32,r['delay'],14,r['delayc'],500)
        s.txt(824,y+62,r['rates'],11,MUTED)
        action={'切换节点':'switch','测速':'test','复制地址':'copy'}[r['action']]
        s.link(1048,y+32,r['action'],action)
        s.link(1048,y+63,'编辑配置','edit',size=12)
        s.toggle(1160,y+18,r['on'],'toggle')
        s.more(1220,y+55)
        s.end()
    s.txt(216,736,'5 个代理 · 3 个运行中 · 1 个异常',11,MUTED)
    s.link(1168,736,'全部测速','test',size=12)


def note(s,x,y,n,title,lines):
    s.num(x+12,y+12,n)
    s.txt(x+35,y+17,title,16,INK,600)
    s.lines(x,y+58,lines,13,MUTED,25)


def board_head(s,index,title,subtitle):
    s.rect(0,0,1680,1120,'#f7f8fa',r=0)
    s.txt(32,39,f'ROUTA  /  PROXY FLOW  /  {index}',11,BLUE,600,extra='letter-spacing="1.4"')
    s.txt(32,78,title,29,INK,600)
    s.txt(1648,43,'浅色 Windows 风格 · 设计评审稿',12,MUTED,anchor='end')
    s.txt(1648,74,subtitle,12,MUTED,anchor='end')


def single_board(ui):
    s=SVG(1680,1120,'Routa 代理页方案一：单个代理的完整布局与信息层级')
    board_head(s,'01','一个代理，先看清流量去向','右侧和底部为设计标注，非页面常驻内容')
    s.rect(32,112,1280,768,'white',r=8,extra='filter="url(#shadow)"')
    s.group(32,112);s.p.append(ui[ui.index('<g font-family='):ui.rindex('</svg>')]);s.end()
    note(s,1340,133,1,'名称与状态先行',['“机场”与“运行中”并列。','启停与更多菜单位于右上角，','不打断主要信息的阅读顺序。'])
    note(s,1340,335,2,'入口 → 当前节点',['协议和端口指向实际节点，','形成一条连续的流量路径。','“切换节点”紧邻去向，','常用操作一步可达。'])
    note(s,1340,566,3,'策略、性能逐级后退',['策略与网卡合并为一行。','延迟、速率、累计量集中到底栏；','配置入口统一为“编辑配置”。'])
    s.line(1340,768,1648,768,'#dce1e8')
    s.lines(1340,798,['872 × 270 px 的信息卡片','宽度有上限，按信息关系横向展开；','留白用于保持焦点，不添加装饰面板。'],12,MUTED,25)
    for x in [32,578,1124]:s.rect(x,912,524,162,'white','#e3e6ea',8)
    s.txt(52,943,'布局变化',15,INK,600)
    s.rect(52,961,28,70,'#f1f1f1','#d6d6d6',3)
    for yy in [974,987,1000,1013]:s.line(58,yy,74,yy,'#b4b4b4')
    s.arrow(100,993,25)
    s.rect(145,976,108,48,'#edf4fc','#c7ddef',4)
    s.line(157,992,197,992,'#91b6d6');s.arrow(203,992,15)
    s.line(157,1010,239,1010,'#c4d5e4')
    s.lines(274,978,['纵向平铺 → 横向路径','旧卡至少 350 高 → 新卡 270 高','不随窗口无限拉伸'],12,MUTED,25)
    s.txt(598,943,'控件分工',15,INK,600)
    s.lines(598,975,['直接操作：切换节点、编辑配置、测速、复制。','“⋯”展开：删除代理；仅操作时出现菜单。','名称过长时省略，悬停或键盘聚焦查看完整名称。'],12,MUTED,29)
    s.txt(1144,943,'同一套响应逻辑',15,INK,600)
    s.lines(1144,975,['宽屏保持宽度上限；窄屏在箭头处换行。','代理数量变化时保持视图选择，避免突然重排。','主区内部滚动，标题、筛选和底部计数固定。'],12,MUTED,29)
    s.txt(32,1103,'独立设计产物 · 全部为虚构示例数据 · 未修改应用实现',11,DIM)
    s.txt(1648,1103,'01 / 02',11,DIM,anchor='end')
    s.save('01-single-proxy.svg')


def multiple_board(ui):
    s=SVG(1680,1120,'Routa 代理页方案二：多代理紧凑列表、状态对比和切换反馈')
    board_head(s,'02','多个代理，用同一条路径快速比较','右侧为评审标注 · 底部为节点切换反馈示意')
    s.rect(32,112,1280,768,'white',r=8,extra='filter="url(#shadow)"')
    s.group(32,112);s.p.append(ui[ui.index('<g font-family='):ui.rindex('</svg>')]);s.end()
    note(s,1340,133,1,'从卡片变为等高行',['每行 96 px；名称、路径、','性能和操作在固定位置。','同屏呈现 5 个代理，','扫描时无需逐卡寻找字段。'])
    note(s,1340,363,2,'让异常易于比较',['运行状态带文字，不只依赖颜色。','较高延迟用琥珀色提示；','关闭时速率显示“—”。','失败行就近显示原因和“未生效”。'])
    note(s,1340,593,3,'常用入口保持一致',['手动策略直接显示“切换节点”。','自动策略显示“测速”。','启停、编辑和更多操作始终对齐，','删除收进“⋯”，降低误触。'])
    s.txt(1340,831,'列表先对齐，操作不藏在悬停中。',12,MUTED)
    xs=[32,578,1124]
    states=[('切换中','800 ms 循环 · 不移动布局',BLUE,'正在切换到东京02…'),('切换成功','160 ms 淡入 · 短暂高亮',GREEN,'已切换到东京02'),('切换失败','保留选择 · 原位提示与重试',RED,'切换未生效，请重试')]
    for x,(title,sub,c,msg) in zip(xs,states):
        s.rect(x,912,524,162,'white','#e3e6ea',8)
        s.txt(x+20,942,title,15,c,600)
        s.txt(x+504,942,sub,11,MUTED,anchor='end')
        s.rect(x+20,961,484,42,{'切换中':'#eef5fc','切换成功':'#edf7ed','切换失败':'#fdf0ee'}[title],r=4)
        if title=='切换中':
            s.path(f'M{x+40} 971a10 10 0 1 1-9 6',BLUE,2)
        else:s.icon('CheckCircleOutlineOutlined' if title=='切换成功' else 'ErrorOutlineOutlined',x+29,972,c,20)
        s.txt(x+61,988,msg,13,c,500)
        if title=='切换失败':s.link(x+456,988,'重试',size=12)
        texts={'切换中':['当前生效节点保持可见；提交按钮禁用，保留原位。','只对路径箭头做轻微流动，避免整行闪烁。'],'切换成功':['确认运行状态后更新节点名；成功提示 2 秒后淡出。','输入、列表和底部按钮位置保持稳定。'],'切换失败':['错误保留在固定反馈区，不关闭选择弹窗。','已保存未生效时明确区分；可重试或查看详情。']}
        s.lines(x+20,1028,texts[title],12,MUTED,24)
    s.txt(32,1103,'固定高度的节点弹窗：标题 / 搜索 / 底部按钮固定，列表内部滚动；减少动态效果时关闭流动动画。',11,DIM)
    s.txt(1648,1103,'02 / 02',11,DIM,anchor='end')
    s.save('02-multiple-proxies.svg')


def main():
    s=SVG(1280,768,'Routa 单代理完整页面');single(s);one=s.save('single-ui.svg')
    s=SVG(1280,768,'Routa 多代理紧凑列表');multiple(s);many=s.save('multiple-ui.svg')
    single_board(one);multiple_board(many)
    data={'single':one,'multiple':many}
    (ROOT/'screens.json').write_text(json.dumps(data,ensure_ascii=False),encoding='utf-8')
    template = (ROOT/'prototype.template.html').read_text(encoding='utf-8')
    (ROOT/'index.html').write_text(template.replace('/* SCREEN_DATA */', 'const screens = '+json.dumps(data,ensure_ascii=False)+';'),encoding='utf-8')
    print('Generated four editable SVG design artifacts and prototype screen data.')


if __name__ == '__main__':main()
