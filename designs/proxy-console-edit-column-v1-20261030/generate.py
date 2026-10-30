"""Design-only refinement: a dedicated per-row edit column."""
from base import *


def console(s):
    frame(s)
    s.rect(216,124,700,40,'white','#c8cdd3',5);s.icon('SearchOutlined',230,134,MUTED,20)
    s.txt(262,150,'搜索代理、端口或节点',14,DIM)
    s.rect(844,132,58,24,'#f2f3f4',r=4);s.txt(873,149,'Ctrl K',11,MUTED,anchor='middle')
    s.button(930,128,148,'全部 6',icon='FilterListOutlined')
    s.button(1090,128,158,'全部测速',icon='RefreshOutlined')
    s.rect(216,188,1032,448,'white','#dedfe2',6)
    s.rect(217,189,1030,39,'#fafafa',r=5);s.rect(217,216,1030,12,'#fafafa',r=0)
    for x,label in [(240,'端口 / 协议'),(360,'代理'),(492,'当前去向'),(712,'策略'),(824,'延迟 / 速率'),(1016,'状态'),(1116,'编辑')]:
        s.txt(x,213,label,11,MUTED)
    for i,r in enumerate(ROWS):
        y=228+i*68
        s.p.append(f'<g data-row="{i}" data-name="{r["name"]}" tabindex="0" role="button" aria-label="选择 {r["name"]}" style="cursor:pointer">')
        s.rect(217,y,1030,68,'#e9f3ff' if i==0 else 'transparent',r=0,extra='data-selection="true"')
        s.rect(217,y+8,3,52,BLUE if i==0 else 'transparent',r=1.5,extra='data-selection-edge="true"')
        if i:s.line(217,y,1247,y)
        c=INK if r['on'] else DIM
        s.txt(240,y+29,r['port'],19,c,500,extra='font-family="Consolas,Segoe UI,sans-serif"')
        s.txt(240,y+50,r['proto'],10,MUTED)
        s.txt(360,y+30,r['name'],14,c,600 if i==0 else 400)
        s.txt(492,y+30,'未生效' if r['kind']=='red' else r['node'],14,DIM if r['kind']=='red' else c,500)
        s.txt(492,y+51,'端口被占用' if r['kind']=='red' else r['nic'],10,RED if r['kind']=='red' else MUTED)
        s.txt(712,y+30,r['mode'],12,MUTED)
        s.txt(824,y+29,r['delay'],13,AMBER if i==2 else MUTED)
        s.txt(824,y+50,f"↑ {r['up']}  ↓ {r['down']}",10,MUTED)
        s.badge(1016,y+20,r['state'],r['kind'])
        s.p.append(f'<g data-edit="{i}" tabindex="0" role="button" aria-label="编辑配置 {r["name"]}" style="cursor:pointer">')
        s.rect(1106,y+17,86,34,'transparent',r=4,extra='data-edit-hit="true"')
        s.txt(1116,y+39,'编辑配置',12,BLUE,500)
        s.end()
        s.more(1222,y+33)
        s.end()
    s.line(1098,189,1098,635,'#edf0f3')
    s.rect(216,656,1032,64,'white','#d4dbe3',6)
    s.txt(236,681,'当前选中：机场  :7893 → 新加坡01',13,INK,600,extra='data-current-title="true"')
    s.txt(236,704,'手动选择 · 系统默认网卡',11,MUTED,extra='data-current-meta="true"')
    s.txt(788,695,'启用',11,MUTED);s.toggle(832,681)
    s.button(888,672,180,'切换节点  Enter',True,h=32)
    s.button(1080,672,120,'测速  T',h=32);s.more(1224,688)
    s.txt(216,748,'6 个代理 · 4 个运行中 · 1 个关闭 · 1 个异常',11,MUTED)


def main():
    s=SVG(1280,768,'Routa 代理终端：独立编辑列');console(s);ui=s.save('01-edit-column.svg')
    template=(ROOT/'viewer.template.html').read_text(encoding='utf-8')
    (ROOT/'index.html').write_text(template.replace('<!-- SCREEN -->',ui).replace('/* ROW_DATA */','const rows='+json.dumps(ROWS,ensure_ascii=False)+';'),encoding='utf-8')
    print('Created the dedicated edit-column preview and local interaction demo.')


if __name__=='__main__':main()
