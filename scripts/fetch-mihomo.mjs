// Downloads the pinned mihomo release and extracts the Windows exe into src-tauri/binaries.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync, copyFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const binDir = join(root, "src-tauri", "binaries");
const version = readFileSync(join(binDir, "MIHOMO_VERSION"), "utf8").trim();

// Digests are tied to `version` (v1.19.33); update together when bumping.
const ZIP_SHA256 = "6ab2180875f9f6fff8b19386d760daec257f66b45f72b3afc650634272fa837a";
const EXE_SHA256 = "1cde41cfffd3de0aedad63c0a8515e1ce5eea507e8133aed0f637d316c93800a";

const target = join(binDir, "mihomo-x86_64-pc-windows-msvc.exe");
const sha256 = (buf) => createHash("sha256").update(buf).digest("hex");

if (existsSync(target) && sha256(readFileSync(target)) === EXE_SHA256) {
  console.log(`mihomo ${version} already present, skipping`);
  process.exit(0);
}

const url = `https://github.com/MetaCubeX/mihomo/releases/download/${version}/mihomo-windows-amd64-${version}.zip`;

async function download(attempts = 5) {
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

// Local zip: `--zip <path>` or MIHOMO_ZIP env var skips the download (digest is still verified).
const zipArgIdx = process.argv.indexOf("--zip");
const localZip = zipArgIdx !== -1 ? process.argv[zipArgIdx + 1] : process.env.MIHOMO_ZIP;
if (zipArgIdx !== -1 && !localZip) {
  console.error("--zip requires a path");
  process.exit(1);
}
const zip = localZip ? readFileSync(resolve(localZip)) : await download();
const zipDigest = sha256(zip);
if (zipDigest !== ZIP_SHA256) {
  console.error(`zip sha256 mismatch: expected ${ZIP_SHA256}, got ${zipDigest}`);
  process.exit(1);
}

const work = mkdtempSync(join(tmpdir(), "mihomo-"));
try {
  const zipPath = join(work, "mihomo.zip");
  writeFileSync(zipPath, zip);
  // Use Windows' bundled bsdtar explicitly: bare `tar` in Git Bash is GNU tar, which cannot extract zip
  const bsdtar = join(process.env.SystemRoot ?? "C:\Windows", "System32", "tar.exe");
  execFileSync(bsdtar, ["-xf", zipPath, "-C", work], { stdio: "inherit" });
  const extracted = join(work, `mihomo-windows-amd64.exe`);
  if (!existsSync(extracted)) throw new Error("mihomo-windows-amd64.exe not found in archive");
  const exeDigest = sha256(readFileSync(extracted));
  if (exeDigest !== EXE_SHA256) {
    throw new Error(`exe sha256 mismatch: expected ${EXE_SHA256}, got ${exeDigest}`);
  }
  copyFileSync(extracted, target);
  console.log(`installed ${target}`);
} catch (e) {
  console.error(e.message ?? e);
  process.exit(1);
} finally {
  rmSync(work, { recursive: true, force: true });
}
