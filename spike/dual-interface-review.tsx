// Development-only fixture harness. Not imported by the product entry or build.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { createRoot } from "react-dom/client";
import type { Model, RuntimeStatus, ProcRouteStatus } from "../src/bindings";
import type { DesktopState } from "../src/api/desktop";

const params = new URLSearchParams(location.search);
const mode = params.get("mode");
const empty = params.get("empty") === "1";
const m: Model = {
  settings: {
    autostart: false,
    core_path: null,
    controller_port: 19097,
    delay_test_url: "https://example.invalid",
    delay_test_interval_mins: 10,
  },
  subscriptions: [],
  node_groups: [],
  manual_nodes: empty
    ? []
    : [
        {
          id: "node1",
          name: "示例节点 · 香港",
          groups: [],
          kind: {
            Proxy: {
              raw: {
                name: "示例节点 · 香港",
                type: "socks5",
                server: "example.invalid",
                port: 1080,
              },
            },
          },
        },
      ],
  exits: empty
    ? []
    : [
        {
          id: "e1",
          name: "默认代理",
          interface: null,
          kind: { Node: { node: { manual_id: "node1" } } },
        },
      ],
  listeners: [
    {
      id: "l1",
      name: "默认监听器",
      listener_type: "Mixed",
      listen: "127.0.0.1",
      port: 7881,
      auth: null,
      exit: empty ? null : "e1",
      user_enabled: true,
    },
    {
      id: "l2",
      name: "下载",
      listener_type: "Socks",
      listen: "127.0.0.1",
      port: 7882,
      auth: null,
      exit: empty ? null : "e1",
      user_enabled: true,
    },
  ],
  process_routing: {
    enabled: true,
    on_routa_exit: "Block",
    rules: [
      {
        id: "r1",
        exe_path: "C:\\Apps\\Chrome\\chrome.exe",
        listener: "l1",
        on_fault: "Block",
        user_enabled: true,
        reset_on_change: false,
      },
      {
        id: "r2",
        exe_path: "C:\\Apps\\Telegram\\Telegram.exe",
        listener: "l1",
        on_fault: "Block",
        user_enabled: false,
        reset_on_change: false,
      },
    ],
  },
};
const desktop: DesktopState = {
  preferences: {
    mode: mode === "Welcome" ? null : mode === "Full" ? "Full" : "Simple",
    listener_id: "l1",
    exit_id: "e1",
    system_listener_id: null,
    capture_mode: "System",
    auto_connect: false,
    close_to_tray: true,
    managed_rules: ["r1", "r2"],
    selected_apps: ["r1"],
  },
  connection: null,
  requested: null,
  error: null,
  is_admin: false,
  system_listener_id: null,
};
const status: RuntimeStatus = {
  core: "Running",
  last_apply: null,
  health: {
    exits: {
      e1: {
        status: "Ok",
        effective_members: [],
        missing_members: [],
        interface: null,
      },
    },
    listeners: {
      l1: empty ? "NoExit" : "Active",
      l2: empty ? "NoExit" : "Active",
    },
  },
  delays: {},
  traffic: null,
  interfaces: [],
  failed_listeners: [],
  effective_listeners: empty ? [] : ["l1", "l2"],
  startup_error: null,
  save_error: null,
  tailscale: {},
  tailscale_exit_errors: {},
  tailscale_exit_retrying: {},
  core_stop_reason: null,
  core_recovering: false,
};
const route: ProcRouteStatus = {
  component: "Running",
  actions: {},
  last_interruption: null,
  conflicts: [],
  apply_errors: [],
  truncated_flows: false,
};
const copy = <T,>(v: T) => structuredClone(v);
mockWindows("main");
mockIPC(
  async (cmd, args) => {
    if (cmd === "desktop_state") return copy(desktop);
    if (cmd === "get_model") return copy(m);
    if (cmd === "get_status") return copy(status);
    if (cmd === "procroute_status") return copy(route);
    if (cmd === "group_now") return {};
    if (cmd === "save_ui_preferences") {
      desktop.preferences = {
        ...desktop.preferences,
        ...(args?.input as DesktopState["preferences"]),
      };
      return copy(desktop.preferences);
    }
    if (cmd === "change_process_listener") {
      const r = m.process_routing.rules.find((r) => r.id === args?.id);
      if (r) r.listener = String(args?.listener);
      return;
    }
    if (cmd === "get_connection_details") return [];
    if (cmd === "read_core_log")
      return "本地界面验收环境：未启动真实内核或修改系统代理。";
    if (cmd === "list_running_processes") return [];
    if (cmd === "set_desktop_connection")
      throw "本地界面验收环境不接管系统网络";
    if (cmd.startsWith("plugin:")) return null;
    throw `验收环境未提供命令：${cmd}`;
  },
  { shouldMockEvents: true },
);
const { default: App } = await import("../src/App");
createRoot(document.getElementById("root")!).render(<App />);
