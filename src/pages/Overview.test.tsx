import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { Model, RuntimeStatus } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
    coreStart: vi.fn(async () => ({ status: "ok", data: null })),
    coreStop: vi.fn(),
    coreRestart: vi.fn(),
    resetTraffic: vi.fn(),
    selectGroupMember: vi.fn(),
    saveListener: vi.fn(async () => ({ status: "ok", data: { NotRunning: { error: "" } } })),
    groupNow: vi.fn(async () => ({ status: "ok", data: { 香港自动: "香港自动/机场A/HK 03" } })),
    saveExit: vi.fn(),
    deleteListener: vi.fn(async () => ({ status: "ok", data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } })),
  },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { collectProblems, Overview } from "./Overview";

const model: Model = {
  subscriptions: [],
  exits: [
    { id: "e1", name: "香港", kind: "Direct", interface: null },
    { id: "e2", name: "日本", kind: "Direct", interface: null },
    {
      id: "e3",
      name: "香港自动",
      kind: { Group: { strategy: "UrlTest", members: [], selected: null } },
      interface: null,
    },
  ],
  listeners: [
    { id: "l1", name: "办公", listener_type: "Mixed", listen: "0.0.0.0", port: 7890, auth: null, exit: "e1", user_enabled: true },
    { id: "l2", name: "下载", listener_type: "Mixed", listen: "0.0.0.0", port: 7891, auth: null, exit: "e3", user_enabled: true },
    { id: "l3", name: "备用", listener_type: "Mixed", listen: "0.0.0.0", port: 7892, auth: null, exit: null, user_enabled: true },
  ],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [],
  node_groups: [],
};

const status: RuntimeStatus = {
  core: "Running",
  last_apply: null,
  health: {
    exits: {
      e1: { status: "Ok", effective_members: [], missing_members: [], interface: null },
      e2: { status: "NodeMissing", effective_members: [], missing_members: [], interface: null },
    },
    listeners: { l1: "Active", l2: { ExitBroken: { exit_status: "NodeMissing" } }, l3: "NoExit" },
  },
  delays: { 香港: { ms: 123, tested_at: 0 } },
  traffic: null,
  interfaces: [],
  failed_listeners: [],
  effective_listeners: ["l1"],
  startup_error: null,
  save_error: null,
  tailscale: {},
};

function renderOverview() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <ApplyResultProvider>
        <MemoryRouter>
          <Overview />
        </MemoryRouter>
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
}

describe("Overview", () => {
  beforeEach(() => {
    vi.mocked(commands.getModel).mockResolvedValue(model);
    vi.mocked(commands.getStatus).mockResolvedValue(status);
  });

  it("shows the banner and one row per listener", async () => {
    renderOverview();
    const banner = await screen.findByRole("alert");
    expect(banner).toHaveTextContent("节点已失效");
    const rows = screen.getAllByRole("row");
    expect(rows).toHaveLength(1 + 3);
    expect(within(rows[1]).getByText("运行中")).toBeInTheDocument();
    expect(within(rows[1]).getByText("123 ms")).toBeInTheDocument();
    expect(within(rows[1]).getByText("系统默认")).toBeInTheDocument();
    expect(within(rows[2]).getByText("节点已失效，请重选")).toBeInTheDocument();
  });

  it("saves the chosen exit immediately", async () => {
    renderOverview();
    const row = (await screen.findAllByRole("row"))[1];
    fireEvent.mouseDown(within(row).getByRole("combobox", { name: "办公 出口" }));
    fireEvent.click(await screen.findByRole("option", { name: "日本" }));
    await waitFor(() =>
      expect(commands.saveListener).toHaveBeenCalledWith(expect.objectContaining({ id: "l1", exit: "e2" })),
    );
  });

  it("shows a rejected inline exit save", async () => {
    vi.mocked(commands.saveListener).mockResolvedValueOnce({
      status: "error",
      error: [{ InterfaceMismatch: { exit: "日本", node: "家里" } }],
    });
    renderOverview();
    const row = (await screen.findAllByRole("row"))[1];
    fireEvent.mouseDown(within(row).getByRole("combobox", { name: "办公 出口" }));
    fireEvent.click(await screen.findByRole("option", { name: "日本" }));
    await waitFor(() => expect(commands.saveListener).toHaveBeenCalled());
    expect(await screen.findByText("出口「日本」与 Tailscale「家里」的网卡不一致")).toBeInTheDocument();
  });

  it("clears the exit with 不接", async () => {
    renderOverview();
    const row = (await screen.findAllByRole("row"))[1];
    fireEvent.mouseDown(within(row).getByRole("combobox", { name: "办公 出口" }));
    fireEvent.click(await screen.findByRole("option", { name: "不接" }));
    await waitFor(() =>
      expect(commands.saveListener).toHaveBeenCalledWith(expect.objectContaining({ id: "l1", exit: null })),
    );
  });

  it("opens the new-exit dialog from the exit select", async () => {
    renderOverview();
    const row = (await screen.findAllByRole("row"))[1];
    fireEvent.mouseDown(within(row).getByRole("combobox", { name: "办公 出口" }));
    fireEvent.click(await screen.findByRole("option", { name: "+ 新建出口…" }));
    expect(await screen.findByText("新建出口")).toBeInTheDocument();
  });

  it("shows 未接出口 for a listener without exit", async () => {
    renderOverview();
    const rows = await screen.findAllByRole("row");
    expect(within(rows[3]).getAllByText("未接出口").length).toBeGreaterThan(0);
  });

  it("shows the group's current node without the exit prefix", async () => {
    renderOverview();
    expect(await screen.findByText("机场A/HK 03")).toBeInTheDocument();
  });

  it("opens the new-listener dialog", async () => {
    renderOverview();
    fireEvent.click(await screen.findByRole("button", { name: "新建监听器" }));
    expect(await screen.findByRole("dialog")).toHaveTextContent("新建监听器");
  });

  it("marks port-failed listeners ahead of Active", async () => {
    vi.mocked(commands.getStatus).mockResolvedValue({
      ...status,
      failed_listeners: [{ id: "l1", name: "办公", port: 7890 }],
    });
    renderOverview();
    expect(await screen.findByText("端口被占用")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("端口 7890 被占用");
  });

  it("marks listeners not in effect when the last apply failed closed", async () => {
    vi.mocked(commands.getStatus).mockResolvedValue({
      ...status,
      last_apply: { FailedClosed: { error: "bad config", uncut: 0 } },
    });
    renderOverview();
    const rows = await screen.findAllByRole("row");
    expect(within(rows[1]).getByText("未生效")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("配置未生效：bad config");
  });

  it("marks listeners not in effect when mihomo holds a config without them", async () => {
    // A retry from the fail-closed empty config was rejected again.
    vi.mocked(commands.getStatus).mockResolvedValue({
      ...status,
      last_apply: { FailedKeptPrevious: { error: "still bad" } },
      effective_listeners: [],
    });
    renderOverview();
    const rows = await screen.findAllByRole("row");
    expect(within(rows[1]).getByText("未生效")).toBeInTheDocument();
    expect(within(rows[1]).queryByText("运行中")).not.toBeInTheDocument();
  });

  it("edits a listener from its row", async () => {
    renderOverview();
    fireEvent.click(await screen.findByRole("button", { name: "编辑 办公" }));
    expect(await screen.findByRole("dialog")).toHaveTextContent("编辑监听器");
    expect(screen.getByLabelText("名称")).toHaveValue("办公");
  });

  it("deletes a listener after confirming", async () => {
    renderOverview();
    fireEvent.click(await screen.findByRole("button", { name: "删除 下载" }));
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("确定删除监听器「下载」吗？");
    fireEvent.click(within(dialog).getByRole("button", { name: "删除" }));
    await waitFor(() => expect(commands.deleteListener).toHaveBeenCalledWith("l2"));
  });

  it("asks before resetting traffic totals", async () => {
    renderOverview();
    fireEvent.click(await screen.findByRole("button", { name: "清零" }));
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("清零累计流量？");
    expect(commands.resetTraffic).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("button", { name: "清零" }));
    await waitFor(() => expect(commands.resetTraffic).toHaveBeenCalled());
  });

  it("hides the getting-started card once a listener has an exit", async () => {
    renderOverview();
    await screen.findAllByRole("row");
    expect(screen.queryByText("开始使用：三步让一个端口能上网")).not.toBeInTheDocument();
  });

  it("shows the getting-started card and an empty row with no listeners", async () => {
    vi.mocked(commands.getModel).mockResolvedValue({ ...model, listeners: [] });
    renderOverview();
    expect(await screen.findByText("开始使用：三步让一个端口能上网")).toBeInTheDocument();
    expect(screen.getByText("还没有监听器")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "新建出口" }));
    expect(await screen.findByRole("dialog")).toHaveTextContent("新建出口");
  });

  it("enables start only when stopped", async () => {
    vi.mocked(commands.getStatus).mockResolvedValue({ ...status, core: "Stopped" });
    renderOverview();
    const start = await screen.findByRole("button", { name: "启动" });
    expect(start).toBeEnabled();
    expect(screen.getByRole("button", { name: "停止" })).toBeDisabled();
    fireEvent.click(start);
    await waitFor(() => expect(commands.coreStart).toHaveBeenCalled());
    // let the post-mutation refetch settle so no state updates leak past the test
    await waitFor(() => expect(commands.getStatus).toHaveBeenCalledTimes(2));
  });
});

describe("collectProblems", () => {
  const healthy: RuntimeStatus = { ...status, health: { exits: {}, listeners: {} } };
  it("reports nothing for a running core with a clean reload", () =>
    expect(
      collectProblems(model, { ...healthy, last_apply: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } }),
    ).toEqual([]));
  it("reports a stopped or crashed core", () => {
    expect(collectProblems(model, { ...healthy, core: "Stopped" })).toEqual(["mihomo 已停止"]);
    expect(collectProblems(model, { ...healthy, core: { Crashed: { exit_code: 3 } } })).toEqual([
      "mihomo 已崩溃（退出码 3）",
    ]);
  });
  it("reports every non-Reloaded last apply", () => {
    const of = (last_apply: RuntimeStatus["last_apply"]) => collectProblems(model, { ...healthy, last_apply });
    expect(of({ FailedKeptPrevious: { error: "a" } })).toEqual(["配置未生效：a"]);
    expect(of({ FailedClosed: { error: "b", uncut: 0 } })).toEqual(["配置未生效：b"]);
    expect(of({ FailedCoreStopped: { error: "c" } })).toEqual(["配置未生效：c"]);
    expect(of({ NotRunning: { error: "d" } })).toEqual(["配置未生效：d"]);
  });
});
