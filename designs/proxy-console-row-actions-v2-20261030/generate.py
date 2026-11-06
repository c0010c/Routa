"""Design-only revision: complete per-row operations, no selection action bar."""
from base import *


def begin_action(s,kind,i,label,role='button',checked=None):
    s.p.append(f'<g data-{kind}="{i}" tabindex="0" role="{role}" aria-label="{label}"'+(f' aria-checked="{str(checked).lower()}"' if checked is not None else '')+' style="cursor:pointer">')


def inline_toggle(s,i,r,y):
    begin_action(s,'toggle',i,'启用 '+r['name'],'switch',r['on'])
    s.rect(1049,y+14,48,40,'transparent',r=4)
    s.rect(1056,y+25,34,18,BLUE if r['on'] else 'white',None if r['on'] else '#8a8a8a',9,extra='data-toggle-track="true"')
    s.dot(1081 if r['on'] else 1065,y+34,6,'white' if r['on'] else '#8a8a8a')
    s.end()


def console(s,picker=False):
    frame(s)
    s.rect(216,124,700,40,'white','#c8cdd3',5);s.icon('SearchOutlined',230,134,MUTED,20)
    s.txt(262,150,'搜索代理、端口或节点',14,DIM)
    s.rect(844,132,58,24,'#f2f3f4',r=4);s.txt(873,149,'Ctrl K',11,MUTED,anchor='middle')
    s.button(930,128,148,'全部 6',icon='FilterListOutlined')
    s.button(1090,128,158,'全部测速',icon='RefreshOutlined',action='test-all')
    s.rect(216,188,1032,448,'white','#dedfe2',6)
    s.rect(217,189,1030,39,'#fafafa',r=5);s.rect(217,216,1030,12,'#fafafa',r=0)
    for x,label in [(240,'端口 / 协议'),(344,'代理'),(480,'当前去向'),(780,'延迟 / 速率'),(960,'状态 / 启用'),(1120,'编辑')]:s.txt(x,213,label,11,MUTED)
    for i,r in enumerate(ROWS):
        y=228+i*68;c=INK if r['on'] else DIM;failed=r['kind']=='red'
        s.p.append(f'<g data-row="{i}" data-name="{r["name"]}">')
        s.rect(217,y,1030,68,'transparent',r=0,extra='data-row-bg="true"')
        if i:s.line(217,y,1247,y)
        s.txt(240,y+29,r['port'],19,c,500,extra='font-family="Consolas,Segoe UI,sans-serif"')
        s.txt(240,y+50,r['proto'],10,MUTED)
        s.txt(344,y+30,r['name'],14,c,500)
        s.txt(480,y+28,'未生效' if failed else r['node'],14,DIM if failed else c,500,extra='data-node="true"')
        if r['mode']=='手动选择':
            begin_action(s,'switch',i,'切换 '+r['name']+' 的节点')
            s.rect(672,y+8,86,32,'transparent',r=4,extra='data-action-hit="true"')
            s.txt(682,y+29,'切换节点',11,BLUE,500)
            s.end()
        s.txt(480,y+52,'端口被占用 · 配置：'+r['node'] if failed else r['mode']+' · '+r['nic'],10,RED if failed else MUTED,extra='data-meta="true"')
        s.txt(780,y+28,r['delay'],13,AMBER if i==2 else MUTED,extra='data-delay="true"')
        s.txt(780,y+51,f"↑ {r['up']}  ↓ {r['down']}",10,MUTED,extra='data-rates="true"')
        cstate,bg={'green':(GREEN,'#edf7ed'),'gray':(MUTED,'#f0f0f0'),'red':(RED,'#fdf0ee')}[r['kind']]
        s.rect(960,y+22,76,24,bg,r=12,extra='data-state-bg="true"');s.dot(970,y+34,2.5,cstate)
        s.txt(978,y+38,r['state'],11,cstate,500,extra='data-state="true"')
        inline_toggle(s,i,r,y)
        begin_action(s,'edit',i,'编辑 '+r['name']+' 的配置')
        s.rect(1110,y+17,86,34,'transparent',r=4,extra='data-action-hit="true"')
        s.txt(1120,y+39,'编辑配置',12,BLUE,500);s.end()
        begin_action(s,'more-row',i,r['name']+' 的更多操作')
        s.rect(1207,y+18,30,30,'transparent',r=4,extra='data-action-hit="true"')
        for xx in [1217,1222,1227]:s.dot(xx,y+33,1.35,MUTED)
        s.end();s.end()
    s.line(1104,189,1104,635,'#edf0f3')
    # Only a quiet count footer remains; there is no current-row model or toolbar.
    s.txt(216,740,'6 个代理 · 4 个运行中 · 1 个关闭 · 1 个异常',11,MUTED,extra='data-count="true"')
    if picker:
        x,y,w,h=530,276,366,364
        s.rect(x,y,w,h,'white','#c5cdd8',7,extra='filter="url(#shadow)"')
        s.txt(x+20,y+30,'切换节点 · 机场',17,INK,600);s.txt(x+20,y+54,'当前生效：新加坡01',11,MUTED)
        s.rect(x+20,y+70,w-40,30,'white','#d6d6d6',4);s.icon('SearchOutlined',x+29,y+78,DIM,15);s.txt(x+53,y+90,'搜索节点',11,DIM)
        for n,(name,delay) in enumerate([('新加坡01','182 ms'),('东京02','58 ms'),('香港03','76 ms')]):
            yy=y+113+n*48;active=n==1
            s.rect(x+20,yy,w-40,40,'#edf5fd' if active else 'white',BLUE if active else '#e1e4e8',4)
            s.txt(x+34,yy+25,name,12,INK,500);s.txt(x+w-34,yy+25,delay,11,MUTED,anchor='end')
        s.line(x,y+h-54,x+w,y+h-54)
        s.txt(x+20,y+h-23,'3 个节点',11,MUTED);s.button(x+w-128,y+h-42,108,'确认切换',True,h=30)


def main():
    s=SVG(1280,768,'Routa 代理列表：全部操作归属对应行');console(s);ui=s.save('01-row-actions.svg')
    s=SVG(1280,768,'Routa 行内节点切换：就近打开候选');console(s,True);s.save('02-node-picker.svg')
    template=(ROOT/'viewer.template.html').read_text(encoding='utf-8')
    (ROOT/'index.html').write_text(template.replace('<!-- SCREEN -->',ui).replace('/* ROW_DATA */','const rows='+json.dumps(ROWS,ensure_ascii=False)+';'),encoding='utf-8')
    print('Created the row-scoped design and node-picker preview. No bottom action bar.')


if __name__=='__main__':main()
