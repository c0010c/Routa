import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Exit, Model, RuntimeStatus } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
    saveExit: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ExitDialog } from "./ExitDialog";

const node = (name: string) => ({ name, proxy_type: "ss", server: "s", raw: null });
const model: Model = {
  subscriptions: [
    {
      id: "s1",
      name: "机场A",
      url: "u",
      update_interval_mins: null,
      fallback_exit: null,
      last_updated: null,
      last_result: null,
      nodes: [node("香港1"), node("日本1")],
    },
  ],
  exits: [],
  listeners: [],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [],
  node_groups: [],
};

const status: RuntimeStatus = {
  core: "Running",
  last_apply: null,
  health: {
    exits: {
      e1: {
        status: "Ok",
        effective_members: [],
        missing_members: [{ subscription_id: "s1", name: "已删节点" }],
        interface: null,
      },
    },
    listeners: {},
  },
  delays: { "机场A/香港1": { ms: 88, tested_at: 0 } },
  traffic: null,
  interfaces: [
    { guid: "{G1}", friendly_name: "以太网", description: "", is_up: true, ipv4: ["192.168.1.2"], ipv6: [], mihomo_name: "以太网" },
    { guid: "{G2}", friendly_name: "WLAN", description: "", is_up: true, ipv4: ["169.254.1.1"], ipv6: [], mihomo_name: "WLAN" },
  ],
  failed_listeners: [],
  effective_listeners: [],
  startup_error: null,
  save_error: null,
  tailscale: {},
};

function renderDialog(exit: Exit) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <ExitDialog exit={exit} onClose={vi.fn()} />
    </QueryClientProvider>,
  );
}

async function pickType(label: string) {
  fireEvent.mouseDown(screen.getByRole("combobox", { name: "类型" }));
  fireEvent.click(await screen.findByRole("option", { name: label }));
}

describe("ExitDialog", () => {
  beforeEach(() => {
    vi.mocked(commands.getModel).mockResolvedValue(model);
    vi.mocked(commands.getStatus).mockResolvedValue(status);
  });

  it("shows strategy select and multi node picker for groups, hides picker for direct", async () => {
    renderDialog({ id: "e0", name: "x", kind: "Direct", interface: null });
    await waitFor(() => expect(commands.getStatus).toHaveBeenCalled());
    expect(screen.queryByRole("combobox", { name: "节点" })).not.toBeInTheDocument();

    await pickType("策略组");
    expect(screen.getByRole("combobox", { name: "策略" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "成员" })).toBeInTheDocument();

    await pickType("节点");
    expect(screen.queryByRole("combobox", { name: "策略" })).not.toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "节点" })).toBeInTheDocument();

    await pickType("直连");
    expect(screen.queryByRole("combobox", { name: "节点" })).not.toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: "成员" })).not.toBeInTheDocument();
  });

  it("groups nodes by subscription with latency, and searches", async () => {
    renderDialog({ id: "e0", name: "x", kind: "Direct", interface: null });
    await pickType("节点");
    await waitFor(() => expect(commands.getModel).toHaveBeenCalled());
    const input = screen.getByRole("combobox", { name: "节点" });
    await waitFor(() => expect(commands.getStatus).toHaveBeenCalled());
    act(() => input.focus());
    fireEvent.mouseDown(input);
    expect(await screen.findByText("机场A")).toBeInTheDocument();
    expect(await screen.findByText("88 ms")).toBeInTheDocument();
    fireEvent.change(input, { target: { value: "日本" } });
    await waitFor(() => expect(screen.queryByText("香港1")).not.toBeInTheDocument());
    expect(screen.getByText("日本1")).toBeInTheDocument();
  });

  it("lists interfaces with an unavailable chip and system default", async () => {
    renderDialog({ id: "e0", name: "x", kind: "Direct", interface: "{GONE}" });
    await waitFor(() => expect(commands.getStatus).toHaveBeenCalled());
    fireEvent.mouseDown(screen.getByRole("combobox", { name: "网卡" }));
    const list = await screen.findByRole("listbox");
    expect(within(list).getByText("系统默认")).toBeInTheDocument();
    expect(within(list).getByText("以太网")).toBeInTheDocument();
    expect(within(list).getAllByText("不可用")).toHaveLength(1);
    expect(within(list).getByText("（已移除的网卡）")).toBeInTheDocument();
  });

  it("shows name errors on the exit name field and clears them on edit", async () => {
    vi.mocked(commands.saveExit).mockResolvedValue({
      status: "error",
      error: [{ InvalidName: { name: "DIRECT", reason: "与 mihomo 内置名称冲突" } }],
    });
    renderDialog({ id: "e0", name: "DIRECT", kind: "Direct", interface: null });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("与 mihomo 内置名称冲突")).toBeInTheDocument();
    expect(screen.getByLabelText("名称")).toHaveAttribute("aria-invalid", "true");
    fireEvent.change(screen.getByLabelText("名称"), { target: { value: "直连2" } });
    expect(screen.queryByText("与 mihomo 内置名称冲突")).not.toBeInTheDocument();
    await waitFor(() => expect(commands.getStatus).toHaveBeenCalled());
  });

  it("keeps member order when adding a node next to a whole-group member", async () => {
    vi.mocked(commands.saveExit).mockResolvedValue({ status: "ok", data: { NotRunning: null } } as never);
    renderDialog({
      id: "e2",
      name: "组",
      kind: {
        Group: {
          strategy: "Fallback",
          members: [{ subscription_id: "s1", name: "香港1" }, { group: { Subscription: "s1" } }],
          selected: null,
        },
      },
      interface: null,
    });
    await waitFor(() => expect(commands.getModel).toHaveBeenCalled());
    const input = screen.getByRole("combobox", { name: "成员" });
    act(() => input.focus());
    fireEvent.mouseDown(input);
    fireEvent.click(await screen.findByRole("option", { name: /日本1/ }));
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(commands.saveExit).toHaveBeenCalled());
    const saved = vi.mocked(commands.saveExit).mock.calls[0][0];
    expect((saved.kind as { Group: { members: unknown[] } }).Group.members).toEqual([
      { subscription_id: "s1", name: "香港1" },
      { group: { Subscription: "s1" } },
      { subscription_id: "s1", name: "日本1" },
    ]);
  });

  it("removes missing members on demand", async () => {
    renderDialog({
      id: "e1",
      name: "组",
      kind: {
        Group: {
          strategy: "UrlTest",
          members: [
            { subscription_id: "s1", name: "香港1" },
            { subscription_id: "s1", name: "已删节点" },
          ],
          selected: null,
        },
      },
      interface: null,
    });
    expect(await screen.findByText(/失效成员：已删节点/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "移除失效成员" }));
    await waitFor(() => expect(screen.queryByText(/失效成员：/)).not.toBeInTheDocument());
  });

  it("saves a whole group plus a manual node and shows the expanded count", async () => {
    vi.mocked(commands.getModel).mockResolvedValue({
      ...model,
      manual_nodes: [{ id: "m1", name: "家里", groups: [], kind: { Proxy: { raw: {} } } }],
    });
    vi.mocked(commands.saveExit).mockResolvedValue({ status: "ok", data: { NotRunning: null } } as never);
    renderDialog({ id: "e3", name: "组", kind: { Group: { strategy: "UrlTest", members: [], selected: null } }, interface: null });
    await waitFor(() => expect(commands.getModel).toHaveBeenCalled());
    const input = screen.getByRole("combobox", { name: "成员" });
    act(() => input.focus());
    fireEvent.mouseDown(input);
    fireEvent.click(await screen.findByRole("option", { name: /机场A（整组/ }));
    fireEvent.mouseDown(input);
    fireEvent.click(await screen.findByRole("option", { name: /家里/ }));
    expect(await screen.findByText(/展开后共 3 个节点（已去重）/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(commands.saveExit).toHaveBeenCalled());
    const saved = vi.mocked(commands.saveExit).mock.calls[0][0];
    expect((saved.kind as { Group: { members: unknown[] } }).Group.members).toEqual([
      { group: { Subscription: "s1" } },
      { manual_id: "m1" },
    ]);
  });

  it("locks the interface to the tailscale instance NIC", async () => {
    vi.mocked(commands.getModel).mockResolvedValue({
      ...model,
      manual_nodes: [
        {
          id: "t1",
          name: "ts",
          groups: [],
          kind: { Tailscale: { hostname: "h", exit_node: "x", ephemeral: false, udp: true, auth_key: null, interface: "{G1}" } },
        },
      ],
    });
    vi.mocked(commands.saveExit).mockResolvedValue({ status: "ok", data: { NotRunning: null } } as never);
    renderDialog({ id: "e4", name: "ts出口", kind: { Node: { node: { manual_id: "t1" } } }, interface: null });
    expect(await screen.findByText(/跟随 Tailscale 实例：以太网/)).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "网卡" })).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(commands.saveExit).toHaveBeenCalled());
    expect(vi.mocked(commands.saveExit).mock.calls[0][0].interface).toBe("{G1}");
  });

  it("warns before saving when a mixed exit NIC differs from the instance NIC", async () => {
    vi.mocked(commands.getModel).mockResolvedValue({
      ...model,
      manual_nodes: [
        {
          id: "t1",
          name: "ts",
          groups: [],
          kind: { Tailscale: { hostname: "h", exit_node: "x", ephemeral: false, udp: true, auth_key: null, interface: "{G1}" } },
        },
      ],
    });
    renderDialog({
      id: "e5",
      name: "混合",
      kind: { Group: { strategy: "UrlTest", members: [{ manual_id: "t1" }, { subscription_id: "s1", name: "香港1" }], selected: null } },
      interface: "{G2}",
    });
    expect(await screen.findByText("「ts」的网卡是 以太网，需与出口一致")).toBeInTheDocument();
  });
});
