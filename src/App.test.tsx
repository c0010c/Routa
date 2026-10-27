import { render, screen, within } from "@testing-library/react";
import App from "./App";

vi.mock("./api/events", () => ({ useRuntimeEvents: () => {} }));
vi.mock("./bindings", () => ({
  commands: {
    getModel: () => new Promise(() => {}),
    getStatus: () => new Promise(() => {}),
  },
}));

describe("App", () => {
  it("renders the five navigation items", () => {
    render(<App />);
    const nav = screen.getByRole("navigation", { name: "主导航" });
    const labels = within(nav)
      .getAllByRole("link")
      .map((l) => l.textContent);
    expect(labels).toEqual(["概览", "Listener", "出口", "节点", "Tailscale", "设置"]);
  });
});
