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
import { Nodes, visibleRows, type Row } from "./Nodes";

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
  process_routing: { enabled: false, on_routa_exit: "Block", rules: [] },
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
  tailscale: {},
  tailscale_exit_errors: {},
  tailscale_exit_retrying: {},
  core_stop_reason: null,
  core_recovering: false,
};

function renderNodes(st: RuntimeStatus = status) {
  vi.mocked(commands.getModel).mockResolvedValue(model);
  vi.mocked(commands.getStatus).mockResolvedValue(st);
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
  it("uses outlined buttons for secondary actions", async () => {
    renderNodes();
    const nav = await screen.findByRole("navigation", { name: "节点分组" });
    fireEvent.click(within(nav).getByText("未分组"));
    expect(screen.getByRole("button", { name: "全部测速" })).toHaveClass("MuiButton-outlined");
    expect(screen.getByRole("button", { name: "粘贴 YAML" })).toHaveClass("MuiButton-outlined");
    expect(screen.getByRole("button", { name: "新建节点" })).toHaveClass("MuiButton-contained");
  });

  it("puts node and subscription edit/delete in row menus", async () => {
    renderNodes();
    const nav = await screen.findByRole("navigation", { name: "节点分组" });
    fireEvent.click(within(nav).getByRole("button", { name: "操作 机场A" }));
    expect(screen.getByRole("menuitem", { name: "编辑 机场A" })).toBeInTheDocument();
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    fireEvent.click(within(nav).getByText("未分组"));
    fireEvent.click(screen.getByRole("button", { name: "操作 家用 socks" }));
    expect(screen.getByRole("menuitem", { name: "删除 家用 socks" })).toBeInTheDocument();
  });

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
    expect(within(tsRow).getByRole("cell", { name: /^tailscale/ })).toBeInTheDocument();
    const link = within(tsRow).getByRole("link", { name: "去 Tailscale 页编辑" });
    expect(link).toHaveAttribute("href", "/tailscale");
  });

  it("offers socks5/http nodes and Tailscale instances under 新建节点", async () => {
    renderNodes();
    fireEvent.click(within(await screen.findByRole("navigation", { name: "节点分组" })).getByText("自建组1"));
    fireEvent.click(screen.getByRole("button", { name: "新建节点" }));
    expect(screen.getByRole("menuitem", { name: "socks5 / http 节点" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("menuitem", { name: "Tailscale 实例" }));
    expect(await screen.findByText("新建 Tailscale 实例")).toBeInTheDocument();
    // Created from a group: the instance joins it.
    fireEvent.click(screen.getByRole("button", { name: "高级选项" }));
    expect(screen.getByRole("checkbox", { name: "自建组1" })).toBeChecked();
  });

  it("shows the login state on Tailscale rows", async () => {
    renderNodes({ ...status, tailscale: { "手动/家里": { NeedsLogin: { url: "https://login.tailscale.com/a/abc" } } } });
    fireEvent.click(within(await screen.findByRole("navigation", { name: "节点分组" })).getByText("未分组"));
    const tsRow = (await screen.findByRole("cell", { name: "家里" })).closest("tr")!;
    expect(await within(tsRow).findByText("待登录")).toBeInTheDocument();
    const socksRow = screen.getByRole("cell", { name: "家用 socks" }).closest("tr")!;
    expect(within(socksRow).queryByText("待登录")).not.toBeInTheDocument();
  });
});

describe("visibleRows", () => {
  const row = (name: string, server = "s"): Row => ({ key: name, name, type: "ss", server, proxyName: name });
  const rows = [row("香港 01", "hk.a.com"), row("日本 01", "jp.a.com"), row("美国 01", "us.a.com"), row("新加坡")];
  const delays = { "香港 01": { ms: 300, tested_at: 0 }, "日本 01": { ms: 80, tested_at: 0 }, "美国 01": { ms: null, tested_at: 0 } };

  it("filters by name or server, case-insensitively", () => {
    expect(visibleRows(rows, delays, "香港", false).map((r) => r.name)).toEqual(["香港 01"]);
    expect(visibleRows(rows, delays, " JP.A ", false).map((r) => r.name)).toEqual(["日本 01"]);
  });

  it("sorts by delay with timeouts and untested last, keeping the original otherwise", () => {
    expect(visibleRows(rows, delays, "", false).map((r) => r.name)).toEqual(["香港 01", "日本 01", "美国 01", "新加坡"]);
    expect(visibleRows(rows, delays, "", true).map((r) => r.name)).toEqual(["日本 01", "香港 01", "美国 01", "新加坡"]);
  });
});

describe("Nodes table", () => {
  beforeEach(() => {
    vi.mocked(commands.getModel).mockResolvedValue(model);
    vi.mocked(commands.getStatus).mockResolvedValue(status);
  });

  it("searches and reports no match", async () => {
    render(
      <QueryClientProvider client={new QueryClient()}>
        <ApplyResultProvider>
          <MemoryRouter>
            <Nodes />
          </MemoryRouter>
        </ApplyResultProvider>
      </QueryClientProvider>,
    );
    const search = await screen.findByLabelText("搜索节点");
    fireEvent.change(search, { target: { value: "不存在" } });
    expect(screen.getByText("没有匹配「不存在」的节点")).toBeInTheDocument();
    fireEvent.change(search, { target: { value: "香港" } });
    expect(screen.getByText("香港 01")).toBeInTheDocument();
  });
});
