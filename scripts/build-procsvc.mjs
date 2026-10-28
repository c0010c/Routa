// Builds the process-routing service (routa-procsvc, release) and copies it to
// src-tauri/binaries with the target-triple suffix Tauri's externalBin requires.
// Extra arguments are passed to cargo; `--locked` is added on CI (`CI` env var set), which also
// covers the run from tauri.conf.json's beforeBuildCommand. Uses its own target dir: tauri-build copies
// the externalBin to target/<profile>/routa-procsvc.exe, which would otherwise be the same file
// we copy from here.
import { execFileSync } from "node:child_process";
import { copyFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const tauriDir = join(root, "src-tauri");
const targetDir = join(tauriDir, "target", "procsvc-build");

execFileSync(
  "cargo",
  [
    "build",
    "--release",
    "-p",
    "routa-procsvc",
    "--manifest-path",
    join(tauriDir, "Cargo.toml"),
    "--target-dir",
    targetDir,
    ...(process.env.CI && !process.argv.includes("--locked") ? ["--locked"] : []),
    ...process.argv.slice(2),
  ],
  { cwd: root, stdio: "inherit" },
);

const target = join(tauriDir, "binaries", "routa-procsvc-x86_64-pc-windows-msvc.exe");
copyFileSync(join(targetDir, "release", "routa-procsvc.exe"), target);
console.log(`copied to ${target}`);
