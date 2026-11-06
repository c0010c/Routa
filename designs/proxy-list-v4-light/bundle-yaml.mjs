import {build} from 'vite';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const dir=path.dirname(fileURLToPath(import.meta.url));
await build({configFile:false,logLevel:'warn',build:{outDir:dir,emptyOutDir:false,minify:true,lib:{entry:path.resolve('node_modules/.pnpm/yaml@1.10.3/node_modules/yaml/browser/dist/index.js'),name:'RoutaYaml',formats:['iife'],fileName:()=> 'yaml-browser.js'}}});
fs.copyFileSync('node_modules/.pnpm/yaml@1.10.3/node_modules/yaml/LICENSE',path.join(dir,'YAML-LICENSE'));
