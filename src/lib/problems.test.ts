import { describe, expect, it } from "vitest";
import type { Model, RuntimeStatus } from "../bindings";
import { collectProblems } from "./problems";

const model: Model = {
  subscriptions: [],
  exits: [
    { id: "e1", name: "香港", kind: "Direct", interface: null },
    { id: "e2", name: "日本", kind: "Direct", interface: null },
    {
      id: "e3",
      name: "香港自动",
      kind: { Group: { strategy: "UrlTest", members: [], selected: null } },
      interface: null,
    },
  ],
  listeners: [
    { id: "l1", name: "办公", listener_type: "Mixed", listen: "0.0.0.0", port: 7890, auth: null, exit: "e1", user_enabled: true },
    { id: "l2", name: "下载", listener_type: "Mixed", listen: "0.0.0.0", port: 7891, auth: null, exit: "e3", user_enabled: true },
    { id: "l3", name: "备用", listener_type: "Mixed", listen: "0.0.0.0", port: 7892, auth: null, exit: null, user_enabled: true },
  ],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "u" },
  manual_nodes: [],
  node_groups: [],
  process_routing: { enabled: false, on_routa_exit: "Block", rules: [] },
};

const status: RuntimeStatus = {
  core: "Running",
  last_apply: null,
  health: {
    exits: {
      e1: { status: "Ok", effective_members: [], missing_members: [], interface: null },
      e2: { status: "NodeMissing", effective_members: [], missing_members: [], interface: null },
    },
    listeners: { l1: "Active", l2: { ExitBroken: { exit_status: "NodeMissing" } }, l3: "NoExit" },
  },
  delays: { 香港: { ms: 123, tested_at: 0 } },
  traffic: null,
  interfaces: [],
  failed_listeners: [],
  effective_listeners: ["l1"],
  startup_error: null,
  save_error: null,
  tailscale: {},
  tailscale_exit_errors: {},
  tailscale_exit_retrying: {},
  core_stop_reason: null,
  core_recovering: false,
};

describe("collectProblems", () => {
  const healthy: RuntimeStatus = { ...status, health: { exits: {}, listeners: {} } };
  it("reports nothing for a running core with a clean reload", () =>
    expect(
      collectProblems(model, { ...healthy, last_apply: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } }),
    ).toEqual([]));
  it("reports a stopped or crashed core", () => {
    expect(collectProblems(model, { ...healthy, core: "Stopped" })).toEqual(["mihomo 已停止"]);
    expect(collectProblems(model, { ...healthy, core: { Crashed: { exit_code: 3 } } })).toEqual([
      "mihomo 已崩溃（退出码 3）",
    ]);
  });
  it("reports every non-Reloaded last apply", () => {
    const of = (last_apply: RuntimeStatus["last_apply"]) => collectProblems(model, { ...healthy, last_apply });
    expect(of({ FailedKeptPrevious: { error: "a" } })).toEqual(["已保存，但未生效，目前仍使用原配置"]);
    expect(of({ FailedClosed: { error: "b", uncut: 0 } })).toEqual(["已保存，但未生效，代理端口已关闭，请检查配置后手动重试"]);
    expect(of({ FailedCoreStopped: { error: "c" } })).toEqual(["已保存，但未生效，内核已停止，请检查配置后手动重试"]);
    expect(of({ NotRunning: { error: "d" } })).toEqual(["已保存，但内核未能启动，请检查设置后手动重试"]);
  });
});
