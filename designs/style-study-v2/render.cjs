const fs=require('node:fs'),path=require('node:path');
const sharp=require('C:/Users/Q/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');
const root=__dirname,styles=JSON.parse(fs.readFileSync(path.join(root,'styles.json'),'utf8'));
const esc=s=>s.replaceAll('&','&amp;').replaceAll('<','&lt;');
(async()=>{
 const layers=[],labels=[];
 for(let i=0;i<styles.length;i++){
  const t=styles[i],input=path.join(root,t.id+'.svg');
  await sharp(input,{density:144}).png().toFile(path.join(root,t.id+'.png'));
  const x=32+i%2*1032,y=112+Math.floor(i/2)*784;
  layers.push({input:await sharp(input).resize(1000,693).png().toBuffer(),left:x,top:y+70});
  labels.push(`<text x="${x}" y="${y+24}" font-size="21" font-weight="600">${esc(t.name)}</text><text x="${x}" y="${y+49}" font-size="13" fill="#707782">${esc(t.desc)}</text>`);
 }
 const extra=`<text x="1090" y="1830" font-size="25" font-weight="600">这一组怎么比较</text><text x="1090" y="1880" font-size="18" fill="#687180">精密线框：更硬朗、更有结构。</text><text x="1090" y="1920" font-size="18" fill="#687180">深海蓝：更鲜明的深色方向。</text><text x="1090" y="1960" font-size="18" fill="#687180">鼠尾草：偏自然、柔和。</text><text x="1090" y="2000" font-size="18" fill="#687180">紫灰缎面：偏细腻、轻盈。</text><text x="1090" y="2040" font-size="18" fill="#687180">复古桌面：强调传统桌面工具感。</text><text x="1090" y="2120" font-size="15" fill="#9298a0">布局、功能与示例数据保持一致，仅探索视觉风格。</text>`;
 const heading=Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="2096" height="2464"><g font-family="Microsoft YaHei UI,Microsoft YaHei,Segoe UI,sans-serif" fill="#202126"><text x="32" y="49" font-size="30" font-weight="600">Routa · 更多视觉风格</text><text x="32" y="81" font-size="14" fill="#707782">同一套 Listener 布局，比较线条、表面层级、色调与控件质感。</text>${labels.join('')}${extra}</g></svg>`);
 await sharp({create:{width:2096,height:2464,channels:4,background:'#fafbfc'}}).composite([{input:heading,left:0,top:0},...layers]).png().toFile(path.join(root,'style-comparison.png'));
 console.log('Rendered five style mockups and comparison sheet');
})();
