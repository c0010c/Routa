import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Model, RuntimeStatus } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
    saveListener: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { Listeners } from "./Listeners";

const model: Model = {
  subscriptions: [],
  exits: [],
  listeners: [
    { id: "l1", name: "下载", listener_type: "Mixed", listen: "0.0.0.0", port: 7891, auth: null, exit: null, user_enabled: false },
  ],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [],
  node_groups: [],
};
const status: RuntimeStatus = {
  core: "Running",
  last_apply: null,
  health: { exits: {}, listeners: { l1: "DisabledByUser" } },
  delays: {},
  traffic: null,
  interfaces: [],
  failed_listeners: [],
  effective_listeners: [],
  startup_error: null,
  save_error: null,
};

describe("Listeners", () => {
  it("surfaces validation errors from the enable switch in the snackbar", async () => {
    vi.mocked(commands.getModel).mockResolvedValue(model);
    vi.mocked(commands.getStatus).mockResolvedValue(status);
    vi.mocked(commands.saveListener).mockResolvedValue({
      status: "error",
      error: [{ PortConflict: { port: 7891, with: "浏览器" } }],
    });
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <ApplyResultProvider>
          <Listeners />
        </ApplyResultProvider>
      </QueryClientProvider>,
    );
    fireEvent.click(await screen.findByRole("switch", { name: "启用 下载" }));
    expect(await screen.findByText("端口 7891 已被「浏览器」占用")).toBeInTheDocument();
    await waitFor(() => expect(commands.saveListener).toHaveBeenCalled());
  });
});
