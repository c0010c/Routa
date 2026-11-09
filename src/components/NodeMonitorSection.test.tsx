import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Model, MonitorStatus, MonitorTarget, NodeMonitorSettings, RuntimeStatus } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
    saveSettings: vi.fn(),
    monitorProbeNow: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "./ApplyResultSnackbar";
import { NodeMonitorSection } from "./NodeMonitorSection";

const monitorOn: NodeMonitorSettings = {
  enabled: true, scope: "InUse", members: [], interval_secs: 300, fail_threshold: 2,
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
    expect(screen.queryByText("监测范围")).not.toBeInTheDocument();
  });

  it("toggle saves enabled", async () => {
    renderSection({ enabled: false });
    fireEvent.click(await screen.findByRole("switch", { name: "节点监测" }));
    await waitFor(() => expect(commands.saveSettings).toHaveBeenCalled());
    expect(savedMonitor().enabled).toBe(true);
  });

  it("interval select offers 10 seconds", async () => {
    renderSection({ enabled: true });
    fireEvent.mouseDown(await screen.findByRole("combobox", { name: "探测间隔" }));
    fireEvent.click(await screen.findByRole("option", { name: "10 秒" }));
    await waitFor(() => expect(commands.saveSettings).toHaveBeenCalled());
    expect(savedMonitor().interval_secs).toBe(10);
  });

  it("switching to custom prefills in-use nodes", async () => {
    renderSection({ enabled: true }, monitor({ targets: [target("A"), target("B")] }));
    await screen.findByText("A");
    fireEvent.mouseDown(screen.getByRole("combobox", { name: "监测范围" }));
    fireEvent.click(await screen.findByRole("option", { name: "指定节点" }));
    await waitFor(() => expect(commands.saveSettings).toHaveBeenCalled());
    expect(savedMonitor().scope).toBe("Custom");
    expect(savedMonitor().members).toHaveLength(2);
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

  it("paused footer", async () => {
    renderSection({ enabled: true }, monitor({ paused_reason: "CoreNotRunning", targets: [target("A")] }));
    expect(await screen.findByText("内核未运行，监测已暂停")).toBeInTheDocument();
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
