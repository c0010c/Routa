import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { AiAppView, Model } from "../bindings";

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

describe("AiAppsPage", () => {
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
});
