import { render, screen } from "@testing-library/react";
import { DelayText, delayDescription } from "./DelayText";

it("hides a stale number and retains its measurement details on demand", () => {
  const result = { ms: 170, tested_at: 1_700_000_000, stale: true };
  render(<DelayText result={result} kind="proxy" target="工作出口" interfaceName="Ethernet" />);
  expect(screen.getByLabelText("待重测")).toHaveTextContent("待重测");
  expect(screen.queryByLabelText("170 ms")).not.toBeInTheDocument();
  const details = delayDescription({ result, kind: "proxy", target: "工作出口", interfaceName: "Ethernet" });
  expect(details).toContain("当前代理延迟：待重测");
  expect(details).toContain("上次结果：170 ms");
  expect(details).toContain("测试完成：");
  expect(details).toContain("出口网卡：Ethernet");
});

it("does not present a cached success as current when the core is stopped", () => {
  render(<DelayText result={{ ms: 42, tested_at: 1 }} running={false} />);
  expect(screen.getByLabelText("内核未运行")).toBeInTheDocument();
  expect(screen.queryByLabelText("42 ms")).not.toBeInTheDocument();
});

it("distinguishes pending, never tested, and connection failures", () => {
  const { rerender } = render(<DelayText pending />);
  expect(screen.getByLabelText("测量中…")).toBeInTheDocument();
  expect(screen.getByRole("progressbar")).toBeInTheDocument();
  rerender(<DelayText emptyText="未测试" />);
  expect(screen.getByLabelText("未测试")).toBeInTheDocument();
  rerender(<DelayText result={{ ms: null, tested_at: 1, error: "Failed" }} />);
  expect(screen.getByLabelText("连接失败")).toBeInTheDocument();
});
