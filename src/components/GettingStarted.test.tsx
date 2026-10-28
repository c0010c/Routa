import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
import type { Exit, Listener, Model } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    saveExit: vi.fn(async () => ({ status: "ok", data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } })),
    saveListener: vi.fn(async () => ({ status: "ok", data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } })),
  },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "./ApplyResultSnackbar";
import { GettingStarted, needsGettingStarted, uniqueName } from "./GettingStarted";

const base: Model = {
  subscriptions: [],
  exits: [],
  listeners: [],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [],
  node_groups: [],
  process_routing: { enabled: false, on_routa_exit: "Block", rules: [] },
};

const listener = (over: Partial<Listener>): Listener => ({
  id: "l1",
  name: "默认",
  listener_type: "Mixed",
  listen: "0.0.0.0",
  port: 7891,
  auth: null,
  exit: null,
  user_enabled: true,
  ...over,
});

function Loc() {
  const loc = useLocation();
  return <div data-testid="loc">{loc.pathname + loc.search}</div>;
}

function renderCard(m: Model, cb = { onNewExit: vi.fn(), onNewListener: vi.fn() }) {
  const qc = new QueryClient();
  render(
    <QueryClientProvider client={qc}>
      <ApplyResultProvider>
        <MemoryRouter>
          <GettingStarted model={m} {...cb} />
          <Loc />
        </MemoryRouter>
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
  return cb;
}

describe("GettingStarted", () => {
  beforeEach(() => {
    vi.mocked(commands.saveExit).mockClear();
    vi.mocked(commands.saveListener).mockClear();
  });

  it("is needed until some listener has an exit", () => {
    expect(needsGettingStarted(base)).toBe(true);
    expect(needsGettingStarted({ ...base, listeners: [listener({})] })).toBe(true);
    expect(needsGettingStarted({ ...base, listeners: [listener({ exit: "e1" })] })).toBe(false);
  });

  it("picks the first free name", () => {
    expect(uniqueName("直连", [])).toBe("直连");
    expect(uniqueName("直连", ["直连", "直连 2"])).toBe("直连 3");
  });

  it("marks done steps and wires the step buttons", () => {
    const exit: Exit = { id: "e1", name: "x", kind: "Direct", interface: null };
    const cb = renderCard({ ...base, exits: [exit] });
    expect(screen.queryByLabelText("第 1 步已完成")).not.toBeInTheDocument();
    expect(screen.getByLabelText("第 2 步已完成")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "新建出口" }));
    fireEvent.click(screen.getByRole("button", { name: "新建监听器" }));
    expect(cb.onNewExit).toHaveBeenCalled();
    expect(cb.onNewListener).toHaveBeenCalled();
  });

  it("quick start creates a direct exit and a listener wired to it", async () => {
    renderCard({ ...base, subscriptions: [], exits: [] });
    fireEvent.click(screen.getByRole("button", { name: "快速开始：建一个直连端口" }));
    await waitFor(() => expect(commands.saveListener).toHaveBeenCalledTimes(1));
    const exit = vi.mocked(commands.saveExit).mock.calls[0][0];
    expect(exit).toMatchObject({ name: "直连", kind: "Direct", interface: null });
    const l = vi.mocked(commands.saveListener).mock.calls[0][0];
    expect(l).toMatchObject({ name: "默认", port: 7891, exit: exit.id });
    await waitFor(() => expect(screen.getByTestId("loc")).toHaveTextContent(`/?highlight=${l.id}`));
    expect(await screen.findByText("已创建：端口 7891 走「直连」，可以用了")).toBeInTheDocument();
  });

  it("quick start reuses an existing direct exit and avoids name clashes", async () => {
    const exit: Exit = { id: "e9", name: "家里", kind: "Direct", interface: null };
    renderCard({ ...base, exits: [exit], listeners: [listener({ name: "默认", port: 7891 })] });
    fireEvent.click(screen.getByRole("button", { name: "快速开始：建一个直连端口" }));
    await waitFor(() => expect(commands.saveListener).toHaveBeenCalledTimes(1));
    expect(commands.saveExit).not.toHaveBeenCalled();
    expect(vi.mocked(commands.saveListener).mock.calls[0][0]).toMatchObject({ name: "默认 2", port: 7892, exit: "e9" });
  });
});

describe("GettingStarted button hierarchy", () => {
  it("keeps quick start as the only contained button", () => {
    renderCard({ ...base, exits: [{ id: "e1", name: "直连", kind: "Direct", interface: null }] });
    const contained = screen.getAllByRole("button").filter((b) => b.classList.contains("MuiButton-contained"));
    expect(contained.map((b) => b.textContent)).toEqual(["快速开始：建一个直连端口"]);
  });
});
