import { act, render, screen } from "@testing-library/react";
import { MemoryRouter, useLocation, useNavigationType } from "react-router-dom";
import { clearMonitorFocus, setMonitorFocus, useMonitorFocus } from "../api/monitorFocus";
import { useMonitorFocusNavigation } from "./Layout";

function Probe() {
  useMonitorFocusNavigation();
  const loc = useLocation();
  const type = useNavigationType();
  const focus = useMonitorFocus();
  return (
    <>
      <div data-testid="loc">{loc.pathname + loc.search}</div>
      <div data-testid="nav">{type}</div>
      <div data-testid="focus">{focus === null ? "none" : JSON.stringify(focus)}</div>
    </>
  );
}

function setup(path: string) {
  render(
    <MemoryRouter initialEntries={[path]}>
      <Probe />
    </MemoryRouter>,
  );
}

describe("useMonitorFocusNavigation", () => {
  beforeEach(() => clearMonitorFocus());
  afterEach(() => clearMonitorFocus());

  it("listeners focus opens the proxies page and is cleared", () => {
    setup("/");
    act(() => setMonitorFocus("Listeners"));
    expect(screen.getByTestId("loc")).toHaveTextContent("/listeners");
    expect(screen.getByTestId("nav")).toHaveTextContent("PUSH");
    expect(screen.getByTestId("focus")).toHaveTextContent("none");
  });

  it("network focus opens network settings and is cleared", () => {
    setup("/");
    act(() => setMonitorFocus("Settings"));
    expect(screen.getByTestId("loc")).toHaveTextContent("/settings?tab=network");
    expect(screen.getByTestId("focus")).toHaveTextContent("none");
  });

  it("node focus on the node page replaces instead of pushing", () => {
    setup("/nodes");
    act(() => setMonitorFocus({ Node: { manual_id: "m1" } }));
    expect(screen.getByTestId("loc")).toHaveTextContent("/nodes");
    expect(screen.getByTestId("nav")).toHaveTextContent("REPLACE");
    // Left for the node page to consume.
    expect(screen.getByTestId("focus")).not.toHaveTextContent("none");
  });

  it("node focus from another page pushes /nodes", () => {
    setup("/settings");
    act(() => setMonitorFocus({ Node: { manual_id: "m1" } }));
    expect(screen.getByTestId("loc")).toHaveTextContent("/nodes");
    expect(screen.getByTestId("nav")).toHaveTextContent("PUSH");
  });

  it("listeners focus while on the proxies page replaces", () => {
    setup("/listeners");
    act(() => setMonitorFocus("Listeners"));
    expect(screen.getByTestId("nav")).toHaveTextContent("REPLACE");
  });
});
