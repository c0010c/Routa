/* Review-only interaction model. No network requests, OS calls or real proxy changes. */
(() => {
  'use strict';
  const $ = (s, root=document) => root.querySelector(s);
  const escape = value => String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const icon = name => `<svg class="icon" aria-hidden="true" viewBox="0 0 24 24">${(window.ROUTA_ICONS[name] || window.ROUTA_ICONS.SettingsOutlined).map(d=>`<path d="${d}"/>`).join('')}</svg>`;
  const btn = (label, action, kind='', attrs='') => `<button class="${kind}" data-action="${action}" ${attrs}>${label}</button>`;
  const status = (label,kind='good') => `<span class="status ${kind}"><span class="dot"></span>${label}</span>`;
  const toggle = (label,checked,action,disabled=false) => `<button class="switch" type="button" role="switch" aria-label="${escape(label)}" aria-checked="${checked}" data-action="${action}" ${disabled?'disabled':''}></button>`;
  const note = (content,kind='info') => `<div class="notice ${kind}" role="${kind==='error'?'alert':'note'}">${icon(kind==='error'?'ErrorOutlineOutlined':'InfoOutlined')}<div>${content}</div></div>`;
  const tag = text => `<span class="tag">${text}</span>`;
  const sceneGroups = [
    ['开始使用',[['welcome','首次启动','选择界面模式'],['simple-empty','首次进入','无节点时的引导'],['import','导入订阅','表单、校验与异步反馈'],['import-success','导入完成','准备选节点，不自动连接']]],
    ['简洁版',[['simple-home','待连接','选好节点后开启代理'],['simple-connected','代理已开启','连接控制与实时数据'],['node-picker','选择节点','搜索、测速与确认切换'],['simple-apps','指定应用','节点与应用同页'],['app-picker','添加应用','从运行应用中多选'],['component','组件安装','按需申请管理员权限'],['nodes','订阅与节点','资源列表、更新与管理'],['settings','设置','模式切换与基础偏好'],['diagnostics','诊断','面向问题的检查与修复']]],
    ['异常与边界',[['loading','加载中','首次加载与操作占用'],['error-connect','连接失败','保留选择、提供重试'],['error-import','订阅更新失败','保留旧节点与重试入口'],['error-permission','权限未授予','回到原有连接方式'],['error-port','端口被占用','冲突定位与端口修复']]],
    ['完整版',[['full-overview','概览','运行、流量和监听器状态'],['full-listeners','代理','默认 7881 混合监听器'],['full-process','进程分流','规则、故障策略和连接'],['full-exits','出口','节点、策略组与直连'],['full-nodes','订阅与节点','订阅、手动节点与 Tailscale'],['full-settings','设置','界面、常规与内核设置']]],
    ['完整版表单',[['full-listener-dialog','新建监听器','端口、协议、出口与访问设置'],['full-exit-dialog','新建出口','单节点、策略组与直连'],['full-rule-dialog','新建分流规则','应用、监听器与故障策略'],['full-subscription-dialog','编辑订阅','更新策略与备用出口'],['full-tailscale','Tailscale 节点','实例、登录与出口节点']]]
  ];
  const sceneInfo=Object.fromEntries(sceneGroups.flatMap(([,items])=>items.map(([id,title,desc])=>[id,{title,desc}])));
  const seedNodes = () => [
    {id:'hk1',name:'香港 01',sub:'日常订阅',type:'Trojan',delay:42},
    {id:'hk2',name:'香港 02',sub:'日常订阅',type:'Shadowsocks',delay:56},
    {id:'jp1',name:'日本 01',sub:'日常订阅',type:'VMess',delay:68},
    {id:'sg1',name:'新加坡 01',sub:'日常订阅',type:'Trojan',delay:86},
    {id:'us1',name:'美国 01',sub:'日常订阅',type:'VLESS',delay:null},
    {id:'backup',name:'备用香港',sub:'备用订阅',type:'Trojan',delay:-1}
  ];
  const seedApps=()=>[
    {id:'chrome',name:'Google Chrome',exe:'chrome.exe',path:'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',icon:'LanguageOutlined',on:true,listenerId:'default'},
    {id:'telegram',name:'Telegram',exe:'Telegram.exe',path:'C:\\Apps\\Telegram\\Telegram.exe',icon:'SendOutlined',on:true,listenerId:'default'},
    {id:'wechat',name:'微信',exe:'WeChat.exe',path:'C:\\Apps\\WeChat\\WeChat.exe',icon:'ChatBubbleOutlineOutlined',on:false,listenerId:'default'},
    {id:'steam',name:'Steam',exe:'steam.exe',path:'C:\\Program Files (x86)\\Steam\\steam.exe',icon:'SportsEsportsOutlined',on:false}
  ];
  const defaults=()=>({scene:'welcome',page:'home',mode:'simple',welcomeMode:'simple',nodes:seedNodes(),selected:'hk1',pendingNode:'hk1',connected:false,coreRunning:true,scope:'system',method:'system',apps:seedApps(),appSelection:[],component:true,modal:null,draft:{},errors:{},dirty:false,busy:'',toast:'',error:null,resourceTab:'subscription',overviewTab:'summary',selectedSub:'日常订阅',listeners:[{id:'default',name:'默认监听器',port:7881,type:'混合',exit:'日常出口',enabled:true}],exits:[{name:'日常出口',type:'节点',node:'香港 01'},{name:'开发出口',type:'自动测速',node:'新加坡 01'},{name:'直连',type:'直连',node:'系统默认网卡'}],autostart:false,autoConnect:false,fault:'Block',simulation:'success',testing:false,subError:false,subUpdated:false,fullEnabled:true,tsLogged:false,originalModal:null,formSaved:null,searches:{},rulePending:{},ruleErrors:{},ruleFailedTargets:{}});
  let s=defaults(), operation=0, toastTimer;
  const node=()=>s.nodes.find(n=>n.id===s.selected);
  const latency=n=>n.delay===null?'<span class="dim">未测速</span>':n.delay<0?'<span class="warn">超时</span>':`<span class="good mono">${n.delay} ms</span>`;
  function go(scene, fixture=false){
    if(fixture){const simulation=s.simulation;s=defaults();s.simulation=simulation;initializeScene(scene);}
    else {s.scene=scene;s.modal=null;s.error=null;if(scene.startsWith('full-')){s.mode='full';s.page=scene.slice(5);}else{s.page=scene==='simple-connected'||scene==='simple-home'?'home':scene;}}
    history.replaceState(null,'','#'+scene);render();
  }
  function initializeScene(scene){
    s.scene=scene;
    if(scene==='welcome'){s.page='welcome';s.nodes=[];s.selected=null;return;}
    if(scene.startsWith('full-')){
      s.mode='full';s.page=scene.slice(5);s.connected=true;
      const dialogs={'listener-dialog':['listeners','listener'],'exit-dialog':['exits','exit'],'rule-dialog':['process','rule'],'subscription-dialog':['nodes','subscription']};
      if(dialogs[s.page]){[s.page,s.modal]=dialogs[s.page];initDraft(s.modal);}
      if(s.page==='tailscale'){s.page='nodes';s.resourceTab='tailscale';}
      return;
    }
    s.page='home';
    if(scene==='simple-empty'){s.nodes=[];s.selected=null;}
    if(scene==='simple-connected')s.connected=true;
    if(scene==='simple-apps'||scene==='app-picker'){s.scope='apps';s.connected=true;}
    if(scene==='app-picker'){s.modal='apps';s.appSelection=[];}
    if(scene==='node-picker'){s.connected=true;s.modal='picker';s.pendingNode=s.selected;}
    if(scene==='import'){s.nodes=[];s.selected=null;s.modal='import';initDraft('import');}
    if(scene==='import-success'){s.page='nodes';s.toast='订阅已导入，选择节点后即可连接';}
    if(scene==='nodes'||scene==='settings'||scene==='diagnostics')s.page=scene;
    if(scene==='component'){s.component=false;s.modal='permission';}
    if(scene==='loading')s.page='loading';
    if(scene==='error-connect')s.error='connect';
    if(scene==='error-port')s.error='port';
    if(scene==='error-permission'){s.error='permission';s.component=false;}
    if(scene==='error-import'){s.page='nodes';s.subError=true;s.connected=true;}
  }
  function notify(message){s.toast=message;clearTimeout(toastTimer);renderToast();toastTimer=setTimeout(()=>{s.toast='';renderToast();},4200);}
  function renderToast(){const old=$('.toast');if(old)old.remove();if(s.toast)$('#app').insertAdjacentHTML('beforeend',`<div class="toast" role="status">${escape(s.toast)}</div>`);}
  function windowTitle(){return `<div class="window-title"><span class="app-mark tiny"></span>Routa<span class="window-controls" aria-hidden="true"><span>−</span><span>□</span><span>×</span></span></div>`;}
  function brand(){return `<div class="brand"><span class="app-mark">${icon('RouteOutlined')}</span>Routa${tag('简洁版')}</div>`;}
  function simpleFooter(){return `<div class="simple-footer">${btn(icon('DnsOutlined')+'订阅与节点','page:nodes','ghost')}${btn(icon('SettingsOutlined')+'设置','page:settings','ghost')}</div>`;}
  function connectionError(){
    if(!s.error)return '';
    const errors={connect:['未能开启代理','节点暂时无法连接。可以重试或更换节点。'],port:['端口 7881 被占用','代理尚未开启，请更换端口后重试。'],permission:['未获得管理员权限','已保留原来的连接方式，可以再次尝试。'],node:['节点切换失败','已保留原来的节点和连接。']};
    const [title,desc]=errors[s.error]||errors.connect;
    return `<div class="mb16">${note(`<b>${title}</b><p>${desc}</p><div class="row gap8">${btn(s.error==='port'?'更换端口':'重试',s.error==='port'?'modal:repair':s.error==='permission'?'modal:permission':'connect','link')}${btn('查看诊断','page:diagnostics','link')}</div>`,'error')}</div>`;
  }
  function home(){
    if(!s.nodes.length)return `${brand()}<div class="empty"><span class="empty-icon">${icon('DnsOutlined')}</span><h2>添加节点后即可开始</h2><p>导入订阅，或手动添加一个节点。</p>${btn('导入订阅','modal:import','primary wide')}<div class="mt16">${btn('手动添加节点','modal:manual','link')}</div></div>`;
    const n=node();
    const busy=Boolean(s.busy);
    const controls=btn((busy?'<span class="spinner"></span>':'')+(busy?'正在应用…':s.connected?'关闭代理':'开启代理'),'connect',`${s.connected?'':'primary '}wide connection-button`,busy?'disabled':'');
    const scopeControls=`<div class="range-row"><span class="small muted">代理范围 ${infoTip('系统代理仅影响遵循系统代理设置的应用；指定应用通过分流组件接管。')}</span><div class="segmented">${btn(s.method==='tun'?'TUN 代理':'系统代理','scope:system',s.scope==='system'?'selected':'',s.busy?'disabled':'')}${btn('指定应用','scope:apps',s.scope==='apps'?'selected':'',s.busy?'disabled':'')}</div></div>`;
    const other=s.listeners.length>1?`<div class="mt16">${note(`另有 ${s.listeners.length-1} 个监听器仍在运行。${btn('在完整版查看','full:overview','link')}`)}</div>`:'';
    if(s.scope==='apps')return `${brand()}${connectionError()}<div class="compact-connection"><div class="grow">${status(s.connected?'应用代理已开启':'应用代理未开启',s.connected?'good':'dim')}<h2>${escape(n?.name||'请选择节点')}</h2><div class="row small node-change">${n?latency(n):''}${btn('更换节点','modal:picker','link')}</div></div><div style="width:170px">${controls}</div></div>${scopeControls}<div class="row spread mb16"><h3>指定应用</h3>${btn('＋ 添加应用','modal:apps')}</div><div class="app-grid">${s.apps.map(app=>`<div class="app-tile ${app.on?'enabled':''}"><div class="app-avatar">${icon(app.icon)}</div><div class="grow"><b>${escape(app.name)}</b><p>${escape(app.exe)}</p></div>${toggle(app.name+'走代理',app.on,'app:'+app.id,!!s.busy)}</div>`).join('')}</div><div class="row spread small muted mt16"><span>${s.apps.filter(a=>a.on).length} 个应用已选择，其余应用直连</span>${infoTip('应用代理故障时，选中应用默认阻断；主动关闭代理后恢复直连。')}</div><details class="mt16"><summary>故障时怎么办</summary><div class="detail-content">${faultField()}</div></details>${other}`;
    return `${brand()}${connectionError()}<div class="simple-status"><h2>${status(s.connected?'代理已开启':'代理未开启',s.connected?'good':'dim')}</h2><p>${s.method==='tun'?'TUN · 虚拟网卡':'系统代理'}</p></div><div class="node-label">当前节点</div><button class="node-pick" data-action="modal:picker">${icon('LanguageOutlined')}<div class="grow"><strong>${escape(n?.name||'请选择节点')}</strong><p>${escape(n?.sub||'未选择')}</p></div><span class="latency">${n?latency(n):''}</span>${icon('ChevronRightOutlined')}</button>${controls}${scopeControls}<div class="speeds"><div><span class="small dim">↓ 下载</span><div class="rate">${s.connected?'2.4 MB/s':'—'}</div></div><div><span class="small dim">↑ 上传</span><div class="rate">${s.connected?'128 KB/s':'—'}</div></div></div><details><summary>连接选项</summary><div class="detail-content stack"><label class="field">连接方式<select data-change="method"><option value="system" ${s.method==='system'?'selected':''}>系统代理</option><option value="tun" ${s.method==='tun'?'selected':''}>TUN（虚拟网卡）</option></select></label><div class="row spread small"><span class="muted">本地端口</span><span class="mono">127.0.0.1:${s.listeners[0].port} · 混合</span></div>${btn('修改端口','modal:repair','link')}</div></details>${other}`;
  }
  function infoTip(text){return `<span title="${escape(text)}" tabindex="0" aria-label="${escape(text)}" class="small-icon">${icon('InfoOutlined')}</span>`;}
  function welcome(){return `${brand()}<div class="welcome"><h1>先选一种使用方式</h1><p class="muted small">以后可在设置中更改</p>${[['simple','简洁版','选节点、开代理，按需选择应用'],['full','完整版','多监听器、多出口与精细分流']].map(([id,title,desc])=>`<button class="mode-card ${s.welcomeMode===id?'selected':''}" data-action="welcome-mode:${id}" aria-pressed="${s.welcomeMode===id}"><span class="radio-dot"></span><div class="grow"><strong>${title}</strong><p>${desc}</p></div>${id==='simple'?tag('推荐'):''}</button>`).join('')}${btn('开始使用','start','primary wide')}<p class="hint">切换界面会保留已有配置</p></div>`;}
  function backHead(title){return `<div class="back-heading">${btn(icon('ArrowBackOutlined'),'page:home','ghost icon-button','aria-label="返回连接"')}<h1>${title}</h1></div>`;}
  function faultField(){return `<label class="field">应用代理意外故障时<select data-change="fault"><option value="Block" ${s.fault==='Block'?'selected':''}>阻断选中应用（推荐）</option><option value="Direct" ${s.fault==='Direct'?'selected':''}>允许选中应用直连</option></select></label>`;}
  function settings(){
    const full=s.mode==='full';
    return `${full?'':backHead('设置')}<section class="settings-section"><h2>界面</h2><div class="card"><div class="setting-row"><div class="grow"><b>界面模式</b><p>切换后保留现有连接和配置</p></div><select aria-label="界面模式" data-change="mode"><option value="simple" ${!full?'selected':''}>简洁版</option><option value="full" ${full?'selected':''}>完整版</option></select></div></div></section><section class="settings-section"><h2>常规</h2><div class="card"><div class="setting-row"><div class="grow">开机启动</div>${toggle('开机启动',s.autostart,'autostart')}</div><div class="setting-row"><div class="grow">启动后自动连接<p>仅在已有可用节点时连接</p></div>${toggle('启动后自动连接',s.autoConnect,'autoConnect')}</div><div class="setting-row"><div class="grow">关闭窗口时</div><select aria-label="关闭窗口时"><option>最小化到托盘</option><option>退出程序</option></select></div></div></section>${full?`<section class="settings-section"><h2>内核与测速</h2><form id="settings-form" class="card pad20 stack"><div class="form-grid">${field('控制端口','controller','9090','number')}${field('自动测速间隔（分钟）','interval','10','number')}</div>${field('测速地址','test-url','https://www.gstatic.com/generate_204')}${field('内核路径','core','', 'text','留空使用内置内核')}<div class="form-actions">${btn('保存设置','save-settings','primary','type="button"')}</div></form></section>`:''}<section class="settings-section"><div class="card"><div class="setting-row"><span class="grow">故障诊断</span>${btn('打开','page:diagnostics','link')}</div>${full?`<div class="setting-row"><span class="grow">Tailscale 实例</span>${btn('管理','tailscale','link')}</div>`:''}<div class="setting-row"><span class="grow">关于 Routa</span><span class="small dim">v0.2.0</span></div></div></section>`;
  }
  function resources(full=false){
    const subscriptions=[...new Set(s.nodes.map(n=>n.sub))];
    const tabs=full?`<div class="tabs">${[['subscription','订阅'],['manual','手动节点'],['tailscale','Tailscale']].map(([id,label])=>btn(label,'resource-tab:'+id,s.resourceTab===id?'active':'')).join('')}</div>`:'';
    let content='';
    if(s.resourceTab==='tailscale'&&full){
      content=`<div class="row spread mb16"><span class="small muted">Tailscale 节点与出口实例</span>${btn('＋ 添加实例','modal:tailscale','primary')}</div><div class="card pad20"><div class="row"><span class="app-avatar">${icon('HubOutlined')}</span><div class="grow"><h3>家庭网络</h3><p class="small muted mt16">home-router · exit-node</p></div>${status(s.tsLogged?'已登录':'未登录',s.tsLogged?'good':'warn')}</div><div class="form-actions">${btn(s.tsLogged?'查看实例':'登录 Tailscale','ts-login','primary')}${btn('编辑','modal:tailscale')}</div></div>`;
    }else if(s.resourceTab==='manual'&&full){
      const manual=s.nodes.filter(n=>n.sub==='手动节点');
      content=`<div class="row spread mb16"><span class="small muted">手动维护的独立节点</span>${btn('＋ 添加节点','modal:manual','primary')}</div>${manual.length?`<div class="card">${manual.map(n=>`<div class="setting-row"><div class="grow"><b>${escape(n.name)}</b><p>${escape(n.type)}</p></div>${latency(n)}${btn('使用','use-node:'+n.id,'link')}</div>`).join('')}</div>`:`<div class="empty card"><h2>还没有手动节点</h2><p>支持 HTTP、SOCKS5 和 YAML 配置。</p>${btn('添加节点','modal:manual','primary')}</div>`}`;
    }else{
      content=`<div class="row spread mb16"><span class="small muted">${subscriptions.length} 个订阅 · ${s.nodes.length} 个节点</span>${btn('＋ 导入订阅','modal:import','primary')}</div>${s.subError?`<div class="mb16">${note(`<b>日常订阅更新失败</b><p>无法连接订阅地址，原有节点仍可使用。</p>${btn('重试更新','refresh-sub','link')}`,'error')}</div>`:''}${!subscriptions.length?`<div class="empty card"><h2>还没有订阅</h2><p>导入订阅后，在连接页选择节点。</p>${btn('导入订阅','modal:import','primary')}</div>`:subscriptions.map((name,i)=>`<div class="card mb16"><div class="setting-row"><div class="grow"><b>${escape(name)}</b><p>${s.nodes.filter(n=>n.sub===name).length} 个节点 · ${s.subUpdated?'刚刚更新':'今天已更新'}</p></div>${btn(s.busy==='refresh'?'更新中…':'更新','refresh-sub','',s.busy?'disabled':'')}${btn(icon('EditOutlined'),'edit-sub:'+i,'ghost icon-button',`aria-label="编辑${escape(name)}"`)}${btn(icon('DeleteOutlineOutlined'),'delete-sub:'+i,'ghost icon-button',`aria-label="删除${escape(name)}"`)}</div></div>`).join('')}<div class="toolbar"><h3>节点</h3>${btn(s.testing?'测速中…':'测速','test-nodes','',s.testing?'disabled':'')}</div><input class="search mb16" aria-label="搜索节点" placeholder="搜索节点名称或订阅" data-filter="resource-nodes"><div class="card table-wrap"><table><thead><tr><th>节点</th>${full?'<th>类型</th>':''}<th>延迟</th><th></th></tr></thead><tbody>${s.nodes.map(n=>`<tr data-search="resource-nodes" data-text="${escape(n.name+' '+n.sub)}"><td><b>${escape(n.name)}</b><p>${escape(n.sub)}</p></td>${full?`<td>${escape(n.type)}</td>`:''}<td>${s.testing?'<span class="spinner"></span>':latency(n)}</td><td>${s.selected===n.id?tag('当前'):btn('使用','use-node:'+n.id,'link',n.delay<0?'disabled':'')}</td></tr>`).join('')||'<tr><td colspan="4" class="muted">暂无节点</td></tr>'}</tbody></table></div>`;
    }
    return `${full?'':backHead('订阅与节点')}${tabs}${content}`;
  }
  function diagnostics(){return `${s.mode==='simple'?backHead('故障诊断'):''}<div class="stack">${note('检查结果为演示数据。真实诊断会读取运行状态，不自动修改配置。')}<div class="card">${[['mihomo 内核',s.coreRunning?'运行正常':'已停止',s.coreRunning?'good':'dim'],['节点连通性',s.error?'检查未通过':'可连接',s.error?'warn':'good'],['本地端口',s.error==='port'?'7881 被占用':`${s.listeners[0].port} 可用`,s.error==='port'?'bad':'good'],['系统代理',s.connected&&s.scope==='system'?'已开启':'未开启',s.connected&&s.scope==='system'?'good':'dim'],['应用分流组件',s.component?'可用':'尚未安装',s.component?'good':'warn']].map(([a,b,c])=>`<div class="setting-row"><span class="grow">${a}</span>${status(b,c)}</div>`).join('')}</div><div class="row">${btn('重新检查','diagnose','primary')}${btn('查看日志','modal:logs')}</div><details><summary>技术详情</summary><div class="log-lines">示例：controller=127.0.0.1:9090\nlistener=127.0.0.1:${s.listeners[0].port}\ncore=running</div></details></div>`;}
  const menu=[['overview','概览','SpaceDashboardOutlined'],['listeners','代理','SwapHorizOutlined'],['process','进程分流','AltRouteOutlined'],['exits','出口','LanguageOutlined'],['nodes','订阅与节点','DnsOutlined']];
  function fullShell(){
    const current=menu.find(m=>m[0]===s.page);
    const title=current?.[1]||({settings:'设置',diagnostics:'故障诊断'}[s.page]||'概览');
    const nav=menu.map(([id,label,name])=>btn(icon(name)+label,'full:'+id,s.page===id?'active':'')).join('');
    let body={overview:overview,listeners:listeners,process:processPage,exits:exitsPage,nodes:()=>resources(true),settings,diagnostics}[s.page]?.()||overview();
    const headActions={listeners:btn('＋ 新建监听器','modal:listener','primary'),process:btn('＋ 添加规则','modal:rule','primary'),exits:btn('＋ 新建出口','modal:exit','primary')};
    return `<div class="full-layout"><aside class="sidebar"><div class="brand"><span class="app-mark">${icon('RouteOutlined')}</span>Routa</div><nav aria-label="主导航">${nav}</nav><div class="bottom-nav">${btn(icon('SettingsOutlined')+'设置','full:settings',`nav-item ${s.page==='settings'?'active':''}`)}<div class="version">v0.2.0</div></div></aside><main class="full-main"><div class="page-head"><div><h1>${title}</h1>${s.page==='listeners'?'<p>默认提供一个 7881 混合监听器</p>':''}</div>${headActions[s.page]||(s.page==='overview'?'':status(s.coreRunning?'内核运行中':'内核已停止',s.coreRunning?'good':'dim'))}</div>${body}</main></div>`;
  }
  function overview(){return `<div class="card setting-row"><span class="status good"><span class="dot"></span></span><div class="grow"><b>${s.coreRunning?'内核运行中':'内核已停止'}</b><p>控制端口 9090</p></div>${btn(s.coreRunning?'重启':'启动','core-restart')}${s.coreRunning?btn('停止','core-stop'):''}</div><div class="metrics">${[['下载速度',s.connected?'2.4 MB/s':'—'],['上传速度',s.connected?'128 KB/s':'—'],['当前连接',s.connected?'18':'0']].map(([a,b])=>`<div class="card metric"><p>${a}</p><strong class="mono">${b}</strong></div>`).join('')}</div><div class="tabs">${[['summary','运行概况'],['connections','连接'],['logs','日志']].map(([id,label])=>btn(label,'overview-tab:'+id,s.overviewTab===id?'active':'')).join('')}</div>${s.overviewTab==='logs'?`<div class="card log-lines">[INFO] mihomo core started\n[INFO] Listener 127.0.0.1:${s.listeners[0].port} active\n[INFO] Configuration applied\n[INFO] Subscription refresh completed</div>`:s.overviewTab==='connections'?`<div class="card table-wrap"><table><thead><tr><th>应用</th><th>目标</th><th>监听器</th><th>流量</th></tr></thead><tbody><tr><td>chrome.exe</td><td>example.invalid:443</td><td>7881</td><td>2.4 MB</td></tr><tr><td>Telegram.exe</td><td>149.154.*.*:443</td><td>7881</td><td>128 KB</td></tr></tbody></table></div>`:`<div class="card pad20 mb16"><div class="row spread"><h3>流量趋势</h3><span class="small dim">最近 60 秒 · 示例</span></div><div class="chart" role="img" aria-label="示例下载速度趋势">${[12,18,15,24,34,21,31,39,49,29,43,64,51,36,74,48,58,63,39,51,34,29,42,27,31,20,30,42,28,23].map(h=>`<span style="height:${h}px"></span>`).join('')}</div></div><div class="toolbar"><h3>代理状态</h3>${btn('管理代理','full:listeners','link')}</div>${listenerTable()}`}`;}
  function listenerTable(){return `<div class="card table-wrap"><table><thead><tr><th>监听器</th><th>出口</th><th>状态</th><th></th></tr></thead><tbody>${s.listeners.map(l=>`<tr><td><b>${escape(l.name)}</b><p>127.0.0.1:${l.port} · ${escape(l.type)}</p></td><td>${escape(l.exit)}</td><td>${status(l.enabled?(s.coreRunning?'运行中':'内核未运行'):'已停用',l.enabled&&s.coreRunning?'good':'dim')}</td><td>${btn('管理','full:listeners','link')}</td></tr>`).join('')}</tbody></table></div>`;}
  function listeners(){return s.listeners.map((l,i)=>`<div class="card listener-card"><div class="listener-head">${icon('SwapHorizOutlined')}<h3>${escape(l.name)}</h3>${tag(escape(l.type))}${status(l.enabled?(s.coreRunning?'运行中':'内核未运行'):'已停用',l.enabled&&s.coreRunning?'good':'dim')}<span class="grow"></span>${toggle(l.name+'启用',l.enabled,'listener-toggle:'+i)}${btn(icon('EditOutlined'),'edit-listener:'+i,'ghost icon-button',`aria-label="编辑${escape(l.name)}"`)}${btn(icon('DeleteOutlineOutlined'),'delete-listener:'+i,'ghost icon-button',`aria-label="删除${escape(l.name)}"`)}</div><div class="listener-info"><div><span class="small muted">监听地址</span><strong class="mono">127.0.0.1:${l.port}</strong><div class="row small muted mt16"><span>${l.type==='混合'?'HTTP / SOCKS5':escape(l.type)}</span>${btn('复制','copy-listener:'+i,'link')}</div></div><div><label class="small muted">出口<select aria-label="${escape(l.name)}出口" data-listener-exit="${i}">${s.exits.map(e=>`<option ${l.exit===e.name?'selected':''}>${escape(e.name)}</option>`).join('')}</select></label><p class="small muted mt16">${escape(s.exits.find(e=>e.name===l.exit)?.node||'未选择')}</p></div></div><div class="listener-foot"><span>关联进程 ${s.apps.filter(a=>a.on&&(a.listenerId||'default')===l.id).length}</span>${btn('查看规则','full:process','link')}<span class="grow"></span>${i===0?btn(s.connected?'系统代理已开启':'设为系统代理','listener-system:'+i,'link'):''}</div></div>`).join('')+`<details><summary>接管方式与高级选项</summary><div class="detail-content">${note(`按需设置系统代理或虚拟网卡接管。${btn('查看连接设置','switch-simple','link')}`)}</div></details>`;}
  function usableForProcess(listener){return listener&&listener.type!=='HTTP';}
  function assignedListener(app){return s.listeners.find(l=>l.id===(app.listenerId||'default'));}
  function ruleListenerCell(app){
    const current=assignedListener(app),pending=s.rulePending[app.id];
    const value=pending||app.listenerId||'default';
    const options=s.listeners.filter(usableForProcess);
    const missing=!options.some(l=>l.id===value);
    return `<div class="rule-listener"><select aria-label="${escape(app.name)}监听器" data-rule-listener="${app.id}" ${pending||s.busy?'disabled':''} aria-busy="${Boolean(pending)}">${missing?'<option value="" selected disabled>监听器不可用</option>':''}${options.map(l=>`<option value="${escape(l.id)}" ${value===l.id?'selected':''} ${!l.enabled?'disabled':''}>${l.port} · ${escape(l.name)}${l.enabled?'':'（已停用）'}</option>`).join('')}<option value="__new__">＋ 新建监听器…</option></select>${pending?'<p class="rule-feedback muted" role="status"><span class="spinner"></span> 正在应用…</p>':s.ruleErrors[app.id]?`<p class="rule-feedback bad" role="alert">${escape(s.ruleErrors[app.id])} ${btn('重试','retry-listener:'+app.id,'link')}</p>`:''}</div>`;
  }
  function processPage(){return `<div class="card setting-row"><div class="grow"><b>进程分流</b><p>${s.component?'组件就绪':'尚未安装分流组件'}</p></div>${toggle('进程分流',s.fullEnabled,'full-enabled')}</div><div class="toolbar"><input class="search" placeholder="搜索进程名称或路径" aria-label="搜索进程" data-filter="rules"><span class="small muted">${s.apps.filter(a=>a.on).length} 条规则已启用</span></div><div class="card table-wrap"><table><thead><tr><th>进程</th><th>监听器 ${infoTip('选择后自动保存。后续新连接使用新监听器，不强制断开已有连接。')}</th><th>状态</th><th>启用</th><th></th></tr></thead><tbody>${s.apps.map((a,i)=>{
    const target=assignedListener(a),pending=s.rulePending[a.id];
    const ready=usableForProcess(target)&&target.enabled&&s.coreRunning;
    const label=pending?'应用中':!a.on||!s.fullEnabled?'直连':ready?'已代理':'已阻断';
    return `<tr data-search="rules" data-text="${escape(a.name+' '+a.path)}"><td><b>${escape(a.exe)}</b><p>${escape(a.name)}</p></td><td>${ruleListenerCell(a)}</td><td>${status(label,pending?'dim':label==='已代理'?'good':label==='已阻断'?'warn':'dim')}</td><td>${toggle(a.name+'规则',a.on,'app:'+a.id,Boolean(pending)||Boolean(s.busy))}</td><td>${btn('编辑','edit-rule:'+i,'link',pending||s.busy?'disabled':'')}</td></tr>`;
  }).join('')}</tbody></table></div><details class="mt24"><summary>退出行为与故障策略</summary><div class="detail-content stack">${faultField()}<label class="field">主动退出 Routa 时<select><option>恢复直连</option><option>阻断选中应用</option></select></label><span class="small muted">节点或出口失效时阻断受影响应用，避免流量意外直连。</span></div></details>`;}
  function changeRuleListener(appId,targetId){
    const app=s.apps.find(a=>a.id===appId);
    if(!app||s.rulePending[appId]||s.busy)return;
    if(targetId==='__new__'){
      openModal('listener');s.draft.ruleTargetId=appId;render();return;
    }
    const target=s.listeners.find(l=>l.id===targetId);
    if(!usableForProcess(target)||!target.enabled){render();return;}
    if((app.listenerId||'default')===targetId){render();return;}
    const stateAtStart=s,simulation=s.simulation;
    s.rulePending[appId]=targetId;delete s.ruleErrors[appId];render();
    setTimeout(()=>{
      if(s!==stateAtStart)return;
      const stillAvailable=s.listeners.find(l=>l.id===targetId);
      delete s.rulePending[appId];
      if(simulation!=='success'||!usableForProcess(stillAvailable)||!stillAvailable.enabled){
        const old=assignedListener(app);
        s.ruleErrors[appId]=old?`切换失败，仍使用 ${old.port}`:'切换失败，请重新选择';
        s.ruleFailedTargets[appId]=targetId;
      }else{
        app.listenerId=targetId;delete s.ruleErrors[appId];delete s.ruleFailedTargets[appId];
        notify(`${app.name} 已切换到 ${target.port}${app.on?'':'，规则仍未启用'}`);
      }
      render();
    },650);
  }
  function exitsPage(){return `<div class="exit-grid">${s.exits.map((e,i)=>`<div class="card exit-card"><div class="row"><h3 class="grow">${escape(e.name)}</h3>${status('正常')}</div><div class="meta">${escape(e.type)}<br><b>${escape(e.node)}</b><br>网卡：系统默认</div><div class="actions">${btn('编辑','edit-exit:'+i,'link')}${btn('测速','test-exit:'+i,'link')}${btn('删除','delete-exit:'+i,'link')}</div></div>`).join('')}</div>`;}
  function field(label,name,value='',type='text',placeholder=''){
    const error=s.errors[name];
    return `<label class="field">${label}<input name="${name}" type="${type}" value="${escape(s.draft[name]??value)}" placeholder="${escape(placeholder)}" ${error?'aria-invalid="true"':''}>${error?`<span class="field-error">${escape(error)}</span>`:''}</label>`;
  }
  function selectField(label,name,options,value){return `<label class="field">${label}<select name="${name}" ${['exit-type','rule-app'].includes(name)?'data-form-select="'+name+'"':''}>${options.map(([id,label])=>`<option value="${escape(id)}" ${(s.draft[name]??value)===id?'selected':''}>${escape(label)}</option>`).join('')}</select></label>`;}
  function initDraft(type){
    s.errors={};s.dirty=false;
    const drafts={import:{name:'',url:''},subscription:{name:s.selectedSub,url:'https://example.invalid/subscription',interval:'1440'},manual:{name:'',server:'',port:'1080',protocol:'socks5'},listener:{name:'',port:String(s.listeners.length===1?7882:7883),type:'混合',exit:s.exits[0]?.name||'',listen:'127.0.0.1'},exit:{name:'','exit-type':'Node',node:'hk1',strategy:'UrlTest'},rule:{'rule-app':'chrome',listener:s.listeners.find(l=>usableForProcess(l)&&l.enabled)?.id||'',fault:'Block'},tailscale:{name:'家庭网络',hostname:'routa-home','exit-node':'home-router'},repair:{port:String(s.error==='port'?7882:s.listeners[0].port)}};
    s.draft=drafts[type]||{};
  }
  function openModal(type){if(s.busy)return;s.modal=type;initDraft(type);if(type==='picker')s.pendingNode=s.selected;if(type==='apps')s.appSelection=[];render();}
  function modalFrame(title,body,actions,wider=false){return `<div class="overlay"><section class="dialog ${wider?'wider':''}" role="dialog" aria-modal="true" aria-label="${escape(title)}" tabindex="-1"><header class="dialog-head"><h2>${title}</h2>${btn(icon('CloseOutlined'),'close-modal','ghost icon-button',`aria-label="关闭${escape(title)}" ${s.busy?'disabled':''}`)}</header><div class="dialog-body">${body}</div><footer class="dialog-actions">${actions}</footer></section></div>`;}
  function modalContent(){
    const type=s.modal;if(!type)return '';
    const cancel=btn('取消','close-modal','',s.busy?'disabled':'');
    const saving=s.busy?'<span class="spinner"></span> 正在处理…':'保存';
    const commonError=s.errors.form?`<div class="mb16">${note(escape(s.errors.form),'error')}</div>`:'';
    if(type==='import'||type==='subscription')return modalFrame(type==='import'?'导入订阅':'编辑订阅',`<form id="active-form" class="stack">${commonError}${field('订阅链接','url','','url','https://…')}${field('名称（可选）','name','','text','根据订阅地址自动命名')}<details><summary>更多选项</summary><div class="detail-content stack">${field('自动更新间隔（分钟）','interval','1440','number')}${s.mode==='full'?selectField('拉取失败时使用出口','fallback',[['','不使用'],...s.exits.map(e=>[e.name,e.name])],''):''}</div></details></form>`,`${cancel}${btn(s.busy?saving:type==='import'?'导入':'保存','save-subscription','primary',s.busy?'disabled':'')}`);
    if(type==='picker')return modalFrame('选择节点',`<div class="row spread mb16"><span class="small muted">${s.nodes.length} 个可用选择</span>${btn(s.testing?'测速中…':'测速','test-nodes','',s.testing?'disabled':'')}</div><input class="search" aria-label="搜索可选节点" placeholder="搜索节点或订阅" data-filter="picker"><div class="select-list">${s.nodes.map(n=>`<button class="select-row ${s.pendingNode===n.id?'selected':''}" data-action="pending-node:${n.id}" data-search="picker" data-text="${escape(n.name+' '+n.sub)}" ${n.delay<0?'disabled':''} aria-pressed="${s.pendingNode===n.id}"><span class="radio-dot"></span><div class="grow"><b>${escape(n.name)}</b><p>${escape(n.sub)} · ${escape(n.type)}</p></div>${s.testing?'<span class="spinner"></span>':latency(n)}</button>`).join('')}</div>${s.error==='node'?`<div class="mt16">${note('切换失败，原节点仍在使用。','error')}</div>`:''}`,`<span class="hint">${s.connected?'切换后新连接使用所选节点':'选择不会自动开启代理'}</span>${cancel}${btn(s.busy?'<span class="spinner"></span> 切换中…':'使用此节点','apply-node','primary',s.busy||!s.pendingNode?'disabled':'')}`);
    if(type==='apps')return modalFrame('添加应用',`<div class="tabs">${btn('正在运行','apps-running','active')}${btn('选择程序文件','apps-file','')}</div><input class="search" aria-label="搜索运行应用" placeholder="搜索应用名称或路径" data-filter="app-picker"><div class="select-list">${seedApps().map(a=>`<label class="select-row" data-search="app-picker" data-text="${escape(a.name+' '+a.path)}"><input type="checkbox" value="${a.id}" data-app-select ${s.appSelection.includes(a.id)?'checked':''}><span class="app-avatar">${icon(a.icon)}</span><span class="grow"><b>${escape(a.name)}</b><p>${escape(a.exe)}</p></span></label>`).join('')}</div><details class="mt16"><summary>应用故障时的处理</summary><div class="detail-content">${faultField()}</div></details>`,`<span class="hint" id="app-selection-count">已选择 ${s.appSelection.length} 项</span>${cancel}${btn('添加所选应用','add-apps','primary',s.appSelection.length?'':'disabled')}`);
    if(type==='permission')return modalFrame(s.permissionKind==='tun'?'启用 TUN':'启用应用代理',`<p class="muted small" style="line-height:1.8">${s.permissionKind==='tun'?'启用虚拟网卡需要配置 TUN 服务，并请求管理员权限。':'首次使用需要安装分流组件。安装时 Windows 会请求管理员权限。'}</p><div class="mt16">${note('未授权或安装失败时，保留原来的连接方式。')}</div>${commonError}`,`${btn('暂不启用','close-modal','',s.busy?'disabled':'')}${btn(s.busy?saving:'安装并启用','install-component','primary',s.busy?'disabled':'')}`);
    if(type==='listener')return modalFrame(s.draft.editIndex!==undefined?'编辑监听器':'新建监听器',`<form id="active-form" class="stack">${commonError}${field('名称','name','','text','例如：开发代理')}<div class="form-grid">${field('端口','port','7882','number')}${selectField('协议','type',[['混合','混合（HTTP + SOCKS5）'],['HTTP','HTTP'],['SOCKS5','SOCKS5']].filter(([type])=>!s.draft.ruleTargetId||type!=='HTTP'),'混合')}</div>${selectField('出口','exit',s.exits.map(e=>[e.name,e.name]),s.exits[0]?.name)}<details><summary>访问与认证</summary><div class="detail-content stack">${field('监听地址','listen','127.0.0.1')}${field('用户名（可选）','user')}${field('密码','password','','password')}</div></details></form>`,`${cancel}${btn(saving,'save-listener','primary',s.busy?'disabled':'')}`);
    if(type==='exit')return modalFrame(s.draft.editIndex!==undefined?'编辑出口':'新建出口',`<form id="active-form" class="stack">${commonError}${field('名称','name','','text','例如：日常出口')}${selectField('出口类型','exit-type',[['Node','单个节点'],['Group','策略组'],['Direct','直连']],'Node')}${s.draft['exit-type']==='Direct'?'':s.draft['exit-type']==='Group'?`${selectField('选择策略','strategy',[['UrlTest','自动测速'],['Fallback','故障转移'],['LoadBalance','负载均衡'],['Select','手动选择']],'UrlTest')}<label class="field">成员节点</label><div class="select-list">${s.nodes.slice(0,4).map(n=>`<label class="select-row"><input type="checkbox" name="members" value="${n.id}" ${s.draft.members?.includes(n.id)?'checked':''}>${escape(n.name)}</label>`).join('')}</div>`:selectField('使用节点','node',s.nodes.map(n=>[n.id,n.name]),'hk1')}<details><summary>网卡绑定</summary><div class="detail-content">${selectField('出口网卡','interface',[['default','系统默认'],['ethernet','以太网'],['wifi','Wi-Fi']],'default')}</div></details></form>`,`${cancel}${btn(saving,'save-exit','primary',s.busy?'disabled':'')}`);
    if(type==='rule')return modalFrame(s.draft.editIndex!==undefined?'编辑分流规则':'新建分流规则',`<form id="active-form" class="stack">${commonError}${selectField('应用','rule-app',s.apps.map(a=>[a.id,`${a.name} · ${a.exe}`]),'chrome')}${selectField('监听器','listener',s.listeners.filter(usableForProcess).map(l=>[l.id,`${l.port} · ${l.name}`]),s.listeners.find(usableForProcess)?.id||'')}${selectField('代理意外故障时','fault',[['Block','阻断此应用（推荐）'],['Direct','允许直连']],'Block')}<details><summary>更多选项</summary><div class="detail-content"><label class="row small"><input type="checkbox" name="reset">规则修改后断开已有 IPv4 TCP 连接</label></div></details></form>`,`${cancel}${btn(saving,'save-rule','primary',s.busy?'disabled':'')}`);
    if(type==='manual')return modalFrame('添加手动节点',`<form id="active-form" class="stack">${commonError}${field('名称','name')}${selectField('协议','protocol',[['socks5','SOCKS5'],['http','HTTP']],'socks5')}<div class="form-grid">${field('服务器','server','','text','example.invalid')}${field('端口','port','1080','number')}</div><details><summary>认证与其他协议</summary><div class="detail-content stack">${field('用户名','username')}${field('密码','password','','password')}<label class="field">或粘贴单节点 YAML<textarea name="yaml" placeholder="name: …&#10;type: …"></textarea></label></div></details></form>`,`${cancel}${btn(saving,'save-manual','primary',s.busy?'disabled':'')}`);
    if(type==='tailscale')return modalFrame('Tailscale 实例',`<form id="active-form" class="stack">${field('名称','name','家庭网络')}${field('实例主机名','hostname','routa-home')}${field('出口节点','exit-node','home-router')}<details><summary>高级选项</summary><div class="detail-content stack">${field('Auth key（可选）','key','','password')}<label class="row small"><input type="checkbox" checked>启用 UDP</label></div></details><span class="small muted">保存后可在实例卡片中登录。</span></form>`,`${cancel}${btn('保存实例','save-ts','primary')}`);
    if(type==='repair')return modalFrame('修改本地端口',`<form id="active-form" class="stack">${commonError}${s.error==='port'?note('7881 已被其他程序占用，请换用一个空闲端口。','error'):''}${field('端口','port',String(s.listeners[0].port),'number')}<p class="small muted">修改后同步更新此代理对应的系统代理和应用规则。</p></form>`,`${cancel}${btn('保存端口','save-port','primary')}`);
    if(type==='delete')return modalFrame('确认删除',`<p style="line-height:1.8">${escape(s.draft.description||'删除后将移除此项配置。')}</p>${s.draft.used?`<div class="mt16">${note('仍有配置正在引用此项。请先选择替代目标，再删除。','error')}</div><div class="mt16">${selectField('替代目标','replacement',s.draft.alternatives||[],'')}</div>`:''}`,`${cancel}${btn('删除','confirm-delete','danger',s.draft.used&&!s.draft.alternatives?.length?'disabled':'')}`);
    if(type==='unsaved')return modalFrame('放弃未保存的修改？','<p class="small muted">已保存的配置不会受到影响。</p>',`${btn('继续编辑','continue-edit')}${btn('放弃修改','discard-edit','danger')}`);
    if(type==='logs')return modalFrame('运行日志',`<div class="log-lines">[INFO] Local prototype initialized\n[INFO] Listener ${s.listeners[0].port} active\n[INFO] Selected node: ${escape(node()?.name||'none')}\n[INFO] No OS proxy settings changed</div>`,btn('关闭','close-modal'));
    if(type==='file')return modalFrame('选择程序文件',`<form id="active-form" class="stack">${field('程序路径','path','','text','C:\\Apps\\Example\\example.exe')}<span class="small muted">实际产品使用 Windows 文件选择器；此处用路径输入演示。</span></form>`,`${cancel}${btn('添加应用','add-file','primary')}`);
    return '';
  }

  function applyFilter(input){
    const group=input.dataset.filter,q=input.value.trim().toLowerCase();
    const rows=[...document.querySelectorAll(`[data-search="${group}"]`)];
    rows.forEach(el=>el.hidden=!el.dataset.text.toLowerCase().includes(q));
    let empty=document.querySelector(`[data-empty-search="${group}"]`);
    if(!empty){empty=document.createElement('p');empty.dataset.emptySearch=group;empty.className='small muted mt16';empty.textContent='未找到匹配项';input.insertAdjacentElement('afterend',empty);}
    empty.hidden=!q||rows.some(el=>!el.hidden);
  }
  function render(){
    const app=$('#app');
    app.className='product-window'+(s.mode==='full'?' full':s.page==='home'&&s.scope==='apps'?' expanded':'');
    app.innerHTML=windowTitle()+(s.mode==='full'?fullShell():`<div class="simple-content">${({welcome,home,nodes:resources,settings,diagnostics,loading:()=>`${brand()}<div class="stack" aria-label="正在加载"><div class="skeleton" style="width:40%;height:22px"></div><div class="skeleton" style="height:92px"></div><div class="skeleton" style="height:42px"></div><div class="skeleton" style="width:70%"></div></div><div class="mt24">${btn('重试加载','loaded','link')}</div>`}[s.page]||home)()}</div>${s.page==='welcome'?'':simpleFooter()}`)+modalContent();
    $('#scene-title').textContent=sceneInfo[s.scene]?.title||'交互演示';
    $('#scene-desc').textContent=sceneInfo[s.scene]?.desc||'所有配置操作均为本地模拟';
    document.querySelectorAll('[data-scene]').forEach(b=>b.classList.toggle('active',b.dataset.scene===s.scene));
    renderToast();
    document.querySelectorAll('[data-filter]').forEach(input=>{input.value=s.searches[input.dataset.filter]||'';applyFilter(input);});
    if(s.modal){const d=$('.dialog');d?.focus({preventScroll:true});}else if(s.restoreFocus){s.restoreFocus=false;const trigger=[...document.querySelectorAll('#app [data-action]')].find(el=>el.dataset.action===s.focusRestore);(trigger||$('#app [data-action]'))?.focus({preventScroll:true});}
  }

  function captureDraft(){
    const form=$('#active-form');if(!form)return;
    const fd=new FormData(form);
    s.draft={...s.draft,...Object.fromEntries(fd.entries())};
    if(form.querySelector('[name="members"]'))s.draft.members=fd.getAll('members');
  }
  function invalid(errors){s.errors=errors;render();$('.dialog [aria-invalid=true]')?.focus();}
  function portValid(value){return /^\d+$/.test(String(value))&&Number(value)>=1&&Number(value)<=65535;}
  function syncDefaultExit(){
    const current=node();if(!current)return;
    const listener=s.listeners.find(l=>l.id==='default')||s.listeners[0];
    let exit=s.exits.find(e=>e.name===listener.exit&&e.type==='节点');
    if(!exit){
      const base='默认代理';let name=base,index=2;
      while(s.exits.some(e=>e.name===name))name=base+' '+index++;
      exit={name,type:'节点',node:current.name};s.exits.push(exit);
    }
    exit.node=current.name;listener.exit=exit.name;
  }
  function asyncOperation(name,done,failed){
    if(s.busy)return;
    const id=++operation;s.errors={};s.busy=name;render();
    setTimeout(()=>{if(id!==operation)return;s.busy='';if(s.simulation!=='success'&&failed)failed(s.simulation);else done();render();},650);
  }
  function closeModal(force=false){
    if(s.busy)return;
    if(s.dirty&&!force&&$('#active-form')){captureDraft();s.originalModal=s.modal;s.modal='unsaved';render();return;}
    s.modal=null;s.dirty=false;s.errors={};s.error=s.error==='node'?null:s.error;s.restoreFocus=true;render();
  }
  function chooseScope(scope){
    if(scope===s.scope||s.busy)return;
    if(scope==='apps'&&!s.component){s.modal='permission';s.permissionKind='apps';render();return;}
    const apply=()=>{s.scope=scope;s.error=null;if(scope==='apps')s.fullEnabled=s.connected;notify(scope==='apps'?'已切换为指定应用':'已切换为系统代理');};
    if(s.connected)asyncOperation('scope',apply,reason=>{s.error=reason==='permission'?'permission':'connect';});
    else{apply();render();}
  }
  function saveSubscription(){
    captureDraft();const d=s.draft;const errors={};
    try{const u=new URL(d.url||'');if(!['http:','https:'].includes(u.protocol))throw Error();}catch{errors.url='请输入有效的 http 或 https 订阅链接';}
    if(d.interval!==undefined&&(!/^\d+$/.test(d.interval)||Number(d.interval)<1))errors.interval='请输入大于 0 的更新间隔';
    if(Object.keys(errors).length){invalid(errors);return;}
    const edit=s.modal==='subscription';
    asyncOperation('import',()=>{
      const name=(d.name||'').trim()||new URL(d.url).hostname;
      if(edit){s.nodes=s.nodes.map(n=>n.sub===s.selectedSub?{...n,sub:name}:n);}
      else{
        const duplicates=s.nodes.some(n=>n.sub===name);
        if(duplicates){s.errors={name:'这个订阅名称已存在，请换一个名称'};return;}
        const additions=seedNodes().slice(0,4).map((n,i)=>({...n,id:'import-'+Date.now()+'-'+i,sub:name}));
        s.nodes.push(...additions);if(!s.nodes.some(n=>n.id===s.selected)){s.selected=additions[0].id;s.pendingNode=s.selected;}
      }
      s.modal=null;s.dirty=false;s.page='nodes';s.resourceTab='subscription';s.scene=s.mode==='full'?'full-nodes':'import-success';s.subUpdated=true;
      notify(edit?'订阅已保存':'订阅已导入，选择节点后即可连接');
    },()=>{s.errors={form:'无法获取订阅，请检查链接和网络后重试。已保留输入内容。'};});
  }
  function saveListener(){
    captureDraft();const d=s.draft,errors={};
    if(!d.name?.trim())errors.name='请输入监听器名称';
    if(!portValid(d.port))errors.port='端口必须在 1–65535 之间';
    if(s.listeners.some((l,i)=>l.port===Number(d.port)&&i!==d.editIndex))errors.port='这个端口已被其他监听器使用';
    if(!d.exit)errors.form='请选择出口';
    if(d.ruleTargetId&&d.type==='HTTP')errors.form='进程分流需要混合或 SOCKS5 监听器';
    if(Object.keys(errors).length){invalid(errors);return;}
    asyncOperation('save',()=>{
      const entry={id:d.editIndex!==undefined?s.listeners[d.editIndex].id:'l-'+Date.now(),name:d.name.trim(),port:Number(d.port),type:d.type,exit:d.exit,enabled:true};
      if(d.editIndex!==undefined)s.listeners[d.editIndex]=entry;else s.listeners.push(entry);
      if(d.ruleTargetId){const app=s.apps.find(a=>a.id===d.ruleTargetId);if(app)app.listenerId=entry.id;}
      s.modal=null;s.dirty=false;notify(d.ruleTargetId?'监听器已创建，并应用到该进程':'监听器已保存并生效');
    },()=>{s.errors={form:'配置未能生效，原有监听器仍在运行。请检查端口后重试。'};});
  }
  function saveExit(){
    captureDraft();const d=s.draft,errors={};
    if(!d.name?.trim())errors.name='请输入出口名称';
    if(s.exits.some((e,i)=>e.name===d.name.trim()&&i!==d.editIndex))errors.name='出口名称已存在';
    if(d['exit-type']==='Group'&&!d.members?.length)errors.form='请至少选择一个成员节点';
    if(Object.keys(errors).length){invalid(errors);return;}
    asyncOperation('save',()=>{
      const types={Direct:'直连',Node:'节点',Group:{UrlTest:'自动测速',Fallback:'故障转移',LoadBalance:'负载均衡',Select:'手动选择'}[d.strategy]};
      const selected=s.nodes.find(n=>n.id===(d['exit-type']==='Group'?d.members[0]:d.node));
      const entry={name:d.name.trim(),type:types[d['exit-type']],node:d['exit-type']==='Direct'?'系统默认网卡':selected?.name||'未选择'};
      if(d.editIndex!==undefined){const old=s.exits[d.editIndex].name;s.exits[d.editIndex]=entry;s.listeners.forEach(l=>{if(l.exit===old)l.exit=entry.name;});}else s.exits.push(entry);
      s.modal=null;s.dirty=false;notify('出口已保存并生效');
    },()=>{s.errors={form:'出口无法应用，请检查成员和网卡设置。原有出口已保留。'};});
  }
  function deleteItem(kind,index){
    s.dirty=false;s.errors={};s.modal='delete';
    if(kind==='sub'){
      const name=[...new Set(s.nodes.map(n=>n.sub))][index];
      const used=s.nodes.find(n=>n.id===s.selected)?.sub===name;
      s.draft={kind,index,name,used,description:`删除「${name}」及其中的节点。${used?'当前代理正在使用此订阅。':'其他订阅不受影响。'}`,alternatives:s.nodes.filter(n=>n.sub!==name&&n.delay!==-1).map(n=>[n.id,n.name])};
    }else if(kind==='exit'){
      const name=s.exits[index].name;
      s.draft={kind,index,name,used:s.listeners.some(l=>l.exit===name),description:`删除出口「${name}」。`,alternatives:s.exits.filter((_,i)=>i!==index).map(e=>[e.name,e.name])};
    }else{
      const l=s.listeners[index];
      s.draft={kind,index,name:l.name,used:s.apps.some(a=>(a.listenerId||'default')===l.id),description:`删除「${l.name}」并释放端口 ${l.port}。`,alternatives:s.listeners.filter((e,i)=>i!==index&&usableForProcess(e)&&e.enabled).map(e=>[e.id,e.name])};
    }
    render();
  }
  function confirmDelete(){
    const d=s.draft;
    const replacement=$('[name=replacement]')?.value;
    if(d.used&&!replacement)return;
    if(d.kind==='sub'){
      s.nodes=s.nodes.filter(n=>n.sub!==d.name);
      if(d.used)s.selected=replacement;
      if(!s.nodes.length){s.selected=null;s.connected=false;}
    }else if(d.kind==='exit'){
      s.exits.splice(d.index,1);s.listeners.forEach(l=>{if(l.exit===d.name)l.exit=replacement;});
    }else{
      if(s.listeners.length===1){notify('请保留一个默认监听器；可以先停用它');return;}
      const removed=s.listeners[d.index];s.apps.forEach(a=>{if((a.listenerId||'default')===removed.id)a.listenerId=replacement;});s.listeners.splice(d.index,1);
    }
    s.modal=null;notify('已删除，并更新引用关系');render();
  }
  function handle(action){
    const [name,value]=action.split(':');
    if(s.busy&&!['close-modal','page','full'].includes(name))return;
    if(name==='page'){
      if(s.mode==='full')go('full-'+(value==='home'?'overview':value));
      else go(value==='home'?(s.connected?'simple-connected':'simple-home'):value);
      return;
    }
    if(name==='full'){s.mode='full';go('full-'+value);return;}
    if(name==='modal'){openModal(value);return;}
    if(name==='welcome-mode'){s.welcomeMode=value;render();return;}
    if(name==='start'){s.mode=s.welcomeMode;s.modal=null;s.page=s.mode==='simple'?'home':'overview';s.scene=s.mode==='simple'?'simple-empty':'full-overview';if(!s.nodes.length){s.selected=null;s.exits=[{name:'直连',type:'直连',node:'系统默认网卡'}];s.listeners[0].exit='直连';}history.replaceState(null,'','#'+s.scene);render();return;}
    if(name==='close-modal'){closeModal();return;}
    if(name==='continue-edit'){s.modal=s.originalModal;render();return;}
    if(name==='discard-edit'){s.dirty=false;s.modal=null;render();return;}
    if(name==='scope'){chooseScope(value);return;}
    if(name==='connect'){
      if(s.connected){asyncOperation('connect',()=>{s.connected=false;if(s.scope==='apps')s.fullEnabled=false;s.error=null;notify('代理已关闭');});return;}
      if(!s.nodes.length){openModal('import');return;}
      if(!node()){openModal('picker');return;}
      if(s.scope==='apps'&&!s.apps.some(a=>a.on)){notify('请先选择至少一个应用');return;}
      asyncOperation('connect',()=>{syncDefaultExit();s.connected=true;s.coreRunning=true;s.error=null;if(s.scope==='apps')s.fullEnabled=true;s.scene=s.scope==='apps'?'simple-apps':'simple-connected';notify('代理已开启');},reason=>{s.error=reason==='port'?'port':reason==='permission'?'permission':'connect';});return;
    }
    if(name==='retry-listener'){changeRuleListener(value,s.ruleFailedTargets[value]);return;}
    if(name==='pending-node'){s.pendingNode=value;render();return;}
    if(name==='apply-node'){
      asyncOperation('node',()=>{s.selected=s.pendingNode;syncDefaultExit();s.error=null;s.modal=null;notify('已切换到 '+node()?.name);},()=>{s.error='node';});return;
    }
    if(name==='use-node'){s.pendingNode=value;s.modal='picker';render();return;}
    if(name==='test-nodes'){
      s.testing=true;render();setTimeout(()=>{s.testing=false;s.nodes.forEach(n=>{if(n.delay===null)n.delay=156;});render();notify('测速完成，超时节点已标记');},700);return;
    }
    if(name==='app'){
      const app=s.apps.find(a=>a.id===value);if(!app)return;
      asyncOperation('rule',()=>{app.on=!app.on;notify(app.name+(app.on?'已选择走代理':'已恢复直连'));},()=>{notify('规则未能应用，已保留原设置');});return;
    }
    if(name==='add-apps'){
      if(!s.appSelection.length)return;
      s.appSelection.forEach(id=>{const a=s.apps.find(a=>a.id===id);if(a)a.on=true;});s.modal=null;notify('已添加所选应用');render();return;
    }
    if(name==='apps-running'){return;}
    if(name==='apps-file'){s.modal='file';s.draft={};render();return;}
    if(name==='add-file'){
      captureDraft();const path=s.draft.path||'';
      if(!/^[A-Za-z]:\\.*\.exe$/i.test(path)){invalid({path:'请输入一个 Windows .exe 程序路径'});return;}
      const exists=s.apps.find(a=>a.path.toLowerCase()===path.toLowerCase());
      if(exists){invalid({path:'这个程序已在应用列表中'});return;}
      const exe=path.split('\\').pop();s.apps.push({id:'a-'+Date.now(),name:exe,exe,path,icon:'AppsOutlined',on:true});s.modal=null;s.dirty=false;notify('已添加 '+exe);render();return;
    }
    if(name==='install-component'){
      asyncOperation('install',()=>{if(s.permissionKind==='tun'){s.tunReady=true;s.method='tun';}else{s.component=true;s.scope='apps';}s.modal=null;s.error=null;notify(s.permissionKind==='tun'?'TUN 已就绪':'分流组件已就绪');},()=>{s.modal=null;s.error='permission';s.scope='system';notify('未获得权限，原连接方式已保留');});return;
    }
    if(name==='save-subscription'){saveSubscription();return;}
    if(name==='refresh-sub'){
      asyncOperation('refresh',()=>{s.subError=false;s.subUpdated=true;notify('订阅已更新');},()=>{s.subError=true;notify('更新失败，原有节点仍可使用');});return;
    }
    if(name==='edit-sub'){
      s.selectedSub=[...new Set(s.nodes.map(n=>n.sub))][Number(value)];openModal('subscription');return;
    }
    if(name==='delete-sub'){deleteItem('sub',Number(value));return;}
    if(name==='delete-exit'){deleteItem('exit',Number(value));return;}
    if(name==='delete-listener'){deleteItem('listener',Number(value));return;}
    if(name==='confirm-delete'){confirmDelete();return;}
    if(name==='save-listener'){saveListener();return;}
    if(name==='edit-listener'){openModal('listener');s.draft={...s.listeners[Number(value)],editIndex:Number(value),listen:'127.0.0.1'};render();return;}
    if(name==='listener-toggle'){s.listeners[Number(value)].enabled=!s.listeners[Number(value)].enabled;render();notify('监听器状态已更新');return;}
    if(name==='copy-listener'){notify('演示地址：127.0.0.1:'+s.listeners[Number(value)].port);return;}
    if(name==='listener-system'){s.connected=!s.connected;render();notify(s.connected?'系统代理已开启':'系统代理已关闭');return;}
    if(name==='save-exit'){saveExit();return;}
    if(name==='edit-exit'){
      openModal('exit');const e=s.exits[Number(value)];s.draft={name:e.name,'exit-type':e.type==='直连'?'Direct':e.type==='节点'?'Node':'Group',node:s.nodes.find(n=>n.name===e.node)?.id||'hk1',members:['hk1','jp1'],strategy:'UrlTest',editIndex:Number(value)};render();return;
    }
    if(name==='test-exit'){notify('测速完成：42 ms（示例）');return;}
    if(name==='save-rule'){
      captureDraft();const a=s.apps.find(a=>a.id===s.draft['rule-app']);if(!a){invalid({form:'请选择应用'});return;}
      const listener=s.listeners.find(l=>l.id===s.draft.listener);if(!usableForProcess(listener)){invalid({form:'请选择支持进程分流的混合或 SOCKS5 监听器'});return;}
      asyncOperation('rule',()=>{a.listenerId=listener.id;if(s.draft.editIndex===undefined)a.on=true;s.modal=null;s.dirty=false;notify('分流规则已保存并生效');},()=>{s.errors={form:'规则未能生效，原设置已保留'};});return;
    }
    if(name==='edit-rule'){openModal('rule');s.draft={'rule-app':s.apps[Number(value)].id,listener:s.apps[Number(value)].listenerId||'default',fault:s.fault,editIndex:Number(value)};render();return;}
    if(name==='full-enabled'){if(!s.component){openModal('permission');return;}s.fullEnabled=!s.fullEnabled;render();notify('进程分流状态已更新');return;}
    if(name==='save-manual'){
      captureDraft();const d=s.draft,errors={};if(!d.name?.trim())errors.name='请输入名称';if(!d.server?.trim()&&!d.yaml)errors.server='请输入服务器地址';if(!portValid(d.port))errors.port='请输入有效端口';if(Object.keys(errors).length){invalid(errors);return;}
      s.nodes.push({id:'manual-'+Date.now(),name:d.name.trim(),sub:'手动节点',type:d.protocol.toUpperCase(),delay:null});if(!node())s.selected=s.nodes.at(-1).id;s.modal=null;s.dirty=false;notify('手动节点已添加');render();return;
    }
    if(name==='save-port'){
      captureDraft();if(!portValid(s.draft.port)){invalid({port:'端口必须在 1–65535 之间'});return;}
      if(s.listeners.slice(1).some(l=>l.port===Number(s.draft.port))){invalid({port:'端口已被其他监听器使用'});return;}
      s.listeners[0].port=Number(s.draft.port);s.error=null;s.modal=null;s.dirty=false;notify('端口已更新，可以重新开启代理');render();return;
    }
    if(name==='resource-tab'){s.resourceTab=value;render();return;}
    if(name==='overview-tab'){s.overviewTab=value;render();return;}
    if(name==='autostart'||name==='autoConnect'){s[name]=!s[name];render();notify('设置已保存');return;}
    if(name==='save-settings'){
      const form=$('#settings-form');const d=Object.fromEntries(new FormData(form));
      if(!portValid(d.controller)){notify('控制端口必须在 1–65535 之间');return;}
      if(s.listeners.some(l=>l.port===Number(d.controller))){notify('控制端口与监听器端口冲突');return;}
      notify('设置已保存并应用（示例）');return;
    }
    if(name==='tailscale'){s.resourceTab='tailscale';go('full-nodes');return;}
    if(name==='save-ts'){captureDraft();if(!s.draft.name?.trim()){invalid({name:'请输入实例名称'});return;}s.modal=null;s.dirty=false;s.resourceTab='tailscale';render();notify('实例已保存，请登录');return;}
    if(name==='ts-login'){if(s.tsLogged){openModal('logs');return;}s.tsLogged=true;render();notify('演示：Tailscale 登录完成，未打开真实登录页');return;}
    if(name==='diagnose'){s.error=null;render();notify('检查完成（示例）');return;}
    if(name==='core-restart'){asyncOperation('restart',()=>{s.coreRunning=true;notify('内核重启完成（示例）');});return;}
    if(name==='core-stop'){s.connected=false;s.coreRunning=false;render();notify('内核已停止，系统代理已释放（示例）');return;}
    if(name==='switch-simple'){s.mode='simple';go(s.connected?'simple-connected':'simple-home');return;}
    if(name==='loaded'){s.page='home';s.scene='simple-home';render();return;}
  }

  $('#scene-nav').innerHTML=sceneGroups.map(([title,items])=>`<section class="review-group"><h2>${title}</h2>${items.map(([id,label])=>`<button data-scene="${id}">${label}</button>`).join('')}</section>`).join('');
  document.addEventListener('click',event=>{
    const scene=event.target.closest('[data-scene]');
    if(scene){operation++;go(scene.dataset.scene,true);return;}
    const b=event.target.closest('[data-action]');if(b&&!b.disabled){if(b.dataset.action.startsWith('modal:')||b.dataset.action.startsWith('edit-'))s.focusRestore=b.dataset.action;handle(b.dataset.action);}
  });
  document.addEventListener('input',event=>{
    const target=event.target;
    if(target.closest('#active-form')){s.dirty=true;captureDraft();if(target.name&&s.errors[target.name]){delete s.errors[target.name];target.removeAttribute('aria-invalid');target.closest('.field')?.querySelector('.field-error')?.remove();}}
    if(target.dataset.filter){s.searches[target.dataset.filter]=target.value;applyFilter(target);}
  });
  document.addEventListener('change',event=>{
    const t=event.target;
    if(t.dataset.ruleListener){changeRuleListener(t.dataset.ruleListener,t.value);return;}
    if(t.hasAttribute('data-app-select')){
      s.appSelection=[...document.querySelectorAll('[data-app-select]:checked')].map(c=>c.value);
      $('#app-selection-count').textContent=`已选择 ${s.appSelection.length} 项`;
      $('[data-action="add-apps"]').disabled=!s.appSelection.length;
    }
    if(t.dataset.formSelect){captureDraft();render();}
    if(t.dataset.change==='mode'){
      s.mode=t.value;s.modal=null;s.page=s.mode==='full'?'overview':'home';s.scene=s.mode==='full'?'full-overview':s.connected?'simple-connected':'simple-home';render();notify('已切换界面，现有连接和配置已保留');
    }
    if(t.dataset.change==='method'){
      if(t.value==='tun'&&!s.tunReady){s.permissionKind='tun';s.modal='permission';render();}
      else{s.method=t.value;render();notify('连接方式已更新（模拟）');}
    }
    if(t.dataset.change==='fault'){s.fault=t.value;notify('故障策略已保存');}
    if(t.dataset.listenerExit!==undefined){s.listeners[Number(t.dataset.listenerExit)].exit=t.value;notify('出口已应用');}
  });
  document.addEventListener('submit',event=>{event.preventDefault();if(s.modal==='import'||s.modal==='subscription')saveSubscription();});
  document.addEventListener('keydown',event=>{
    if(event.key==='Escape'&&s.modal){event.preventDefault();closeModal();}
    if(event.key==='Tab'&&s.modal){
      const dialog=$('.dialog');const focusable=[...dialog.querySelectorAll('button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),[tabindex="0"]')].filter(el=>el.offsetParent!==null);
      const first=focusable[0],last=focusable.at(-1);
      if(event.shiftKey&&(document.activeElement===first||document.activeElement===dialog)){event.preventDefault();last?.focus();}
      else if(!event.shiftKey&&(document.activeElement===last||document.activeElement===dialog)){event.preventDefault();first?.focus();}
    }
  });
  $('#simulation').addEventListener('change',event=>{s.simulation=event.target.value;});
  $('#reset-prototype').addEventListener('click',()=>{operation++;s=defaults();$('#simulation').value='success';go('welcome',true);});
  window.addEventListener('hashchange',()=>{const scene=location.hash.slice(1);if(sceneInfo[scene])go(scene,true);});
  go(sceneInfo[location.hash.slice(1)]?location.hash.slice(1):'welcome',true);
})();
