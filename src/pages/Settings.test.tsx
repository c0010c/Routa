import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Model } from "../bindings";

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

const model: Model = {
  subscriptions: [],
  exits: [],
  listeners: [],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [],
  node_groups: [],
};

function renderPage() {
  vi.mocked(commands.getModel).mockResolvedValue(model);
  vi.mocked(commands.saveSettings).mockResolvedValue({ status: "ok", data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } });
  vi.mocked(commands.coreRestart).mockResolvedValue({ status: "ok", data: null });
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <ApplyResultProvider>
        <Settings />
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
}

describe("Settings", () => {
  it("enables saving only with unsaved changes, and says so", async () => {
    renderPage();
    const url = await screen.findByLabelText("测速 URL");
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
    expect(screen.getByText("已保存")).toBeInTheDocument();
    fireEvent.change(url, { target: { value: "https://x.test" } });
    expect(screen.getByRole("button", { name: "保存" })).toBeEnabled();
    expect(screen.getByText("有未保存的修改")).toBeInTheDocument();
  });

  it("reports the backend restart after the controller port changes, without restarting itself", async () => {
    renderPage();
    fireEvent.change(await screen.findByLabelText("控制端口"), { target: { value: "19100" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("设置已保存，mihomo 正在重启")).toBeInTheDocument();
    expect(commands.saveSettings).toHaveBeenCalledWith(expect.objectContaining({ controller_port: 19100 }));
    await waitFor(() => expect(commands.saveSettings).toHaveBeenCalledTimes(1));
    expect(commands.coreRestart).not.toHaveBeenCalled();
  });

  it("does not restart when only unrelated fields change", async () => {
    renderPage();
    fireEvent.change(await screen.findByLabelText("测速 URL"), { target: { value: "https://x.test" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("设置已保存")).toBeInTheDocument();
    expect(commands.coreRestart).not.toHaveBeenCalled();
  });

  it("blocks saving while the interval is out of range", async () => {
    renderPage();
    await screen.findByLabelText("控制端口");
    fireEvent.change(screen.getByLabelText("单位"), { target: { value: "天" } });
    fireEvent.change(screen.getByLabelText("间隔"), { target: { value: "400" } });
    const save = screen.getByRole("button", { name: "保存" });
    expect(screen.getByText("超出范围")).toBeInTheDocument();
    expect(save).toBeDisabled();
    fireEvent.click(save);
    expect(commands.saveSettings).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("间隔"), { target: { value: "2" } });
    expect(save).toBeEnabled();
  });

  it("groups settings into 常规 / 内核 / 测速 sections", async () => {
    renderPage();
    const general = await screen.findByRole("region", { name: "常规" });
    const core = screen.getByRole("region", { name: "内核" });
    const delay = screen.getByRole("region", { name: "测速" });
    expect(within(general).getByText("开机自启")).toBeInTheDocument();
    expect(within(core).getByLabelText("mihomo 内核路径")).toBeInTheDocument();
    expect(within(core).getByLabelText("控制端口")).toBeInTheDocument();
    expect(within(core).getByRole("button", { name: "打开日志目录" })).toBeInTheDocument();
    expect(within(delay).getByText("自动测速")).toBeInTheDocument();
    expect(within(delay).getByLabelText("测速 URL")).toBeInTheDocument();
  });
});
