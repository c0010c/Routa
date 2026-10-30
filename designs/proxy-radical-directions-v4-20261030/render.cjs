const path=require('node:path');
const sharp=require('C:/Users/Q/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');
(async()=>{
 const names=['01-connection-stage','02-proxy-console','03-port-matrix','stage-single','stage-multiple','console-single','console-multiple','matrix-single','matrix-multiple','motion-static'];
 for(const name of names){await sharp(path.join(__dirname,name+'.svg'),{density:name==='motion-static'?72:144}).png().toFile(path.join(__dirname,name+'.png'));console.log(name+'.png')}
 for(let start=0;start<80;start+=4)await Promise.all(Array.from({length:Math.min(4,80-start)},(_,i)=>{const file=path.join(__dirname,'motion-frames',String(start+i).padStart(3,'0'));return sharp(file+'.svg',{density:72}).png().toFile(file+'.png')}));
 console.log('Rendered 80 motion frames without a browser.');
})();
