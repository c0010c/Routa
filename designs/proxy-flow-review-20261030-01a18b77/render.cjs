// Local vector rasterization only. Does not launch or control a browser.
const fs = require('node:fs');
const path = require('node:path');
const sharp = require('C:/Users/Q/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/sharp');
(async () => {
  for (const name of ['01-single-proxy', '02-multiple-proxies', 'single-ui', 'multiple-ui']) {
    const input = path.join(__dirname, name + '.svg');
    const output = path.join(__dirname, name + '.png');
    await sharp(input, { density: 144 }).png().toFile(output);
    const meta = await sharp(output).metadata();
    console.log(JSON.stringify({ file: path.basename(output), width: meta.width, height: meta.height }));
  }
})();
