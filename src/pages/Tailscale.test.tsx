import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ManualNode, Model, RuntimeStatus, TailnetPeer, TailscaleOpts, TailscaleState } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
    saveManualNode: vi.fn(),
    deleteManualNode: vi.fn(),
    saveExit: vi.fn(),
    retryTailscaleExitNodes: vi.fn(),
    testDelays: vi.fn(),
    tailnetPeers: vi.fn(),
    openLoginUrl: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { CHECK_RETRY_MS, EXIT_NODE_MAX_RETRIES } from "../components/TailscaleState";
import { TAILSCALE_EMPTY_HELP, Tailscale } from "./Tailscale";
import { NO_EXIT_NODE_HELP, newTailscaleNode } from "./TailscaleDialog";

const exitPeer: TailnetPeer = {
  host_name: "VM-0-5-opencloudos",
  dns_name: "vm-0-5-opencloudos.tailb4bf67.ts.net",
  ips: ["100.91.166.56"],
  online: true,
  exit_node_option: true,
};
const selfPeer: TailnetPeer = {
  host_name: "mihomo-家里",
  dns_name: "mihomo-jia-li.tailb4bf67.ts.net",
  ips: ["100.121.28.62"],
  online: true,
  exit_node_option: false,
};

const tsNode: ManualNode = {
  id: "t1",
  name: "家里",
  groups: [],
  kind: { Tailscale: { hostname: "mihomo-家里", exit_node: "100.91.166.56", ephemeral: false, udp: true, auth_key: null, interface: "g-eth" } },
};
const model: Model = {
  subscriptions: [],
  exits: [{ id: "e1", name: "出口甲", interface: null, kind: { Group: { strategy: "Select", members: [{ manual_id: "t1" }], selected: null } } }],
  listeners: [],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [tsNode],
  node_groups: [],
};
const status = {
  health: { exits: {}, listeners: {} },
  delays: {},
  interfaces: [{ guid: "g-eth", friendly_name: "以太网", is_up: true, ipv4: ["192.168.1.2"], ipv6: [] }],
  tailscale: {},
  tailscale_exit_errors: {},
} as unknown as RuntimeStatus;

const withState = (s: TailscaleState, name = "手动/家里", extra: Partial<RuntimeStatus> = {}) =>
  ({ ...status, tailscale: { [name]: s }, ...extra }) as RuntimeStatus;
const tsOpts = (tsNode.kind as { Tailscale: TailscaleOpts }).Tailscale;
const withOpts = (o: Partial<TailscaleOpts>): Model => ({
  ...model,
  manual_nodes: [{ ...tsNode, kind: { Tailscale: { ...tsOpts, ...o } } }],
});

function renderPage(m: Model = model, st: RuntimeStatus = status, peers: TailnetPeer[] | string = [exitPeer, selfPeer]) {
  vi.mocked(commands.getModel).mockResolvedValue(m);
  vi.mocked(commands.getStatus).mockResolvedValue(st);
  vi.mocked(commands.saveManualNode).mockResolvedValue({ status: "ok", data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } });
  vi.mocked(commands.tailnetPeers).mockResolvedValue(typeof peers === "string" ? { status: "error", error: peers } : { status: "ok", data: peers });
  vi.mocked(commands.openLoginUrl).mockResolvedValue({ status: "ok", data: null });
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <ApplyResultProvider>
        <Tailscale />
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
  return qc;
}

const saved = (i: number) => vi.mocked(commands.saveManualNode).mock.calls[i][0];

beforeEach(() => vi.clearAllMocks());

describe("newTailscaleNode", () => {
  it("starts without an exit node and not ephemeral", () => {
    expect(newTailscaleNode().kind).toEqual({
      Tailscale: { hostname: "mihomo-", exit_node: "", ephemeral: false, udp: true, auth_key: null, interface: null },
    });
  });
});

describe("Tailscale page card", () => {
  it("shows the exit node by name, references and details", async () => {
    renderPage();
    const card = await screen.findByRole("article", { name: "家里" });
    expect(within(card).getByText("被出口引用：出口甲")).toBeInTheDocument();
    expect(await within(card).findByText(/机器名 mihomo-家里 · 网卡 以太网 · UDP 开/)).toBeInTheDocument();
    expect(within(card).getByText("状态未知")).toBeInTheDocument();
    // Peers are only read once logged in; before that the raw value shows.
    expect(within(card).getByText("出口节点：100.91.166.56")).toBeInTheDocument();
  });

  it("NeedsLogin with a URL opens it in the browser and copies it", async () => {
    const url = "https://login.tailscale.com/a/1654b85a0197ca";
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    renderPage(model, withState({ NeedsLogin: { url } }));
    const card = await screen.findByRole("article", { name: "家里" });
    expect(await within(card).findByText("待登录")).toBeInTheDocument();
    expect(within(card).getByText(url)).toBeInTheDocument();
    fireEvent.click(within(card).getByRole("button", { name: "在浏览器中打开" }));
    await waitFor(() => expect(commands.openLoginUrl).toHaveBeenCalledWith(url));
    fireEvent.click(within(card).getByRole("button", { name: "复制链接" }));
    expect(writeText).toHaveBeenCalledWith(url);
    expect(await within(card).findByRole("button", { name: "已复制" })).toBeInTheDocument();
  });

  it("tells the user to copy the link when the browser cannot be opened", async () => {
    renderPage(model, withState({ NeedsLogin: { url: "https://login.tailscale.com/a/abc" } }));
    vi.mocked(commands.openLoginUrl).mockResolvedValue({ status: "error", error: "无法打开浏览器: x" });
    const card = await screen.findByRole("article", { name: "家里" });
    fireEvent.click(await within(card).findByRole("button", { name: "在浏览器中打开" }));
    expect(await within(card).findByText("无法打开浏览器: x，请复制链接后在浏览器中打开")).toBeInTheDocument();
  });

  it("shows a visible message when copying fails", async () => {
    Object.defineProperty(navigator, "clipboard", { value: undefined, configurable: true });
    vi.spyOn(console, "error").mockImplementation(() => {});
    renderPage(model, withState({ NeedsLogin: { url: "https://login.tailscale.com/a/abc" } }));
    const card = await screen.findByRole("article", { name: "家里" });
    fireEvent.click(await within(card).findByRole("button", { name: "复制链接" }));
    expect(await within(card).findByText("复制失败，请手动选择链接")).toBeInTheDocument();
  });

  it("NeedsLogin without a URL says the link is coming", async () => {
    renderPage(model, withState({ NeedsLogin: { url: null } }));
    const card = await screen.findByRole("article", { name: "家里" });
    expect(await within(card).findByText("正在获取授权链接…若长时间没有出现，请检查网络或该实例的网卡设置")).toBeInTheDocument();
    expect(within(card).queryByRole("button", { name: "在浏览器中打开" })).not.toBeInTheDocument();
  });

  it("logged in: IP and exit node name come from the system client", async () => {
    renderPage(model, withState({ LoggedIn: { ip: null } }));
    const card = await screen.findByRole("article", { name: "家里" });
    expect(await within(card).findByText("已登录 · 100.121.28.62")).toBeInTheDocument();
    expect(within(card).getByText("出口节点：VM-0-5-opencloudos · 100.91.166.56")).toBeInTheDocument();
  });

  it("logged in without a known IP", async () => {
    renderPage(model, withState({ LoggedIn: { ip: null } }), "本机没有安装 Tailscale 客户端");
    const card = await screen.findByRole("article", { name: "家里" });
    expect(await within(card).findByText("已登录")).toBeInTheDocument();
  });

  it("shows the login failure reason", async () => {
    renderPage(model, withState({ Failed: { reason: "无法连接控制服务器" } }));
    const card = await screen.findByRole("article", { name: "家里" });
    expect(await within(card).findByText("登录失败：无法连接控制服务器")).toBeInTheDocument();
  });

  const failing = (retrying: number | null) =>
    withState({ LoggedIn: { ip: null } }, "手动/家里", {
      tailscale_exit_errors: { "手动/家里": "no node found in netmap with IP 100.91.166.56" },
      tailscale_exit_retrying: retrying === null ? {} : { "手动/家里": retrying },
    });

  it("shows the automatic exit node retry in progress", async () => {
    renderPage(model, failing(2));
    const card = await screen.findByRole("article", { name: "家里" });
    expect(await within(card).findByText(`出口节点这次没设置上，正在自动重启实例重试（第 2/${EXIT_NODE_MAX_RETRIES} 次）…`)).toBeInTheDocument();
    expect(within(card).queryByRole("button", { name: "重试" })).not.toBeInTheDocument();
  });

  it("after the automatic retries: says why, retries on demand or changes the exit node", async () => {
    renderPage(model, failing(null));
    vi.mocked(commands.retryTailscaleExitNodes).mockResolvedValue({ status: "ok", data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } });
    const card = await screen.findByRole("article", { name: "家里" });
    expect(await within(card).findByText(/出口节点没有生效：no node found in netmap with IP 100.91.166.56。内核启动实例时偶尔会设置失败/)).toBeInTheDocument();
    fireEvent.click(within(card).getByRole("button", { name: "重试" }));
    await waitFor(() => expect(commands.retryTailscaleExitNodes).toHaveBeenCalled());
    fireEvent.click(within(card).getByRole("button", { name: "更换出口节点" }));
    expect(await screen.findByText("编辑 Tailscale 实例")).toBeInTheDocument();
  });

  it("warns about auto:any", async () => {
    renderPage(withOpts({ exit_node: "auto:any" }));
    const card = await screen.findByRole("article", { name: "家里" });
    expect(within(card).getByText("auto:any 在当前内核下无法自动选中出口节点，请改选一个出口节点")).toBeInTheDocument();
  });

  it("says a logged-in instance without exit node only reaches the tailnet", async () => {
    renderPage(withOpts({ exit_node: "" }), withState({ LoggedIn: { ip: null } }));
    const card = await screen.findByRole("article", { name: "家里" });
    expect(await within(card).findByText("未设置出口节点，只能访问 tailnet 内的设备")).toBeInTheDocument();
    expect(within(card).getByText("出口节点：不使用（只访问 tailnet 内设备）")).toBeInTheDocument();
  });

  it("tests the delay and shows the result", async () => {
    vi.mocked(commands.testDelays).mockResolvedValue({ status: "ok", data: [["手动/家里", { ms: 210, tested_at: 1, error: null }]] });
    renderPage(model, withState({ LoggedIn: { ip: null } }, "手动/家里", { delays: { "手动/家里": { ms: null, tested_at: 0, error: "Failed" } } }));
    const card = await screen.findByRole("article", { name: "家里" });
    expect(await within(card).findByText("连接失败")).toBeInTheDocument();
    fireEvent.click(within(card).getByRole("button", { name: "测速" }));
    await waitFor(() => expect(commands.testDelays).toHaveBeenCalledWith(["手动/家里"]));
  });

  it("creates an exit for the instance", async () => {
    renderPage();
    const card = await screen.findByRole("article", { name: "家里" });
    fireEvent.click(within(card).getByRole("button", { name: "用它新建出口" }));
    expect(await screen.findByText("新建出口")).toBeInTheDocument();
    expect(screen.getByLabelText("名称")).toHaveValue("家里");
  });

  it("shows the DanglingRef error when deleting a referenced instance", async () => {
    renderPage();
    vi.mocked(commands.deleteManualNode).mockResolvedValue({ status: "error", error: [{ DanglingRef: { what: "出口「出口甲」" } }] });
    fireEvent.click(await screen.findByRole("button", { name: "操作 家里" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "删除 家里" }));
    fireEvent.click(await screen.findByRole("button", { name: "删除" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("出口甲");
  });

  it("shows an empty state without instances", async () => {
    renderPage({ ...model, manual_nodes: [], exits: [] });
    expect(await screen.findByText("还没有 Tailscale 实例")).toBeInTheDocument();
    expect(screen.getByText(TAILSCALE_EMPTY_HELP)).toBeInTheDocument();
  });
});

describe("edit dialog", () => {
  const openEdit = async () => {
    fireEvent.click(await screen.findByRole("button", { name: "操作 家里" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "编辑 家里" }));
    await screen.findByText("编辑 Tailscale 实例");
  };

  it("keeps the hostname when renamed and warns that changing it registers a new machine", async () => {
    renderPage();
    await openEdit();
    fireEvent.change(screen.getByLabelText("名称"), { target: { value: "办公室" } });
    fireEvent.click(screen.getByRole("button", { name: "高级选项" }));
    expect(screen.getByLabelText("tailnet 中的机器名")).toHaveValue("mihomo-家里");
    expect(screen.getByText("修改后会在 tailnet 中注册为一台新机器，需要重新登录")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(commands.saveManualNode).toHaveBeenCalled());
    expect(saved(0).name).toBe("办公室");
    expect(saved(0).kind).toMatchObject({ Tailscale: { hostname: "mihomo-家里", exit_node: "100.91.166.56" } });
  });

  it("picks another exit node from the tailnet list", async () => {
    renderPage();
    await openEdit();
    fireEvent.click(await screen.findByLabelText("不使用出口节点（只访问 tailnet 内设备）"));
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(commands.saveManualNode).toHaveBeenCalled());
    expect(saved(0).kind).toMatchObject({ Tailscale: { exit_node: "" } });
  });

  it("refuses auto:any typed by hand", async () => {
    renderPage(model, status, "本机没有安装 Tailscale 客户端");
    await openEdit();
    expect(await screen.findByText(/无法列出 tailnet 中的设备：本机没有安装 Tailscale 客户端/)).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("exit-node"), { target: { value: "auto:any" } });
    expect(screen.getByText("auto:any 在当前内核下无法自动选中出口节点，请改选一个出口节点")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  });
});

describe("new instance wizard", () => {
  const loggedIn = withState({ LoggedIn: { ip: null } });

  async function throughLogin(qc: QueryClient) {
    fireEvent.click(await screen.findByRole("button", { name: "新建实例" }));
    fireEvent.change(await screen.findByLabelText("名称"), { target: { value: "家里" } });
    expect(screen.getByText("在 tailnet 中显示为 mihomo-家里")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "保存并登录" }));
    await waitFor(() => expect(commands.saveManualNode).toHaveBeenCalledTimes(1));
    expect(saved(0).kind).toMatchObject({ Tailscale: { hostname: "mihomo-家里", exit_node: "", ephemeral: false } });
    expect(await screen.findByRole("button", { name: "稍后登录" })).toBeInTheDocument();
    act(() => qc.setQueryData(["status"], withState({ NeedsLogin: { url: "https://login.tailscale.com/a/x1" } })));
    expect(await screen.findByRole("button", { name: "在浏览器中打开" })).toBeInTheDocument();
    act(() => qc.setQueryData(["status"], loggedIn));
    expect(await screen.findByText("VM-0-5-opencloudos")).toBeInTheDocument();
  }

  it("names, logs in, applies the exit node, verifies and creates an exit", async () => {
    vi.mocked(commands.testDelays).mockResolvedValue({ status: "ok", data: [["手动/家里", { ms: 210, tested_at: 1, error: null }]] });
    const qc = renderPage({ ...model, manual_nodes: [], exits: [] });
    await throughLogin(qc);
    fireEvent.click(screen.getByText("VM-0-5-opencloudos"));
    fireEvent.click(screen.getByRole("button", { name: "应用并测速" }));
    await waitFor(() => expect(commands.saveManualNode).toHaveBeenCalledTimes(2));
    expect(saved(1).kind).toMatchObject({ Tailscale: { exit_node: "100.91.166.56" } });
    expect(await screen.findByText("出口节点已生效，测速 210 ms")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "用它新建出口" }));
    expect(await screen.findByText("新建出口")).toBeInTheDocument();
  });

  it("keeps checking while the backend retries, stops once it gave up", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const failed = (retrying: Record<string, number>) => ({
      ...loggedIn,
      tailscale_exit_errors: { "手动/家里": "no node found in netmap" },
      tailscale_exit_retrying: retrying,
    });
    let call = 0;
    vi.mocked(commands.testDelays).mockImplementation(async () => {
      call++;
      // First test: a retry is under way. Second: retries used up.
      qc.setQueryData(["status"], failed(call === 1 ? { "手动/家里": 1 } : {}));
      return { status: "ok", data: [["手动/家里", { ms: null, tested_at: 1, error: "Failed" }]] };
    });
    const qc = renderPage({ ...model, manual_nodes: [], exits: [] });
    await throughLogin(qc);
    fireEvent.click(screen.getByText("VM-0-5-opencloudos"));
    try {
      fireEvent.click(screen.getByRole("button", { name: "应用并测速" }));
      expect(await screen.findByText(new RegExp(`正在自动重启实例重试（第 1/${EXIT_NODE_MAX_RETRIES} 次）`))).toBeInTheDocument();
      await act(() => vi.advanceTimersByTimeAsync(CHECK_RETRY_MS));
      expect(await screen.findByText(/出口节点没有生效：no node found in netmap/)).toBeInTheDocument();
      expect(commands.testDelays).toHaveBeenCalledTimes(2);
      expect(screen.getByRole("button", { name: "重新应用并测速" })).toBeInTheDocument();
      // The wizard has its own retry button; the alert offers none.
      expect(screen.queryByRole("button", { name: "重试" })).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("retries while the instance cold-starts", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      vi.mocked(commands.testDelays)
        .mockResolvedValueOnce({ status: "ok", data: [["手动/家里", { ms: null, tested_at: 1, error: "Timeout" }]] })
        .mockResolvedValueOnce({ status: "ok", data: [["手动/家里", { ms: 199, tested_at: 2, error: null }]] });
      const qc = renderPage({ ...model, manual_nodes: [], exits: [] });
      await throughLogin(qc);
      fireEvent.click(screen.getByText("VM-0-5-opencloudos"));
      fireEvent.click(screen.getByRole("button", { name: "应用并测速" }));
      expect(await screen.findByText(/正在验证（\d+ 秒）/)).toBeInTheDocument();
      await act(() => vi.advanceTimersByTimeAsync(CHECK_RETRY_MS));
      expect(await screen.findByText("出口节点已生效，测速 199 ms")).toBeInTheDocument();
      expect(commands.testDelays).toHaveBeenCalledTimes(2);
    } finally {
      vi.useRealTimers();
    }
  });

  it("finishes without an exit node and without testing", async () => {
    const qc = renderPage({ ...model, manual_nodes: [], exits: [] });
    await throughLogin(qc);
    fireEvent.click(screen.getByLabelText("不使用出口节点（只访问 tailnet 内设备）"));
    expect(screen.getByText(NO_EXIT_NODE_HELP)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "完成" }));
    await waitFor(() => expect(screen.queryByText("新建 Tailscale 实例")).not.toBeInTheDocument());
    // Step 1 already saved "no exit node": nothing more to save.
    expect(commands.saveManualNode).toHaveBeenCalledTimes(1);
    expect(commands.testDelays).not.toHaveBeenCalled();
  });

  it("hostname follows the name until edited by hand; ephemeral is explained", async () => {
    renderPage();
    fireEvent.click(await screen.findByRole("button", { name: "新建实例" }));
    fireEvent.click(await screen.findByRole("button", { name: "高级选项" }));
    fireEvent.change(screen.getByLabelText("名称"), { target: { value: "x" } });
    expect(screen.getByLabelText("tailnet 中的机器名")).toHaveValue("mihomo-x");
    fireEvent.change(screen.getByLabelText("tailnet 中的机器名"), { target: { value: "custom" } });
    fireEvent.change(screen.getByLabelText("名称"), { target: { value: "y" } });
    expect(screen.getByLabelText("tailnet 中的机器名")).toHaveValue("custom");
    expect(screen.getByText(/建议关闭：临时节点离线一段时间后会被 tailnet 自动删除/)).toBeInTheDocument();
  });

  it("requires the auth key when that login method is chosen", async () => {
    renderPage();
    fireEvent.click(await screen.findByRole("button", { name: "新建实例" }));
    fireEvent.change(await screen.findByLabelText("名称"), { target: { value: "k" } });
    fireEvent.click(screen.getByLabelText("使用 auth key"));
    expect(screen.getByRole("button", { name: "保存并登录" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("auth key"), { target: { value: "tskey-auth-1" } });
    fireEvent.click(screen.getByRole("button", { name: "保存并登录" }));
    await waitFor(() => expect(commands.saveManualNode).toHaveBeenCalled());
    expect(saved(0).kind).toMatchObject({ Tailscale: { auth_key: "tskey-auth-1" } });
    expect(await screen.findByText("正在使用 auth key 登录…")).toBeInTheDocument();
  });
});
