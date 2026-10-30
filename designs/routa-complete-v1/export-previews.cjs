const fs = require('node:fs');
const path = require('node:path');
const sharp = require('C:/Users/Q/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');
const root = __dirname, screens = path.join(root, 'screens');
const scenes = JSON.parse(fs.readFileSync(path.join(screens, 'manifest.json'), 'utf8'));
const esc = s => s.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('"', '&quot;');

(async () => {
  for (const item of scenes) {
    const file = path.join(screens, item.id + '-review.jpg');
    const metadata = await sharp(file).metadata();
    const scale = metadata.width / item.box.pageWidth;
    const left = Math.round(item.box.x * scale), top = Math.round(item.box.y * scale);
    const width = Math.min(Math.round(item.box.width * scale), metadata.width - left);
    const height = Math.min(Math.round(item.box.height * scale), metadata.height - top);
    if (width < 1 || height < 1) throw Error('Invalid bounds: ' + item.id);
    await sharp(file).extract({left, top, width, height}).png().toFile(path.join(screens, item.id + '.png'));
  }

  const panels = [
    {id:'simple-connected',label:'简洁版 / 日常连接',x:32,y:100,w:868,h:640},
    {id:'simple-apps',label:'简洁版 / 指定应用',x:940,y:100,w:868,h:640},
    {id:'full-overview',label:'完整版 / 概览',x:32,y:822,w:868,h:600},
    {id:'full-listeners',label:'完整版 / 代理',x:940,y:822,w:868,h:600},
  ];
  const labels = panels.map(p=>`<text x="${p.x}" y="${p.y-20}" font-size="20" font-weight="600">${p.label}</text>`).join('');
  const heading = Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="1840" height="1460"><g font-family="Segoe UI,Microsoft YaHei UI,Microsoft YaHei,sans-serif" fill="#20242c"><text x="32" y="34" font-size="24" font-weight="600">Routa · 完整交互与 UI 设计</text><text x="1808" y="33" text-anchor="end" font-size="13" fill="#707986">亮色版 · 29 个页面与状态 · 可点击原型</text>${labels}</g></svg>`);
  const layers = [{input:heading,left:0,top:0}];
  for (const p of panels) {
    const meta = await sharp(path.join(screens,p.id+'.png')).metadata();
    const scale = Math.min(p.w/meta.width,p.h/meta.height);
    const width=Math.round(meta.width*scale),height=Math.round(meta.height*scale);
    layers.push({input:await sharp(path.join(screens,p.id+'.png')).resize(width,height).png().toBuffer(),left:p.x+Math.round((p.w-width)/2),top:p.y});
  }
  await sharp({create:{width:1840,height:1460,channels:4,background:'#e9ebef'}}).composite(layers).png().toFile(path.join(root,'design-overview.png'));

  fs.writeFileSync(path.join(screens,'index.html'),`<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Routa · 页面与状态总览</title><style>body{font:14px 'Segoe UI','Microsoft YaHei',sans-serif;color:#20242c;background:#eef0f4;margin:30px}h1{font-size:25px}main{display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:24px}article{background:white;padding:16px;border:1px solid #d9dde4;border-radius:8px}article img{width:100%;height:340px;object-fit:contain;object-position:top;background:#f3f3f3}h2{font-size:15px}a{color:#0067c0;text-decoration:none}.links{margin:15px 0 25px;display:flex;gap:25px}</style></head><body><h1>Routa · 29 个页面与状态</h1><div class="links"><a href="../index.html">可点击原型</a><a href="../specification.html">交互规范</a></div><main>${scenes.map(s=>`<article><a href="${s.id}.png" target="_blank" rel="noopener"><img src="${s.id}.png" alt="${esc(s.label)}" loading="lazy"></a><h2>${esc(s.label)}</h2><a href="../index.html#${s.id}">进入此交互场景 →</a></article>`).join('')}</main></body></html>`, 'utf8');
  console.log(`Exported ${scenes.length} UI screenshots, a gallery, and design-overview.png`);
})();
