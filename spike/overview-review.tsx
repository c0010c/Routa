// Development-only IPC fixtures. Uses production components; never imported by the product entry.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { createRoot } from "react-dom/client";
import type { ConnectionInfo, DesktopState } from "../src/bindings";

const params = new URLSearchParams(location.search);
const desktop: DesktopState = {
  preferences: {
    mode: "Full", listener_id: null, exit_id: null, system_listener_id: null,
    capture_mode: "System", auto_connect: false, close_to_tray: true,
    managed_rules: [], selected_apps: [],
  },
  connection: null, requested: null, error: null, is_admin: false, system_listener_id: null,
};
const samples: ConnectionInfo[] = [
  { id: "1", process: "C:\\Apps\\Chrome\\chrome.exe", target: "example.com:443", listener: "机场", node: "新加坡01", protocol: "TCP", uploaded: 12288, downloaded: 290816 },
  { id: "2", process: "C:\\Apps\\Chrome\\chrome.exe", target: "static.example.com:443", listener: "机场", node: "香港02", protocol: "TCP", uploaded: 4096, downloaded: 532480 },
  { id: "3", process: "C:\\Apps\\Chrome\\chrome.exe", target: "cdn.example.net:443", listener: "机场", node: "新加坡01", protocol: "UDP", uploaded: 8192, downloaded: 1572864 },
  { id: "4", process: "C:\\Apps\\Code\\Code.exe", target: "extensions.example.com:443", listener: "办公", node: "日本01", protocol: "TCP", uploaded: 6144, downloaded: 409600 },
  { id: "5", process: "", target: "api.example.org:443", listener: "机场", node: "新加坡01", protocol: "TCP", uploaded: 2048, downloaded: 20480 },
];
mockWindows("main");
mockIPC(async (command) => {
  if (command === "desktop_state") return structuredClone(desktop);
  if (command === "get_model" || command === "get_status") return new Promise(() => {});
  if (command === "get_connection_details") {
    if (params.has("error")) throw "内核无响应，请重试";
    if (params.has("empty")) return [];
    const elapsed = performance.now() / 1000;
    return samples.map((row, i) => ({ ...row,
      uploaded: row.uploaded + Math.round(elapsed * (i + 1) * 512),
      downloaded: row.downloaded + Math.round(elapsed * (i + 1) * 32768),
    }));
  }
  if (command === "read_core_log") {
    if (params.has("error")) throw "日志文件读取失败";
    return params.has("empty") ? "" : [
      'time="2026-10-29T22:49:09+08:00" level=warning msg="[TCP] dial 机场 127.0.0.1:38501 --> example.com:443 error: i/o timeout\\nretry another address: network unreachable"',
      ...Array.from({ length: 24 }, (_, i) => `time="2026-10-30T20:00:${String(i).padStart(2, "0")}+08:00" level=info msg="已加载代理配置 · 节点 ${i + 1}"`),
      'time="2026-10-30T20:01:00+08:00" level=warning msg="[TCP] dial 机场 127.0.0.1:4001 --> cdn.example.net:443 error: i/o timeout\\ndial tcp 192.0.2.1:443: i/o timeout\\nnetwork unreachable"',
      'time="2026-10-30T20:01:05+08:00" level=error msg="订阅更新失败：请求超时\\nretry failed"',
      'time="2026-10-30T20:02:00+08:00" level=info msg="Initial configuration complete, total time: 11ms"',
    ].join("\n");
  }
  if (command.startsWith("plugin:")) return null;
  throw `验收环境未提供命令：${command}`;
}, { shouldMockEvents: true });
const { default: App } = await import("../src/App");
createRoot(document.getElementById("root")!).render(<App />);
