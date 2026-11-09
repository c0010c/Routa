// Builds the prototype into one self-contained HTML file that opens from file://.
import { build } from "vite";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const out = path.join(here, "dist");
await build({
  configFile: path.join(here, "vite.config.mts"),
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
  logLevel: "warn",
  build: {
    outDir: out, emptyOutDir: true, assetsInlineLimit: 1e9,
    lib: { entry: path.join(here, "preview.tsx"), formats: ["iife"], name: "NodeMonitorProto", fileName: () => "proto.js" },
  },
});
mkdirSync(out, { recursive: true });
const js = readFileSync(path.join(out, "proto.js"), "utf8").replace(/<\/script/gi, "<\/script");
writeFileSync(path.join(here, "prototype.html"),
  `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Routa · 节点监测原型 · 示例数据</title></head><body><div id="root"></div><script>${js}</script></body></html>`);
console.log("wrote", path.join(here, "prototype.html"));
