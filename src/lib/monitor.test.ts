import type { MonitorStatus, MonitorTarget, NodeMonitorSettings, NodeRef } from "../bindings";
import {
  customMembersFrom,
  droppedMessage,
  failureText,
  intervalLabel,
  targetsForNode,
  withNode,
  withoutNode,
} from "./monitor";

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
  enabled: true, scope: "InUse", members: [], interval_secs: 300, fail_threshold: 2,
  latency_limit_ms: 1000, notify_recovery: true, ...over,
});

describe("monitor helpers", () => {
  it("labels intervals", () => {
    expect(intervalLabel(10)).toBe("10 秒");
    expect(intervalLabel(180)).toBe("3 分钟");
    expect(intervalLabel(3600)).toBe("1 小时");
  });

  it("customMembersFrom drops auto groups and dedupes paths of one node", () => {
    const st = status([target("p1", a), target("p2", a), target("g", null, "工作出口")]);
    expect(customMembersFrom(st)).toEqual({ members: [a], dropped: ["工作出口"] });
  });

  it("withNode enables custom scope when disabled", () => {
    const r = withNode(settings({ enabled: false }), status([]), a);
    expect(r.settings).toMatchObject({ enabled: true, scope: "Custom", members: [a] });
  });

  it("withNode adds to the in-use nodes", () => {
    const r = withNode(settings(), status([target("p1", a), target("g", null, "工作出口")]), b);
    expect(r.settings.scope).toBe("Custom");
    expect(r.settings.members).toEqual([a, b]);
    expect(r.dropped).toEqual(["工作出口"]);
  });

  it("withNode appends in custom scope without duplicating", () => {
    const s = settings({ scope: "Custom", members: [a] });
    expect(withNode(s, status([]), b).settings.members).toEqual([a, b]);
    expect(withNode(s, status([]), a).settings.members).toEqual([a]);
  });

  it("withoutNode expands groups through status targets", () => {
    const s = settings({ scope: "Custom", members: [{ group: { Subscription: "s1" } }, c] });
    const st = status([target("p1", a), target("p2", b), target("p3", c)]);
    const r = withoutNode(s, st, b);
    expect(r.settings.scope).toBe("Custom");
    expect(r.settings.members).toEqual([a, c]);
  });

  it("finds targets for a node", () => {
    const st = status([target("p1", a), target("p2", b), target("p3", a)]);
    expect(targetsForNode(st, a).map((t) => t.proxy)).toEqual(["p1", "p3"]);
  });

  it("describes failures and dropped exits", () => {
    expect(failureText("Timeout")).toBe("超时");
    expect(failureText("Failed")).toBe("连接失败");
    expect(failureText({ OverLimit: 1280 })).toBe("延迟 1280 ms 超过上限");
    expect(droppedMessage(["工作出口"])).toBe("自动选择的出口「工作出口」不能加入指定节点，已不再监测");
    expect(droppedMessage(["A", "B"])).toBe("自动选择的出口「A」、「B」不能加入指定节点，已不再监测");
  });
});
