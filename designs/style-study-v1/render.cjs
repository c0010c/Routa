const fs=require('node:fs');
const path=require('node:path');
const sharp=require('C:/Users/Q/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');
const root=__dirname;
const styles=JSON.parse(fs.readFileSync(path.join(root,'styles.json'),'utf8'));
const esc=s=>s.replaceAll('&','&amp;').replaceAll('<','&lt;');
(async()=>{
  const layers=[];
  const labels=[];
  for(let i=0;i<styles.length;i++){
    const s=styles[i];
    const input=path.join(root,s.id+'.svg');
    await sharp(input,{density:144}).png().toFile(path.join(root,s.id+'.png'));
    const x=32+(i%2)*1032, y=112+Math.floor(i/2)*784;
    const shot=await sharp(input).resize(1000,693).png().toBuffer();
    layers.push({input:shot,left:x,top:y+70});
    labels.push(`<text x="${x}" y="${y+24}" font-size="21" font-weight="600">${esc(s.name)}</text><text x="${x}" y="${y+49}" font-size="13" fill="#707782">${esc(s.desc)}</text>`);
  }
  const heading=Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="2096" height="2464"><g font-family="Microsoft YaHei UI,Microsoft YaHei,Segoe UI,sans-serif" fill="#202126"><text x="32" y="49" font-size="30" font-weight="600">Routa · 视觉风格对照</text><text x="32" y="81" font-size="14" fill="#707782">相同布局、功能与示例数据，仅比较表面质感、边框、圆角、阴影和配色。</text>${labels.join('')}</g></svg>`);
  await sharp({create:{width:2096,height:2464,channels:4,background:'#fafbfc'}}).composite([{input:heading,left:0,top:0},...layers]).png().toFile(path.join(root,'style-comparison.png'));
  console.log('Rendered six full-size mockups and style-comparison.png');
})();
