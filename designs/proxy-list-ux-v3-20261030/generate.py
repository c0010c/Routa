"""UX review artifacts, separate from application source and older designs."""
from base import *


def action(s,x,y,w,label,i,kind='edit',primary=False):
    s.p.append(f'<g data-{kind}="{i}" role="button" aria-label="{label} {ROWS[i]["name"]}">')
    if primary:s.rect(x,y,w,34,'white','#d6dce3',4)
    s.txt(x+w/2,y+22,label,12,BLUE,500,'middle');s.end()


def page(s,picker=False):
    frame(s)
    # Visible filters explain scope; search and batch test share one quiet toolbar.
    for x,w,label,on in [(216,76,'全部 6',True),(300,90,'运行中 4',False),(398,90,'已关闭 1',False),(496,76,'异常 1',False)]:
        if on:s.rect(x,128,w,34,'#e7f1fc',r=5)
        s.txt(x+w/2,150,label,12,BLUE if on else MUTED,500 if on else 400,'middle')
    s.rect(754,128,326,34,'white','#d0d5dc',4);s.icon('SearchOutlined',766,136,DIM,18)
    s.txt(792,150,'搜索名称、端口或节点',12,DIM)
    s.button(1092,128,156,'全部测速',icon='RefreshOutlined')
    s.rect(216,188,1032,496,'white','#dedfe2',7)
    s.rect(217,189,1030,39,'#fafafa',r=6);s.rect(217,216,1030,12,'#fafafa',r=0)
    for x,label in [(240,'代理 / 入口'),(480,'当前去向'),(780,'延迟 / 速率'),(966,'运行状态'),(1060,'启用'),(1130,'编辑')]:s.txt(x,213,label,12,MUTED)
    for i,r in enumerate(ROWS):
        y=228+i*76;off=not r['on'];failed=r['kind']=='red'
        if i:s.line(217,y,1247,y)
        s.txt(240,y+30,r['name'],15,INK if not off else MUTED,600)
        s.txt(240,y+55,f"{r['proto']} :{r['port']}",12,MUTED)
        s.icon('ContentCopyOutlined',355,y+42,DIM,16)
        s.txt(480,y+30,'未生效' if failed else '—' if off else r['node'],15,DIM if failed or off else INK,500)
        meta='配置：'+r['node']+' · 端口被占用' if failed else '配置：'+r['node']+' · '+r['nic'] if off else r['mode']+' · '+r['nic']
        s.txt(480,y+55,meta,11,RED if failed else MUTED)
        if failed:action(s,662,y+12,94,'修改端口',i,'repair')
        elif r['mode']=='手动选择':action(s,662,y+12,94,'切换节点',i,'switch',True)
        s.txt(780,y+30,r['delay'],14,AMBER if i==2 else MUTED)
        s.txt(780,y+55,f"↑ {r['up']}  ↓ {r['down']}",11,MUTED)
        c=RED if failed else MUTED if off else GREEN
        if failed:s.rect(960,y+24,78,26,'#fdf0ee',r=13)
        s.dot(970,y+37,2.5,c);s.txt(979,y+41,r['state'],12,c)
        s.toggle(1060,y+28,r['on'])
        action(s,1116,y+20,84,'编辑配置',i)
        s.more(1224,y+37)
    s.line(1104,189,1104,683,'#edf0f3')
    s.txt(216,739,'显示 6 / 6 个代理',12,MUTED)
    if picker:
        x,y,w,h=500,270,412,400
        s.rect(x,y,w,h,'white','#c8d1dd',7,extra='filter="url(#shadow)"')
        s.txt(x+20,y+31,'切换节点 · 机场',18,INK,600)
        s.path(f'M{x+w-29} {y+20}l10 10m0-10-10 10',MUTED,1.2)
        s.txt(x+20,y+57,'当前生效：新加坡01',12,MUTED)
        s.rect(x+20,y+72,w-40,34,'white','#d5d5d5',4);s.icon('SearchOutlined',x+31,y+81,DIM,17);s.txt(x+57,y+94,'搜索节点',12,DIM)
        for n,(name,delay) in enumerate([('新加坡01','182 ms'),('东京02','58 ms'),('香港03','76 ms')]):
            yy=y+118+n*49;active=n==1
            s.rect(x+20,yy,w-40,42,'#edf5fd' if active else 'white',BLUE if active else '#dfe3e8',5)
            s.txt(x+34,yy+26,name,13,INK,500);s.txt(x+w-34,yy+26,delay,12,MUTED,anchor='end')
        s.rect(x+20,y+279,w-40,53,'#fff7e8',r=4)
        s.txt(x+32,y+301,'选择已保存，尚未生效。',12,AMBER,500)
        s.txt(x+32,y+321,'当前仍使用新加坡01，可重试生效。',11,AMBER)
        s.line(x,y+h-56,x+w,y+h-56)
        s.txt(x+20,y+h-21,'3 个节点',12,MUTED)
        s.button(x+w-222,y+h-43,80,'取消',h=32)
        s.button(x+w-130,y+h-43,110,'重试生效',True,h=32)


def main():
    for name,picker in [('01-ux-list',False),('02-recoverable-switch',True)]:
        s=SVG(1280,768,'Routa 代理页 UX 优化');page(s,picker);s.save(name+'.svg')
    names=['RouteOutlined','SpaceDashboardOutlined','SwapHorizOutlined','AltRouteOutlined','LanguageOutlined','DnsOutlined','SettingsOutlined','AddOutlined','SearchOutlined','RefreshOutlined','ContentCopyOutlined','MoreHoriz']
    icons={}
    for name in names:
        source=(PROJECT/'node_modules'/'@mui'/'icons-material'/(name+'.mjs')).read_text(encoding='utf-8')
        paths=re.findall(r'd:\s*"([^"]+)"',source)
        icons[name]='<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" aria-hidden="true">'+''.join('<path d="'+d+'"/>' for d in paths)+'</svg>'
    template=(ROOT/'prototype.template.html').read_text(encoding='utf-8')
    (ROOT/'index.html').write_text(template.replace('/* ROW_DATA */','const initialRows='+json.dumps(ROWS,ensure_ascii=False)+';').replace('/* ICON_DATA */','const icons='+json.dumps(icons,ensure_ascii=False)+';'),encoding='utf-8')
    print('Created UX review images and native HTML prototype.')


if __name__=='__main__':main()
