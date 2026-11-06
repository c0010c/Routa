from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
source = (HERE.parent / 'node-compact-v1' / 'index.html').read_text(encoding='utf-8')

def replace_once(old, new):
    global source
    if old not in source:
        raise RuntimeError(f'Missing design insertion point: {old[:100]}')
    source = source.replace(old, new, 1)

source = source.replace('紧凑节点设计预览 V1', '88px 两行节点设计预览 V2')
source = source.replace('节点卡片 V1', '88 px 两行节点 V2')
source = source.replace('延迟与状态</button>', '动效与状态</button>')
source = source.replace('--fair:#5f7a18', '--fair:#0f7b0f')
replace_once("n.ms<=150?'green':n.ms<=300?'fair':n.ms<=600?'amber':'red'", "n.ms<=300?'green':n.ms<=600?'amber':'red'")
replace_once('<span class="green"><i class="dot"></i>≤150 ms</span><span class="fair"><i class="dot"></i>151–300 ms</span>', '<span class="green"><i class="dot"></i>≤300 ms</span>')

comparison = '''<section class="view comparison" id="comparison">
<div class="comparison-top"><div><div class="eyebrow">ROUTA / NODE CARDS / V2</div><h1>88 px，两行就够了</h1><div class="subtitle">名称与延迟并排，协议、来源和操作收在底行。文字保持可读，列表更加紧凑。</div></div><span class="proposal-tag">设计预览 · 示例数据</span></div>
<div class="compare-grid three-way">
<div class="compare-panel"><div class="compare-label">当前实现<span class="dimension">276 × 144 px</span></div><div class="compare-scene"><div id="before-card" class="compare-node"></div><div class="measure old"><b>144</b><span>px 高</span><span>三层信息</span></div></div><div class="compare-notes">名称、协议来源和延迟纵向排列。</div></div>
<div class="compare-panel"><div class="compare-label">上一版 V1<span class="dimension">276 × 96 px</span></div><div class="compare-scene"><div id="middle-card" class="compare-node"></div><div class="measure middle"><b>96</b><span>px 高</span><span>收紧间距</span></div></div><div class="compare-notes">保留三层排列，卡片高度减少 33%。</div></div>
<div class="compare-panel proposed"><div class="compare-label">两行方案 V2<span class="dimension">276 × 88 px</span></div><div class="compare-scene"><div id="after-card" class="compare-node"></div><div class="measure new"><b>88</b><span>px 高</span><span>减少 39%</span></div></div><div class="compare-notes">延迟右对齐，短名称与长名称保持同高。</div></div>
</div>
<div class="comparison-bottom"><div class="detail-panel"><h2 class="section-title">名称优先，延迟一眼可比</h2><div class="anatomy"><div id="anatomy-card" class="compare-node"></div><div class="anatomy-notes"><b>名称 14 px / 18 px</b> · 最多两行<br><b>延迟 18 px</b> · 固定靠右<br><b>协议、来源 12 px</b> · 底行对齐<br>完整名称与测量详情悬停查看</div></div><div class="state-samples" id="state-samples"></div><div class="small" style="margin-top:12px">点击下方两张卡片可体验选择过渡；红色延迟始终保留。</div></div>
<div class="detail-panel"><h2 class="section-title">更紧凑，也更安静</h2><div class="density-row"><span class="density-label">卡片高度</span><div class="height-track"><span class="height-current">144 px</span><span class="height-v1">96 px</span><span class="height-v2">88 px</span></div></div><div class="density-highlight"><b>−39%</b><span>相对当前卡片高度 · 内边距保持 12 px</span></div><div class="motion-note">悬停 120 ms：背景与边框变化，卡片保持原位。<br>选择 120 ms：蓝色描边与勾选，不改变延迟颜色。<br>测量结束 150 ms：结果淡入；排序 180 ms：平移归位。</div><div class="small" style="margin-top:13px">动效可在完整节点页和切换弹窗中体验。</div></div></div>
<div class="comparison-bottomline"><span>同宽 276 px 对照 · 卡片间距 12 px · 长名称两行省略</span><span>蓝色表达选择 · 绿 / 橙 / 红表达延迟状态</span></div>
</section>'''
source = re.sub(r'<section class="view comparison" id="comparison">.*?</section>', lambda _: comparison, source, count=1, flags=re.S)

css = '''
/* V2 standalone design: two rows, stable targets and shared status rules. */
body.capture .viewer{display:none}body.capture .stage{margin:0}
.node-card{height:88px;border-radius:7px;box-shadow:none}
.card-core{padding:11px;display:grid;grid-template-columns:minmax(0,1fr)74px;grid-template-rows:36px 20px;gap:8px;align-content:start}
.node-card .node-title{grid-column:1;grid-row:1;padding:0;height:36px;line-height:18px;font-size:14px;text-align:left}
.node-delay{grid-column:2;grid-row:1;display:flex;flex-direction:column;align-items:flex-end;gap:1px;min-width:0;align-self:start}
.node-delay .delay{display:inline-flex;justify-content:flex-end;line-height:21px;min-height:21px;gap:4px;font-size:18px;min-width:74px;text-align:right}
.node-delay .delay .unit{line-height:1}
.node-delay .delay.failure,.node-delay .delay.unset,.node-delay .delay.pending{font-size:12px;line-height:21px}
.node-delay .delay.pending{gap:5px}
.node-card .node-meta{grid-column:1/3;grid-row:2;margin:0;height:20px;gap:7px;padding-right:27px;align-self:center}
.node-card .node-meta .pill{height:18px;line-height:16px;font-size:12px;padding:0 5px}
.node-card .source{font-size:12px;line-height:18px}
.node-card .card-more{top:auto;bottom:6px;right:6px;width:26px;height:26px}
.selection-mark{height:14px;font-size:11px;line-height:14px;gap:4px;white-space:nowrap}
.selection-mark:empty{visibility:hidden}.node-card.current .selection-mark{color:#616161}.node-card.selected .selection-mark{color:var(--blue)}
.node-card:hover,.node-card.hover-demo{border-color:#b5b5b5;background:#fafafa;box-shadow:none}
.node-card.selected,.node-card.selected:hover{border-color:var(--blue);background:#f1f7fd;box-shadow:inset 0 0 0 1px var(--blue)}
.node-card .card-core:active{background:#00000004}.node-card:focus-within{outline:none}.node-card:has(:focus-visible){outline:2px solid var(--blue);outline-offset:2px}
.check{width:13px;height:13px;font-size:9px;font-style:normal}.current-dot{display:inline-block;flex-shrink:0}
.compare-grid.three-way{grid-template-columns:repeat(3,minmax(0,1fr));gap:18px;margin-top:26px}
.three-way .compare-panel{height:290px;padding:20px}.three-way .compare-panel.proposed{padding-top:18px}
.three-way .compare-label{font-size:14px;gap:8px}.three-way .dimension{font-size:11px}
.three-way .compare-scene{gap:13px;margin-top:22px}.three-way .compare-node{width:276px}
.three-way .measure{padding-left:12px;flex:1;gap:5px;white-space:nowrap}.measure.middle{height:96px}.measure.new{height:88px}
.three-way .compare-notes{font-size:12px;line-height:1.8;margin-top:15px}
.medium{height:96px;position:relative;padding:9px 11px;border:1px solid #ddd;border-radius:7px;background:#fff}
.medium .node-title{height:34px;line-height:17px;padding-right:26px}.medium .node-meta{height:16px;margin-top:2px}.medium .node-meta .pill{height:16px;line-height:14px}
.medium .node-bottom{margin-top:4px;height:20px}.medium .card-more{top:5px;right:5px;bottom:auto}
.comparison-bottom{margin-top:24px;grid-template-columns:1.08fr 1fr}.detail-panel{height:332px}.anatomy-notes{line-height:1.9;font-size:12px}
.state-samples{margin-top:22px}.height-track{display:flex;gap:7px;align-items:center;flex:1}.height-track span{height:28px;display:flex;align-items:center;justify-content:center;font-size:12px;border-radius:4px;white-space:nowrap}
.height-current{width:37%;background:#e4e7eb;color:#5c626a}.height-v1{width:25%;background:#dae6ef;color:#506b80}.height-v2{width:23%;background:#d3e9fa;color:var(--blue)}
.density-highlight{margin-top:24px}.motion-note{margin-top:18px;padding-top:15px;line-height:2}
.dialog-body{padding-top:16px;padding-bottom:16px}.dialog{transform-origin:center}.modal-overlay.opening{animation:overlay-in 160ms ease-out}.modal-overlay.opening .dialog{animation:dialog-in 160ms cubic-bezier(.2,.8,.2,1)}
@keyframes overlay-in{from{background:#0000}to{background:#0005}}@keyframes dialog-in{from{opacity:0;transform:translateY(6px)}to{opacity:1;transform:translateY(0)}}
.spinner{height:10px;width:10px;border:1.5px solid #c7c7c7;border-top-color:#616161;border-radius:50%;animation:spin 700ms linear infinite;flex-shrink:0}
@keyframes spin{to{transform:rotate(360deg)}}
.status-board .subtitle{margin-top:12px;color:#656b74;font-size:14px}.status-board .node-title{font-size:14px}.status-content{margin-top:28px}
.motion-demo{display:flex;gap:10px;margin-top:15px;align-items:center}.motion-demo>span{font-size:12px;color:#777}.proxy-delay{display:inline-block;min-width:80px}.dialog-body .empty{padding-top:140px}
@media(prefers-reduced-motion:reduce){*{animation:none!important;transition:none!important}}
'''
replace_once('</style>', css + '\n</style>')

card_function = '''function card(n,i,options={}){
 const selected=!!options.selected,current=!!options.current;
 const selectAttr=options.selectable?`role="radio" aria-checked="${selected}" ${options.demo?'data-demo-select':'data-select'}="${i}"`:'';
 const mark=selected?`<i class="check">✓</i>${current?'当前使用':'待切换'}`:current?'<i class="current-dot"></i>当前使用':n.ts?'已连接':'';
 return `<article class="node-card ${options.selectable?'selectable':''} ${selected?'selected':''} ${current?'current':''} ${options.hover?'hover-demo':''}" data-index="${i}"><button class="card-core" ${selectAttr} aria-label="${esc(n.name)}" data-tip="${esc(tip(n))}"><span class="node-title">${esc(n.name)}</span><span class="node-delay"><span class="delay num ${color(n)} ${n.error?'failure':n.ms==null?'unset':''}" data-tip="${esc(tip(n))}">${delayMarkup(n)}</span><span class="selection-mark">${mark}</span></span><span class="node-meta"><span class="pill">${esc(n.protocol)}</span><span class="source" data-tip="${esc(n.source)}">${esc(n.source)}</span></span></button><button class="icon card-more" aria-label="更多操作 ${esc(n.name)}" data-more="${i}">⋯</button></article>`;
}
function delayMarkup(n){return n.error||(n.ms==null?'未测试':`${n.ms}<span class="unit">ms</span>`)}'''
source = re.sub(r'^function card\(.*?\n(?=function shell)', lambda _: card_function+'\n', source, count=1, flags=re.M|re.S)

replace_once('let chosen=0;', 'let currentNode=1,chosen=1;')
source = source.replace('current:i===1', 'current:i===currentNode')
source = source.replace('chosen===1', 'chosen===currentNode')
replace_once("document.getElementById('after-card').innerHTML=card(data[1],1);", '''document.getElementById('middle-card').innerHTML=`<article class="medium"><span class="node-title">新加坡 · 狮城 01</span><button class="icon card-more" aria-label="更多操作">⋯</button><span class="node-meta"><span class="pill">VLESS</span><span class="source">示例订阅 A</span></span><span class="node-bottom"><span class="delay amber num">487<span class="unit">ms</span></span></span></article>`;
document.getElementById('after-card').innerHTML=card(data[1],1);''')
replace_once("card(data[0],0,{hover:true})+card(data[1],1,{selected:true,selectable:true})", "card(data[0],0,{selectable:true,demo:true})+card(data[2],2,{selected:true,selectable:true,demo:true})")
replace_once("if(select){chosen=Number(select.dataset.select);renderDialog();return}", "if(select){chooseNode(Number(select.dataset.select));return}")
replace_once("if(e.target.closest('#confirm')){feedback('设计演示：已选择 '+data[chosen].name+'；代理延迟将标记为待重测');return}", "if(e.target.closest('#confirm')){confirmDemo();return}")

old_measure = "if(e.target.closest('[data-test]')||e.target.closest('[data-action=\"test-one\"]')){const btn=e.target.closest('button');const old=btn.textContent;btn.textContent='测量中…';btn.disabled=true;setTimeout(()=>{btn.textContent=old;btn.disabled=false;feedback('设计演示：延迟状态已更新，示例数据保持固定');},850)}"
replace_once(old_measure, "if(e.target.closest('[data-test]')||e.target.closest('[data-action=\"test-one\"]')){runMeasurement(e.target.closest('button'));}")
old_sort = "if(e.target.closest('#sort')){document.querySelectorAll('#node-groups .cards').forEach(grid=>[...grid.children].sort((a,b)=>(data[+a.dataset.index].ms??Infinity)-(data[+b.dataset.index].ms??Infinity)).forEach(el=>grid.append(el)));feedback('已按节点延迟排序')}"
replace_once(old_sort, "if(e.target.closest('#sort')){sortEnabled=!sortEnabled;document.getElementById('sort').textContent=sortEnabled?'⇅ 恢复默认顺序':'⇅ 延迟排序';sortNodes();}")
replace_once("if(view==='dialog')document.querySelector('.modal-overlay').hidden=false;", "if(view==='dialog'){const overlay=document.querySelector('.modal-overlay');overlay.hidden=false;overlay.classList.remove('opening');void overlay.offsetWidth;overlay.classList.add('opening');}")

old_key_start = "document.addEventListener('keydown',e=>{if(e.key==='Escape')"
key_pos = source.index(old_key_start)
key_end = source.index("\nconst initial=", key_pos)
source = source[:key_pos] + '''document.addEventListener('keydown',e=>{
 if(e.key==='Escape'){hideTip();document.getElementById('menu').hidden=true;const overlay=document.querySelector('.modal-overlay');if(!document.getElementById('dialog').hidden&&!overlay.hidden)overlay.hidden=true;}
 const trigger=e.target.closest('#dialog-cards [data-select]');
 if(trigger&&['ArrowRight','ArrowLeft','ArrowDown','ArrowUp','Home','End'].includes(e.key)){
  e.preventDefault();const options=[...document.querySelectorAll('#dialog-cards [data-select]')],at=options.indexOf(trigger),step={ArrowRight:1,ArrowLeft:-1,ArrowDown:3,ArrowUp:-3}[e.key]||0;
  const next=e.key==='Home'?0:e.key==='End'?options.length-1:Math.max(0,Math.min(options.length-1,at+step));
  chooseNode(+options[next].dataset.select);options[next].focus();
 }
});
''' + source[key_end:]

extra_script = '''
if(new URLSearchParams(location.search).has('capture'))document.body.classList.add('capture');
let sortEnabled=false,measuring=false;
const reduced=()=>matchMedia('(prefers-reduced-motion: reduce)').matches;
function chooseNode(index){
 chosen=index;
 document.querySelectorAll('#dialog-cards .node-card').forEach(el=>{
  const i=+el.dataset.index,selected=i===chosen,current=i===currentNode;
  el.classList.toggle('selected',selected);el.classList.toggle('current',current);
  el.querySelector('[role="radio"]').setAttribute('aria-checked',String(selected));
  el.querySelector('.selection-mark').innerHTML=selected?`<i class="check">✓</i>${current?'当前使用':'待切换'}`:current?'<i class="current-dot"></i>当前使用':'';
 });
 document.getElementById('confirm').disabled=chosen===currentNode;
}
function confirmDemo(){
 currentNode=chosen;renderDialog();
 document.querySelectorAll('.proxy dd.node>span').forEach(el=>el.textContent=data[currentNode].name);
 document.querySelectorAll('.proxy-delay').forEach(el=>{el.textContent='待重测 ⓘ';el.className='proxy-delay neutral';el.dataset.tip='节点已切换，等待重新测量当前代理延迟。\\n旧结果：170 ms · 今天 14:18:03\\n旧节点：新加坡 · 狮城 01';});
 document.querySelector('.modal-overlay').hidden=true;feedback('设计演示：已切换为 '+data[currentNode].name);
}
function sortNodes(){
 document.querySelectorAll('#node-groups .cards').forEach(grid=>{
  const cards=[...grid.children],old=new Map(cards.map(el=>[el,el.getBoundingClientRect()]));
  cards.sort((a,b)=>sortEnabled?((data[+a.dataset.index].ms??Infinity)-(data[+b.dataset.index].ms??Infinity)||+a.dataset.index-+b.dataset.index):+a.dataset.index-+b.dataset.index).forEach(el=>grid.append(el));
  if(!reduced())cards.forEach(el=>{const from=old.get(el),to=el.getBoundingClientRect();if(from.x!==to.x||from.y!==to.y)el.animate([{transform:`translate(${from.x-to.x}px,${from.y-to.y}px)`},{transform:'translate(0,0)'}],{duration:180,easing:'cubic-bezier(.2,.8,.2,1)'});});
 });
}
async function runMeasurement(button){
 if(measuring)return;measuring=true;
 if(button.closest('.proxy')){
  const previous=button.textContent;button.textContent='测量中…';button.disabled=true;
  document.querySelectorAll('.proxy-delay').forEach(el=>{el.className='proxy-delay neutral';el.textContent='测量中…';});
  await new Promise(resolve=>setTimeout(resolve,1100));
  document.querySelectorAll('.proxy-delay').forEach(el=>{el.className='proxy-delay green';el.textContent='170 ms ⓘ';el.dataset.tip='当前代理延迟：170 ms\\n测试对象：工作代理（出口）\\n示例测量完成：刚刚\\n当前节点：'+data[currentNode].name+'\\n出口网卡：Ethernet';if(!reduced())el.animate([{opacity:.25},{opacity:1}],{duration:150,easing:'ease-out'});});
  button.textContent=previous;button.disabled=false;measuring=false;feedback('当前代理延迟已完成示例测量');return;
 }
 const view=[...document.querySelectorAll('.view')].find(el=>!el.hidden);
 const single=button.dataset.action==='test-one'?+document.getElementById('menu').dataset.index:null;
 const cards=[...view.querySelectorAll('.node-card')].filter(el=>single==null||+el.dataset.index===single);
 const oldButton=button.textContent;button.textContent='测量中…';button.disabled=true;
 cards.forEach(card=>{const el=card.querySelector('.delay');el.className='delay num neutral pending';el.innerHTML='<i class="spinner" aria-hidden="true"></i>测量中';card.setAttribute('aria-busy','true');});
 await new Promise(resolve=>setTimeout(resolve,1100));
 cards.forEach(card=>{const n=data[+card.dataset.index],el=card.querySelector('.delay');el.className=`delay num ${color(n)} ${n.error?'failure':n.ms==null?'unset':''}`;el.innerHTML=delayMarkup(n);card.removeAttribute('aria-busy');if(!reduced())el.animate([{opacity:.25},{opacity:1}],{duration:150,easing:'ease-out'});});
 button.textContent=oldButton;button.disabled=false;measuring=false;if(sortEnabled)sortNodes();feedback('示例测量完成');
}
document.addEventListener('click',e=>{
 const demo=e.target.closest('[data-demo-select]');if(demo){const host=demo.closest('.state-samples');host.querySelectorAll('.node-card').forEach(card=>{const active=card===demo.closest('.node-card');card.classList.toggle('selected',active);card.querySelector('[role="radio"]').setAttribute('aria-checked',String(active));card.querySelector('.selection-mark').innerHTML=active?'<i class="check">✓</i>已选择':'';});}
 const nav=e.target.closest('[data-nav]');if(nav?.dataset.nav==='节点')showView('nodes');if(nav?.dataset.nav==='代理'){showView('dialog');document.querySelector('.modal-overlay').hidden=true;}
});
'''
replace_once('\nconst initial=', '\n'+extra_script+'\nconst initial=')

# Four sample IDs on the state board must point to their actual demo records.
replace_once("[data[1],data[2],data[3],data[4],data[5],data[7]].map((n,i)=>card(n,i))", "[1,2,3,4,5,7].map(i=>card(data[i],i))")

(HERE / 'index.html').write_text(source, encoding='utf-8')
print('Created V2 preview:', HERE / 'index.html')
