"""Compare standard library icons without changing application source."""
from pathlib import Path
from html import escape
import re

ROOT=Path(__file__).parent
ICONS=ROOT.parents[1]/'node_modules'/'@mui'/'icons-material'
parts=[]

def rect(x,y,w,h,c,r=0,stroke='none'):
    parts.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{c}" stroke="{stroke}"/>')

def text(x,y,s,size=14,c='#efefef',weight=400):
    parts.append(f'<text x="{x}" y="{y}" font-size="{size}" fill="{c}" font-weight="{weight}">{escape(s)}</text>')

def icon(name,x,y,c='#c5c5c5'):
    source=(ICONS/(name+'.mjs')).read_text(encoding='utf-8')
    paths=re.findall(r'd:\s*"([^"]+)"',source)
    assert paths, name
    parts.append(f'<g transform="translate({x} {y})" fill="{c}">')
    parts.extend(f'<path d="{d}"/>' for d in paths)
    parts.append('</g>')

parts.append('<svg xmlns="http://www.w3.org/2000/svg" width="880" height="650" viewBox="0 0 880 650">')
parts.append('<g font-family="Segoe UI,Microsoft YaHei UI,Microsoft YaHei,sans-serif">')
rect(0,0,880,650,'#171717')
text(32,46,'左侧菜单 · 图标备选',24,weight=600)
text(32,76,'统一使用标准 Material 图标，对比形状与功能含义。',13,'#a3a3a3')

variants=[
    ('A · 简洁功能', ['SpaceDashboardOutlined','SwapHorizOutlined','AltRouteOutlined','LanguageOutlined','DnsOutlined','SettingsOutlined']),
    ('B · 网络设备', ['SpaceDashboardOutlined','RouterOutlined','AccountTreeOutlined','OutputOutlined','DynamicFeedOutlined','SettingsOutlined'])
]
labels=['概览','代理','进程分流','出口','订阅与节点','设置']
for i,(heading,names) in enumerate(variants):
    x=32+i*424
    text(x,119,heading,17,weight=600)
    rect(x,142,392,456,'#202020',8,'#373737')
    for j,(label,name) in enumerate(zip(labels,names)):
        y=166+j*55 if j<5 else 536
        selected=j==1
        if selected:
            rect(x+12,y-3,368,46,'#303030',6)
            rect(x+12,y+10,3,20,'#4cc2ff',1.5)
        if j==5:
            rect(x+24,y-17,344,1,'#383838')
        icon(name,x+30,y+8,'#f3f3f3' if selected else '#bcbcbc')
        text(x+74,y+27,label,16,'#f3f3f3' if selected else '#c5c5c5',600 if selected else 400)

text(32,628,'设计对比 · 图标按实际菜单尺寸展示',12,'#858585')
parts.append('</g></svg>')
(ROOT/'03-standard-icon-options.svg').write_text('\n'.join(parts),encoding='utf-8')
print('Saved standard icon comparison')
