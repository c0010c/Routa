import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Listener, Model } from "../bindings";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(),
    getStatus: vi.fn(),
    saveListener: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ListenerDialog, newListener } from "./ListenerDialog";

const model: Model = {
  subscriptions: [],
  exits: [],
  listeners: [],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [],
  node_groups: [],
};

const listener: Listener = {
  id: "l1",
  name: "浏览器",
  listener_type: "Mixed",
  listen: "0.0.0.0",
  port: 7891,
  auth: null,
  exit: null,
  user_enabled: true,
};

function renderDialog(l: Listener, onClose = vi.fn()) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <ListenerDialog listener={l} onClose={onClose} />
    </QueryClientProvider>,
  );
  return onClose;
}

describe("ListenerDialog", () => {
  beforeEach(() => {
    vi.mocked(commands.getModel).mockResolvedValue(model);
  });

  it("shows a port conflict under the port field", async () => {
    vi.mocked(commands.saveListener).mockResolvedValue({
      status: "error",
      error: [{ PortConflict: { port: 7891, with: "浏览器" } }],
    });
    const onClose = renderDialog({ ...listener, name: "下载" });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    const msg = await screen.findByText("端口 7891 已被「浏览器」占用");
    expect(screen.getByLabelText("端口")).toHaveAttribute("aria-invalid", "true");
    expect(msg).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
    await waitFor(() => expect(commands.getModel).toHaveBeenCalled());
  });

  it("shows invalid names under the name field", async () => {
    vi.mocked(commands.saveListener).mockResolvedValue({
      status: "error",
      error: [{ InvalidName: { name: "", reason: "名称不能为空" } }],
    });
    renderDialog({ ...listener, name: "" });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("名称不能为空")).toBeInTheDocument();
    await waitFor(() => expect(commands.getModel).toHaveBeenCalled());
  });

  it("masks the password and clears a field error on edit", async () => {
    vi.mocked(commands.saveListener).mockResolvedValue({
      status: "error",
      error: [{ PortConflict: { port: 7891, with: "浏览器" } }],
    });
    renderDialog({ ...listener, auth: { username: "u", password: "p" } });
    expect(screen.getByLabelText("密码")).toHaveAttribute("type", "password");
    fireEvent.click(screen.getByRole("button", { name: "显示" }));
    expect(screen.getByLabelText("密码")).toHaveAttribute("type", "text");
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await screen.findByText("端口 7891 已被「浏览器」占用");
    fireEvent.change(screen.getByLabelText("端口"), { target: { value: "7892" } });
    expect(screen.queryByText("端口 7891 已被「浏览器」占用")).not.toBeInTheDocument();
  });

  it("rejects out-of-range ports without calling the backend", async () => {
    vi.mocked(commands.saveListener).mockClear();
    renderDialog(listener);
    fireEvent.change(screen.getByLabelText("端口"), { target: { value: "70000" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("端口需在 1–65535 之间")).toBeInTheDocument();
    expect(commands.saveListener).not.toHaveBeenCalled();
  });
});

describe("newListener", () => {
  it("defaults to 7891 with no listeners and max+1 otherwise", () => {
    expect(newListener([]).port).toBe(7891);
    expect(newListener([{ ...listener, port: 7000 }, { ...listener, port: 7005 }]).port).toBe(7006);
    const l = newListener([]);
    expect(l.listener_type).toBe("Mixed");
    expect(l.listen).toBe("0.0.0.0");
    expect(l.user_enabled).toBe(true);
  });

  it("never proposes a port above 65535", () => {
    const p = newListener([{ ...listener, port: 65535 }, { ...listener, port: 1024 }]).port;
    expect(p).toBe(1025);
  });
});
