import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { Model, RuntimeStatus } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
    saveListener: vi.fn(),
    testDelays: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { Listeners } from "./Listeners";

const model: Model = {
  subscriptions: [],
  exits: [],
  listeners: [
    { id: "l1", name: "下载", listener_type: "Mixed", listen: "0.0.0.0", port: 7891, auth: null, exit: null, user_enabled: false },
  ],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [],
  node_groups: [],
  process_routing: { enabled: false, on_routa_exit: "Block", rules: [] },
};
const status: RuntimeStatus = {
  core: "Running",
  last_apply: null,
  health: { exits: {}, listeners: { l1: "DisabledByUser" } },
  delays: {},
  traffic: null,
  interfaces: [],
  failed_listeners: [],
  effective_listeners: [],
  startup_error: null,
  save_error: null,
  tailscale: {},
  tailscale_exit_errors: {},
  tailscale_exit_retrying: {},
  core_stop_reason: null,
  core_recovering: false,
};

describe("Listeners", () => {
  it("surfaces validation errors from the enable switch in the snackbar", async () => {
    vi.mocked(commands.getModel).mockResolvedValue(model);
    vi.mocked(commands.getStatus).mockResolvedValue(status);
    vi.mocked(commands.saveListener).mockResolvedValue({
      status: "error",
      error: [{ PortConflict: { port: 7891, with: "浏览器" } }],
    });
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <ApplyResultProvider>
          <Listeners />
        </ApplyResultProvider>
      </QueryClientProvider>,
    );
    fireEvent.click(await screen.findByRole("switch", { name: "启用 下载" }));
    expect(await screen.findByText("端口 7891 已被「浏览器」占用")).toBeInTheDocument();
    await waitFor(() => expect(commands.saveListener).toHaveBeenCalled());
  });

  it("shows 未接出口 for a listener without exit", async () => {
    vi.mocked(commands.getModel).mockResolvedValue(model);
    vi.mocked(commands.getStatus).mockResolvedValue(status);
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <ApplyResultProvider>
          <Listeners />
        </ApplyResultProvider>
      </QueryClientProvider>,
    );
    expect(await screen.findByText("未设置")).toBeInTheDocument();
  });

  function renderList(m: Model) {
    vi.mocked(commands.getModel).mockResolvedValue(m);
    vi.mocked(commands.getStatus).mockResolvedValue(status);
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <ApplyResultProvider>
          <MemoryRouter>
            <Listeners />
          </MemoryRouter>
        </ApplyResultProvider>
      </QueryClientProvider>,
    );
  }

  it("shows an empty state instead of an empty table", async () => {
    renderList({ ...model, listeners: [] });
    expect(await screen.findByText("暂无代理，新增代理配置端口与去向。")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.getByRole("button", {name:"新增代理"})).toBeInTheDocument();
  });

  it("keeps enable, edit and delete directly in the card header", async () => {
    renderList(model);
    expect(await screen.findByRole("switch", { name: "启用 下载" })).toBeInTheDocument();
    expect(screen.getByRole("button", {name:"编辑 下载"})).toBeInTheDocument();
    expect(screen.getByRole("button", {name:"删除 下载"})).toBeInTheDocument();
    expect(screen.queryByRole("button", {name:"复制"})).not.toBeInTheDocument();
  });
});

it("measures the proxy's exit path from its delay value and prevents repeat submissions", async () => {
  const data: Model = {
    ...model,
    manual_nodes: [{ id: "n", name: "节点甲", groups: [], kind: { Proxy: { raw: { type: "ss" } } } }],
    exits: [{ id: "e", name: "代理出口", interface: null, kind: { Node: { node: { manual_id: "n" } } } }],
    listeners: [{ ...model.listeners[0], exit: "e", user_enabled: true }],
  };
  vi.mocked(commands.getModel).mockResolvedValue(data);
  vi.mocked(commands.getStatus).mockResolvedValue({ ...status, delays: { "代理出口": { ms: 132, tested_at: 1 }, "手动/节点甲": { ms: 42, tested_at: 1 } } });
  let finish!: (value: Awaited<ReturnType<typeof commands.testDelays>>) => void;
  vi.mocked(commands.testDelays).mockReset().mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}><ApplyResultProvider><Listeners /></ApplyResultProvider></QueryClientProvider>);
  const measure = await screen.findByRole("button", { name: "测量 下载 的代理延迟" });
  expect(measure).toHaveTextContent("132");
  fireEvent.click(measure);
  fireEvent.click(measure);
  await waitFor(() => expect(commands.testDelays).toHaveBeenCalledWith(["代理出口"]));
  await waitFor(() => expect(measure).toBeDisabled());
  expect(measure).toHaveAttribute("aria-busy", "true");
  fireEvent.click(measure);
  expect(commands.testDelays).toHaveBeenCalledTimes(1);
  finish({ status: "ok", data: [] });
  await waitFor(() => expect(measure).not.toBeDisabled());
});

it("updates the switch and runtime status after saving without remounting the page", async () => {
  let currentModel: Model = {
    ...model,
    exits: [{ id: "e1", name: "直连", kind: "Direct", interface: null }],
    listeners: [{ ...model.listeners[0], exit: "e1" }],
  };
  let currentStatus = status;
  vi.mocked(commands.getModel).mockImplementation(async () => structuredClone(currentModel));
  vi.mocked(commands.getStatus).mockImplementation(async () => structuredClone(currentStatus));
  vi.mocked(commands.saveListener).mockImplementation(async (saved) => {
    currentModel = { ...currentModel, listeners: [saved] };
    currentStatus = {
      ...currentStatus,
      health: {
        ...currentStatus.health,
        listeners: { l1: saved.user_enabled ? "Active" : "DisabledByUser" },
      },
      effective_listeners: saved.user_enabled ? ["l1"] : [],
    };
    return { status: "ok", data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } };
  });
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <ApplyResultProvider>
        <MemoryRouter><Listeners /></MemoryRouter>
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
  const toggle = await screen.findByRole("switch", { name: "启用 下载" });
  expect(toggle).not.toBeChecked();
  fireEvent.click(toggle);
  await waitFor(() => expect(commands.saveListener).toHaveBeenLastCalledWith(
    expect.objectContaining({ id: "l1", user_enabled: true }),
  ));
  await waitFor(() => expect(toggle).toBeChecked());
  expect(await screen.findByText("运行中")).toBeInTheDocument();
  fireEvent.click(toggle);
  await waitFor(() => expect(commands.saveListener).toHaveBeenLastCalledWith(
    expect.objectContaining({ id: "l1", user_enabled: false }),
  ));
  await waitFor(() => expect(toggle).not.toBeChecked());
  expect(await screen.findByText("已停用")).toBeInTheDocument();
});
