import { render, screen } from "@testing-library/react";
import type { MonitorTarget } from "../bindings";
import { MonitorMark } from "./MonitorMark";

const target = (over: Partial<MonitorTarget> = {}): MonitorTarget => ({
  proxy: "订阅/甲", node: { subscription_id: "s1", name: "甲" }, label: "甲", used_by: [],
  fails: 0, down: false, down_since: null, last_ms: 48, last_failure: null, probed: true, not_applied: false,
  ...over,
});

describe("MonitorMark", () => {
  it("renders nothing without targets", () => {
    const { container } = render(<MonitorMark targets={[]} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows the monitoring mark for healthy targets", () => {
    render(<MonitorMark targets={[target()]} />);
    expect(screen.getByLabelText("监测中")).toHaveTextContent("监测");
  });

  it("shows the unavailable mark when any target is down", () => {
    render(<MonitorMark targets={[target(), target({ down: true, fails: 3 })]} />);
    expect(screen.getByLabelText("监测：不可用")).toHaveTextContent("不可用");
  });
});
