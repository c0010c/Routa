"""Runtime connections and log placement concepts. Static illustrative data only."""
from layout import *

def nav(label,y,name='overview',selected=False,child=False):
    if selected: rect(9,y,182,40,'#e4e4e4',r=5); rect(10,y+9,3,22,BLUE,r=1.5)
    if not child:
        if name=='log':
            rect(28,y+11,14,17,'none','#32373d',1.5)
            for d in [16,20,24]: line(31,y+d,39,y+d,'#32373d')
        elif name=='connections':
            path(f'M28 {y+16}h13m-4-4 4 4-4 4M41 {y+25}H28m4-4-4 4 4 4','#32373d')
        else: icon(name,27,y+12,'#32373d')
    txt(67 if child else 58,y+26,label,12 if child else 14,MUTED if child else INK,600 if selected else 400)
    if child: line(51,y+2,51,y+36,'#cfd2d6'); line(51,y+20,59,y+20,'#cfd2d6')

def search(x,y,w,label='搜索域名、IP 或程序'):
    rect(x,y,w,32,'#fff','#d9dde3',4)
    add(f'<circle cx="{x+14}" cy="{y+14}" r="4" fill="none" stroke="#9299a2"/>'); line(x+17,y+17,x+21,y+21,'#9299a2'); txt(x+30,y+21,label,11,'#9299a2')
def redbutton(x,y,w,label):
    rect(x,y,w,32,'#fff','#e4bdbb',4); txt(x+w/2,y+21,label,12,'#b52f28',500,'middle')
def checkbox(x,y,on=False):
    if on: tick(x,y)
    else: rect(x,y,14,14,'#fff','#c9cdd3',2)
def xmark(x,y,c='#8c9198'): path(f'M{x-3} {y-3}l6 6m0-6-6 6',c,1.2)

CONNS=[
    ('docs.example.com:443','chrome.exe','浏览器 · 7894','SG 新加坡01','TCP','2.4 MB','2 分 18 秒'),
    ('api.example.net:443','Code.exe','机场 · 7893','SG 新加坡01','TCP','628 KB','46 秒'),
    ('media.example.net:443','局域网设备','机场 · 7893','SG 新加坡01','UDP','8.1 MB','1 分 32 秒'),
    ('files.example.org:443','aria2c.exe','下载器 · 7895','JP 东京02','TCP','12.3 MB','4 分 05 秒'),
    ('images.example.com:443','chrome.exe','浏览器 · 7894','SG 新加坡01','TCP','946 KB','18 秒'),
]
LOGS=[
    ('14:32:08','INFO','内核','TCP docs.example.com:443 using 机场出口'),
    ('14:32:07','INFO','分流组件','Code.exe routed to 机场 :7893'),
    ('14:32:06','WARN','内核','Connection timeout: update.example.org:443'),
    ('14:32:04','INFO','应用','已应用出口「机场出口」的配置'),
    ('14:32:01','INFO','内核','Listener 浏览器 started on 127.0.0.1:7894'),
    ('14:31:58','INFO','内核','Tailscale instance 家庭网络 is running'),
    ('14:31:55','INFO','分流组件','Process routing rules applied (3 rules)'),
    ('14:31:52','INFO','应用','订阅「机场订阅」更新成功，共 24 个节点'),
]
def level(x,y,s):
    bg='#fff3db' if s=='WARN' else '#f0f2f5'; c='#996b13' if s=='WARN' else '#6b7583'
    rect(x,y,45,19,bg,r=3); txt(x+22.5,y+13,s,9,c,500,'middle')
def logrow(x,y,w,row,selected=False,compact=False):
    tm,lv,src,msg=row
    if selected: rect(x,y,w,39 if compact else 43,'#fff8e9')
    txt(x+13,y+24,tm,10,MUTED); level(x+86,y+11,lv)
    txt(x+145,y+24,src,10,MUTED)
    txt(x+221,y+24,msg,10 if compact else 11,INK)

