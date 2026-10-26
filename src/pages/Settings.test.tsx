import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
  it("restarts mihomo after the controller port changes", async () => {
    renderPage();
    fireEvent.change(await screen.findByLabelText("控制端口"), { target: { value: "19100" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("设置已保存，mihomo 正在重启")).toBeInTheDocument();
    expect(commands.saveSettings).toHaveBeenCalledWith(expect.objectContaining({ controller_port: 19100 }));
    await waitFor(() => expect(commands.coreRestart).toHaveBeenCalledTimes(1));
  });

  it("does not restart when only unrelated fields change", async () => {
    renderPage();
    fireEvent.change(await screen.findByLabelText("测速 URL"), { target: { value: "https://x.test" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("设置已保存")).toBeInTheDocument();
    expect(commands.coreRestart).not.toHaveBeenCalled();
  });
});
