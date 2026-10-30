import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import type {
  CaptureMode,
  DesktopState,
  Model,
  RuntimeStatus,
} from "../bindings";
import { InterfaceProvider } from "../components/InterfaceProvider";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { resetTrafficStore } from "../api/trafficStore";
import { RuntimeSummary } from "./RuntimeSummary";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
    groupNow: vi.fn(),
    coreStart: vi.fn(async () => ({ status: "ok", data: null })),
    coreStop: vi.fn(async () => ({ status: "ok", data: null })),
    coreRestart: vi.fn(async () => ({ status: "ok", data: null })),
    resetTraffic: vi.fn(async () => ({ status: "ok", data: null })),
  },
}));
import { commands } from "../bindings";

const initialModel: Model = {
  subscriptions: [],
  exits: [
    {
      id: "e1",
      name: "默认出口",
      kind: { Node: { node: { manual_id: "n1" } } },
      interface: null,
    },
  ],
  listeners: [
    {
      id: "l1",
      name: "办公",
      listener_type: "Mixed",
      listen: "127.0.0.1",
      port: 7893,
      auth: null,
      exit: "e1",
      user_enabled: true,
    },
  ],
  settings: {
    autostart: false,
    core_path: null,
    controller_port: 19097,
    delay_test_interval_mins: 10,
    delay_test_url: "u",
  },
  manual_nodes: [
    { id: "n1", name: "新加坡01", groups: [], kind: { Proxy: { raw: {} } } },
  ],
  node_groups: [],
  process_routing: { enabled: false, on_routa_exit: "Block", rules: [] },
};
const initialStatus: RuntimeStatus = {
  core: "Running",
  last_apply: null,
  health: {
    exits: {
      e1: {
        status: "Ok",
        effective_members: [{ manual_id: "n1" }],
        missing_members: [],
        interface: null,
      },
    },
    listeners: { l1: "Active" },
  },
  delays: {},
  interfaces: [],
  failed_listeners: [],
  effective_listeners: ["l1"],
  startup_error: null,
  save_error: null,
  tailscale: {},
  tailscale_exit_errors: {},
  tailscale_exit_retrying: {},
  core_stop_reason: null,
  core_recovering: false,
  traffic: {
    total_rate: { up: 1024, down: 2048 },
    per_listener_rate: {},
    totals: { total: { up: 1024, down: 2048 }, per_listener: {} },
  },
};
const initialDesktop: DesktopState = {
  preferences: {
    mode: "Full",
    listener_id: "l1",
    exit_id: "e1",
    system_listener_id: null,
    capture_mode: "System",
    auto_connect: false,
    close_to_tray: true,
    managed_rules: [],
    selected_apps: [],
  },
  connection: null,
  requested: null,
  error: null,
  is_admin: true,
  system_listener_id: null,
};
let model: Model;
let status: RuntimeStatus;
let desktop: DesktopState;
let desktopFailure: Error | null;

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function renderSummary() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const view = render(
    <QueryClientProvider client={client}>
      <ApplyResultProvider>
        <InterfaceProvider>
          <MemoryRouter>
            <RuntimeSummary />
          </MemoryRouter>
        </InterfaceProvider>
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
  return { ...view, client };
}
const capturePanel = () =>
  within(screen.getByRole("region", { name: "系统接管" }));
const settingsPanel = () =>
  within(screen.getByRole("region", { name: "接管设置" }));
async function ready() {
  await waitFor(() =>
    expect(
      screen.getByRole("combobox", { name: "默认代理" }),
    ).not.toHaveAttribute("aria-disabled", "true"),
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  resetTrafficStore();
  model = structuredClone(initialModel);
  status = structuredClone(initialStatus);
  desktop = structuredClone(initialDesktop);
  desktopFailure = null;
  vi.mocked(commands.getModel).mockImplementation(async () =>
    structuredClone(model),
  );
  vi.mocked(commands.getStatus).mockImplementation(async () =>
    structuredClone(status),
  );
  vi.mocked(commands.groupNow).mockResolvedValue({ status: "ok", data: {} });
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "desktop_state") {
      if (desktopFailure) throw desktopFailure;
      return structuredClone(desktop);
    }
    if (command === "set_desktop_connection") {
      const mode = (args as { mode: CaptureMode | null }).mode;
      desktop = { ...desktop, requested: mode, connection: mode, error: null };
      return structuredClone(desktop);
    }
    if (command === "bind_default_listener") {
      desktop.preferences.listener_id = (args as { id: string }).id;
      return structuredClone(desktop.preferences);
    }
    throw new Error(`Unexpected command: ${command}`);
  });
});

it("separates a running core from disabled capture and preserves real statistics", async () => {
  renderSummary();
  await ready();
  expect(
    within(screen.getByRole("region", { name: "内核" })).getByText("运行中"),
  ).toBeInTheDocument();
  expect(capturePanel().getByText("未开启")).toBeInTheDocument();
  expect(
    capturePanel().getByText("系统代理 关闭 · TUN 关闭"),
  ).toBeInTheDocument();
  expect(screen.getByText("1 KB/s")).toBeInTheDocument();
  expect(screen.getByText("2 KB/s")).toBeInTheDocument();
  expect(screen.getByText("3 KB")).toBeInTheDocument();
  expect(screen.getByText("1 / 1")).toBeInTheDocument();
  expect(settingsPanel().getByText("新加坡01")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "管理代理" })).toHaveAttribute(
    "href",
    "/listeners",
  );
  expect(screen.getByRole("link", { name: "故障诊断" })).toHaveAttribute(
    "href",
    "/diagnostics",
  );
  expect(screen.queryByText(/所有应用.*直连/)).not.toBeInTheDocument();
});

it.each(["System", "Tun"] as const)(
  "shows confirmed %s capture with its proxy and selected node",
  async (mode) => {
    desktop.connection = desktop.requested = mode;
    renderSummary();
    expect(
      await capturePanel().findByText(
        mode === "System" ? "系统代理已开启" : "TUN 已开启",
      ),
    ).toBeInTheDocument();
    expect(capturePanel().getByText("办公 · 混合 :7893")).toBeInTheDocument();
    expect(capturePanel().getByText("已选节点：新加坡01")).toBeInTheDocument();
    expect(
      screen.getByRole("button", {
        name: mode === "System" ? "关闭系统代理" : "关闭 TUN",
      }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("combobox", { name: "默认代理" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
  },
);

it("shows the actual system listener when the proxy page overrides the default", async () => {
  model.listeners.push({
    ...model.listeners[0],
    id: "l2",
    name: "下载",
    port: 7894,
  });
  desktop.connection = desktop.requested = "System";
  desktop.system_listener_id = "l2";
  renderSummary();
  expect(
    await capturePanel().findByText("下载 · 混合 :7894"),
  ).toBeInTheDocument();
  expect(screen.getByRole("combobox", { name: "默认代理" })).toHaveTextContent(
    "办公 · 混合 :7893",
  );
});

it("switches capture mutually exclusively and closes the active mode using the real command contract", async () => {
  renderSummary();
  await ready();
  fireEvent.click(screen.getByRole("button", { name: "开启系统代理" }));
  expect(await capturePanel().findByText("系统代理已开启")).toBeInTheDocument();
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "开启 TUN" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "开启 TUN" }));
  expect(await capturePanel().findByText("TUN 已开启")).toBeInTheDocument();
  expect(capturePanel().queryByText("系统代理已开启")).not.toBeInTheDocument();
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "关闭 TUN" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "关闭 TUN" }));
  expect(await capturePanel().findByText("未开启")).toBeInTheDocument();
  expect(
    vi
      .mocked(invoke)
      .mock.calls.filter(([cmd]) => cmd === "set_desktop_connection")
      .map(([, args]) => args),
  ).toEqual([{ mode: "System" }, { mode: "Tun" }, { mode: null }]);
});

it("keeps capture unconfirmed and blocks duplicate operations while enabling", async () => {
  const pending = deferred<DesktopState>();
  const original = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((cmd, args) =>
    cmd === "set_desktop_connection" ? pending.promise : original(cmd, args),
  );
  renderSummary();
  await ready();
  fireEvent.click(screen.getByRole("button", { name: "开启系统代理" }));
  expect(
    await screen.findByRole("button", { name: "正在开启系统代理…" }),
  ).toBeDisabled();
  expect(screen.getByRole("button", { name: "开启 TUN" })).toBeDisabled();
  expect(screen.getByRole("combobox", { name: "默认代理" })).toHaveAttribute(
    "aria-disabled",
    "true",
  );
  expect(capturePanel().queryByText("系统代理已开启")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "正在开启系统代理…" }));
  expect(
    vi
      .mocked(invoke)
      .mock.calls.filter(([cmd]) => cmd === "set_desktop_connection"),
  ).toHaveLength(1);
  desktop.connection = desktop.requested = "System";
  await act(async () => {
    pending.resolve(desktop);
  });
  expect(await capturePanel().findByText("系统代理已开启")).toBeInTheDocument();
});

it("reports an enable failure without displaying enabled capture", async () => {
  const original = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((cmd, args) =>
    cmd === "set_desktop_connection"
      ? Promise.reject(new Error("系统代理设置失败"))
      : original(cmd, args),
  );
  renderSummary();
  await ready();
  fireEvent.click(screen.getByRole("button", { name: "开启系统代理" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "系统代理设置失败",
  );
  expect(capturePanel().getByText("未开启")).toBeInTheDocument();
  expect(capturePanel().queryByText("系统代理已开启")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "开启系统代理" })).toBeEnabled();
});

it("keeps the previous confirmed mode when switching fails and the backend restores it", async () => {
  desktop.connection = desktop.requested = "System";
  const original = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((cmd, args) =>
    cmd === "set_desktop_connection"
      ? Promise.reject(new Error("TUN 网卡未就绪"))
      : original(cmd, args),
  );
  renderSummary();
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "开启 TUN" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "开启 TUN" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("TUN 网卡未就绪");
  expect(capturePanel().getByText("系统代理已开启")).toBeInTheDocument();
  expect(capturePanel().queryByText("TUN 已开启")).not.toBeInTheDocument();
});

it("allows closing an unconfirmed request even if the proxy reference is gone", async () => {
  desktop.requested = "System";
  desktop.error = "默认监听器未能生效";
  model.listeners = [];
  renderSummary();
  expect(await capturePanel().findByText("接管异常")).toBeInTheDocument();
  expect(
    capturePanel().getByText("系统代理 未确认 · TUN 关闭"),
  ).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "关闭系统代理" })).toBeEnabled();
  fireEvent.click(screen.getByRole("button", { name: "关闭系统代理" }));
  expect(await capturePanel().findByText("未开启")).toBeInTheDocument();
  expect(invoke).toHaveBeenCalledWith("set_desktop_connection", { mode: null });
});

it("does not mistake a saved capture preference for active capture", async () => {
  desktop.preferences.capture_mode = "Tun";
  renderSummary();
  await ready();
  expect(capturePanel().getByText("未开启")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "开启 TUN" })).toHaveAttribute(
    "aria-pressed",
    "false",
  );
});

it("shows missing defaults and binds another proxy before allowing capture", async () => {
  desktop.preferences.listener_id = "deleted";
  renderSummary();
  await ready();
  const select = screen.getByRole("combobox", { name: "默认代理" });
  expect(select).toHaveTextContent("默认代理已不存在");
  expect(screen.getByRole("button", { name: "开启系统代理" })).toBeDisabled();
  fireEvent.mouseDown(select);
  fireEvent.click(
    await screen.findByRole("option", { name: "办公 · 混合 :7893" }),
  );
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "开启系统代理" })).toBeEnabled(),
  );
  expect(invoke).toHaveBeenCalledWith("bind_default_listener", { id: "l1" });
  expect(settingsPanel().getByText("新加坡01")).toBeInTheDocument();
});

it.each(["node", "exit", "empty"])(
  "handles a missing %s without inventing a selected node",
  async (missing) => {
    if (missing === "node") model.manual_nodes = [];
    if (missing === "exit") model.exits = [];
    if (missing === "empty") {
      model.listeners = [];
      desktop.preferences.listener_id = null;
    }
    renderSummary();
    await ready();
    expect(
      settingsPanel().getByText(
        missing === "node"
          ? "（已删除）"
          : missing === "exit"
            ? "出口已不存在"
            : "尚未选择默认代理",
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "开启系统代理" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "开启 TUN" })).toBeDisabled();
  },
);

it("retains the status panels and settings while APIs are loading", async () => {
  vi.mocked(commands.getModel).mockImplementation(() => new Promise(() => {}));
  vi.mocked(commands.getStatus).mockImplementation(() => new Promise(() => {}));
  vi.mocked(invoke).mockImplementation(() => new Promise(() => {}));
  renderSummary();
  expect(
    within(screen.getByRole("region", { name: "内核" })).getByText("读取中…"),
  ).toBeInTheDocument();
  expect(capturePanel().getByText("读取中…")).toBeInTheDocument();
  expect(
    settingsPanel().getByRole("combobox", { name: "默认代理" }),
  ).toHaveAttribute("aria-disabled", "true");
  expect(screen.getByRole("button", { name: "开启系统代理" })).toBeDisabled();
  expect(screen.queryByText("0 B/s")).not.toBeInTheDocument();
});

it("shows desktop failures as unknown and can retry", async () => {
  desktopFailure = new Error("无法读取系统接管");
  renderSummary();
  expect(await capturePanel().findByText("状态未知")).toBeInTheDocument();
  expect(capturePanel().queryByText("未开启")).not.toBeInTheDocument();
  expect(screen.getByRole("alert")).toHaveTextContent("无法读取系统接管");
  desktopFailure = null;
  fireEvent.click(screen.getByRole("button", { name: "重新读取" }));
  expect(await capturePanel().findByText("未开启")).toBeInTheDocument();
});

it("does not present stale capture as confirmed after a background state failure", async () => {
  desktop.connection = desktop.requested = "System";
  const { client } = renderSummary();
  expect(await capturePanel().findByText("系统代理已开启")).toBeInTheDocument();
  desktopFailure = new Error("状态接口断开");
  await act(async () => {
    await client.invalidateQueries({ queryKey: ["desktop"] });
  });
  expect(await capturePanel().findByText("状态未知")).toBeInTheDocument();
  expect(capturePanel().queryByText("系统代理已开启")).not.toBeInTheDocument();
});

it("does not confirm capture when the core state cannot be read", async () => {
  desktop.connection = desktop.requested = "System";
  vi.mocked(commands.getStatus).mockRejectedValue(
    new Error("内核状态读取中断"),
  );
  renderSummary();
  expect(await capturePanel().findByText("状态未知")).toBeInTheDocument();
  expect(capturePanel().queryByText("系统代理已开启")).not.toBeInTheDocument();
  expect(screen.getByRole("alert")).toHaveTextContent("内核状态读取中断");
  expect(screen.getByRole("button", { name: "关闭系统代理" })).toBeEnabled();
});

it("distinguishes app capture from system proxy and TUN", async () => {
  desktop.connection = desktop.requested = "Apps";
  renderSummary();
  expect(await capturePanel().findByText("未开启")).toBeInTheDocument();
  expect(capturePanel().getByText("指定应用代理已开启")).toBeInTheDocument();
  expect(
    capturePanel().getByText("系统代理 关闭 · TUN 关闭"),
  ).toBeInTheDocument();
});

it("requires administrator permission to enable TUN but still allows closing an active TUN", async () => {
  desktop.is_admin = false;
  const { client } = renderSummary();
  await ready();
  expect(screen.getByRole("button", { name: "开启 TUN" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "开启系统代理" })).toBeEnabled();
  desktop.connection = desktop.requested = "Tun";
  await act(async () => {
    await client.invalidateQueries({ queryKey: ["desktop"] });
  });
  expect(await screen.findByRole("button", { name: "关闭 TUN" })).toBeEnabled();
});

it("does not allow a SOCKS proxy to be used as a Windows system proxy", async () => {
  model.listeners[0].listener_type = "Socks";
  renderSummary();
  await ready();
  expect(screen.getByRole("button", { name: "开启系统代理" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "开启 TUN" })).toBeEnabled();
});

it("reads a group's actual current node rather than assuming its saved selection is active", async () => {
  model.exits[0].kind = {
    Group: {
      strategy: "Select",
      members: [{ manual_id: "n1" }],
      selected: { manual_id: "n1" },
    },
  };
  desktop.connection = desktop.requested = "System";
  vi.mocked(commands.groupNow).mockResolvedValue({
    status: "ok",
    data: { 默认出口: "默认出口/机场A/香港02" },
  });
  renderSummary();
  expect(
    await capturePanel().findByText("已选节点：机场A/香港02"),
  ).toBeInTheDocument();
  expect(commands.groupNow).toHaveBeenCalledWith(["默认出口"]);
  expect(settingsPanel().getByText("· 手动选择")).toBeInTheDocument();
});

it("shows a group lookup failure instead of its stale saved node", async () => {
  model.exits[0].kind = {
    Group: {
      strategy: "UrlTest",
      members: [{ manual_id: "n1" }],
      selected: null,
    },
  };
  desktop.connection = desktop.requested = "Tun";
  vi.mocked(commands.groupNow).mockRejectedValue(new Error("core unreachable"));
  renderSummary();
  expect(
    await capturePanel().findByText("已选节点：当前节点读取失败"),
  ).toBeInTheDocument();
});

it("does not claim that a newly saved fixed node is active after a rejected apply", async () => {
  desktop.connection = desktop.requested = "System";
  status.last_apply = { FailedKeptPrevious: { error: "配置被拒绝" } };
  renderSummary();
  expect(
    await capturePanel().findByText("已选节点：当前节点未确认"),
  ).toBeInTheDocument();
  expect(settingsPanel().getByText("新加坡01")).toBeInTheDocument();
  expect(screen.getByRole("alert")).toHaveTextContent("未生效");
});

it("locks capture during a default-proxy save and keeps the old selection on failure", async () => {
  model.listeners.push({
    ...model.listeners[0],
    id: "l2",
    name: "下载",
    port: 7894,
  });
  const pending = deferred<DesktopState["preferences"]>();
  const original = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((cmd, args) =>
    cmd === "bind_default_listener" ? pending.promise : original(cmd, args),
  );
  renderSummary();
  await ready();
  fireEvent.mouseDown(screen.getByRole("combobox", { name: "默认代理" }));
  fireEvent.click(
    await screen.findByRole("option", { name: "下载 · 混合 :7894" }),
  );
  expect(await screen.findByRole("status")).toHaveTextContent(
    "正在保存默认代理",
  );
  expect(screen.getByRole("button", { name: "开启系统代理" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "开启 TUN" })).toBeDisabled();
  await act(async () => {
    pending.reject(new Error("默认代理保存失败"));
  });
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "默认代理保存失败",
  );
  expect(screen.getByRole("combobox", { name: "默认代理" })).toHaveTextContent(
    "办公 · 混合 :7893",
  );
  expect(screen.getByRole("button", { name: "开启系统代理" })).toBeEnabled();
});

it("keeps the previous traffic total without displaying stale rates after a core crash", async () => {
  status.core = { Crashed: { exit_code: 7 } };
  renderSummary();
  await ready();
  expect(screen.queryByText("1 KB/s")).not.toBeInTheDocument();
  expect(screen.queryByText("2 KB/s")).not.toBeInTheDocument();
  expect(screen.getByText("3 KB")).toBeInTheDocument();
});

it("preserves kernel-control failure feedback", async () => {
  vi.mocked(commands.coreStop).mockRejectedValueOnce(new Error("内核停止失败"));
  renderSummary();
  await ready();
  fireEvent.click(screen.getByRole("button", { name: "停止" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("内核停止失败");
  expect(
    within(screen.getByRole("region", { name: "内核" })).getByText("运行中"),
  ).toBeInTheDocument();
});

it.each(["Stopped", "Starting", { Crashed: { exit_code: 7 } }] as const)(
  "preserves the actual core state %j and available controls",
  async (coreState) => {
    status.core = coreState;
    status.effective_listeners = [];
    renderSummary();
    const panel = within(screen.getByRole("region", { name: "内核" }));
    expect(
      await panel.findByText(
        coreState === "Stopped"
          ? "已停止"
          : coreState === "Starting"
            ? "启动中"
            : "已崩溃",
      ),
    ).toBeInTheDocument();
    if (coreState === "Starting")
      expect(
        screen.getByRole("button", { name: "开启系统代理" }),
      ).toBeDisabled();
    else {
      fireEvent.click(screen.getByRole("button", { name: "启动" }));
      await waitFor(() => expect(commands.coreStart).toHaveBeenCalled());
    }
    if (typeof coreState === "object")
      expect(panel.getByText("退出码 7")).toBeInTheDocument();
  },
);

it("preserves traffic reset confirmation", async () => {
  renderSummary();
  await ready();
  fireEvent.click(screen.getByRole("button", { name: "清零" }));
  const dialog = await screen.findByRole("dialog");
  expect(commands.resetTraffic).not.toHaveBeenCalled();
  fireEvent.click(within(dialog).getByRole("button", { name: "清零" }));
  await waitFor(() => expect(commands.resetTraffic).toHaveBeenCalled());
});
