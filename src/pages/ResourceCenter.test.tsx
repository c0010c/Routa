import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  fireEvent,
  render,
  screen,
  within,
  waitFor,
} from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { ResourceCenter } from "./ResourceCenter";
import type { Model, RuntimeStatus } from "../bindings";
vi.mock("../bindings", () => ({
  commands: { getModel: vi.fn(), getStatus: vi.fn(), testDelays: vi.fn() },
}));
import { commands } from "../bindings";
const model: Model = {
  subscriptions: [
    {
      id: "s1",
      name: "来源甲",
      url: "https://example.test/sub",
      update_interval_mins: 60,
      fallback_exit: null,
      last_updated: null,
      last_result: null,
      nodes: [{ name: "订阅节点", proxy_type: "ss", server: "test", raw: {} }],
    },
  ],
  manual_nodes: [
    {
      id: "m1",
      name: "手动节点甲",
      groups: ["g1"],
      kind: { Proxy: { raw: { type: "socks5" } } },
    },
    {
      id: "m2",
      name: "游离节点",
      groups: [],
      kind: { Proxy: { raw: { type: "http" } } },
    },
  ],
  node_groups: [
    {
      id: "g1",
      name: "跨来源组",
      members: [{ subscription_id: "s1", name: "订阅节点" }],
    },
  ],
  listeners: [],
  exits: [],
  settings: {
    autostart: false,
    core_path: null,
    controller_port: 19097,
    delay_test_interval_mins: 10,
    delay_test_url: "u",
  },
  process_routing: { enabled: false, on_routa_exit: "Block", rules: [] },
};
function setup(
  subscriptionsOnly = false,
  path = "/nodes",
  data: Model = model,
) {
  vi.mocked(commands.getModel).mockResolvedValue(data);
  vi.mocked(commands.getStatus).mockResolvedValue({
    delays: {},
    interfaces: [],
  } as unknown as RuntimeStatus);
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={[path]}>
        <ResourceCenter subscriptionsOnly={subscriptionsOnly} />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}
it("groups nodes across sources and puts only unassigned nodes in 未分组", async () => {
  setup();
  const g = await screen.findByRole("button", { name: "▾ 跨来源组 · 2" });
  const section = g.closest(".MuiPaper-root")! as HTMLElement;
  expect(within(section).getByText("订阅节点")).toBeInTheDocument();
  expect(within(section).getByText("手动节点甲")).toBeInTheDocument();
  expect(within(section).queryByText("游离节点")).not.toBeInTheDocument();
  expect(screen.queryByRole("tab")).not.toBeInTheDocument();
});
it("searches collapsed groups and restores collapse when cleared", async () => {
  setup();
  fireEvent.click(await screen.findByRole("button", { name: "全部折叠" }));
  expect(screen.queryByText("订阅节点")).not.toBeInTheDocument();
  const search = screen.getByLabelText("搜索名称、来源、协议或分组");
  fireEvent.change(search, { target: { value: "订阅节点" } });
  expect(screen.getByText("订阅节点")).toBeInTheDocument();
  fireEvent.change(search, { target: { value: "" } });
  expect(screen.queryByText("订阅节点")).not.toBeInTheDocument();
});
it("renders standalone subscription cards with a source-filtered node link", async () => {
  setup(true, "/subscriptions");
  expect(await screen.findByRole("link", { name: "查看节点" })).toHaveAttribute(
    "href",
    "/nodes?source=s1",
  );
  expect(screen.queryByRole("table")).not.toBeInTheDocument();
  expect(screen.getByText("https://example.test/sub")).toBeInTheDocument();
});
it("filters a subscription source across groups", async () => {
  setup(false, "/nodes?source=s1");
  expect(await screen.findByText("订阅节点")).toBeInTheDocument();
  expect(screen.queryByText("手动节点甲")).not.toBeInTheDocument();
  expect(screen.queryByText("游离节点")).not.toBeInTheDocument();
});

it("shows subscription nodes under their source instead of 未分组 when no custom group is assigned", async () => {
  setup(false, "/nodes", { ...model, node_groups: [] });
  const source = await screen.findByRole("button", { name: "▾ 来源甲 · 1" });
  const section = source.closest(".MuiPaper-root") as HTMLElement;
  expect(
    within(section).getByRole("article", { name: "订阅节点" }),
  ).toBeInTheDocument();
  const ungrouped = screen
    .getByRole("button", { name: "▾ 未分组 · 2" })
    .closest(".MuiPaper-root") as HTMLElement;
  expect(within(ungrouped).queryByText("订阅节点")).not.toBeInTheDocument();
  expect(
    within(ungrouped).getByRole("article", { name: "游离节点" }),
  ).toBeInTheDocument();
  expect(screen.queryByRole("table")).not.toBeInTheDocument();
});
it("does not duplicate a custom-grouped subscription node in its source section", async () => {
  setup();
  expect(
    await screen.findByRole("button", { name: "▾ 跨来源组 · 2" }),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: /▾ 来源甲/ }),
  ).not.toBeInTheDocument();
  expect(screen.getAllByRole("article", { name: "订阅节点" })).toHaveLength(1);
});
it("keeps nodes with the same name in different subscription sections", async () => {
  const second = { ...model.subscriptions[0], id: "s2", name: "来源乙" };
  setup(false, "/nodes", {
    ...model,
    node_groups: [],
    manual_nodes: [],
    subscriptions: [...model.subscriptions, second],
  });
  const first = await screen.findByRole("button", { name: "▾ 来源甲 · 1" });
  const other = screen.getByRole("button", { name: "▾ 来源乙 · 1" });
  for (const button of [first, other])
    expect(
      within(button.closest(".MuiPaper-root") as HTMLElement).getByRole(
        "article",
        { name: "订阅节点" },
      ),
    ).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: /未分组/ }),
  ).not.toBeInTheDocument();
});
it("searches collapsed subscription sections and restores collapse on clear", async () => {
  setup(false, "/nodes", { ...model, node_groups: [] });
  fireEvent.click(await screen.findByRole("button", { name: "全部折叠" }));
  expect(
    screen.queryByRole("article", { name: "订阅节点" }),
  ).not.toBeInTheDocument();
  const search = screen.getByLabelText("搜索名称、来源、协议或分组");
  fireEvent.change(search, { target: { value: "来源甲" } });
  expect(screen.getByRole("article", { name: "订阅节点" })).toBeInTheDocument();
  expect(
    screen.queryByRole("article", { name: "游离节点" }),
  ).not.toBeInTheDocument();
  fireEvent.change(search, { target: { value: "" } });
  expect(
    screen.queryByRole("article", { name: "订阅节点" }),
  ).not.toBeInTheDocument();
});
it("filters source fallback sections without including other sources or manual nodes", async () => {
  setup(false, "/nodes?source=s1", {
    ...model,
    node_groups: [],
    subscriptions: [
      ...model.subscriptions,
      {
        ...model.subscriptions[0],
        id: "s2",
        name: "来源乙",
        nodes: [{ ...model.subscriptions[0].nodes[0], name: "其他订阅节点" }],
      },
    ],
  });
  expect(
    await screen.findByRole("article", { name: "订阅节点" }),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("article", { name: "其他订阅节点" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("article", { name: "游离节点" }),
  ).not.toBeInTheDocument();
});

it("moves node actions into the right-click menu and tests the selected node", async () => {
  vi.mocked(commands.testDelays).mockResolvedValue({ status: "ok", data: [] });
  setup();
  const card = await screen.findByRole("article", { name: "订阅节点" });
  expect(
    within(card).queryByRole("button", { name: "用于新代理" }),
  ).not.toBeInTheDocument();
  fireEvent.contextMenu(card, { clientX: 120, clientY: 150 });
  expect(
    await screen.findByRole("menuitem", { name: "用于新代理 订阅节点" }),
  ).toBeInTheDocument();
  fireEvent.click(screen.getByRole("menuitem", { name: "测延迟 订阅节点" }));
  await waitFor(() =>
    expect(commands.testDelays).toHaveBeenCalledWith(["来源甲/订阅节点"]),
  );
});

it("opens proxy creation from the node context menu without submitting it", async () => {
  setup();
  fireEvent.contextMenu(
    await screen.findByRole("article", { name: "订阅节点" }),
    { clientX: 120, clientY: 150 },
  );
  fireEvent.click(
    await screen.findByRole("menuitem", { name: "用于新代理 订阅节点" }),
  );
  expect(
    await screen.findByRole("dialog", { name: "新增代理" }),
  ).toBeInTheDocument();
  expect(screen.getByLabelText("名称")).toHaveValue("订阅节点");
});
