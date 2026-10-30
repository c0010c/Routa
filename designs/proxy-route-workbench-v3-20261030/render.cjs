const path=require('node:path');
const sharp=require('C:/Users/Q/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');
(async()=>{
 for(const name of ['01-single-workbench','02-multiple-workbench','single-ui','multiple-ui','picker-ui','failure-ui','success-ui']){
  await sharp(path.join(__dirname,name+'.svg'),{density:144}).png().toFile(path.join(__dirname,name+'.png'));
  console.log(name+'.png');
 }
 for(let start=0;start<64;start+=4){
  await Promise.all(Array.from({length:Math.min(4,64-start)},(_,i)=>{
   const file=path.join(__dirname,'motion-frames',String(start+i).padStart(3,'0'));
   return sharp(file+'.svg',{density:72}).png().toFile(file+'.png');
  }));
 }
 console.log('Rendered animation frames locally.');
})();
