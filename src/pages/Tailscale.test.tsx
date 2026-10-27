import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Model, RuntimeStatus } from "../bindings";

vi.mock("../bindings", () => ({
  commands: { getModel: vi.fn(), getStatus: vi.fn(), saveManualNode: vi.fn(), deleteManualNode: vi.fn() },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { Tailscale } from "./Tailscale";
import { newTailscaleNode } from "./TailscaleDialog";

const model: Model = {
  subscriptions: [],
  exits: [{ id: "e1", name: "出口甲", interface: null, kind: { Group: { strategy: "Select", members: [{ manual_id: "t1" }], selected: null } } }],
  listeners: [],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [
    {
      id: "t1",
      name: "家里",
      groups: [],
      kind: { Tailscale: { hostname: "mihomo-家里", exit_node: "auto:any", ephemeral: false, udp: true, auth_key: null, interface: "g-eth" } },
    },
  ],
  node_groups: [],
};
const status = {
  interfaces: [{ guid: "g-eth", friendly_name: "以太网", is_up: true, ipv4: ["192.168.1.2"], ipv6: [] }],
} as unknown as RuntimeStatus;

function renderPage(m: Model = model) {
  vi.mocked(commands.getModel).mockResolvedValue(m);
  vi.mocked(commands.getStatus).mockResolvedValue(status);
  vi.mocked(commands.saveManualNode).mockResolvedValue({ status: "ok", data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } });
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <ApplyResultProvider>
        <Tailscale />
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
}

beforeEach(() => vi.clearAllMocks());

describe("newTailscaleNode", () => {
  it("uses the documented defaults", () => {
    const n = newTailscaleNode();
    expect(n.kind).toEqual({
      Tailscale: { hostname: "mihomo-", exit_node: "auto:any", ephemeral: false, udp: true, auth_key: null, interface: null },
    });
  });
});

describe("Tailscale page", () => {
  it("shows instance details, references and a placeholder login state", async () => {
    renderPage();
    const card = await screen.findByRole("article", { name: "家里" });
    expect(within(card).getByText(/mihomo-家里/)).toBeInTheDocument();
    expect(within(card).getByText(/auto:any/)).toBeInTheDocument();
    expect(within(card).getByText(/以太网/)).toBeInTheDocument();
    expect(within(card).getByText("被出口引用：出口甲")).toBeInTheDocument();
    expect(within(card).getByText("状态未知")).toBeInTheDocument();
  });

  it("creates an instance whose hostname follows the name", async () => {
    renderPage();
    fireEvent.click(await screen.findByRole("button", { name: "新建实例" }));
    fireEvent.change(await screen.findByLabelText("名称"), { target: { value: "家里2" } });
    expect(screen.getByLabelText("hostname")).toHaveValue("mihomo-家里2");
    fireEvent.change(screen.getByLabelText("名称"), { target: { value: "家里" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(commands.saveManualNode).toHaveBeenCalled());
    const n = vi.mocked(commands.saveManualNode).mock.calls[0][0];
    expect(n.name).toBe("家里");
    expect(n.kind).toMatchObject({ Tailscale: { hostname: "mihomo-家里", exit_node: "auto:any" } });
  });

  it("lists exits of kind Node as references", async () => {
    renderPage({
      ...model,
      exits: [{ id: "e2", name: "单点出口", interface: null, kind: { Node: { node: { manual_id: "t1" } } } }],
    });
    const card = await screen.findByRole("article", { name: "家里" });
    expect(within(card).getByText("被出口引用：单点出口")).toBeInTheDocument();
  });

  it("keeps the hostname when an existing instance is renamed", async () => {
    renderPage();
    fireEvent.click(await screen.findByRole("button", { name: "编辑 家里" }));
    await screen.findByText("编辑 Tailscale 实例");
    fireEvent.change(screen.getByLabelText("名称"), { target: { value: "办公室" } });
    expect(screen.getByLabelText("hostname")).toHaveValue("mihomo-家里");
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(commands.saveManualNode).toHaveBeenCalled());
    const n = vi.mocked(commands.saveManualNode).mock.calls[0][0];
    expect(n.name).toBe("办公室");
    expect(n.kind).toMatchObject({ Tailscale: { hostname: "mihomo-家里" } });
  });

  it("stops following the name once hostname is edited by hand", async () => {
    renderPage();
    fireEvent.click(await screen.findByRole("button", { name: "新建实例" }));
    fireEvent.change(await screen.findByLabelText("hostname"), { target: { value: "custom" } });
    fireEvent.change(screen.getByLabelText("名称"), { target: { value: "x" } });
    expect(screen.getByLabelText("hostname")).toHaveValue("custom");
  });

  it("explains that auto:any may not select an exit node", async () => {
    renderPage();
    fireEvent.click(await screen.findByRole("button", { name: "新建实例" }));
    expect(await screen.findByText("auto:any 在当前内核下可能无法自动选中出口节点，建议填写出口节点的 IP 或名称")).toBeInTheDocument();
  });

  it("shows the DanglingRef error when deleting a referenced instance", async () => {
    renderPage();
    vi.mocked(commands.deleteManualNode).mockResolvedValue({ status: "error", error: [{ DanglingRef: { what: "出口「出口甲」" } }] });
    fireEvent.click(await screen.findByRole("button", { name: "删除 家里" }));
    fireEvent.click(await screen.findByRole("button", { name: "删除" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("出口甲");
  });
});
