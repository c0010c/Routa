import { verifyService, verifyFrontend, guiSources } from "./build-artifacts.mjs";

try {
  const service = verifyService();
  const frontend = verifyFrontend();
  // build.rs 将这些校验过的摘要嵌入 GUI，可用于核对 --skip-build 打包产物。
  console.log(`cargo:rustc-env=ROUTA_SERVICE_SHA256=${service.exeSha256}`);
  console.log(`cargo:rustc-env=ROUTA_FRONTEND_SHA256=${frontend.filesSha256}`);
  console.log(`cargo:rustc-env=ROUTA_GUI_SOURCE_SHA256=${guiSources()}`);
} catch (error) {
  console.error(`构建产物检查失败：${error.message}\n请先构建分流服务和前端，再构建主程序。`);
  process.exitCode = 1;
}
