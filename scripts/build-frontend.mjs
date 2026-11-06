import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { root, frontendSources, frontendFiles, frontendReceipt } from "./build-artifacts.mjs";

for (const [tool, args] of [
  ["typescript/bin/tsc", ["--noEmit"]],
  ["vite/bin/vite.js", ["build"]],
]) {
  execFileSync(process.execPath, [join(root, "node_modules", tool), ...args], { cwd: root, stdio: "inherit" });
}
writeFileSync(frontendReceipt, JSON.stringify({ sourceSha256: frontendSources(), filesSha256: frontendFiles() }, null, 2));
