import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import type { Model, RuntimeStatus } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { Nodes } from "./Nodes";

const model: Model = {
  subscriptions: [
    {
      id: "s1",
      name: "机场A",
      url: "https://example.com/sub",
      update_interval_mins: 1440,
      fallback_exit: null,
      last_updated: 1_700_000_000,
      last_result: { Failed: { reason: "订阅返回了 HTML 页面" } },
      nodes: [{ name: "香港 01", proxy_type: "ss", server: "hk.example.com", raw: {} }],
    },
  ],
  exits: [],
  listeners: [],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
};
const status: RuntimeStatus = {
  core: "Running",
  last_apply: null,
  health: { exits: {}, listeners: {} },
  delays: { "机场A/香港 01": { ms: 123, tested_at: 0 } },
  traffic: null,
  interfaces: [],
  failed_listeners: [],
  startup_error: null,
  save_error: null,
};

describe("Nodes", () => {
  it("shows the failure reason on the card and keeps the old nodes listed", async () => {
    vi.mocked(commands.getModel).mockResolvedValue(model);
    vi.mocked(commands.getStatus).mockResolvedValue(status);
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <ApplyResultProvider>
          <Nodes />
        </ApplyResultProvider>
      </QueryClientProvider>,
    );
    expect(await screen.findByText(/订阅返回了 HTML 页面/)).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "香港 01" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "123 ms" })).toBeInTheDocument();
  });
});
