const path=require('node:path');
const sharp=require('C:/Users/Q/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');
sharp(path.join(__dirname,'01-edit-column.svg'),{density:144}).png().toFile(path.join(__dirname,'01-edit-column.png')).then(()=>console.log('Rendered 2560 x 1536 preview locally.'));
