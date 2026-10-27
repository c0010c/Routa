import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
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
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [],
  node_groups: [],
};

describe("SubscriptionDialog", () => {
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
    fireEvent.change(screen.getByLabelText("单位"), { target: { value: "天" } });
    fireEvent.change(screen.getByLabelText("间隔"), { target: { value: "400" } });
    const save = screen.getByRole("button", { name: "保存" });
    expect(save).toBeDisabled();
    fireEvent.click(save);
    expect(commands.saveSubscription).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("间隔"), { target: { value: "2" } });
    expect(save).toBeEnabled();
  });
});
