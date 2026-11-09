import type { Model, MonitorStatus, MonitorTarget, NodeMonitorSettings, NodeRef } from "../bindings";
import { failureText, intervalLabel, skippedMessage, targetsForNode, withNode, withoutNode } from "./monitor";

const a: NodeRef = { subscription_id: "s1", name: "A" };
const b: NodeRef = { subscription_id: "s1", name: "B" };
const c: NodeRef = { manual_id: "m1" };

const target = (proxy: string, node: NodeRef | null, label = proxy): MonitorTarget => ({
  proxy, node, label, used_by: [], fails: 0, down: false, down_since: null, last_ms: null,
  last_failure: null, probed: false, not_applied: false,
});
const status = (targets: MonitorTarget[]): MonitorStatus => ({
  last_run: null, next_run: null, probing: false, paused_reason: null, network_down: false, targets,
});
const settings = (over: Partial<NodeMonitorSettings> = {}): NodeMonitorSettings => ({
  enabled: true, members: [], interval_secs: 300, fail_threshold: 2,
  latency_limit_ms: 1000, notify_recovery: true, ...over,
});
const model = {
  subscriptions: [{ id: "s1", nodes: [{ name: "A" }, { name: "B" }] }],
  manual_nodes: [{ id: "m1", name: "M", groups: [], kind: { Proxy: { raw: {} } } }],
  node_groups: [],
} as unknown as Model;
const group = { group: { Subscription: "s1" } };

describe("monitor helpers", () => {
  it("labels intervals", () => {
    expect(intervalLabel(10)).toBe("10 秒");
    expect(intervalLabel(180)).toBe("3 分钟");
    expect(intervalLabel(3600)).toBe("1 小时");
  });

  it("withNode turns monitoring on with the in-use nodes first", () => {
    expect(withNode(settings({ enabled: false }), b, [a]).members).toEqual([a, b]);
    expect(withNode(settings({ enabled: false }), a, [a]).members).toEqual([a]);
    expect(withNode(settings({ enabled: false }), a, [a]).enabled).toBe(true);
  });

  it("withNode keeps existing members and ignores in-use nodes", () => {
    const s = settings({ enabled: false, members: [c] });
    expect(withNode(s, b, [a]).members).toEqual([c, b]);
    expect(withNode(settings({ members: [a] }), b, [c]).members).toEqual([a, b]);
    expect(withNode(settings({ members: [a] }), a, []).members).toEqual([a]);
  });

  it("withoutNode drops a node member and leaves the rest", () => {
    const s = settings({ members: [group, a, c] });
    expect(withoutNode(settings({ members: [b, c] }), model, b).members).toEqual([c]);
    // A group that does not contain the node stays a group.
    expect(withoutNode(s, model, c).members).toEqual([group, a]);
  });

  it("withoutNode expands only the group containing the node", () => {
    const custom = { group: { Custom: "g" } };
    const s = settings({ members: [group, custom, c] });
    expect(withoutNode(s, model, b).members).toEqual([a, custom, c]);
  });

  it("finds targets for a node", () => {
    const st = status([target("p1", a), target("p2", b), target("p3", a)]);
    expect(targetsForNode(st, a).map((t) => t.proxy)).toEqual(["p1", "p3"]);
  });

  it("describes failures and dropped exits", () => {
    expect(failureText("Timeout")).toBe("超时");
    expect(failureText("Failed")).toBe("连接失败");
    expect(failureText({ OverLimit: 1280 })).toBe("延迟 1280 ms 超过上限");
    expect(skippedMessage(["工作出口"])).toBe("自动选择的出口「工作出口」没有固定节点，未加入监测");
    expect(skippedMessage(["A", "B"])).toBe("自动选择的出口「A」、「B」没有固定节点，未加入监测");
  });
});
