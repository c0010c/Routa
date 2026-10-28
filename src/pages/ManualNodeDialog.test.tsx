import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ManualNode, Model } from "../bindings";

vi.mock("../bindings", () => ({
  commands: { getModel: vi.fn(), saveManualNode: vi.fn(), parseProxyYaml: vi.fn() },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { ManualNodeDialog, newManualNode } from "./ManualNodeDialog";

const model: Model = {
  subscriptions: [],
  exits: [],
  listeners: [],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [],
  node_groups: [{ id: "g1", name: "家里" }],
  process_routing: { enabled: false, on_routa_exit: "Block", rules: [] },
};

function renderDialog(node: ManualNode, mode?: "form" | "yaml", existing: ManualNode[] = []) {
  vi.mocked(commands.getModel).mockResolvedValue({ ...model, manual_nodes: existing });
  vi.mocked(commands.saveManualNode).mockResolvedValue({ status: "ok", data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } });
  const onClose = vi.fn();
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <ApplyResultProvider>
        <ManualNodeDialog node={node} mode={mode} onClose={onClose} />
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
  return onClose;
}

const saved = () => vi.mocked(commands.saveManualNode).mock.calls[0][0];

beforeEach(() => vi.clearAllMocks());

describe("ManualNodeDialog", () => {
  it("saves a socks5 node from the form", async () => {
    const onClose = renderDialog(newManualNode());
    await screen.findByText("新建节点");
    fireEvent.change(screen.getByLabelText("名称"), { target: { value: "家用" } });
    fireEvent.change(screen.getByLabelText("服务器"), { target: { value: "100.64.0.2" } });
    fireEvent.change(screen.getByLabelText("端口"), { target: { value: "1080" } });
    fireEvent.click(await screen.findByLabelText("家里"));
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    const n = saved();
    expect(n.name).toBe("家用");
    expect(n.groups).toEqual(["g1"]);
    expect("Proxy" in n.kind && n.kind.Proxy.raw).toMatchObject({ type: "socks5", server: "100.64.0.2", port: 1080 });
  });

  it("keeps fields the form does not know about", async () => {
    const node: ManualNode = {
      id: "m1",
      name: "旧",
      groups: [],
      kind: { Proxy: { raw: { name: "旧", type: "socks5", server: "a", port: 1, "x-extra": 1 } } },
    };
    const onClose = renderDialog(node, undefined, [node]);
    await screen.findByText("编辑节点");
    expect(screen.getByLabelText("服务器")).toHaveValue("a");
    fireEvent.change(screen.getByLabelText("服务器"), { target: { value: "b" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    const n = saved();
    expect("Proxy" in n.kind && n.kind.Proxy.raw).toMatchObject({ server: "b", port: 1, "x-extra": 1 });
  });

  it("opens non-form types in YAML mode", async () => {
    const node: ManualNode = {
      id: "m2",
      name: "v",
      groups: [],
      kind: { Proxy: { raw: { name: "v", type: "vmess", server: "s" } } },
    };
    renderDialog(node, undefined, [node]);
    expect((await screen.findByLabelText("YAML") as HTMLTextAreaElement).value).toContain("vmess");
    expect(screen.queryByLabelText("服务器")).not.toBeInTheDocument();
  });

  it("warns about stripped fields in YAML mode", async () => {
    vi.mocked(commands.parseProxyYaml).mockResolvedValue({
      status: "ok",
      data: { raw: { name: "v", type: "vmess", server: "s" }, stripped: ["dialer-proxy"] },
    });
    renderDialog(newManualNode(), "yaml");
    fireEvent.change(await screen.findByLabelText("YAML"), {
      target: { value: "{name: v, type: vmess, server: s, dialer-proxy: x}" },
    });
    expect(await screen.findByText(/以下字段会被忽略：.*dialer-proxy/)).toBeInTheDocument();
  });

  it("shows the parse error under the text box and does not save", async () => {
    vi.mocked(commands.parseProxyYaml).mockResolvedValue({ status: "error", error: "节点缺少 type" });
    renderDialog(newManualNode(), "yaml");
    fireEvent.change(await screen.findByLabelText("YAML"), { target: { value: "{name: v}" } });
    expect(await screen.findByText("节点缺少 type")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(commands.parseProxyYaml).toHaveBeenCalledTimes(2));
    expect(screen.getByText("节点缺少 type")).toBeInTheDocument();
    expect(commands.saveManualNode).not.toHaveBeenCalled();
  });

  it("turns a pasted tailscale proxy into a Tailscale instance", async () => {
    vi.mocked(commands.parseProxyYaml).mockResolvedValue({
      status: "ok",
      data: { raw: { name: "ts", type: "tailscale", hostname: "h", "state-dir": "x", "exit-node": "auto:any", ephemeral: true }, stripped: [] },
    });
    const onClose = renderDialog(newManualNode(), "yaml");
    fireEvent.change(await screen.findByLabelText("YAML"), {
      target: { value: "{name: ts, type: tailscale, hostname: h, state-dir: x, exit-node: auto:any}" },
    });
    expect(await screen.findByText(/已转为 Tailscale 实例，需要在 Tailscale 页重新登录/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    const n = saved();
    expect(n.name).toBe("ts");
    expect("Tailscale" in n.kind && n.kind.Tailscale).toEqual({
      hostname: "h",
      exit_node: "auto:any",
      ephemeral: false,
      udp: true,
      auth_key: null,
      interface: null,
    });
    expect(JSON.stringify(n)).not.toContain("state-dir");
  });
});
