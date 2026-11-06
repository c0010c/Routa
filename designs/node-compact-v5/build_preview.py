from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
source = (HERE.parent / 'node-compact-v4' / 'index.html').read_text(encoding='utf-8')
source = source.replace('64px 单行节点卡片预览 V4', '64px 节点卡片比例微调 V5')
source = source.replace('64 px 单行卡片 V4', '64 px 卡片比例 V5')
# A four-digit sample makes the width and hierarchy checks reviewable.
source = source.replace("source:'示例订阅 A',ms:782", "source:'示例订阅 A',ms:1280", 1)

comparison = '''<section class="view comparison" id="comparison">
<div class="comparison-top"><div><div class="eyebrow">ROUTA / NODE CARDS / V5</div><h1>64 px 卡片，重新平衡字号</h1><div class="subtitle">名称保持 14 px，延迟从 18 px 收到 16 px；单位与操作图标降低视觉分量。</div></div><span class="proposal-tag">设计预览 · 比例微调</span></div>
<div class="compare-grid narrow-compare">
<div class="compare-panel"><div class="compare-label">上一版 V4<span class="dimension">名称 14 px / 延迟 18 px</span></div><div class="compare-scene"><div id="before-card" class="compare-node previous-slot"></div><div class="measure new"><b>18</b><span>px 延迟</span><span>ms 为 12 px</span></div></div><div class="compare-notes">在 64 px 高度内，延迟数字和单位比较醒目。</div></div>
<div class="compare-panel proposed"><div class="compare-label">比例微调 V5<span class="dimension">名称 14 px / 延迟 16 px</span></div><div class="compare-scene"><div id="after-card" class="compare-node narrow-slot"></div><div class="measure new"><b>16</b><span>px 延迟</span><span>ms 为 11 px · 次要色</span></div></div><div class="compare-notes">名称保持主层级，延迟依靠颜色、对齐和字重便于比较。</div></div>
</div>
<div class="comparison-bottom narrow-bottom">
<div class="detail-panel"><h2 class="section-title">卡片比例</h2><div class="type-spec"><span>卡片</span><b>约 224 × 64 px</b><span>名称</span><b>14 px / 600</b><span>延迟</span><b>16 px / 600</b><span>单位 ms</span><b>11 px / 400</b><span>协议、来源</span><b>12 px / 400</b><span>更多图标</span><b>16 px · 点击区 26 px</b></div><div class="small" style="margin-top:17px">名称继续单行省略，完整内容保留悬停提示。</div></div>
<div class="detail-panel"><h2 class="section-title">小数值、四位数与选中状态</h2><div class="state-samples" id="state-samples"></div><div class="sample-labels"><span>短数值</span><span>四位延迟</span><span>浅蓝选中</span></div><div class="selection-notes"><span class="selection-swatch"></span><span>浅蓝底色与小勾选保持清楚，延迟继续使用状态色</span></div><div class="motion-note">协议标签改为更轻的浅灰底；来源保持可读。<br>名称、延迟和底部信息仍分别对齐，卡片高度与点击范围保持稳定。</div><div class="small" style="margin-top:13px">超时、连接失败、未测试保持 12 px，避免状态文字挤占名称。</div></div>
</div>
<div id="anatomy-card" hidden></div>
<div class="comparison-bottomline"><span>同宽同高对照 · 约 224 × 64 px · 示例数据</span><span>名称优先 · 延迟易比较 · 辅助元素更轻</span></div>
</section>'''
source = re.sub(r'<section class="view comparison" id="comparison">.*?</section>', lambda _: comparison, source, count=1, flags=re.S)

css = '''
/* V5: typography hierarchy within the accepted 64 px footprint. */
.card-core{grid-template-columns:minmax(0,1fr)60px}
.node-delay .delay{font-size:16px;font-weight:600;min-width:60px;line-height:20px;gap:3px}
.node-delay .delay .unit{font-size:11px;font-weight:400;color:#616161;line-height:1}
.node-card .node-meta .pill{background:#f4f4f4;border-color:transparent;color:#616161;font-weight:400}
.node-card.selected .node-meta .pill{background:#ffffff70;border-color:transparent}
.node-card .card-more{font-size:16px;color:#707070;width:26px;height:26px}
.node-card .check{width:12px;height:12px;font-size:8px}
.node-card .selection-mark{gap:4px}
.node-card .node-title{font-size:14px;font-weight:600}
.node-delay .delay.failure,.node-delay .delay.unset,.node-delay .delay.pending{font-size:12px;font-weight:500}
.wide-previous{height:64px}
.wide-previous .card-core{padding:9px 11px;grid-template-columns:minmax(0,1fr)64px;grid-template-rows:20px 18px;row-gap:6px;column-gap:6px}
.wide-previous .node-title{display:block;height:20px;line-height:20px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;overflow-wrap:normal;-webkit-line-clamp:unset}
.wide-previous .node-delay{height:20px}
.wide-previous .node-delay .delay{font-size:18px;min-width:64px;line-height:20px;height:20px;gap:4px}
.wide-previous .node-delay .delay .unit{font-size:12px;color:inherit}
.wide-previous .node-meta{height:18px}
.wide-previous .node-meta .pill{background:transparent;border-color:#d6d6d6}
.wide-previous .card-more{font-size:19px;bottom:4px;color:#616161}
.narrow-compare .compare-panel:first-child .measure{height:64px}
.narrow-bottom .detail-panel{height:310px}
.type-spec{display:grid;grid-template-columns:100px 1fr;gap:11px 12px;font-size:12px;line-height:17px}
.type-spec>span{color:#616161}.type-spec>b{font-weight:500}
'''
source = source.replace('</style>', css + '\n</style>', 1)

source = source.replace("card(data[6],6).replace('node-card','node-card wide-previous')", "card(data[1],1).replace('node-card','node-card wide-previous')", 1)
source = source.replace("document.getElementById('after-card').innerHTML=card(data[6],6)", "document.getElementById('after-card').innerHTML=card(data[1],1)", 1)
old_samples = "card(data[0],0,{selectable:true,demo:true})+card(data[3],3,{selectable:true,demo:true,hover:true})+card(data[2],2,{selected:true,selectable:true,demo:true})"
new_samples = "card(data[0],0,{selectable:true,demo:true})+card(data[2],2,{selectable:true,demo:true})+card(data[1],1,{selected:true,selectable:true,demo:true})"
if old_samples not in source:
    raise RuntimeError('Missing proportion comparison sample insertion point')
source = source.replace(old_samples, new_samples, 1)

(HERE / 'index.html').write_text(source, encoding='utf-8')
print('Created V5 preview:', HERE / 'index.html')
