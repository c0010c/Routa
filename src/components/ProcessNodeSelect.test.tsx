import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import type { Model, ProcessRule } from "../bindings";
import { ProcessNodeSelect } from "./ProcessNodeSelect";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const model = {
  subscriptions: [],
  exits: [],
  listeners: [],
  settings: {},
  manual_nodes: [
    { id: "m1", name: "东京", kind: { Proxy: { raw: "ss://x" } } },
    { id: "m2", name: "大阪", kind: { Proxy: { raw: "ss://y" } } },
  ],
  node_groups: [],
  process_routing: { enabled: true, rules: [] },
} as unknown as Model;

const rule: ProcessRule = {
  id: "r1",
  name: "",
  processes: [{ kind: "Name", value: "chrome.exe" }],
  learned: [],
  node: { manual_id: "m1" },
  on_fault: "Direct",
  reset_on_change: false,
  user_enabled: true,
};

function mount(r: ProcessRule, reason?: string | null, m: Model = model, status?: unknown) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: Infinity }, mutations: { retry: false } },
  });
  qc.setQueryData(["model"], m);
  if (status) qc.setQueryData(["status"], status);
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <ProcessNodeSelect rule={r} model={m} reason={reason} />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

beforeEach(() => vi.mocked(invoke).mockReset());

it("shows the unset-node state as an error button", () => {
  mount({ ...rule, node: null });
  const btn = screen.getByRole("button", { name: "未选择节点，请选择节点" });
  expect(btn).toHaveClass("MuiButton-colorError");
});

it("shows the broken-node reason", () => {
  mount(rule, "节点已失效，请更换节点");
  expect(
    screen.getByRole("button", { name: "节点已失效，请更换节点" }),
  ).toBeInTheDocument();
});

it("shows the node name normally", () => {
  mount(rule);
  expect(screen.getByRole("button", { name: "东京" })).not.toHaveClass(
    "MuiButton-colorError",
  );
});

it("changes the node via change_process_node", async () => {
  vi.mocked(invoke).mockResolvedValue(undefined);
  mount(rule);
  fireEvent.click(screen.getByRole("button", { name: "东京" }));
  fireEvent.click(await screen.findByText("大阪"));
  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith("change_process_node", {
      id: "r1",
      node: { manual_id: "m2" },
    }),
  );
});

it("shows the error and retries the failed target", async () => {
  let fail = true;
  vi.mocked(invoke).mockImplementation(async (cmd) => {
    if (cmd === "change_process_node" && fail) {
      fail = false;
      throw "服务未确认，已恢复原节点";
    }
  });
  mount(rule);
  fireEvent.click(screen.getByRole("button", { name: "东京" }));
  fireEvent.click(await screen.findByText("大阪"));
  expect(await screen.findByRole("alert")).toHaveTextContent("已恢复原节点");
  fireEvent.click(screen.getByRole("button", { name: "重试" }));
  await waitFor(() =>
    expect(
      vi.mocked(invoke).mock.calls.filter((c) => c[0] === "change_process_node"),
    ).toEqual([
      ["change_process_node", { id: "r1", node: { manual_id: "m2" } }],
      ["change_process_node", { id: "r1", node: { manual_id: "m2" } }],
    ]),
  );
  await waitFor(() =>
    expect(screen.queryByRole("alert")).not.toBeInTheDocument(),
  );
});

const withHidden = {
  ...model,
  exits: [
    { id: "e1", name: "__proc_exit_l1", kind: { Node: { node: { manual_id: "m1" } } }, interface: null, hidden: true },
  ],
  listeners: [
    { id: "l1", name: "__proc_l1", listener_type: "Socks", listen: "127.0.0.1", port: 37800, auth: null, exit: "e1", user_enabled: true, hidden: true },
  ],
  settings: { delay_test_url: "https://example.test/204" },
} as unknown as Model;
const running = {
  core: "Running",
  delays: { __proc_exit_l1: { ms: 123, error: null, tested_at: 1, stale: false } },
  interfaces: [],
};

it("shows the rule's proxy delay and re-measures it through the hidden exit", async () => {
  vi.mocked(invoke).mockImplementation(async (cmd) => (cmd === "get_status" ? running : []));
  mount(rule, null, withHidden, running);
  const measure = screen.getByRole("button", { name: "测量 chrome.exe 的代理延迟" });
  expect(measure).toHaveTextContent("123");
  expect(document.body.textContent).not.toContain("__proc");
  fireEvent.click(measure);
  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith("test_delays", { proxyNames: ["__proc_exit_l1"] }),
  );
});

it("hides the delay when the rule's node is broken or unset", () => {
  mount(rule, "节点已失效，请更换节点", withHidden, running);
  expect(screen.queryByRole("button", { name: /代理延迟/ })).not.toBeInTheDocument();
});

it("does not match a hidden exit with a different interface", () => {
  mount({ ...rule, interface: "{NIC}" }, null, withHidden, running);
  expect(screen.queryByRole("button", { name: /代理延迟/ })).not.toBeInTheDocument();
});
