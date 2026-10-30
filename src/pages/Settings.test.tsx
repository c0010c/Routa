import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  fireEvent,
  act,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { ApplyOutcome, Model } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    saveSettings: vi.fn(),
    coreRestart: vi.fn(),
    openLogDir: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { Settings } from "./Settings";
import { SimpleSettings } from "./SimpleSettings";

const model: Model = {
  subscriptions: [],
  exits: [],
  listeners: [],
  settings: {
    autostart: false,
    core_path: null,
    controller_port: 19097,
    delay_test_interval_mins: 10,
    delay_test_url: "u",
  },
  manual_nodes: [],
  node_groups: [],
  process_routing: { enabled: false, on_routa_exit: "Block", rules: [] },
};

function renderPage(initial = model, simple = false) {
  vi.mocked(commands.getModel).mockResolvedValue(initial);
  vi.mocked(commands.saveSettings).mockResolvedValue({
    status: "ok",
    data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } },
  });
  vi.mocked(commands.coreRestart).mockResolvedValue({
    status: "ok",
    data: null,
  });
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <ApplyResultProvider>
        <MemoryRouter>
          {simple ? <SimpleSettings /> : <Settings />}
        </MemoryRouter>
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
}

describe("Settings", () => {
  beforeEach(() => vi.clearAllMocks());

  it("keeps only core parameters as explicit drafts", async () => {
    renderPage();
    const url = await screen.findByLabelText("测速 URL");
    expect(screen.getByRole("button", { name: "保存并重启" })).toBeDisabled();
    expect(screen.queryByText("已保存")).not.toBeInTheDocument();
    fireEvent.change(url, { target: { value: "https://x.test" } });
    expect(screen.getByRole("button", { name: "保存并重启" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("控制端口"), { target: { value: "19100" } });
    expect(screen.getByRole("button", { name: "保存并重启" })).toBeEnabled();
    expect(screen.getByText("有未保存的修改")).toBeInTheDocument();
  });

  it("reports the actual apply result after a port change, without restarting itself", async () => {
    renderPage();
    fireEvent.change(await screen.findByLabelText("控制端口"), {
      target: { value: "19100" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存并重启" }));
    expect(await screen.findByText("已保存并生效")).toBeInTheDocument();
    expect(commands.saveSettings).toHaveBeenCalledWith(
      expect.objectContaining({ controller_port: 19100 }),
    );
    await waitFor(() => expect(commands.saveSettings).toHaveBeenCalledTimes(1));
    expect(commands.coreRestart).not.toHaveBeenCalled();
  });

  it("saves URL on blur without an explicit save or restart", async () => {
    renderPage();
    fireEvent.change(await screen.findByLabelText("测速 URL"), {
      target: { value: "https://x.test" },
    });
    expect(commands.saveSettings).not.toHaveBeenCalled();
    fireEvent.blur(screen.getByLabelText("测速 URL"));
    expect(await screen.findByText("已保存")).toBeInTheDocument();
    expect(commands.saveSettings).toHaveBeenCalledWith({ ...model.settings, delay_test_url: "https://x.test" });
    expect(screen.getByRole("button", { name: "保存并重启" })).toBeDisabled();
    expect(commands.coreRestart).not.toHaveBeenCalled();
  });

  it("isolates an invalid core draft from immediate saves and keeps later saves current", async () => {
    renderPage();
    fireEvent.change(await screen.findByLabelText("控制端口"), { target: { value: "99999" } });
    fireEvent.click(screen.getByRole("switch", { name: "开机自启" }));
    await screen.findByText("已保存");
    expect(commands.saveSettings).toHaveBeenLastCalledWith({ ...model.settings, autostart: true });
    expect(screen.getByLabelText("控制端口")).toHaveValue(99999);
    const url = screen.getByLabelText("测速 URL");
    fireEvent.change(url, { target: { value: "https://new.test" } });
    fireEvent.keyDown(url, { key: "Enter" });
    await waitFor(() => expect(commands.saveSettings).toHaveBeenCalledTimes(2));
    expect(commands.saveSettings).toHaveBeenLastCalledWith({ ...model.settings, autostart: true, delay_test_url: "https://new.test" });
    await waitFor(() => expect(screen.getByRole("button", { name: "保存并重启" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "保存并重启" }));
    expect(screen.getByText("端口必须在 1–65535 之间")).toBeInTheDocument();
    expect(commands.saveSettings).toHaveBeenCalledTimes(2);
    fireEvent.change(screen.getByLabelText("控制端口"), { target: { value: "19100" } });
    fireEvent.click(screen.getByRole("button", { name: "保存并重启" }));
    await screen.findByText("已保存并生效");
    expect(commands.saveSettings).toHaveBeenLastCalledWith({ ...model.settings, autostart: true, delay_test_url: "https://new.test", controller_port: 19100 });
  });

  it("locks immediate and explicit submissions until the request completes", async () => {
    renderPage();
    const autostart = await screen.findByRole("switch", { name: "开机自启" });
    let finish!: (result: Awaited<ReturnType<typeof commands.saveSettings>>) => void;
    vi.mocked(commands.saveSettings).mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    fireEvent.change(screen.getByLabelText("控制端口"), { target: { value: "19100" } });
    fireEvent.click(autostart);
    expect(screen.getByText("保存中…")).toBeInTheDocument();
    expect(autostart).toBeDisabled();
    expect(screen.getByLabelText("测速 URL")).toBeDisabled();
    expect(screen.getByRole("button", { name: "保存并重启" })).toBeDisabled();
    fireEvent.click(autostart);
    fireEvent.click(screen.getByRole("button", { name: "保存并重启" }));
    await waitFor(() => expect(commands.saveSettings).toHaveBeenCalledTimes(1));
    await act(async () => finish({ status: "ok", data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } }));
    expect(await screen.findByText("已保存")).toBeInTheDocument();
    expect(autostart).toBeChecked();
    expect(screen.getByRole("button", { name: "保存并重启" })).toBeEnabled();
  });

  it("shows a local autostart failure, keeps the committed value, and lets the user retry", async () => {
    renderPage();
    const autostart = await screen.findByRole("switch", { name: "开机自启" });
    vi.mocked(commands.saveSettings).mockResolvedValueOnce({ status: "error", error: "开机自启未能保存，请重试" });
    fireEvent.click(autostart);
    const general = screen.getByRole("region", { name: "常规" });
    expect(await within(general).findByRole("alert")).toHaveTextContent("开机自启未能保存，请重试");
    expect(autostart).not.toBeChecked();
    expect(screen.queryByText("已保存")).not.toBeInTheDocument();
    fireEvent.click(autostart);
    expect(await within(general).findByText("已保存")).toBeInTheDocument();
    expect(within(general).queryByRole("alert")).not.toBeInTheDocument();
    expect(commands.saveSettings).toHaveBeenCalledTimes(2);
  });

  it("saves interval switches immediately and commits valid text on blur", async () => {
    renderPage();
    const delay = await screen.findByRole("region", { name: "测速" });
    fireEvent.click(within(delay).getByRole("switch", { name: "启用" }));
    await within(delay).findByText("已保存");
    expect(commands.saveSettings).toHaveBeenLastCalledWith({ ...model.settings, delay_test_interval_mins: null });
    fireEvent.click(within(delay).getByRole("switch", { name: "启用" }));
    await waitFor(() => expect(within(delay).getByLabelText("间隔")).toBeEnabled());
    fireEvent.change(within(delay).getByLabelText("间隔"), { target: { value: "15" } });
    expect(commands.saveSettings).toHaveBeenCalledTimes(2);
    fireEvent.blur(within(delay).getByLabelText("间隔"));
    await waitFor(() => expect(commands.saveSettings).toHaveBeenCalledTimes(3));
    expect(commands.saveSettings).toHaveBeenLastCalledWith({ ...model.settings, delay_test_interval_mins: 15 });
  });

  it("restores the interval control after a failed immediate save", async () => {
    renderPage();
    const delay = await screen.findByRole("region", { name: "测速" });
    vi.mocked(commands.saveSettings).mockRejectedValueOnce(new Error("internal: failed"));
    fireEvent.click(within(delay).getByRole("switch", { name: "启用" }));
    expect(await within(delay).findByText("未能保存，请重试")).toBeInTheDocument();
    expect(within(delay).getByRole("switch", { name: "启用" })).toBeChecked();
    expect(within(delay).getByLabelText("间隔")).toHaveValue(10);
    expect(screen.queryByText("已保存")).not.toBeInTheDocument();
  });

  it("fades only the success feedback without shifting the control", async () => {
    renderPage();
    const autostart = await screen.findByRole("switch", { name: "开机自启" });
    vi.useFakeTimers();
    try {
      await act(async () => {
        fireEvent.click(autostart);
        await vi.advanceTimersByTimeAsync(50);
      });
      expect(screen.getByText("已保存")).toBeInTheDocument();
      await act(async () => { await vi.advanceTimersByTimeAsync(2500); });
      expect(screen.queryByText("已保存")).not.toBeInTheDocument();
      expect(autostart).toBeChecked();
      expect(screen.getByRole("region", { name: "常规" }).querySelector('[role="status"]')).toBeInTheDocument();
    } finally { vi.useRealTimers(); }
  });

  it("uses the same immediate feedback for autostart in simple mode", async () => {
    renderPage(model, true);
    fireEvent.click(await screen.findByRole("switch", { name: "开机启动" }));
    expect(await screen.findByText("已保存")).toBeInTheDocument();
    expect(commands.saveSettings).toHaveBeenCalledWith({ ...model.settings, autostart: true });
    expect(screen.queryByRole("button", { name: "保存并重启" })).not.toBeInTheDocument();
  });

  it("shows backend port conflicts on the field and waits for a manual resubmission", async () => {
    renderPage();
    vi.mocked(commands.saveSettings).mockResolvedValueOnce({
      status: "error",
      error: [{ PortConflict: { port: 7890, with: "控制端口" } }],
    });
    const port = await screen.findByLabelText("控制端口");
    fireEvent.change(port, { target: { value: "7890" } });
    fireEvent.click(screen.getByRole("button", { name: "保存并重启" }));
    expect(
      await screen.findByText(
        "控制端口 7890 与已有 Listener 冲突，请使用其他端口",
      ),
    ).toBeInTheDocument();
    expect(port).toHaveAttribute("aria-invalid", "true");
    expect(
      screen.queryByText("设置已保存，mihomo 正在重启"),
    ).not.toBeInTheDocument();
    expect(screen.getByText("有未保存的修改")).toBeInTheDocument();
    fireEvent.change(port, { target: { value: "19100" } });
    expect(screen.queryByText(/与已有 Listener 冲突/)).not.toBeInTheDocument();
    expect(port).toHaveAttribute("aria-invalid", "false");
    expect(commands.saveSettings).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "保存并重启" }));
    expect(await screen.findByText("已保存并生效")).toBeInTheDocument();
    expect(commands.saveSettings).toHaveBeenCalledTimes(2);
  });

  it("names the conflicting listener, including a disabled listener", async () => {
    renderPage({
      ...model,
      listeners: [
        {
          id: "l1",
          name: "浏览器",
          listener_type: "Mixed",
          listen: "127.0.0.1",
          port: 7890,
          auth: null,
          exit: null,
          user_enabled: false,
        },
      ],
    });
    vi.mocked(commands.saveSettings).mockResolvedValueOnce({
      status: "error",
      error: [{ PortConflict: { port: 7890, with: "控制端口" } }],
    });
    fireEvent.change(await screen.findByLabelText("控制端口"), {
      target: { value: "7890" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存并重启" }));
    expect(
      await screen.findByText(
        "控制端口 7890 与 Listener「浏览器」冲突，请使用其他端口",
      ),
    ).toBeInTheDocument();
  });

  it.each<[ApplyOutcome, string]>([
    [
      { NotRunning: { error: "mihomo 未运行，配置将在内核启动后生效" } },
      "已保存，启动内核后生效",
    ],
    [
      { NotRunning: { error: "internal: launch failed" } },
      "已保存，但内核未能启动，请检查设置后手动重试",
    ],
    [
      { FailedKeptPrevious: { error: "internal: PUT rejected" } },
      "已保存，但未生效，目前仍使用原配置",
    ],
    [
      { FailedClosed: { error: "internal: timeout", uncut: 0 } },
      "已保存，但未生效，代理端口已关闭，请检查配置后手动重试",
    ],
    [
      { FailedCoreStopped: { error: "internal: fallback failed" } },
      "已保存，但未生效，内核已停止，请检查配置后手动重试",
    ],
    [
      {
        Reloaded: {
          cut: 0,
          uncut: 0,
          failed_listeners: [{ id: "l1", name: "办公", port: 7890 }],
        },
      },
      "监听器「办公」端口被占用，请更换端口",
    ],
    [
      { Reloaded: { cut: 0, uncut: 1, failed_listeners: [] } },
      "部分连接未能关闭，请检查内核运行状态",
    ],
  ])(
    "keeps an apply warning/failure visible and saved without retrying: %j",
    async (outcome, message) => {
      renderPage();
      vi.mocked(commands.saveSettings).mockResolvedValueOnce({
        status: "ok",
        data: outcome,
      });
      fireEvent.change(await screen.findByLabelText("控制端口"), {
        target: { value: "19100" },
      });
      fireEvent.click(screen.getByRole("button", { name: "保存并重启" }));
      expect(await screen.findByText(message)).toBeInTheDocument();
      expect(screen.getAllByRole("alert")).toHaveLength(1);
      expect(screen.queryByText("已保存并生效")).not.toBeInTheDocument();
      expect(
        screen.queryByText("设置已保存，mihomo 正在重启"),
      ).not.toBeInTheDocument();
      expect(screen.queryByText(/internal:/)).not.toBeInTheDocument();
      expect(screen.queryByText("已保存")).not.toBeInTheDocument();
      expect(screen.getByRole("button", { name: "保存并重启" })).toBeDisabled();
      expect(commands.saveSettings).toHaveBeenCalledTimes(1);
      expect(commands.coreRestart).not.toHaveBeenCalled();
    },
  );

  it("replaces a previous success with a concise save error, without retrying", async () => {
    renderPage();
    const url = await screen.findByLabelText("测速 URL");
    fireEvent.change(url, { target: { value: "https://first.test" } });
    fireEvent.blur(url);
    await screen.findByText("已保存");
    vi.mocked(commands.saveSettings).mockRejectedValueOnce(
      new Error("internal: IPC unavailable"),
    );
    fireEvent.change(url, { target: { value: "https://second.test" } });
    fireEvent.blur(url);
    expect(
      await screen.findByText("未能保存，请重试"),
    ).toBeInTheDocument();
    expect(screen.queryByText("已保存")).not.toBeInTheDocument();
    expect(screen.queryByText(/internal:/)).not.toBeInTheDocument();
    expect(url).toHaveValue("https://second.test");
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    expect(commands.saveSettings).toHaveBeenCalledTimes(2);
  });

  it("replaces the previous success when the next apply fails", async () => {
    renderPage();
    const port = await screen.findByLabelText("控制端口");
    fireEvent.change(port, { target: { value: "19100" } });
    fireEvent.click(screen.getByRole("button", { name: "保存并重启" }));
    await screen.findByText("已保存并生效");
    vi.mocked(commands.saveSettings).mockResolvedValueOnce({
      status: "ok",
      data: { FailedKeptPrevious: { error: "internal: rejected" } },
    });
    fireEvent.change(port, { target: { value: "19101" } });
    fireEvent.click(screen.getByRole("button", { name: "保存并重启" }));
    expect(
      await screen.findByText("已保存，但未生效，目前仍使用原配置"),
    ).toBeInTheDocument();
    expect(screen.queryByText("已保存并生效")).not.toBeInTheDocument();
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    expect(commands.saveSettings).toHaveBeenCalledTimes(2);
  });

  it("keeps technical config rejection details out of the form", async () => {
    renderPage();
    vi.mocked(commands.saveSettings).mockResolvedValueOnce({
      status: "error",
      error: [{ CoreRejected: { message: "internal: YAML parse error" } }],
    });
    fireEvent.change(await screen.findByLabelText("控制端口"), {
      target: { value: "19100" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存并重启" }));
    expect(
      await screen.findByText("设置无法使用，请检查后重新保存"),
    ).toBeInTheDocument();
    expect(screen.queryByText(/internal:/)).not.toBeInTheDocument();
    expect(screen.getByText("有未保存的修改")).toBeInTheDocument();
    expect(commands.saveSettings).toHaveBeenCalledTimes(1);
  });

  it("does not save an invalid interval or block an independent core draft", async () => {
    renderPage();
    await screen.findByLabelText("控制端口");
    fireEvent.change(screen.getByLabelText("间隔"), {
      target: { value: "" },
    });
    fireEvent.change(screen.getByLabelText("单位"), {
      target: { value: "天" },
    });
    fireEvent.change(screen.getByLabelText("间隔"), {
      target: { value: "400" },
    });
    const save = screen.getByRole("button", { name: "保存并重启" });
    expect(screen.getByText("超出范围")).toBeInTheDocument();
    expect(save).toBeDisabled();
    fireEvent.click(save);
    expect(commands.saveSettings).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("间隔"), { target: { value: "2" } });
    expect(commands.saveSettings).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("控制端口"), { target: { value: "19100" } });
    expect(save).toBeEnabled();
    fireEvent.click(save);
    await screen.findByText("已保存并生效");
    expect(commands.saveSettings).toHaveBeenCalledWith({ ...model.settings, controller_port: 19100 });
  });

  it("groups settings into 常规 / 内核 / 测速 sections", async () => {
    renderPage();
    const general = await screen.findByRole("region", { name: "常规" });
    const core = screen.getByRole("region", { name: "内核" });
    const delay = screen.getByRole("region", { name: "测速" });
    expect(within(general).getByText("开机自启")).toBeInTheDocument();
    expect(within(core).getByText("内置 mihomo")).toBeInTheDocument();
    expect(within(core).queryByLabelText("mihomo 内核路径")).not.toBeInTheDocument();
    expect(within(core).getByLabelText("控制端口")).toBeInTheDocument();
    expect(
      within(core).getByRole("button", { name: "打开日志目录" }),
    ).toBeInTheDocument();
    expect(within(delay).getByText("自动测速")).toBeInTheDocument();
    expect(within(delay).getByLabelText("测速 URL")).toBeInTheDocument();
  });
});
