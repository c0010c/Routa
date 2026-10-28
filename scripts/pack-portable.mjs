// Builds the app without installers and zips a portable folder:
// routa.exe + mihomo.exe + routa-procsvc.exe + procsvc\ (process-routing files) + `portable` marker + LICENSE.
// `--skip-build` reuses an existing release build.
import { execFileSync, execSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const release = join(root, "src-tauri", "target", "release");
const procsvcSrc = join(root, "src-tauri", "binaries", "procsvc");
const { productName, version } = JSON.parse(
  readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"),
);

if (!process.argv.includes("--skip-build")) {
  execSync("pnpm tauri build --no-bundle", { cwd: root, stdio: "inherit" });
}

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

const outDir = join(release, "bundle", "portable");
const stage = join(outDir, productName);
const zipPath = join(outDir, `${productName}_${version}_x64_portable.zip`);
rmSync(stage, { recursive: true, force: true });
rmSync(zipPath, { force: true });
mkdirSync(join(stage, "procsvc"), { recursive: true });
for (const [src, name] of files) copyFileSync(src, join(stage, name));
writeFileSync(join(stage, "portable"), MARKER_NOTE);

// Windows' bundled bsdtar writes zip when the output ends in .zip (-a).
const bsdtar = join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe");
execFileSync(bsdtar, ["-a", "-c", "-f", zipPath, "-C", outDir, productName], { stdio: "inherit" });
rmSync(stage, { recursive: true, force: true });
console.log(`created ${zipPath}`);
