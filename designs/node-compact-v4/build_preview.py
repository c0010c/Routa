from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
source = (HERE.parent / 'node-compact-v3' / 'index.html').read_text(encoding='utf-8')
source = source.replace('窄卡片与颜色选择预览 V3', '64px 单行节点卡片预览 V4')
source = source.replace('224 px 窄卡片 V3', '64 px 单行卡片 V4')

comparison = '''<section class="view comparison" id="comparison">
<div class="comparison-top"><div><div class="eyebrow">ROUTA / NODE CARDS / V4</div><h1>单行名称，64 px 高度</h1><div class="subtitle">名称只占一行，超出直接显示省略号。保留窄卡片、浅蓝选中底色与完整名称提示。</div></div><span class="proposal-tag">设计预览 · 示例数据</span></div>
<div class="compare-grid narrow-compare">
<div class="compare-panel"><div class="compare-label">上一版 V3<span class="dimension">224 × 88 px</span></div><div class="compare-scene"><div id="before-card" class="compare-node previous-slot"></div><div class="measure new"><b>88</b><span>px 高</span><span>名称最多两行</span></div></div><div class="compare-notes">保留两行名称空间，底行展示协议与来源。</div></div>
<div class="compare-panel proposed"><div class="compare-label">单行方案 V4<span class="dimension">224 × 64 px</span></div><div class="compare-scene"><div id="after-card" class="compare-node narrow-slot"></div><div class="measure new"><b>64</b><span>px 高 · 再减少约 27%</span><span>名称单行省略</span></div></div><div class="compare-notes">名称、延迟同一行；协议、来源和操作收在底行。</div></div>
</div>
<div class="comparison-bottom narrow-bottom">
<div class="detail-panel"><h2 class="section-title">长名称直接省略</h2><div id="anatomy-card" class="narrow-slot"></div><div class="narrow-notes">名称保持 14 px，不换行。<br>超出宽度用省略号截断。<br>悬停或聚焦可查看完整名称。</div><div class="small" style="margin-top:16px">协议与来源仍为 12 px，卡片宽度沿用上一版。</div></div>
<div class="detail-panel"><h2 class="section-title">浅蓝选中，紧凑排列</h2><div class="state-samples" id="state-samples"></div><div class="sample-labels"><span>默认</span><span>悬停</span><span>选中</span></div><div class="selection-notes"><span class="selection-swatch"></span><span>保留浅蓝填充、蓝色侧标与小勾选</span></div><div class="motion-note">名称和卡片始终同高，选择与悬停不改变尺寸。<br>测量时间、完整名称等辅助信息仍放在悬停提示中。</div><div class="small" style="margin-top:13px">节点页五列，切换弹窗四列；列表滚动时底部操作保持固定。</div></div>
</div>
<div class="comparison-bottomline"><span>同宽 224 px 对照 · 高度 88 → 64 px · 单行省略号</span><span>浅蓝色表达选择 · 绿 / 橙 / 红表达延迟</span></div>
</section>'''
source = re.sub(r'<section class="view comparison" id="comparison">.*?</section>', lambda _: comparison, source, count=1, flags=re.S)

css = '''
/* V4: a single title line, with no business-source changes. */
.node-card{height:64px}
.card-core{padding:9px 11px;grid-template-rows:20px 18px;row-gap:6px}
.node-card .node-title{display:block;min-width:0;height:20px;line-height:20px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;overflow-wrap:normal;-webkit-line-clamp:unset}
.node-delay{height:20px;gap:0}
.node-delay .delay{line-height:20px;min-height:20px;height:20px}
.node-delay .delay.failure,.node-delay .delay.unset,.node-delay .delay.pending{line-height:20px}
.node-card .node-meta{height:18px;line-height:18px}
.node-card .selection-mark{position:absolute;right:36px;bottom:9px;height:18px;line-height:18px;font-size:11px}
.node-card:has(.selection-mark:not(:empty)) .node-meta{padding-right:72px}
.node-card.selected:not(.current) .node-meta{padding-right:50px}
.node-card .card-more{bottom:4px}
.node-card.selected:before{top:8px;bottom:8px}
.narrow-compare .previous-slot{width:224px}
.narrow-compare .measure{height:64px}
.narrow-compare .compare-panel:first-child .measure{height:88px}
.wide-previous{height:88px}
.wide-previous .card-core{padding:11px;grid-template-columns:minmax(0,1fr)64px;grid-template-rows:36px 20px;row-gap:8px;column-gap:6px}
.wide-previous .node-title{display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;height:36px;line-height:18px;white-space:normal;overflow-wrap:anywhere;text-overflow:clip}
.wide-previous .node-delay{height:36px}
.wide-previous .node-delay .delay{min-width:64px;height:21px;line-height:21px}
.wide-previous .node-meta{height:20px}
.wide-previous .card-more{bottom:6px}
.narrow-bottom .detail-panel{height:310px}
.narrow-bottom .motion-note{margin-top:18px}
.narrow-notes{margin-top:18px}
'''
source = source.replace('</style>', css + '\n</style>', 1)

source = source.replace("card(data[1],1).replace('node-card','node-card wide-previous')", "card(data[6],6).replace('node-card','node-card wide-previous')", 1)
source = source.replace("document.getElementById('after-card').innerHTML=card(data[1],1)", "document.getElementById('after-card').innerHTML=card(data[6],6)", 1)

# Compact status marks leave the metadata row available in the shorter card.
source = source.replace("current?'当前使用':'待切换'", "current?'当前':''")
source = source.replace('<i class="current-dot"></i>当前使用', '<i class="current-dot"></i>当前')
source = source.replace('<i class="check">✓</i>已选择', '<i class="check">✓</i>')

(HERE / 'index.html').write_text(source, encoding='utf-8')
print('Created V4 preview:', HERE / 'index.html')
