// Downloads the pinned WinDivert release into third_party/windivert (gitignored) and copies the
// x64 runtime files (WinDivert.dll / WinDivert64.sys) into src-tauri/binaries/procsvc.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const version = "2.2.2";
const dirName = `WinDivert-${version}-A`;
const thirdPartyDir = join(root, "third_party", "windivert");
const extractedDir = join(thirdPartyDir, dirName);
const headerPath = join(extractedDir, "include", "windivert.h");
const outDir = join(root, "src-tauri", "binaries", "procsvc");

// Digest is tied to `version`; update together when bumping.
// windivert.h is missing from the -A zip, so it is fetched separately from the matching source tag.
const HEADER_SHA256 = "5017a1768c1592fd664c0c2d3d2d30f81fad4ab98d322b1914c6f0a33fcacdf9";
const ZIP_SHA256 = "63cb41763bb4b20f600b6de04e991a9c2be73279e317d4d82f237b150c5f3f15";

const sha256 = (buf) => createHash("sha256").update(buf).digest("hex");
const runtimeFiles = ["WinDivert.dll", "WinDivert64.sys"];

function copyRuntime() {
  mkdirSync(outDir, { recursive: true });
  for (const f of runtimeFiles) {
    const src = join(extractedDir, "x64", f);
    if (!existsSync(src)) throw new Error(`${src} not found`);
    copyFileSync(src, join(outDir, f));
  }
}

const marker = join(thirdPartyDir, ".zip-sha256");
if (
  existsSync(marker) &&
  readFileSync(marker, "utf8").trim() === ZIP_SHA256 &&
  existsSync(headerPath) &&
  existsSync(join(extractedDir, "x64", "WinDivert.lib"))
) {
  copyRuntime();
  console.log(`WinDivert ${version} already present, skipping download`);
  process.exit(0);
}

const url = `https://github.com/basil00/WinDivert/releases/download/v${version}/${dirName}.zip`;
const headerUrl = `https://raw.githubusercontent.com/basil00/WinDivert/v${version}/include/windivert.h`;

async function download(url, attempts = 5) {
  let lastErr;
  for (let i = 1; i <= attempts; i++) {
    try {
      console.log(`downloading ${url} (attempt ${i}/${attempts})`);
      const res = await fetch(url, { redirect: "follow", signal: AbortSignal.timeout(900_000) });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      return Buffer.from(await res.arrayBuffer());
    } catch (e) {
      lastErr = e;
      console.warn(`attempt ${i} failed: ${e.message ?? e}`);
      await new Promise((r) => setTimeout(r, 2000 * i));
    }
  }
  throw new Error(`download failed after ${attempts} attempts: ${lastErr}`);
}

// Local zip: `--zip <path>` or WINDIVERT_ZIP env var skips the download (digest is still verified).
const zipArgIdx = process.argv.indexOf("--zip");
const localZip = zipArgIdx !== -1 ? process.argv[zipArgIdx + 1] : process.env.WINDIVERT_ZIP;
if (zipArgIdx !== -1 && !localZip) {
  console.error("--zip requires a path");
  process.exit(1);
}
const zip = localZip ? readFileSync(resolve(localZip)) : await download(url);
const zipDigest = sha256(zip);
if (zipDigest !== ZIP_SHA256) {
  console.error(`zip sha256 mismatch: expected ${ZIP_SHA256}, got ${zipDigest}`);
  process.exit(1);
}

try {
  rmSync(extractedDir, { recursive: true, force: true });
  mkdirSync(thirdPartyDir, { recursive: true });
  // Use Windows' bundled bsdtar explicitly: bare `tar` in Git Bash is GNU tar, which cannot extract zip.
  // The zip is streamed via stdin so no temporary copy is written to disk.
  const bsdtar = join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe");
  execFileSync(bsdtar, ["-xf", "-", "-C", thirdPartyDir], { input: zip, stdio: ["pipe", "inherit", "inherit"] });
  const header = await download(headerUrl);
  const headerDigest = sha256(header);
  if (headerDigest !== HEADER_SHA256) {
    throw new Error(`windivert.h sha256 mismatch: expected ${HEADER_SHA256}, got ${headerDigest}`);
  }
  mkdirSync(dirname(headerPath), { recursive: true });
  writeFileSync(headerPath, header);
  copyRuntime();
  writeFileSync(marker, ZIP_SHA256);
  console.log(`installed WinDivert ${version} into ${thirdPartyDir} and ${outDir}`);
} catch (e) {
  console.error(e.message ?? e);
  process.exit(1);
}
