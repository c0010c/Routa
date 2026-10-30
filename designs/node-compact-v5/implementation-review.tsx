// Isolated visual acceptance harness. These are the production components;
// only IPC is replaced with deterministic, credential-free fixture data.
import { createRoot } from "react-dom/client";
import { CssBaseline, ThemeProvider } from "@mui/material";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { theme } from "../../src/theme";
import { Layout } from "../../src/components/Layout";
import { ApplyResultProvider } from "../../src/components/ApplyResultSnackbar";
import { ResourceCenter } from "../../src/pages/ResourceCenter";
import { Listeners } from "../../src/pages/Listeners";
import { commands, type Model, type RuntimeStatus, type NodeRef, type DelayResult } from "../../src/bindings";

const names = ["日本 · 东京 01", "新加坡 · 狮城 01", "美国 · 洛杉矶 02", "日本 · 大阪 02", "德国 · 法兰克福 01", "英国 · 伦敦 01", "日本 · 东京 IPLC 专线 03 · 流媒体解锁 · 高倍率 · 长期备用线路 · Ultra Premium", "澳大利亚 · 悉尼 01", "香港 · 中环 01", "日本 · 东京 05", "新加坡 · 滨海 02", "加拿大 · 温哥华 01", "美国 · 圣何塞 01", "荷兰 · 阿姆斯特丹 01", "法国 · 巴黎 01", "新加坡 · 狮城 IEPL 备用线路 03 · 流媒体与工作专用", "台湾 · 台北 01", "韩国 · 首尔 01", "美国 · 西雅图 03", "英国 · 曼彻斯特 02"];
const protocols = ["Shadowsocks", "VLESS", "Trojan", "Trojan", "VLESS", "Hysteria2", "Shadowsocks", "VMess"];
const values = [48, 487, 1280, 170, null, null, 63, undefined, 32, 118, 268, 361, 525, null, null, undefined, 89, 143, 601, 590];
const completed = Math.floor(Date.now() / 1000) - 120;
let model = {
  subscriptions: [0, 1].map((i) => ({ id: `s${i + 1}`, name: `示例订阅 ${i ? "B" : "A"}`, url: "https://example.invalid/subscription", update_interval_mins: 60, fallback_exit: null, last_updated: null, last_result: null, nodes: names.slice(i ? 8 : 0, i ? 20 : 8).map((name, j) => ({ name, proxy_type: protocols[(j + (i ? 8 : 0)) % protocols.length], server: "example.invalid", raw: { name, type: "socks5", server: "192.0.2.20", port: 1080 } })) })),
  manual_nodes: [
    { id: "office", name: "办公 SOCKS5", groups: [], kind: { Proxy: { raw: { name: "办公 SOCKS5", type: "socks5", server: "192.0.2.30", port: 1080 } } } },
    { id: "home", name: "家里的 Tailscale", groups: [], kind: { Tailscale: { hostname: "example-home" } } },
    { id: "backup", name: "备用 HTTP", groups: [], kind: { Proxy: { raw: { name: "备用 HTTP", type: "http", server: "192.0.2.31", port: 8080 } } } },
  ],
  node_groups: [{ id: "daily", name: "日常使用", members: names.slice(0, 8).map((name) => ({ subscription_id: "s1", name })) }],
  exits: [{ id: "work-exit", name: "工作出口", interface: "ethernet", kind: { Group: { strategy: "Select", members: [{ group: { Subscription: "s1" } }, { group: { Subscription: "s2" } }], selected: { subscription_id: "s1", name: names[1] } } } }],
  listeners: [{ id: "work", name: "工作代理", listener_type: "Mixed", listen: "127.0.0.1", port: 7890, auth: null, exit: "work-exit", user_enabled: true }],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "https://www.gstatic.com/generate_204" },
  process_routing: { enabled: false, on_routa_exit: "Block", rules: [] },
} as unknown as Model;

const delays: Record<string, DelayResult> = {};
names.forEach((name, i) => {
  if (values[i] === undefined) return;
  delays[`示例订阅 ${i < 8 ? "A" : "B"}/${name}`] = { ms: values[i]!, tested_at: completed, error: values[i] === null ? [4, 13].includes(i) ? "Timeout" : "Failed" : null };
});
delays["工作出口"] = { ms: 170, tested_at: completed - 120 };
delays["手动/办公 SOCKS5"] = { ms: 36, tested_at: completed };
delays["手动/家里的 Tailscale"] = { ms: 112, tested_at: completed };
let status = {
  core: "Running", last_apply: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } },
  health: { exits: { "work-exit": { status: "Ok", effective_members: [], missing_members: [], interface: null } }, listeners: { work: "Active" } },
  delays, traffic: null, interfaces: [{ guid: "ethernet", friendly_name: "Ethernet", description: "Example Adapter", is_up: true, ipv4: ["192.0.2.2"], ipv6: [], mihomo_name: "Ethernet" }],
  failed_listeners: [], effective_listeners: ["work"], startup_error: null, save_error: null,
  tailscale: { "手动/家里的 Tailscale": { LoggedIn: { ip: null } } }, tailscale_exit_errors: {}, tailscale_exit_retrying: {}, core_stop_reason: null, core_recovering: false,
} as unknown as RuntimeStatus;
const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity }, mutations: { retry: false } } });
client.setQueryData(["model"], structuredClone(model));
client.setQueryData(["status"], structuredClone(status));
// Prevent all unconfigured commands from accidentally reaching a native bridge.
for (const name of Object.keys(commands)) (commands as any)[name] = async () => { throw new Error("此入口仅用于正式组件验收，使用示例数据。"); };
commands.getModel = async () => structuredClone(model);
commands.getStatus = async () => structuredClone(status);
commands.testDelays = async (targets) => {
  await new Promise((resolve) => setTimeout(resolve, 800));
  const result = targets.map((name) => [name, { ...status.delays[name], ms: status.delays[name]?.ms ?? null, tested_at: Math.floor(Date.now() / 1000), stale: false }] as [string, DelayResult]);
  status = { ...status, delays: { ...status.delays, ...Object.fromEntries(result) } };
  client.setQueryData(["status"], structuredClone(status));
  return { status: "ok", data: result };
};
commands.selectGroupMember = async (_id: string, node: NodeRef) => {
  model = structuredClone(model);
  const kind = model.exits[0].kind;
  if (typeof kind === "object" && "Group" in kind) kind.Group.selected = node;
  status = { ...status, delays: { ...status.delays, "工作出口": { ...status.delays["工作出口"], stale: true } } };
  client.setQueryData(["model"], structuredClone(model));
  client.setQueryData(["status"], structuredClone(status));
  return { status: "ok", data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } };
};

const view = new URLSearchParams(location.search).get("view");
createRoot(document.getElementById("root")!).render(
  <ThemeProvider theme={theme} defaultMode={view === "dark" ? "dark" : "light"} modeStorageKey={`routa-v5-review-color-${view || "light"}`}>
    <CssBaseline />
    <QueryClientProvider client={client}><ApplyResultProvider><MemoryRouter initialEntries={[view === "proxy" ? "/listeners" : "/nodes"]}>
      <Layout><Routes><Route path="/nodes" element={<ResourceCenter />} /><Route path="/listeners" element={<Listeners />} /></Routes></Layout>
    </MemoryRouter></ApplyResultProvider></QueryClientProvider>
    <div style={{ position: "fixed", bottom: 4, right: 12, fontSize: 11, color: "#777", pointerEvents: "none" }}>正式组件验收 · 示例数据</div>
  </ThemeProvider>,
);
