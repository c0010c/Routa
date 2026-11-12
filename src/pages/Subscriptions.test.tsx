import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import type { Model, RuntimeStatus, Subscription, SubscriptionPreview } from "../bindings";
import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { Subscriptions } from "./Subscriptions";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../bindings", () => ({ commands: { getModel: vi.fn(), getStatus: vi.fn() } }));
const one: Subscription = { id: "s1", name: "北辰", url: "https://example.test/sub?token=SECRET", nodes: [{ name: "日本", proxy_type: "ss", server: "s", raw: {} }], last_updated: 100, last_result: { Ok: { node_count: 1 } }, update_interval_mins: 60, fallback_exit: null };
const two: Subscription = { ...one, id: "s2", name: "远山", last_result: { Failed: { reason: "下载超时" } } };
const data = { subscriptions: [one, two], listeners: [], exits: [], node_groups: [], manual_nodes: [], process_routing: { enabled: false, rules: [], exit_prompt: true }, settings: { node_monitor: { members: [] } } } as unknown as Model;
function setup(subscriptions: Subscription[] = data.subscriptions) {
  vi.mocked(commands.getModel).mockResolvedValue(structuredClone({ ...data, subscriptions }));
  vi.mocked(commands.getStatus).mockResolvedValue({ core: "Stopped", last_apply: null } as RuntimeStatus);
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  const view = render(<QueryClientProvider client={qc}><ApplyResultProvider><MemoryRouter><Subscriptions /></MemoryRouter></ApplyResultProvider></QueryClientProvider>);
  return { qc, ...view };
}
beforeEach(() => vi.clearAllMocks());
it("shows compact source-only cards with no embedded result alerts", async () => {
  setup(); const card = await screen.findByRole("article", { name: "远山" });
  expect(within(card).getByText("更新失败")).toBeInTheDocument();
  expect(within(card).queryByRole("alert")).not.toBeInTheDocument();
  expect(screen.queryByText(/SECRET/)).not.toBeInTheDocument();
  expect(screen.getAllByText("example.test")).toHaveLength(2);
  expect(within(card).getByRole("link", { name: "查看节点" })).toHaveAttribute("href", "/nodes?source=s2");
});
it("reports failure in a toast outside the card, and only locks the updated subscription", async () => {
  let reject!: (reason: unknown) => void;
  vi.mocked(invoke).mockImplementation(() => new Promise((_, r) => { reject = r; }));
  setup(); const card = await screen.findByRole("article", { name: "远山" });
  fireEvent.click(within(card).getByRole("button", { name: "重试更新" }));
  expect(within(screen.getByRole("article", { name: "北辰" })).getByRole("button", { name: "更新" })).toBeEnabled();
  await act(async () => reject(new Error("下载超时")));
  const alert = await screen.findByRole("alert");
  expect(alert).toHaveTextContent("原有节点未被替换");
  expect(alert.closest("article")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "关闭结果提示" }));
  await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
});
it("searches only name or host without category tabs", async () => {
  setup(); await screen.findByRole("article", { name: "北辰" });
  expect(screen.queryByRole("group", { name: "筛选订阅" })).not.toBeInTheDocument();
  expect(screen.queryByText(/需要关注/)).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("搜索名称或来源"), { target: { value: "北辰" } });
  expect(screen.getByRole("article", { name: "北辰" })).toBeInTheDocument();
  expect(screen.queryByRole("article", { name: "远山" })).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("搜索名称或来源"), { target: { value: "SECRET" } });
  expect(screen.queryByRole("article")).not.toBeInTheDocument();
});
it("requires reviewing an affected node before committing its replacement", async () => {
  const preview: SubscriptionPreview = { subscription: one, base: "base", added: [], removed: ["旧节点"], affected: ["ChatGPT"], skipped_tailscale: 0, via_proxy: false };
  vi.mocked(invoke).mockResolvedValue(preview);
  setup(); const card = await screen.findByRole("article", { name: "北辰" });
  fireEvent.click(within(card).getByRole("button", { name: "更新" }));
  expect(await screen.findByRole("dialog", { name: "预览并确认订阅" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "确认并应用" })).toBeDisabled();
  expect(vi.mocked(invoke).mock.calls.map(c => c[0])).toEqual(["preview_subscription"]);
});
it("keeps only editing and deletion in the normal subscription menu", async () => {
  setup(); const card = await screen.findByRole("article", { name: "北辰" });
  fireEvent.click(within(card).getByRole("button", { name: "操作 北辰" }));
  const menu = await screen.findByRole("menu");
  expect(within(menu).getAllByRole("menuitem").map(item => item.textContent)).toEqual(["编辑订阅", "删除订阅"]);
});
it("reports downloaded subscriptions as updated even while the core is stopped", async () => {
  const saved = { ...one, info: { usage: null, last_success: 100, history: [{ at: 100, result: one.last_result!, added: [], removed: [], via_proxy: false, apply: { NotRunning: { error: "stopped" } } }] } };
  vi.mocked(invoke).mockResolvedValueOnce({ subscription: saved, base: "base", added: [], removed: [], affected: [], skipped_tailscale: 0, via_proxy: false }).mockResolvedValueOnce({ NotRunning: { error: "stopped" } });
  setup([saved, two]); const card = await screen.findByRole("article", { name: "北辰" });
  expect(within(card).getByText("更新成功")).toBeInTheDocument();
  expect(within(card).queryByText(/待内核启动/)).not.toBeInTheDocument();
  fireEvent.click(within(card).getByRole("button", { name: "更新" }));
  const toast = await screen.findByRole("alert");
  expect(toast).toHaveTextContent("北辰更新成功 · 1 个节点");
  expect(toast).not.toHaveTextContent(/内核|更新记录/);
  expect(within(card).getByText("更新成功")).toBeInTheDocument();
});

const affectedPreview: SubscriptionPreview = { subscription: one, base: "base", added: [], removed: ["旧节点"], affected: ["工作代理"], skipped_tailscale: 0, via_proxy: false };
const reloaded = { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } };
async function chooseReplacement() {
  fireEvent.mouseDown(screen.getByLabelText("替代节点"));
  fireEvent.click(await screen.findByRole("option", { name: "日本" }));
}
it.each(["confirm", "skip"])("continues the batch after an affected subscription: %s", async choice => {
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "preview_subscription") {
      const sub = (args as { s: Subscription }).s;
      return { ...affectedPreview, subscription: sub, affected: sub.id === one.id ? ["工作代理"] : [] };
    }
    return reloaded;
  });
  setup(); await screen.findByRole("article", { name: "北辰" });
  fireEvent.click(screen.getByRole("button", { name: "全部更新" }));
  await screen.findByRole("dialog", { name: "预览并确认订阅" });
  expect(vi.mocked(invoke).mock.calls.filter(c => c[0] === "preview_subscription")).toHaveLength(1);
  if (choice === "confirm") {
    await chooseReplacement();
    fireEvent.click(screen.getByRole("button", { name: "确认并应用" }));
  } else fireEvent.click(screen.getByRole("button", { name: "保留原配置" }));
  await waitFor(() => expect(vi.mocked(invoke).mock.calls.filter(c => c[0] === "preview_subscription")).toHaveLength(2));
  await waitFor(() => expect(screen.getByRole("button", { name: "全部更新" })).toBeEnabled());
  const commits = vi.mocked(invoke).mock.calls.filter(c => c[0] === "commit_subscription_preview");
  expect(commits).toHaveLength(choice === "confirm" ? 2 : 1);
});
it("keeps the batch paused on a rejected confirmation and resumes after retry", async () => {
  let attempts = 0;
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "preview_subscription") {
      const sub = (args as { s: Subscription }).s;
      return { ...affectedPreview, subscription: sub, affected: sub.id === one.id ? ["工作代理"] : [] };
    }
    if (++attempts === 1) throw "配置检查失败";
    return reloaded;
  });
  setup(); await screen.findByRole("article", { name: "北辰" });
  fireEvent.click(screen.getByRole("button", { name: "全部更新" }));
  await screen.findByRole("dialog"); await chooseReplacement();
  fireEvent.click(screen.getByRole("button", { name: "确认并应用" }));
  await screen.findByText("配置检查失败");
  expect(vi.mocked(invoke).mock.calls.filter(c => c[0] === "preview_subscription")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: "确认并应用" }));
  await waitFor(() => expect(vi.mocked(invoke).mock.calls.filter(c => c[0] === "preview_subscription")).toHaveLength(2));
});
it("clears an old apply warning on runtime recovery while retaining history and download errors", async () => {
  const failed = { ...one, info: { usage: null, last_success: null, history: [{ at: 100, result: one.last_result!, added: [], removed: [], via_proxy: false, apply: { FailedClosed: { error: "reload failed", uncut: 0 } } }] } };
  const { qc } = setup([failed, two]);
  const card = await screen.findByRole("article", { name: "北辰" });
  expect(within(card).getByText("配置未生效")).toBeInTheDocument();
  act(() => { qc.setQueryData(["status"], { core: "Running", last_apply: reloaded }); });
  await waitFor(() => expect(within(card).getByText("更新成功")).toBeInTheDocument());
  expect(within(screen.getByRole("article", { name: "远山" })).getByText("更新失败")).toBeInTheDocument();
  expect(qc.getQueryData<Model>(["model"])?.subscriptions[0].info?.history[0].apply).toEqual(failed.info.history[0].apply);
});

it("queues concurrent affected previews without losing either confirmation", async () => {
  vi.mocked(invoke).mockImplementation(async (command, args) => command === "preview_subscription"
    ? { ...affectedPreview, subscription: (args as { s: Subscription }).s } : reloaded);
  setup(); await screen.findByRole("article", { name: "北辰" });
  fireEvent.click(within(screen.getByRole("article", { name: "北辰" })).getByRole("button", { name: "更新" }));
  fireEvent.click(within(screen.getByRole("article", { name: "远山" })).getByRole("button", { name: "重试更新" }));
  await screen.findByRole("dialog");
  await waitFor(() => expect(vi.mocked(invoke).mock.calls.filter(c => c[0] === "preview_subscription")).toHaveLength(2));
  await chooseReplacement();
  fireEvent.click(screen.getByRole("button", { name: "确认并应用" }));
  await waitFor(() => expect(within(screen.getByRole("dialog")).getByText("远山")).toBeInTheDocument());
  expect(screen.getByRole("button", { name: "确认并应用" })).toBeDisabled();
  await chooseReplacement();
  fireEvent.click(screen.getByRole("button", { name: "确认并应用" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  expect(vi.mocked(invoke).mock.calls.filter(c => c[0] === "commit_subscription_preview")).toHaveLength(2);
});
it("does not resume a paused batch after leaving the page", async () => {
  vi.mocked(invoke).mockResolvedValue(affectedPreview);
  const { unmount } = setup(); await screen.findByRole("article", { name: "北辰" });
  fireEvent.click(screen.getByRole("button", { name: "全部更新" }));
  await screen.findByRole("dialog");
  await act(async () => unmount());
  expect(vi.mocked(invoke).mock.calls.map(c => c[0])).toEqual(["preview_subscription"]);
});
