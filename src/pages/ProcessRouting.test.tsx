import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { Model, ProcRouteStatus } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
    procrouteStatus: vi.fn(),
    procrouteInstall: vi.fn(),
    procrouteUninstall: vi.fn(),
    procrouteCleanup: vi.fn(),
    saveProcessRule: vi.fn(),
    deleteProcessRule: vi.fn(),
    saveProcessRoutingSettings: vi.fn(),
    listRunningProcesses: vi.fn(),
  },
}));

import { resetFlowsStore, setFlows } from "../api/procroute";
import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { ProcessRoutingPage } from "./ProcessRouting";

const model: Model = {
  subscriptions: [],
  exits: [],
  listeners: [
    { id: "l1", name: "浏览器", listener_type: "Mixed", listen: "0.0.0.0", port: 7891, auth: null, exit: null, user_enabled: true },
  ],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [],
  node_groups: [],
  process_routing: {
    enabled: true,
    on_routa_exit: "Block",
    rules: [
      { id: "r1", exe_path: "C:\\Apps\\curl.exe", listener: "l1", on_fault: "Block", reset_on_change: false, user_enabled: true },
    ],
  },
};

const base: ProcRouteStatus = {
  component: "Running",
  actions: {},
  last_interruption: null,
  conflicts: [],
  apply_errors: [],
  truncated_flows: false,
};

function renderPage(status: ProcRouteStatus, m: Model = model) {
  vi.mocked(commands.getModel).mockResolvedValue(m);
  vi.mocked(commands.procrouteStatus).mockResolvedValue(status);
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <ApplyResultProvider>
        <MemoryRouter>
          <ProcessRoutingPage />
        </MemoryRouter>
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
}

describe("ProcessRoutingPage", () => {
  afterEach(() => resetFlowsStore());

  it("not installed shows enable button", async () => {
    vi.mocked(commands.procrouteInstall).mockResolvedValue({ status: "ok", data: null });
    renderPage({ ...base, component: "NotInstalled" });
    const btn = await screen.findByRole("button", { name: "启用进程分流" });
    expect(screen.getByText("未安装")).toBeInTheDocument();
    fireEvent.click(btn);
    await waitFor(() => expect(commands.procrouteInstall).toHaveBeenCalled());
  });

  it("needs update shows update button", async () => {
    renderPage({ ...base, component: "NeedsUpdate" });
    expect(await screen.findByRole("button", { name: "更新组件" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "启用进程分流" })).not.toBeInTheDocument();
  });

  it("shows the error of a failed component and apply errors", async () => {
    renderPage({ ...base, component: { Error: { message: "管道无响应" } }, apply_errors: ["规则 curl.exe 下发失败"] });
    expect(await screen.findByText("错误：管道无响应")).toBeInTheDocument();
    expect(screen.getByText("规则 curl.exe 下发失败")).toBeInTheDocument();
  });

  it("blocked rule shows reason chip", async () => {
    renderPage({
      ...base,
      actions: { r1: { action: "Block", reason: "Listener 已停用", up_bps: 0, down_bps: 0 } },
    });
    expect(await screen.findByText("已阻断")).toBeInTheDocument();
    expect(screen.getByText("Listener 已停用")).toBeInTheDocument();
    expect(screen.getByText("curl.exe")).toBeInTheDocument();
  });

  it("drawer shows truncation hint", async () => {
    renderPage({
      ...base,
      truncated_flows: true,
      actions: { r1: { action: "Proxy", reason: null, up_bps: 2048, down_bps: 0 } },
    });
    fireEvent.click(await screen.findByText("curl.exe"));
    act(() =>
      setFlows([
        { pid: 7, rule_id: "r1", exe_path: "C:\\Apps\\curl.exe", host: "example.com", port: 443, udp: false, age_ms: 65000, sent: 10, received: 2048 },
        { pid: 8, rule_id: "r2", exe_path: "C:\\x.exe", host: "other.org", port: 80, udp: true, age_ms: null, sent: 0, received: 0 },
      ]),
    );
    const drawer = await screen.findByRole("presentation");
    expect(within(drawer).getByText("仅显示最近 400 条")).toBeInTheDocument();
    expect(within(drawer).getByText("example.com")).toBeInTheDocument();
    expect(within(drawer).queryByText("other.org")).not.toBeInTheDocument();
  });

  it("surfaces a cancelled cleanup in the snackbar", async () => {
    vi.mocked(commands.procrouteCleanup).mockResolvedValue({ status: "error", error: "已取消" });
    renderPage(base);
    fireEvent.click(await screen.findByRole("button", { name: "更多组件操作" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "清理残留阻断规则" }));
    fireEvent.click(await screen.findByRole("button", { name: "清理" }));
    expect(await screen.findByText("已取消")).toBeInTheDocument();
  });
});
