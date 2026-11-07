import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import type { Model, ProcessEntry, ProcessRule, ProcRouteStatus } from "../bindings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

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
    removeLearnedProcessPath: vi.fn(),
    listRunningProcesses: vi.fn(),
  },
}));

import { resetFlowsStore, setFlows } from "../api/procroute";
import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { ProcessRoutingPage } from "./ProcessRouting";

const N = (value: string): ProcessEntry => ({ kind: "Name", value });
const P = (value: string): ProcessEntry => ({ kind: "Path", value });

const model: Model = {
  subscriptions: [],
  exits: [],
  listeners: [
    { id: "l1", name: "浏览器", listener_type: "Mixed", listen: "0.0.0.0", port: 7891, auth: null, exit: null, user_enabled: true },
  ],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [{ id: "m1", name: "东京", kind: { Proxy: { raw: "ss://x" } }, groups: [] }],
  node_groups: [],
  process_routing: {
    enabled: true,
    rules: [
      { id: "r1", name: "", processes: [N("curl.exe")], learned: [], node: { manual_id: "m1" }, on_fault: "Direct", reset_on_change: false, user_enabled: true },
      {
        id: "r2",
        name: "AI 工具",
        processes: [N("ChatGPT.exe"), N("codex.exe"), N("Claude.exe"), N("node.exe"), P("C:\\Tools\\git.exe")],
        learned: [],
        node: { manual_id: "m1" },
        on_fault: "Direct",
        reset_on_change: false,
        user_enabled: true,
      },
    ],
    exit_prompt: true,
  },
};

const withRules = (...rules: ProcessRule[]): Model => ({ ...model, process_routing: { ...model.process_routing, rules } });

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
  beforeEach(() => {
    localStorage.clear();
    vi.mocked(invoke).mockReset();
  });
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
    await screen.findByText("运行中");
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
    // The unnamed rule is labelled by its process, which is also listed as a chip.
    expect(screen.getAllByText("curl.exe")).toHaveLength(2);
  });

  it.each([
    ["Unknown", "状态未知"], ["Pending", "应用中"], ["Failed", "应用失败"],
  ])("shows unconfirmed state %s without claiming success", async (action, label) => {
    renderPage({ ...base, actions: { r1: { action, reason: null, up_bps: 0, down_bps: 0 } } });
    const row = (await screen.findAllByText("curl.exe"))[0].closest("tr")!;
    expect(within(row).getByText(label)).toBeInTheDocument();
    expect(screen.queryByText("已代理")).not.toBeInTheDocument();
    expect(screen.queryByText("已阻断")).not.toBeInTheDocument();
  });

  it("missing service confirmation shows unknown", async () => {
    renderPage({ ...base, component: "NotInstalled" });
    expect(await screen.findAllByText("状态未知")).toHaveLength(2);
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
    fireEvent.click((await screen.findAllByText("curl.exe"))[0]);
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

  it("lists each rule with its processes and folds the rest", async () => {
    renderPage(base);
    const row = (await screen.findByText("AI 工具")).closest("tr")!;
    for (const name of ["ChatGPT.exe", "codex.exe", "Claude.exe", "node.exe"]) {
      expect(within(row).getByText(name)).toBeInTheDocument();
    }
    expect(within(row).getByText("+1")).toBeInTheDocument();
    expect(within(row).queryByText("git.exe")).not.toBeInTheDocument();
  });

  it("labels a program's entries with one chip", async () => {
    const chrome: ProcessRule = {
      ...model.process_routing.rules[0],
      id: "r3",
      name: "浏览器",
      processes: [N("chrome.exe"), P("C:\\Chrome\\chrome.exe")],
    };
    renderPage(base, withRules(chrome));
    const row = (await screen.findByText("浏览器")).closest("tr")!;
    expect(within(row).getByText("chrome.exe + 1 个位置")).toBeInTheDocument();
  });

  it("tags blocking rules and lists learned locations with removal", async () => {
    const A = "C:\\A\\curl.exe";
    vi.mocked(commands.removeLearnedProcessPath).mockResolvedValue({ status: "ok", data: null });
    const learning: ProcessRule = {
      ...model.process_routing.rules[0],
      on_fault: "Block",
      learned: [A, "C:\\B\\curl.exe"],
    };
    renderPage(base, withRules(learning));
    const row = (await screen.findAllByText("curl.exe"))[0].closest("tr")!;
    expect(within(row).getByText("不可用时阻断")).toBeInTheDocument();
    fireEvent.click(within(row).getByRole("button", { name: /已记录 2 个位置/ }));
    // Opening the list doesn't open the connections drawer.
    expect(screen.queryByRole("presentation")).not.toBeInTheDocument();
    fireEvent.click(within(row).getByRole("button", { name: `移除 ${A}` }));
    await waitFor(() => expect(commands.removeLearnedProcessPath).toHaveBeenCalledWith("r1", A));
  });

  it("searches entry values", async () => {
    renderPage(base);
    const search = await screen.findByLabelText("搜索规则名称或进程");
    fireEvent.change(search, { target: { value: "c:\\tools" } });
    expect(screen.getByText("AI 工具")).toBeInTheDocument();
    expect(screen.queryByText("curl.exe")).not.toBeInTheDocument();
  });

  it("searches rule names and processes", async () => {
    renderPage(base);
    const search = await screen.findByLabelText("搜索规则名称或进程");
    fireEvent.change(search, { target: { value: "codex" } });
    expect(screen.getByText("AI 工具")).toBeInTheDocument();
    expect(screen.queryByText("curl.exe")).not.toBeInTheDocument();
    fireEvent.change(search, { target: { value: "nothing" } });
    expect(screen.getByText("没有匹配的规则")).toBeInTheDocument();
  });

  it("keeps the switch next to the title and explains a paused state", async () => {
    vi.mocked(commands.saveProcessRoutingSettings).mockResolvedValue({ status: "ok", data: null });
    renderPage(base, { ...model, process_routing: { ...model.process_routing, enabled: false } });
    expect(await screen.findByText("进程分流已暂停，下面的规则都不生效。")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "启用进程分流规则" })).not.toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: "恢复" }));
    await waitFor(() => expect(commands.saveProcessRoutingSettings).toHaveBeenCalledWith(true));
    // Exit policy and component maintenance live in Settings now.
    expect(screen.queryByText("退出行为与故障策略")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "更多组件操作" })).not.toBeInTheDocument();
  });

  it("hides the switch until the component is installed", async () => {
    renderPage({ ...base, component: "NotInstalled" });
    await screen.findByText("未安装");
    expect(screen.queryByRole("switch", { name: "启用进程分流规则" })).not.toBeInTheDocument();
  });

  it("toggles a rule through set_process_rule_enabled, not saveProcessRule", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    vi.mocked(commands.saveProcessRule).mockClear();
    renderPage(base);
    const row = (await screen.findAllByText("curl.exe"))[0].closest("tr")!;
    fireEvent.click(within(row).getByRole("switch", { name: "启用 curl.exe" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("set_process_rule_enabled", { id: "r1", enabled: false }),
    );
    expect(commands.saveProcessRule).not.toHaveBeenCalled();
  });

  it("ignores a second toggle while the first is pending", async () => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockImplementation((cmd: string) =>
      cmd === "set_process_rule_enabled" ? new Promise(() => {}) : Promise.resolve(undefined),
    );
    renderPage(base);
    const row = (await screen.findAllByText("curl.exe"))[0].closest("tr")!;
    fireEvent.click(within(row).getByRole("switch", { name: "启用 curl.exe" }));
    fireEvent.click(within(row).getByRole("button", { name: "操作 curl.exe" }));
    const item = await screen.findByRole("menuitem", { name: "禁用 curl.exe" });
    expect(item).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(item);
    const calls = vi.mocked(invoke).mock.calls.filter(([c]) => c === "set_process_rule_enabled");
    expect(calls).toHaveLength(1);
  });

  it("explains that switching node reconnects the app", async () => {
    renderPage(base);
    await screen.findAllByText("curl.exe");
    expect(screen.getByText(/换节点后该程序的现有连接会断开重连/)).toBeInTheDocument();
    expect(screen.queryByText(/HTTP 类型的监听器不能选/)).not.toBeInTheDocument();
    expect(screen.queryByText(/不强制断开已有连接/)).not.toBeInTheDocument();
  });

  it("shows the error on the row when enabling fails", async () => {
    vi.mocked(invoke).mockRejectedValue("服务未确认，已恢复原状态");
    renderPage(base);
    const row = (await screen.findAllByText("curl.exe"))[0].closest("tr")!;
    fireEvent.click(within(row).getByRole("switch", { name: "启用 curl.exe" }));
    expect(await within(row).findByText("服务未确认，已恢复原状态")).toBeInTheDocument();
  });
});
