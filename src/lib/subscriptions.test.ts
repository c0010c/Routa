import type { Subscription } from "../bindings";
import { applicationProblem, subscriptionHost, subscriptionProblem } from "./subscriptions";
it("never exposes credentials or token paths in the source label", () => {
  expect(subscriptionHost("https://user:pass@example.test/token/secret?key=secret")).toBe("example.test");
});
it("does not claim the previous configuration survived a fail-closed outcome", () => {
  expect(applicationProblem({ FailedClosed: { error: "reload failed", uncut: 0 } })).toContain("相关代理已关闭");
  expect(applicationProblem({ FailedKeptPrevious: { error: "reload failed" } })).toContain("仍使用原配置");
  expect(applicationProblem({ NotRunning: { error: "stopped" } })).toBeNull();
});

it("only clears historical apply failures after a complete reload on a running core", () => {
  const apply = { FailedClosed: { error: "failed", uncut: 0 } };
  const sub = { last_result: { Ok: { node_count: 1 } }, info: { history: [{ apply }] } } as Subscription;
  const success = { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } };
  expect(subscriptionProblem(sub)).toContain("相关代理已关闭");
  expect(subscriptionProblem(sub, { core: "Stopped", last_apply: success })).toContain("相关代理已关闭");
  expect(subscriptionProblem(sub, { core: "Running", last_apply: apply })).toContain("相关代理已关闭");
  expect(subscriptionProblem(sub, { core: "Running", last_apply: { Reloaded: { ...success.Reloaded, uncut: 1 } } })).toContain("相关代理已关闭");
  expect(subscriptionProblem(sub, { core: "Running", last_apply: success })).toBeNull();
  expect(subscriptionProblem({ ...sub, last_result: { Failed: { reason: "下载超时" } } }, { core: "Running", last_apply: success })).toBe("下载超时");
});
