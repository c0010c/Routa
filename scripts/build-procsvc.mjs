// Builds the process-routing service (routa-procsvc, release) and copies it to
// src-tauri/binaries with the target-triple suffix Tauri's externalBin requires.
// Extra arguments are passed to cargo (e.g. `--locked`).
import { execFileSync } from "node:child_process";
import { copyFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const tauriDir = join(root, "src-tauri");

execFileSync(
  "cargo",
  [
    "build",
    "--release",
    "-p",
    "routa-procsvc",
    "--manifest-path",
    join(tauriDir, "Cargo.toml"),
    ...process.argv.slice(2),
  ],
  { cwd: root, stdio: "inherit" },
);

const target = join(tauriDir, "binaries", "routa-procsvc-x86_64-pc-windows-msvc.exe");
copyFileSync(join(tauriDir, "target", "release", "routa-procsvc.exe"), target);
console.log(`copied to ${target}`);
