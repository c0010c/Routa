import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
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
  process_routing: { enabled: false, on_routa_exit: "Block", rules: [] },
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

function Loc() {
  const loc = useLocation();
  return <div data-testid="loc">{loc.pathname + loc.search}</div>;
}

async function modelLoaded() {
  await waitFor(() => expect(commands.getModel).toHaveBeenCalled());
  await new Promise((r) => setTimeout(r, 0));
}

function renderDialog(l: Listener, onClose = vi.fn()) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <ListenerDialog listener={l} onClose={onClose} />
        <Loc />
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return onClose;
}

describe("ListenerDialog", () => {
  beforeEach(() => {
    vi.mocked(commands.getModel).mockResolvedValue(model);
  });

  it("has no exit field", async () => {
    renderDialog(listener);
    expect(screen.queryByLabelText("出口")).not.toBeInTheDocument();
    await waitFor(() => expect(commands.getModel).toHaveBeenCalled());
  });

  it("navigates to the overview highlight after creating", async () => {
    vi.mocked(commands.saveListener).mockResolvedValue({ status: "ok", data: { NotRunning: { error: "" } } });
    renderDialog({ ...listener, id: "new1", name: "新" });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(screen.getByTestId("loc")).toHaveTextContent("/?highlight=new1"));
  });

  it("wires a new listener to the only exit", async () => {
    const exit = { id: "e1", name: "直连", kind: "Direct" as const, interface: null };
    vi.mocked(commands.getModel).mockResolvedValue({ ...model, exits: [exit] });
    vi.mocked(commands.saveListener).mockResolvedValue({ status: "ok", data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } });
    vi.mocked(commands.saveListener).mockClear();
    renderDialog({ ...listener, id: "new2", name: "新" });
    await modelLoaded();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(commands.saveListener).toHaveBeenCalledTimes(1));
    expect(vi.mocked(commands.saveListener).mock.calls[0][0].exit).toBe("e1");
  });

  it("leaves the exit empty when there are several", async () => {
    const e = (id: string) => ({ id, name: id, kind: "Direct" as const, interface: null });
    vi.mocked(commands.getModel).mockResolvedValue({ ...model, exits: [e("a"), e("b")] });
    vi.mocked(commands.saveListener).mockResolvedValue({ status: "ok", data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } });
    vi.mocked(commands.saveListener).mockClear();
    renderDialog({ ...listener, id: "new3", name: "新" });
    await modelLoaded();
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(commands.saveListener).toHaveBeenCalledTimes(1));
    expect(vi.mocked(commands.saveListener).mock.calls[0][0].exit).toBeNull();
  });

  it("does not navigate when editing", async () => {
    vi.mocked(commands.getModel).mockResolvedValue({ ...model, listeners: [listener] });
    vi.mocked(commands.saveListener).mockResolvedValue({ status: "ok", data: { NotRunning: { error: "" } } });
    const onClose = renderDialog(listener);
    await waitFor(() => expect(commands.getModel).toHaveBeenCalled());
    await new Promise((r) => setTimeout(r, 20));
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(screen.getByTestId("loc")).toHaveTextContent(/^\/$/);
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

  it("uses a switch for auth and explains the listen address", async () => {
    renderDialog(listener);
    const auth = screen.getByRole("switch", { name: "启用认证" });
    expect(screen.queryByRole("checkbox", { name: "启用认证" })).toBeNull();
    expect(screen.getByText("0.0.0.0 表示局域网内其他设备也能连接；只本机使用请填 127.0.0.1")).toBeInTheDocument();
    fireEvent.click(auth);
    expect(screen.getByLabelText("用户名")).toBeInTheDocument();
    await waitFor(() => expect(commands.getModel).toHaveBeenCalled());
  });
});
