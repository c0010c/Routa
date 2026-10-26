import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
  },
}));

import { commands } from "../bindings";
import { Overview } from "./Overview";

const model: Model = {
  subscriptions: [],
  exits: [
    { id: "e1", name: "香港", kind: "Direct", interface: null },
    { id: "e2", name: "美国", kind: "Direct", interface: null },
  ],
  listeners: [
    { id: "l1", name: "办公", listener_type: "Mixed", listen: "0.0.0.0", port: 7890, auth: null, exit: "e1", user_enabled: true },
    { id: "l2", name: "下载", listener_type: "Mixed", listen: "0.0.0.0", port: 7891, auth: null, exit: "e2", user_enabled: true },
    { id: "l3", name: "备用", listener_type: "Mixed", listen: "0.0.0.0", port: 7892, auth: null, exit: null, user_enabled: true },
  ],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
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
  startup_error: null,
  save_error: null,
};

function renderOverview() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <Overview />
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

  it("marks port-failed listeners ahead of Active", async () => {
    vi.mocked(commands.getStatus).mockResolvedValue({
      ...status,
      failed_listeners: [{ id: "l1", name: "办公", port: 7890 }],
    });
    renderOverview();
    expect(await screen.findByText("端口被占用")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("端口 7890 被占用");
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
