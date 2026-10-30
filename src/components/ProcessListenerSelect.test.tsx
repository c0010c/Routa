import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import type { Listener, ProcessRule } from "../bindings";
import { ProcessListenerSelect } from "./ProcessListenerSelect";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const rule: ProcessRule = {
  id: "r1",
  exe_path: "C:\\Apps\\chrome.exe",
  listener: "l1",
  user_enabled: false,
  on_fault: "Block",
  reset_on_change: false,
};
const listeners: Listener[] = [
  {
    id: "l1",
    name: "默认",
    listener_type: "Mixed",
    listen: "127.0.0.1",
    port: 7881,
    exit: null,
    auth: null,
    user_enabled: true,
  },
  {
    id: "l2",
    name: "下载",
    listener_type: "Socks",
    listen: "127.0.0.1",
    port: 7882,
    exit: null,
    auth: null,
    user_enabled: true,
  },
  {
    id: "l3",
    name: "HTTP",
    listener_type: "Http",
    listen: "127.0.0.1",
    port: 7883,
    exit: null,
    auth: null,
    user_enabled: true,
  },
  {
    id: "l4",
    name: "备用",
    listener_type: "Mixed",
    listen: "127.0.0.1",
    port: 7884,
    exit: null,
    auth: null,
    user_enabled: false,
  },
];
function mount() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <ProcessListenerSelect rule={rule} listeners={listeners} />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}
beforeEach(() => vi.mocked(invoke).mockReset());
it("only offers SOCKS-capable listeners and keeps disabled listeners unavailable", async () => {
  mount();
  fireEvent.mouseDown(screen.getByRole("combobox"));
  expect(
    await screen.findByRole("option", { name: "7882 · 下载" }),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("option", { name: /HTTP/ }),
  ).not.toBeInTheDocument();
  expect(screen.getByRole("option", { name: /备用/ })).toHaveAttribute(
    "aria-disabled",
    "true",
  );
});
it("disables the row while applying without enabling a disabled rule", async () => {
  let resolve: () => void = () => {};
  vi.mocked(invoke).mockReturnValue(
    new Promise<void>((done) => {
      resolve = done;
    }),
  );
  mount();
  fireEvent.mouseDown(screen.getByRole("combobox"));
  fireEvent.click(await screen.findByRole("option", { name: "7882 · 下载" }));
  expect(await screen.findByRole("status")).toHaveTextContent("正在应用");
  expect(screen.getByRole("combobox")).toHaveAttribute("aria-disabled", "true");
  expect(invoke).toHaveBeenCalledWith("change_process_listener", {
    id: "r1",
    listener: "l2",
  });
  await act(async () => resolve());
  await waitFor(() =>
    expect(screen.queryByRole("status")).not.toBeInTheDocument(),
  );
  expect(rule.user_enabled).toBe(false);
});
it("restores the original selection on failure and retries the failed target", async () => {
  vi.mocked(invoke)
    .mockRejectedValueOnce("服务未确认，已恢复原监听器")
    .mockResolvedValue(undefined);
  mount();
  fireEvent.mouseDown(screen.getByRole("combobox"));
  fireEvent.click(await screen.findByRole("option", { name: "7882 · 下载" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("已恢复原监听器");
  expect(screen.getByRole("combobox")).toHaveTextContent("7881 · 默认");
  fireEvent.click(screen.getByRole("button", { name: "重试" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));
  expect(vi.mocked(invoke).mock.calls[1]).toEqual([
    "change_process_listener",
    { id: "r1", listener: "l2" },
  ]);
  await waitFor(() =>
    expect(screen.queryByRole("alert")).not.toBeInTheDocument(),
  );
});
