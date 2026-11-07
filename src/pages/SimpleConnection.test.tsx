import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import type { DesktopState, Model, ProcRouteStatus, RuntimeStatus } from "../bindings";
import App from "../App";

vi.mock("../api/events", () => ({ useRuntimeEvents: () => {} }));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => false, invoke: vi.fn() }));

const model: Model = {
  subscriptions: [],
  exits: [{ id: "e1", name: "默认", interface: null, kind: { Node: { node: { manual_id: "n1" } } } }],
  listeners: [{ id: "l1", name: "默认", listener_type: "Mixed", listen: "127.0.0.1", port: 7881, auth: null, exit: "e1", user_enabled: true }],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: null, delay_test_url: "u" },
  manual_nodes: [{ id: "n1", name: "香港 01", groups: [], kind: { Proxy: { raw: {} } } }],
  node_groups: [],
  process_routing: { enabled: false, rules: [], exit_prompt: true },
};
const status: RuntimeStatus = {
  core: "Running", last_apply: null, health: { exits: {}, listeners: { l1: "Active" } }, delays: {}, traffic: null,
  interfaces: [], failed_listeners: [], effective_listeners: ["l1"], startup_error: null, save_error: null,
  tailscale: {}, tailscale_exit_errors: {}, tailscale_exit_retrying: {}, core_stop_reason: null, core_recovering: false,
};
const route: ProcRouteStatus = { component: "Running", actions: {}, last_interruption: null, conflicts: [], apply_errors: [], truncated_flows: false };

vi.mock("../bindings", () => ({
  commands: new Proxy(
    {},
    {
      get: (_, name: string) => {
        if (name === "getModel") return async () => structuredClone(model);
        if (name === "getStatus") return async () => structuredClone(status);
        if (name === "procrouteStatus") return async () => structuredClone(route);
        if (name === "updateStatus") return async () => ({ current_version: "0", state: { kind: "Unsupported" }, last_checked_unix: null });
        if (name === "listRunningProcesses") return async () => [];
        return async () => ({ status: "ok", data: null });
      },
    },
  ),
}));

let desktop: DesktopState;
let calls: { command: string; args: unknown }[];
beforeEach(() => {
  calls = [];
  desktop = {
    preferences: {
      mode: "Simple", listener_id: "l1", exit_id: "e1", system_listener_id: null, capture_mode: "System",
      auto_connect: false, close_to_tray: true, taskbar_status: { enabled: false, mode: "Rotate", traffic: "Values" },
      managed_rules: [], selected_apps: [],
    },
    connection: null, requested: null, error: null, is_admin: false, system_listener_id: null, taskbar_status_supported: true,
  };
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    calls.push({ command, args });
    if (command === "desktop_state") return structuredClone(desktop);
    if (command === "procroute_status") return structuredClone(route);
    if (command === "save_ui_preferences") {
      desktop.preferences = (args as { input: DesktopState["preferences"] }).input;
      return structuredClone(desktop.preferences);
    }
    if (command === "relaunch_as_admin") throw "已取消";
    return null;
  });
  window.location.hash = "#/";
});

it("offers system proxy, chosen apps and TUN as one choice", async () => {
  render(<App />);
  const system = await screen.findByRole("button", { name: "系统代理" });
  const scope = system.closest('[role="group"]') as HTMLElement;
  expect(within(scope).getAllByRole("button").map((b) => b.textContent)).toEqual(["系统代理", "指定应用", "TUN"]);
  expect(system).toHaveAttribute("aria-pressed", "true");
  fireEvent.click(screen.getByRole("button", { name: /连接选项/ }));
  // The second System/TUN switch inside the options is gone.
  expect(screen.getAllByRole("button", { name: "系统代理" })).toHaveLength(1);
  expect(screen.queryByRole("button", { name: "TUN（需要管理员权限）" })).not.toBeInTheDocument();
});

it("asks for an elevated restart when TUN is picked without admin rights", async () => {
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "TUN" }));
  const dialog = await screen.findByRole("dialog", { name: "TUN 需要管理员权限" });
  fireEvent.click(within(dialog).getByRole("button", { name: "以管理员身份重启" }));
  expect(await within(dialog).findByText("已取消授权，保持当前连接方式。")).toBeInTheDocument();
  expect(calls.find((c) => c.command === "relaunch_as_admin")?.args).toEqual({ connectTun: true });
  expect(calls.some((c) => c.command === "set_desktop_connection" || c.command === "save_ui_preferences")).toBe(false);
});

it("switches straight to TUN when already running as administrator", async () => {
  desktop.is_admin = true;
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "TUN" }));
  await waitFor(() =>
    expect(calls.find((c) => c.command === "save_ui_preferences")?.args).toEqual({
      input: expect.objectContaining({ capture_mode: "Tun" }),
    }),
  );
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});

it("lets the user get back from the nodes and subscriptions pages", async () => {
  render(<App />);
  const nav = await screen.findByRole("navigation", { name: "简洁导航" });
  expect(within(nav).getAllByRole("link").map((l) => l.textContent)).toEqual(["连接", "订阅与节点", "设置"]);
  expect(within(nav).getByRole("link", { name: "连接" })).toHaveAttribute("aria-current", "page");

  fireEvent.click(within(nav).getByRole("link", { name: "订阅与节点" }));
  expect(await screen.findByRole("heading", { name: "节点" })).toBeInTheDocument();
  expect(within(nav).getByRole("link", { name: "订阅与节点" })).toHaveAttribute("aria-current", "page");

  fireEvent.click(screen.getByRole("link", { name: "订阅" }));
  expect(await screen.findByRole("heading", { name: "订阅" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("link", { name: "返回节点" }));
  expect(await screen.findByRole("heading", { name: "节点" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("link", { name: "返回连接" }));
  expect(await screen.findByText("当前节点")).toBeInTheDocument();
});

it("hints when an app rule uses another node than the simple node", async () => {
  model.process_routing.rules = [
    { id: "r1", name: "浏览器", processes: [{ kind: "Name", value: "chrome.exe" }], learned: [], node: { manual_id: "other" }, on_fault: "Direct", reset_on_change: false, user_enabled: true },
  ];
  desktop.preferences.capture_mode = "Apps";
  desktop.preferences.managed_rules = ["r1"];
  desktop.preferences.selected_apps = ["r1"];
  try {
    render(<App />);
    expect(await screen.findByText("使用其他节点，关闭并重新勾选后使用当前节点")).toBeInTheDocument();
  } finally {
    model.process_routing.rules = [];
  }
});
