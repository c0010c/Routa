import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { Listener, Model, ProcessRule } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
    saveProcessRule: vi.fn(),
    listRunningProcesses: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ProcessRuleDialog } from "./ProcessRuleDialog";

const l = (id: string, name: string, listener_type: Listener["listener_type"], port: number): Listener => ({
  id,
  name,
  listener_type,
  listen: "127.0.0.1",
  port,
  auth: null,
  exit: null,
  user_enabled: true,
});

const model: Model = {
  subscriptions: [],
  exits: [],
  listeners: [l("l1", "浏览器", "Mixed", 7891), l("l2", "网页", "Http", 7892), l("l3", "下载", "Socks", 7893)],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [],
  node_groups: [],
  process_routing: { enabled: true, on_routa_exit: "Block", rules: [] },
};

/** A rule migrated from the old one-exe shape: matched by full path, blocks on fault. */
const rule: ProcessRule = {
  id: "r1",
  name: "",
  processes: ["C:\\Apps\\curl.exe"],
  match_full_path: true,
  listener: "l1",
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

describe("ProcessRuleDialog", () => {
  beforeEach(() => {
    vi.mocked(commands.getModel).mockResolvedValue(model);
    vi.mocked(commands.listRunningProcesses).mockResolvedValue([
      { pid: 42, name: "curl.exe", exe_path: "C:\\Apps\\curl.exe" },
    ]);
  });

  it("listener select excludes http listeners", async () => {
    renderDialog();
    await loaded();
    fireEvent.mouseDown(screen.getByRole("combobox", { name: /监听器/ }));
    const list = await screen.findByRole("listbox");
    const names = within(list)
      .getAllByRole("option")
      .map((o) => o.textContent);
    expect(names.some((n) => n?.includes("浏览器"))).toBe(true);
    expect(names.some((n) => n?.includes("下载"))).toBe(true);
    expect(names.some((n) => n?.includes("网页"))).toBe(false);
  });

  it("points to the listeners page when no mixed or socks listener exists", async () => {
    vi.mocked(commands.getModel).mockResolvedValue({ ...model, listeners: [l("l2", "网页", "Http", 7892)] });
    renderDialog();
    await loaded();
    expect(screen.getByRole("link", { name: "监听器" })).toHaveAttribute("href", "/listeners");
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
      screen.getByText("只在 mihomo 或分流组件意外故障时生效；Listener 因节点失效、网卡断开被停用时，始终阻断"),
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

  it("maps listener type error to listener field", async () => {
    vi.mocked(commands.saveProcessRule).mockResolvedValue({
      status: "error",
      error: [{ ListenerTypeNotAllowed: { listener: "浏览器" } }],
    });
    renderDialog(rule);
    await loaded();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("Listener「浏览器」是 HTTP 类型，不能用于进程分流")).toBeInTheDocument();
  });

  it("saves typed process names and closes", async () => {
    vi.mocked(commands.saveProcessRule).mockResolvedValue({ status: "ok", data: null });
    vi.mocked(commands.saveProcessRule).mockClear();
    const onClose = renderDialog();
    await loaded();
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
      listener: "l1",
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

  it("shows unsupported IPv6 entry errors on the listener field", async () => {
    vi.mocked(commands.saveProcessRule).mockResolvedValue({
      status: "error",
      error: [{ ListenerAddressNotAllowed: { listener: "浏览器", listen: "::1" } }],
    });
    const onClose = renderDialog(rule);
    await loaded();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("Listener「浏览器」的监听地址「::1」不能用于进程分流：仅支持 IPv4 入口，请修改监听地址或选择其他 Listener")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });
});
