import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import type { AiAppConflict, AiAppView, Model } from "../bindings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
    procrouteStatus: vi.fn(),
    saveProcessRoutingSettings: vi.fn(),
    aiAppsList: vi.fn(),
    enableAiApp: vi.fn(),
    disableAiApp: vi.fn(),
    launchAiApp: vi.fn(),
    aiAppConflicts: vi.fn(),
    testAiAppDelay: vi.fn(),
    openAiAppDownload: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { pages } from "../components/Layout";
import { AiAppsPage } from "./AiApps";

const model = {
  subscriptions: [],
  exits: [],
  listeners: [],
  settings: { delay_test_url: "u" },
  manual_nodes: [{ id: "m1", name: "东京", kind: { Proxy: { raw: "ss://x" } }, groups: [] }],
  node_groups: [],
  process_routing: { enabled: true, rules: [], exit_prompt: true },
} as unknown as Model;

const app = (over: Partial<AiAppView>): AiAppView => ({
  id: "claude",
  name: "Claude",
  icon: "data:image/png;base64,",
  download_url: "https://x",
  installed: true,
  running: false,
  rule_id: null,
  enabled: false,
  node: null,
  held: [],
  ...over,
});

function renderPage(apps: AiAppView[], enabled = true, actions: Record<string, unknown> = {}) {
  vi.mocked(commands.getModel).mockResolvedValue({
    ...model,
    process_routing: { ...model.process_routing, enabled },
  });
  vi.mocked(commands.aiAppsList).mockResolvedValue(apps);
  vi.mocked(commands.getStatus).mockResolvedValue({ core: "Running", delays: {}, interfaces: [] } as never);
  vi.mocked(commands.procrouteStatus).mockResolvedValue({ actions } as never);
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <ApplyResultProvider>
        <MemoryRouter>
          <AiAppsPage />
        </MemoryRouter>
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
}

let skip = false;
let invoked: { command: string; args: unknown }[];
const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const tokyo = { manual_id: "m1" };
const conflict = (over: Partial<AiAppConflict> = {}): AiAppConflict => ({
  rule_id: "w1", rule_name: "工作软件", process: "ChatGPT.exe", removes_rule: false, ...over,
});
const pickTokyo = async () => fireEvent.click(await screen.findByRole("radio", { name: /东京/ }));

describe("AiAppsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    skip = false;
    invoked = [];
    vi.mocked(invoke).mockImplementation(async (command, args) => {
      invoked.push({ command, args });
      if (command === "desktop_state") return { preferences: { mode: "Simple", skip_ai_reset_prompt: skip } };
      return undefined;
    });
    vi.mocked(commands.aiAppConflicts).mockResolvedValue(ok([]));
    vi.mocked(commands.enableAiApp).mockResolvedValue(ok({ NotRunning: null }) as never);
    vi.mocked(commands.disableAiApp).mockResolvedValue(ok({ NotRunning: null }) as never);
    vi.mocked(commands.launchAiApp).mockResolvedValue(ok("Launched"));
    vi.mocked(commands.openAiAppDownload).mockResolvedValue(ok(null));
  });

  it("在进程分流上方显示 AI 应用菜单", () => {
    const labels = pages.map((p) => p.label);
    expect(labels.indexOf("AI 应用")).toBeGreaterThanOrEqual(0);
    expect(labels.indexOf("AI 应用")).toBe(labels.indexOf("进程分流") - 1);
  });

  it("未安装的卡片只显示下载链接", async () => {
    renderPage([app({ installed: false })]);
    fireEvent.click(await screen.findByText("未安装 · 下载"));
    expect(commands.openAiAppDownload).toHaveBeenCalledWith("claude");
    // only the header switch remains
    expect(screen.getAllByRole("switch")).toHaveLength(1);
  });

  it("运行中的应用按钮为 运行中", async () => {
    renderPage([app({ running: true })]);
    expect(await screen.findByRole("button", { name: "运行中" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "启动" })).toBeNull();
  });

  it("总开关关闭时开着的开关有暂停提示且无横幅文字", async () => {
    renderPage([app({ enabled: true, rule_id: "r1", node: { manual_id: "m1" } })], false);
    const sw = await screen.findByRole("switch", { name: "Claude 代理开关" });
    fireEvent.mouseOver(sw);
    expect(await screen.findByText("进程分流已关闭，开启后恢复")).toBeInTheDocument();
    expect(screen.queryByText(/进程分流已关闭，以下应用/)).toBeNull();
    expect(screen.getByRole("button", { name: "启动" })).toBeEnabled();
  });

  it("页头总开关切换调用 saveProcessRoutingSettings", async () => {
    vi.mocked(commands.saveProcessRoutingSettings).mockResolvedValue({ status: "ok", data: null });
    renderPage([app({})], true);
    fireEvent.click(await screen.findByRole("switch", { name: "进程分流总开关" }));
    await waitFor(() => expect(commands.saveProcessRoutingSettings).toHaveBeenCalledWith(false));
  });

  it("右上角小圆点：生效为绿色，暂停为灰色，失败为红色", async () => {
    const a = app({ enabled: true, rule_id: "r1", node: { manual_id: "m1" } });
    const view = renderPage([a], true, { r1: { action: "Proxy", reason: "" } });
    await waitFor(() => expect(screen.getByTestId("ai-dot")).toHaveAttribute("data-state", "green"));
    view.unmount();
    const paused = renderPage([a], false, { r1: { action: "Proxy", reason: "" } });
    await screen.findByRole("switch", { name: "Claude 代理开关" });
    expect(screen.getByTestId("ai-dot")).toHaveAttribute("data-state", "gray");
    paused.unmount();
    renderPage([a], true, { r1: { action: "Failed", reason: "出错了" } });
    await waitFor(() => expect(screen.getByTestId("ai-dot")).toHaveAttribute("data-state", "red"));
  });

  it("有进程被手动规则占着时小圆点为黄色，悬停说明，点击走冲突确认后移交", async () => {
    const held = [conflict({ process: "Codex.exe" })];
    vi.mocked(commands.aiAppConflicts).mockResolvedValue(ok(held));
    const a = app({ enabled: true, rule_id: "r1", node: tokyo, held });
    renderPage([a], true, { r1: { action: "Proxy", reason: "" } });
    const dot = await screen.findByRole("button", { name: "把 Claude 的进程移交给 AI 应用" });
    await waitFor(() => expect(screen.getByTestId("ai-dot")).toHaveAttribute("data-state", "yellow"));
    fireEvent.mouseOver(dot);
    expect(await screen.findByText(/Codex\.exe 由进程分流规则「工作软件」管理/)).toBeInTheDocument();
    expect(screen.getByText(/点击移交给 AI 应用/)).toBeInTheDocument();
    fireEvent.click(dot);
    fireEvent.click(await screen.findByRole("button", { name: "移除并开启" }));
    await waitFor(() => expect(commands.enableAiApp).toHaveBeenCalledWith("claude", tokyo));
  });

  it("黄色小圆点：取消冲突确认则不移交；暂停时不显示黄色", async () => {
    const held = [conflict({ process: "Codex.exe" })];
    vi.mocked(commands.aiAppConflicts).mockResolvedValue(ok(held));
    const a = app({ enabled: true, rule_id: "r1", node: tokyo, held });
    const view = renderPage([a], true, { r1: { action: "Proxy", reason: "" } });
    fireEvent.click(await screen.findByRole("button", { name: "把 Claude 的进程移交给 AI 应用" }));
    fireEvent.click(await screen.findByRole("button", { name: "取消" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "移除并开启" })).toBeNull());
    expect(commands.enableAiApp).not.toHaveBeenCalled();
    view.unmount();
    renderPage([a], false, { r1: { action: "Proxy", reason: "" } });
    await screen.findByRole("switch", { name: "Claude 代理开关" });
    expect(screen.getByTestId("ai-dot")).toHaveAttribute("data-state", "gray");
    expect(screen.queryByRole("button", { name: "把 Claude 的进程移交给 AI 应用" })).toBeNull();
  });

  it("等待服务确认时小圆点为灰色", async () => {
    const a = app({ enabled: true, rule_id: "r1", node: { manual_id: "m1" } });
    renderPage([a], true, { r1: { action: "Pending", reason: "等待服务确认当前设置" } });
    await waitFor(() => expect(commands.procrouteStatus).toHaveBeenCalled());
    await new Promise((r) => setTimeout(r, 50));
    expect(screen.getByTestId("ai-dot")).toHaveAttribute("data-state", "gray");
  });

  it("打开下载页失败时显示错误文字", async () => {
    vi.mocked(commands.openAiAppDownload).mockResolvedValue({ status: "error", error: "无法打开浏览器: x" });
    renderPage([app({ installed: false })]);
    fireEvent.click(await screen.findByText("未安装 · 下载"));
    expect(await screen.findByText("无法打开浏览器: x")).toBeInTheDocument();
  });

  it("运行中：开关关着也只调用 launchAiApp，不走开启流程", async () => {
    renderPage([app({ running: true })]);
    fireEvent.click(await screen.findByRole("button", { name: "运行中" }));
    await waitFor(() => expect(commands.launchAiApp).toHaveBeenCalledWith("claude"));
    expect(commands.aiAppConflicts).not.toHaveBeenCalled();
    expect(commands.enableAiApp).not.toHaveBeenCalled();
    expect(screen.queryByRole("radio", { name: /东京/ })).toBeNull();
  });

  it("流程进行中禁用该卡片的开关、节点和主按钮，重复点击不会再启动一次", async () => {
    let finish!: () => void;
    vi.mocked(commands.launchAiApp).mockReturnValue(
      new Promise((r) => {
        finish = () => r(ok("Launched"));
      }),
    );
    renderPage([app({ rule_id: "r1", node: tokyo, enabled: true })]);
    const main = await screen.findByRole("button", { name: "启动" });
    fireEvent.click(main);
    await waitFor(() => expect(main).toBeDisabled());
    fireEvent.click(main);
    expect(screen.getByRole("switch", { name: "Claude 代理开关" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "东京" })).toBeDisabled();
    expect(commands.launchAiApp).toHaveBeenCalledTimes(1);
    finish();
    await waitFor(() => expect(main).toBeEnabled());
    expect(commands.launchAiApp).toHaveBeenCalledTimes(1);
  });

  it("总开关关闭时关开关、换节点都不弹断开框", async () => {
    renderPage([app({ rule_id: "r1", node: tokyo, enabled: true, running: true })], false);
    fireEvent.click(await screen.findByRole("switch", { name: "Claude 代理开关" }));
    await waitFor(() => expect(commands.disableAiApp).toHaveBeenCalledWith("claude"));
    fireEvent.click(screen.getByText("东京"));
    await pickTokyo();
    await waitFor(() =>
      expect(invoked.find((c) => c.command === "change_process_node")?.args).toEqual({ id: "r1", node: tokyo }),
    );
    expect(screen.queryByText("断开 Claude 的现有连接")).toBeNull();
  });

  it("首次开启先选节点，取消则不调用 enableAiApp", async () => {
    renderPage([app({})]);
    fireEvent.click(await screen.findByRole("switch", { name: "Claude 代理开关" }));
    await screen.findByRole("radio", { name: /东京/ });
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    await waitFor(() => expect(screen.queryByRole("radio", { name: /东京/ })).toBeNull());
    expect(commands.aiAppConflicts).not.toHaveBeenCalled();
    expect(commands.enableAiApp).not.toHaveBeenCalled();
  });

  it("选好节点后调用 enableAiApp", async () => {
    renderPage([app({})]);
    fireEvent.click(await screen.findByRole("switch", { name: "Claude 代理开关" }));
    await pickTokyo();
    await waitFor(() => expect(commands.enableAiApp).toHaveBeenCalledWith("claude", tokyo));
  });

  it("enableAiApp 失败时显示错误文字", async () => {
    vi.mocked(commands.enableAiApp).mockResolvedValue({ status: "error", error: "无法启用规则" });
    renderPage([app({ rule_id: "r1", node: tokyo })]);
    fireEvent.click(await screen.findByRole("switch", { name: "Claude 代理开关" }));
    expect(await screen.findByText("无法启用规则")).toBeInTheDocument();
  });

  it("有冲突时显示冲突框，确认后才开启", async () => {
    vi.mocked(commands.aiAppConflicts).mockResolvedValue(ok([conflict()]));
    renderPage([app({ rule_id: "r1", node: tokyo })]);
    fireEvent.click(await screen.findByRole("switch", { name: "Claude 代理开关" }));
    expect(await screen.findByText("与进程分流规则冲突")).toBeInTheDocument();
    expect(screen.getByText("规则「工作软件」里的 ChatGPT.exe 也在分流。")).toBeInTheDocument();
    expect(screen.getByText("开启后会把它从该规则中移除，规则里的其他进程不受影响。")).toBeInTheDocument();
    expect(commands.enableAiApp).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "移除并开启" }));
    await waitFor(() => expect(commands.enableAiApp).toHaveBeenCalledWith("claude", tokyo));
  });

  it("冲突框取消则不开启", async () => {
    vi.mocked(commands.aiAppConflicts).mockResolvedValue(ok([conflict()]));
    renderPage([app({ rule_id: "r1", node: tokyo })]);
    fireEvent.click(await screen.findByRole("switch", { name: "Claude 代理开关" }));
    fireEvent.click(await screen.findByRole("button", { name: "取消" }));
    await waitFor(() => expect(screen.queryByText("与进程分流规则冲突")).toBeNull());
    expect(commands.enableAiApp).not.toHaveBeenCalled();
  });

  it("整条规则会被删除时显示 开启后会删除该规则。", async () => {
    vi.mocked(commands.aiAppConflicts).mockResolvedValue(ok([conflict({ removes_rule: true })]));
    renderPage([app({ rule_id: "r1", node: tokyo })]);
    fireEvent.click(await screen.findByRole("switch", { name: "Claude 代理开关" }));
    expect(await screen.findByText("开启后会删除该规则。")).toBeInTheDocument();
    expect(screen.queryByText(/其他进程不受影响/)).toBeNull();
  });

  it("应用运行中开启时显示断开框；勾选下次不再提示后保存 skip_ai_reset_prompt=true", async () => {
    renderPage([app({ rule_id: "r1", node: tokyo, running: true })]);
    await waitFor(() => expect(invoked.some((c) => c.command === "desktop_state")).toBe(true));
    fireEvent.click(await screen.findByRole("switch", { name: "Claude 代理开关" }));
    expect(await screen.findByText("断开 Claude 的现有连接")).toBeInTheDocument();
    expect(screen.getByText("会断开 Claude 已有的连接，让它重新连接。应用可能会短暂显示重连。")).toBeInTheDocument();
    expect(commands.enableAiApp).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("checkbox", { name: "下次不再提示" }));
    fireEvent.click(screen.getByRole("button", { name: "继续" }));
    await waitFor(() => expect(commands.enableAiApp).toHaveBeenCalledWith("claude", tokyo));
    await waitFor(() =>
      expect(invoked.find((c) => c.command === "save_ui_preferences")?.args).toEqual({
        input: { mode: "Simple", skip_ai_reset_prompt: true },
      }),
    );
  });

  it("断开框取消则不开启", async () => {
    renderPage([app({ rule_id: "r1", node: tokyo, running: true })]);
    fireEvent.click(await screen.findByRole("switch", { name: "Claude 代理开关" }));
    fireEvent.click(await screen.findByRole("button", { name: "取消" }));
    await waitFor(() => expect(screen.queryByText("断开 Claude 的现有连接")).toBeNull());
    expect(commands.enableAiApp).not.toHaveBeenCalled();
  });

  it("skip_ai_reset_prompt=true 时不显示断开框", async () => {
    skip = true;
    renderPage([app({ rule_id: "r1", node: tokyo, running: true })]);
    await waitFor(() => expect(invoked.some((c) => c.command === "desktop_state")).toBe(true));
    await new Promise((r) => setTimeout(r, 50));
    fireEvent.click(await screen.findByRole("switch", { name: "Claude 代理开关" }));
    await waitFor(() => expect(commands.enableAiApp).toHaveBeenCalledWith("claude", tokyo));
    expect(screen.queryByText("断开 Claude 的现有连接")).toBeNull();
  });

  it("关闭开关在运行中也弹断开框", async () => {
    renderPage([app({ rule_id: "r1", node: tokyo, enabled: true, running: true })]);
    fireEvent.click(await screen.findByRole("switch", { name: "Claude 代理开关" }));
    expect(await screen.findByText("断开 Claude 的现有连接")).toBeInTheDocument();
    expect(commands.disableAiApp).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "继续" }));
    await waitFor(() => expect(commands.disableAiApp).toHaveBeenCalledWith("claude"));
  });

  it("换节点在运行中也弹断开框，确认后才调用 changeNode", async () => {
    renderPage([app({ rule_id: "r1", node: tokyo, enabled: true, running: true })]);
    fireEvent.click(await screen.findByText("东京"));
    await pickTokyo();
    expect(await screen.findByText("断开 Claude 的现有连接")).toBeInTheDocument();
    expect(invoked.some((c) => c.command === "change_process_node")).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "继续" }));
    await waitFor(() =>
      expect(invoked.find((c) => c.command === "change_process_node")?.args).toEqual({ id: "r1", node: tokyo }),
    );
  });

  it("启动：未启用时先走开启流程再调用 launchAiApp", async () => {
    vi.mocked(commands.aiAppConflicts).mockResolvedValue(ok([conflict()]));
    renderPage([app({ rule_id: "r1", node: tokyo })]);
    fireEvent.click(await screen.findByRole("button", { name: "启动" }));
    fireEvent.click(await screen.findByRole("button", { name: "移除并开启" }));
    await waitFor(() => expect(commands.launchAiApp).toHaveBeenCalledWith("claude"));
    expect(vi.mocked(commands.enableAiApp).mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(commands.launchAiApp).mock.invocationCallOrder[0],
    );
  });

  it("启动：规则已启用时直接调用 launchAiApp", async () => {
    renderPage([app({ rule_id: "r1", node: tokyo, enabled: true })]);
    fireEvent.click(await screen.findByRole("button", { name: "启动" }));
    await waitFor(() => expect(commands.launchAiApp).toHaveBeenCalledWith("claude"));
    expect(commands.enableAiApp).not.toHaveBeenCalled();
  });
});
