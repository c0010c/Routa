"""Same listener layout in six surface treatments, for style comparison only."""
from pathlib import Path
import xml.etree.ElementTree as ET
import re, json

ROOT=Path(__file__).parent
SOURCE=ROOT.parent/'listener-capture-actions-v2/02-listener-section-toolbar.svg'
NS='http://www.w3.org/2000/svg'
ET.register_namespace('',NS)
def tag(name): return '{'+NS+'}'+name

styles=[
 dict(id='01-mica',name='云母 · 参考',desc='淡蓝灰底色，轻微冷暖渐变，克制的原生桌面质感',bg='#eef1f7',panel='#ffffff',border='#dce2eb',ink='#202126',muted='#6d7480',accent='#0067c0',success='#147d3e',select='#e0e7f1',badge='#eaf3fc',radius=8),
 dict(id='02-acrylic',name='亚克力 · 轻透',desc='更明显的通透与层叠，冰蓝色调，浮起的白色面板',bg='#e7effb',panel='#ffffff',border='#ffffff',ink='#1b2e42',muted='#62778d',accent='#0879ce',success='#21825d',select='#ffffff',badge='#e0f0ff',radius=11),
 dict(id='03-flat',name='极简 · 平面',desc='纯白底、细分隔、较小圆角，强调文字和数据',bg='#ffffff',panel='#ffffff',border='#e4e7eb',ink='#181d24',muted='#67717e',accent='#215fcc',success='#1b7d50',select='#edf2fa',badge='#edf3fc',radius=2),
 dict(id='04-soft-neutral',name='柔和 · 灰白',desc='雾灰蓝背景、柔和阴影、大一些的圆角，低对比层次',bg='#eef1f5',panel='#ffffff',border='#e6ebf1',ink='#283747',muted='#758393',accent='#527695',success='#477a62',select='#dfe7ef',badge='#eaf0f6',radius=12),
 dict(id='05-warm-stone',name='暖灰 · 纸感',desc='米白与暖灰，石色边框，灰绿色强调，安静耐看',bg='#f3f0ea',panel='#fffdf9',border='#ded8cd',ink='#342f28',muted='#81786c',accent='#3d746c',success='#537856',select='#e6e1d6',badge='#e7efea',radius=6),
 dict(id='06-graphite',name='深色 · 石墨',desc='炭灰底、分明的表面层级，浅蓝强调，适合暗色环境',bg='#202224',panel='#2b2e31',border='#41464c',ink='#edf0f3',muted='#aeb7c0',accent='#83bae3',success='#90c8a1',select='#343d46',badge='#344d61',radius=5),
]

gray_strokes={'#e1e4e8','#dedfe3','#dedfe2','#d9dde3','#d9dde3','#e0e2e5','#d7dce4','#d9dde3','#d0d6de','#edf0f3','#e9edf3','#e5e5e5','#eeeeee'}
gray_text={'#6d7179','#81858b','#8b929b','#8b929a','#6d7480','#32373d','#616161'}
for st in styles:
    root=ET.fromstring(SOURCE.read_text(encoding='utf-8'))
    defs=root.find(tag('defs')); group=root.find(tag('g'))
    # Soft shadows apply only to large content surfaces, never fields or buttons.
    filt=ET.SubElement(defs,tag('filter'),{'id':'surfaceShadow','x':'-10%','y':'-15%','width':'120%','height':'140%'})
    ET.SubElement(filt,tag('feDropShadow'),{'dx':'0','dy':'3' if st['id']!='04-soft-neutral' else '5','stdDeviation':'7' if st['id']!='04-soft-neutral' else '11','flood-color':'#23364b','flood-opacity':'.055' if st['id']!='04-soft-neutral' else '.075'})
    bgGrad=ET.SubElement(defs,tag('linearGradient'),{'id':'styleBackground','x1':'0','y1':'0','x2':'1','y2':'1'})
    colors=['#e7eef9','#f3f4f8','#f4eef7'] if st['id']=='01-mica' else ['#deecfb','#edf4fc','#e4e4f7']
    for off,c in zip(['0','.55','1'],colors): ET.SubElement(bgGrad,tag('stop'),{'offset':off,'stop-color':c})
    for el in root.iter():
        typ=el.tag.rsplit('}',1)[-1]
        fill=el.get('fill'); stroke=el.get('stroke'); x=el.get('x'); y=el.get('y')
        if stroke in gray_strokes: el.set('stroke',st['border'])
        if stroke=='#202126': el.set('stroke',st['ink'])
        if stroke=='#32373d': el.set('stroke',st['ink'] if st['id']=='06-graphite' else st['muted'])
        if stroke=='#6d7179': el.set('stroke',st['muted'])
        if fill=='#202126': el.set('fill',st['ink'])
        if fill in gray_text: el.set('fill',st['muted'])
        if fill=='#147d3e': el.set('fill',st['success'])
        if fill=='#0067c0': el.set('fill',st['accent'])
        if stroke=='#0067c0': el.set('stroke',st['accent'])
        if fill=='#eaf3fc': el.set('fill',st['badge'])
        if fill=='#e4e4e4': el.set('fill',st['select'])
        if fill=='#f3f3f3': el.set('fill',st['bg'])
        if typ=='rect':
            w=float(el.get('width','0')); h=float(el.get('height','0'))
            large=w>220 and h>65
            if fill in ['#fff','#ffffff']:
                el.set('fill',st['panel'])
                if large:
                    el.set('rx',str(st['radius']))
                    if st['id'] in ['01-mica','02-acrylic','04-soft-neutral']: el.set('filter','url(#surfaceShadow)')
                    if st['id']=='02-acrylic': el.set('fill-opacity','.76'); el.set('stroke-opacity','.95')
                    if st['id']=='03-flat' and y=='108': el.set('stroke','none')
                elif w>50 and h>22: el.set('rx','2' if st['id']=='03-flat' else '6' if st['id']=='04-soft-neutral' else '4')
            if x=='0' and y=='0':
                el.set('fill','url(#styleBackground)' if st['id'] in ['01-mica','02-acrylic'] else st['bg'])
            if x=='1' and y=='33':
                el.set('fill',{'01-mica':'#eaf0f8','02-acrylic':'#edf3fc','03-flat':'#f7f8fa','04-soft-neutral':'#e9edf3','05-warm-stone':'#ece8df','06-graphite':'#25282b'}[st['id']])
        if fill=='#fff' and typ in ['text','path','circle'] and st['id']=='06-graphite': el.set('fill' if typ!='path' else 'stroke','#20272e')
        if typ=='path' and stroke=='#fff' and st['id']=='06-graphite': el.set('stroke','#20272e')
        if typ=='text' and st['id']=='06-graphite' and fill=='#b17b18': el.set('fill','#dfbd78')
    if st['id']=='03-flat':
        # Metric separators replace the individual metric-card outlines.
        for xx in [453,694,935]: ET.SubElement(group,tag('path'),{'d':f'M{xx} 122V180','stroke':'#e4e7eb','stroke-width':'1'})
    if st['id']=='02-acrylic':
        blur=ET.SubElement(defs,tag('filter'),{'id':'backgroundBlur','x':'-70%','y':'-70%','width':'240%','height':'240%'})
        ET.SubElement(blur,tag('feGaussianBlur'),{'stdDeviation':'64'})
        # These background washes sit behind the UI; they are an acrylic-style visual study.
        for cx,cy,rx,ry,c in [(450,160,230,155,'#a8d4f2'),(1050,560,180,200,'#c7bce9')]:
            shape=ET.Element(tag('ellipse'),{'cx':str(cx),'cy':str(cy),'rx':str(rx),'ry':str(ry),'fill':c,'opacity':'.25','filter':'url(#backgroundBlur)'})
            group.insert(1,shape)
    ET.indent(root)
    (ROOT/(st['id']+'.svg')).write_text(ET.tostring(root,encoding='unicode'),encoding='utf-8')
(ROOT/'styles.json').write_text(json.dumps(styles,ensure_ascii=False,indent=2),encoding='utf-8')
