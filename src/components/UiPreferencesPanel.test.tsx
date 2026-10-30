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
};

function ModePanel() {
  return <UiPreferencesPanel key={useInterface()?.state?.preferences.mode} />;
}
function setup() {
  let current: DesktopState = {
    preferences: structuredClone(preferences), connection: null, requested: null,
    error: null, is_admin: false, system_listener_id: null,
  };
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "save_ui_preferences") {
      current = { ...current, preferences: (args as { input: UiPreferences }).input };
      return current.preferences;
    }
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
  await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2)); // initial query plus exactly one save
  await act(async () => { reject(new Error("internal: write failed")); });
  expect(await screen.findByRole("alert")).toHaveTextContent("未能保存，请重试");
  expect(control).not.toBeChecked();
  expect(control).toBeEnabled();
  expect(screen.queryByText("已保存")).not.toBeInTheDocument();
  expect(screen.queryByText(/internal:/)).not.toBeInTheDocument();
});
