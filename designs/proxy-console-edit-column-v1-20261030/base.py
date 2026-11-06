"""Three intentionally different Routa design directions. Design artifacts only."""
from drawing import *

ROWS=[
 dict(name='机场',port='7893',proto='SOCKS5',node='新加坡01',mode='手动选择',nic='系统默认网卡',delay='182 ms',up='24 KB/s',down='2.8 MB/s',state='运行中',kind='green',on=True),
 dict(name='开发工具',port='7894',proto='Mixed',node='新加坡01',mode='手动选择',nic='以太网',delay='182 ms',up='8 KB/s',down='420 KB/s',state='运行中',kind='green',on=True),
 dict(name='下载专用',port='7895',proto='HTTP',node='美国西部02',mode='自动测速',nic='系统默认网卡',delay='356 ms',up='126 KB/s',down='8.2 MB/s',state='运行中',kind='green',on=True),
 dict(name='浏览器',port='7896',proto='Mixed',node='东京01',mode='手动选择',nic='系统默认网卡',delay='58 ms',up='12 KB/s',down='1.4 MB/s',state='运行中',kind='green',on=True),
 dict(name='内网直连',port='7897',proto='SOCKS5',node='直连',mode='直连',nic='Wi-Fi',delay='—',up='—',down='—',state='已关闭',kind='gray',on=False),
 dict(name='备用线路',port='7898',proto='Mixed',node='香港02',mode='手动选择',nic='系统默认网卡',delay='—',up='—',down='—',state='启动失败',kind='red',on=True),
]


def frame(s,white=False,title='代理'):
    s.rect(0,0,1280,768,BG,'#d8d8d8',8)
    s.rect(1,1,1278,31,'#fafafa',r=7);s.line(0,32,1280,32,'#e8e8e8')
    s.rect(15,10,12,12,BLUE,r=3);s.txt(36,22,'Routa',11,MUTED)
    s.line(1159,16,1169,16,MUTED);s.rect(1202,11,9,9,'none',MUTED,0);s.path('M1247 11l9 9m0-9-9 9',MUTED,1)
    if white:s.rect(184,33,1095,734,'white',r=0)
    s.rect(20,56,26,26,BLUE,r=6);s.icon('RouteOutlined',25,61,'white',16);s.txt(57,75,'Routa',17,INK,600)
    for i,(icon,label) in enumerate([('SpaceDashboardOutlined','概览'),('SwapHorizOutlined','代理'),('AltRouteOutlined','进程分流'),('LanguageOutlined','订阅'),('DnsOutlined','节点')]):
        y=109+i*46
        if i==1:s.rect(8,y,168,40,'#e5e9ed',r=5);s.rect(8,y+11,3,18,BLUE,r=1.5)
        s.icon(icon,24,y+10,INK if i==1 else MUTED,20);s.txt(60,y+26,label,14,INK,600 if i==1 else 400)
    s.icon('SettingsOutlined',24,683,MUTED,20);s.txt(60,699,'设置',14);s.txt(24,744,'v0.2.0',11,DIM)
    s.txt(216,88,title,26,INK,600);s.dot(1015,78,3,GREEN);s.txt(1027,82,'内核运行中',12,MUTED)
    s.button(1136,61,112,'新增代理',True,'AddOutlined')

