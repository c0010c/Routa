import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";

const handlers = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
    handlers.set(name, handler);
    return () => handlers.delete(name);
  }),
}));

vi.mock("../bindings", () => ({
  commands: {
    setExitPrompt: vi.fn(),
    quitApp: vi.fn(),
    disableRoutingAndQuit: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "./ApplyResultSnackbar";
import { ExitConfirmDialog } from "./ExitConfirmDialog";

function renderDialog() {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ApplyResultProvider>
        <ExitConfirmDialog />
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
}

async function ask(names: string[]) {
  await waitFor(() => expect(handlers.has("confirm-exit")).toBe(true));
  act(() => handlers.get("confirm-exit")!({ payload: names }));
  return screen.findByRole("dialog");
}

describe("ExitConfirmDialog", () => {
  beforeEach(() => {
    handlers.clear();
    vi.mocked(commands.setExitPrompt).mockReset().mockResolvedValue({ status: "ok", data: null });
    vi.mocked(commands.quitApp).mockReset().mockResolvedValue(undefined);
    vi.mocked(commands.disableRoutingAndQuit).mockReset().mockResolvedValue({ status: "ok", data: null });
  });

  it("lists the blocking rules after the event", async () => {
    renderDialog();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await ask(["游戏", "AI 工具"]);
    expect(screen.getByText("退出 Routa？")).toBeInTheDocument();
    expect(
      screen.getByText("下面这些规则选了阻断，退出后对应程序会断网，直到 Routa 再次运行：游戏、AI 工具"),
    ).toBeInTheDocument();
  });

  it("不再提示 + 仍然退出 turns the prompt off, then quits", async () => {
    renderDialog();
    await ask(["游戏"]);
    fireEvent.click(screen.getByRole("checkbox", { name: "不再提示" }));
    fireEvent.click(screen.getByRole("button", { name: "仍然退出" }));
    await waitFor(() => expect(commands.quitApp).toHaveBeenCalled());
    expect(commands.setExitPrompt).toHaveBeenCalledWith(false);
    expect(vi.mocked(commands.setExitPrompt).mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(commands.quitApp).mock.invocationCallOrder[0],
    );
    expect(commands.disableRoutingAndQuit).not.toHaveBeenCalled();
  });

  it("停用进程分流并退出 disables routing and quits", async () => {
    renderDialog();
    await ask(["游戏"]);
    fireEvent.click(screen.getByRole("button", { name: "停用进程分流并退出" }));
    await waitFor(() => expect(commands.disableRoutingAndQuit).toHaveBeenCalled());
    expect(commands.setExitPrompt).not.toHaveBeenCalled();
    expect(commands.quitApp).not.toHaveBeenCalled();
  });

  it("取消 only closes", async () => {
    renderDialog();
    await ask(["游戏"]);
    fireEvent.click(screen.getByRole("checkbox", { name: "不再提示" }));
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(commands.setExitPrompt).not.toHaveBeenCalled();
    expect(commands.quitApp).not.toHaveBeenCalled();
    expect(commands.disableRoutingAndQuit).not.toHaveBeenCalled();
  });
});
