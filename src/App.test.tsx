import { render, screen, within } from "@testing-library/react";
import pkg from "../package.json";
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
    expect(labels).toEqual(["概览", "监听器", "出口", "节点", "Tailscale", "设置"]);
  });
});

describe("Layout sidebar", () => {
  it("shows the brand and version", () => {
    render(<App />);
    const nav = screen.getByRole("navigation", { name: "主导航" });
    expect(screen.getByText("Routa")).toBeInTheDocument();
    expect(screen.getByText(`v${pkg.version}`)).toBeInTheDocument();
    expect(nav).toBeInTheDocument();
  });

  it("marks only the current page as selected", () => {
    window.location.hash = "#/";
    render(<App />);
    const links = within(screen.getByRole("navigation", { name: "主导航" })).getAllByRole("link");
    const selected = links.filter((l) => l.classList.contains("Mui-selected")).map((l) => l.textContent);
    expect(selected).toEqual(["概览"]);
  });
});
