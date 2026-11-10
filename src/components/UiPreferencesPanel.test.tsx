import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import type { DesktopState, UiPreferences } from "../bindings";
import { InterfaceProvider, useInterface } from "./InterfaceProvider";
import { UiPreferencesPanel } from "./UiPreferencesPanel";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const preferences: UiPreferences = {
  mode: "Full", listener_id: null, exit_id: null, system_listener_id: null,
  capture_mode: "System", auto_connect: false, close_to_tray: true,
  managed_rules: [], selected_apps: [], skip_ai_reset_prompt: false,
  taskbar_status: { enabled: true, mode: "Rotate", traffic: "Values" },
};
const model = { listeners: [
  { id: "l1", name: "监听一", port: 7890, user_enabled: true },
  { id: "l2", name: "监听二", port: 7891, user_enabled: false },
  { id: "l3", name: "监听三", port: 7892, user_enabled: true },
] };

function ModePanel() {
  return <UiPreferencesPanel key={useInterface()?.state?.preferences.mode} />;
}
function setup(overrides: Partial<DesktopState> = {}) {
  let current: DesktopState = {
    preferences: structuredClone(preferences), connection: null, requested: null,
    error: null, is_admin: false, system_listener_id: null,
    taskbar_status_supported: true, ...overrides,
  };
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "save_ui_preferences") {
      current = { ...current, preferences: (args as { input: UiPreferences }).input };
      return current.preferences;
    }
    if (command === "get_model") return model;
    return current;
  });
  return render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
    <InterfaceProvider><ModePanel /></InterfaceProvider>
  </QueryClientProvider>);
}

beforeEach(() => vi.clearAllMocks());

it("saves auto-connect immediately with local feedback and keeps unrelated preferences", async () => {
  setup();
  const control = await screen.findByRole("switch", { name: "启动后自动连接" });
  fireEvent.click(control);
  expect(await screen.findByText("已保存")).toBeInTheDocument();
  expect(control).toBeChecked();
  expect(invoke).toHaveBeenCalledWith("save_ui_preferences", { input: { ...preferences, auto_connect: true } });
  fireEvent.mouseDown(screen.getByRole("combobox", { name: "关闭窗口时" }));
  fireEvent.click(screen.getByRole("option", { name: "退出程序" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("save_ui_preferences", { input: { ...preferences, auto_connect: true, close_to_tray: false } }));
});

it("turns the taskbar status bar off", async () => {
  setup();
  fireEvent.click(await screen.findByRole("switch", { name: "任务栏状态栏" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("save_ui_preferences", {
    input: { ...preferences, taskbar_status: { enabled: false, mode: "Rotate", traffic: "Values" } },
  }));
});

async function pick(label: string, option: string) {
  fireEvent.mouseDown(await screen.findByRole("combobox", { name: label }));
  fireEvent.click(await screen.findByRole("option", { name: option }));
}

it("offers only rotation and pinning as display modes", async () => {
  setup();
  fireEvent.mouseDown(await screen.findByRole("combobox", { name: "显示内容" }));
  const options = await screen.findAllByRole("option");
  expect(options.map((o) => o.textContent)).toEqual(["轮播", "固定"]);
  expect(screen.queryByRole("combobox", { name: "固定的代理" })).not.toBeInTheDocument();
});

it("pins the first enabled proxy when switching to fixed", async () => {
  setup();
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("get_model"));
  await pick("显示内容", "固定");
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("save_ui_preferences", {
    input: { ...preferences, taskbar_status: { enabled: true, mode: { Fixed: "l1" }, traffic: "Values" } },
  }));
});

it("picks the pinned proxy from enabled proxies labelled with their port", async () => {
  setup({ preferences: { ...preferences, taskbar_status: { enabled: true, mode: { Fixed: "l1" }, traffic: "Values" } } });
  expect(await screen.findByRole("combobox", { name: "显示内容" })).toHaveTextContent("固定");
  expect(await screen.findByRole("combobox", { name: "固定的代理" })).toHaveTextContent("监听一（7890）");
  fireEvent.mouseDown(screen.getByRole("combobox", { name: "固定的代理" }));
  const options = await screen.findAllByRole("option");
  expect(options.map((o) => o.textContent)).toEqual(["监听一（7890）", "监听三（7892）"]);
  fireEvent.click(options[1]);
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("save_ui_preferences", {
    input: { ...preferences, taskbar_status: { enabled: true, mode: { Fixed: "l3" }, traffic: "Values" } },
  }));
});

it("returns to rotation from fixed", async () => {
  setup({ preferences: { ...preferences, taskbar_status: { enabled: true, mode: { Fixed: "l1" }, traffic: "Values" } } });
  await screen.findByRole("combobox", { name: "固定的代理" });
  await pick("显示内容", "轮播");
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("save_ui_preferences", {
    input: { ...preferences, taskbar_status: { enabled: true, mode: "Rotate", traffic: "Values" } },
  }));
});

it("offers numbers or chart for traffic, numbers by default", async () => {
  setup();
  const select = await screen.findByRole("combobox", { name: "流量显示" });
  expect(select).toHaveTextContent("数值");
  fireEvent.mouseDown(select);
  const options = await screen.findAllByRole("option");
  expect(options.map((o) => o.textContent)).toEqual(["数值", "折线图"]);
  fireEvent.click(options[1]);
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("save_ui_preferences", {
    input: { ...preferences, taskbar_status: { enabled: true, mode: "Rotate", traffic: "Chart" } },
  }));
});

it("hides the traffic choice while the bar is off", async () => {
  setup({ preferences: { ...preferences, taskbar_status: { enabled: false, mode: "Rotate", traffic: "Values" } } });
  await screen.findByRole("switch", { name: "任务栏状态栏" });
  expect(screen.queryByRole("combobox", { name: "流量显示" })).not.toBeInTheDocument();
});

it("disables the taskbar status bar below Windows 11", async () => {
  setup({ taskbar_status_supported: false });
  expect(await screen.findByRole("switch", { name: "任务栏状态栏" })).toBeDisabled();
  expect(screen.getByText("仅支持 Windows 11")).toBeInTheDocument();
});

it("shows rotation when the pinned proxy is disabled or gone", async () => {
  for (const id of ["l2", "gone"]) {
    const { unmount } = setup({ preferences: { ...preferences, taskbar_status: { enabled: true, mode: { Fixed: id }, traffic: "Values" } } });
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("get_model"));
    expect(await screen.findByRole("combobox", { name: "显示内容" })).toHaveTextContent("轮播");
    expect(screen.queryByRole("combobox", { name: "固定的代理" })).not.toBeInTheDocument();
    unmount();
  }
});

it("keeps feedback across the panel remount caused by a mode change", async () => {
  setup();
  fireEvent.mouseDown(await screen.findByRole("combobox", { name: "界面模式" }));
  fireEvent.click(screen.getByRole("option", { name: "简洁版" }));
  expect(await screen.findByText("已保存")).toBeInTheDocument();
  expect(screen.getByRole("combobox", { name: "界面模式" })).toHaveTextContent("简洁版");
  expect(invoke).toHaveBeenCalledWith("save_ui_preferences", { input: { ...preferences, mode: "Simple" } });
});

it("blocks repeated submissions and shows an error without a false success or changed value", async () => {
  setup();
  const control = await screen.findByRole("switch", { name: "启动后自动连接" });
  let reject!: (error: Error) => void;
  vi.mocked(invoke).mockImplementationOnce(() => new Promise((_, fail) => { reject = fail; }));
  fireEvent.click(control);
  fireEvent.click(control);
  expect(control).toBeDisabled();
  expect(screen.getByRole("combobox", { name: "关闭窗口时" })).toHaveAttribute("aria-disabled", "true");
  expect(screen.getByText("保存中…")).toBeInTheDocument();
  await waitFor(() => expect(vi.mocked(invoke).mock.calls.filter(([c]) => c === "save_ui_preferences")).toHaveLength(1));
  await act(async () => { reject(new Error("internal: write failed")); });
  expect(await screen.findByRole("alert")).toHaveTextContent("未能保存，请重试");
  expect(control).not.toBeChecked();
  expect(control).toBeEnabled();
  expect(screen.queryByText("已保存")).not.toBeInTheDocument();
  expect(screen.queryByText(/internal:/)).not.toBeInTheDocument();
});

it("says the bar shows the system proxy or TUN traffic while one is on", async () => {
  const { unmount } = setup({ connection: "Tun", requested: "Tun" });
  expect(await screen.findByText("已开启 TUN，状态栏显示 TUN 的流量")).toBeInTheDocument();
  unmount();
  const second = setup({ connection: "System", requested: "System" });
  expect(await screen.findByText("已开启系统代理，状态栏显示系统代理的流量")).toBeInTheDocument();
  second.unmount();
  setup({ connection: "Apps", requested: "Apps" });
  await screen.findByRole("combobox", { name: "显示内容" });
  expect(screen.queryByText(/状态栏显示/)).not.toBeInTheDocument();
});
