// Prototype-only dev server: serves the repo root and swaps the production
// NodeCard for a wrapper that adds the monitor mark and menu items.
import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../..");
const wrapper = path.join(here, "MonitoredNodeCard.tsx");

const swapNodeCard: Plugin = {
  name: "swap-node-card",
  enforce: "pre",
  async resolveId(source, importer, options) {
    if (!importer || !/NodeCard$/.test(source)) return null;
    if (path.normalize(importer) === path.normalize(wrapper)) return null;
    const resolved = await this.resolve(source, importer, { ...options, skipSelf: true });
    if (resolved && /src[\\/]components[\\/]NodeCard\.tsx$/.test(resolved.id)) return wrapper;
    return null;
  },
};

export default defineConfig({
  root,
  plugins: [swapNodeCard, react()],
  clearScreen: false,
  server: { port: 1437, strictPort: true, open: false, watch: { ignored: ["**/src-tauri/**"] } },
});
