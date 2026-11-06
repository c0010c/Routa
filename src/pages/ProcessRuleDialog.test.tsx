import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { Model, NodeRef, ProcessRule } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
    testDelays: vi.fn(),
    saveProcessRule: vi.fn(),
    listRunningProcesses: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ProcessRuleDialog } from "./ProcessRuleDialog";

const n1: NodeRef = { subscription_id: "s", name: "n1" };

const model = {
  subscriptions: [
    { id: "s", name: "来源甲", url: "https://example.test/sub", update_interval_mins: 60, fallback_exit: null, last_updated: null, last_result: null,
      nodes: [{ name: "n1", proxy_type: "ss", server: "example.test", raw: {} }] },
  ],
  exits: [],
  listeners: [],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [
    { id: "ts", name: "家里", groups: [], kind: { Tailscale: { hostname: "h", exit_node: "x", ephemeral: false, udp: true, auth_key: null, interface: "{NIC-TS}" } } },
  ],
  node_groups: [],
  process_routing: { enabled: true, on_routa_exit: "Block", rules: [] },
} as unknown as Model;

/** A rule migrated from the old one-exe shape: matched by full path, blocks on fault. */
const rule: ProcessRule = {
  id: "r1",
  name: "",
  processes: ["C:\\Apps\\curl.exe"],
  match_full_path: true,
  node: n1,
  on_fault: "Block",
  reset_on_change: false,
  user_enabled: true,
};

function renderDialog(r?: ProcessRule, onClose = vi.fn()) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <ProcessRuleDialog open rule={r} onClose={onClose} />
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return onClose;
}

async function loaded() {
  await waitFor(() => expect(commands.getModel).toHaveBeenCalled());
  await waitFor(() => expect(commands.listRunningProcesses).toHaveBeenCalled());
  await new Promise((r) => setTimeout(r, 0));
}

async function pickNode(name: string) {
  fireEvent.click(screen.getByRole("button", { name: /节点/ }));
  fireEvent.click(await screen.findByRole("radio", { name }));
}

describe("ProcessRuleDialog", () => {
  beforeEach(() => {
    vi.mocked(commands.getModel).mockResolvedValue(model);
    vi.mocked(commands.listRunningProcesses).mockResolvedValue([
      { pid: 42, name: "curl.exe", exe_path: "C:\\Apps\\curl.exe" },
    ]);
  });

  it("shows ipv4 hint when reset enabled", async () => {
    renderDialog(rule);
    await loaded();
    expect(screen.queryByText(/仅对 IPv4 TCP 连接有效/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("switch", { name: "规则变化时断开旧连接" }));
    expect(screen.getByText("仅对 IPv4 TCP 连接有效；更换代理目标不触发断开")).toBeInTheDocument();
  });

  it("shows fault policy explanation", async () => {
    renderDialog(rule);
    await loaded();
    expect(
      screen.getByText("只在 mihomo 或分流组件意外故障时生效；节点失效、网卡断开时，始终阻断"),
    ).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "阻断" })).toBeChecked();
  });

  it("maps duplicate error to path field", async () => {
    vi.mocked(commands.saveProcessRule).mockResolvedValue({
      status: "error",
      error: [{ DuplicateProcessRule: { exe_path: "C:\\Apps\\curl.exe" } }],
    });
    const onClose = renderDialog(rule);
    await loaded();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("C:\\Apps\\curl.exe 已在其他规则中，一个程序只能属于一条规则")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "进程" })).toHaveAttribute("aria-invalid", "true");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("saves typed process names and closes", async () => {
    vi.mocked(commands.saveProcessRule).mockResolvedValue({ status: "ok", data: null });
    vi.mocked(commands.saveProcessRule).mockClear();
    const onClose = renderDialog();
    await loaded();
    await pickNode("n1 · 来源甲");
    const input = screen.getByRole("combobox", { name: "进程" });
    fireEvent.change(input, { target: { value: "ChatGPT.exe" } });
    fireEvent.keyDown(input, { key: "Enter" });
    // A pasted path is cut down to its file name when matching by name; text left in the box is saved too.
    fireEvent.change(input, { target: { value: "D:\\Tools\\codex.exe" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    const saved = vi.mocked(commands.saveProcessRule).mock.calls[0][0];
    expect(saved).toMatchObject({
      name: "",
      processes: ["ChatGPT.exe", "codex.exe"],
      match_full_path: false,
      on_fault: "Direct",
      reset_on_change: false,
    });
  });

  it("warns about name matching and only allows blocking with full paths", async () => {
    renderDialog();
    await loaded();
    expect(screen.getByText(/按进程名匹配：同名程序装在哪里都会被分流/)).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "直连" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "阻断" })).toBeDisabled();
    // Turning on full paths swaps a running program's name for its path and allows blocking.
    const input = screen.getByRole("combobox", { name: "进程" });
    fireEvent.change(input, { target: { value: "curl.exe" } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.click(screen.getByRole("switch", { name: "按完整路径匹配" }));
    expect(screen.getByRole("button", { name: "C:\\Apps\\curl.exe" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "阻断" })).toBeEnabled();
    expect(screen.queryByText(/按进程名匹配：同名程序装在哪里都会被分流/)).not.toBeInTheDocument();
  });

  it("refuses a program that already belongs to another rule", async () => {
    const other: ProcessRule = { ...rule, id: "r2", name: "AI 工具", processes: ["ChatGPT.exe"], match_full_path: false, on_fault: "Direct" };
    vi.mocked(commands.getModel).mockResolvedValue({ ...model, process_routing: { ...model.process_routing, rules: [other] } });
    renderDialog();
    await loaded();
    const input = screen.getByRole("combobox", { name: "进程" });
    fireEvent.change(input, { target: { value: "chatgpt.EXE" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(await screen.findByText("chatgpt.EXE 已在规则「AI 工具」中，一个程序只能属于一条规则")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "chatgpt.EXE" })).not.toBeInTheDocument();
  });

  it("requires at least one process", async () => {
    vi.mocked(commands.saveProcessRule).mockClear();
    renderDialog();
    await loaded();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("至少添加一个进程")).toBeInTheDocument();
    expect(commands.saveProcessRule).not.toHaveBeenCalled();
  });

  it("requires a node and does not save without one", async () => {
    vi.mocked(commands.saveProcessRule).mockClear();
    renderDialog();
    await loaded();
    const input = screen.getByRole("combobox", { name: "进程" });
    fireEvent.change(input, { target: { value: "a.exe" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("请选择一个节点")).toBeInTheDocument();
    expect(commands.saveProcessRule).not.toHaveBeenCalled();
  });

  it("saves the picked node with a null interface", async () => {
    vi.mocked(commands.saveProcessRule).mockResolvedValue({ status: "ok", data: null });
    vi.mocked(commands.saveProcessRule).mockClear();
    const onClose = renderDialog();
    await loaded();
    fireEvent.change(screen.getByRole("combobox", { name: "进程" }), { target: { value: "a.exe" } });
    await pickNode("n1 · 来源甲");
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    const saved = vi.mocked(commands.saveProcessRule).mock.calls[0][0];
    expect(saved.node).toEqual(n1);
    expect(saved.interface).toBeNull();
  });

  it("locks the exit NIC for a Tailscale node and shows the instance NIC", async () => {
    vi.mocked(commands.saveProcessRule).mockResolvedValue({ status: "ok", data: null });
    vi.mocked(commands.saveProcessRule).mockClear();
    const onClose = renderDialog({ ...rule, interface: "{OTHER}" });
    await loaded();
    fireEvent.click(screen.getByText("高级选项"));
    expect(screen.getByRole("combobox", { name: "出口网卡" })).not.toHaveAttribute("aria-disabled", "true");
    fireEvent.click(screen.getByRole("button", { name: /节点/ }));
    fireEvent.click(await screen.findByRole("radio", { name: /家里/ }));
    const nic = screen.getByRole("combobox", { name: "出口网卡" });
    expect(nic).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByText("在 Tailscale 节点设置里修改")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    const saved = vi.mocked(commands.saveProcessRule).mock.calls[0][0];
    expect(saved.node).toEqual({ manual_id: "ts" });
    expect(saved.interface).toBeNull();
  });
});
