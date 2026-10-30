from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
source = (HERE.parent / 'node-compact-v2' / 'index.html').read_text(encoding='utf-8')
source = source.replace('88px 两行节点设计预览 V2', '窄卡片与颜色选择预览 V3')
source = source.replace('88 px 两行节点 V2', '224 px 窄卡片 V3')

comparison = '''<section class="view comparison" id="comparison">
<div class="comparison-top"><div><div class="eyebrow">ROUTA / NODE CARDS / V3</div><h1>卡片更窄，选择更清楚</h1><div class="subtitle">宽度收至约 224 px，高度保持 88 px。选中用浅蓝底色标识，延迟继续保留自己的状态色。</div></div><span class="proposal-tag">设计预览 · 示例数据</span></div>
<div class="compare-grid narrow-compare">
<div class="compare-panel"><div class="compare-label">上一版 V2<span class="dimension">276 × 88 px</span></div><div class="compare-scene"><div id="before-card" class="compare-node previous-slot"></div><div class="measure new"><b>276</b><span>px 宽</span><span>节点页 4 列</span></div></div><div class="compare-notes">名称与延迟并排，底行展示协议、来源和更多操作。</div></div>
<div class="compare-panel proposed"><div class="compare-label">窄卡片 V3<span class="dimension">224 × 88 px</span></div><div class="compare-scene"><div id="after-card" class="compare-node narrow-slot"></div><div class="measure new"><b>224</b><span>px 宽 · 减少约 19%</span><span>节点页 5 列</span></div></div><div class="compare-notes">名称保持 14 px、最多两行；辅助信息保持 12 px。</div></div>
</div>
<div class="comparison-bottom narrow-bottom">
<div class="detail-panel"><h2 class="section-title">长名称与辅助信息</h2><div id="anatomy-card" class="narrow-slot"></div><div class="narrow-notes">名称最多两行，超出省略。<br>来源单行省略，悬停查看完整内容。<br>延迟靠右，更多操作固定在右下角。</div><div class="small" style="margin-top:16px">保留 12 px 内边距与 12 px 卡片间距。</div></div>
<div class="detail-panel"><h2 class="section-title">浅蓝底色，清楚标识选择</h2><div class="state-samples" id="state-samples"></div><div class="sample-labels"><span>默认</span><span>悬停</span><span>选中</span></div><div class="selection-notes"><span class="selection-swatch"></span><span>浅蓝填充 + 蓝色侧标 + 小勾选</span></div><div class="motion-note">点击上方卡片体验选择：底色在 120 ms 内过渡，卡片保持原位。<br>选中高延迟节点时，红色延迟与蓝色选择状态同时保留。</div><div class="small" style="margin-top:13px">切换弹窗使用四列窄卡片，标题和底部操作继续固定。</div></div>
</div>
<div class="comparison-bottomline"><span>对照卡片为 276 / 224 px 实际宽度 · 高度均为 88 px</span><span>浅蓝色表达选择 · 绿 / 橙 / 红表达延迟</span></div>
</section>'''
source = re.sub(r'<section class="view comparison" id="comparison">.*?</section>', lambda _: comparison, source, count=1, flags=re.S)

css = '''
/* V3: narrower cards and a filled selection state. */
.cards{grid-template-columns:repeat(5,minmax(0,1fr))}
.card-core{grid-template-columns:minmax(0,1fr)64px;column-gap:6px}
.node-delay .delay{min-width:64px}
.node-card.selected,.node-card.selected.hover-demo,.node-card.selected:hover{background:#e5f1fc;border-color:#b0cde5;box-shadow:none}
.node-card.selected:before{content:"";position:absolute;left:0;top:10px;bottom:10px;width:3px;background:var(--blue);border-radius:0 2px 2px 0;pointer-events:none}
.node-card.hover-demo,.node-card:hover{background:#f7f9fb;border-color:#c2cbd3}
.node-card.selected:hover{background:#e0eefa;border-color:#a7c7e2}
.node-card .selection-mark{font-size:11px}
.node-card .source{min-width:0}
.dialog{width:1008px;max-width:calc(100% - 40px)}
.dialog-grid{grid-template-columns:repeat(4,minmax(0,1fr))}
.narrow-compare{grid-template-columns:repeat(2,minmax(0,1fr));gap:24px;margin-top:28px}
.narrow-compare .compare-panel{height:268px}
.narrow-compare .compare-scene{gap:28px;margin-top:26px}
.narrow-compare .previous-slot{width:276px}.narrow-slot{width:224px;flex-shrink:0}
.narrow-compare .narrow-slot{width:224px}.narrow-compare .measure{padding-left:18px;height:88px;gap:6px}
.wide-previous .card-core{grid-template-columns:minmax(0,1fr)74px;column-gap:8px}
.wide-previous .node-delay .delay{min-width:74px}
.narrow-bottom{grid-template-columns:390px minmax(0,1fr);gap:24px;margin-top:24px}
.narrow-bottom .detail-panel{height:336px}
.narrow-bottom .section-title{margin-bottom:20px}
.narrow-notes{font-size:12px;line-height:1.95;color:#616161;margin-top:16px}
.narrow-bottom .state-samples{display:grid;grid-template-columns:repeat(3,224px);gap:12px;margin-top:0}
.narrow-bottom .state-samples .node-card{width:224px}
.sample-labels{display:grid;grid-template-columns:repeat(3,224px);gap:12px;margin-top:10px;color:#777;font-size:12px;text-align:center}
.selection-notes{display:flex;align-items:center;gap:9px;margin-top:22px;font-size:12px;color:#555}
.selection-swatch{width:18px;height:14px;background:#e5f1fc;border-left:3px solid var(--blue);border-radius:2px}
.narrow-bottom .motion-note{margin-top:15px;padding-top:13px;line-height:1.9}
.comparison-bottomline{margin-top:22px}
'''
source = source.replace('</style>', css + '\n</style>', 1)

source = re.sub(r'^document\.getElementById\(\'middle-card\'\)\.innerHTML=.*?;\n', '', source, count=1, flags=re.M)
source = re.sub(r'^document\.getElementById\(\'before-card\'\)\.innerHTML=.*?;\n', "document.getElementById('before-card').innerHTML=card(data[1],1).replace('node-card','node-card wide-previous');\n", source, count=1, flags=re.M)

old_samples = "card(data[0],0,{selectable:true,demo:true})+card(data[2],2,{selected:true,selectable:true,demo:true})"
new_samples = "card(data[0],0,{selectable:true,demo:true})+card(data[3],3,{selectable:true,demo:true,hover:true})+card(data[2],2,{selected:true,selectable:true,demo:true})"
if old_samples not in source:
    raise RuntimeError('Missing comparison selection samples')
source = source.replace(old_samples, new_samples, 1)

# Keyboard navigation follows the four-column candidate layout.
source = source.replace('ArrowDown:3,ArrowUp:-3', 'ArrowDown:4,ArrowUp:-4')

(HERE / 'index.html').write_text(source, encoding='utf-8')
print('Created V3 preview:', HERE / 'index.html')
