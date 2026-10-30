import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const serviceExe = join(root, "src-tauri/binaries/routa-procsvc-x86_64-pc-windows-msvc.exe");
export const serviceReceipt = join(root, "src-tauri/binaries/routa-procsvc.build.json");
export const frontendReceipt = join(root, "dist/routa-frontend.json");
export const sha256 = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");

export function treeHash(base, inputs, exclude = []) {
  const files = [];
  function visit(path) {
    if (!existsSync(path)) return;
    const rel = relative(base, path).replaceAll("\\", "/");
    if (exclude.includes(rel)) return;
    if (statSync(path).isDirectory()) {
      for (const child of readdirSync(path)) visit(join(path, child));
    } else files.push(path);
  }
  inputs.forEach((input) => visit(join(base, input)));
  const hash = createHash("sha256");
  for (const path of [...new Set(files)].sort()) {
    hash.update(relative(base, path).replaceAll("\\", "/")).update("\0");
    hash.update(readFileSync(path)).update("\0");
  }
  return hash.digest("hex");
}

export const serviceSources = () => treeHash(root, [
  "src-tauri/procsvc", "src-tauri/procsvc-proto", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock",
  "scripts/build-procsvc.mjs", "scripts/build-artifacts.mjs",
]);
export const frontendSources = () => treeHash(root, [
  "src", "public", "index.html", "package.json", "pnpm-lock.yaml", "vite.config.ts",
  "tsconfig.json", "tsconfig.node.json", "scripts/build-frontend.mjs", "scripts/build-artifacts.mjs",
]);
export const guiSources = () => treeHash(root, [
  "src-tauri/src", "src-tauri/build.rs", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock",
  "src-tauri/tauri.conf.json", "src-tauri/capabilities", "src-tauri/icons", "src-tauri/nsis",
]);
export const frontendFiles = () => treeHash(join(root, "dist"), ["."], ["routa-frontend.json"]);
export const protocolVersion = () => Number(readFileSync(join(root, "src-tauri/procsvc-proto/src/lib.rs"), "utf8")
  .match(/pub const PROTO_VERSION:\s*u32\s*=\s*(\d+)/)?.[1]);

export function readBuildInfo(exe) {
  // 老 GUI 不认识此参数时会正常启动；先检查标记，拒绝执行没有信息入口的旧程序。
  if (!readFileSync(exe).includes(Buffer.from("ROUTA_BUILD_INFO_V1"))) {
    throw new Error(`构建信息缺失，请重新构建：${exe}`);
  }
  const info = JSON.parse(execFileSync(exe, ["--build-info"], { encoding: "utf8", timeout: 10000, windowsHide: true }));
  assertMatch(info.format, "ROUTA_BUILD_INFO_V1", "不支持的构建信息格式");
  return info;
}

export function assertMatch(actual, expected, message) {
  if (actual !== expected) throw new Error(message);
}

export function verifyService({ receiptPath = serviceReceipt, exe = serviceExe, sourceHash = serviceSources(), protocol = protocolVersion() } = {}) {
  const receipt = JSON.parse(readFileSync(receiptPath, "utf8"));
  assertMatch(receipt.sourceSha256, sourceHash, "分流服务源码已变化，请先运行 node scripts/build-procsvc.mjs");
  assertMatch(receipt.exeSha256, sha256(exe), "分流服务 EXE 与构建记录不一致，请重新构建分流服务");
  assertMatch(readBuildInfo(exe).protocolVersion, protocol, "分流服务协议版本与 GUI 源码不一致");
  return receipt;
}

export function verifyFrontend() {
  const receipt = JSON.parse(readFileSync(frontendReceipt, "utf8"));
  assertMatch(receipt.sourceSha256, frontendSources(), "前端源码已变化，请先运行 node scripts/build-frontend.mjs");
  assertMatch(receipt.filesSha256, frontendFiles(), "前端产物与构建记录不一致，请重新构建前端");
  return receipt;
}

export function verifyRelease(release) {
  const service = verifyService();
  const frontend = verifyFrontend();
  assertMatch(sha256(join(release, "routa-procsvc.exe")), service.exeSha256, "release 目录中的分流服务已过期，请重新构建主程序");
  const gui = readBuildInfo(join(release, "routa.exe"));
  assertMatch(gui.serviceSha256, service.exeSha256, "主程序与分流服务不是同一批构建，请重新构建主程序");
  assertMatch(gui.frontendSha256, frontend.filesSha256, "主程序未包含最新前端，请重新构建主程序");
  assertMatch(gui.protocolVersion, protocolVersion(), "主程序的分流协议已过期，请重新构建主程序");
  assertMatch(gui.sourceSha256, guiSources(), "主程序源码已变化，请重新构建主程序");
  assertMatch(gui.customProtocol, true, "主程序未启用 custom-protocol，不能用于便携包");
  return { service, frontend, gui };
}
