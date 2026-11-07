import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { Model, NodeRef, ProcessRule } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
    testDelays: vi.fn(),
    saveProcessRule: vi.fn(),
    saveProcessRuleConfirmed: vi.fn(),
    removeLearnedProcessPath: vi.fn(),
    listRunningProcesses: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { ProcessRuleDialog } from "./ProcessRuleDialog";

const n1: NodeRef = { subscription_id: "s", name: "n1" };
const CANARY = "C:\\Users\\Q\\AppData\\Local\\Google\\Chrome SxS\\Application\\chrome.exe";
const OLD = "C:\\Apps\\Codex\\1.0\\a.exe";

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
  process_routing: { enabled: true, rules: [], exit_prompt: true },
} as unknown as Model;

/** A rule matched by full path that blocks when Routa or a component is unavailable. */
const rule: ProcessRule = {
  id: "r1",
  name: "",
  processes: [{ kind: "Path", value: "C:\\Apps\\curl.exe" }],
  learned: [],
  node: n1,
  on_fault: "Block",
  reset_on_change: false,
  user_enabled: true,
};

const canaryRule: ProcessRule = {
  ...rule,
  id: "r2",
  name: "Chrome Canary",
  processes: [{ kind: "Path", value: CANARY }],
  on_fault: "Direct",
};

const withRules = (...rules: ProcessRule[]): Model => ({ ...model, process_routing: { ...model.process_routing, rules } });

function renderDialog(r?: ProcessRule, onClose = vi.fn()) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <ApplyResultProvider>
        <MemoryRouter>
          <ProcessRuleDialog open rule={r} onClose={onClose} />
        </MemoryRouter>
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
  return onClose;
}

async function loaded() {
  await waitFor(() => expect(commands.getModel).toHaveBeenCalled());
  await waitFor(() => expect(commands.listRunningProcesses).toHaveBeenCalled());
  await new Promise((r) => setTimeout(r, 0));
}

async function pickNode(name: string | RegExp) {
  fireEvent.click(screen.getByRole("button", { name: /节点/ }));
  fireEvent.click(await screen.findByRole("radio", { name }));
}

function addProcess(v: string) {
  const input = screen.getByRole("combobox", { name: "进程" });
  fireEvent.change(input, { target: { value: v } });
  fireEvent.keyDown(input, { key: "Enter" });
}

const saved = () => vi.mocked(commands.saveProcessRuleConfirmed).mock.calls[0][0];

describe("ProcessRuleDialog", () => {
  beforeEach(() => {
    vi.mocked(commands.getModel).mockResolvedValue(model);
    vi.mocked(commands.listRunningProcesses).mockResolvedValue([
      { pid: 42, name: "curl.exe", exe_path: "C:\\Apps\\curl.exe" },
    ]);
    vi.mocked(commands.saveProcessRuleConfirmed).mockReset();
    vi.mocked(commands.saveProcessRuleConfirmed).mockResolvedValue({ status: "ok", data: null });
    vi.mocked(commands.saveProcessRule).mockClear();
    vi.mocked(commands.removeLearnedProcessPath).mockReset();
  });

  it("lays out the fields in order without advanced options", async () => {
    renderDialog();
    await loaded();
    const fields = [
      screen.getByRole("textbox", { name: "规则名称（可选）" }),
      screen.getByRole("combobox", { name: "进程" }),
      screen.getByRole("button", { name: /节点/ }),
      screen.getByRole("combobox", { name: "网卡" }),
      screen.getByRole("switch", { name: "启用" }),
      screen.getByRole("radiogroup", { name: "Routa 或组件不可用时" }),
      screen.getByRole("switch", { name: "规则变化时断开旧连接" }),
    ];
    for (let i = 1; i < fields.length; i++) {
      expect(fields[i - 1].compareDocumentPosition(fields[i]) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    }
    expect(screen.queryByText("高级选项")).not.toBeInTheDocument();
    expect(screen.queryByText("按完整路径匹配")).not.toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "直连" })).toBeChecked();
  });

  it("allows blocking with only process names", async () => {
    renderDialog();
    await loaded();
    addProcess("chrome.exe");
    expect(screen.getByRole("radio", { name: "阻断" })).toBeEnabled();
    fireEvent.click(screen.getByRole("radio", { name: "阻断" }));
    expect(screen.getByRole("radio", { name: "阻断" })).toBeChecked();
  });

  it("notes locations another enabled rule owns when blocking", async () => {
    vi.mocked(commands.getModel).mockResolvedValue(withRules(canaryRule));
    renderDialog();
    await loaded();
    addProcess("chrome.exe");
    expect(screen.queryByText(/不受这条规则阻断/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("radio", { name: "阻断" }));
    expect(screen.getByText("chrome.exe 在「Chrome Canary」指定的位置不受这条规则阻断。")).toBeInTheDocument();
  });

  it("info_toast_on_newly_learning", async () => {
    vi.mocked(commands.getModel).mockResolvedValue(withRules(canaryRule));
    const onClose = renderDialog();
    await loaded();
    addProcess("chrome.exe");
    await pickNode("n1 · 来源甲");
    fireEvent.click(screen.getByRole("radio", { name: "阻断" }));
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(saved()).toMatchObject({ processes: [{ kind: "Name", value: "chrome.exe" }], on_fault: "Block" });
    const toast = await screen.findByText(/已开启按进程名阻断/);
    expect(toast.closest(".MuiAlert-root")).toHaveClass("MuiAlert-colorInfo");
    expect(toast).toHaveTextContent("chrome.exe 联网时会自动记录位置，分流服务停止后也能拦住；还没联网过的位置除外。");
    expect(toast).toHaveTextContent("「Chrome Canary」指定的 chrome.exe 不受影响。");
  });

  it("no info toast when the saved rule or process routing is off", async () => {
    const routingOff: Model = { ...withRules(), process_routing: { ...withRules().process_routing, enabled: false } };
    for (const [m, ruleOn] of [
      [routingOff, true],
      [withRules(), false],
    ] as const) {
      vi.mocked(commands.getModel).mockResolvedValue(m);
      const onClose = renderDialog();
      await loaded();
      addProcess("chrome.exe");
      await pickNode("n1 · 来源甲");
      fireEvent.click(screen.getByRole("radio", { name: "阻断" }));
      if (!ruleOn) fireEvent.click(screen.getByRole("switch", { name: "启用" }));
      fireEvent.click(screen.getByRole("button", { name: "保存" }));
      await waitFor(() => expect(onClose).toHaveBeenCalled());
      expect(saved()).toMatchObject({ on_fault: "Block", user_enabled: ruleOn });
      await new Promise((r) => setTimeout(r, 0));
      expect(screen.queryByText(/已开启按进程名阻断/)).not.toBeInTheDocument();
      cleanup();
      vi.mocked(commands.saveProcessRuleConfirmed).mockClear();
    }
  });

  it("no_info_toast_when_nothing_newly_learning", async () => {
    const learning: ProcessRule = { ...rule, processes: [{ kind: "Name", value: "a.exe" }] };
    vi.mocked(commands.getModel).mockResolvedValue(withRules(learning));
    const onClose = renderDialog(learning);
    await loaded();
    await pickNode(/家里/);
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(saved().node).toEqual({ manual_id: "ts" });
    await new Promise((r) => setTimeout(r, 0));
    expect(screen.queryByText(/已开启按进程名阻断/)).not.toBeInTheDocument();
  });

  it("shows learned locations when editing a learning rule; 移除 calls removeLearnedProcessPath", async () => {
    vi.mocked(commands.removeLearnedProcessPath).mockResolvedValue({ status: "ok", data: null });
    const learning: ProcessRule = { ...rule, processes: [{ kind: "Name", value: "a.exe" }], learned: [OLD] };
    vi.mocked(commands.getModel).mockResolvedValue(withRules(learning));
    renderDialog(learning);
    await loaded();
    fireEvent.click(screen.getByRole("button", { name: /已记录 1 个位置/ }));
    fireEvent.click(screen.getByRole("button", { name: `移除 ${OLD}` }));
    await waitFor(() => expect(commands.removeLearnedProcessPath).toHaveBeenCalledWith("r1", OLD));
  });

  it("keeps the reset hint behind a help icon", async () => {
    renderDialog(rule);
    await loaded();
    expect(screen.getByRole("switch", { name: "规则变化时断开旧连接" })).not.toBeChecked();
    expect(screen.queryByText(/仅对 IPv4 TCP 连接有效/)).not.toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "阻断" })).toBeChecked();
  });

  it("maps duplicate error to the process field", async () => {
    vi.mocked(commands.saveProcessRuleConfirmed).mockResolvedValue({
      status: "error",
      error: [{ DuplicateProcessRule: { exe_path: "C:\\Apps\\curl.exe" } }],
    });
    const onClose = renderDialog(rule);
    await loaded();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("C:\\Apps\\curl.exe 已在其他规则中，同一个进程名或路径只能属于一条规则")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "进程" })).toHaveAttribute("aria-invalid", "true");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("saves through save_process_rule_confirmed, not saveProcessRule", async () => {
    const onClose = renderDialog(rule);
    await loaded();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(commands.saveProcessRuleConfirmed).toHaveBeenCalledWith(expect.objectContaining({ id: "r1" }));
    expect(commands.saveProcessRule).not.toHaveBeenCalled();
  });

  it("keeps the dialog open and shows the message when the save is not confirmed", async () => {
    const text = "规则保存未能确认生效，已恢复原状态，请检查组件状态后重试";
    vi.mocked(commands.saveProcessRuleConfirmed).mockResolvedValue({ status: "error", error: text });
    const onClose = renderDialog(rule);
    await loaded();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(text);
    expect(onClose).not.toHaveBeenCalled();
  });

  it("saves typed entries with their kinds and closes", async () => {
    const onClose = renderDialog();
    await loaded();
    await pickNode("n1 · 来源甲");
    addProcess("ChatGPT.exe");
    addProcess("D:\\Tools\\codex.exe");
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(saved()).toMatchObject({
      name: "",
      processes: [
        { kind: "Name", value: "ChatGPT.exe" },
        { kind: "Path", value: "D:\\Tools\\codex.exe" },
      ],
      learned: [],
      on_fault: "Direct",
      reset_on_change: false,
    });
    expect(saved()).not.toHaveProperty("match_full_path");
  });

  it("saves text still in the process box as one more entry", async () => {
    const onClose = renderDialog();
    await loaded();
    await pickNode("n1 · 来源甲");
    addProcess("ChatGPT.exe");
    fireEvent.change(screen.getByRole("combobox", { name: "进程" }), { target: { value: "codex.exe" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(saved().processes).toEqual([
      { kind: "Name", value: "ChatGPT.exe" },
      { kind: "Name", value: "codex.exe" },
    ]);
  });

  it("does not save when the text left in the process box is invalid", async () => {
    const onClose = renderDialog();
    await loaded();
    await pickNode("n1 · 来源甲");
    fireEvent.change(screen.getByRole("combobox", { name: "进程" }), { target: { value: "notes.txt" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText(/notes\.txt 不是 \.exe 程序/)).toBeInTheDocument();
    expect(screen.queryByText("至少添加一个进程")).not.toBeInTheDocument();
    expect(commands.saveProcessRuleConfirmed).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("requires at least one process", async () => {
    renderDialog();
    await loaded();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("至少添加一个进程")).toBeInTheDocument();
    expect(commands.saveProcessRuleConfirmed).not.toHaveBeenCalled();
  });

  it("requires a node and does not save without one", async () => {
    renderDialog();
    await loaded();
    addProcess("a.exe");
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("请选择一个节点")).toBeInTheDocument();
    expect(commands.saveProcessRuleConfirmed).not.toHaveBeenCalled();
  });

  it("saves the picked node with a null interface", async () => {
    const onClose = renderDialog();
    await loaded();
    addProcess("a.exe");
    await pickNode("n1 · 来源甲");
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(saved().node).toEqual(n1);
    expect(saved().interface).toBeNull();
  });

  it("locks the NIC for a Tailscale node and shows the instance NIC", async () => {
    const onClose = renderDialog({ ...rule, interface: "{OTHER}" });
    await loaded();
    expect(screen.getByRole("combobox", { name: "网卡" })).not.toHaveAttribute("aria-disabled", "true");
    await pickNode(/家里/);
    expect(screen.getByRole("combobox", { name: "网卡" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByText("在 Tailscale 节点设置里修改")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(saved().node).toEqual({ manual_id: "ts" });
    expect(saved().interface).toBeNull();
  });
});
