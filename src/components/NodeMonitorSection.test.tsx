import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Model, MonitorStatus, MonitorTarget, NodeMonitorSettings, RuntimeStatus } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
    saveSettings: vi.fn(),
    monitorProbeNow: vi.fn(),
    monitorInUseNodes: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "./ApplyResultSnackbar";
import { NodeMonitorSection } from "./NodeMonitorSection";

const monitorOn: NodeMonitorSettings = {
  enabled: true, members: [], interval_secs: 300, fail_threshold: 2,
  latency_limit_ms: 1000, notify_recovery: true,
};

const model = (nm: Partial<NodeMonitorSettings> = {}): Model => ({
  subscriptions: [],
  exits: [],
  listeners: [],
  settings: {
    autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u",
    node_monitor: { ...monitorOn, enabled: false, ...nm },
  },
  manual_nodes: [],
  node_groups: [],
  process_routing: { enabled: false, rules: [], exit_prompt: true },
});

const target = (name: string, over: Partial<MonitorTarget> = {}): MonitorTarget => ({
  proxy: `订阅/${name}`, node: { subscription_id: "s1", name }, label: name, used_by: ["工作代理 :7890"],
  fails: 0, down: false, down_since: null, last_ms: 48, last_failure: null, probed: true, not_applied: false,
  ...over,
});

const monitor = (over: Partial<MonitorStatus> = {}): MonitorStatus => ({
  last_run: null, next_run: null, probing: false, paused_reason: null, network_down: false, targets: [], ...over,
});

function renderSection(nm: Partial<NodeMonitorSettings>, mon: MonitorStatus = monitor()) {
  vi.mocked(commands.getModel).mockResolvedValue(model(nm));
  vi.mocked(commands.getStatus).mockResolvedValue({ monitor: mon } as unknown as RuntimeStatus);
  vi.mocked(commands.saveSettings).mockResolvedValue({
    status: "ok",
    data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } },
  });
  vi.mocked(commands.monitorProbeNow).mockResolvedValue({ status: "ok", data: null });
  vi.mocked(commands.monitorInUseNodes).mockResolvedValue({
    status: "ok",
    data: { nodes: [{ subscription_id: "s1", name: "A" }, { manual_id: "m1" }], auto_exits: ["自动"] },
  });
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <ApplyResultProvider>
        <NodeMonitorSection />
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
}

const savedMonitor = () =>
  vi.mocked(commands.saveSettings).mock.lastCall![0].node_monitor;

describe("NodeMonitorSection", () => {
  beforeEach(() => vi.clearAllMocks());

  it("hides rows when disabled", async () => {
    renderSection({ enabled: false });
    expect(await screen.findByRole("switch", { name: "节点监测" })).toBeInTheDocument();
    expect(screen.queryByText("监测节点")).not.toBeInTheDocument();
  });

  it("first enable prefills the in-use nodes", async () => {
    renderSection({ enabled: false });
    fireEvent.click(await screen.findByRole("switch", { name: "节点监测" }));
    await waitFor(() => expect(commands.saveSettings).toHaveBeenCalled());
    expect(savedMonitor().enabled).toBe(true);
    expect(savedMonitor().members).toEqual([{ subscription_id: "s1", name: "A" }, { manual_id: "m1" }]);
    expect(await screen.findByText("自动选择的出口「自动」没有固定节点，未加入监测")).toBeInTheDocument();
  });

  it("enabling keeps members chosen before", async () => {
    renderSection({ enabled: false, members: [{ manual_id: "m2" }] });
    fireEvent.click(await screen.findByRole("switch", { name: "节点监测" }));
    await waitFor(() => expect(commands.saveSettings).toHaveBeenCalled());
    expect(savedMonitor().members).toEqual([{ manual_id: "m2" }]);
    expect(commands.monitorInUseNodes).not.toHaveBeenCalled();
  });

  it("interval select offers 10 seconds", async () => {
    renderSection({ enabled: true });
    fireEvent.mouseDown(await screen.findByRole("combobox", { name: "探测间隔" }));
    fireEvent.click(await screen.findByRole("option", { name: "10 秒" }));
    await waitFor(() => expect(commands.saveSettings).toHaveBeenCalled());
    expect(savedMonitor().interval_secs).toBe(10);
  });

  it("adds the in-use nodes that are not members yet", async () => {
    renderSection({ enabled: true, members: [{ manual_id: "m1" }, { manual_id: "m2" }] });
    fireEvent.click(await screen.findByRole("button", { name: "加入正在使用的节点" }));
    await waitFor(() => expect(commands.saveSettings).toHaveBeenCalled());
    expect(savedMonitor().members).toEqual([{ manual_id: "m1" }, { manual_id: "m2" }, { subscription_id: "s1", name: "A" }]);
  });

  it("target row shows source and users", async () => {
    renderSection({ enabled: true }, monitor({ last_run: 1_700_000_000, targets: [target("A")] }));
    expect(await screen.findByText("订阅 · 工作代理 :7890")).toBeInTheDocument();
  });

  it("clearing_latency_limit_saves_null", async () => {
    renderSection({ enabled: true });
    const input = await screen.findByLabelText("延迟上限");
    fireEvent.change(input, { target: { value: "" } });
    fireEvent.blur(input);
    await waitFor(() => expect(commands.saveSettings).toHaveBeenCalled());
    expect(savedMonitor().latency_limit_ms).toBeNull();
  });

  it("invalid latency is not saved", async () => {
    renderSection({ enabled: true });
    const input = await screen.findByLabelText("延迟上限");
    fireEvent.change(input, { target: { value: "0" } });
    fireEvent.blur(input);
    expect(await screen.findByText("请输入 1–60000 的整数")).toBeInTheDocument();
    expect(commands.saveSettings).not.toHaveBeenCalled();
  });

  it("footer shows summary and probe button", async () => {
    renderSection({ enabled: true }, monitor({
      last_run: 1_700_000_000,
      next_run: 1_700_000_300,
      targets: [target("A"), target("B", { down: true, last_failure: "Timeout" })],
    }));
    expect(await screen.findByText(/1 不可用/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "立即探测" }));
    await waitFor(() => expect(commands.monitorProbeNow).toHaveBeenCalled());
  });

  it("footer counts only probed healthy targets as normal", async () => {
    renderSection({ enabled: true }, monitor({
      last_run: 1_700_000_000,
      next_run: 1_700_000_300,
      targets: [
        target("A"),
        target("B", { down: true, fails: 2, last_failure: "Timeout" }),
        target("C", { probed: false, last_ms: null }),
        target("D", { not_applied: true }),
        target("E", { fails: 1, last_failure: "Timeout" }),
      ],
    }));
    expect(await screen.findByText(/· 1 正常$/)).toBeInTheDocument();
    expect(screen.getByText(/1 不可用/)).toBeInTheDocument();
  });

  it("network down footer line", async () => {
    renderSection({ enabled: true }, monitor({
      last_run: 1_700_000_000,
      network_down: true,
      targets: [target("A"), target("B")],
    }));
    expect(await screen.findByText("网络可能不可用，节点状态暂不更新")).toBeInTheDocument();
  });

  it("no network down line normally", async () => {
    renderSection({ enabled: true }, monitor({ last_run: 1_700_000_000, targets: [target("A")] }));
    await screen.findByText(/· 1 正常$/);
    expect(screen.queryByText("网络可能不可用，节点状态暂不更新")).not.toBeInTheDocument();
  });

  it("paused footer", async () => {
    renderSection({ enabled: true }, monitor({ paused_reason: "CoreNotRunning", targets: [target("A")] }));
    expect(await screen.findByText("内核未运行，监测已暂停")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "立即探测" })).toBeDisabled();
  });

  it("config-not-applied footer", async () => {
    renderSection({ enabled: true }, monitor({ paused_reason: "ConfigNotApplied", targets: [target("A")] }));
    expect(await screen.findByText("新配置未生效，监测已暂停；配置恢复正常后自动继续")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "立即探测" })).toBeDisabled();
  });

  it("target row texts", async () => {
    renderSection({ enabled: true }, monitor({
      last_run: 1_700_000_000,
      targets: [
        target("A", { probed: false }),
        target("B", { not_applied: true, probed: false }),
        target("C", { down: true, fails: 2, last_failure: "Timeout" }),
        target("D", { fails: 1 }),
        target("E"),
      ],
    }));
    expect(await screen.findByText("等待探测")).toBeInTheDocument();
    expect(screen.getByText("出口未生效")).toBeInTheDocument();
    expect(screen.getByText("不可用 · 超时")).toBeInTheDocument();
    expect(screen.getByText("失败 1/2")).toBeInTheDocument();
    expect(screen.getByText("48 ms")).toBeInTheDocument();
  });
});
