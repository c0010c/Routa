import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { NodePickerDialog } from "./NodePickerDialog";
import type { Model, NodeRef, RuntimeStatus } from "../bindings";
vi.mock("../bindings", () => ({
  commands: { getModel: vi.fn(), getStatus: vi.fn(), testDelays: vi.fn() },
}));

const one: NodeRef = { subscription_id: "s", name: "节点一" };
const two: NodeRef = { subscription_id: "s", name: "节点二" };
const node = (name: string) => ({ name, proxy_type: "ss", server: "example.test", raw: {} });
const model = {
  subscriptions: [{
    id: "s", name: "来源甲", url: "https://example.test/sub", update_interval_mins: 60,
    fallback_exit: null, last_updated: null, last_result: null,
    nodes: [node("节点一"), node("节点二")],
  }],
  manual_nodes: [],
  node_groups: [],
  listeners: [],
  exits: [],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  process_routing: { enabled: false, on_routa_exit: "Block", rules: [] },
} as unknown as Model;
const status = { core: "Running", delays: {} } as unknown as RuntimeStatus;

function setup(value: NodeRef | null = null) {
  const onPick = vi.fn(), onClose = vi.fn();
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  qc.setQueryData(["model"], model);
  qc.setQueryData(["status"], status);
  render(
    <QueryClientProvider client={qc}>
      <NodePickerDialog title="选择节点" value={value} onClose={onClose} onPick={onPick} />
    </QueryClientProvider>,
  );
  return { onPick, onClose };
}

it("picks the clicked node", async () => {
  const { onPick } = setup();
  fireEvent.click(await screen.findByRole("radio", { name: "节点二 · 来源甲" }));
  expect(onPick).toHaveBeenCalledWith(two);
});

it("marks the current value", async () => {
  setup(one);
  expect(await screen.findByRole("radio", { name: "节点一 · 来源甲" })).toHaveAttribute("aria-checked", "true");
  expect(screen.getByRole("radio", { name: "节点二 · 来源甲" })).toHaveAttribute("aria-checked", "false");
});

it("filters cards by search", async () => {
  setup();
  await screen.findByRole("radio", { name: "节点二 · 来源甲" });
  fireEvent.change(screen.getByLabelText("搜索节点、来源或协议"), { target: { value: "节点二" } });
  expect(screen.queryByRole("radio", { name: "节点一 · 来源甲" })).not.toBeInTheDocument();
  expect(screen.getByRole("radio", { name: "节点二 · 来源甲" })).toBeInTheDocument();
});
