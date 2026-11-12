import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
import type { Model } from "../bindings";

vi.mock("../bindings", () => ({
  commands: { getModel: vi.fn(), saveSubscription: vi.fn() },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { SubscriptionDialog, newSubscription } from "./SubscriptionDialog";

const model: Model = {
  subscriptions: [],
  exits: [],
  listeners: [],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u", node_monitor: { enabled: false, members: [], interval_secs: 300, fail_threshold: 2, latency_limit_ms: 1000, notify_recovery: true } },
  manual_nodes: [],
  node_groups: [],
  process_routing: { enabled: false, rules: [], exit_prompt: true },
};

describe("SubscriptionDialog", () => {
  it("downloads a preview first and commits only after confirmation", async () => {
    const sub = { ...newSubscription(), name: "示例", url: "https://example.test/sub" };
    vi.mocked(commands.getModel).mockResolvedValue(model);
    vi.mocked(invoke).mockImplementation(async (command) => command === "preview_subscription" ? { subscription: { ...sub, nodes: [{ name: "日本", proxy_type: "ss", server: "s", raw: {} }] }, base: "base", added: ["日本"], removed: [], affected: [], skipped_tailscale: 0, via_proxy: false } : { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } });
    const close = vi.fn();
    render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}><ApplyResultProvider><SubscriptionDialog subscription={sub} onClose={close} /></ApplyResultProvider></QueryClientProvider>);
    fireEvent.click(await screen.findByRole("button", { name: "预览节点" }));
    await screen.findByRole("dialog", { name: "预览并确认订阅" });
    expect(invoke).toHaveBeenCalledWith("preview_subscription", expect.anything());
    expect(close).not.toHaveBeenCalled();
    expect(vi.mocked(invoke).mock.calls.some(c => c[0] === "commit_subscription_preview")).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "确认并应用" }));
    await waitFor(() => expect(close).toHaveBeenCalled());
    expect(invoke).toHaveBeenCalledWith("commit_subscription_preview", expect.objectContaining({ replacement: null }));
  });
  it("blocks saving while the interval is out of range", async () => {
    vi.mocked(commands.getModel).mockResolvedValue(model);
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <ApplyResultProvider>
          <SubscriptionDialog subscription={newSubscription()} onClose={() => {}} />
        </ApplyResultProvider>
      </QueryClientProvider>,
    );
    await screen.findByText("新建订阅");
    fireEvent.change(screen.getByLabelText("URL"),{target:{value:"https://example.invalid/sub"}});
    fireEvent.click(screen.getByText("高级选项"));
    fireEvent.change(screen.getByLabelText("单位"), { target: { value: "天" } });
    fireEvent.change(screen.getByLabelText("间隔"), { target: { value: "400" } });
    const save = screen.getByRole("button", { name: "预览节点" });
    expect(save).toBeDisabled();
    fireEvent.click(save);
    expect(commands.saveSubscription).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("间隔"), { target: { value: "2" } });
    expect(save).toBeEnabled();
  });
});
