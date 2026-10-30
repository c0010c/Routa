"""Five functional architectures expressed as desktop screens, not implementation."""
from ui import *

def nav(label,y,name='overview',selected=False,child=False):
    if selected:
        rect(9,y,182,40,'#e4e4e4',r=5); rect(10,y+9,3,22,BLUE,r=1.5)
    if not child: icon(name,27,y+12,'#32373d')
    txt(67 if child else 58,y+26,label,12 if child else 14,MUTED if child else INK,600 if selected else 400)
    if child: line(51,y+2,51,y+36,'#cfd2d6'); line(51,y+20,59,y+20,'#cfd2d6')
def section(label,y): txt(24,y,label,10,'#81858b')
def shell():
    start('#f3f3f3')
    rect(1,33,199,798,'#f3f3f3')
    rect(20,48,25,25,BLUE,r=5); path('M28 54v10q0 3 3 3h5M36 55v8m-2-2 2 2 2-2','#fff',1.5)
    txt(55,66,'Routa',16,weight=600); txt(22,807,'v0.2.0',10,MUTED)
def title(label,desc=None):
    txt(224,80,label,25,weight=600)
    if desc: txt(224,105,desc,12,MUTED)
    dot(1032,73); txt(1044,78,'内核运行中',11,MUTED); more(1161,74)
def tabs(items,x=224,y=128):
    for w,label,selected in items:
        txt(x+12,y+19,label,13,INK if selected else MUTED,600 if selected else 400)
        if selected: rect(x+12,y+31,w-24,3,BLUE,r=1.5)
        x+=w
    line(224,y+35,1176,y+35,'#dedfe2')
def tick(x,y,c=BLUE):
    rect(x,y,15,15,c,r=3); path(f'M{x+3} {y+8}l3 3 6-7','#fff',1.4)
def cellSelect(x,y,w,label): field(x,y,w,label)
def address(x,y,s): txt(x,y,s,12); copy(x+len(s)*6.8+14,y-12)
def stat(x,y,w,label,value):
    rect(x,y,w,88,'#fff',BORDER,7); txt(x+16,y+26,label,11,MUTED); txt(x+16,y+62,value,23,weight=600)

