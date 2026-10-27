import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import type { Model, RuntimeStatus } from "../bindings";

vi.mock("../bindings", () => ({
  commands: { getModel: vi.fn(), getStatus: vi.fn() },
}));

import { commands } from "../bindings";
import { MemberPicker } from "./MemberPicker";

const model: Model = {
  subscriptions: [
    {
      id: "s1",
      name: "机场A",
      url: "u",
      update_interval_mins: null,
      fallback_exit: null,
      last_updated: null,
      last_result: null,
      nodes: [{ name: "香港 01", proxy_type: "ss", server: "h", raw: {} }],
    },
  ],
  exits: [],
  listeners: [],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [{ id: "m1", name: "家里", groups: ["g1"], kind: { Proxy: { raw: {} } } }],
  node_groups: [{ id: "g1", name: "自用" }],
};
const status = {
  core: "Running",
  health: { exits: {}, listeners: {} },
  delays: {},
} as unknown as RuntimeStatus;

describe("MemberPicker", () => {
  it("offers whole-group options but never 未分组, and emits a group member", async () => {
    vi.mocked(commands.getModel).mockResolvedValue(model);
    vi.mocked(commands.getStatus).mockResolvedValue(status);
    const onChange = vi.fn();
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <MemberPicker value={[]} onChange={onChange} />
      </QueryClientProvider>,
    );
    await screen.findByRole("combobox");
    fireEvent.mouseDown(screen.getByRole("combobox"));
    expect(await screen.findByText("机场A（整组 · 1）")).toBeInTheDocument();
    expect(screen.getByText("自用（整组 · 1）")).toBeInTheDocument();
    expect(screen.queryByText("未分组")).not.toBeInTheDocument();
    fireEvent.click(screen.getByText("机场A（整组 · 1）"));
    expect(onChange).toHaveBeenCalledWith([{ group: { Subscription: "s1" } }]);
  });
});
