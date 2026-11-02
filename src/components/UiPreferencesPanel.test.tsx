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
  managed_rules: [], selected_apps: [],
  taskbar_status: { enabled: true, mode: "Rotate" },
};
const model = { listeners: [{ id: "l1", name: "监听一", user_enabled: true }, { id: "l2", name: "监听二", user_enabled: false }] };

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
  render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
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
    input: { ...preferences, taskbar_status: { enabled: false, mode: "Rotate" } },
  }));
});

it("pins the taskbar status bar to one listener", async () => {
  setup();
  const select = await screen.findByRole("combobox", { name: "显示内容" });
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("get_model"));
  fireEvent.mouseDown(select);
  fireEvent.click(await screen.findByRole("option", { name: "固定：监听一" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("save_ui_preferences", {
    input: { ...preferences, taskbar_status: { enabled: true, mode: { Fixed: "l1" } } },
  }));
});

it("disables the taskbar status bar below Windows 11", async () => {
  setup({ taskbar_status_supported: false });
  expect(await screen.findByRole("switch", { name: "任务栏状态栏" })).toBeDisabled();
  expect(screen.getByText("仅支持 Windows 11")).toBeInTheDocument();
});

it("offers only enabled listeners for pinning", async () => {
  setup();
  const select = await screen.findByRole("combobox", { name: "显示内容" });
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("get_model"));
  fireEvent.mouseDown(select);
  expect(await screen.findByRole("option", { name: "固定：监听一" })).toBeInTheDocument();
  expect(screen.queryByRole("option", { name: "固定：监听二" })).not.toBeInTheDocument();
});

it("shows rotation when the pinned listener is disabled", async () => {
  setup({ preferences: { ...preferences, taskbar_status: { enabled: true, mode: { Fixed: "l2" } } } });
  expect(await screen.findByRole("combobox", { name: "显示内容" })).toHaveTextContent("轮播");
});

it("shows rotation when the pinned listener no longer exists", async () => {
  setup({ preferences: { ...preferences, taskbar_status: { enabled: true, mode: { Fixed: "gone" } } } });
  expect(await screen.findByRole("combobox", { name: "显示内容" })).toHaveTextContent("轮播");
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
