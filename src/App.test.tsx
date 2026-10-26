import { render, screen, within } from "@testing-library/react";
import App from "./App";

describe("App", () => {
  it("renders the five navigation items", () => {
    render(<App />);
    const nav = screen.getByRole("navigation", { name: "主导航" });
    for (const label of ["概览", "Listener", "出口", "节点", "设置"]) {
      expect(within(nav).getByText(label)).toBeInTheDocument();
    }
    expect(within(nav).getAllByRole("link")).toHaveLength(5);
  });
});
