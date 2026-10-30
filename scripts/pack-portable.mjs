// Builds the app without installers and zips a portable folder:
// routa.exe + mihomo.exe + routa-procsvc.exe + procsvc\ (process-routing files) + `portable` marker + LICENSE.
// `--skip-build` reuses an existing release build.
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { verifyRelease, sha256, assertMatch } from "./build-artifacts.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const release = join(root, "src-tauri", "target", "release");
const procsvcSrc = join(root, "src-tauri", "binaries", "procsvc");
const { productName, version } = JSON.parse(
  readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"),
);

if (!process.argv.includes("--skip-build")) {
  execFileSync(process.execPath, [join(root, "node_modules/@tauri-apps/cli/tauri.js"), "build", "--no-bundle", "--features", "custom-protocol"], { cwd: root, stdio: "inherit" });
}

// --skip-build 也须证明 GUI、前端、协议、服务来自一致的构建。
const build = verifyRelease(release);

// The marker's content is only a note for users; the app checks that the file exists.
const MARKER_NOTE =
  "This file enables portable mode: all data is stored in the data folder next to the exe.\r\n" +
  "Delete it to store data in %APPDATA% instead.\r\n" +
  "此文件用于开启便携模式：所有数据保存在 exe 旁边的 data 文件夹中。\r\n" +
  "删除它则改为保存到 %APPDATA%。\r\n";

const files = [
  [join(release, `${productName}.exe`), `${productName}.exe`],
  [join(release, "mihomo.exe"), "mihomo.exe"],
  [join(release, "routa-procsvc.exe"), "routa-procsvc.exe"],
  ...["ProxyBridgeCore.dll", "WinDivert.dll", "WinDivert64.sys"].map((f) => [
    join(procsvcSrc, f),
    join("procsvc", f),
  ]),
  [join(root, "LICENSE"), "LICENSE"],
];
for (const [src] of files) {
  if (!existsSync(src)) {
    console.error(
      `missing ${src}; run without --skip-build (and \`pnpm fetch-mihomo\`, ` +
        "`pnpm fetch-windivert`, `pnpm build-proxybridge` first)",
    );
    process.exit(1);
  }
}

const outputIndex = process.argv.indexOf("--out-dir");
if (outputIndex >= 0 && !process.argv[outputIndex + 1]) throw new Error("--out-dir 缺少目录");
const stamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\..+/, "").replace("T", "-");
const outDir = outputIndex >= 0 ? resolve(process.argv[outputIndex + 1]) : join(release, "bundle", `portable-${stamp}`);
const stage = join(outDir, productName);
const zipPath = join(outDir, `${productName}_${version}_x64_portable.zip`);
if (existsSync(stage) || existsSync(zipPath)) throw new Error(`输出已存在，请换一个 --out-dir，避免覆盖便携程序或 data：${outDir}`);
mkdirSync(join(stage, "procsvc"), { recursive: true });
for (const [src, name] of files) copyFileSync(src, join(stage, name));
writeFileSync(join(stage, "portable"), MARKER_NOTE);
for (const [src, name] of files) assertMatch(sha256(join(stage, name)), sha256(src), `打包文件校验失败：${name}`);
writeFileSync(join(stage, "build-info.json"), JSON.stringify({
  version, createdAt: new Date().toISOString(), ...build,
  files: Object.fromEntries([...files.map(([, name]) => name), "portable"].map((name) => [name.replaceAll("\\", "/"), sha256(join(stage, name))])),
}, null, 2));

// Windows' bundled bsdtar writes zip when the output ends in .zip (-a).
const bsdtar = join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe");
execFileSync(bsdtar, ["-a", "-c", "-f", zipPath, "-C", outDir, productName], { stdio: "inherit" });
console.log(`created ${zipPath}`);
console.log(`portable folder: ${stage}`);
