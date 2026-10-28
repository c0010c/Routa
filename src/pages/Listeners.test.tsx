import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { Model, RuntimeStatus } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
    saveListener: vi.fn(),
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
    expect(await screen.findByText("未接出口")).toBeInTheDocument();
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
    expect(await screen.findByText("还没有监听器")).toBeInTheDocument();
    expect(screen.queryByRole("columnheader")).toBeNull();
    expect(screen.getByRole("link", { name: "回概览页按步骤开始 →" })).toBeInTheDocument();
  });

  it("keeps the enable switch inline and edit/delete in the row menu", async () => {
    renderList(model);
    expect(await screen.findByRole("switch", { name: "启用 下载" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "操作 下载" }));
    expect(screen.getByRole("menuitem", { name: "编辑 下载" })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "删除 下载" })).toBeInTheDocument();
  });
});
