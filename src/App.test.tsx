import { fireEvent, render, screen, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import type { DesktopState } from "./api/desktop";
import pkg from "../package.json";
import App from "./App";

vi.mock("./api/events", () => ({ useRuntimeEvents: () => {} }));
vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => false,
  invoke: vi.fn(),
}));
vi.mock("./bindings", () => ({
  commands: {
    getModel: () => new Promise(() => {}),
    getStatus: () => new Promise(() => {}),
  },
}));

const desktop: DesktopState = {
  preferences: {
    mode: "Full",
    listener_id: null,
    exit_id: null,
    system_listener_id: null,
    capture_mode: "System",
    auto_connect: false,
    close_to_tray: true,
    managed_rules: [],
    selected_apps: [],
  },
  connection: null,
  requested: null,
  error: null,
  is_admin: false,
  system_listener_id: null,
};
beforeEach(() => {
  vi.mocked(invoke).mockResolvedValue(structuredClone(desktop));
  window.location.hash = "#/";
});
describe("App", () => {
  it("renders the navigation items", async () => {
    render(<App />);
    const nav = await screen.findByRole("navigation", { name: "主导航" });
    const labels = within(nav)
      .getAllByRole("link")
      .map((l) => l.textContent);
    expect(labels).toEqual([
      "概览",
      "代理",
      "进程分流",
      "订阅",
      "节点",
      "设置",
    ]);
  });
});

describe("Layout sidebar", () => {
  it("shows the brand and version", async () => {
    render(<App />);
    const nav = await screen.findByRole("navigation", { name: "主导航" });
    expect(screen.getByText("Routa")).toBeInTheDocument();
    expect(screen.getByText(`v${pkg.version}`)).toBeInTheDocument();
    expect(nav).toBeInTheDocument();
  });

  it("marks only the current page as selected", async () => {
    window.location.hash = "#/";
    render(<App />);
    const links = within(
      await screen.findByRole("navigation", { name: "主导航" }),
    ).getAllByRole("link");
    const selected = links
      .filter((l) => l.classList.contains("Mui-selected"))
      .map((l) => l.textContent);
    expect(selected).toEqual(["概览"]);
  });
});

it("asks a new user to choose an interface and persists their choice", async () => {
  let current = structuredClone(desktop);
  current.preferences.mode = null;
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "save_ui_preferences") {
      current.preferences = (
        args as { input: DesktopState["preferences"] }
      ).input;
      return current.preferences;
    }
    return current;
  });
  render(<App />);
  expect(await screen.findByText("先选一种使用方式")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "开始使用" }));
  expect(
    await screen.findByRole("navigation", { name: "简洁导航" }),
  ).toBeInTheDocument();
  expect(invoke).toHaveBeenCalledWith(
    "save_ui_preferences",
    expect.objectContaining({
      input: expect.objectContaining({ mode: "Simple" }),
    }),
  );
  expect(
    screen.queryByRole("navigation", { name: "主导航" }),
  ).not.toBeInTheDocument();
});
