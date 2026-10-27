import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { Model, RuntimeStatus } from "../bindings";

vi.mock("../bindings", () => ({
  commands: { getModel: vi.fn(), getStatus: vi.fn(), testDelays: vi.fn(() => new Promise(() => {})) },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { Exits } from "./Exits";

const model: Model = {
  subscriptions: [],
  exits: [{ id: "e1", name: "直连", kind: "Direct", interface: null }],
  listeners: [],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [],
  node_groups: [],
};
const status: RuntimeStatus = {
  core: "Running",
  last_apply: null,
  health: { exits: { e1: { status: "Ok", effective_members: [], missing_members: [], interface: null } }, listeners: {} },
  delays: { 直连: { ms: 650, tested_at: 0 } },
  traffic: null,
  interfaces: [],
  failed_listeners: [],
  effective_listeners: [],
  startup_error: null,
  save_error: null,
  tailscale: {},
};

function renderExits(m: Model) {
  vi.mocked(commands.getModel).mockResolvedValue(m);
  vi.mocked(commands.getStatus).mockResolvedValue(status);
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <ApplyResultProvider>
        <MemoryRouter>
          <Exits />
        </MemoryRouter>
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
}

describe("Exits", () => {
  it("shows an empty state instead of an empty table", async () => {
    renderExits({ ...model, exits: [] });
    expect(await screen.findByText("还没有出口")).toBeInTheDocument();
    expect(screen.queryByRole("columnheader")).toBeNull();
  });

  it("shows the delay and puts edit/delete in the row menu", async () => {
    renderExits(model);
    expect(await screen.findByText("650 ms")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "操作 直连" }));
    expect(screen.getByRole("menuitem", { name: "编辑 直连" })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "删除 直连" })).toBeInTheDocument();
  });

  it("disables 测速 while a delay test is running", async () => {
    renderExits(model);
    fireEvent.click(await screen.findByRole("button", { name: "操作 直连" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "测速 直连" }));
    await waitFor(() => expect(commands.testDelays).toHaveBeenCalledWith(["直连"]));
    fireEvent.click(screen.getByRole("button", { name: "操作 直连" }));
    expect(screen.getByRole("menuitem", { name: "测速 直连" })).toHaveAttribute("aria-disabled", "true");
  });
});
