const path=require('node:path');
const sharp=require('C:/Users/Q/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');
(async()=>{for(const name of ['01-ux-list','02-recoverable-switch'])await sharp(path.join(__dirname,name+'.svg'),{density:144}).png().toFile(path.join(__dirname,name+'.png'));console.log('Rendered both UX previews at 2560 x 1536.');})();
