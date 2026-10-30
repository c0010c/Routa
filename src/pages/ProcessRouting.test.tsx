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
    procrouteRetry: vi.fn(),
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
  const view = render(
    <QueryClientProvider client={qc}>
      <ApplyResultProvider>
        <MemoryRouter>
          <ProcessRoutingPage />
        </MemoryRouter>
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
  return { ...view, qc };
}

describe("ProcessRoutingPage", () => {
  beforeEach(() => localStorage.clear());
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
    expect(await screen.findByRole("button", { name: "更新分流服务" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "启用进程分流" })).not.toBeInTheDocument();
  });

  it("failed update keeps a retryable button and reports the actual error", async () => {
    let finish: (result: { status: "error"; error: string }) => void = () => {};
    vi.mocked(commands.procrouteInstall).mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
    renderPage({ ...base, component: "NeedsUpdate" });
    const button = await screen.findByRole("button", { name: "更新分流服务" });
    fireEvent.click(button);
    await waitFor(() => expect(button).toBeDisabled());
    act(() => finish({ status: "error", error: "旧服务进程未退出，请稍后重试" }));
    expect(await screen.findByText("旧服务进程未退出，请稍后重试")).toBeInTheDocument();
    await waitFor(() => expect(button).toBeEnabled());
    vi.mocked(commands.procrouteInstall).mockResolvedValueOnce({ status: "ok", data: null });
    fireEvent.click(button);
    await waitFor(() => expect(button).toBeEnabled());
  });

  it.each(["Stopped", { Error: { message: "服务启动失败" } }] as const)("offers service repair after a failed installation: %s", async (component) => {
    renderPage({ ...base, component });
    expect(await screen.findByRole("button", { name: "修复分流服务" })).toBeInTheDocument();
  });

  it("shows the error of a failed component and apply errors", async () => {
    renderPage({ ...base, component: { Error: { message: "管道无响应" } }, apply_errors: ["规则 curl.exe 下发失败"] });
    expect(await screen.findByText("错误：管道无响应")).toBeInTheDocument();
    expect(screen.getByText("规则 curl.exe 下发失败")).toBeInTheDocument();
  });

  it("keeps a dismissed interruption hidden after status updates and remounting", async () => {
    const status = { ...base, last_interruption: { at_unix: 1793247873, reason: "Routa 意外退出" } };
    const { qc, unmount } = renderPage(status);
    expect(await screen.findByText(/最近一次中断：/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "关闭中断提示" }));
    expect(screen.queryByText(/最近一次中断：/)).not.toBeInTheDocument();

    act(() => qc.setQueryData(["procroute"], { ...status, conflicts: ["组件冲突"] }));
    expect(await screen.findByText("组件冲突")).toBeInTheDocument();
    expect(screen.queryByText(/最近一次中断：/)).not.toBeInTheDocument();

    unmount();
    renderPage(status);
    await screen.findByText("分流组件");
    expect(screen.queryByText(/最近一次中断：/)).not.toBeInTheDocument();
  });

  it.each([
    { at_unix: 1793247874, reason: "Routa 意外退出" },
    { at_unix: 1793247873, reason: "分流服务意外退出后已重启" },
  ])("shows a new interruption after dismissing the previous one: %j", async (interruption) => {
    const { qc } = renderPage({ ...base, last_interruption: { at_unix: 1793247873, reason: "Routa 意外退出" } });
    fireEvent.click(await screen.findByRole("button", { name: "关闭中断提示" }));

    act(() => qc.setQueryData(["procroute"], { ...base, last_interruption: interruption }));
    expect(await screen.findByText(/最近一次中断：/)).toHaveTextContent(interruption.reason);
    expect(screen.getByRole("button", { name: "关闭中断提示" })).toBeInTheDocument();
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

  it.each([
    ["Unknown", "状态未知"], ["Pending", "应用中"], ["Failed", "应用失败"],
  ])("shows unconfirmed state %s without claiming success", async (action, label) => {
    renderPage({ ...base, actions: { r1: { action, reason: null, up_bps: 0, down_bps: 0 } } });
    expect(await screen.findByText(label)).toBeInTheDocument();
    expect(screen.queryByText("已代理")).not.toBeInTheDocument();
    expect(screen.queryByText("已阻断")).not.toBeInTheDocument();
  });

  it("missing service confirmation shows unknown", async () => {
    renderPage({ ...base, component: "NotInstalled" });
    expect(await screen.findByText("状态未知")).toBeInTheDocument();
    expect(screen.queryByText("已阻断")).not.toBeInTheDocument();
  });

  it("only retries after clicking and preserves failures until confirmed success", async () => {
    vi.mocked(commands.procrouteRetry).mockClear();
    vi.mocked(commands.procrouteRetry).mockResolvedValue({ status: "error", error: "仍然失败" });
    const failed = { ...base, apply_errors: ["代理应用失败"] };
    const { qc } = renderPage(failed);
    const button = await screen.findByRole("button", { name: "重试" });
    act(() => qc.setQueryData(["procroute"], { ...failed, conflicts: ["状态刷新"] }));
    expect(commands.procrouteRetry).not.toHaveBeenCalled();
    fireEvent.click(button);
    expect(await screen.findByText("仍然失败")).toBeInTheDocument();
    expect(screen.getByText("代理应用失败")).toBeInTheDocument();
    expect(commands.procrouteRetry).toHaveBeenCalledTimes(1);
    vi.mocked(commands.procrouteRetry).mockResolvedValue({ status: "ok", data: null });
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    await waitFor(() => expect(commands.procrouteRetry).toHaveBeenCalledTimes(2));
    act(() => qc.setQueryData(["procroute"], { ...base, actions: { r1: { action: "Proxy", reason: null, up_bps: 0, down_bps: 0 } } }));
    expect(await screen.findByText("已代理")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "重试" })).not.toBeInTheDocument();
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
