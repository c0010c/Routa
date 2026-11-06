const path=require('node:path');
const sharp=require('C:/Users/Q/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');
(async()=>{
 for(const name of ['01-single-refined','02-list-refined','single-ui','multiple-ui','motion-static']){
  await sharp(path.join(__dirname,name+'.svg'),{density:name==='motion-static'?72:144}).png().toFile(path.join(__dirname,name+'.png'));
  console.log(name+'.png');
 }
 for(let start=0;start<72;start+=4){
  await Promise.all(Array.from({length:Math.min(4,72-start)},(_,i)=>{
   const base=path.join(__dirname,'motion-frames',String(start+i).padStart(3,'0'));
   return sharp(base+'.svg',{density:72}).png().toFile(base+'.png');
  }));
 }
 console.log('Rendered 72 animation frames locally without a browser.');
})();
