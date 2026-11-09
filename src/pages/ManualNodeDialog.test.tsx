import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ManualNode, Model } from "../bindings";

vi.mock("../bindings", () => ({
  commands: { getModel: vi.fn(), saveManualNode: vi.fn(), parseProxyYaml: vi.fn(), parseShareLink: vi.fn() },
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
  process_routing: { enabled: false, rules: [], exit_prompt: true },
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

const PBK = "Gq9XwR0oZ3nLk2bYp7VtHc5mJdE1sAfU8iQyN4xKr0M";
const realityRaw = {
  name: "东京",
  type: "vless",
  server: "203.0.113.10",
  port: 443,
  uuid: "3f1c2a9e-7b4d-4e2a-9c1f-0a1b2c3d4e5f",
  flow: "xtls-rprx-vision",
  tls: true,
  servername: "www.microsoft.com",
  "client-fingerprint": "safari",
  "reality-opts": { "public-key": PBK, "short-id": "6ba85179" },
  network: "tcp",
};
const rawOfSaved = () => {
  const k = saved().kind;
  return "Proxy" in k ? (k.Proxy.raw as Record<string, unknown>) : null;
};
const pasteLink = async (text: string) =>
  fireEvent.change(await screen.findByLabelText("分享链接"), { target: { value: text } });

async function chooseType(label: string) {
  fireEvent.mouseDown(await screen.findByRole("combobox", { name: "类型" }));
  fireEvent.click(within(screen.getByRole("listbox")).getByText(label));
}

describe("ManualNodeDialog VLESS + REALITY", () => {
  it("saves a node filled in by hand", async () => {
    const onClose = renderDialog(newManualNode());
    await chooseType("VLESS + REALITY");
    fireEvent.change(screen.getByLabelText("名称"), { target: { value: "自建" } });
    fireEvent.change(screen.getByLabelText("服务器"), { target: { value: "192.0.2.1" } });
    fireEvent.change(screen.getByLabelText("端口"), { target: { value: "8443" } });
    fireEvent.change(screen.getByLabelText("UUID"), { target: { value: "u-1" } });
    fireEvent.change(screen.getByLabelText("SNI"), { target: { value: "www.apple.com" } });
    fireEvent.change(screen.getByLabelText("Public Key"), { target: { value: PBK } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(rawOfSaved()).toEqual({
      name: "自建",
      type: "vless",
      server: "192.0.2.1",
      port: 8443,
      uuid: "u-1",
      network: "tcp",
      tls: true,
      udp: true,
      flow: "xtls-rprx-vision",
      servername: "www.apple.com",
      "client-fingerprint": "chrome",
      "reality-opts": { "public-key": PBK, "short-id": "", "support-x25519mlkem768": true },
    });
  });

  it("fills the form from a pasted link and saves it", async () => {
    vi.mocked(commands.parseShareLink).mockResolvedValue({ status: "ok", data: { raw: realityRaw, stripped: [] } });
    const onClose = renderDialog(newManualNode());
    await pasteLink("vless://x");
    await waitFor(() => expect(screen.getByLabelText("UUID")).toHaveValue(realityRaw.uuid));
    expect(screen.getByLabelText("名称")).toHaveValue("东京");
    expect(screen.getByLabelText("Short ID")).toHaveValue("6ba85179");
    expect(screen.getByText(/已填写/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(rawOfSaved()).toMatchObject({ ...realityRaw, udp: true });
    // Xray links carry no ML-KEM flag; Xray clients offer it by default, so the form does too.
    expect((rawOfSaved()!["reality-opts"] as Record<string, unknown>)["support-x25519mlkem768"]).toBe(true);
  });

  it("keeps an explicit ML-KEM flag from the link", async () => {
    vi.mocked(commands.parseShareLink).mockResolvedValue({
      status: "ok",
      data: {
        raw: { ...realityRaw, "reality-opts": { "public-key": PBK, "short-id": "", "support-x25519mlkem768": false } },
        stripped: [],
      },
    });
    const onClose = renderDialog(newManualNode());
    await pasteLink("vless://x");
    await waitFor(() => expect(screen.getByLabelText("UUID")).toHaveValue(realityRaw.uuid));
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect((rawOfSaved()!["reality-opts"] as Record<string, unknown>)["support-x25519mlkem768"]).toBe(false);
  });

  it("replaces a fingerprint mihomo does not support with chrome", async () => {
    vi.mocked(commands.parseShareLink).mockResolvedValue({
      status: "ok",
      data: { raw: { ...realityRaw, "client-fingerprint": "chrome140" }, stripped: [] },
    });
    const onClose = renderDialog(newManualNode());
    await pasteLink("vless://x");
    expect(await screen.findByText(/指纹 chrome140 mihomo 不支持，已改为 chrome/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(rawOfSaved()!["client-fingerprint"]).toBe("chrome");
  });

  it("flags an unsupported fingerprint on an existing node", async () => {
    const node: ManualNode = {
      id: "r",
      name: "东京",
      groups: [],
      kind: { Proxy: { raw: { ...realityRaw, "client-fingerprint": "chrome140" } } },
    };
    renderDialog(node, undefined, [node]);
    fireEvent.click(await screen.findByRole("button", { name: /高级/ }));
    expect(screen.getByText(/mihomo 不支持 chrome140/)).toBeInTheDocument();
  });

  it("says only the first of several links was used", async () => {
    vi.mocked(commands.parseShareLink).mockResolvedValue({ status: "ok", data: { raw: realityRaw, stripped: [] } });
    renderDialog(newManualNode());
    await pasteLink("vless://a\nvless://b\n\nvless://c");
    expect(await screen.findByText(/共 3 条，只用了第一条/)).toBeInTheDocument();
  });

  it("offers YAML for links the form cannot show", async () => {
    const ws = { ...realityRaw, "reality-opts": undefined, network: "ws", name: "WS" };
    vi.mocked(commands.parseShareLink).mockResolvedValue({ status: "ok", data: { raw: ws, stripped: [] } });
    vi.mocked(commands.parseProxyYaml).mockResolvedValue({ status: "ok", data: { raw: ws, stripped: [] } });
    renderDialog(newManualNode());
    await pasteLink("vless://ws");
    fireEvent.click(await screen.findByRole("button", { name: "用 YAML 打开" }));
    expect(((await screen.findByLabelText("YAML")) as HTMLTextAreaElement).value).toContain('"ws"');
  });

  it("shows the parse error for unknown links", async () => {
    vi.mocked(commands.parseShareLink).mockResolvedValue({ status: "error", error: "无法识别这条链接" });
    renderDialog(newManualNode());
    await pasteLink("https://x");
    expect(await screen.findByText("无法识别这条链接")).toBeInTheDocument();
  });

  it("warns about a node with the same server, port and uuid", async () => {
    const other: ManualNode = { id: "o", name: "老节点", groups: [], kind: { Proxy: { raw: { ...realityRaw, name: "老节点" } } } };
    vi.mocked(commands.parseShareLink).mockResolvedValue({ status: "ok", data: { raw: realityRaw, stripped: [] } });
    renderDialog(newManualNode(), undefined, [other]);
    await pasteLink("vless://x");
    expect(await screen.findByText(/已有相同节点「老节点」/)).toBeInTheDocument();
  });

  it("splits host:port typed into the server field", async () => {
    renderDialog(newManualNode());
    await chooseType("VLESS + REALITY");
    fireEvent.change(screen.getByLabelText("服务器"), { target: { value: "1.2.3.4:8443" } });
    expect(screen.getByLabelText("服务器")).toHaveValue("1.2.3.4");
    expect(screen.getByLabelText("端口")).toHaveValue("8443");
  });

  it("blocks saving a malformed public key or short id", async () => {
    vi.mocked(commands.parseShareLink).mockResolvedValue({ status: "ok", data: { raw: realityRaw, stripped: [] } });
    renderDialog(newManualNode());
    await pasteLink("vless://x");
    await waitFor(() => expect(screen.getByLabelText("UUID")).toHaveValue(realityRaw.uuid));
    fireEvent.change(screen.getByLabelText("Public Key"), { target: { value: "privateKeyPastedHere" } });
    expect(screen.getByText(/应为 43 位字符/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Public Key"), { target: { value: PBK } });
    fireEvent.change(screen.getByLabelText("Short ID"), { target: { value: "abc" } });
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  });

  it("opens an existing REALITY node in the form and keeps unknown fields", async () => {
    const node: ManualNode = { id: "r", name: "东京", groups: [], kind: { Proxy: { raw: { ...realityRaw, alpn: ["h2"] } } } };
    const onClose = renderDialog(node, undefined, [node]);
    expect(await screen.findByLabelText("Public Key")).toHaveValue(PBK);
    expect(screen.queryByLabelText("YAML")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /高级/ }));
    fireEvent.click(screen.getByLabelText("XTLS Vision 流控"));
    // An existing node without the flag keeps mihomo's default (off) until the user turns it on.
    expect(screen.getByLabelText(/X25519MLKEM768/)).not.toBeChecked();
    fireEvent.click(screen.getByLabelText(/X25519MLKEM768/));
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    const raw = rawOfSaved()!;
    expect((raw["reality-opts"] as Record<string, unknown>)["support-x25519mlkem768"]).toBe(true);
    expect(raw.alpn).toEqual(["h2"]);
    expect(raw["client-fingerprint"]).toBe("safari");
    expect(raw.flow).toBeUndefined();
  });
});
