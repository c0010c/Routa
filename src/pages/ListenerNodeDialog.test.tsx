import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ListenerNodeDialog } from "./ListenerNodeDialog";
import { Listeners } from "./Listeners";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import type { Model, RuntimeStatus } from "../bindings";
vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
    selectGroupMember: vi.fn(),
    saveListener: vi.fn(),
    saveExit: vi.fn(),
    testDelays: vi.fn(),
  },
}));
import { commands } from "../bindings";
const a = { manual_id: "a" },
  b = { subscription_id: "s", name: "节点乙" };
const model: Model = {
  subscriptions: [
    {
      id: "s",
      name: "来源甲",
      url: "https://example.test/sub",
      update_interval_mins: 60,
      fallback_exit: null,
      last_updated: null,
      last_result: null,
      nodes: [
        { name: "节点乙", proxy_type: "ss", server: "example.test", raw: {} },
      ],
    },
  ],
  manual_nodes: [
    {
      id: "a",
      name: "节点甲",
      groups: [],
      kind: { Proxy: { raw: { type: "http" } } },
    },
    {
      id: "outside",
      name: "非成员节点",
      groups: ["other"],
      kind: { Proxy: { raw: { type: "http" } } },
    },
  ],
  node_groups: [{ id: "other", name: "其他分组", members: [] }],
  listeners: [
    {
      id: "l",
      name: "浏览器",
      listener_type: "Mixed",
      listen: "127.0.0.1",
      port: 7881,
      auth: null,
      exit: "e",
      user_enabled: true,
    },
  ],
  exits: [
    {
      id: "e",
      name: "private",
      interface: null,
      kind: {
        Group: {
          strategy: "Select",
          members: [a, { group: { Subscription: "s" } }],
          selected: a,
        },
      },
    },
  ],
  settings: {
    autostart: false,
    core_path: null,
    controller_port: 19097,
    delay_test_interval_mins: 10,
    delay_test_url: "u",
  },
  process_routing: { enabled: false, on_routa_exit: "Block", rules: [] },
};
const status = {
  core: "Running",
  last_apply: null,
  health: { exits: {}, listeners: { l: "Active" } },
  delays: { "来源甲/节点乙": { ms: 42, tested_at: 0 } },
  interfaces: [],
  failed_listeners: [],
  effective_listeners: ["l"],
  tailscale: {},
  tailscale_exit_errors: {},
  tailscale_exit_retrying: {},
} as unknown as RuntimeStatus;
function setup(
  props: Partial<Parameters<typeof ListenerNodeDialog>[0]> = {},
  cards = false,
  data: Model = model,
) {
  vi.mocked(commands.getModel).mockResolvedValue(data);
  vi.mocked(commands.getStatus).mockResolvedValue(status);
  vi.mocked(commands.saveExit).mockReset().mockResolvedValue({ status: "ok", data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } });
  vi.mocked(commands.selectGroupMember)
    .mockReset()
    .mockResolvedValue({
      status: "ok",
      data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } },
    });
  const close = vi.fn();
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  qc.setQueryData(["model"], data);
  qc.setQueryData(["status"], status);
  render(
    <QueryClientProvider client={qc}>
      <ApplyResultProvider>
        {cards ? (
          <Listeners />
        ) : (
          <ListenerNodeDialog
            exitId="e"
            listenerName="浏览器"
            onClose={close}
            {...props}
          />
        )}
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
  return close;
}
it("shows effective strategy members as node cards with sources and delay", async () => {
  setup();
  expect(
    await screen.findByRole("radio", { name: "节点甲 · 手动节点" }),
  ).toHaveAttribute("aria-checked", "true");
  expect(
    screen.getByRole("radio", { name: "节点乙 · 来源甲" }),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("radio", { name: /非成员/ }),
  ).not.toBeInTheDocument();
  expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  expect(screen.getByLabelText("42 ms")).toBeInTheDocument();
  expect(commands.selectGroupMember).not.toHaveBeenCalled();
});
it("commits only the confirmed node selection through the real command contract", async () => {
  const close = setup();
  fireEvent.click(
    await screen.findByRole("radio", { name: "节点乙 · 来源甲" }),
  );
  expect(commands.selectGroupMember).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "确认切换" }));
  await waitFor(() =>
    expect(commands.selectGroupMember).toHaveBeenCalledWith("e", b),
  );
  await waitFor(() => expect(close).toHaveBeenCalledOnce());
});

it("navigates candidate radios from the keyboard without committing", async () => {
  setup();
  const first = await screen.findByRole("radio", { name: "节点甲 · 手动节点" });
  fireEvent.keyDown(first, { key: "ArrowRight" });
  expect(screen.getByRole("radio", { name: "节点乙 · 来源甲" })).toHaveAttribute("aria-checked", "true");
  expect(screen.getByText("2 / 2 个候选节点")).toBeInTheDocument();
  expect(commands.selectGroupMember).not.toHaveBeenCalled();
});

it("measures a candidate from its menu without changing the chosen node", async () => {
  setup();
  vi.mocked(commands.testDelays).mockResolvedValue({ status: "ok", data: [] });
  fireEvent.click(await screen.findByRole("button", { name: "操作 节点乙" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "测延迟 节点乙" }));
  await waitFor(() => expect(commands.testDelays).toHaveBeenCalledWith(["来源甲/节点乙"]));
  expect(screen.getByRole("radio", { name: "节点甲 · 手动节点" })).toHaveAttribute("aria-checked", "true");
  expect(commands.selectGroupMember).not.toHaveBeenCalled();
});
it("cancels draft selection without committing", async () => {
  const close = setup();
  fireEvent.click(
    await screen.findByRole("radio", { name: "节点乙 · 来源甲" }),
  );
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(close).toHaveBeenCalledOnce();
  expect(commands.selectGroupMember).not.toHaveBeenCalled();
});
it("searches members without losing the pending selection", async () => {
  setup();
  fireEvent.click(
    await screen.findByRole("radio", { name: "节点乙 · 来源甲" }),
  );
  fireEvent.change(screen.getByLabelText("搜索节点、来源或协议"), {
    target: { value: "节点甲" },
  });
  expect(
    screen.queryByRole("radio", { name: "节点乙 · 来源甲" }),
  ).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("搜索节点、来源或协议"), {
    target: { value: "" },
  });
  expect(
    screen.getByRole("radio", { name: "节点乙 · 来源甲" }),
  ).toHaveAttribute("aria-checked", "true");
});
it("leaves the popup open and shows backend validation failures", async () => {
  const close = setup();
  vi.mocked(commands.selectGroupMember).mockResolvedValue({
    status: "error",
    error: [{ DanglingRef: { what: "策略成员已失效" } }],
  });
  fireEvent.click(
    await screen.findByRole("radio", { name: "节点乙 · 来源甲" }),
  );
  fireEvent.click(screen.getByRole("button", { name: "确认切换" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("策略成员已失效");
  expect(close).not.toHaveBeenCalled();
});
it("returns a form's selection without saving until the parent form submits", async () => {
  const select = vi.fn();
  const close = setup({
    draft: { members: [a, b], selected: a },
    onSelect: select,
  });
  fireEvent.click(
    await screen.findByRole("radio", { name: "节点乙 · 来源甲" }),
  );
  fireEvent.click(screen.getByRole("button", { name: "选择节点" }));
  expect(select).toHaveBeenCalledWith(b, [a, b]);
  expect(close).toHaveBeenCalledOnce();
  expect(commands.selectGroupMember).not.toHaveBeenCalled();
});
it("opens the large picker directly from a manual-strategy proxy card", async () => {
  setup({}, true);
  expect(screen.getByRole("article", { name: "浏览器" })).toBeInTheDocument();
  expect(screen.queryByRole("table")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "切换 浏览器 的节点" }));
  expect(
    await screen.findByRole("dialog", { name: "切换节点 · 浏览器" }),
  ).toBeInTheDocument();
  expect(screen.getAllByRole("radio")).toHaveLength(2);
});

it("intersects real custom groups with candidates without losing a pending selection", async () => {
  setup({}, false, {
    ...model,
    node_groups: [
      { id: "common", name: "常用节点", members: [a, { manual_id: "outside" }] },
      { id: "backup", name: "备用节点", members: [b, b] },
      { id: "outside", name: "无关分组", members: [{ manual_id: "outside" }] },
    ],
  });
  fireEvent.click(await screen.findByRole("radio", { name: "节点乙 · 来源甲" }));
  fireEvent.click(screen.getByRole("button", { name: "常用节点" }));
  expect(screen.getAllByRole("radio")).toHaveLength(1);
  expect(screen.getByRole("radio", { name: "节点甲 · 手动节点" })).toBeInTheDocument();
  expect(screen.getByText("已选择：节点乙")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "确认切换" })).toBeEnabled();
  expect(screen.queryByRole("button", { name: "无关分组" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "仅可用" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "备用节点" }));
  expect(screen.getAllByRole("radio")).toHaveLength(1);
  expect(screen.getByRole("radio", { name: "节点乙 · 来源甲" })).toHaveAttribute("aria-checked", "true");
  fireEvent.click(screen.getByRole("button", { name: "全部分组" }));
  expect(screen.getAllByRole("radio")).toHaveLength(2);
  expect(commands.selectGroupMember).not.toHaveBeenCalled();
});

const freeNode = { manual_id: "free" };
const withUngroupedNode: Model = {
  ...model,
  manual_nodes: [...model.manual_nodes, {
    id: "free", name: "未分组的新节点", groups: [], kind: { Proxy: { raw: { type: "ss" } } },
  }],
};

it("shows ungrouped manual nodes and adds only the confirmed node to the current exit", async () => {
  const close = setup({}, false, withUngroupedNode);
  fireEvent.click(await screen.findByRole("button", { name: "未分组" }));
  expect(screen.getAllByRole("radio")).toHaveLength(2);
  expect(screen.queryByRole("radio", { name: /非成员节点/ })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("radio", { name: "未分组的新节点 · 手动节点" }));
  expect(screen.getByText("确认后加入当前代理并切换")).toBeInTheDocument();
  expect(commands.saveExit).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "确认切换" }));
  await waitFor(() => expect(commands.saveExit).toHaveBeenCalledWith({
    ...model.exits[0],
    kind: { Group: { strategy: "Select", members: [a, { group: { Subscription: "s" } }, freeNode], selected: freeNode } },
  }));
  expect(commands.selectGroupMember).not.toHaveBeenCalled();
  await waitFor(() => expect(close).toHaveBeenCalledOnce());
});

it("cancels a new ungrouped selection without changing exit membership", async () => {
  const close = setup({}, false, withUngroupedNode);
  fireEvent.click(await screen.findByRole("radio", { name: "未分组的新节点 · 手动节点" }));
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(close).toHaveBeenCalledOnce();
  expect(commands.saveExit).not.toHaveBeenCalled();
  expect(commands.selectGroupMember).not.toHaveBeenCalled();
});

it("returns an ungrouped node and its membership to the editing form without saving", async () => {
  const select = vi.fn();
  const close = setup({ draft: { members: [b], selected: b }, onSelect: select }, false, withUngroupedNode);
  fireEvent.click(await screen.findByRole("radio", { name: "未分组的新节点 · 手动节点" }));
  fireEvent.click(screen.getByRole("button", { name: "选择节点" }));
  expect(select).toHaveBeenCalledWith(freeNode, [b, freeNode]);
  expect(close).toHaveBeenCalledOnce();
  expect(commands.saveExit).not.toHaveBeenCalled();
  expect(commands.selectGroupMember).not.toHaveBeenCalled();
});
