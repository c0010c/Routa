"""Five further visual treatments using identical listener content and geometry."""
from pathlib import Path
import xml.etree.ElementTree as ET
import json
ROOT=Path(__file__).parent
NS='http://www.w3.org/2000/svg'; ET.register_namespace('',NS)
def tag(s): return '{'+NS+'}'+s
src=ROOT.parent/'listener-capture-actions-v2/02-listener-section-toolbar.svg'
themes=[
 dict(id='01-precision',name='精密 · 线框',desc='窄圆角、蓝灰细线、强调网格与数据，偏专业仪表',bg='#eef2f7',nav='#e6edf5',panel='#fbfdff',border='#bac7d5',ink='#102942',muted='#5f758b',accent='#174d88',success='#26734c',select='#d7e4f2',badge='#e0eaf6',head='#e4ecf5',radius=1),
 dict(id='02-deep-ocean',name='深海 · 蓝调',desc='深蓝分层、青蓝强调，冷静而有科技感',bg='#10202f',nav='#0b1926',panel='#172d40',border='#2a465a',ink='#deedf5',muted='#9ab4c5',accent='#73c6cd',success='#8dcbb3',select='#234459',badge='#294b5d',head='#193347',radius=6),
 dict(id='03-sage',name='自然 · 鼠尾草',desc='浅灰绿底、苔绿强调、柔和轮廓，低饱和的自然感',bg='#eef1e9',nav='#e3e8dc',panel='#fafbf7',border='#d4dccc',ink='#2e3c2c',muted='#78856f',accent='#658457',success='#517454',select='#d9e3d2',badge='#e4eddc',head='#f1f5ec',radius=10),
 dict(id='04-satin',name='紫灰 · 缎面',desc='雾紫灰底、细腻高光、淡紫强调，温和且精致',bg='#f0edf5',nav='#eae5f1',panel='#fdfcff',border='#dcd5e8',ink='#3c3350',muted='#857a95',accent='#7d67a5',success='#4d876f',select='#e1d8ec',badge='#ece5f6',head='#f6f3fb',radius=10),
 dict(id='05-classic',name='经典 · 复古桌面',desc='灰色控件、硬朗直角、立体边框，回到传统 Windows 工具',bg='#c7c7c7',nav='#d4d0c8',panel='#d4d0c8',border='#808080',ink='#101010',muted='#505050',accent='#000080',success='#1d6729',select='#000080',badge='#e1e7f1',head='#d4d0c8',radius=0),
]
gray={'#e1e4e8','#dedfe3','#dedfe2','#d9dde3','#e0e2e5','#d7dce4','#d0d6de','#edf0f3','#e9edf3','#e5e5e5','#eeeeee'}
for t in themes:
    root=ET.fromstring(src.read_text(encoding='utf-8')); g=root.find(tag('g')); defs=root.find(tag('defs'))
    dark=t['id']=='02-deep-ocean'; classic=t['id']=='05-classic'
    toggles=[]; checks=[]
    grad=ET.SubElement(defs,tag('linearGradient'),{'id':'satinSurface','x1':'0','y1':'0','x2':'0','y2':'1'})
    ET.SubElement(grad,tag('stop'),{'stop-color':'#ffffff'}); ET.SubElement(grad,tag('stop'),{'offset':'1','stop-color':'#f8f5fc'})
    for el in list(g.iter()):
        typ=el.tag.rsplit('}',1)[-1]; fill=el.get('fill'); stroke=el.get('stroke')
        x=float(el.get('x','0')); y=float(el.get('y','0')); w=float(el.get('width','0')); h=float(el.get('height','0'))
        if stroke in gray: el.set('stroke',t['border'])
        if stroke in ['#202126','#32373d']: el.set('stroke',t['ink'])
        if stroke=='#6d7179': el.set('stroke',t['muted'])
        if stroke=='#0067c0': el.set('stroke',t['accent'])
        if fill=='#202126': el.set('fill',t['ink'])
        if fill in ['#6d7179','#81858b','#8b929b','#8b929a','#32373d','#616161']: el.set('fill',t['muted'])
        if fill=='#147d3e': el.set('fill',t['success'])
        if fill=='#0067c0': el.set('fill',t['accent'])
        if fill=='#eaf3fc': el.set('fill',t['badge'])
        if fill=='#e4e4e4': el.set('fill',t['select'])
        if fill=='#f3f3f3': el.set('fill',t['bg'])
        if typ=='rect':
            large=w>220 and h>65
            if fill in ['#fff','#ffffff']:
                el.set('fill',t['panel']); el.set('rx',str(t['radius'] if large else min(t['radius'],4)))
                if t['id']=='04-satin' and large: el.set('fill','url(#satinSurface)')
                if classic:
                    el.set('fill','#ffffff' if (h==34 or h>130) else t['panel'])
            if x==0 and y==0: el.set('fill',t['bg']); el.set('rx',str(t['radius']))
            if x==1 and y==33: el.set('fill',t['nav'])
            if classic and fill=='#0067c0' and w>60 and h>25:
                el.set('fill',t['panel']); el.set('stroke','#808080'); el.set('rx','0')
            if classic and w==32 and h==17:
                toggles.append((x,y)); el.set('width','14'); el.set('height','14'); el.set('y',str(y+2)); el.set('fill','#fff'); el.set('stroke','#808080'); el.set('rx','0')
            if classic and (large or (w>60 and h>25)):
                a=ET.Element(tag('path'),{'d':f'M{x+1} {y+h-1}V{y+1}H{x+w-1}','fill':'none','stroke':'#ffffff','stroke-width':'1'})
                b=ET.Element(tag('path'),{'d':f'M{x+w-1} {y+1}V{y+h-1}H{x+1}','fill':'none','stroke':'#707070','stroke-width':'1'})
                idx=list(g).index(el) if el in list(g) else -1
                if idx>=0: g.insert(idx+1,a); g.insert(idx+2,b)
        if dark and fill=='#fff' and typ in ['text','circle']: el.set('fill','#10242f')
        if dark and stroke=='#fff': el.set('stroke','#10242f')
        if classic and typ=='text':
            if 99<y<139 and x<195: el.set('fill','#ffffff')
            if fill=='#fff': el.set('fill','#101010')
    # A table-header band strengthens each theme's material hierarchy.
    table=next(e for e in g if e.tag==tag('rect') and e.get('x')=='224' and e.get('y')=='280')
    pos=list(g).index(table)
    g.insert(pos+1,ET.Element(tag('rect'),{'x':'225','y':'281','width':'950','height':'40','fill':t['head'],'rx':str(t['radius'])}))
    if classic:
        caption=ET.Element(tag('rect'),{'x':'1','y':'1','width':'1198','height':'30','fill':'#000080'})
        g.insert(1,caption)
        for e in list(g):
            if e.tag==tag('text') and float(e.get('y','0'))<31: e.set('fill','#fff')
            if e.tag==tag('path') and 'M1080 16' in e.get('d',''): e.set('stroke','#fff')
            if e.tag==tag('path') and 'M1173 11' in e.get('d',''): e.set('stroke','#fff')
            if e.tag==tag('rect') and e.get('x')=='1127': e.set('stroke','#fff')
            if e.tag==tag('rect') and e.get('fill')=='none' and float(e.get('x','0'))<50 and 100<float(e.get('y','0'))<140: e.set('stroke','#fff')
            if e.tag==tag('circle') and e.get('r')=='5.5':
                cx=float(e.get('cx','0')); cy=float(e.get('cy','0'))
                match=next(((a,b) for a,b in toggles if abs(cx-a-24)<.1 and abs(cy-b-8.5)<.1),None)
                if match:
                    a,b=match; g.remove(e); checks.append(ET.Element(tag('path'),{'d':f'M{a+3} {b+9}l3 3 5-7','fill':'none','stroke':'#101010','stroke-width':'1.5'}))
        for c in checks: g.append(c)
    ET.indent(root); (ROOT/(t['id']+'.svg')).write_text(ET.tostring(root,encoding='unicode'),encoding='utf-8')
(ROOT/'styles.json').write_text(json.dumps(themes,ensure_ascii=False,indent=2),encoding='utf-8')
