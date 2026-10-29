import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter, Route, Routes, useLocation } from "react-router-dom";
import type { Model, RuntimeStatus } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(), getStatus: vi.fn(), deleteExit: vi.fn(),
    saveListener: vi.fn(), saveSubscription: vi.fn(), groupNow: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { Exits } from "./Exits";
import { Nodes } from "./Nodes";
import { Overview } from "./Overview";

const initialModel: Model = {
  exits: [
    { id: "e1", name: "直连", kind: "Direct", interface: null },
    { id: "e2", name: "备用", kind: "Direct", interface: null },
  ],
  listeners: [{ id: "l1", name: "默认 2", listener_type: "Mixed", listen: "127.0.0.1", port: 7890, auth: null, exit: "e1", user_enabled: false }],
  subscriptions: [{ id: "s1", name: "机场 A", url: "https://example.com/sub", update_interval_mins: null, fallback_exit: "e1", last_updated: null, last_result: null, nodes: [] }],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [], node_groups: [],
  process_routing: { enabled: false, on_routa_exit: "Block", rules: [] },
};
const status: RuntimeStatus = {
  core: "Running", last_apply: null, health: { exits: {}, listeners: {} }, delays: {}, traffic: null,
  interfaces: [], failed_listeners: [], effective_listeners: [], startup_error: null, save_error: null,
  tailscale: {}, tailscale_exit_errors: {}, tailscale_exit_retrying: {}, core_stop_reason: null, core_recovering: false,
};
const applied = { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } };
let currentModel: Model;

function Location() {
  const location = useLocation();
  return <output data-testid="location">{location.pathname}{location.search}</output>;
}

function renderFlow(path = "/exits") {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  render(
    <QueryClientProvider client={qc}>
      <ApplyResultProvider>
        <MemoryRouter initialEntries={[path]}>
          <Location />
          <Routes>
            <Route path="/exits" element={<Exits />} />
            <Route path="/" element={<Overview />} />
            <Route path="/nodes" element={<Nodes />} />
          </Routes>
        </MemoryRouter>
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
  return qc;
}

async function openDelete() {
  fireEvent.click(await screen.findByRole("button", { name: "操作 直连" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "删除 直连" }));
  return screen.findByRole("dialog");
}

beforeEach(() => {
  vi.clearAllMocks();
  currentModel = structuredClone(initialModel);
  vi.mocked(commands.getModel).mockImplementation(async () => currentModel);
  vi.mocked(commands.getStatus).mockResolvedValue(status);
  vi.mocked(commands.groupNow).mockResolvedValue({ status: "ok", data: {} });
  vi.mocked(commands.deleteExit).mockImplementation(async (id) => {
    currentModel = { ...currentModel, exits: currentModel.exits.filter((e) => e.id !== id) };
    return { status: "ok", data: applied };
  });
  vi.mocked(commands.saveListener).mockImplementation(async (listener) => {
    currentModel = { ...currentModel, listeners: currentModel.listeners.map((l) => l.id === listener.id ? listener : l) };
    return { status: "ok", data: applied };
  });
  vi.mocked(commands.saveSubscription).mockImplementation(async (subscription) => {
    currentModel = { ...currentModel, subscriptions: currentModel.subscriptions.map((s) => s.id === subscription.id ? subscription : s) };
    return { status: "ok", data: applied };
  });
});

describe("exit deletion reference guidance", () => {
  it.each(["listener", "subscription", "both"])("blocks before submitting with %s references, including disabled users", async (kind) => {
    if (kind === "listener") currentModel.subscriptions = [];
    if (kind === "subscription") currentModel.listeners = [];
    renderFlow();
    const dialog = await openDelete();
    expect(within(dialog).getByText("暂时无法删除「直连」")).toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "删除" })).not.toBeInTheDocument();
    if (kind !== "subscription") {
      expect(within(dialog).getByText("默认 2")).toBeInTheDocument();
      expect(within(dialog).getByRole("link", { name: "处理监听器「默认 2」" })).toHaveAttribute("href", "/?highlight=l1&resolveExit=e1");
    }
    if (kind !== "listener") {
      expect(within(dialog).getByText("机场 A")).toBeInTheDocument();
      expect(within(dialog).getByRole("link", { name: "处理订阅「机场 A」" })).toHaveAttribute("href", "/nodes?editSubscription=s1&resolveExit=e1");
    }
    expect(commands.deleteExit).not.toHaveBeenCalled();
  });

  it("keeps all users visible and follows live model updates", async () => {
    currentModel.listeners.push({ ...currentModel.listeners[0], id: "l2", name: "办公", user_enabled: true });
    const qc = renderFlow();
    const dialog = await openDelete();
    expect(within(dialog).getByText("监听器（2）")).toBeInTheDocument();
    expect(within(dialog).getByRole("link", { name: "处理监听器「办公」" })).toHaveAttribute("href", "/?highlight=l2&resolveExit=e1");
    act(() => qc.setQueryData(["model"], { ...currentModel, listeners: [], subscriptions: [] }));
    expect(await screen.findByRole("button", { name: "删除" })).toBeEnabled();
    act(() => qc.setQueryData(["model"], currentModel));
    expect(await screen.findByText("暂时无法删除「直连」")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "删除" })).not.toBeInTheDocument();
  });

  it("requires confirmation and locks the dialog while submitting", async () => {
    currentModel.listeners = [];
    currentModel.subscriptions = [];
    let finish = () => {};
    vi.mocked(commands.deleteExit).mockImplementationOnce(() => new Promise((resolve) => {
      finish = () => resolve({ status: "ok", data: applied });
    }));
    renderFlow();
    await openDelete();
    expect(commands.deleteExit).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "删除" }));
    expect(await screen.findByRole("button", { name: "删除中…" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "取消" })).toBeDisabled();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(commands.deleteExit).toHaveBeenCalledExactlyOnceWith("e1");
    await act(async () => finish());
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("locates the existing listener exit, saves 不接 and returns for a separate delete confirmation", async () => {
    currentModel.subscriptions = [];
    renderFlow();
    const dialog = await openDelete();
    fireEvent.click(within(dialog).getByRole("link", { name: "处理监听器「默认 2」" }));
    const select = await screen.findByRole("combobox", { name: "默认 2 出口" });
    await waitFor(() => expect(select).toHaveFocus());
    expect(select.closest("tr")).toHaveClass("Mui-selected");
    fireEvent.mouseDown(select);
    fireEvent.click(await screen.findByRole("option", { name: "不接" }));
    await waitFor(() => expect(commands.saveListener).toHaveBeenCalledWith(expect.objectContaining({ id: "l1", exit: null })));
    const back = screen.getByRole("link", { name: "返回删除检查" });
    await waitFor(() => expect(back).not.toHaveAttribute("aria-disabled", "true"));
    fireEvent.click(back);
    expect(await screen.findByRole("button", { name: "删除" })).toBeEnabled();
    expect(commands.deleteExit).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "删除" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.queryByRole("button", { name: "操作 直连" })).not.toBeInTheDocument();
    expect(commands.deleteExit).toHaveBeenCalledExactlyOnceWith("e1");
  });

  it("opens the requested subscription once, focuses its fallback and returns after saving 不使用", async () => {
    currentModel.listeners = [];
    currentModel.subscriptions.unshift({ ...currentModel.subscriptions[0], id: "other", name: "其他订阅", fallback_exit: null });
    const qc = renderFlow();
    const dialog = await openDelete();
    fireEvent.click(within(dialog).getByRole("link", { name: "处理订阅「机场 A」" }));
    const edit = await screen.findByRole("dialog");
    expect(within(edit).getByLabelText("名称")).toHaveValue("机场 A");
    const fallback = within(edit).getByRole("combobox", { name: "备用出口" });
    await waitFor(() => expect(fallback).toHaveFocus());
    expect(screen.getByTestId("location")).toHaveTextContent("/nodes?resolveExit=e1");
    fireEvent.mouseDown(fallback);
    fireEvent.click(await screen.findByRole("option", { name: "不使用" }));
    fireEvent.click(within(edit).getByRole("button", { name: "保存" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(commands.saveSubscription).toHaveBeenCalledWith(expect.objectContaining({ id: "s1", fallback_exit: null, url: "https://example.com/sub" }));
    await act(async () => { await qc.invalidateQueries({ queryKey: ["model"] }); });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("link", { name: "返回删除检查" }));
    expect(await screen.findByRole("button", { name: "删除" })).toBeEnabled();
    expect(commands.deleteExit).not.toHaveBeenCalled();
  });

  it("keeps 返回删除检查 disabled while an inline listener save is pending", async () => {
    let finish = () => {};
    vi.mocked(commands.saveListener).mockImplementationOnce(() => new Promise((resolve) => {
      finish = () => resolve({ status: "ok", data: applied });
    }));
    renderFlow("/?highlight=l1&resolveExit=e1");
    fireEvent.mouseDown(await screen.findByRole("combobox", { name: "默认 2 出口" }));
    fireEvent.click(await screen.findByRole("option", { name: "备用" }));
    const back = screen.getByRole("link", { name: "返回删除检查" });
    await waitFor(() => expect(back).toHaveAttribute("aria-disabled", "true"));
    await act(async () => finish());
    await waitFor(() => expect(back).not.toHaveAttribute("aria-disabled", "true"));
  });

  it("preserves the existing highlight entry without forcing focus or adding a return action", async () => {
    renderFlow("/?highlight=l1");
    const select = await screen.findByRole("combobox", { name: "默认 2 出口" });
    await waitFor(() => expect(select.closest("tr")).toHaveClass("Mui-selected"));
    expect(select).not.toHaveFocus();
    expect(screen.queryByRole("link", { name: "返回删除检查" })).not.toBeInTheDocument();
  });

  it("keeps references when subscription editing is cancelled and does not reopen on a model refresh", async () => {
    const qc = renderFlow();
    const dialog = await openDelete();
    fireEvent.click(within(dialog).getByRole("link", { name: "处理订阅「机场 A」" }));
    const edit = await screen.findByRole("dialog");
    fireEvent.click(within(edit).getByRole("button", { name: "取消" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await act(async () => { await qc.invalidateQueries({ queryKey: ["model"] }); });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("link", { name: "返回删除检查" }));
    const blocked = await screen.findByRole("dialog");
    expect(within(blocked).getByText("默认 2")).toBeInTheDocument();
    expect(within(blocked).getByText("机场 A")).toBeInTheDocument();
    expect(commands.saveSubscription).not.toHaveBeenCalled();
    expect(commands.deleteExit).not.toHaveBeenCalled();
  });

  it("shows a normal failure once inside the dialog and permits retry", async () => {
    currentModel.listeners = [];
    currentModel.subscriptions = [];
    vi.mocked(commands.deleteExit).mockRejectedValueOnce(new Error("连接已断开"));
    renderFlow();
    const dialog = await openDelete();
    fireEvent.click(within(dialog).getByRole("button", { name: "删除" }));
    expect(await screen.findByText("删除失败：连接已断开")).toBeInTheDocument();
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    expect(within(dialog).getByRole("alert")).toHaveTextContent("连接已断开");
    fireEvent.click(within(dialog).getByRole("button", { name: "删除" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(commands.deleteExit).toHaveBeenCalledTimes(2);
  });

  it("rebuilds guidance from refreshed data when the backend reports a reference", async () => {
    currentModel.listeners = [];
    currentModel.subscriptions = [];
    vi.mocked(commands.deleteExit).mockImplementationOnce(async () => {
      currentModel = structuredClone(initialModel);
      return { status: "error", error: [{ DanglingRef: { what: "internal-uuid-must-not-be-shown" } }] };
    });
    renderFlow();
    const dialog = await openDelete();
    fireEvent.click(within(dialog).getByRole("button", { name: "删除" }));
    expect(await screen.findByText("暂时无法删除「直连」")).toBeInTheDocument();
    expect(within(dialog).getByText("默认 2")).toBeInTheDocument();
    expect(screen.queryByText(/internal-uuid/)).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "删除" })).not.toBeInTheDocument();
  });

  it("keeps other validation failures readable inside the dialog", async () => {
    currentModel.listeners = [];
    currentModel.subscriptions = [];
    vi.mocked(commands.deleteExit).mockResolvedValueOnce({
      status: "error", error: [{ CoreRejected: { message: "配置检查未通过" } }],
    });
    renderFlow();
    const dialog = await openDelete();
    fireEvent.click(within(dialog).getByRole("button", { name: "删除" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent("mihomo 拒绝了该配置：配置检查未通过");
    expect(within(dialog).getByRole("button", { name: "删除" })).toBeEnabled();
  });

  it("does not claim runtime success when the existing apply outcome reports a failure", async () => {
    currentModel.listeners = [];
    currentModel.subscriptions = [];
    vi.mocked(commands.deleteExit).mockResolvedValueOnce({
      status: "ok", data: { FailedKeptPrevious: { error: "内核拒绝重载" } },
    });
    renderFlow();
    await openDelete();
    fireEvent.click(screen.getByRole("button", { name: "删除" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(await screen.findByText("已保存，但未生效，目前仍使用原配置")).toBeInTheDocument();
    expect(screen.queryByText(/内核拒绝重载/)).not.toBeInTheDocument();
    expect(screen.queryByText(/删除成功/)).not.toBeInTheDocument();
  });

  it("blocks deletion if rechecking fails, then allows an explicit successful recheck", async () => {
    currentModel.listeners = [];
    currentModel.subscriptions = [];
    vi.mocked(commands.deleteExit).mockResolvedValueOnce({ status: "error", error: [{ DanglingRef: { what: "internal" } }] });
    renderFlow();
    const dialog = await openDelete();
    vi.mocked(commands.getModel).mockRejectedValueOnce(new Error("无法读取配置"));
    fireEvent.click(within(dialog).getByRole("button", { name: "删除" }));
    expect(await screen.findByText(/未能读取最新配置，请重新检查后再删除/)).toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "删除" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "重新检查" }));
    expect(await screen.findByRole("button", { name: "删除" })).toBeEnabled();
    expect(commands.deleteExit).toHaveBeenCalledTimes(1);
  });

  it("does not parse unknown reference strings or enable delete until another check", async () => {
    currentModel.listeners = [];
    currentModel.subscriptions = [];
    vi.mocked(commands.deleteExit).mockResolvedValueOnce({ status: "error", error: [{ DanglingRef: { what: "internal" } }] });
    renderFlow();
    await openDelete();
    fireEvent.click(screen.getByRole("button", { name: "删除" }));
    expect(await screen.findByText("配置引用发生变化，暂时无法删除。请重新检查后重试。")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "删除" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "重新检查" }));
    expect(await screen.findByRole("button", { name: "删除" })).toBeEnabled();
  });

  it.each([
    ["/exits?delete=gone", "该出口已不存在"],
    ["/?highlight=gone&resolveExit=e1", "该监听器已不存在，请返回重新检查。"],
    ["/nodes?editSubscription=gone&resolveExit=e1", "该订阅已不存在，请返回重新检查。"],
  ])("reports a missing target at %s without mutating anything", async (path, message) => {
    renderFlow(path);
    expect(await screen.findByText(message)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "删除" })).not.toBeInTheDocument();
    expect(commands.deleteExit).not.toHaveBeenCalled();
    expect(commands.saveListener).not.toHaveBeenCalled();
    expect(commands.saveSubscription).not.toHaveBeenCalled();
  });

  it("consumes the return parameter so closing is not undone by a model refresh", async () => {
    currentModel.listeners = [];
    currentModel.subscriptions = [];
    const qc = renderFlow("/exits?delete=e1");
    fireEvent.click(await screen.findByRole("button", { name: "取消" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.getByTestId("location")).toHaveTextContent(/^\/exits$/);
    await act(async () => { await qc.invalidateQueries({ queryKey: ["model"] }); });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("reports initial model read failures and resumes the return check after reloading", async () => {
    vi.mocked(commands.getModel).mockRejectedValueOnce(new Error("无法读取配置"));
    renderFlow("/exits?delete=e1");
    expect(await screen.findByRole("alert")).toHaveTextContent("无法读取配置");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "重新加载" }));
    expect(await screen.findByText("暂时无法删除「直连」")).toBeInTheDocument();
    expect(commands.deleteExit).not.toHaveBeenCalled();
  });
});
