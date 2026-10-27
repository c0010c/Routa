import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { Model, RuntimeStatus } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { Nodes } from "./Nodes";

const model: Model = {
  subscriptions: [
    {
      id: "s1",
      name: "机场A",
      url: "https://example.com/sub",
      update_interval_mins: 1440,
      fallback_exit: null,
      last_updated: 1_700_000_000,
      last_result: { Failed: { reason: "订阅返回了 HTML 页面" } },
      nodes: [{ name: "香港 01", proxy_type: "ss", server: "hk.example.com", raw: {} }],
    },
  ],
  exits: [],
  listeners: [],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [
    {
      id: "m1",
      name: "家用 socks",
      groups: [],
      kind: { Proxy: { raw: { name: "家用 socks", type: "socks5", server: "100.64.0.2", port: 1080 } } },
    },
    {
      id: "t1",
      name: "家里",
      groups: [],
      kind: {
        Tailscale: { hostname: "mihomo-家里", exit_node: "auto:any", ephemeral: false, udp: true, auth_key: null, interface: null },
      },
    },
  ],
  node_groups: [{ id: "g1", name: "自建组1" }],
};
const status: RuntimeStatus = {
  core: "Running",
  last_apply: null,
  health: { exits: {}, listeners: {} },
  delays: { "机场A/香港 01": { ms: 123, tested_at: 0 }, "手动/家用 socks": { ms: 45, tested_at: 0 } },
  traffic: null,
  interfaces: [],
  failed_listeners: [],
  effective_listeners: [],
  startup_error: null,
  save_error: null,
};

function renderNodes() {
  vi.mocked(commands.getModel).mockResolvedValue(model);
  vi.mocked(commands.getStatus).mockResolvedValue(status);
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <ApplyResultProvider>
        <MemoryRouter>
          <Nodes />
        </MemoryRouter>
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
}

describe("Nodes", () => {
  it("shows the failure reason on the card and keeps the old nodes listed", async () => {
    renderNodes();
    expect(await screen.findByText(/订阅返回了 HTML 页面/)).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "香港 01" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "123 ms" })).toBeInTheDocument();
  });

  it("lists subscriptions and manual groups on the left; shows manual nodes when selected", async () => {
    renderNodes();
    const nav = await screen.findByRole("navigation", { name: "节点分组" });
    expect(within(nav).getByText("机场A")).toBeInTheDocument();
    expect(within(nav).getByText("未分组")).toBeInTheDocument();
    expect(within(nav).getByText("自建组1")).toBeInTheDocument();

    fireEvent.click(within(nav).getByText("未分组"));
    expect(screen.getByRole("cell", { name: "家用 socks" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "45 ms" })).toBeInTheDocument();
    expect(screen.queryByRole("cell", { name: "香港 01" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "新建节点" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "粘贴 YAML" })).toBeInTheDocument();

    const tsRow = screen.getByRole("cell", { name: "家里" }).closest("tr")!;
    expect(within(tsRow).getByRole("cell", { name: "tailscale" })).toBeInTheDocument();
    const link = within(tsRow).getByRole("link", { name: "去 Tailscale 页编辑" });
    expect(link).toHaveAttribute("href", "/tailscale");
  });
});
